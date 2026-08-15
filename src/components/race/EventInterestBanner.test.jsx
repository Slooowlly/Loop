import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import EventInterestBanner from "./EventInterestBanner";

// O `Tooltip` espera 400ms antes de abrir. Relógio falso em vez de `waitFor`: a
// suíte inteira roda junto do cargo, e esperar tempo real aqui é o jeito de
// arrumar um teste que cai sozinho sob carga.
function avancarOAtraso() {
  act(() => {
    vi.advanceTimersByTime(600);
  });
}

const gatilhoDe = (faixa) => faixa.querySelector("[data-tooltip]");

describe("EventInterestBanner (F-07 — interesse esperado)", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("mostra o tier do backend, o público e a cota de estrela do jogador", () => {
    render(
      <EventInterestBanner
        interestLabel="Grande público"
        audienceEstimate={62000}
        audienceRankLabel="Uma das maiores da temporada"
        fameSharePct={18}
      />,
    );

    const faixa = screen.getByTestId("event-interest-banner");
    // O rótulo do tier vem PRONTO do backend (`tier_label`) — o front não traduz.
    expect(faixa).toHaveTextContent("Grande público");
    expect(faixa).toHaveTextContent("62.000");

    // O porte e a cota saíram do layout: só existem no balão. `data-tooltip` é a
    // alça estática que o `Tooltip` deixa no gatilho, então o teste lê o texto
    // sem simular hover nem esperar os 400ms de atraso do balão.
    expect(faixa).not.toHaveTextContent("Uma das maiores da temporada");
    const gatilho = faixa.querySelector("[data-tooltip]");
    expect(gatilho.getAttribute("data-tooltip")).toBe(
      "Uma das maiores da temporada. A sua estrela puxa cerca de 18% do público desta etapa.",
    );
  });

  it("o balão abre no hover com o porte e a cota, e fecha ao sair", () => {
    vi.useFakeTimers();
    render(
      <EventInterestBanner
        interestLabel="Grande público"
        audienceEstimate={62000}
        audienceRankLabel="Uma das maiores da temporada"
        fameSharePct={18}
      />,
    );

    const gatilho = gatilhoDe(screen.getByTestId("event-interest-banner"));
    fireEvent.mouseEnter(gatilho);
    avancarOAtraso();

    const balao = screen.getByTestId("tooltip");
    expect(balao).toHaveTextContent("Uma das maiores da temporada");
    expect(balao).toHaveTextContent("18%");

    fireEvent.mouseLeave(gatilho);
    expect(screen.queryByTestId("tooltip")).toBeNull();
  });

  it("sem equipe o balão fica só com o porte da ocasião", () => {
    render(
      <EventInterestBanner
        interestLabel="Público moderado"
        audienceEstimate={41000}
        audienceRankLabel="Uma das menores da temporada"
        fameSharePct={null}
      />,
    );

    const gatilho = screen.getByTestId("event-interest-banner").querySelector("[data-tooltip]");
    // Um "0% do público" diria que a estrela do jogador não puxa ninguém, quando o
    // dado é que não há equipe para puxar.
    expect(gatilho.getAttribute("data-tooltip")).toBe("Uma das menores da temporada");
  });

  it("sem público a faixa não desenha nada, em vez de uma barra em zero", () => {
    const { container } = render(
      <EventInterestBanner interestLabel="-" audienceEstimate={0} audienceRankLabel="" fameSharePct={null} />,
    );

    expect(container).toBeEmptyDOMElement();
  });

  it("a barra mede contra o teto da escala de tiers, e satura em vez de estourar", () => {
    render(
      <EventInterestBanner
        interestLabel="Evento principal"
        audienceEstimate={180000}
        audienceRankLabel=""
        fameSharePct={null}
      />,
    );

    expect(screen.getByTestId("event-interest-bar")).toHaveStyle({ width: "100%" });
  });

  it("sem porte e sem cota não sobra balão vazio pendurado no hover", () => {
    vi.useFakeTimers();
    render(
      <EventInterestBanner
        interestLabel="Público modesto"
        audienceEstimate={12000}
        audienceRankLabel=""
        fameSharePct={null}
      />,
    );

    const faixa = screen.getByTestId("event-interest-banner");
    // Sem os dois dados o `Tooltip` devolve o filho cru, então nem o `data-tooltip`
    // existe. Sem esse caminho, o hover abriria um balão em branco.
    expect(gatilhoDe(faixa)).toBeNull();
    // E sem balão o cursor não promete leitura escondida.
    expect(faixa.firstElementChild.className).not.toContain("cursor-help");

    fireEvent.mouseEnter(faixa.firstElementChild);
    avancarOAtraso();
    expect(screen.queryByTestId("tooltip")).toBeNull();
  });
});
