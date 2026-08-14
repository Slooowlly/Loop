import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import useCareerStore from "../../stores/useCareerStore";
import { initialState } from "../../stores/career/state";
import useFluxoDeAvanco from "./useFluxoDeAvanco";

// Um botão só, sete destinos. Qual deles vale depende da fase da temporada, de haver
// etapa pendente, de o jogador ter equipe e da aba aberta — e o rótulo tem que anunciar
// o destino certo, senão o jogador clica em "Avançar calendário" e cai no mercado.
//
// Isto morava dentro do componente de layout do cabeçalho, sem teste. O caso mais fácil
// de quebrar sem ninguém ver é o DESVIO PELAS NOTÍCIAS: ele gasta um clique, vale uma vez
// por temporada, e some para quem já está na aba certa. Errar qualquer uma das três
// condições ou faz o jogador ficar preso num botão que não avança, ou o faz pular o
// fechamento do ano sem ver.

const ONDE = { activeTab: "standings", onTabChange: vi.fn() };

/// Monta o hook sobre um estado de store montado à mão.
function montar(estado, onde = {}) {
  useCareerStore.setState({ ...initialState, ...estado });
  const onTabChange = vi.fn();
  const view = renderHook(() => useFluxoDeAvanco({ ...ONDE, onTabChange, ...onde }));
  return { ...view, onTabChange };
}

const EQUIPE = { id: "T1", categoria: "gt3" };

beforeEach(() => {
  useCareerStore.setState({ ...initialState });
});

describe("rótulo do botão", () => {
  it("com etapa marcada, anuncia o avanço do calendário", () => {
    const { result } = montar({
      playerTeam: EQUIPE,
      season: { numero: 3, fase: "Temporada" },
      nextRace: { id: "R4" },
    });
    expect(result.current.rotuloDoAvanco()).toBe("Avançar calendário");
  });

  it("enquanto algo avança, o rótulo vira estado — e o botão fica desabilitado", () => {
    const { result } = montar({
      playerTeam: EQUIPE,
      season: { numero: 3, fase: "Temporada" },
      nextRace: { id: "R4" },
      isCalendarAdvancing: true,
    });
    expect(result.current.avancoEmCurso).toBe(true);
    expect(result.current.rotuloDoAvanco()).toBe("Avançando...");
  });

  it("agente livre sem etapa pula a temporada inteira", () => {
    const { result } = montar({
      playerTeam: null,
      season: { numero: 3, fase: "Temporada" },
      nextRace: null,
    });
    expect(result.current.isFreeAgent).toBe(true);
    expect(result.current.rotuloDoAvanco()).toBe("Pular temporada");
  });

  it("na pré-temporada, abre o mercado", () => {
    const { result } = montar({
      playerTeam: EQUIPE,
      season: { numero: 3, fase: "PreTemporada" },
      nextRace: null,
    });
    expect(result.current.rotuloDoAvanco()).toBe("Abrir mercado");
  });

  it("nas fases legado 9D, mantém os rótulos do bloco especial", () => {
    const legado = (fase) =>
      montar({ playerTeam: EQUIPE, season: { numero: 3, fase }, nextRace: null }).result;

    expect(legado("BlocoRegular").current.rotuloDoAvanco()).toBe("Avançar para convocação");
    expect(legado("BlocoEspecial").current.rotuloDoAvanco()).toBe("Pular bloco especial");
    expect(legado("PosEspecial").current.rotuloDoAvanco()).toBe("Encerrar temporada");
  });
});

