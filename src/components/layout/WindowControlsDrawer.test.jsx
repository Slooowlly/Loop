import { act, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

import WindowControlsDrawer from "./WindowControlsDrawer";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(true),
}));

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) =>
    selector({
      clearCareer: vi.fn(),
      isDirty: false,
      isLoaded: false,
      flushSave: vi.fn(),
    }),
}));

describe("WindowControlsDrawer", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it("renders a square hover target around the drawer chevron", async () => {
    render(
      <MemoryRouter initialEntries={["/dashboard"]}>
        <WindowControlsDrawer />
      </MemoryRouter>,
    );

    const hoverTarget = screen.getByTestId("window-controls-hover-target");
    expect(hoverTarget).toHaveClass("h-10");
    expect(hoverTarget).toHaveClass("w-10");
  });

  it("is only window controls now (no Home shortcut) and confirms before closing", async () => {
    render(
      <MemoryRouter initialEntries={["/dashboard"]}>
        <WindowControlsDrawer />
      </MemoryRouter>,
    );

    const hoverTarget = screen.getByTestId("window-controls-hover-target");
    fireEvent.mouseEnter(hoverTarget);

    await act(async () => {
      vi.advanceTimersByTime(500);
    });

    // O "voltar ao menu" saiu do drawer (agora é menu de pausa / menu da equipe).
    expect(screen.queryByRole("button", { name: /home/i })).not.toBeInTheDocument();

    // Fechar o app ainda pede confirmação (com opção de salvar).
    fireEvent.click(screen.getByRole("button", { name: /fechar app/i }));
    expect(screen.getByText(/fechar o loop/i)).toBeInTheDocument();
    expect(screen.getByText(/você pode salvar o progresso antes de fechar o jogo/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /cancelar/i }));
    expect(screen.queryByText(/fechar o loop/i)).not.toBeInTheDocument();
  });

  it("disables the hover hotspot while the drawer is open so the close button stays clickable", async () => {
    render(
      <MemoryRouter initialEntries={["/dashboard"]}>
        <WindowControlsDrawer />
      </MemoryRouter>,
    );

    const hoverTarget = screen.getByTestId("window-controls-hover-target");
    fireEvent.mouseEnter(hoverTarget);

    await act(async () => {
      vi.advanceTimersByTime(500);
    });

    expect(hoverTarget.className).toContain("pointer-events-none");
  });
});
