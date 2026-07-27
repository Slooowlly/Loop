import { describe, expect, it } from "vitest";
import { FADE_MS, SLIDE_MS, createTowerAnimator, rowKey } from "./towerAnimation";

describe("rowKey", () => {
  it("prefere o CarIdx, que é o que não muda quando o piloto troca de posição", () => {
    expect(rowKey({ idx: 7, name: "Rick Van Zwiet" })).toBe("i7");
    expect(rowKey({ idx: 0, name: "Renan" })).toBe("i0"); // idx 0 é válido
  });

  it("cai no nome quando não há idx (preview/mock)", () => {
    expect(rowKey({ name: "Renan" })).toBe("nRenan");
    expect(rowKey({ idx: -1, name: "Renan" })).toBe("nRenan");
  });

  it("aguenta carro ausente", () => {
    expect(rowKey(null)).toBe("");
  });
});

const layout = (entries) => new Map(entries);

describe("createTowerAnimator", () => {
  it("linha nova nasce no lugar certo e só faz fade", () => {
    const a = createTowerAnimator();
    a.sync(layout([["i1", 100]]), 0);

    expect(a.state("i1", 0)).toEqual({ y: 100, alpha: 0 });
    expect(a.state("i1", FADE_MS).alpha).toBe(1);
    expect(a.state("i1", FADE_MS).y).toBe(100); // sem deslize: já nasceu no lugar
  });

  it("mudança de posição desliza do y antigo ao novo e chega no destino", () => {
    const a = createTowerAnimator();
    a.sync(layout([["i1", 100]]), 0);
    a.sync(layout([["i1", 70]]), 1000); // subiu uma casa

    expect(a.state("i1", 1000).y).toBe(100); // começa de onde estava
    const meio = a.state("i1", 1000 + SLIDE_MS / 2).y;
    expect(meio).toBeLessThan(100);
    expect(meio).toBeGreaterThan(70);
    expect(a.state("i1", 1000 + SLIDE_MS).y).toBe(70); // chegou
    expect(a.state("i1", 5000).y).toBe(70); // e fica
  });

  it("desliza mesmo quando a troca cai no instante zero do relógio", () => {
    // Regressão: o sentinela de "nunca deslizou" era `startedAt === 0` e colidia com
    // um `now` real de 0 — a linha saltava direto pro destino, sem animação.
    const a = createTowerAnimator();
    a.sync(layout([["i1", 100]]), 0);
    a.sync(layout([["i1", 70]]), 0);

    expect(a.state("i1", 0).y).toBe(100); // ainda no lugar antigo
    expect(a.state("i1", SLIDE_MS / 2).y).toBeGreaterThan(70);
    expect(a.state("i1", SLIDE_MS).y).toBe(70);
  });

  it("ultrapassagem em cima de ultrapassagem continua de onde parou", () => {
    const a = createTowerAnimator();
    a.sync(layout([["i1", 100]]), 0);
    a.sync(layout([["i1", 70]]), 1000);
    const noMeio = a.state("i1", 1000 + SLIDE_MS / 2).y;

    // Nova troca de posição ANTES de o deslize anterior terminar.
    a.sync(layout([["i1", 40]]), 1000 + SLIDE_MS / 2);

    // Não pode voltar pro y antigo: retoma exatamente de onde a linha está.
    expect(a.state("i1", 1000 + SLIDE_MS / 2).y).toBeCloseTo(noMeio, 5);
    expect(a.state("i1", 1000 + SLIDE_MS / 2 + SLIDE_MS).y).toBe(40);
  });

  it("linha que sai da janela é esquecida e volta como nova", () => {
    const a = createTowerAnimator();
    a.sync(layout([["i1", 100]]), 0);
    a.sync(layout([]), 1000); // saiu da faixa visível da torre
    expect(a.state("i1", 1000)).toBeNull();

    a.sync(layout([["i1", 250]]), 2000); // reapareceu bem longe
    // Não pode deslizar de 100 até 250: ela não estava lá esse tempo todo.
    expect(a.state("i1", 2000)).toEqual({ y: 250, alpha: 0 });
  });

  it("hasMotion cobre o deslize inteiro e só ele", () => {
    const a = createTowerAnimator();
    a.sync(layout([["i1", 100]]), 0);
    expect(a.hasMotion(0)).toBe(true); // fade de entrada conta como movimento
    expect(a.hasMotion(FADE_MS)).toBe(false); // assentou

    a.sync(layout([["i1", 70]]), 1000);
    expect(a.hasMotion(1000)).toBe(true);
    expect(a.hasMotion(1000 + SLIDE_MS - 1)).toBe(true); // ainda a caminho
    expect(a.hasMotion(1000 + SLIDE_MS)).toBe(false); // chegou: pode desacelerar
    expect(a.hasMotion(9000)).toBe(false);
  });

  it("hasMotion é falso na torre vazia e com animação desligada", () => {
    expect(createTowerAnimator().hasMotion(0)).toBe(false);
    const parado = createTowerAnimator({ slideMs: 0, fadeMs: 0 });
    parado.sync(layout([["i1", 100]]), 0);
    parado.sync(layout([["i1", 70]]), 10);
    expect(parado.hasMotion(10)).toBe(false);
  });

  it("reset limpa tudo", () => {
    const a = createTowerAnimator();
    a.sync(layout([["i1", 100]]), 0);
    a.reset();
    expect(a.state("i1", 0)).toBeNull();
  });

  it("com duração zero não anima (caminho do VR / testes)", () => {
    const a = createTowerAnimator({ slideMs: 0, fadeMs: 0 });
    a.sync(layout([["i1", 100]]), 0);
    a.sync(layout([["i1", 70]]), 10);
    expect(a.state("i1", 10)).toEqual({ y: 70, alpha: 1 });
  });
});
