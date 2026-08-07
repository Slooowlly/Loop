import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import PreseasonSpread from "./PreseasonSpread";

// Um parágrafo "gordo" como os que a IA devolve quando o grid está cheio.
function paragrafo(n) {
  return `Parágrafo ${n}. ` + "Texto de corpo da matéria de pré-temporada. ".repeat(12);
}

function corpoCom(qtd) {
  return Array.from({ length: qtd }, (_, i) => paragrafo(i + 1)).join("\n\n");
}

const BASE = {
  catLabel: "Mazda Rookie",
  year: 2026,
  openingRace: null,
  standings: [],
  driverStandings: [],
  playerTeam: null,
  worldNotes: [],
};

function renderSpread(preview) {
  return render(<PreseasonSpread {...BASE} preview={preview} />);
}

describe("PreseasonSpread — matéria longa", () => {
  it("mantém a matéria curta inteira na página esquerda", () => {
    const { container } = renderSpread({ body: corpoCom(2) });

    expect(container.querySelectorAll(".page-l .prose-cols p")).toHaveLength(2);
    expect(container.querySelector(".prose-cont")).toBeNull();
  });

  it("continua a matéria longa na página direita, sem perder parágrafo", () => {
    const { container } = renderSpread({ body: corpoCom(8) });

    const esq = container.querySelectorAll(".page-l .prose-cols p");
    const dir = container.querySelectorAll(".prose-cont p");
    expect(esq.length).toBeGreaterThanOrEqual(2);
    expect(dir.length).toBeGreaterThanOrEqual(2);
    expect(esq.length + dir.length).toBe(8);
    expect(screen.getByText(/Parágrafo 8\./)).toBeInTheDocument();
  });

  it("encolhe o título quando a manchete e o olho são longos", () => {
    const { container } = renderSpread({
      body: corpoCom(1),
      headline: "Mazda MX-5 Rookie Cup: a nova era começa em aberto",
      standfirst:
        "Com o trono vago e um grid igualado, a 27ª temporada da categoria promete uma disputa acirrada entre novatos e veteranos em busca da glória.",
    });

    expect(container.querySelector("h1").className).toContain("display--dense");
  });

  it("mantém o título no corpo cheio quando a manchete é curta", () => {
    const { container } = renderSpread({ body: corpoCom(1), headline: "O ano que vem aí", standfirst: "Temporada 2026" });

    expect(container.querySelector("h1").className).toBe("display");
  });
});

describe("PreseasonSpread — grid de pilotos", () => {
  const STANDINGS = [
    { id: "t1", posicao: 1, nome: "Equipe Cotada", pontos: 0, cor_primaria: "#f00" },
    { id: "t2", posicao: 2, nome: "Equipe do Meio", pontos: 0, cor_primaria: "#0f0" },
  ];
  // Chegam fora de ordem de propósito: quem manda é a ordem das equipes.
  const DRIVERS = [
    { id: "d1", nome: "Piloto B2", equipe_id: "t2", equipe_nome: "Equipe do Meio", pontos: 0 },
    { id: "d2", nome: "Piloto A1", equipe_id: "t1", equipe_nome: "Equipe Cotada", pontos: 0 },
    { id: "d3", nome: "Sem Vaga", equipe_id: null, pontos: 0 },
    { id: "d4", nome: "Piloto B1", equipe_id: "t2", equipe_nome: "Equipe do Meio", pontos: 0 },
    { id: "d5", nome: "Piloto A2", equipe_id: "t1", equipe_nome: "Equipe Cotada", pontos: 0 },
  ];

  it("lista os pilotos por expectativa da equipe, com companheiros lado a lado", () => {
    const { container } = render(
      <PreseasonSpread
        {...BASE}
        preview={{ body: corpoCom(1) }}
        standings={STANDINGS}
        driverStandings={DRIVERS}
      />,
    );

    const nomes = [...container.querySelectorAll(".page-r .res-row .rn")].map((el) => el.textContent);
    expect(nomes).toEqual(["Piloto A1", "Piloto A2", "Piloto B2", "Piloto B1", "Sem Vaga"]);
  });
});

describe("PreseasonSpread — grid ancorado no tamanho da matéria", () => {
  // Seis equipes de dois pilotos: o grid típico de uma categoria de entrada.
  const STANDINGS = Array.from({ length: 6 }, (_, i) => ({
    id: `t${i + 1}`,
    posicao: i + 1,
    nome: `Equipe ${i + 1}`,
    pontos: 0,
    cor_primaria: "#888",
  }));
  const DRIVERS = Array.from({ length: 12 }, (_, i) => ({
    id: `d${i + 1}`,
    nome: `Piloto ${i + 1}`,
    equipe_id: `t${Math.floor(i / 2) + 1}`,
    equipe_nome: `Equipe ${Math.floor(i / 2) + 1}`,
    pontos: 0,
  }));

  function renderGrid(body) {
    return render(
      <PreseasonSpread
        {...BASE}
        preview={{ body }}
        standings={STANDINGS}
        driverStandings={DRIVERS}
      />,
    );
  }

  it("quebra o grid em duas colunas quando a matéria é curta", () => {
    const { container } = renderGrid(corpoCom(1));

    expect(container.querySelector(".page-r .res-list").className).toContain("res-list--split");
    expect(container.querySelectorAll(".page-r .res-row")).toHaveLength(12);
  });

  it("mantém o grid em coluna única quando a matéria sustenta a altura", () => {
    // Três parágrafos gordos: texto longo que ainda cabe todo na página esquerda.
    const denso = Array.from({ length: 3 }, (_, i) => paragrafo(i + 1).repeat(2)).join("\n\n");
    const { container } = renderGrid(denso);

    expect(container.querySelector(".page-r .res-list").className).not.toContain("res-list--split");
    expect(container.querySelectorAll(".page-r .res-row")).toHaveLength(12);
  });
});
