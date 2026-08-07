import { describe, it, expect } from "vitest";

import { comprimeSequenciasDeAnos, formatSequenciaDeAnos } from "./sequenciaDeAnos";

describe("comprimeSequenciasDeAnos", () => {
  it("junta tres anos seguidos ou mais num intervalo", () => {
    const blocos = comprimeSequenciasDeAnos([2025, 2024, 2023, 2022, 2021, 2020, 2019, 2018, 2017]);

    expect(blocos).toHaveLength(1);
    expect(blocos[0].label).toBe("2017 ~ 2025");
    // A entrada e descendente e o intervalo se le do comeco.
    expect(blocos[0].key).toBe("2017-2025");
  });

  it("nao junta dois anos seguidos", () => {
    // "2024 ~ 2025" e mais largo que "2024 2025" e ainda esconde que sao dois.
    expect(comprimeSequenciasDeAnos([2025, 2024]).map((b) => b.label)).toEqual(["2025", "2024"]);
  });

  it("quebra a sequencia no buraco", () => {
    // Um intervalo de 2019 a 2023 mentiria sobre 2021.
    const blocos = comprimeSequenciasDeAnos([2023, 2022, 2020, 2019]);

    expect(blocos.map((b) => b.label)).toEqual(["2023", "2022", "2020", "2019"]);
  });

  it("mantem a ordem descendente entre os blocos", () => {
    const blocos = comprimeSequenciasDeAnos([2025, 2024, 2023, 2014, 2011, 2008]);

    expect(blocos.map((b) => b.label)).toEqual(["2023 ~ 2025", "2014", "2011", "2008"]);
  });

  it("nao funde dois titulos do mesmo ano", () => {
    // Dois campeonatos no mesmo ano, em categorias diferentes: dois trofeus.
    const blocos = comprimeSequenciasDeAnos([2020, 2020, 2019]);

    expect(blocos).toHaveLength(3);
    expect(blocos.map((b) => b.label)).toEqual(["2020", "2020", "2019"]);
  });

  it("corta a sequencia quando a serie muda", () => {
    // A troca de equipe no meio da dinastia nao pode virar um intervalo so: o
    // chip carrega a logo de UMA casa.
    const titulos = [
      { ano: 2025, equipe: "McLaren" },
      { ano: 2024, equipe: "McLaren" },
      { ano: 2023, equipe: "McLaren" },
      { ano: 2022, equipe: "Ferrari" },
      { ano: 2021, equipe: "Ferrari" },
    ];

    const blocos = comprimeSequenciasDeAnos(titulos, {
      ano: (item) => item.ano,
      serie: (item) => item.equipe,
    });

    expect(blocos.map((b) => b.label)).toEqual(["2023 ~ 2025", "2022", "2021"]);
    expect(blocos[0].itens).toHaveLength(3);
  });

  it("ignora entrada vazia ou invalida", () => {
    expect(comprimeSequenciasDeAnos(null)).toEqual([]);
    expect(comprimeSequenciasDeAnos([2025, Number.NaN, 2024]).map((b) => b.label)).toEqual([
      "2025",
      "2024",
    ]);
  });
});

describe("formatSequenciaDeAnos", () => {
  it("escreve os blocos em texto corrido", () => {
    expect(formatSequenciaDeAnos([2025, 2024, 2023, 2014])).toBe("2023 ~ 2025, 2014");
  });

  it("devolve vazio sem anos", () => {
    expect(formatSequenciaDeAnos([])).toBe("");
  });
});
