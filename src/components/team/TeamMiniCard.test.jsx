import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import TeamMiniCard from "./TeamMiniCard";

function equipe(overrides = {}) {
  return {
    id: "T1",
    nome: "Belgian Racing Team",
    cor_primaria: "#58a6ff",
    founded_year: 2004,
    temp_posicao: 2,
    pontos: 300,
    vitorias: 6,
    cash_balance: 4_000_000,
    car_level: 9,
    car_performance: 90,
    confiabilidade: 88,
    pit_crew_quality: 91,
    categoria_titulos: 7,
    categoria_vitorias: 35,
    ...overrides,
  };
}

function montar(team = equipe()) {
  return render(
    <TeamMiniCard team={team}>
      <span>Logo</span>
    </TeamMiniCard>,
  );
}

describe("TeamMiniCard", () => {
  it("abre no clique e mostra identidade, temporada e estrutura", async () => {
    montar();
    fireEvent.click(screen.getByText("Logo"));

    const ficha = await screen.findByTestId("team-mini-card");
    expect(ficha.textContent).toContain("Belgian Racing Team");
    expect(ficha.textContent).toContain("Desde 2004");
    expect(ficha.textContent).toContain("300");
    // Tiers, e nao os numeros crus: "88 de confiabilidade" nao responde se isso
    // e bom — o rotulo responde, e e o mesmo do ranking da categoria.
    expect(ficha.textContent).toContain("Dominante");
    expect(ficha.textContent).toContain("Inquebrável");
    expect(ficha.textContent).toContain("7 títulos");
  });

  it("fecha no Escape", async () => {
    montar();
    fireEvent.click(screen.getByText("Logo"));
    await screen.findByTestId("team-mini-card");

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByTestId("team-mini-card")).toBeNull());
  });

  it("equipe sem titulo nem vitoria na categoria diz isso em vez de mostrar zeros", async () => {
    montar(equipe({ categoria_titulos: 0, categoria_vitorias: 0 }));
    fireEvent.click(screen.getByText("Logo"));

    const ficha = await screen.findByTestId("team-mini-card");
    expect(ficha.textContent).toContain("Sem título nem vitória nesta categoria");
  });

  // O logo mora dentro de um card cujo duplo clique abre o historico mundial da
  // equipe. Abrir a ficha nao pode disparar aquilo junto.
  it("o clique no gatilho nao vaza para o card em volta", async () => {
    const noCard = vi.fn();
    render(
      <div onClick={noCard}>
        <TeamMiniCard team={equipe()}>
          <span>Logo</span>
        </TeamMiniCard>
      </div>,
    );

    fireEvent.click(screen.getByText("Logo"));
    await screen.findByTestId("team-mini-card");
    expect(noCard).not.toHaveBeenCalled();
  });
});
