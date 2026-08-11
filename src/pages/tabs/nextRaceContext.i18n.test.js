import { describe, it, expect, afterEach } from "vitest";
import i18n, { DEFAULT_LANGUAGE } from "../../i18n/index.js";
import { buildBriefingContext } from "./nextRaceContext.js";

function withLang(lang, fn) {
  i18n.changeLanguage(lang);
  try {
    return fn();
  } finally {
    i18n.changeLanguage(DEFAULT_LANGUAGE);
  }
}

afterEach(() => i18n.changeLanguage(DEFAULT_LANGUAGE));

// Cenário de campeonato em andamento (não-estreia) para exercitar muitos fatos:
// situação, objetivo, forma, líder, rival direto, rótulo de rivalidade, histórico
// de pista, companheiro, favorito, clima molhado e risco de quebra.
function fixture() {
  return {
    player: { id: "p1", nome: "Ana Costa" },
    playerTeam: { id: "t1", nome: "Alfa Racing", cor_primaria: "#ff0000" },
    season: { id: "s3", ano: 2027, numero: 3, total_rodadas: 12 },
    nextRace: {
      id: "r5",
      track_name: "Monza",
      rodada: 5,
      clima: "Wet",
      temperatura: 22,
      horario: "14:00",
      event_interest: { tier_label: "Alto", display_value: 50000 },
    },
    nextRaceBriefing: {
      track_history: {
        has_data: true,
        starts: 3,
        best_finish: 2,
        dnfs: 1,
        last_finish: 4,
        last_visit_season: 2,
      },
      primary_rival: {
        driver_id: "d1",
        driver_name: "Bea Luz",
        championship_position: 2,
        is_ahead: true,
        gap_points: 8,
        rivalry_label: "O Clássico",
      },
      weekend_stories: [
        { id: 1, title: "Título em jogo", summary: "reta final" },
        { id: 2, title: "Novato surpresa" },
      ],
    },
    driverStandings: [
      { id: "d0", nome: "Leo Prime", is_jogador: false, posicao_campeonato: 1, pontos: 60, equipe_id: "t0", skill: 88, vitorias: 4, podios: 6, results: [{ position: 1 }, { position: 2 }, { position: 1 }] },
      { id: "d1", nome: "Bea Luz", is_jogador: false, posicao_campeonato: 2, pontos: 48, equipe_id: "t2", skill: 84, vitorias: 2, podios: 5, results: [{ position: 2 }, { position: 3 }, { position: 2 }] },
      { id: "p1", nome: "Ana Costa", is_jogador: true, posicao_campeonato: 3, pontos: 40, equipe_id: "t1", skill: 80, vitorias: 1, podios: 3, results: [{ position: 3 }, { position: 5 }, { is_dnf: true }] },
      { id: "d3", nome: "Caio Vaz", is_jogador: false, posicao_campeonato: 4, pontos: 30, equipe_id: "t1", skill: 76, vitorias: 0, podios: 1, results: [{ position: 6 }, { position: 7 }, { position: 5 }] },
    ],
    teamStandings: [
      { id: "t0", nome: "Zero Motors", posicao: 1, pontos: 100 },
      { id: "t1", nome: "Alfa Racing", posicao: 3, pontos: 70 },
    ],
    briefingPhraseHistory: { entries: [] },
    playerInterests: null,
    breakdownForecast: {
      available: true,
      overall_level: "alto",
      parts: [{ part_name: "Motor", level: "alto" }],
    },
  };
}

describe("nextRaceContext i18n (Fase 4 — fatos da prévia)", () => {
  it("baseline PT: cabeçalhos + fatos-chave", () => {
    const { aiFacts } = buildBriefingContext(fixture());
    expect(aiFacts).toContain("CENÁRIO:");
    expect(aiFacts).toContain("Monza");
    expect(aiFacts).toContain("temporada 2027");
    expect(aiFacts).toContain("EIXO DA CORRIDA");
    expect(aiFacts).toContain("Situação no campeonato: 3º lugar");
    expect(aiFacts).toContain("FATOR CLIMA");
    expect(aiFacts).toContain("Companheiro de equipe: Caio Vaz");
    expect(aiFacts).toContain("RISCO DE QUEBRA DE PEÇA");
  });

  it("EN: troca o idioma dos fatos e mantém dados", () => {
    withLang("en-US", () => {
      const { aiFacts } = buildBriefingContext(fixture());
      expect(aiFacts).toContain("SCENARIO:");
      expect(aiFacts).toContain("Monza");
      expect(aiFacts).toContain("2027 season");
      expect(aiFacts).toContain("RACE AXIS");
      expect(aiFacts).toContain("Championship situation: 3rd place");
      expect(aiFacts).toContain("WEATHER FACTOR");
      expect(aiFacts).toContain("Teammate: Caio Vaz");
      expect(aiFacts).toContain("PART FAILURE RISK");
      expect(aiFacts).toContain("Bea Luz is ahead of you by 8 point(s).");
      // Sem sufixo de ordinal quebrado.
      expect(aiFacts).not.toContain("3th");
    });
  });
});

// A direção da comparação com o rival direto é nomeada e recomputada da PRÓPRIA
// tabela: frase ambígua ("à frente por N") já fez a IA inverter quem liderava.
describe("nextRaceContext — direção do rival direto", () => {
  it("rival à frente: frase nomeada com os dois totais e as duas posições", () => {
    const { aiFacts } = buildBriefingContext(fixture());
    expect(aiFacts).toContain(
      "Rival direto: Bea Luz é 2º no campeonato com 48 ponto(s); você é 3º com 40 ponto(s). Bea Luz está à frente de você por 8 ponto(s).",
    );
  });

  it("jogador à frente: a direção vem da tabela mesmo com o flag do backend defasado", () => {
    const data = fixture();
    // Tabela invertida em relação ao resumo do backend (que ainda diz is_ahead=true).
    data.driverStandings[1].pontos = 40;
    data.driverStandings[1].posicao_campeonato = 3;
    data.driverStandings[2].pontos = 48;
    data.driverStandings[2].posicao_campeonato = 2;
    const { aiFacts } = buildBriefingContext(data);
    expect(aiFacts).toContain(
      "Rival direto: Bea Luz é 3º no campeonato com 40 ponto(s); você é 2º com 48 ponto(s). Bea Luz está atrás de você por 8 ponto(s).",
    );
  });

  it("empate em pontos: frase própria, sem 'por 0 ponto(s)'", () => {
    const data = fixture();
    data.driverStandings[2].pontos = 48;
    const { aiFacts } = buildBriefingContext(data);
    expect(aiFacts).toContain("Bea Luz está empatado em pontos com você.");
    expect(aiFacts).not.toContain("por 0 ponto(s)");
  });
});
