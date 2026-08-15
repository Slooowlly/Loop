import { render, screen } from "@testing-library/react";

import WeatherForecastStrip from "./WeatherForecastStrip";
import { COND } from "./weatherTimelineData";

// Uma corrida que começa no sol, encobre no meio e termina na chuva.
const TIMELINE = {
  intensity: "Moderada",
  scenario: "Frente fria chegando",
  is_wet_race: true,
  points: [
    { frac: 0.6, event_type: 3 },
    { frac: 0.0, event_type: 0 },
    { frac: 0.85, event_type: 7 },
    { frac: 0.3, event_type: 0 },
  ],
};

describe("WeatherForecastStrip (prévia do clima no card de Condição de Pista)", () => {
  it("desenha o gradiente na ordem da corrida e um ícone por mudança de condição", () => {
    const { container } = render(
      <WeatherForecastStrip careerId={1} raceId={9} mockData={TIMELINE} />,
    );

    // Os pontos chegam fora de ordem do backend e precisam sair ordenados por
    // fração: um `stop` deslocado inverte o sentido do gradiente inteiro.
    const stops = [...container.querySelectorAll("stop")];
    expect(stops.map((s) => s.getAttribute("offset"))).toEqual([
      "0.00%",
      "30.00%",
      "60.00%",
      "85.00%",
    ]);
    expect(stops.map((s) => s.getAttribute("stop-color"))).toEqual([
      COND[0].c,
      COND[0].c,
      COND[3].c,
      COND[7].c,
    ]);

    // Sol → sol não é mudança, então são três ícones para quatro pontos.
    const tira = screen.getByTestId("weather-forecast-strip");
    expect(tira.textContent).toContain(COND[0].icon);
    expect(tira.textContent).toContain(COND[3].icon);
    expect(tira.textContent).toContain(COND[7].icon);
  });

  it("sem carreira ou corrida a tira some, em vez de virar um erro dentro do card", () => {
    render(<WeatherForecastStrip careerId={null} raceId={null} />);

    expect(screen.queryByTestId("weather-forecast-strip")).toBeNull();
    expect(screen.queryByTestId("weather-forecast-strip-skeleton")).toBeNull();
  });

  it("linha do tempo vazia não desenha uma faixa em branco", () => {
    render(<WeatherForecastStrip careerId={1} raceId={9} mockData={{ points: [] }} />);

    expect(screen.queryByTestId("weather-forecast-strip")).toBeNull();
  });
});
