import { describe, expect, it } from "vitest";

import { FAKE_SCENARIOS, buildFakeScenario } from "./debugFixtures";
import { CAR_PART_KEYS, carPartsRadar, lineupDossier, roundFlow, seasonLedger } from "./gridMetrics";
import { EXPENSE_LINES } from "../teamMetrics";

// O valor de um cenário de bancada está em ser COERENTE. Um fixture onde o net não
// fecha com entradas menos saídas, ou onde o caixa não anda com os resultados, faz a
// tela mentir de um jeito que nunca aconteceria no jogo — e aí a inspeção visual
// ensina a corrigir um problema que não existe (ou a ignorar um que existe).
describe("buildFakeScenario", () => {
  const ids = FAKE_SCENARIOS.map((scenario) => scenario.id);

  it.each(ids)("cenário %s: o resultado da rodada fecha com entradas menos saídas", (id) => {
    const { team, report } = buildFakeScenario(id);
    expect(team.last_round_net).toBe(team.last_round_income - team.last_round_expenses);
    expect(report.latest.net).toBe(report.latest.income_total - report.latest.expenses_total);
  });

  it.each(ids)("cenário %s: o acumulado da temporada é a soma das rodadas do timeline", (id) => {
    const { report } = buildFakeScenario(id);
    const somaNets = report.cash_timeline.reduce((total, point) => total + point.net, 0);
    expect(report.season.net).toBe(somaNets);
    expect(report.season.round).toBe(report.cash_timeline.length);
  });

  it.each(ids)("cenário %s: o caixa anda com os resultados, rodada a rodada", (id) => {
    const { team, report } = buildFakeScenario(id);
    const timeline = report.cash_timeline;
    for (let index = 1; index < timeline.length; index += 1) {
      expect(timeline[index].cash_balance).toBe(timeline[index - 1].cash_balance + timeline[index].net);
    }
    expect(timeline[timeline.length - 1].cash_balance).toBe(team.cash_balance);
  });

  it.each(ids)("cenário %s: a equipe do jogador está no grid, com o mesmo id e a posição declarada", (id) => {
    const { team, teams, report } = buildFakeScenario(id);
    const noGrid = teams.find((row) => row.id === team.id);
    expect(noGrid).toBeTruthy();
    expect(noGrid.posicao).toBe(report.current_position);
    expect(noGrid.cash_balance).toBe(team.cash_balance);
  });

  it.each(ids)("cenário %s: as linhas de despesa somam o total da rodada", (id) => {
    const { report } = buildFakeScenario(id);
    const soma = EXPENSE_LINES.reduce((total, line) => total + (report.latest[line.key] ?? 0), 0);
    // As fatias são arredondadas uma a uma, então tolera-se o erro de arredondamento
    // de cinco linhas — nunca uma diferença estrutural.
    expect(Math.abs(soma - report.latest.expenses_total)).toBeLessThanOrEqual(EXPENSE_LINES.length);
  });

  it.each(ids)("cenário %s: os dois gráficos da aba Dinheiro conseguem desenhar", (id) => {
    const fixture = buildFakeScenario(id);
    expect(roundFlow(fixture.report.latest, EXPENSE_LINES).hasData).toBe(true);
    const ledger = seasonLedger({ report: fixture.report, team: fixture.team, season: fixture.season });
    expect(ledger.hasData).toBe(true);
    expect(ledger.columns.length).toBeGreaterThan(0);
  });

  it("o cenário no vermelho realmente perde dinheiro por rodada", () => {
    const { team, report } = buildFakeScenario("vermelho");
    expect(team.last_round_net).toBeLessThan(0);
    expect(report.season.net).toBeLessThan(0);
  });

  it("o cenário endividado tem dívida e estado financeiro de crise", () => {
    const { team } = buildFakeScenario("divida");
    expect(team.debt_balance).toBeGreaterThan(0);
    expect(team.financial_state).toBe("crisis");
  });

  it.each(ids)("cenário %s: entrega a dupla e um pelotão para servir de régua", (id) => {
    const { team, drivers } = buildFakeScenario(id);
    const dupla = drivers.filter((driver) => driver.equipe_id === team.id);
    expect(dupla).toHaveLength(2);
    expect(dupla.map((driver) => driver.id)).toEqual([team.hierarquia_n1_id, team.hierarquia_n2_id]);
    // Sem pelotão não há média de categoria, e o dossiê perderia justamente a régua.
    expect(drivers.length).toBeGreaterThanOrEqual(10);
  });

  it.each(ids)("cenário %s: o dossiê da dupla fecha com o fixture", (id) => {
    const fixture = buildFakeScenario(id);
    const rows = [
      { role: "N1", salary: fixture.team.piloto_1_salario_anual, driver: fixture.drivers[0] },
      { role: "N2", salary: fixture.team.piloto_2_salario_anual, driver: fixture.drivers[1] },
    ];
    const dossier = lineupDossier({ drivers: fixture.drivers, rows });
    expect(dossier.hasGrid).toBe(true);
    expect(dossier.drivers.every((driver) => driver.hasDetail)).toBe(true);
    expect(dossier.drivers.every((driver) => driver.skill > 0 && driver.skill <= 100)).toBe(true);
  });

  it("a força da dupla acompanha o cenário: a dominante não tem a pior dupla do grid", () => {
    const forte = buildFakeScenario("dominante").drivers[0];
    const fraca = buildFakeScenario("vermelho").drivers[0];
    expect(forte.skill).toBeGreaterThan(fraca.skill);
  });

  it("o cenário de primeira rodada tem exatamente uma rodada registrada", () => {
    const { report } = buildFakeScenario("estreante");
    expect(report.cash_timeline).toHaveLength(1);
  });

  it.each(ids)("cenário %s: cada equipe do grid tem as 11 peças, e elas variam entre si", (id) => {
    const { teams, cars, team } = buildFakeScenario(id);
    expect(cars).toHaveLength(teams.length);
    expect(cars.map((car) => car.team_id)).toEqual(teams.map((row) => row.id));

    for (const car of cars) {
      expect(car.parts.map((part) => part.key)).toEqual(CAR_PART_KEYS);
      for (const part of car.parts) {
        expect(part.level).toBeGreaterThanOrEqual(1);
        expect(part.level).toBeLessThanOrEqual(10);
      }
    }

    // O radar só tem o que mostrar se as peças de uma equipe forem DESIGUAIS: onze
    // níveis idênticos desenham um polígono regular e ensinam a corrigir um problema
    // que não existe.
    const doJogador = cars.find((car) => car.team_id === team.id);
    const niveis = new Set(doJogador.parts.map((part) => part.level));
    expect(niveis.size).toBeGreaterThan(1);

    // E o radar precisa conseguir desenhar o payload inteiro.
    const radar = carPartsRadar({ cars, playerTeamId: team.id });
    expect(radar.hasData).toBe(true);
    expect(radar.player).toBeTruthy();
  });

  it("cenários fixos são determinísticos: mesma semente, mesmo desenho", () => {
    expect(buildFakeScenario("vermelho")).toEqual(buildFakeScenario("vermelho"));
  });
});
