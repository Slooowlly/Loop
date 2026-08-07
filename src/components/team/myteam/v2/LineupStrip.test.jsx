import { render, screen } from "@testing-library/react";

import LineupStrip from "./LineupStrip";
import { garageClimate } from "../teamMetrics";

// A faixa só desenha número que o payload sustenta: o retorno da presença e as
// inversões de hierarquia só aparecem quando aconteceram de fato.
function renderStrip(overrides = {}) {
  return render(
    <LineupStrip
      presence={53.2}
      climate={garageClimate({ hierarquia_status: "estavel", hierarquia_tensao: 0 })}
      sponsorshipIncome={80_000}
      gateIncome={12_000}
      {...overrides}
    />,
  );
}

describe("LineupStrip", () => {
  it("conta o que a presença rendeu na última rodada", () => {
    renderStrip();

    expect(screen.getByTestId("bond-presence-meter")).toHaveTextContent(
      "Na última rodada rendeu $80,000 de patrocínio e $12,000 de bilheteria.",
    );
  });

  it("omite a bilheteria quando a linha vem zerada", () => {
    renderStrip({ gateIncome: 0 });

    const meter = screen.getByTestId("bond-presence-meter");
    expect(meter).toHaveTextContent("Na última rodada rendeu $80,000 de patrocínio.");
    expect(meter).not.toHaveTextContent("bilheteria");
  });

  it("esconde a presença quando não há lineup lido", () => {
    renderStrip({ presence: 0 });

    expect(screen.queryByTestId("bond-presence-meter")).toBeNull();
    // O clima continua: ele não depende da mídia da dupla.
    expect(screen.getByTestId("bond-climate")).toBeTruthy();
  });

  it("só cita inversões quando houve alguma", () => {
    renderStrip();
    expect(screen.queryByTestId("bond-inversions")).toBeNull();

    renderStrip({
      climate: garageClimate({
        hierarquia_status: "inversao",
        hierarquia_tensao: 40,
        hierarquia_inversoes_temporada: 1,
      }),
    });
    expect(screen.getAllByTestId("bond-inversions")[0]).toHaveTextContent(
      "1 inversão de hierarquia nesta temporada.",
    );
  });
});
