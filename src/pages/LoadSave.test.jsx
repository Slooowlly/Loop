import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

import LoadSave from "./LoadSave";

const mockInvoke = vi.fn();
const mockLoadCareer = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args) => mockInvoke(...args),
}));

vi.mock("../stores/useCareerStore", () => ({
  default: (selector) =>
    selector({
      loadCareer: mockLoadCareer,
    }),
}));

describe("LoadSave", () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (command) => {
      if (command === "list_saves") {
        return [
          {
            career_id: "save-001",
            player_name: "Rodrigo",
            category_name: "Stock Car",
            season: 1,
            year: 2026,
            difficulty: "medio",
            last_played: "2026-04-02T12:00:00Z",
            created: "2026-04-01T12:00:00Z",
            total_races: 12,
          },
        ];
      }

      if (command === "delete_career") {
        return null;
      }

      return null;
    });

    mockLoadCareer.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("asks for confirmation in the app before deleting a save", async () => {
    render(
      <MemoryRouter>
        <LoadSave />
      </MemoryRouter>,
    );

    expect(await screen.findByText("Rodrigo")).toBeInTheDocument();
    expect(screen.getByText("Ano 2026")).toBeInTheDocument();
    expect(screen.queryByText(/temporada 1/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /deletar/i }));

    expect(screen.getByText(/tem certeza que deseja deletar este save/i)).toBeInTheDocument();
    expect(screen.getByText(/essa ação não pode ser desfeita/i)).toBeInTheDocument();
    expect(screen.getByTestId("delete-save-actions")).toHaveClass("justify-center");
    expect(mockInvoke).not.toHaveBeenCalledWith("delete_career", { careerId: "save-001" });

    fireEvent.click(screen.getByRole("button", { name: /cancelar/i }));

    expect(
      screen.queryByText(/tem certeza que deseja deletar este save/i),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^deletar$/i }));
    fireEvent.click(screen.getByRole("button", { name: /confirmar exclusão/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("delete_career", { careerId: "save-001" });
    });
  });
});
