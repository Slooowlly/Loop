import { render, screen } from "@testing-library/react";

import { RecentFormStrip } from "./RecentFormStrip.jsx";

// A faixa saiu de dentro de `DriverDetailModalV2.jsx` em 11/08/2026. O que ela
// desenha dentro da ficha continua guardado pela fatia `*.curvaDeCampeonato` —
// os casos daqui são os que só existem depois do corte: a faixa monta com as
// SUAS props, sem payload de ficha, sem store e sem a ponte do Tauri.
describe("RecentFormStrip", () => {
  const temporadas = [
    {
      season_number: 5,
      ano: 2025,
      atual: false,
      resultados: [
        { rodada: 1, chegada: 4, dnf: false },
        { rodada: 2, chegada: 18, dnf: false },
        { rodada: 3, chegada: null, dnf: true },
      ],
    },
    {
      season_number: 6,
      ano: 2026,
      atual: true,
      resultados: [
        { rodada: 1, chegada: 1, dnf: false },
        { rodada: 2, chegada: 4, dnf: false },
      ],
    },
  ];

  // A coluna de resultado é o segundo `div` de dentro do alvo: o primeiro é o
  // trilho vazio, que existe para um último lugar não ficar sem pixel algum.
  const alturaDaColuna = (strip, ano, rodada) =>
    strip.querySelector(`[data-season="${ano}"] [data-round="${rodada}"] div:last-of-type`).style
      .height;

  it("monta sozinha, so com as temporadas", () => {
    render(<RecentFormStrip seasons={temporadas} entries={[]} context={null} />);

    const strip = screen.getByTestId("driver-detail-form-strip");
    const grupos = strip.querySelectorAll("[data-season]");
    expect(grupos).toHaveLength(2);
    expect(grupos[0]).toHaveAttribute("data-season", "2025");
    expect(grupos[1]).toHaveAttribute("data-current", "true");
    expect(strip.querySelector('[data-round="3"]')).toHaveAttribute("data-dnf", "true");
  });

  // A régua é COMUM aos grupos. Com uma escala por temporada, o P4 de 2025
  // desenharia mais alto que o P4 de 2026 só porque o pior resultado do ano foi
  // outro, e a faixa deixaria de comparar as duas.
  it("mede as duas temporadas pela mesma regua", () => {
    render(<RecentFormStrip seasons={temporadas} entries={[]} context={null} />);
    const strip = screen.getByTestId("driver-detail-form-strip");

    expect(alturaDaColuna(strip, 2025, 1)).toBe(alturaDaColuna(strip, 2026, 2));
    // O piso da escala é P20 mesmo quando ninguém chegou tão atrás: P1 é a
    // coluna cheia, e o P18 do ano fechado não vira o novo fundo do eixo.
    expect(alturaDaColuna(strip, 2026, 1)).toBe("100%");
    expect(Number.parseFloat(alturaDaColuna(strip, 2025, 2))).toBeCloseTo((2 / 19) * 100, 5);
  });

  it("cai numa faixa unica quando so ha a janela antiga do payload", () => {
    render(
      <RecentFormStrip
        seasons={null}
        entries={[
          { rodada: 1, chegada: 3, dnf: false },
          { rodada: 2, chegada: 5, dnf: false },
        ]}
        context={null}
      />,
    );

    const strip = screen.getByTestId("driver-detail-form-strip");
    expect(strip.querySelectorAll("[data-season]")).toHaveLength(0);
    expect(strip.querySelectorAll("[data-round]")).toHaveLength(2);
  });

  // Sem corrida nenhuma a faixa não some da tela: vira a frase que diz POR QUE
  // não há nada, e o motivo sai do contexto — os três casos são textos
  // diferentes, e nenhum deles é a faixa vazia.
  it("troca a faixa pela explicacao quando nao ha corrida", () => {
    const textos = new Set();
    for (const context of [null, "sem_corridas_temporada_passada", "sem_time_temporada_passada"]) {
      const { container, unmount } = render(
        <RecentFormStrip seasons={[]} entries={[]} context={context} />,
      );
      expect(screen.queryByTestId("driver-detail-form-strip")).not.toBeInTheDocument();
      textos.add(container.textContent);
      unmount();
    }
    expect(textos.size).toBe(3);
  });
});
