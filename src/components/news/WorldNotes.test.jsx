import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import WorldNotes from "./WorldNotes";

// A diagramação de breves numera as notas de forma contínua e desce por coluna
// (01-03 na página esquerda, 04-06 na direita). Como a divisão é feita no JSX,
// e não pelo fluxo do grid, ela é a única lógica do componente que pode quebrar.
function notas(n) {
  return Array.from({ length: n }, (_, i) => ({
    id: i + 1,
    tag: "RECORDE",
    tone: "recorde",
    text: `Nota ${i + 1}`,
  }));
}

describe("WorldNotes", () => {
  it("não desenha nada sem nota", () => {
    const { container } = render(<WorldNotes notes={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("divide em duas colunas e numera de forma contínua", () => {
    const { container } = render(<WorldNotes notes={notas(6)} />);
    const colunas = container.querySelectorAll(".wn-col");
    expect(colunas).toHaveLength(2);
    expect(colunas[0].querySelectorAll(".wn-item")).toHaveLength(3);
    expect(colunas[1].querySelectorAll(".wn-item")).toHaveLength(3);
    const numeros = [...container.querySelectorAll(".wn-num")].map(
      (e) => e.textContent,
    );
    expect(numeros).toEqual(["01", "02", "03", "04", "05", "06"]);
  });

  it("com número ímpar de notas a página esquerda leva a sobra", () => {
    const { container } = render(<WorldNotes notes={notas(5)} />);
    const colunas = container.querySelectorAll(".wn-col");
    expect(colunas[0].querySelectorAll(".wn-item")).toHaveLength(3);
    expect(colunas[1].querySelectorAll(".wn-item")).toHaveLength(2);
    expect(
      [...container.querySelectorAll(".wn-num")].map((e) => e.textContent),
    ).toEqual(["01", "02", "03", "04", "05"]);
  });

  it("com uma nota só o bloco encolhe para a página esquerda", () => {
    const { container } = render(<WorldNotes notes={notas(1)} />);
    expect(container.querySelectorAll(".wn-col")).toHaveLength(1);
    // Sem a marca de solo o fio do cabeçalho atravessa o vão inteiro com uma
    // nota só embaixo, e a página direita nasce vazia.
    expect(container.querySelector(".world-notes")).toHaveClass(
      "world-notes--solo",
    );
    expect(screen.getByText("Nota 1")).toBeInTheDocument();
  });

  it("com duas notas cada página leva a sua, sem virar nota única", () => {
    const { container } = render(<WorldNotes notes={notas(2)} />);
    expect(container.querySelectorAll(".wn-col")).toHaveLength(2);
    expect(container.querySelector(".world-notes")).not.toHaveClass(
      "world-notes--solo",
    );
  });

  it("o cabeçalho tem uma peça por folha ocupada", () => {
    // Um cabeçalho só, atravessando o vão, soldaria as duas folhas numa página
    // única. Com nota única a folha direita não recebe peça nenhuma.
    const { container: cheio } = render(<WorldNotes notes={notas(4)} />);
    expect(cheio.querySelectorAll(".wn-head-pg")).toHaveLength(2);

    const { container: um } = render(<WorldNotes notes={notas(1)} />);
    expect(um.querySelectorAll(".wn-head-pg")).toHaveLength(1);
    // A contagem acompanha a peça que existe, senão some da tela.
    expect(um.querySelectorAll(".wn-head-count")).toHaveLength(1);
  });
});
