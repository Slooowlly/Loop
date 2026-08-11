import { describe, expect, it } from "vitest";

import { MEDAL_COLORS } from "./teamHistoryV2Logic";
import {
  CHIP_GAP,
  CHIP_HEIGHT,
  CHIP_MIN_STEP,
  chaveDaRodada,
  chipWidth,
  dicaDeTexto,
  formatMeetingAge,
  montarDica,
  rotuloFaixa,
  seasonTooltip,
} from "./teamHistoryV2Labels";

// Teste espelho do miolo de rótulos extraído de TeamHistoryDrawerV2.jsx.
//
// O que se guarda aqui é o par tela/leitor de tela: a dica desenhada mostra a cor e
// omite a colocação por escrito, e o texto acessível faz o contrário. Nenhum teste de
// componente cobre esse desencontro — na tela os dois parecem iguais.

/// Um `t` de teste: devolve a própria chave com as variáveis penduradas, para o teste
/// falar de ESTRUTURA (que chave, com que argumento) e não do texto em português, que
/// mora no i18n e pode ser reescrito sem que nada aqui esteja errado.
function tFake(chave, vars) {
  if (!vars) return chave;
  const args = Object.entries(vars).map(([k, v]) => `${k}=${v}`).join(",");
  return `${chave}(${args})`;
}

describe("montarDica", () => {
  it("achata header, meta e linhas no texto acessível", () => {
    const dica = montarDica("2024 · GT3", "P2 · 12 corridas", [
      { id: "first", color: "#f2c46d", texto: "3×", textoAcessivel: "3× vitórias" },
    ]);
    expect(dica.texto).toBe("2024 · GT3\nP2 · 12 corridas\n\n3× vitórias");
  });

  it("a linha sem texto acessível cai no texto da tela", () => {
    const dica = montarDica("2024", "meta", [{ id: "empty", color: null, texto: "sem top 5" }]);
    expect(dica.texto).toBe("2024\nmeta\n\nsem top 5");
  });

  it("sem linhas, não sobra a linha em branco de separação", () => {
    // A linha vazia existe para separar o cabeçalho da lista; sem lista, ela vira um
    // parágrafo mudo no meio do que o leitor de tela anuncia.
    expect(montarDica("2024", "meta", []).texto).toBe("2024\nmeta");
  });

  it("preserva as partes estruturadas para quem desenha o balão", () => {
    const linhas = [{ id: "first", color: "#f2c46d", texto: "1×" }];
    const dica = montarDica("2024", "meta", linhas);
    expect(dica).toMatchObject({ header: "2024", meta: "meta", linhas });
  });
});

describe("dicaDeTexto", () => {
  it("separa o par header/meta que o i18n entrega colado", () => {
    const dica = dicaDeTexto("2019\nA equipe corria na GT4");
    expect(dica.header).toBe("2019");
    expect(dica.meta).toBe("A equipe corria na GT4");
    expect(dica.linhas).toEqual([]);
  });

  it("junta em uma linha o que vier com mais de uma quebra", () => {
    expect(dicaDeTexto("2019\nprimeira\nsegunda").meta).toBe("primeira segunda");
  });

  it("texto sem quebra vira header sem meta", () => {
    const dica = dicaDeTexto("2019");
    expect(dica.header).toBe("2019");
    expect(dica.meta).toBe("");
  });
});

describe("seasonTooltip", () => {
  const steps = [
    { id: "first", count: 3, color: MEDAL_COLORS.first },
    { id: "second", count: 1, color: MEDAL_COLORS.second },
  ];

  function dica(extra = {}) {
    return seasonTooltip(tFake, {
      row: { year: 2024, category: "GT3", position: "P2" },
      races: 12,
      topFive: 4,
      steps,
      dnfs: 0,
      ...extra,
    });
  }

  it("o cabeçalho junta ano e categoria", () => {
    expect(dica().header).toBe("2024 · GT3");
  });

  it("sem categoria, o cabeçalho é só o ano", () => {
    expect(dica({ row: { year: 2024, position: "P2" } }).header).toBe("2024");
  });

  it("temporada sem colocação usa a outra chave de meta", () => {
    // O travessão é o "sem colocação" do dossiê; mandá-lo para a chave com posição
    // imprimiria "P—" no balão.
    expect(dica({ row: { year: 2024, position: "—" } }).meta).toContain("metaNoPosition");
    expect(dica({ row: { year: 2024, position: null } }).meta).toContain("metaNoPosition");
    expect(dica().meta).toContain("seasonTooltip.meta(");
  });

  it("na tela só a contagem; no leitor de tela, a colocação por extenso", () => {
    // É a razão de a linha ter dois textos: o quadradinho colorido ao lado já diz a
    // colocação para quem enxerga, e não diz nada para quem ouve.
    const linha = dica().linhas[0];
    expect(linha.texto).toBe("myTeamTab.history.records.seasonTooltip.countShort(value=3)");
    expect(linha.textoAcessivel).toContain("medals.first");
    expect(linha.color).toBe(MEDAL_COLORS.first);
  });

  it("temporada sem nenhum top 5 diz isso em vez de sair vazia", () => {
    const linhas = dica({ steps: [] }).linhas;
    expect(linhas).toHaveLength(1);
    expect(linhas[0].id).toBe("empty");
    expect(linhas[0].color).toBeNull();
  });

  it("o abandono entra por último, com rótulo escrito e cor própria", () => {
    // Ele não é uma colocação: as outras linhas contam onde a equipe terminou, esta
    // conta o domingo em que ela não terminou.
    const linhas = dica({ dnfs: 2 }).linhas;
    expect(linhas[linhas.length - 1]).toMatchObject({ id: "dnf", color: MEDAL_COLORS.dnf });
    expect(linhas[linhas.length - 1].texto).toContain("medals.dnf");
  });

  it("sem abandono, a linha não aparece", () => {
    expect(dica({ dnfs: 0 }).linhas.some((l) => l.id === "dnf")).toBe(false);
  });

  it("não passa `count` ao i18next", () => {
    // `count` liga a máquina de plural e mandaria procurar chaves `_one`/`_other`
    // que não existem — o balão sairia com a chave crua.
    const texto = dica({ dnfs: 2 }).texto;
    expect(texto).not.toMatch(/\bcount=/);
    expect(texto).toMatch(/value=/);
  });
});