describe("desvio pelas Notícias no fim do campeonato", () => {
  const FIM_DE_ANO = {
    playerTeam: EQUIPE,
    season: { numero: 3, fase: "Encerramento" },
    nextRace: null,
  };

  it("o primeiro clique leva às Notícias, sem virar o ano", () => {
    const { result, onTabChange } = montar(FIM_DE_ANO);

    expect(result.current.rotuloDoAvanco()).toBe("Ver o fechamento do ano");
    act(() => result.current.avancar());

    expect(onTabChange).toHaveBeenCalledWith("news");
    // Nada de temporada aconteceu: o clique só mudou de aba.
    expect(useCareerStore.getState().isAdvancing).toBe(false);
  });

  it("o segundo clique avança de verdade — o desvio vale UMA vez por temporada", () => {
    const { result } = montar(FIM_DE_ANO);

    act(() => result.current.avancar());
    // Depois do desvio consumido, o rótulo já anuncia o mercado.
    expect(result.current.rotuloDoAvanco()).toBe("Avançar para pré-temporada");
  });

  it("quem já está nas Notícias não é levado até lá de novo", () => {
    const { result, onTabChange } = montar(FIM_DE_ANO, { activeTab: "news" });

    expect(result.current.rotuloDoAvanco()).toBe("Avançar para pré-temporada");
    act(() => result.current.avancar());
    expect(onTabChange).not.toHaveBeenCalledWith("news");
  });

  it("não desvia quando ainda há etapa a correr — o ano não acabou", () => {
    const { result } = montar({ ...FIM_DE_ANO, nextRace: { id: "R12" } });
    expect(result.current.rotuloDoAvanco()).toBe("Avançar calendário");
  });

  it("agente livre não passa pelo desvio: ele pula a temporada", () => {
    const { result } = montar({ ...FIM_DE_ANO, playerTeam: null });
    expect(result.current.rotuloDoAvanco()).toBe("Pular temporada");
  });

  it("save legado não ganha o desvio — o fim de ano dele passa pelo bloco especial", () => {
    const { result } = montar({
      playerTeam: EQUIPE,
      season: { numero: 3, fase: "PosEspecial" },
      nextRace: null,
    });
    expect(result.current.rotuloDoAvanco()).toBe("Encerrar temporada");
  });
});

describe("clique único, dois caminhos", () => {
  it("com dias a passar, troca para o Calendário antes de animar", () => {
    const { result, onTabChange } = montar({
      playerTeam: EQUIPE,
      season: { numero: 3, fase: "Temporada" },
      nextRace: { id: "R4" },
      temporalSummary: { days_until_next_event: 7, current_display_date: "2026-08-11" },
    });

    act(() => result.current.avancar());
    expect(onTabChange).toHaveBeenCalledWith("calendar");
  });

  it("corrida HOJE abre a Sala direto, sem piscar o calendário", () => {
    const { result, onTabChange } = montar({
      playerTeam: EQUIPE,
      season: { numero: 3, fase: "Temporada" },
      nextRace: { id: "R4", display_date: "2026-08-11" },
      temporalSummary: { days_until_next_event: 0, current_display_date: "2026-08-11" },
      calendarDisplayDate: "2026-08-11",
      displayDaysUntilNextEvent: 0,
    });

    act(() => result.current.avancar());
    expect(onTabChange).not.toHaveBeenCalledWith("calendar");
  });
});

describe("quem é o dono do botão", () => {
  it("na Home, com a etapa do jogador, o banner assume o botão", () => {
    const { result } = montar({
      playerTeam: EQUIPE,
      season: { numero: 3, fase: "Temporada" },
      nextRace: { id: "R4" },
    });
    expect(result.current.bannerOwnsAdvance).toBe(true);
  });

  it("vendo OUTRA categoria, o banner é informativo e o botão volta à barra", () => {
    const { result } = montar({
      playerTeam: EQUIPE,
      homeCategory: "gt4",
      season: { numero: 3, fase: "Temporada" },
      nextRace: { id: "R4" },
    });
    expect(result.current.viewingOwnCategory).toBe(false);
    expect(result.current.bannerOwnsAdvance).toBe(false);
  });

  it("fora da Home, o botão é sempre o da barra", () => {
    const { result } = montar(
      {
        playerTeam: EQUIPE,
        season: { numero: 3, fase: "Temporada" },
        nextRace: { id: "R4" },
      },
      { activeTab: "calendar" },
    );
    expect(result.current.bannerOwnsAdvance).toBe(false);
  });

  it("na Sala de Estratégia, o cabeçalho mostra 'Voltar', não 'Avançar'", () => {
    const { result } = montar({
      playerTeam: EQUIPE,
      season: { numero: 3, fase: "Temporada" },
      nextRace: { id: "R4" },
      showRaceBriefing: true,
    });
    expect(result.current.showRaceBriefing).toBe(true);
    expect(result.current.bannerOwnsAdvance).toBe(false);

    act(() => result.current.fecharBriefing());
    expect(useCareerStore.getState().showRaceBriefing).toBe(false);
  });
});
