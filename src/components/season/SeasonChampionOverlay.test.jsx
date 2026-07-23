import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import PauseMenu from "../layout/PauseMenu";
import useCareerStore from "../../stores/useCareerStore";
import SeasonChampionOverlay from "./SeasonChampionOverlay";

const OVERLAY_STATE = { demo: true };

function renderOverlay() {
  return render(<SeasonChampionOverlay />);
}

function expectClosed(container) {
  expect(container.querySelector(".champ-ov")).not.toBeInTheDocument();
  expect(useCareerStore.getState().championOverlay).toBe(null);
}

describe("SeasonChampionOverlay", () => {
  beforeEach(() => {
    useCareerStore.setState({
      championOverlay: { ...OVERLAY_STATE },
    });
  });

  afterEach(() => {
    cleanup();
    useCareerStore.setState({
      championOverlay: null,
    });
  });

  it("fecha em Continuar", () => {
    const { container } = renderOverlay();

    fireEvent.click(screen.getByRole("button", { name: /Continuar/i }));

    expectClosed(container);
  });

  it("fecha no botão Fechar", () => {
    const { container } = renderOverlay();

    fireEvent.click(screen.getByRole("button", { name: /Fechar/i }));

    expectClosed(container);
  });

  it("fecha ao clicar no backdrop", () => {
    const { container } = renderOverlay();

    fireEvent.click(container.querySelector(".champ-ov"));

    expectClosed(container);
  });

  it("captura Escape e cancela sua propagação", () => {
    const { container } = renderOverlay();
    const bubbleListener = vi.fn();
    const escapeEvent = new KeyboardEvent("keydown", {
      key: "Escape",
      bubbles: true,
      cancelable: true,
    });
    window.addEventListener("keydown", bubbleListener);

    try {
      act(() => {
        window.dispatchEvent(escapeEvent);
      });

      expect(escapeEvent.defaultPrevented).toBe(true);
      expect(bubbleListener).not.toHaveBeenCalled();
      expectClosed(container);
    } finally {
      window.removeEventListener("keydown", bubbleListener);
    }
  });

  it("dá precedência ao overlay sobre o menu de pausa ao pressionar Escape", () => {
    const { container } = render(
      <MemoryRouter>
        <PauseMenu />
        <SeasonChampionOverlay />
      </MemoryRouter>,
    );

    fireEvent.keyDown(window, { key: "Escape", bubbles: true, cancelable: true });

    expect(container.querySelector(".champ-ov")).not.toBeInTheDocument();
    expect(screen.queryByText("Pausa")).not.toBeInTheDocument();
    expect(container.querySelector(".glass-strong")).not.toBeInTheDocument();
    expect(useCareerStore.getState().championOverlay).toBe(null);
  });
});
