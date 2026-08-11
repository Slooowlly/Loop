import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import CalendarTabRedesign from "./CalendarTabRedesign";

// A aba de Calendário. Toda a busca mora em `useCalendarData`; o que sobra na tela é
// NAVEGAÇÃO — qual mês está em foco, quando os botões param nas bordas do ano, e a lista
// de próximas etapas que leva a grade até o mês de uma corrida.
//
// A navegação é o que já quebrou aqui em silêncio: "Ver todos" chegou a chamar `goToday`,
// o mesmo handler do botão "Hoje", e o clique trocava o mês da grade sem expandir a lista.
// O modo de falha é sempre esse — nada estoura, o botão só não faz o que o rótulo diz.

const dadosDoCalendario = vi.fn();
vi.mock("../../components/calendar/useCalendarData.js", () => ({
  default: (...args) => dadosDoCalendario(...args),
}));

// A grade e as miniaturas são desenho puro; aqui interessa QUE mês elas recebem.
vi.mock("../../components/calendar/DayCellV2.jsx", () => ({
  default: ({ day, outside }) => (
    <div data-testid="celula" data-outside={String(outside)}>
      {day}
    </div>
  ),
}));
vi.mock("../../components/calendar/MiniMonth.jsx", () => ({
  default: ({ month, onOpen }) => (
    <button type="button" data-testid="mini-mes" onClick={() => onOpen(month)}>
      mini {month}
    </button>
  ),
}));
vi.mock("../../components/calendar/CalendarTicketTooltip.jsx", () => ({
  default: () => <div data-testid="tooltip" />,
}));

const ANO = 2026;

function corrida(id, displayDate, extra = {}) {
  return {
    id,
    display_date: displayDate,
    rodada: Number(id.replace("R", "")),
    track_name: `Pista ${id}`,
    status: "Pendente",
    ...extra,
  };
}

const PROXIMAS = [
  corrida("R1", `${ANO}-02-08`),
  corrida("R2", `${ANO}-03-15`),
  corrida("R3", `${ANO}-04-12`),
  corrida("R4", `${ANO}-05-10`),
  corrida("R5", `${ANO}-06-14`),
  corrida("R6", `${ANO}-09-20`),
  corrida("R7", `${ANO}-10-18`),
];

function comDados(overrides = {}) {
  dadosDoCalendario.mockReturnValue({
    careerId: "C1",
    playerTeam: { categoria: "gt3" },
    nextRace: PROXIMAS[0],
    season: { ano: ANO, fase: "Temporada" },
    temporalSummary: { days_until_next_event: 6 },
    loading: false,
    showLoadingUI: false,
    error: "",
    isLegacyCalendar: false,
    displayedCalendar: PROXIMAS,
    seasonYear: ANO,
    racesByDate: {},
    otherCategoryRacesByDate: {},
    currentDateParts: { year: ANO, month: 1, day: 2 },
    nextRaceEntry: PROXIMAS[0],
    upcoming: PROXIMAS,
    stats: { total: 7, done: 2, countries: 5, durationMin: 210, wet: 1, specials: 0 },
    ...overrides,
  });
}

const mesEmFoco = () =>
  document.querySelector("h3.kcal").textContent.replace(/\s+/g, " ").trim();

beforeEach(() => {
  dadosDoCalendario.mockReset();
  comDados();
});

describe("CalendarTabRedesign — mês em foco", () => {
  it("abre no mês atual do jogador, e não em janeiro", () => {
    render(<CalendarTabRedesign activeTab="calendar" />);
    // `currentDateParts.month` é 1 (fevereiro).
    expect(mesEmFoco()).toBe(`Fevereiro ${ANO}`);
  });

  it("sem data atual, cai no mês da próxima corrida", () => {
    comDados({ currentDateParts: null });
    render(<CalendarTabRedesign activeTab="calendar" />);
    expect(mesEmFoco()).toBe(`Fevereiro ${ANO}`);
  });

  it("as setas andam um mês por clique", () => {
    render(<CalendarTabRedesign activeTab="calendar" />);

    fireEvent.click(screen.getByRole("button", { name: /mês seguinte|próximo mês/i }));
    expect(mesEmFoco()).toBe(`Março ${ANO}`);

    fireEvent.click(screen.getByRole("button", { name: /mês anterior/i }));
    expect(mesEmFoco()).toBe(`Fevereiro ${ANO}`);
  });

  it("as setas param nas bordas do ano em vez de dar a volta", () => {
    comDados({ currentDateParts: { year: ANO, month: 0, day: 5 } });
    const { unmount } = render(<CalendarTabRedesign activeTab="calendar" />);
    expect(screen.getByRole("button", { name: /mês anterior/i })).toBeDisabled();
    unmount();

    comDados({ currentDateParts: { year: ANO, month: 11, day: 5 } });
    render(<CalendarTabRedesign activeTab="calendar" />);
    expect(screen.getByRole("button", { name: /mês seguinte|próximo mês/i })).toBeDisabled();
  });

  it("desenha sempre a grade fixa de 42 células", () => {
    render(<CalendarTabRedesign activeTab="calendar" />);
    expect(screen.getAllByTestId("celula")).toHaveLength(42);
  });

  it("mostra os meses SEGUINTES em miniatura, e nenhum já passado", () => {
    render(<CalendarTabRedesign activeTab="calendar" />);
    // Em foco fevereiro (1) → dez miniaturas, de março a dezembro.
    const minis = screen.getAllByTestId("mini-mes");
    expect(minis).toHaveLength(10);
    expect(minis[0]).toHaveTextContent("mini 2");
    expect(minis.at(-1)).toHaveTextContent("mini 11");
  });

  it("clicar numa miniatura traz aquele mês para o foco", () => {
    render(<CalendarTabRedesign activeTab="calendar" />);
    // As miniaturas começam em março (2), então o índice 2 é maio.
    fireEvent.click(screen.getAllByTestId("mini-mes")[2]);
    expect(mesEmFoco()).toBe(`Maio ${ANO}`);
  });

  it("o botão 'Hoje' volta ao mês atual depois de navegar para longe", () => {
    render(<CalendarTabRedesign activeTab="calendar" />);
    fireEvent.click(screen.getAllByTestId("mini-mes")[5]);
    expect(mesEmFoco()).not.toBe(`Fevereiro ${ANO}`);

    fireEvent.click(screen.getByRole("button", { name: "Hoje" }));
    expect(mesEmFoco()).toBe(`Fevereiro ${ANO}`);
  });

  it("sem data atual, 'Hoje' fica desabilitado em vez de levar a lugar nenhum", () => {
    comDados({ currentDateParts: null });
    render(<CalendarTabRedesign activeTab="calendar" />);
    expect(screen.getByRole("button", { name: "Hoje" })).toBeDisabled();
  });
});

