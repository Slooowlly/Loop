import { renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import useCalendarData from "./useCalendarData";
import useCareerStore from "../../stores/useCareerStore";

// O hook de dados da aba Calendário: o maior dos hooks sem teste, e o único ponto onde a
// tela decide o que é "próximo". A vistoria de 10/08/2026 marcou `calendar/` como diretório
// com 7 fontes e zero testes.
//
// O caso que mais importa aqui é o filtro do `upcoming`. A regra não é "o que não foi
// concluído": uma etapa pendente com data VENCIDA (pulada, cancelada, save antigo) não é
// futura, e se ela encabeçar a lista a aba passa a apontar o jogador para uma corrida que
// não vai acontecer. O comentário no código explica a decisão; até agora nada a travava.

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const CATEGORIA = "gt3";

/// Uma etapa como o backend entrega.
function etapa(extra = {}) {
  return {
    id: `R${extra.round ?? 1}`,
    display_date: "2026-03-14",
    status: "Pendente",
    categoria: CATEGORIA,
    track_name: "Interlagos",
    duracao_corrida_min: 45,
    clima: "Dry",
    season_phase: "BlocoRegular",
    ...extra,
  };
}

/// Põe o store no estado mínimo que o hook lê, sem tocar nos slices reais.
function montarStore(patch = {}) {
  useCareerStore.setState({
    careerId: "C1",
    playerTeam: { categoria: CATEGORIA },
    nextRace: null,
    season: { ano: 2026, fase: "BlocoRegular", rodada_atual: 1 },
    acceptedSpecialOffer: null,
    calendarDisplayDate: null,
    temporalSummary: null,
    ...patch,
  });
}

/// Responde o `get_calendar_for_category` por categoria; o que não estiver no mapa vem vazio.
function responder(porCategoria) {
  invoke.mockImplementation((cmd, args) => {
    if (cmd !== "get_calendar_for_category") return Promise.resolve([]);
    return Promise.resolve(porCategoria[args.category] ?? []);
  });
}

const estadoOriginal = useCareerStore.getState();

beforeEach(() => {
  invoke.mockReset();
  useCareerStore.setState(estadoOriginal, true);
});

describe("carga", () => {
  it("não chama o backend sem carreira ou sem equipe", async () => {
    montarStore({ careerId: null });
    responder({});
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(invoke).not.toHaveBeenCalled();
    expect(result.current.displayedCalendar).toEqual([]);
  });

  it("busca a categoria do jogador e completa a categoria de cada etapa", async () => {
    // O backend pode devolver a etapa sem o campo `categoria`; a tela agrupa por ele, e uma
    // etapa sem categoria some do filtro de linha.
    montarStore();
    responder({ [CATEGORIA]: [etapa({ categoria: undefined })] });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.displayedCalendar).toHaveLength(1));
    expect(result.current.displayedCalendar[0].categoria).toBe(CATEGORIA);
  });

  it("só busca o calendário especial nas fases legadas, e junta as duas listas", async () => {
    montarStore({
      season: { ano: 2026, fase: "BlocoEspecial", rodada_atual: 3 },
      acceptedSpecialOffer: { special_category: "endurance" },
    });
    responder({ [CATEGORIA]: [etapa()], endurance: [etapa({ id: "E1", categoria: "endurance" })] });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.displayedCalendar).toHaveLength(2));
    expect(result.current.isLegacyCalendar).toBe(true);
    const categorias = result.current.displayedCalendar.map((r) => r.categoria);
    expect(categorias).toContain("endurance");
  });

  it("uma falha nas OUTRAS categorias não derruba o calendário do jogador", async () => {
    // As demais categorias entram num segundo lote, depois de a tela já ter desenhado. Um
    // erro ali é decoração perdida, e não motivo para a aba inteira ficar vazia.
    montarStore();
    invoke.mockImplementation((cmd, args) => {
      if (args.category === CATEGORIA) return Promise.resolve([etapa()]);
      return Promise.reject(new Error("sem calendário"));
    });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.displayedCalendar).toHaveLength(1));
    await waitFor(() => expect(result.current.otherCategoryRacesByDate).toEqual({}));
    expect(result.current.error).toBe("");
  });
});

