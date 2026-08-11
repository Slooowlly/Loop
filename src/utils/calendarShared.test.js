import { describe, expect, it } from "vitest";

import {
  ALL_CALENDAR_CATEGORIES,
  buildMonthGrid,
  formatIsoDateKey,
  getMonthPhaseType,
  getRaceTooltipStyle,
  getTrackImageSrc,
  parseDisplayDate,
  weatherLabel,
} from "./calendarShared";

// Helpers puros do calendário. Nenhum deles toca no store nem no backend, e é isso
// que os torna testáveis — e que fazia falta: a grade de 42 células e o
// posicionamento do balão são regras de layout que só apareciam na tela.

describe("getMonthPhaseType", () => {
  it("no calendário atual, janeiro é mercado, fev–nov temporada e dezembro encerramento", () => {
    expect(getMonthPhaseType(0)).toBe("mercado");
    expect(getMonthPhaseType(1)).toBe("regular");
    expect(getMonthPhaseType(10)).toBe("regular");
    expect(getMonthPhaseType(11)).toBe("encerramento");
  });

  it("no calendário legado, o ano termina em bloco especial em vez de encerramento", () => {
    expect(getMonthPhaseType(0, true)).toBe("mercado");
    expect(getMonthPhaseType(7, true)).toBe("regular");
    // Set–Dez: a diferença que faz o legado ser legado.
    expect(getMonthPhaseType(8, true)).toBe("especial");
    expect(getMonthPhaseType(11, true)).toBe("especial");
  });

  it("cobre os doze meses nos dois calendários, sem mês sem fase", () => {
    for (let mes = 0; mes < 12; mes += 1) {
      expect(getMonthPhaseType(mes)).toMatch(/^(mercado|regular|encerramento)$/);
      expect(getMonthPhaseType(mes, true)).toMatch(/^(mercado|regular|especial)$/);
    }
  });
});

describe("parseDisplayDate", () => {
  it("lê a string crua e devolve o mês em base zero, como o Date espera", () => {
    expect(parseDisplayDate("2026-08-11")).toEqual({ year: 2026, month: 7, day: 11 });
    expect(parseDisplayDate("2026-01-01")).toEqual({ year: 2026, month: 0, day: 1 });
  });

  it("aceita o sufixo de hora que algumas etapas trazem junto", () => {
    expect(parseDisplayDate("2026-08-11T14:30:00")).toEqual({ year: 2026, month: 7, day: 11 });
  });

  it("devolve null em vez de uma data inventada quando a string não serve", () => {
    expect(parseDisplayDate(null)).toBeNull();
    expect(parseDisplayDate("")).toBeNull();
    expect(parseDisplayDate("11/08/2026")).toBeNull();
  });

  it("não sofre com fuso: a string manda, e não o `new Date`", () => {
    // Um `new Date("2026-01-01")` a oeste de Greenwich cai em 31/12/2025. Aqui não.
    expect(parseDisplayDate("2026-01-01").day).toBe(1);
  });
});

describe("formatIsoDateKey", () => {
  it("zera à esquerda para a chave bater com a string do backend", () => {
    expect(formatIsoDateKey(2026, 0, 1)).toBe("2026-01-01");
    expect(formatIsoDateKey(2026, 11, 25)).toBe("2026-12-25");
  });

  it("fecha o ciclo com parseDisplayDate", () => {
    const iso = "2026-08-11";
    const { year, month, day } = parseDisplayDate(iso);
    expect(formatIsoDateKey(year, month, day)).toBe(iso);
  });
});

describe("buildMonthGrid", () => {
  it("devolve sempre 42 células — a grade tem altura fixa, sem pulo de layout", () => {
    for (let mes = 0; mes < 12; mes += 1) {
      expect(buildMonthGrid(2026, mes)).toHaveLength(42);
    }
    // Fevereiro de ano bissexto e de ano comum também cabem nas mesmas 6 linhas.
    expect(buildMonthGrid(2024, 1)).toHaveLength(42);
  });

  it("preenche os dias do mês vizinho marcados como `outside`, sem buracos", () => {
    // Janeiro de 2026 começa numa quinta → quatro dias de dezembro à frente.
    const grade = buildMonthGrid(2026, 0);
    expect(grade.slice(0, 4)).toEqual([
      { day: 28, outside: true },
      { day: 29, outside: true },
      { day: 30, outside: true },
      { day: 31, outside: true },
    ]);
    expect(grade[4]).toEqual({ day: 1, outside: false });
    expect(grade.every((cell) => typeof cell.day === "number")).toBe(true);
  });

  it("os dias do próprio mês são exatamente 1..N, na ordem", () => {
    const dentro = buildMonthGrid(2026, 0)
      .filter((cell) => !cell.outside)
      .map((cell) => cell.day);
    expect(dentro).toEqual(Array.from({ length: 31 }, (_, i) => i + 1));
  });

  it("sem dias à frente, a primeira célula já é o dia 1 do mês", () => {
    // 1º de fevereiro de 2026 cai num domingo.
    const grade = buildMonthGrid(2026, 1);
    expect(grade[0]).toEqual({ day: 1, outside: false });
    expect(grade[27]).toEqual({ day: 28, outside: false });
    expect(grade[28]).toEqual({ day: 1, outside: true });
  });
});

