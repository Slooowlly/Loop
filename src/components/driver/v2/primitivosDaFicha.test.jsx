import { render, screen } from "@testing-library/react";

import { DataRow, MedalKey, MetricIcon, MotivationBar } from "./primitivosDaFicha.jsx";

// Os tijolos saíram de `DriverDetailModalV2.jsx` em 11/08/2026. As seções que os
// usam já os cobrem por dentro da ficha; o que se guarda aqui são as regras que
// cada um carrega sozinho e que nenhuma seção repete.
describe("primitivos da ficha", () => {
  // O rank entra ANTES do valor e sem denominador: os totais são os mesmos em
  // todas as linhas do card, e repeti-los quebrava cada linha em duas.
  it("imprime os dois ordinais do recorde, sem total", () => {
    render(
      <DataRow
        label="Vitorias"
        value="12"
        recorde={{ grid: 2, grid_total: 18, mundo: 41, mundo_total: 503 }}
      />,
    );

    const marca = screen.getByTestId("dossier-rank");
    expect(marca).toHaveTextContent("2");
    expect(marca).toHaveTextContent("41");
    expect(marca).not.toHaveTextContent("503");
  });

  // População de um só não é rank: `grid_total` em 1 diria "1º de 1", que é
  // verdadeiro e não informa nada.
  it("omite a marca quando a populacao nao comporta rank", () => {
    render(
      <DataRow
        label="Vitorias"
        value="12"
        recorde={{ grid: 1, grid_total: 1, mundo: null, mundo_total: 503 }}
      />,
    );

    expect(screen.queryByTestId("dossier-rank")).not.toBeInTheDocument();
  });

  // A cor da motivação é o único sinal dela: são três faixas, e o valor ausente
  // cai em zero em vez de desenhar uma barra de largura indefinida.
  it("mapeia a motivacao nas tres faixas de cor", () => {
    const cor = (value) => {
      const { unmount } = render(<MotivationBar value={value} />);
      const barra = screen.getByTestId("driver-detail-motivation");
      // São dois `div` dentro da barra: o trilho e, dentro dele, o preenchido —
      // que é quem carrega a cor e a largura.
      const preenchida = barra.querySelectorAll("div")[1];
      const resultado = { cor: preenchida.style.backgroundColor, largura: preenchida.style.width };
      unmount();
      return resultado;
    };

    expect(cor(82)).toEqual({ cor: "rgb(63, 185, 80)", largura: "82%" });
    expect(cor(55)).toEqual({ cor: "rgb(210, 153, 34)", largura: "55%" });
    expect(cor(12)).toEqual({ cor: "rgb(248, 81, 73)", largura: "12%" });
    expect(cor(undefined)).toEqual({ cor: "rgb(248, 81, 73)", largura: "0%" });
  });

  it("nao desenha icone para metrica que nao tem um", () => {
    const { container } = render(<MetricIcon name="inexistente" />);
    expect(container).toBeEmptyDOMElement();
  });

  it("desenha o icone da metrica conhecida", () => {
    const { container } = render(<MetricIcon name="vitorias" />);
    expect(container.querySelector("svg")).toBeInTheDocument();
  });

  it("pinta a chave de medalha com a cor recebida", () => {
    const { container } = render(<MedalKey color="#e6b34d" label="Vitoria" />);
    expect(container).toHaveTextContent("Vitoria");
    expect(container.querySelector("span > span").style.backgroundColor).toBe("rgb(230, 179, 77)");
  });
});
