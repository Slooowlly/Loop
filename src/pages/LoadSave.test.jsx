import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

import LoadSave from "./LoadSave";

const mockInvoke = vi.fn();
const mockLoadCareer = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args) => mockInvoke(...args),
}));

const mockClearCareer = vi.fn();

vi.mock("../stores/useCareerStore", () => ({
  default: (selector) =>
    selector({
      loadCareer: mockLoadCareer,
      clearCareer: mockClearCareer,
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

      if (command === "list_backups") {
        return [
          {
            season_number: 1,
            file_name: "temporada_001.db",
            file_path: "C:/saves/save-001/backups/temporada_001.db",
            size_kb: 512,
            modified_at: "2026-04-02T12:00:00",
          },
        ];
      }

      return null;
    });

    mockLoadCareer.mockReset();
    mockClearCareer.mockReset();
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

  it("lists the career backups and only restores after an explicit confirmation", async () => {
    render(
      <MemoryRouter>
        <LoadSave />
      </MemoryRouter>,
    );

    expect(await screen.findByText("Rodrigo")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^backups$/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("list_backups", { careerId: "save-001" });
    });

    expect(await screen.findByText("Temporada 1")).toBeInTheDocument();
    expect(screen.getByText(/512 KB/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^restaurar$/i }));

    expect(screen.getByTestId("backup-confirm")).toBeInTheDocument();
    expect(screen.getByText(/todo o progresso feito depois desse ponto será perdido/i)).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("restore_backup", expect.anything());

    fireEvent.click(screen.getByRole("button", { name: /restaurar mesmo assim/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("restore_backup", {
        careerId: "save-001",
        seasonNumber: 1,
      });
    });

    // O estado em memoria precisa ser invalidado: o save no disco mudou.
    expect(mockClearCareer).toHaveBeenCalled();
    await waitFor(() => {
      expect(screen.queryByTestId("backup-confirm")).not.toBeInTheDocument();
    });
  });

  it("warns about overwriting before creating a manual backup", async () => {
    render(
      <MemoryRouter>
        <LoadSave />
      </MemoryRouter>,
    );

    expect(await screen.findByText("Rodrigo")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^backups$/i }));

    expect(await screen.findByText("Temporada 1")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /criar backup agora/i }));

    expect(screen.getByText(/ele será sobrescrito/i)).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("create_season_backup", expect.anything());

    fireEvent.click(screen.getByRole("button", { name: /^criar backup$/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("create_season_backup", {
        careerId: "save-001",
        seasonNumber: 1,
      });
    });
  });
});
