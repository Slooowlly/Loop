import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

import { readCachedPreRaceStandings } from "../../components/race/nextrace/nextRaceHelpers";
import useCareerStore from "../useCareerStore";
import { CHAVES_DE_CACHE_DO_SAVE } from "./helpers";
import { initialState } from "./state";

// TROCA DE CARREIRA. O jogador sai para o menu, abre outro save, e o store é o MESMO objeto
// em memória: o que a carreira anterior deixou lá continua lá até alguém apagar.
//
// O que torna o vazamento invisível é a numeração dos IDs. R001 é a primeira etapa de toda
// carreira e P001 o primeiro piloto de toda carreira, então um cache chaveado só por
// `raceId` BATE na carreira nova e a tela abre com dado do outro save sem nada acusar. Os
// testes daqui montam justamente esse par de saves colidentes.

const estado = () => useCareerStore.getState();

/// Payload de `load_career` — o mínimo que o slice lê.
function carreira(careerId, { raceId = "R001", categoria = "gt3" } = {}) {
  return {
    career_id: careerId,
    difficulty: "Normal",
    player: { id: "P001", nome: `Jogador da ${careerId}` },
    player_team: { id: "T001", categoria, nome: "Equipe" },
    season: { id: "S001", numero: 1, ano: 2026, fase: "Temporada" },
    next_race: { id: raceId, rodada: 1, track_name: "Interlagos", display_date: "2026-03-01" },
    total_drivers: 200,
    total_teams: 60,
  };
}

