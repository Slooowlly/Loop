import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ReliabilityPanel, ResultSpread } from "./TeamHistoryResults.jsx";

// Os dois painéis saíram do drawer em 11/08/2026. O que eles decidem sozinhos é
// o corte de rótulo: abaixo de 5% do total a fatia fica só como cor, porque o
// número sairia recortado no meio da caixa. O dado não se perde — ele desce para
// a legenda. Essa troca é a regra que estes testes travam, junto do descarte de
// faixa vazia e da cor do delta contra o grupo.

describe("ResultSpread", () => {
  it("cala quando nao ha corrida", () => {
    const { container } = render(<ResultSpread spread={{ races: 0, first: 0, podium: 0, nearMiss: 0, topTen: 0, outside: 0 }} />);
    expect(container).toBeEmptyDOMElement();
    expect(render(<ResultSpread spread={null} />).container).toBeEmptyDOMElement();
  });

  it("descarta a faixa zerada em vez de desenhar uma fatia invisivel", () => {
    render(<ResultSpread spread={{ races: 10, first: 4, podium: 6, nearMiss: 0, topTen: 0, outside: 0 }} />);
    const barra = screen.getByTestId("team-history-spread");
    expect(barra.querySelectorAll("[data-band]")).toHaveLength(2);
    expect(barra.querySelector('[data-band="nearMiss"]')).toBeNull();
  });

  it("mantem a ordem do pior para o melhor resultado, da esquerda para a direita", () => {
    render(<ResultSpread spread={{ races: 20, first: 2, podium: 3, nearMiss: 4, topTen: 5, outside: 6 }} />);
    const bandas = [...screen.getByTestId("team-history-spread").querySelectorAll("[data-band]")].map(
      (n) => n.dataset.band
    );
    expect(bandas).toEqual(["first", "podium", "nearMiss", "topTen", "outside"]);
  });

  it("a fatia estreita fica so como cor, e a contagem desce para a legenda", () => {
    // 1 em 100 é 1% do total: a caixa tem menos que a largura de "1 (1%)".
    const { container } = render(
      <ResultSpread spread={{ races: 100, first: 1, podium: 0, nearMiss: 0, topTen: 0, outside: 99 }} />
    );
    const estreita = screen.getByTestId("team-history-spread").querySelector('[data-band="first"]');
    expect(estreita).toBeEmptyDOMElement();
    // A legenda carrega o que a barra não coube: "1º · 1 (1%)".
    expect(container.textContent).toContain("1º · 1 (1%)");
  });

  it("a fatia larga imprime o numero na propria barra e a legenda fica so com o nome", () => {
    const { container } = render(
      <ResultSpread spread={{ races: 10, first: 6, podium: 0, nearMiss: 0, topTen: 0, outside: 4 }} />
    );
    const larga = screen.getByTestId("team-history-spread").querySelector('[data-band="first"]');
    expect(larga).toHaveTextContent("6 (60%)");
    expect(container.textContent).not.toContain("1º · 6 (60%)");
  });

  it("arredonda a porcentagem sem deixar a soma inventar corrida", () => {
    render(<ResultSpread spread={{ races: 3, first: 1, podium: 1, nearMiss: 1, topTen: 0, outside: 0 }} />);
    const bandas = [...screen.getByTestId("team-history-spread").querySelectorAll("[data-band]")];
    bandas.forEach((banda) => expect(banda).toHaveTextContent("1 (33%)"));
  });
});

const CONFIAVEL = {
  races: 50,
  finished: 44,
  mechanical: 4,
  driverError: 2,
  other: 0,
  finishRate: 88,
  groupFinishRate: 80,
};

describe("ReliabilityPanel", () => {
  it("cala quando nao ha largada", () => {
    expect(render(<ReliabilityPanel reliability={{ ...CONFIAVEL, races: 0 }} />).container).toBeEmptyDOMElement();
    expect(render(<ReliabilityPanel reliability={null} />).container).toBeEmptyDOMElement();
  });

  it("mostra a taxa de chegadas nos dois arranjos", () => {
    const solto = render(<ReliabilityPanel reliability={CONFIAVEL} />);
    expect(within(solto.container).getByTestId("team-history-finish-rate")).toHaveTextContent("88%");
    const compacto = render(<ReliabilityPanel reliability={CONFIAVEL} compacto />);
    expect(within(compacto.container).getByTestId("team-history-finish-rate")).toHaveTextContent("88%");
  });

  it("descarta a causa zerada", () => {
    const { container } = render(<ReliabilityPanel reliability={CONFIAVEL} />);
    expect(container.querySelectorAll("[data-band]")).toHaveLength(3);
    expect(container.querySelector('[data-band="other"]')).toBeNull();
  });

  it("acima da media do grupo pinta o delta de verde, abaixo de laranja", () => {
    const acima = render(<ReliabilityPanel reliability={CONFIAVEL} />);
    expect(within(acima.container).getByText("Grupo em 80%")).toHaveStyle({ color: "#3fbf7f" });
    const abaixo = render(
      <ReliabilityPanel reliability={{ ...CONFIAVEL, finishRate: 70, groupFinishRate: 80 }} />
    );
    expect(within(abaixo.container).getByText("Grupo em 80%")).toHaveStyle({ color: "#e5793a" });
  });

  it("empata com o grupo conta como acima", () => {
    // O delta é `>= 0`: uma equipe na média não é uma equipe em queda.
    const { container } = render(
      <ReliabilityPanel reliability={{ ...CONFIAVEL, finishRate: 80, groupFinishRate: 80 }} />
    );
    expect(within(container).getByText("Grupo em 80%")).toHaveStyle({ color: "#3fbf7f" });
  });

  it("no arranjo compacto a causa estreita fica so como cor", () => {
    // 2 em 50 é 4% do total, abaixo do corte de 5%.
    const { container } = render(<ReliabilityPanel reliability={CONFIAVEL} compacto />);
    expect(container.querySelector('[data-band="driverError"]')).toBeEmptyDOMElement();
    expect(container.querySelector('[data-band="finished"]')).toHaveTextContent("44");
    // E a contagem continua legível na legenda.
    expect(container.textContent).toContain("Erro de pilotagem · 2");
  });

  it("cala sobre a peca que mais falhou quando o payload nao traz uma", () => {
    const { container, rerender } = render(<ReliabilityPanel reliability={CONFIAVEL} />);
    expect(container.textContent).not.toContain("Peça que mais falhou");
    rerender(<ReliabilityPanel reliability={{ ...CONFIAVEL, worstPart: "Câmbio" }} />);
    expect(container.textContent).toContain("Peça que mais falhou: Câmbio");
  });
});
