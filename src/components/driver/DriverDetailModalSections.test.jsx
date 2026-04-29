import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { HistorySection, MarketSection, SummarySection } from "./DriverDetailModalSections";

function Section({ title, children }) {
  return (
    <section>
      <h2>{title}</h2>
      {children}
    </section>
  );
}

function renderSummary(resumoAtual, detailOverrides = {}) {
  return render(
    <SummarySection
      SectionComponent={Section}
      detail={{
        stats_carreira: { corridas: 5 },
        stats_temporada: { pontos: 0 },
        performance: { temporada: {} },
        forma: { momento: "em_baixa", ultimas_10: [] },
        resumo_atual: resumoAtual,
        ...detailOverrides,
      }}
      moment={{ label: "Em baixa", color: "text-[#f85149]" }}
    />,
  );
}

describe("SummarySection", () => {
  it("uses the backend summary tone instead of always rendering the verdict as green", () => {
    renderSummary({ veredito: "Crítico", tom: "danger" });

    const card = screen.getByTestId("current-summary-verdict-card");
    expect(screen.getByText("Crítico")).toBeInTheDocument();
    expect(card).toHaveAttribute("data-summary-tone", "danger");
    expect(card).toHaveClass("border-[#f85149]/25");
    expect(card).not.toHaveClass("border-[#3fb950]/20");
  });

  it("renders a good verdict with the correct green background tone", () => {
    renderSummary({ veredito: "Bom", tom: "success" });

    const card = screen.getByTestId("current-summary-verdict-card");
    expect(screen.getByText("Bom")).toBeInTheDocument();
    expect(card).toHaveAttribute("data-summary-tone", "success");
    expect(card).toHaveClass("border-[#3fb950]/25");
    expect(card).toHaveClass("bg-[#3fb950]/10");
  });

  it("makes a previous season without team explicit instead of showing insufficient data", () => {
    renderSummary(
      { veredito: "Avaliação", tom: "info" },
      {
        forma: {
          momento: "sem_dados",
          contexto: "sem_time_temporada_passada",
          ultimas_10: [],
        },
      },
    );

    expect(screen.getByText("Fora do grid")).toBeInTheDocument();
    expect(screen.getByText("Sem time na temporada passada")).toBeInTheDocument();
    expect(screen.queryByText("Dados insuficientes")).not.toBeInTheDocument();
  });
});