/// Backend que responde `load_career` com a carreira pedida e null no resto, salvo override.
function comBackend(porCarreira, overrides = {}) {
  invoke.mockImplementation((comando, args) => {
    if (comando in overrides) {
      const valor = overrides[comando];
      return typeof valor === "function"
        ? Promise.resolve(valor(args))
        : Promise.resolve(valor);
    }
    if (comando === "load_career") return Promise.resolve(porCarreira[args.careerId] ?? null);
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  invoke.mockReset();
  useCareerStore.setState({ ...initialState });
});

describe("caches locais ao save morrem na troca", () => {
  it("as duas carreiras têm a etapa R001, e o cache da primeira não sobrevive na segunda", async () => {
    // Carreira A na etapa R001, com o retrato da etapa e a prévia por IA já em cache.
    useCareerStore.setState({
      careerId: "C_A",
      nextRace: { id: "R001" },
      preRaceStandings: { careerId: "C_A", raceId: "R001", driverStandings: [{ id: "D_A" }] },
      preRaceAi: { careerId: "C_A", raceId: "R001", narrative: "Prévia da A", teamVoice: "Voz" },
      playerInterests: { nemesis: { id: "P001", nome: "Nemesis da A" }, rivais: [] },
      temporalSummary: { current_display_date: "2026-03-01" },
      calendarDisplayDate: "2026-03-01",
    });

    comBackend({ C_B: carreira("C_B", { raceId: "R001" }) });
    await estado().loadCareer("C_B");

    const s = estado();
    expect(s.careerId).toBe("C_B");
    // A etapa da carreira nova tem o MESMO id da que estava em cache.
    expect(s.nextRace.id).toBe("R001");
    expect(s.preRaceStandings).toBeNull();
    expect(s.preRaceAi).toBeNull();
  });

  it("toda chave de cache do save volta ao valor do boot na troca", async () => {
    const sujo = {};
    for (const chave of CHAVES_DE_CACHE_DO_SAVE) {
      sujo[chave] = { deQuem: "C_A" };
    }
    useCareerStore.setState({ careerId: "C_A", ...sujo });

    comBackend({ C_B: carreira("C_B") });
    await estado().loadCareer("C_B");

    const s = estado();
    for (const chave of CHAVES_DE_CACHE_DO_SAVE) {
      // `playerInterests` é rebuscado no fim do `loadCareer`; o resto fica no valor do boot.
      if (chave === "playerInterests") continue;
      expect(s[chave], `chave ${chave}`).toEqual(initialState[chave]);
    }
  });

  it("recarregar a MESMA carreira preserva o cache — o pós-corrida depende disso", async () => {
    useCareerStore.setState({
      careerId: "C_A",
      nextRace: { id: "R001" },
      championOverlay: { drivers: [{ id: "D1" }] },
      preRaceStandings: { careerId: "C_A", raceId: "R001", driverStandings: [] },
    });

    comBackend({ C_A: carreira("C_A", { raceId: "R001" }) });
    await estado().loadCareer("C_A");

    expect(estado().championOverlay).toEqual({ drivers: [{ id: "D1" }] });
    expect(estado().preRaceStandings?.careerId).toBe("C_A");
  });

  it("cache da carreira anterior que escape do reset é RECUSADO na leitura", async () => {
    // O cinto falhou de propósito: o cache de C_A está de pé com a carreira C_B aberta,
    // e a etapa tem o mesmo id nas duas. Só a chave (careerId + raceId) segura.
    useCareerStore.setState({
      careerId: "C_B",
      nextRace: { id: "R001" },
      preRaceStandings: { careerId: "C_A", raceId: "R001", driverStandings: [{ id: "D_A" }] },
    });

    expect(readCachedPreRaceStandings()).toBeNull();
  });

  it("e o prefetch rebusca em vez de aceitar o cache do outro save", async () => {
    useCareerStore.setState({
      careerId: "C_B",
      player: { id: "P001" },
      playerTeam: { id: "T001", categoria: "gt3" },
      season: { id: "S001", numero: 1 },
      nextRace: { id: "R001", display_date: "2026-03-01" },
      preRaceStandings: { careerId: "C_A", raceId: "R001", driverStandings: [{ id: "D_A" }] },
      preRaceAi: { careerId: "C_A", raceId: "R001", narrative: "Prévia da A", teamVoice: "Voz" },
    });
    comBackend({}, { get_drivers_by_category: [{ id: "D_B" }] });

    await estado().prefetchPreRaceBriefing();

    const cache = estado().preRaceStandings;
    expect(cache.careerId).toBe("C_B");
    expect(cache.driverStandings).toEqual([{ id: "D_B" }]);
  });
});

describe("playerInterests na troca", () => {
  it("as duas carreiras têm o piloto P001, e o nemesis da primeira não decora o da segunda", async () => {
    useCareerStore.setState({
      careerId: "C_A",
      playerInterests: { nemesis: { id: "P001", nome: "Nemesis da A" }, rivais: [] },
    });

    comBackend(
      { C_B: carreira("C_B") },
      { get_player_interests: { nemesis: null, rivais: [{ id: "P001", nome: "Rival da B" }] } },
    );
    await estado().loadCareer("C_B");

    const interests = estado().playerInterests;
    expect(interests.nemesis).toBeNull();
    expect(interests.rivais).toEqual([{ id: "P001", nome: "Rival da B" }]);
  });

  it("interesses que falham não conservam o valor anterior — ficam ausentes", async () => {
    useCareerStore.setState({
      careerId: "C_A",
      playerInterests: { nemesis: { id: "P001", nome: "Nemesis da A" }, rivais: [] },
    });

    comBackend(
      { C_B: carreira("C_B") },
      { get_player_interests: () => Promise.reject(new Error("tabela de interesses vazia")) },
    );
    await estado().loadCareer("C_B");
    // `loadPlayerInterests` é fire-and-forget dentro do `loadCareer`: espera o microtask.
    await Promise.resolve();
    await Promise.resolve();

    expect(estado().playerInterests).toBeNull();
  });

  it("a falha sozinha já apaga o valor anterior, sem depender da limpeza da troca", async () => {
    // Mesma carreira, sem `loadCareer` no meio: o que apaga aqui é o próprio catch. Sem
    // ele, o nemesis carregado antes ficaria de pé para sempre.
    useCareerStore.setState({
      careerId: "C_A",
      playerInterests: { nemesis: { id: "P001", nome: "Nemesis antigo" }, rivais: [] },
    });
    comBackend({}, { get_player_interests: () => Promise.reject(new Error("consulta falhou")) });

    await estado().loadPlayerInterests();

    expect(estado().playerInterests).toBeNull();
  });

  it("sair para o menu limpa os interesses junto com o resto", () => {
    useCareerStore.setState({
      careerId: "C_A",
      playerInterests: { nemesis: { id: "P001", nome: "Nemesis da A" }, rivais: [] },
    });

    estado().clearCareer();

    expect(estado().playerInterests).toBeNull();
    expect(estado().careerId).toBeNull();
    // A geração é a única chave que NÃO volta ao boot: zerá-la faria o que está em voo
    // achar que nada mudou e voltar a escrever.
    expect(estado().careerGeneration).toBeGreaterThan(0);
  });
});

describe("troca de carreira durante a animação do calendário", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("a animação da carreira antiga não escreve mais nada na carreira nova", async () => {
    useCareerStore.setState({
      careerId: "C_A",
      player: { id: "P001" },
      playerTeam: { id: "T001", categoria: "gt3" },
      season: { id: "S001", numero: 1 },
      nextRace: { id: "R001", display_date: "2026-03-10" },
      temporalSummary: {
        current_display_date: "2026-03-01",
        next_event_display_date: "2026-03-10",
        days_until_next_event: 9,
        pending_in_phase: 1,
      },
      calendarDisplayDate: "2026-03-01",
      displayDaysUntilNextEvent: 9,
    });
    comBackend({ C_B: carreira("C_B", { raceId: "R001" }) });

    const animacao = estado().startCalendarAdvance();
    // Um tique já rodou: a animação está viva e escrevendo.
    await vi.advanceTimersByTimeAsync(300);
    expect(estado().isCalendarAdvancing).toBe(true);

    // O jogador sai para o menu e abre outro save NO MEIO da animação.
    estado().clearCareer();
    await estado().loadCareer("C_B");
    const dataNaTroca = estado().calendarDisplayDate;

    // O laço antigo continua tiquetaqueando até o fim.
    await vi.advanceTimersByTimeAsync(10000);
    await animacao;

    const s = estado();
    expect(s.careerId).toBe("C_B");
    // Sem o token de aborto, o laço carimbaria 2026-03-10 e abriria a Sala de Estratégia
    // da carreira ANTERIOR por cima da nova.
    expect(s.calendarDisplayDate).toBe(dataNaTroca);
    expect(s.calendarDisplayDate).not.toBe("2026-03-10");
    expect(s.showRaceBriefing).toBe(false);
    expect(s.isCalendarAdvancing).toBe(false);
  });

  it("sem troca, a animação chega ao fim e abre a Sala de Estratégia", async () => {
    useCareerStore.setState({
      careerId: "C_A",
      playerTeam: { id: "T001", categoria: "gt3" },
      season: { id: "S001", numero: 1 },
      nextRace: { id: "R001", display_date: "2026-03-10" },
      temporalSummary: {
        current_display_date: "2026-03-01",
        next_event_display_date: "2026-03-10",
        days_until_next_event: 9,
        pending_in_phase: 1,
      },
      calendarDisplayDate: "2026-03-01",
      displayDaysUntilNextEvent: 9,
    });
    comBackend({});

    const animacao = estado().startCalendarAdvance();
    await vi.advanceTimersByTimeAsync(10000);
    await animacao;

    const s = estado();
    expect(s.calendarDisplayDate).toBe("2026-03-10");
    expect(s.displayDaysUntilNextEvent).toBe(0);
    expect(s.showRaceBriefing).toBe(true);
    expect(s.isCalendarAdvancing).toBe(false);
  });
});

