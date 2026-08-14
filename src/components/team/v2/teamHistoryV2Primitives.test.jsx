import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  BlockLabel,
  HighlightTrophy,
  HistoryStateMessage,
  InfoCard,
  MedalKey,
  MetricIcon,
  MiniMetric,
} from "./teamHistoryV2Primitives.jsx";

// As primitivas saíram do drawer em 11/08/2026 e até então só eram exercitadas
// de lado, por um teste de tela inteira que afirmava outra coisa. O que elas
// prometem é curto e vale trava própria: uma delas some quando não tem o que
// desenhar, outra proíbe caixa alta por decisão de legibilidade registrada no
// código, e o ícone de métrica escolhe pelo `id` e não pelo rótulo traduzido.

describe("MetricIcon", () => {
  it("desenha o icone da metrica conhecida", () => {
    const { container } = render(<MetricIcon name="titles" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg).toHaveAttribute("aria-hidden", "true");
  });

  it("nao desenha nada para metrica desconhecida", () => {
    // Um ícone genérico seria pior que nenhum: ele afirma um significado que o
    // payload não tem.
    const { container } = render(<MetricIcon name="lap_record" />);
    expect(container.querySelector("svg")).toBeNull();
  });

  it("escolhe pelo id, e nao pelo rotulo traduzido", () => {
    // "titles" e "wins" pegam ícones diferentes; o rótulo que o jogador lê nunca
    // entra na conta, senão trocar de idioma trocaria o desenho.
    const titulos = render(<MetricIcon name="titles" />).container.querySelector("svg").innerHTML;
    const vitorias = render(<MetricIcon name="wins" />).container.querySelector("svg").innerHTML;
    expect(titulos).not.toBe(vitorias);
    expect(render(<MetricIcon name="Títulos" />).container.querySelector("svg")).toBeNull();
  });

  it("respeita o tamanho pedido e mantem a espessura da grade", () => {
    const svg = render(<MetricIcon name="podiums" size={22} />).container.querySelector("svg");
    expect(svg).toHaveAttribute("width", "22");
    expect(svg).toHaveAttribute("stroke-width", "1.5");
  });
});

describe("BlockLabel", () => {
  it("nao usa caixa alta nem espacamento largo", () => {
    // A combinação `uppercase tracking-[0.15em] text-[10px] text-text-muted`
    // obrigava a soletrar o rótulo, e a decisão de abandoná-la está registrada
    // no comentário do componente. Esta trava existe para ela não voltar por
    // descuido num arquivo que agora é importado por oito painéis.
    const { container } = render(<BlockLabel>Confiabilidade</BlockLabel>);
    const classe = container.firstChild.className;
    expect(classe).not.toMatch(/uppercase/);
    expect(classe).not.toMatch(/tracking-\[/);
    expect(screen.getByText("Confiabilidade")).toBeInTheDocument();
  });
});

describe("MedalKey", () => {
  it("pinta o quadrado com a cor recebida e mantem o rotulo ao lado", () => {
    const { container } = render(<MedalKey color="#f2c46d" label="1º" />);
    expect(container.querySelector("span > span")).toHaveStyle({ backgroundColor: "#f2c46d" });
    expect(screen.getByText("1º")).toBeInTheDocument();
  });
});

describe("MiniMetric e InfoCard", () => {
  it("MiniMetric mostra rotulo e valor", () => {
    render(<MiniMetric label="Temporadas" value="7" />);
    expect(screen.getByText("Temporadas")).toBeInTheDocument();
    expect(screen.getByText("7")).toBeInTheDocument();
  });

  it("InfoCard cala o detalhe quando ele vem vazio", () => {
    const { container, rerender } = render(<InfoCard label="Folha" value="R$ 1,2 mi" />);
    expect(container.querySelector("p")).toBeNull();
    rerender(<InfoCard label="Folha" value="R$ 1,2 mi" detail="Dois pilotos" />);
    expect(screen.getByText("Dois pilotos")).toBeInTheDocument();
  });
});

describe("HistoryStateMessage", () => {
  it("mostra o erro do dossie quando o carregamento falhou", () => {
    render(<HistoryStateMessage dossier={{ historyStatus: "error", historyError: "Save corrompido" }} />);
    expect(screen.getByText("Save corrompido")).toBeInTheDocument();
  });

  it("cai no texto de carregando fora do estado de erro", () => {
    render(<HistoryStateMessage dossier={{ historyStatus: "loading", historyError: "Save corrompido" }} />);
    expect(screen.queryByText("Save corrompido")).toBeNull();
  });
});

describe("HighlightTrophy", () => {
  it("e ornamento: nao entra na arvore de acessibilidade", () => {
    const { container } = render(<HighlightTrophy />);
    const img = container.querySelector("img");
    expect(img).toHaveAttribute("alt", "");
    expect(img).toHaveAttribute("aria-hidden", "true");
  });
});