describe("upcoming", () => {
  const comHoje = (dia) => ({ calendarDisplayDate: dia });

  it("descarta etapa concluída e ordena por data", async () => {
    montarStore(comHoje("2026-01-01"));
    responder({
      [CATEGORIA]: [
        etapa({ id: "c", display_date: "2026-05-10" }),
        etapa({ id: "a", display_date: "2026-03-14" }),
        etapa({ id: "feita", display_date: "2026-04-01", status: "Concluida" }),
        etapa({ id: "b", display_date: "2026-04-20" }),
      ],
    });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.upcoming.length).toBeGreaterThan(0));
    expect(result.current.upcoming.map((r) => r.id)).toEqual(["a", "b", "c"]);
  });

  it("descarta etapa pendente com data já vencida", async () => {
    // ESTA é a regressão que o arquivo trava. Uma etapa pulada fica `Pendente` para sempre;
    // sem o corte por data ela lidera "próximos" e a aba aponta para o passado.
    montarStore(comHoje("2026-04-15"));
    responder({
      [CATEGORIA]: [
        etapa({ id: "pulada", display_date: "2026-03-14" }),
        etapa({ id: "futura", display_date: "2026-05-10" }),
      ],
    });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.upcoming.length).toBeGreaterThan(0));
    expect(result.current.upcoming.map((r) => r.id)).toEqual(["futura"]);
  });

  it("mantém a etapa de HOJE na lista", async () => {
    // O corte é `>=`: a corrida de hoje ainda não aconteceu.
    montarStore(comHoje("2026-03-14"));
    responder({ [CATEGORIA]: [etapa({ id: "hoje", display_date: "2026-03-14" })] });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.upcoming.length).toBe(1));
    expect(result.current.upcoming[0].id).toBe("hoje");
  });

  it("sem data atual, lista tudo em vez de esconder a temporada", async () => {
    montarStore({ calendarDisplayDate: null, temporalSummary: null });
    responder({
      [CATEGORIA]: [
        etapa({ id: "velha", display_date: "2020-01-01" }),
        etapa({ id: "nova", display_date: "2030-01-01" }),
      ],
    });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.upcoming.length).toBe(2));
  });

  it("usa a data do resumo temporal quando não há data de exibição", async () => {
    montarStore({
      calendarDisplayDate: null,
      temporalSummary: { current_display_date: "2026-04-15" },
    });
    responder({
      [CATEGORIA]: [
        etapa({ id: "pulada", display_date: "2026-03-14" }),
        etapa({ id: "futura", display_date: "2026-05-10" }),
      ],
    });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.upcoming.length).toBe(1));
    expect(result.current.upcoming[0].id).toBe("futura");
  });

  it("não filtra por data fora da aba de calendário", async () => {
    // `currentDateParts` só é calculado quando a aba está ativa; o resto da tela consome
    // `upcoming` sem esse contexto e não pode receber uma lista podada pela metade.
    montarStore(comHoje("2026-04-15"));
    responder({
      [CATEGORIA]: [
        etapa({ id: "pulada", display_date: "2026-03-14" }),
        etapa({ id: "futura", display_date: "2026-05-10" }),
      ],
    });
    const { result } = renderHook(() => useCalendarData("nextRace"));
    await waitFor(() => expect(result.current.upcoming.length).toBe(2));
    expect(result.current.currentDateParts).toBeNull();
  });
});