describe("HistorySection", () => {
  it("renders the extended career history as one visually separated block", () => {
    render(
      <HistorySection
        SectionComponent={Section}
        detail={{
          stats_carreira: { corridas: 31, vitorias: 4, podios: 9 },
          rankings_carreira: {},
        }}
        trajetoria={{
          titulos: 0,
          historico: {
            presenca: {
              tempo_carreira: 7,
              temporadas_disputadas: 4,
              anos_desempregado: 3,
              periodos_desempregado: ["2022->2023", "2025"],
              corridas: 31,
              categorias_disputadas: 4,
            },
            primeiros_marcos: {
              primeiro_podio_corrida: 3,
              primeira_vitoria_corrida: 7,
              primeiro_dnf_corrida: 4,
            },
            auge: {
              melhor_temporada: {
                ano: 2024,
                categoria: "bmw_m2",
                posicao_campeonato: 1,
                pontos: 220,
              },
              maior_sequencia_vitorias: 3,
            },
            mobilidade: {
              promocoes: 2,
              rebaixamentos: 1,
              equipes_defendidas: 3,
              tempo_medio_por_equipe: 1.3,
            },
            eventos_especiais: {
              participacoes: 2,
              convocacoes: 2,
              vitorias: 1,
              podios: 3,
              rankings: {
                participacoes: 5,
                convocacoes: 5,
                vitorias: 9,
                podios: 4,
              },
              melhor_campanha: {
                ano: 2028,
                categoria: "endurance",
                classe: "gt4",
                equipe: "Heart of Racing",
                pontos: 42,
              },
              ultimo_evento: {
                ano: 2028,
                categoria: "endurance",
                classe: "gt4",
                equipe: "Heart of Racing",
              },
              timeline: [
                { ano: 2026, categoria: "production_challenger", classe: "bmw", equipe: "Bayern Division" },
                { ano: 2028, categoria: "endurance", classe: "gt4", equipe: "Heart of Racing" },
              ],
            },
          },
          ano_estreia: 2017,
          equipe_estreia: "Roadster Touring Club",
          categorias_timeline: [
            { categoria: "mazda_rookie", ano_inicio: 2017, ano_fim: 2021 },
            { categoria: "mazda_amador", ano_inicio: 2022, ano_fim: 2024 },
            { categoria: "mazda_rookie", ano_inicio: 2025, ano_fim: 2025 },
          ],
          marcos: [],
        }}
      />,
    );

    const dossier = screen.getByTestId("career-history-dossier");
    expect(dossier).toBeInTheDocument();
    expect(screen.getByText("Tempo de carreira")).toBeInTheDocument();
    expect(screen.getByText("7 anos")).toBeInTheDocument();
    expect(screen.getByText("PRESENÇA")).toBeInTheDocument();
    expect(screen.getByText("Anos desempregado")).toBeInTheDocument();
    expect(screen.getByText("3 anos (2022->2023 | 2025)")).toBeInTheDocument();
    expect(screen.queryAllByText("Corridas")).toHaveLength(1);
    expect(screen.getByText("Primeira vitória")).toBeInTheDocument();
    expect(screen.getByText("7ª corrida")).toBeInTheDocument();
    expect(screen.getByText("AUGE")).toBeInTheDocument();
    expect(screen.getByText("2024, BMW M2")).toBeInTheDocument();
    expect(screen.getByText("MOBILIDADE")).toBeInTheDocument();
    expect(screen.getByText("Promoções")).toBeInTheDocument();
    expect(screen.getByText("EVENTOS ESPECIAIS")).toBeInTheDocument();
    expect(screen.getByText("Participações")).toBeInTheDocument();
    expect(screen.getAllByText("2 (5\u00ba)").length).toBeGreaterThan(0);
    expect(screen.getByText("1 (9\u00ba)")).toBeInTheDocument();
    expect(screen.getByText("3 (4\u00ba)")).toBeInTheDocument();
    expect(screen.getByText("2028, Endurance GT4")).toBeInTheDocument();
    expect(screen.getByText("2026 Production BMW - Bayern Division")).toBeInTheDocument();
    expect(screen.getAllByText("2028 Endurance GT4 - Heart of Racing").length).toBeGreaterThan(0);
    expect(screen.getByAltText("Roadster Touring Club logo")).toBeInTheDocument();
    expect(screen.queryByText("Vivência atual")).not.toBeInTheDocument();
    expect(screen.getByText("Mazda Rookie 2017")).toBeInTheDocument();
    expect(screen.getByText("Mazda Championship 2022")).toBeInTheDocument();
    expect(screen.getByText("Mazda Rookie 2025")).toBeInTheDocument();
  });
});

describe("MarketSection", () => {
  it("integrates quality and performance reading into the market tab", () => {
    render(
      <MarketSection
        SectionComponent={Section}
        detail={{
          nome: "Arthur Lefebvre",
          stats_carreira: { corridas: 31, vitorias: 4, podios: 9 },
          trajetoria: { titulos: 0 },
          leitura_tecnica: {
            itens: [
              { chave: "velocidade", label: "Velocidade", nivel: "Forte", tom: "info" },
            ],
          },
          leitura_desempenho: {
            entregue_posicao: 4,
            esperado_posicao: 7,
            delta_posicao: 3,
            car_performance: 72.4,
            piloto_pontos: 88,
            companheiro_nome: "Companheiro",
            companheiro_pontos: 54,
            leitura: "Entrega acima do pacote.",
          },
          contrato_mercado: {
            contrato: {
              equipe_nome: "Roadster Touring Club",
              papel: "Numero1",
              salario_anual: 120000,
              ano_inicio: 2024,
              ano_fim: 2026,
              anos_restantes: 1,
            },
          },
        }}
        market={{
          valor_mercado: 900000,
          salario_estimado: 120000,
          chance_transferencia: 22,
        }}
      />,
    );

    expect(screen.getByText("Contrato e Mercado")).toBeInTheDocument();
    expect(screen.getByText("Mapa de Qualidade")).toBeInTheDocument();
    expect(screen.getByText("Leitura de Desempenho")).toBeInTheDocument();
    expect(screen.getByText("Velocidade")).toBeInTheDocument();
    expect(screen.getByText("Entrega acima do pacote.")).toBeInTheDocument();
  });
});