describe("getRaceTooltipStyle", () => {
  const viewport = { width: 1000, height: 800 };
  const tamanho = { width: 208, height: 176 };

  it("centraliza no dia e abre ACIMA quando há espaço", () => {
    const style = getRaceTooltipStyle(
      { left: 400, width: 40, top: 300, height: 40 },
      viewport,
      tamanho,
    );
    expect(style.left).toBe(316); // 400 + 20 − 104
    expect(style.top).toBe(116); // 300 − 176 − 8
    expect(style.position).toBe("fixed");
    expect(style.pointerEvents).toBe("none");
  });

  it("cai para BAIXO quando a célula está colada no topo", () => {
    const style = getRaceTooltipStyle(
      { left: 400, width: 40, top: 50, height: 40 },
      viewport,
      tamanho,
    );
    expect(style.top).toBe(98); // 50 + 40 + 8
  });

  it("nunca vaza pela direita da viewport", () => {
    const style = getRaceTooltipStyle(
      { left: 960, width: 30, top: 300, height: 40 },
      viewport,
      tamanho,
    );
    expect(style.left + tamanho.width).toBeLessThanOrEqual(viewport.width - 12);
  });

  it("nunca vaza pela esquerda da viewport", () => {
    const style = getRaceTooltipStyle(
      { left: 4, width: 30, top: 300, height: 40 },
      viewport,
      tamanho,
    );
    expect(style.left).toBeGreaterThanOrEqual(12);
  });

  it("respeita a margem também na vertical, em viewport apertada", () => {
    const baixa = { width: 1000, height: 200 };
    const style = getRaceTooltipStyle(
      { left: 400, width: 40, top: 190, height: 40 },
      baixa,
      tamanho,
    );
    expect(style.top).toBeGreaterThanOrEqual(12);
    expect(style.top).toBeLessThanOrEqual(Math.max(12, baixa.height - tamanho.height - 12));
  });

  it("aplica o deslocamento vertical pedido pelo chamador", () => {
    const celula = { left: 400, width: 40, top: 300, height: 40 };
    const semOffset = getRaceTooltipStyle(celula, viewport, tamanho);
    const comOffset = getRaceTooltipStyle(celula, viewport, tamanho, { verticalOffset: 40 });
    expect(semOffset.top - comOffset.top).toBe(40);
  });

  it("mantém o balão dentro da tela em qualquer coluna da grade", () => {
    for (let coluna = 0; coluna < 7; coluna += 1) {
      const style = getRaceTooltipStyle(
        { left: coluna * 140 + 6, width: 130, top: 400, height: 60 },
        viewport,
        tamanho,
      );
      expect(style.left).toBeGreaterThanOrEqual(12);
      expect(style.left + tamanho.width).toBeLessThanOrEqual(viewport.width - 12);
    }
  });
});

describe("getTrackImageSrc", () => {
  it("devolve null quando não há miniatura — o EventRow depende disso", () => {
    // A política de "miss" é o que diferencia esta chamada da do resto do app: sem
    // null, o calendário desenharia uma <img> quebrada em vez do placeholder colorido.
    expect(getTrackImageSrc({ track_name: "Pista Que Não Existe", track_id: null })).toBeNull();
    expect(getTrackImageSrc(null)).toBeNull();
    expect(getTrackImageSrc({})).toBeNull();
  });

  it("resolve a miniatura quando a pista tem arte", () => {
    // Laguna Seca é uma das 16 pistas com MINIATURA (conjunto bem menor que o das
    // panorâmicas do banner) — o calendário lê deste conjunto, não daquele.
    const src = getTrackImageSrc({ track_name: "Laguna Seca", track_id: null });
    expect(src).toBeTruthy();
    expect(src).toContain("/utilities/tracks/");
  });
});

describe("weatherLabel", () => {
  it("traduz cada clima do calendário para um rótulo próprio", () => {
    const seco = weatherLabel("Dry");
    const chuvaForte = weatherLabel("HeavyRain");
    expect(seco).toBeTruthy();
    expect(chuvaForte).toBeTruthy();
    expect(seco).not.toBe(chuvaForte);
  });

  it("clima desconhecido cai no seco em vez de devolver a chave crua", () => {
    expect(weatherLabel("Furacao")).toBe(weatherLabel("Dry"));
    expect(weatherLabel(null)).toBe(weatherLabel("Dry"));
  });
});

describe("ALL_CALENDAR_CATEGORIES", () => {
  it("lista as nove categorias da escada, sem repetição", () => {
    expect(ALL_CALENDAR_CATEGORIES).toHaveLength(9);
    expect(new Set(ALL_CALENDAR_CATEGORIES).size).toBe(9);
  });

  it("começa nos rookies e termina no endurance — a ordem é a da pirâmide", () => {
    expect(ALL_CALENDAR_CATEGORIES[0]).toBe("mazda_rookie");
    expect(ALL_CALENDAR_CATEGORIES.at(-1)).toBe("endurance");
  });
});