describe("mapas por data e estatísticas", () => {
  it("indexa a etapa do jogador pela data e marca a especial", async () => {
    montarStore();
    responder({
      [CATEGORIA]: [
        etapa({ id: "regular", display_date: "2026-03-14" }),
        etapa({ id: "esp", display_date: "2026-06-20", season_phase: "BlocoEspecial" }),
      ],
    });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(Object.keys(result.current.racesByDate)).toHaveLength(2));
    expect(result.current.racesByDate["2026-03-14"]._isSpecialRace).toBe(false);
    expect(result.current.racesByDate["2026-06-20"]._isSpecialRace).toBe(true);
  });

  it("as outras categorias empilham na mesma data em vez de se sobrescrever", async () => {
    // Duas categorias correm no mesmo fim de semana o tempo todo. Se o mapa guardasse só
    // uma, a célula do dia mostraria uma etapa e esconderia a outra.
    montarStore();
    invoke.mockImplementation((cmd, args) => {
      if (args.category === CATEGORIA) return Promise.resolve([]);
      if (args.category === "gt4") {
        return Promise.resolve([etapa({ id: "g4", categoria: "gt4", display_date: "2026-03-14" })]);
      }
      if (args.category === "endurance") {
        return Promise.resolve([
          etapa({ id: "en", categoria: "endurance", display_date: "2026-03-14" }),
        ]);
      }
      return Promise.resolve([]);
    });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() =>
      expect(result.current.otherCategoryRacesByDate["2026-03-14"]?.length).toBe(2),
    );
  });

  it("conta total, concluídas, países, duração, molhadas e especiais", async () => {
    montarStore();
    responder({
      [CATEGORIA]: [
        etapa({ id: "1", track_name: "Interlagos", duracao_corrida_min: 45, status: "Concluida" }),
        etapa({ id: "2", track_name: "Interlagos", duracao_corrida_min: 30, clima: "Wet" }),
        etapa({ id: "3", track_name: "Spa", duracao_corrida_min: 60, clima: "HeavyRain" }),
        etapa({ id: "4", track_name: "Spa", duracao_corrida_min: 0, season_phase: "BlocoEspecial" }),
      ],
    });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.stats.total).toBe(4));
    expect(result.current.stats).toMatchObject({
      total: 4,
      done: 1,
      durationMin: 135,
      wet: 2,
      specials: 1,
    });
    // Duas pistas, dois países distintos. Interlagos e Spa precisam estar no mapa de países;
    // se um sumir dali a estatística cai em silêncio para 1.
    expect(result.current.stats.countries).toBe(2);
  });
});

describe("ano da temporada", () => {
  it("prefere o ano da temporada", async () => {
    montarStore({ season: { ano: 2031, fase: "BlocoRegular", rodada_atual: 1 } });
    responder({ [CATEGORIA]: [etapa({ display_date: "2026-03-14" })] });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.displayedCalendar).toHaveLength(1));
    expect(result.current.seasonYear).toBe(2031);
  });

  it("cai no ano da primeira etapa quando a temporada não traz ano", async () => {
    montarStore({ season: { fase: "BlocoRegular", rodada_atual: 1 } });
    responder({ [CATEGORIA]: [etapa({ display_date: "2029-03-14" })] });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.seasonYear).toBe(2029));
  });
});

describe("próxima etapa", () => {
  it("acha a entrada do calendário que corresponde à próxima corrida do store", async () => {
    montarStore({ nextRace: { id: "R7" } });
    responder({ [CATEGORIA]: [etapa({ id: "R7", display_date: "2026-07-01" })] });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.nextRaceEntry?.id).toBe("R7"));
  });

  it("devolve null quando a próxima corrida não está no calendário carregado", async () => {
    montarStore({ nextRace: { id: "de-outra-categoria" } });
    responder({ [CATEGORIA]: [etapa({ id: "R1" })] });
    const { result } = renderHook(() => useCalendarData("calendar"));
    await waitFor(() => expect(result.current.displayedCalendar).toHaveLength(1));
    expect(result.current.nextRaceEntry).toBeNull();
  });
});
