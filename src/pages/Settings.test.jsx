import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

import Settings from "./Settings";

const mockInvoke = vi.fn();
const mockNavigate = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args) => mockInvoke(...args),
}));

vi.mock("react-router-dom", async (importOriginal) => ({
  ...(await importOriginal()),
  useNavigate: () => mockNavigate,
}));

vi.mock("../components/ui/ParticleBackdrop", () => ({
  default: () => <div data-testid="particle-backdrop" />,
}));

vi.mock("../components/iracing/RivalryPerceptionPanel", () => ({
  default: () => <div>Rivalidades percebidas (debug)</div>,
}));

const configFixture = {
  language: "pt-BR",
  autosave_enabled: true,
  version: "1.0.0",
};

const yellowStatusFixture = {
  app_ini_found: true,
  installed: true,
  app_ini_path: "C:\\iRacing\\app.ini",
  slot: 3,
  original: "You're welcome",
  current_value: "!y$",
};

const idleCaptureFixture = {
  active: false,
  frames: 0,
  dir: "C:\\Loop\\debug\\race_captures",
};

const activeCaptureFixture = {
  active: true,
  frames: 42,
  dir: "C:\\Loop\\debug\\race_captures",
};

const debugGroupLabels = [
  "Detalhes técnicos",
  "Comando de chat (teste)",
  "Quebra ao vivo (teste)",
  "Testar overlay de rádio",
  "Gravar corrida (debug)",
  "Rivalidades percebidas (debug)",
];

const readOnlyCommands = new Set([
  "get_config",
  "iracing_yellow_macro_status",
  "iracing_auto_yellow_enabled",
  "overlay_demo_enabled",
  "race_capture_status",
  "iracing_spotter_status",
  "list_saves",
]);

let radioDemoEnabled;
let captureFixture;
let spotterStatusFixture;
let savesFixture;

function renderSettings() {
  return render(
    <MemoryRouter initialEntries={["/settings"]}>
      <Settings />
    </MemoryRouter>,
  );
}

async function waitForSettings() {
  await screen.findByText("Idioma");
}

function expectDebugGroupsHidden() {
  debugGroupLabels.forEach((label) => {
    expect(screen.queryByText(label)).not.toBeInTheDocument();
  });
}

function expectDebugGroupsVisible() {
  debugGroupLabels.forEach((label) => {
    expect(screen.getByText(label)).toBeInTheDocument();
  });
}

function expectRegularSettingsVisible() {
  expect(screen.getByText("Idioma")).toBeInTheDocument();
  expect(screen.getByText("Salvamento automático")).toBeInTheDocument();
  expect(screen.getByText("Bandeira amarela automática")).toBeInTheDocument();
}

describe("Settings debug menu", () => {
  beforeEach(() => {
    radioDemoEnabled = false;
    captureFixture = idleCaptureFixture;
    spotterStatusFixture = { app_ini_found: true, enabled: false };
    savesFixture = [{ career_id: "C1", player_name: "Ana" }];

    mockNavigate.mockReset();
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(async (command) => {
      if (!readOnlyCommands.has(command)) {
        throw new Error(`Unexpected mutable command: ${command}`);
      }
      if (command === "get_config") return { ...configFixture };
      if (command === "iracing_yellow_macro_status") return { ...yellowStatusFixture };
      if (command === "iracing_auto_yellow_enabled") return true;
      if (command === "overlay_demo_enabled") return radioDemoEnabled;
      if (command === "race_capture_status") return { ...captureFixture };
      if (command === "iracing_spotter_status") return { ...spotterStatusFixture };
      if (command === "list_saves") return savesFixture;
      return null;
    });
  });

  it("keeps regular settings visible and resets the closed debug menu on remount", async () => {
    const view = renderSettings();
    await waitForSettings();

    expectRegularSettingsVisible();

    const debugToggle = screen.getByRole("switch", { name: "Menu Debug" });
    expect(debugToggle).not.toBeChecked();
    expectDebugGroupsHidden();

    fireEvent.click(debugToggle);

    expect(debugToggle).toBeChecked();
    expectDebugGroupsVisible();
    expectRegularSettingsVisible();

    fireEvent.click(debugToggle);

    expect(debugToggle).not.toBeChecked();
    expectDebugGroupsHidden();
    expectRegularSettingsVisible();

    fireEvent.click(debugToggle);
    expect(debugToggle).toBeChecked();

    view.unmount();
    renderSettings();
    await waitForSettings();

    expect(screen.getByRole("switch", { name: "Menu Debug" })).not.toBeChecked();
    expectDebugGroupsHidden();
  });

  it("preserves active debug controls across close and reopen without triggering their actions", async () => {
    radioDemoEnabled = true;
    captureFixture = activeCaptureFixture;

    renderSettings();
    await waitForSettings();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("overlay_demo_enabled");
      expect(mockInvoke).toHaveBeenCalledWith("race_capture_status");
    });

    const hydrationCallCount = mockInvoke.mock.calls.length;

    const debugToggle = screen.getByRole("switch", { name: "Menu Debug" });
    fireEvent.click(debugToggle);

    const radioDemoToggle = await screen.findByRole("switch", {
      name: /Testar overlay de rádio/i,
    });
    await waitFor(() => expect(radioDemoToggle).toBeChecked());
    expect(await screen.findByRole("button", { name: "Parar (42 frames)" })).toBeInTheDocument();

    fireEvent.click(debugToggle);

    expectDebugGroupsHidden();

    fireEvent.click(debugToggle);

    const reopenedRadioDemoToggle = await screen.findByRole("switch", {
      name: /Testar overlay de rádio/i,
    });
    await waitFor(() => expect(reopenedRadioDemoToggle).toBeChecked());
    expect(await screen.findByRole("button", { name: "Parar (42 frames)" })).toBeInTheDocument();

    const invokedCommands = mockInvoke.mock.calls.map(([command]) => command);
    const additionalCommands = invokedCommands.slice(hydrationCallCount);
    expect(additionalCommands.filter((command) => !readOnlyCommands.has(command))).toEqual([]);
    expect(invokedCommands).not.toContain("overlay_set_demo");
    expect(invokedCommands).not.toContain("race_capture_stop");
  });
});
