import { render, screen } from "@testing-library/react";

import EventInterestCard from "./EventInterestCard";

describe("EventInterestCard (F-07 — interesse esperado)", () => {
  it("mostra o tier do backend, o público e a cota de estrela do jogador", () => {
    render(
      <EventInterestCard
        interestLabel="Grande público"
        audienceEstimate={62000}
        audienceRankLabel="Uma das maiores da temporada"
        fameSharePct={18}
      />,
    );

    const card = screen.getByTestId("event-interest-card");
    // O rótulo do tier vem PRONTO do backend (`tier_label`) — o front não traduz.
    expect(card).toHaveTextContent("Grande público");
    expect(card).toHaveTextContent("62.000");
    expect(card).toHaveTextContent("Uma das maiores da temporada");
    expect(screen.getByTestId("event-interest-share")).toHaveTextContent("18%");
  });

  it("sem equipe não existe cota de público, e a linha não aparece", () => {
    render(
      <EventInterestCard
        interestLabel="Público moderado"
        audienceEstimate={41000}
        audienceRankLabel=""
        fameSharePct={null}
      />,
    );

    expect(screen.getByTestId("event-interest-card")).toBeInTheDocument();
    expect(screen.queryByTestId("event-interest-share")).not.toBeInTheDocument();
  });

  it("sem público o card não desenha nada, em vez de uma barra em zero", () => {
    const { container } = render(
      <EventInterestCard interestLabel="-" audienceEstimate={0} audienceRankLabel="" fameSharePct={null} />,
    );

    expect(container).toBeEmptyDOMElement();
  });

  it("a barra mede contra o teto da escala de tiers, e satura em vez de estourar", () => {
    render(
      <EventInterestCard
        interestLabel="Evento principal"
        audienceEstimate={180000}
        audienceRankLabel=""
        fameSharePct={null}
      />,
    );

    expect(screen.getByTestId("event-interest-bar")).toHaveStyle({ width: "100%" });
  });
});
