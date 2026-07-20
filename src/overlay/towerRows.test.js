import { describe, it, expect } from "vitest";
import {
  buildTowerSections,
  isTeammate,
  orderClasses,
  playerTeam,
  totalCars,
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
