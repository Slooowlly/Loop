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

  it("keeps only the Home shortcut and always asks for confirmation on Home and close", async () => {
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

    expect(screen.getByRole("button", { name: /home/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /configuracoes/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /carregar save/i })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /home/i }));
    expect(screen.getByText(/deseja sair da carreira agora/i)).toBeInTheDocument();
    expect(
      screen.getByText(/você pode salvar antes de voltar ao menu principal ou fechar o jogo/i),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /cancelar/i }));
    expect(screen.queryByText(/deseja sair da carreira agora/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /fechar app/i }));
    expect(screen.getByText(/deseja sair da carreira agora/i)).toBeInTheDocument();
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
