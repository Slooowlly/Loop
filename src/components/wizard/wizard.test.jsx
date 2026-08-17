import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import CategoryCard from "./CategoryCard";
import StepIndicator from "./StepIndicator";
import TeamCard from "./TeamCard";
import { STARTING_CATEGORIES, WIZARD_STEPS } from "../../utils/constants";

// Os cartões do funil de criação de carreira — a primeira coisa que todo jogador novo
// atravessa, e a única tela do jogo que ele não pode pular. Estavam sem nenhum teste.
// (O cartão de dificuldade saiu do funil em 16/08/2026, junto com o passo inteiro.)
//
// O que se guarda aqui é o CONTRATO de seleção: qual valor cada cartão devolve ao ser
// clicado e como ele mostra que está escolhido. Os dois erram de formas diferentes e
// silenciosas — o cartão de equipe aceita duas formas de DTO, o de categoria devolve `id`
// e não o objeto, e a marca visual de "selecionado" é só uma classe.

/// A marca de seleção do GlassCard: borda de destaque + brilho.
function estaSelecionado(elemento) {
  const cartao = elemento.closest("div.rounded-3xl");
  return cartao?.className.includes("border-accent-primary") ?? false;
}

describe("CategoryCard", () => {
  const categoria = STARTING_CATEGORIES[0];

  it("devolve o ID da categoria — é o que a lista de equipes filtra", () => {
    const onSelect = vi.fn();
    render(<CategoryCard category={categoria} selected={false} onSelect={onSelect} />);

    fireEvent.click(screen.getByText(categoria.name));
    expect(onSelect).toHaveBeenCalledWith(categoria.id);
  });

  it("mostra nome e carro — é por eles que o jogador escolhe", () => {
    render(<CategoryCard category={categoria} selected={false} onSelect={vi.fn()} />);
    expect(screen.getByText(categoria.name)).toBeInTheDocument();
    expect(screen.getByText(categoria.car)).toBeInTheDocument();
  });

  it("não quebra quando a categoria não tem arte", () => {
    const semLogo = { ...categoria, logo: null };
    expect(() =>
      render(<CategoryCard category={semLogo} selected={false} onSelect={vi.fn()} />),
    ).not.toThrow();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("marca visualmente só o cartão escolhido", () => {
    const { rerender } = render(
      <CategoryCard category={categoria} selected={false} onSelect={vi.fn()} />,
    );
    expect(estaSelecionado(screen.getByText(categoria.name))).toBe(false);

    rerender(<CategoryCard category={categoria} selected onSelect={vi.fn()} />);
    expect(estaSelecionado(screen.getByText(categoria.name))).toBe(true);
  });
});

describe("TeamCard", () => {
  // O cartão aceita DUAS formas: a do wizard (draft do backend, em português) e a
  // camelCase de outras telas. Aceitar as duas caladamente é o que torna o teste
  // necessário — um DTO renomeado aparece como card em branco, não como erro.
  const doDraft = { id: "T1", nome: "Escuderia Alfa", cor_primaria: "#123456" };
  const camelCase = { id: "T2", name: "Beta Racing", primaryColor: "#abcdef", country: "BR" };

  it("lê o nome nas duas formas de DTO", () => {
    const { unmount } = render(<TeamCard team={doDraft} selected={false} onSelect={vi.fn()} />);
    expect(screen.getByText("Escuderia Alfa")).toBeInTheDocument();
    unmount();

    render(<TeamCard team={camelCase} selected={false} onSelect={vi.fn()} />);
    expect(screen.getByText("Beta Racing")).toBeInTheDocument();
  });

  it("devolve o id da equipe — é ele que vai para `finalize_career_draft`", () => {
    const onSelect = vi.fn();
    render(<TeamCard team={doDraft} selected={false} onSelect={onSelect} />);

    fireEvent.click(screen.getByText("Escuderia Alfa"));
    expect(onSelect).toHaveBeenCalledWith("T1");
  });

  it("cai no índice quando a equipe não tem id", () => {
    const onSelect = vi.fn();
    render(<TeamCard team={{ nome: "Sem Id", index: 3 }} selected={false} onSelect={onSelect} />);

    fireEvent.click(screen.getByText("Sem Id"));
    expect(onSelect).toHaveBeenCalledWith(3);
  });

  it("esconde o desempenho atrás de hachura — é mistério de propósito", () => {
    render(<TeamCard team={doDraft} selected={false} onSelect={vi.fn()} />);
    // Se um dia a barra virar valor real, este teste cai e a decisão volta à mesa.
    expect(screen.getByText("???")).toBeInTheDocument();
  });

  it("só mostra o selo de país quando a equipe tem um", () => {
    const { unmount } = render(<TeamCard team={doDraft} selected={false} onSelect={vi.fn()} />);
    expect(screen.queryByText("BR")).not.toBeInTheDocument();
    unmount();

    render(<TeamCard team={camelCase} selected={false} onSelect={vi.fn()} />);
    expect(screen.getByText("BR")).toBeInTheDocument();
  });

  it("marca visualmente só o cartão escolhido", () => {
    const { rerender } = render(<TeamCard team={doDraft} selected={false} onSelect={vi.fn()} />);
    expect(estaSelecionado(screen.getByText("Escuderia Alfa"))).toBe(false);

    rerender(<TeamCard team={doDraft} selected onSelect={vi.fn()} />);
    expect(estaSelecionado(screen.getByText("Escuderia Alfa"))).toBe(true);
  });
});

describe("StepIndicator", () => {
  const passos = WIZARD_STEPS.map((slug, i) => `Passo ${i + 1} (${slug})`);

  it("desenha uma coluna por passo do funil", () => {
    const { container } = render(<StepIndicator currentStep={1} steps={passos} />);
    const grade = container.querySelector("[style*='grid-template-columns']");
    expect(grade.style.gridTemplateColumns).toBe(`repeat(${passos.length}, minmax(0, 1fr))`);
    passos.forEach((passo) => expect(screen.getByText(passo)).toBeInTheDocument());
  });

  it("preenche por completo os passos já vencidos e parcialmente o atual", () => {
    const { container } = render(<StepIndicator currentStep={3} steps={passos} />);
    const larguras = [...container.querySelectorAll("[style*='width']")].map(
      (barra) => barra.style.width,
    );

    // Passos 1 e 2 vencidos, 3 em curso, 4 em diante intocados.
    expect(larguras.slice(0, 2)).toEqual(["100%", "100%"]);
    expect(larguras[2]).toBe("72%");
    expect(larguras.slice(3).every((w) => w === "24%")).toBe(true);
  });

  it("no primeiro passo, nada aparece como vencido", () => {
    const { container } = render(<StepIndicator currentStep={1} steps={passos} />);
    const larguras = [...container.querySelectorAll("[style*='width']")].map((b) => b.style.width);
    expect(larguras).not.toContain("100%");
  });

  it("no último passo, todos os anteriores aparecem vencidos", () => {
    const { container } = render(
      <StepIndicator currentStep={passos.length} steps={passos} />,
    );
    const larguras = [...container.querySelectorAll("[style*='width']")].map((b) => b.style.width);
    expect(larguras.slice(0, passos.length - 1).every((w) => w === "100%")).toBe(true);
    expect(larguras.at(-1)).toBe("72%");
  });

  it("mostra o progresso em números, e o total acompanha a lista recebida", () => {
    const curto = ["Um", "Dois", "Três"];
    const { container } = render(<StepIndicator currentStep={2} steps={curto} />);
    const cabecalho = container.firstChild;
    expect(within(cabecalho).getByText(/2/)).toBeInTheDocument();
    expect(cabecalho.textContent).toContain("3");
  });
});
