import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import Tooltip from "./Tooltip";

// A janela quente e o atraso vivem em estado de módulo. Cada caso começa num
// instante mais adiante do que o anterior para que a janela deixada por um teste
// já esteja no passado quando o próximo abrir o primeiro balão.
let instante = new Date(2026, 0, 1).getTime();

beforeEach(() => {
  vi.useFakeTimers();
  instante += 60_000;
  vi.setSystemTime(instante);
});

afterEach(() => {
  vi.useRealTimers();
});

function avancar(ms) {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

describe("Tooltip", () => {
  it("espera antes de abrir o primeiro balão", () => {
    render(
      <Tooltip texto="Pontos da temporada">
        <button type="button">42</button>
      </Tooltip>,
    );

    fireEvent.mouseEnter(screen.getByRole("button"));
    avancar(200);
    expect(screen.queryByTestId("tooltip")).toBeNull();

    avancar(300);
    expect(screen.getByTestId("tooltip")).toHaveTextContent("Pontos da temporada");
  });

  // O balão substitui o `title=` em tabelas e grids onde um nó a mais muda o
  // layout: o filho tem que sair do componente com o MESMO pai que entrou.
  it("não embrulha o filho num elemento novo", () => {
    const { container } = render(
      <Tooltip texto="Melhor volta">
        <span className="badge">1:32.004</span>
      </Tooltip>,
    );

    expect(container.firstChild).toBe(screen.getByText("1:32.004"));
    expect(container.firstChild.className).toBe("badge");
  });

  // O alvo LARGO — uma linha de tabela — não pode receber o balão por cima de si
  // mesmo. Os lados horizontais existem para isso, e obedecem ao espaço real: o
  // lado pedido só é abandonado quando não cabe.
  describe("lados horizontais", () => {
    function comAlvoEm(left) {
      render(
        <Tooltip texto="Modificadores" lado="esquerda">
          <button type="button">linha</button>
        </Tooltip>,
      );
      const alvo = screen.getByRole("button");
      alvo.getBoundingClientRect = () => ({
        top: 300,
        bottom: 340,
        left,
        right: left + 400,
        width: 400,
        height: 40,
      });
      return alvo;
    }

    it("pousa ao lado do alvo, e não em cima dele", () => {
      fireEvent.mouseEnter(comAlvoEm(500));
      avancar(400);

      expect(screen.getByTestId("tooltip")).toHaveAttribute("data-lado", "esquerda");
    });

    it("vira para o outro lado quando não cabe no pedido", () => {
      fireEvent.mouseEnter(comAlvoEm(0));
      avancar(400);

      expect(screen.getByTestId("tooltip")).toHaveAttribute("data-lado", "direita");
    });
  });

  // A alça que substituiu o `title=` para quem consulta sem hover.
  it("deixa o texto legível no próprio gatilho", () => {
    render(
      <Tooltip texto="Campeão vigente">
        <span>★</span>
      </Tooltip>,
    );

    expect(screen.getByText("★")).toHaveAttribute("data-tooltip", "Campeão vigente");
  });

  it("não serializa conteúdo montado no gatilho", () => {
    render(
      <Tooltip conteudo={<strong>rico</strong>}>
        <span>alvo</span>
      </Tooltip>,
    );

    expect(screen.getByText("alvo")).not.toHaveAttribute("data-tooltip");
  });

  it("liga o alvo ao balão enquanto ele está na tela", () => {
    render(
      <Tooltip texto="Piloto lesionado">
        <button type="button">GS</button>
      </Tooltip>,
    );

    const alvo = screen.getByRole("button");
    expect(alvo).not.toHaveAttribute("aria-describedby");

    fireEvent.mouseEnter(alvo);
    avancar(400);
    expect(alvo.getAttribute("aria-describedby")).toBe(screen.getByTestId("tooltip").id);

    fireEvent.mouseLeave(alvo);
    expect(screen.queryByTestId("tooltip")).toBeNull();
    expect(alvo).not.toHaveAttribute("aria-describedby");
  });

  // Varrer uma coluna de badges tem que ser leitura contínua, e não uma espera
  // por célula.
  it("abre na hora o balão seguinte dentro da janela quente", () => {
    render(
      <>
        <Tooltip texto="Primeiro">
          <button type="button">um</button>
        </Tooltip>
        <Tooltip texto="Segundo">
          <button type="button">dois</button>
        </Tooltip>
      </>,
    );

    fireEvent.mouseEnter(screen.getByText("um"));
    avancar(400);
    expect(screen.getByTestId("tooltip")).toHaveTextContent("Primeiro");

    fireEvent.mouseLeave(screen.getByText("um"));
    fireEvent.mouseEnter(screen.getByText("dois"));
    expect(screen.getByTestId("tooltip")).toHaveTextContent("Segundo");
  });

  it("deixa um balão por vez na tela", () => {
    render(
      <>
        <Tooltip texto="Primeiro">
          <button type="button">um</button>
        </Tooltip>
        <Tooltip texto="Segundo">
          <button type="button">dois</button>
        </Tooltip>
      </>,
    );

    fireEvent.mouseEnter(screen.getByText("um"));
    avancar(400);
    // Sem passar pelo `mouseleave`: é o caso de sair de um alvo direto para o
    // vizinho colado nele.
    fireEvent.mouseEnter(screen.getByText("dois"));
    avancar(400);

    expect(screen.getAllByTestId("tooltip")).toHaveLength(1);
  });

  it("fecha no Esc para quem abriu pelo teclado", () => {
    render(
      <Tooltip texto="Renovação pendente">
        <button type="button">contrato</button>
      </Tooltip>,
    );

    fireEvent.focus(screen.getByRole("button"));
    expect(screen.getByTestId("tooltip")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByTestId("tooltip")).toBeNull();
  });

  it("preserva os manipuladores do próprio filho", () => {
    const aoEntrar = vi.fn();
    const aoClicar = vi.fn();

    render(
      <Tooltip texto="Abrir dossiê">
        <button type="button" onMouseEnter={aoEntrar} onMouseDown={aoClicar}>
          equipe
        </button>
      </Tooltip>,
    );

    const alvo = screen.getByRole("button");
    fireEvent.mouseEnter(alvo);
    avancar(400);
    fireEvent.mouseDown(alvo);

    expect(aoEntrar).toHaveBeenCalledTimes(1);
    expect(aoClicar).toHaveBeenCalledTimes(1);
    // O clique já entregou o que o balão diria.
    expect(screen.queryByTestId("tooltip")).toBeNull();
  });

  // O caso que motivou `soSeCortado`: no `title=` nativo, um nome que cabia
  // inteiro abria um balão dizendo o nome que já estava na tela.
  describe("soSeCortado", () => {
    function medindo(no, { scrollWidth, clientWidth }) {
      Object.defineProperty(no, "scrollWidth", { value: scrollWidth, configurable: true });
      Object.defineProperty(no, "clientWidth", { value: clientWidth, configurable: true });
    }

    it("fica calado quando o texto cabe", () => {
      render(
        <Tooltip texto="Ayrton Senna" soSeCortado>
          <span className="truncate">Ayrton Senna</span>
        </Tooltip>,
      );

      const alvo = screen.getByText("Ayrton Senna");
      medindo(alvo, { scrollWidth: 120, clientWidth: 120 });

      fireEvent.mouseEnter(alvo);
      avancar(400);
      expect(screen.queryByTestId("tooltip")).toBeNull();
    });

    it("abre quando o texto está cortado", () => {
      render(
        <Tooltip texto="Ayrton Senna da Silva" soSeCortado>
          <span className="truncate">Ayrton Senna da Silva</span>
        </Tooltip>,
      );

      const alvo = screen.getByText("Ayrton Senna da Silva");
      medindo(alvo, { scrollWidth: 240, clientWidth: 120 });

      fireEvent.mouseEnter(alvo);
      avancar(400);
      expect(screen.getByTestId("tooltip")).toHaveTextContent("Ayrton Senna da Silva");
    });

    // Um pixel de diferença é arredondamento subpixel, não texto escondido.
    it("trata folga de 1px como texto que coube", () => {
      render(
        <Tooltip texto="Justo" soSeCortado>
          <span className="truncate">Justo</span>
        </Tooltip>,
      );

      const alvo = screen.getByText("Justo");
      medindo(alvo, { scrollWidth: 121, clientWidth: 120 });

      fireEvent.mouseEnter(alvo);
      avancar(400);
      expect(screen.queryByTestId("tooltip")).toBeNull();
    });

    it("mede de novo a cada hover, não só na montagem", () => {
      render(
        <Tooltip texto="Nome" soSeCortado>
          <span className="truncate">Nome</span>
        </Tooltip>,
      );

      const alvo = screen.getByText("Nome");
      medindo(alvo, { scrollWidth: 100, clientWidth: 100 });
      fireEvent.mouseEnter(alvo);
      avancar(400);
      expect(screen.queryByTestId("tooltip")).toBeNull();
      fireEvent.mouseLeave(alvo);

      // A coluna encolheu — o mesmo nome agora está cortado.
      medindo(alvo, { scrollWidth: 100, clientWidth: 40 });
      fireEvent.mouseEnter(alvo);
      avancar(400);
      expect(screen.getByTestId("tooltip")).toBeInTheDocument();
    });
  });

  it("sai da frente quando não há texto", () => {
    render(
      <Tooltip texto={undefined}>
        <button type="button">sem detalhe</button>
      </Tooltip>,
    );

    fireEvent.mouseEnter(screen.getByRole("button"));
    avancar(400);
    expect(screen.queryByTestId("tooltip")).toBeNull();
  });
});
