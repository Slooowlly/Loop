import { render, screen } from "@testing-library/react";

import { MarketSection } from "./MarketSection.jsx";

// A aba Mercado saiu de dentro de `DriverDetailModalV2.jsx` em 11/08/2026. Os
// casos de conteúdo continuam em `DriverDetailModalV2.mercado.test.jsx`, que
// entra pela ficha; aqui guarda-se o que o corte prometeu: a seção monta com o
// `detail` e mais nada — sem store, sem `invoke`, sem o estado de abas.
//
// Os dados são locais de propósito: puxar o `driverDetailV2TestKit` traria o
// modal inteiro junto, e o arquivo deixaria de provar que a seção anda sozinha.
describe("MarketSection", () => {
  const contrato = (overrides = {}) => ({
    equipe_nome: "Arclight",
    papel: "Numero2",
    salario_anual: 960534,
    ano_inicio: 2026,
    ano_fim: 2026,
    anos_restantes: 0,
    status: "ativo",
    ...overrides,
  });

  const curva = () => [
    { season_number: 4, ano: 2025, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", salario_contrato: 960534, salario_mercado: 980000, atual: false },
    { season_number: 5, ano: 2026, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", salario_contrato: 960534, salario_mercado: 1300000, atual: true },
  ];

  const mercado = (overrides = {}) => ({
    valor_mercado: 3341311,
    salario_estimado: 1300000,
    chance_transferencia: 57,
    forcas_transferencia: { contrato: 54, motivacao: 0, mercado: 3 },
    posicao_valor: 3,
    total_valor: 18,
    categoria_valor: "gt3",
    ...overrides,
  });

  it("monta sozinha, so com o contrato_mercado", () => {
    render(
      <MarketSection detail={{ contrato_mercado: { contrato: contrato(), mercado: mercado(), curva: curva() } }} />,
    );

    expect(screen.getByTestId("driver-detail-transfer-chance")).toHaveTextContent("57%");
    expect(screen.getByTestId("driver-detail-situation")).toBeInTheDocument();
    expect(screen.getByTestId("driver-detail-market-curve")).toBeInTheDocument();
    expect(screen.getByTestId("driver-detail-market-rank")).toBeInTheDocument();
  });

  // A barra empilhada é a decomposição da MESMA chance: as parcelas fecham no
  // total, então a força zerada não desenha faixa e a legenda dela fica apagada.
  it("desenha uma faixa por forca viva e apaga a legenda da forca zerada", () => {
    render(
      <MarketSection detail={{ contrato_mercado: { contrato: contrato(), mercado: mercado(), curva: curva() } }} />,
    );

    const medidor = screen.getByTestId("driver-detail-transfer-meter");
    expect([...medidor.querySelectorAll("[data-forca]")].map((f) => f.dataset.forca)).toEqual([
      "contrato",
      "mercado",
    ]);
    expect(medidor.querySelector('[data-forca-key="motivacao"]').className).toContain("opacity-40");
  });

  // Sem mercado não há termômetro nem card de situação: sobra a curva e a frase.
  // A seção não pode explodir com o payload magro de um save antigo.
  it("sobrevive ao payload sem mercado", () => {
    render(<MarketSection detail={{ contrato_mercado: { contrato: contrato(), curva: null } }} />);

    expect(screen.queryByTestId("driver-detail-transfer-meter")).not.toBeInTheDocument();
    expect(screen.queryByTestId("driver-detail-situation")).not.toBeInTheDocument();
  });

  it("sobrevive ao detail vazio", () => {
    const { container } = render(<MarketSection detail={{}} />);
    expect(container.querySelector("section")).toBeInTheDocument();
  });

  // A régua de vigência é o eixo do tempo do card: um trecho por temporada
  // contratada, cheio no que já foi cumprido.
  it("desenha a regua com um trecho por temporada do contrato", () => {
    render(
      <MarketSection
        detail={{
          contrato_mercado: {
            contrato: contrato({ ano_inicio: 2024, ano_fim: 2026, anos_restantes: 1 }),
            mercado: mercado(),
            curva: curva(),
          },
        }}
      />,
    );

    const regua = screen.getByTestId("driver-detail-contract-ruler");
    const trechos = [...regua.querySelectorAll("[data-temporada]")];
    expect(trechos.map((t) => t.dataset.temporada)).toEqual(["2024", "2025", "2026"]);
    expect(trechos.map((t) => t.dataset.cumprida)).toEqual(["true", "true", undefined]);
  });
});