// OPERAÇÕES DE CORRIDA EM VOO. Simular o fim de semana, importar o resultado do iRacing e
// reabrir uma classificação salva são chamadas longas ao Rust. Todas terminam gravando
// `lastRaceResult` e abrindo a tela de resultado — na carreira que estiver aberta QUANDO a
// resposta chegar, que não é necessariamente a que pediu. O token de geração corta isso.

/// Promise que só resolve quando o teste mandar.
function promessaControlada() {
  let resolver;
  let rejeitar;
  const promessa = new Promise((res, rej) => {
    resolver = res;
    rejeitar = rej;
  });
  return { promessa, resolver, rejeitar };
}

/// Payload de `simulate_race_weekend` — o mínimo que o slice lê.
function resultadoDeEtapa(deQuem) {
  return {
    player_race: { posicao: 1, piloto: `Jogador da ${deQuem}` },
    evaluation: { nota: 9 },
    maintenance: null,
    event_repercussion: null,
    other_categories: [{ categoria: "gt4", deQuem }],
  };
}

describe("troca de carreira com operação de corrida em voo", () => {
  it("a etapa simulada na carreira antiga não abre o resultado na nova", async () => {
    const emVoo = promessaControlada();
    useCareerStore.setState({
      careerId: "C_A",
      nextRace: { id: "R001", thematic_slot: null },
    });
    comBackend(
      { C_B: carreira("C_B", { raceId: "R001" }) },
      { simulate_race_weekend: () => emVoo.promessa },
    );

    const simulacao = estado().simulateRace();
    expect(estado().isSimulating).toBe(true);

    // O jogador sai para o menu e abre outro save NO MEIO da simulação.
    estado().clearCareer();
    await estado().loadCareer("C_B");

    emVoo.resolver(resultadoDeEtapa("C_A"));
    await simulacao;

    const s = estado();
    expect(s.careerId).toBe("C_B");
    // Sem o token, a etapa da C_A carimbaria o resultado e abriria a tela na C_B.
    expect(s.lastRaceResult).toBeNull();
    expect(s.lastRaceId).toBeNull();
    expect(s.lastRaceEvaluation).toBeNull();
    expect(s.otherCategoriesResult).toBeNull();
    expect(s.showResult).toBe(false);
    expect(s.resultIsFresh).toBe(false);
    expect(s.isDirty).toBe(false);
  });

  it("a etapa que FALHA na carreira antiga não pinta erro na nova", async () => {
    const emVoo = promessaControlada();
    useCareerStore.setState({ careerId: "C_A", nextRace: { id: "R001" } });
    comBackend(
      { C_B: carreira("C_B", { raceId: "R001" }) },
      { simulate_race_weekend: () => emVoo.promessa },
    );

    const simulacao = estado().simulateRace();
    estado().clearCareer();
    await estado().loadCareer("C_B");

    emVoo.rejeitar(new Error("motor da simulação caiu"));
    await expect(simulacao).rejects.toThrow("motor da simulação caiu");

    expect(estado().careerId).toBe("C_B");
    expect(estado().error).toBeNull();
  });

  it("sem troca, a etapa simulada abre a tela de resultado como sempre", async () => {
    useCareerStore.setState({ careerId: "C_A", nextRace: { id: "R001" } });
    comBackend({}, { simulate_race_weekend: resultadoDeEtapa("C_A") });

    await estado().simulateRace();

    const s = estado();
    expect(s.lastRaceResult).toEqual({ posicao: 1, piloto: "Jogador da C_A" });
    expect(s.lastRaceId).toBe("R001");
    expect(s.lastRaceOrigem).toBe("simulada");
    expect(s.showResult).toBe(true);
    expect(s.isSimulating).toBe(false);
  });

  it("o import do iRacing disparado na carreira antiga não abre resultado na nova", async () => {
    const emVoo = promessaControlada();
    useCareerStore.setState({ careerId: "C_A", nextRace: { id: "R001" } });
    comBackend(
      { C_B: carreira("C_B", { raceId: "R001" }) },
      { iracing_auto_import_if_ready: () => emVoo.promessa },
    );

    const poll = estado().pollIracingResult();
    expect(estado().iracingImporting).toBe(true);

    estado().clearCareer();
    await estado().loadCareer("C_B");

    emVoo.resolver({
      race_result: { posicao: 3, piloto: "Jogador da C_A" },
      summary: { race_id: "R001", repair_cost: 500 },
      evaluation: { nota: 7 },
      telemetry: { voltas: [] },
    });
    await poll;

    const s = estado();
    expect(s.careerId).toBe("C_B");
    expect(s.lastRaceResult).toBeNull();
    expect(s.lastRaceTelemetry).toBeNull();
    expect(s.iracingRepair).toBeNull();
    expect(s.showResult).toBe(false);
    expect(s.isDirty).toBe(false);
  });

  it("o tique antigo não destrava a bandeira de import da carreira nova", async () => {
    const emVooA = promessaControlada();
    const emVooB = promessaControlada();
    const fila = [emVooA.promessa, emVooB.promessa];
    useCareerStore.setState({ careerId: "C_A", nextRace: { id: "R001" } });
    comBackend(
      { C_B: carreira("C_B", { raceId: "R001" }) },
      { iracing_auto_import_if_ready: () => fila.shift() },
    );

    const pollA = estado().pollIracingResult();
    estado().clearCareer();
    await estado().loadCareer("C_B");

    // A carreira nova começa o próprio poll e ele ainda está em voo.
    const pollB = estado().pollIracingResult();
    expect(estado().iracingImporting).toBe(true);

    // O tique da carreira antiga chega agora.
    emVooA.resolver(null);
    await pollA;

    // Sem o token, o `finally` da C_A abaixaria a bandeira da C_B e o poller da nova
    // dispararia um segundo import por cima do que ainda está rodando.
    expect(estado().iracingImporting).toBe(true);

    emVooB.resolver(null);
    await pollB;
    expect(estado().iracingImporting).toBe(false);
  });

  it("a classificação salva pedida na carreira antiga não abre na nova", async () => {
    const emVoo = promessaControlada();
    useCareerStore.setState({ careerId: "C_A" });
    comBackend(
      { C_B: carreira("C_B", { raceId: "R001" }) },
      { get_saved_race_screen: () => emVoo.promessa },
    );

    const abertura = estado().openSavedRaceScreen("gt3", 1);

    estado().clearCareer();
    await estado().loadCareer("C_B");

    emVoo.resolver({
      race_result: { posicao: 5, piloto: "Jogador da C_A" },
      race_id: "R001",
      evaluation: { nota: 6 },
      telemetry: { voltas: [] },
    });

    await expect(abertura).resolves.toBe(false);

    const s = estado();
    expect(s.careerId).toBe("C_B");
    expect(s.lastRaceResult).toBeNull();
    expect(s.lastRaceEvaluation).toBeNull();
    expect(s.lastRaceTelemetry).toBeNull();
    expect(s.showResult).toBe(false);
  });

  it("sem troca, a classificação salva abre normalmente", async () => {
    useCareerStore.setState({ careerId: "C_A" });
    comBackend(
      {},
      {
        get_saved_race_screen: {
          race_result: { posicao: 5, piloto: "Jogador da C_A" },
          race_id: "R001",
        },
      },
    );

    await expect(estado().openSavedRaceScreen("gt3", 1)).resolves.toBe(true);

    const s = estado();
    expect(s.lastRaceResult).toEqual({ posicao: 5, piloto: "Jogador da C_A" });
    expect(s.lastRaceOrigem).toBe("reaberta");
    expect(s.showResult).toBe(true);
    expect(s.resultIsFresh).toBe(false);
  });
});
