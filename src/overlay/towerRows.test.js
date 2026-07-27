import { describe, it, expect } from "vitest";
import {
  buildTowerSections,
  isTeammate,
  orderClasses,
  playerTeam,
  totalCars,
  createTowerWindow,
} from "./towerRows";

// Helpers pra montar grades fictícias enxutas.
const car = (pos, extra = {}) => ({ pos, name: `P${pos}`, team: `T${pos}`, ...extra });
const cls = (id, cars) => ({ id, label: id.toUpperCase(), color: "#fff", cars });
const data = (...classes) => ({ session: {}, classes });

const positionsOf = (section) =>
  section.rows.map((r) => (r.kind === "separator" ? "|" : r.car.pos));

describe("buildTowerSections", () => {
  it("mostra a grade inteira quando cabe (<= 15 no total)", () => {
    const cars = Array.from({ length: 12 }, (_, i) => car(i + 1, { player: i === 8 }));
    const sections = buildTowerSections(data(cls("gt3", cars)));

    expect(positionsOf(sections[0])).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    expect(positionsOf(sections[0])).not.toContain("|");
  });

  it("acima de 15: classe do jogador vira top 3 + separação + vizinhança (±4)", () => {
    const cars = Array.from({ length: 20 }, (_, i) => car(i + 1, { player: i === 11 })); // jogador em P12
    const sections = buildTowerSections(data(cls("gt3", cars)));

    // top 3, separação, e P8..P16 (jogador ±4)
    expect(positionsOf(sections[0])).toEqual([1, 2, 3, "|", 8, 9, 10, 11, 12, 13, 14, 15, 16]);
  });

  it("nas outras classes mostra só o top 3", () => {
    const gt3 = Array.from({ length: 14 }, (_, i) => car(i + 1, { player: i === 10 }));
    const gt4 = Array.from({ length: 8 }, (_, i) => car(i + 1));
    const sections = buildTowerSections(data(cls("gt3", gt3), cls("gt4", gt4)));

    expect(positionsOf(sections[1])).toEqual([1, 2, 3]);
  });

  it("jogador no pódio: emenda sem separação, mantendo o total de sempre", () => {
    const cars = Array.from({ length: 20 }, (_, i) => car(i + 1, { player: i === 1 })); // P2
    const sections = buildTowerSections(data(cls("gt3", cars)));

    // encosta no top -> lista contínua, sem "|"; a janela desliza pra baixo até o total
    expect(positionsOf(sections[0])).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    expect(positionsOf(sections[0])).not.toContain("|");
  });

  it("líder (P1): janela desliza pra baixo, NÃO encolhe pra 5", () => {
    const cars = Array.from({ length: 20 }, (_, i) => car(i + 1, { player: i === 0 })); // P1
    const sections = buildTowerSections(data(cls("gt3", cars)));

    // nada acima do líder -> estica pra baixo e mostra o total de sempre (12), sem "|"
    expect(positionsOf(sections[0])).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    expect(positionsOf(sections[0])).not.toContain("|");
  });

  it("nao separa pra esconder um piloto so", () => {
    const cars = Array.from({ length: 20 }, (_, i) => car(i + 1, { player: i === 7 })); // P8 -> janela P4..P12
    const sections = buildTowerSections(data(cls("gt3", cars)));

    // buraco seria so P... nenhum: janela comeca em P4, top acaba em P3 -> contiguo
    expect(positionsOf(sections[0])).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
  });

  it("jogador na lanterna (P20): janela desliza pra cima, mantém o total, não estoura", () => {
    const cars = Array.from({ length: 20 }, (_, i) => car(i + 1, { player: i === 19 })); // P20 (ultimo)
    const sections = buildTowerSections(data(cls("gt3", cars)));

    // sem ninguém abaixo -> a janela (9) sobe: top 3 + separação + P12..P20
    expect(positionsOf(sections[0])).toEqual([1, 2, 3, "|", 12, 13, 14, 15, 16, 17, 18, 19, 20]);
  });

  it("totalCars soma todas as classes", () => {
    const d = data(cls("a", [car(1), car(2)]), cls("b", [car(1)]));
    expect(totalCars(d)).toBe(3);
  });
});