describe("CalendarTabRedesign — próximas etapas", () => {
  it("mostra só cinco etapas até alguém pedir o resto", () => {
    const { container } = render(<CalendarTabRedesign activeTab="calendar" />);
    const lista = container.querySelector(".flex.flex-col.gap-2\\.5");
    expect(within(lista).getAllByText(/Pista R/)).toHaveLength(5);
  });

  it("'Ver todos' EXPANDE a lista — não é o botão 'Hoje' disfarçado", () => {
    const { container } = render(<CalendarTabRedesign activeTab="calendar" />);
    const mesAntes = mesEmFoco();

    fireEvent.click(screen.getByRole("button", { name: "Ver todos" }));

    const lista = container.querySelector(".flex.flex-col.gap-2\\.5");
    expect(within(lista).getAllByText(/Pista R/)).toHaveLength(PROXIMAS.length);
    // A regressão de origem: o clique mexia no mês da grade e deixava a lista igual.
    expect(mesEmFoco()).toBe(mesAntes);
  });

  it("'Ver mais' e 'Ver menos' são o mesmo interruptor", () => {
    const { container } = render(<CalendarTabRedesign activeTab="calendar" />);
    const lista = () => container.querySelector(".flex.flex-col.gap-2\\.5");

    fireEvent.click(screen.getByRole("button", { name: /Ver mais/ }));
    expect(within(lista()).getAllByText(/Pista R/)).toHaveLength(PROXIMAS.length);

    fireEvent.click(screen.getByRole("button", { name: /Ver menos/ }));
    expect(within(lista()).getAllByText(/Pista R/)).toHaveLength(5);
  });

  it("com cinco etapas ou menos, não oferece expandir", () => {
    comDados({ upcoming: PROXIMAS.slice(0, 4) });
    render(<CalendarTabRedesign activeTab="calendar" />);
    expect(screen.queryByRole("button", { name: "Ver todos" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Ver mais/ })).not.toBeInTheDocument();
  });

  it("sem etapas à frente, avisa em vez de mostrar uma lista vazia", () => {
    comDados({ upcoming: [] });
    render(<CalendarTabRedesign activeTab="calendar" />);
    expect(screen.getByText("Nenhuma corrida à frente.")).toBeInTheDocument();
  });

  it("clicar numa etapa leva a grade até o mês dela", () => {
    render(<CalendarTabRedesign activeTab="calendar" />);
    // R3 corre em abril; a grade está em fevereiro.
    fireEvent.click(screen.getByText("Pista R3"));
    expect(mesEmFoco()).toBe(`Abril ${ANO}`);
  });
});

describe("CalendarTabRedesign — resumo e estados de carga", () => {
  it("o resumo mostra as etapas concluídas sobre o total", () => {
    render(<CalendarTabRedesign activeTab="calendar" />);
    expect(screen.getByText("2/7")).toBeInTheDocument();
  });

  it("a contagem regressiva vira 'Hoje' no dia da corrida", () => {
    comDados({ temporalSummary: { days_until_next_event: 0 } });
    render(<CalendarTabRedesign activeTab="calendar" />);
    // Aparece no ladrilho de resumo e no botão de navegação — os dois dizem "Hoje".
    expect(screen.getAllByText("Hoje").length).toBeGreaterThan(1);
  });

  it("sem próxima etapa, a contagem some em vez de mostrar zero", () => {
    comDados({ temporalSummary: { days_until_next_event: null } });
    render(<CalendarTabRedesign activeTab="calendar" />);
    expect(screen.getByText("—")).toBeInTheDocument();
  });

  it("erro de carga aparece na tela, no lugar da grade", () => {
    comDados({ error: "calendário indisponível", loading: false });
    render(<CalendarTabRedesign activeTab="calendar" />);
    expect(screen.getByText("calendário indisponível")).toBeInTheDocument();
    expect(screen.queryAllByTestId("celula")).toHaveLength(0);
  });

  it("carga rápida não pisca 'Carregando': só o adiado mostra texto", () => {
    comDados({ loading: true, showLoadingUI: false });
    const { unmount } = render(<CalendarTabRedesign activeTab="calendar" />);
    expect(screen.queryByText(/Carregando/i)).not.toBeInTheDocument();
    expect(screen.queryAllByTestId("celula")).toHaveLength(0);
    unmount();

    comDados({ loading: true, showLoadingUI: true });
    render(<CalendarTabRedesign activeTab="calendar" />);
    expect(screen.getByText(/Carregando/i)).toBeInTheDocument();
  });
});
