import { afterEach, describe, expect, it } from "vitest";

import i18n from "../../i18n/index.js";
import {
  DEFAULT_FILTERS,
  FILTRO_TODOS,
  buildTableSections,
  filterRows,
} from "./globalDriverRanking";

// O ranking global guardava o "sem recorte" dos filtros como a palavra "Todos"/"Todas":
// o mesmo valor era estado e texto de tela. Este teste guarda a separação — o estado é
// uma chave neutra, o rótulo é do i18n — porque a falha é silenciosa: em inglês o filtro
// não some da tela, ele passa a filtrar por um status que nenhum piloto tem.

function piloto(extra = {}) {
  return {
    id: "D1",
    nome: "Piloto",
    status: "Ativo",
    nacionalidade: "Brasileiro",
    categoria_atual: "gt3",
    categorias_historicas: ["gt3"],
    idade: 25,
    titulos: 0,
    ...extra,
  };
}

afterEach(async () => {
  await i18n.changeLanguage("pt-BR");
});

describe("sentinela de filtro independente do idioma", () => {
  it("o padrão dos três filtros de lista não é texto de tela", () => {
    expect(DEFAULT_FILTERS.status).toBe(FILTRO_TODOS);
    expect(DEFAULT_FILTERS.category).toBe(FILTRO_TODOS);
    expect(DEFAULT_FILTERS.nationality).toBe(FILTRO_TODOS);
    // Os três de gatilho já usavam a mesma chave; o valor é um só para os seis.
    expect(DEFAULT_FILTERS.champions).toBe(FILTRO_TODOS);
  });

  it("o padrão não recorta nada", () => {
    const rows = [
      piloto({ id: "D1", status: "Ativo" }),
      piloto({ id: "D2", status: "Aposentado", nacionalidade: "Alemão" }),
      piloto({ id: "D3", status: "Livre", categoria_atual: "gt4", categorias_historicas: ["gt4"] }),
    ];
    expect(filterRows(rows, DEFAULT_FILTERS)).toHaveLength(3);
  });

  it("continua não recortando nada com o app em inglês", async () => {
    // Era aqui que quebrava: o seletor mostrava "All", guardava "Todos", e a linha
    // seguinte procurava um piloto cujo status literal fosse "Todos".
    await i18n.changeLanguage("en-US");
    const rows = [piloto({ id: "D1" }), piloto({ id: "D2", status: "Aposentado" })];
    expect(filterRows(rows, DEFAULT_FILTERS)).toHaveLength(2);
  });

  it("com categoria escolhida, a lista sai em duas seções", () => {
    const rows = [
      piloto({ id: "D1", status: "Ativo", categoria_atual: "gt3" }),
      piloto({ id: "D2", status: "Aposentado", categoria_atual: "gt3", categorias_historicas: ["gt3"] }),
    ];
    const secoes = buildTableSections(rows, "gt3");
    expect(secoes.map((s) => s.key)).toEqual(["current", "past"]);
    expect(secoes[0].label).toBe("Atualmente em GT3");
    expect(secoes[1].label).toBe("Já passaram por GT3");
  });

  it("os cabeçalhos das seções acompanham o idioma", async () => {
    await i18n.changeLanguage("en-US");
    const rows = [
      piloto({ id: "D1", status: "Ativo", categoria_atual: "gt3" }),
      piloto({ id: "D2", status: "Aposentado", categoria_atual: "gt3", categorias_historicas: ["gt3"] }),
    ];
    const secoes = buildTableSections(rows, "gt3");
    expect(secoes[0].label).toBe("Currently in GT3");
    expect(secoes[1].label).toBe("Previously raced in GT3");
  });

  it("sem recorte de categoria a lista é uma seção só, sem cabeçalho", () => {
    const rows = [piloto({ id: "D1" })];
    expect(buildTableSections(rows, FILTRO_TODOS)).toEqual([{ key: "all", label: null, rows }]);
  });
});