describe("orderClasses (ordem canônica)", () => {
  const labels = (cs) => cs.map((c) => c.id);

  it("endurance: LMP2 > GT3 > GT4 (mesmo fora de ordem)", () => {
    const cs = orderClasses([cls("gt4", []), cls("lmp2", []), cls("gt3", [])]);
    expect(labels(cs)).toEqual(["lmp2", "gt3", "gt4"]);
  });

  it("production: BMW > Toyota > Mazda", () => {
    const cs = orderClasses([cls("mazda", []), cls("bmw", []), cls("toyota", [])]);
    expect(labels(cs)).toEqual(["bmw", "toyota", "mazda"]);
  });

  it("classe desconhecida vai pro fim, mantendo ordem de entrada", () => {
    const cs = orderClasses([cls("xyz", []), cls("gt3", []), cls("abc", [])]);
    expect(labels(cs)).toEqual(["gt3", "xyz", "abc"]);
  });

  it("buildTowerSections já entrega ordenado", () => {
    const d = data(cls("gt4", [car(1)]), cls("gt3", [car(1)]), cls("lmp2", [car(1)]));
    const sections = buildTowerSections(d);
    expect(sections.map((s) => s.cls.id)).toEqual(["lmp2", "gt3", "gt4"]);
  });
});

describe("companheiro de equipe", () => {
  const d = data(
    cls("gt3", [
      car(1, { team: "Kitsune" }),
      car(2, { team: "Ferrari" }),
      car(3, { team: "Kitsune", player: true }),
    ]),
  );

  it("playerTeam acha o time do jogador", () => {
    expect(playerTeam(d)).toBe("Kitsune");
  });

  it("isTeammate marca o mesmo time, menos o proprio jogador", () => {
    const team = playerTeam(d);
    const [p1, p2, p3] = d.classes[0].cars;

    expect(isTeammate(p1, team)).toBe(true); // mesmo time
    expect(isTeammate(p2, team)).toBe(false); // outro time
    expect(isTeammate(p3, team)).toBe(false); // e o proprio jogador
  });
});

// ── Janela SOLTA (zona morta) ────────────────────────────────────────────────
// Grade de 20 carros; o jogador começa em P12 e vai andando. `posicoes` devolve o
// que a torre mostraria, com "|" na separação.
describe("createTowerWindow", () => {
  const grade = (playerPos) =>
    data(cls("gt3", Array.from({ length: 20 }, (_, i) => car(i + 1, { player: i === playerPos - 1 }))));
  const posicoes = (playerPos, opts) => positionsOf(buildTowerSections(grade(playerPos), opts)[0]);
  // Em que LINHA da torre o jogador aparece — é o número que a animação move.
  const linhaDoJogador = (playerPos, opts) =>
    buildTowerSections(grade(playerPos), opts)[0].rows.findIndex(
      (r) => r.kind === "car" && r.car.player,
    );

  it("sem janela própria, continua GRUDADA: o jogador nunca muda de linha", () => {
    // Grudada, a janela anda junto com ele: quem rola é o pelotão, e a linha do
    // jogador fica imóvel. É o comportamento antigo, preservado como padrão.
    expect(linhaDoJogador(12)).toBe(linhaDoJogador(11));
    expect(posicoes(12)).not.toEqual(posicoes(11));
  });

  it("solta, uma ultrapassagem normal move o JOGADOR, não a janela", () => {
    const win = createTowerWindow();
    const antes = posicoes(12, { window: win });
    const linha = linhaDoJogador(12, { window: win });
    const depois = posicoes(11, { window: win });

    // As mesmas linhas, na mesma ordem: a janela ficou parada...
    expect(depois).toEqual(antes);
    // ...e quem subiu uma casa foi o jogador — que é o que se quer acompanhar.
    expect(linhaDoJogador(11, { window: win })).toBe(linha - 1);
  });

  it("solta, a janela cede quando o jogador encosta na borda", () => {
    const win = createTowerWindow();
    const inicial = posicoes(12, { window: win });
    const dentro = inicial.filter((p) => p !== "|");
    const ultimo = dentro[dentro.length - 1];

    // Anda até depois da última posição visível: a janela tem de acompanhar.
    const longe = posicoes(ultimo + 2, { window: win });
    expect(longe).not.toEqual(inicial);
    expect(longe).toContain(ultimo + 2);
  });

  it("ao ceder, anda o MÍNIMO — não recentra o jogador", () => {
    const win = createTowerWindow();
    const linhaInicial = linhaDoJogador(12, { window: win });
    // Cai duas posições: a janela cede o necessário e o jogador continua embaixo,
    // não volta pro meio (recentrar é justamente o que deixava tudo rolando).
    const linhaDepois = linhaDoJogador(14, { window: win });
    expect(linhaDepois).toBeGreaterThan(linhaInicial);
    expect(linhaDepois).not.toBe(linhaDoJogador(14)); // ≠ da grudada, que recentra
  });

  it("reset volta a janela a centrar no jogador", () => {
    const win = createTowerWindow();
    posicoes(12, { window: win });
    const cedida = posicoes(16, { window: win }); // caiu 4: a janela cedeu o mínimo
    win.reset();
    const recentrada = posicoes(16, { window: win });
    expect(recentrada).not.toEqual(cedida);
    expect(recentrada).toEqual(posicoes(16)); // igual à grudada
  });
});
