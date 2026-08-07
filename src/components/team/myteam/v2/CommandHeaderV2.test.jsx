import { render, screen } from "@testing-library/react";

import CommandHeaderV2 from "./CommandHeaderV2";

// A faixa de comando tem uma regra que os testes precisam segurar: chip é exceção.
// Equipe em ordem não ganha chip nenhum — o centro da linha fica vazio, e é isso que
// faz qualquer chip futuro ser lido como alarme.
function renderHeader({ team = {}, ...props } = {}) {
  return render(
    <CommandHeaderV2
      team={{ id: 1, nome: "Racing Academy Red", cor_primaria: "#c0392b", cash_balance: 1_687_560, debt_balance: 0, ...team }}
      teams={[
        { id: 1, cash_balance: 1_687_560 },
        { id: 2, cash_balance: 2_100_000 },
      ]}
      standing={{ posicao: 0, pontos: 0 }}
      gridSize={6}
      roundNet={36_283}
      projectedAnnual={0}
      hasProjection={false}
      payroll={31_821}
      salaryCeiling={51_604}
      {...props}
    />,
  );
}

describe("CommandHeaderV2", () => {
  it("não desenha chip quando não há dívida nem folha apertada", () => {
    renderHeader();
    expect(screen.getByTestId("my-team-v2-alerts")).toBeEmptyDOMElement();
  });

  it("junta posição, pontos e ranking de caixa numa sublinha só", () => {
    renderHeader();
    expect(screen.getByTestId("my-team-v2-identity-line").textContent).toBe(
      "Posição ainda não definida · 0 ponto · 2º maior caixa de 2",
    );
  });

  it("mostra fôlego ao lado da rodada enquanto não há projeção", () => {
    renderHeader();
    expect(screen.getByTestId("my-team-v2-cash-line").textContent).toContain("+$36,283 na rodada");
    expect(screen.getByTestId("my-team-v2-cash-line").textContent).toContain("Fôlego");
  });

  it("troca o fôlego pela projeção do ano quando ela existe", () => {
    renderHeader({ hasProjection: true, projectedAnnual: -120_000 });
    expect(screen.getByTestId("my-team-v2-cash-line").textContent).toContain("Ano projetado em -$120,000");
    expect(screen.getByTestId("my-team-v2-cash-line").textContent).not.toContain("Fôlego");
  });

  it("levanta chip de dívida e de folha só quando eles apertam", () => {
    renderHeader({ team: { debt_balance: 250_000 }, payroll: 45_000 });
    expect(screen.getByText("Dívida $250,000")).toBeInTheDocument();
    expect(screen.getByTestId("my-team-v2-payroll-chip")).toBeInTheDocument();
  });
});