describe("rotuloFaixa", () => {
  it("passa valor e percentual para a chave da faixa", () => {
    expect(rotuloFaixa(tFake, { value: 12, percent: 34 }))
      .toBe("myTeamTab.history.sport.spreadValue(value=12,percent=34)");
  });
});

describe("formatMeetingAge", () => {
  it("sem dado, o card cala em vez de inventar 'há 0 semanas'", () => {
    expect(formatMeetingAge(tFake, null)).toBe("myTeamTab.history.identity.rivalAgeUnknown");
    expect(formatMeetingAge(tFake, undefined)).toBe("myTeamTab.history.identity.rivalAgeUnknown");
  });

  it("a semana corrente e a anterior são 'agora'", () => {
    expect(formatMeetingAge(tFake, 0)).toBe("myTeamTab.history.identity.rivalAgeNow");
    expect(formatMeetingAge(tFake, 1)).toBe("myTeamTab.history.identity.rivalAgeNow");
  });

  it("até dois meses conta em semanas", () => {
    expect(formatMeetingAge(tFake, 2)).toContain("rivalAgeWeeks");
    expect(formatMeetingAge(tFake, 8)).toBe("myTeamTab.history.identity.rivalAgeWeeks(count=8)");
  });

  it("de dois meses a um ano conta em meses", () => {
    expect(formatMeetingAge(tFake, 9)).toBe("myTeamTab.history.identity.rivalAgeMonths(count=2)");
    expect(formatMeetingAge(tFake, 51)).toBe("myTeamTab.history.identity.rivalAgeMonths(count=12)");
  });

  it("de um ano em diante conta em anos, arredondando para baixo", () => {
    // Arredondar para cima diria "há 2 anos" no dia seguinte ao primeiro aniversário.
    expect(formatMeetingAge(tFake, 52)).toBe("myTeamTab.history.identity.rivalAgeYears(count=1)");
    expect(formatMeetingAge(tFake, 103)).toBe("myTeamTab.history.identity.rivalAgeYears(count=1)");
    expect(formatMeetingAge(tFake, 104)).toBe("myTeamTab.history.identity.rivalAgeYears(count=2)");
  });
});

describe("chaveDaRodada", () => {
  it("junta ano e rodada", () => {
    expect(chaveDaRodada(2024, 3)).toBe("2024-3");
    expect(chaveDaRodada("2024", "3")).toBe("2024-3");
  });

  it("o ano faz parte da chave", () => {
    // Sem ele, passar o mouse na rodada 3 da fita acenderia a rodada 3 de todos os
    // anos que a campanha desenha.
    expect(chaveDaRodada(2023, 3)).not.toBe(chaveDaRodada(2024, 3));
  });

  it("dado não-numérico não vira chave", () => {
    expect(chaveDaRodada(undefined, 3)).toBeNull();
    expect(chaveDaRodada(2024, undefined)).toBeNull();
    expect(chaveDaRodada(2024, "rodada")).toBeNull();
    expect(chaveDaRodada("", 3)).not.toBeNull();
  });

  it("`null` ESCAPA do corte e vira ano/rodada zero", () => {
    // Comportamento atual, registrado por ser surpreendente: `Number(null)` é 0, e
    // zero passa no `Number.isFinite`. Quem chama hoje sempre tem ano e rodada
    // numéricos (a campanha é de uma temporada, a fita vem do dossiê), então o caso
    // não acontece — mas se um payload trouxer `year: null`, a chave sai "0-N" e
    // acende a rodada N de todo ano nulo junto, em vez de não acender nada.
    expect(chaveDaRodada(null, 3)).toBe("0-3");
    expect(chaveDaRodada(2024, null)).toBe("2024-0");
  });
});

describe("chip da curva de campeonato", () => {
  it("a largura cresce com o número de caracteres", () => {
    // "P1" e "P12" não cabem na mesma caixa fixa.
    expect(chipWidth("P12")).toBeGreaterThan(chipWidth("P1"));
    expect(chipWidth("P1")).toBe(12 + 2 * 6.4);
  });

  it("valor não-texto não quebra a conta", () => {
    expect(chipWidth(12)).toBe(chipWidth("12"));
  });

  it("as constantes do chip são coerentes entre si", () => {
    // O passo mínimo tem de caber o chip mais a folga dos dois lados; abaixo disso a
    // etiqueta vira tarja e o desenho passa a esconder rótulo.
    expect(CHIP_MIN_STEP).toBeGreaterThan(chipWidth("P12"));
    expect(CHIP_GAP).toBeGreaterThan(0);
    expect(CHIP_HEIGHT).toBeGreaterThan(0);
  });
});
