import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { CategoriesSection } from "./TeamHistoryCategories.jsx";

// A seção Categorias saiu do drawer em 11/08/2026 sem nenhum teste próprio: o
// teste de tela do dossiê nunca chega a abri-la. O que ela decide sozinha são
// três coisas que erram em silêncio — a ordem e a largura da pirâmide, a régua
// de altura da trajetória (que é a ESCADA do recorte, não os degraus que a
// equipe pisou) e o ano da troca de categoria, que pertence à passagem que
// entra e não à que sai.

const ESCADA = [
  { categoryId: "mazda_rookie", category: "Mazda Rookie", tier: 0, visited: true, seasons: 4, years: "2018-2021" },
  { categoryId: "gt4", category: "GT4", tier: 1, visited: true, seasons: 2, years: "2022-2023", isCurrent: true },
  { categoryId: "gt3", category: "GT3", tier: 2, visited: false, seasons: 0, years: "" },
];

function secao(extra = {}) {
  return render(
    <CategoriesSection
      dossier={{
        movement: { promotions: 1, relegations: 0, peakCategory: "GT4", homeCategory: "Mazda Rookie", ladder: ESCADA, ...extra.movement },
        categoryPath: extra.categoryPath ?? [],
        outsideScopeSeasons: extra.outsideScopeSeasons ?? [],
        worldFirstYear: extra.worldFirstYear,
        worldLastYear: extra.worldLastYear,
      }}
    />
  );
}

describe("CategoryPyramid", () => {
  it("desenha a escada inteira do topo para a base", () => {
    const { container } = secao();
    const degraus = [...screen.getByTestId("team-history-category-pyramid").querySelectorAll("[data-visited]")];
    expect(degraus.map((el) => el.dataset.category)).toEqual(["gt3", "gt4", "mazda_rookie"]);
    expect(container.textContent).toContain("GT3");
  });

  it("alarga o degrau conforme desce, porque a base e a categoria de entrada", () => {
    secao();
    const degraus = [...screen.getByTestId("team-history-category-pyramid").querySelectorAll("[data-visited]")];
    const larguras = degraus.map((el) => parseFloat(el.style.maxWidth));
    expect(larguras[0]).toBeLessThan(larguras[1]);
    expect(larguras[1]).toBeLessThan(larguras[2]);
    expect(larguras[0]).toBe(58);
    expect(larguras[2]).toBe(100);
  });

  it("marca o degrau nunca pisado, em vez de omiti-lo", () => {
    // Sem os degraus acima, uma estreante vira card solitário e o jogador não vê
    // que ela está no primeiro degrau.
    secao();
    const pisados = [...screen.getByTestId("team-history-category-pyramid").querySelectorAll("[data-visited]")];
    expect(pisados.map((el) => el.dataset.visited)).toEqual(["0", "1", "1"]);
  });

  it("cala quando a escada nao veio no payload", () => {
    const { container } = secao({ movement: { ladder: [] } });
    expect(container.querySelector('[data-testid="team-history-category-pyramid"]')).toBeNull();
  });

  it("um degrau so nao divide por zero na largura", () => {
    secao({ movement: { ladder: [ESCADA[0]] } });
    const unico = screen.getByTestId("team-history-category-pyramid").querySelector("[data-visited]");
    expect(parseFloat(unico.style.maxWidth)).toBe(58);
  });
});

describe("CategoryTrajectory", () => {
  const PASSAGENS = [
    { categoryId: "mazda_rookie", category: "Mazda Rookie", tier: 0, startYear: 2018, endYear: 2021 },
    { categoryId: "gt4", category: "GT4", tier: 1, startYear: 2021, endYear: 2023 },
  ];

  it("cobre do primeiro ao ultimo ano do mundo, e nao so os anos da equipe", () => {
    secao({ categoryPath: PASSAGENS, worldFirstYear: 2016, worldLastYear: 2025 });
    const celulas = [...screen.getByTestId("team-history-category-trajectory").querySelectorAll("[data-year]")];
    expect(celulas).toHaveLength(10);
    expect(celulas[0].dataset.year).toBe("2016");
    expect(celulas.at(-1).dataset.year).toBe("2025");
  });

  it("no ano da troca a celula e da passagem que entra", () => {
    // 2021 fecha o Mazda e abre o GT4. Pintar a que estava saindo esconderia a
    // subida, que é o que a faixa existe para mostrar.
    secao({ categoryPath: PASSAGENS });
    const celulas = screen.getByTestId("team-history-category-trajectory");
    expect(celulas.querySelector('[data-year="2021"]').dataset.category).toBe("gt4");
    expect(celulas.querySelector('[data-year="2020"]').dataset.category).toBe("mazda_rookie");
  });

  it("normaliza a altura pela escada do recorte, e nao pelos degraus da equipe", () => {
    // A equipe só pisou tier 0 e 1, mas a escada vai até o tier 2. Se a régua
    // fosse a da equipe, o GT4 desenharia na altura máxima — "no topo" — quando
    // ainda falta um degrau.
    secao({ categoryPath: PASSAGENS });
    const faixa = screen.getByTestId("team-history-category-trajectory");
    const gt4 = parseFloat(faixa.querySelector('[data-year="2022"]').style.height);
    const teto = 12 + 26;
    expect(gt4).toBeLessThan(teto);
    expect(gt4).toBeCloseTo(12 + (2 / 3) * 26, 5);
  });

  it("o ano fora do recorte fica baixo e sem cor de categoria", () => {
    secao({
      categoryPath: PASSAGENS,
      outsideScopeSeasons: [{ year: 2016, category: "Fórmula Base" }],
      worldFirstYear: 2016,
    });
    const fora = screen.getByTestId("team-history-category-trajectory").querySelector('[data-year="2016"]');
    expect(fora.dataset.category).toBeUndefined();
    expect(fora.style.height).toBe("10px");
    expect(fora).toHaveStyle({ backgroundColor: "#2c3a4c" });
  });

  it("cala sem passagem com ano valido", () => {
    const { container } = secao({ categoryPath: [{ categoryId: "gt4", category: "GT4", tier: 1, startYear: 0, endYear: 0 }] });
    expect(container.querySelector('[data-testid="team-history-category-trajectory"]')).toBeNull();
  });
});

describe("CategoryTimeBars", () => {
  const LINHAS = [
    { categoryId: "mazda_rookie", category: "Mazda Rookie", seasons: 4, races: 48, wins: 2, podiums: 9 },
    { categoryId: "gt4", category: "GT4", seasons: 2, races: 24, wins: 0, podiums: 3 },
  ];

  it("mede a barra contra a categoria de maior permanencia", () => {
    secao({ movement: { timeLines: LINHAS } });
    const barras = screen.getByTestId("team-history-category-time").querySelectorAll("[data-category]");
    expect(barras[0].style.width).toBe("100%");
    expect(barras[1].style.width).toBe("50%");
  });

  it("cai no texto corrido quando o backend antigo nao manda as linhas", () => {
    secao({ movement: { timeLines: [], timeByCategory: "4 anos no Mazda Rookie · 2 no GT4" } });
    expect(screen.queryByTestId("team-history-category-time")).toBeNull();
    expect(screen.getByText("4 anos no Mazda Rookie · 2 no GT4")).toBeInTheDocument();
  });

  it("cala de vez quando nao ha linha nem texto de reserva", () => {
    const { container } = secao({ movement: { timeLines: [] } });
    expect(container.querySelector('[data-testid="team-history-category-time"]')).toBeNull();
  });
});
