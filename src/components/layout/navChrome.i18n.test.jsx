import { describe, it, expect, afterEach } from "vitest";
import { render, screen, act, cleanup } from "@testing-library/react";
import i18n, { DEFAULT_LANGUAGE } from "../../i18n/index.js";
import TabNavigation from "./TabNavigation";
import LeaveToMenuModal from "./LeaveToMenuModal";

afterEach(() => {
  cleanup();
  i18n.changeLanguage(DEFAULT_LANGUAGE);
});

async function setLang(lang) {
  await act(async () => {
    await i18n.changeLanguage(lang);
  });
}

describe("nav chrome i18n (Fase 1 — casca de navegação)", () => {
  it("TabNavigation troca os labels PT↔EN ao vivo (useTranslation)", async () => {
    await setLang("pt-BR");
    render(<TabNavigation activeTab="standings" onTabChange={() => {}} />);
    expect(screen.getByText("Notícias")).toBeInTheDocument();
    expect(screen.getByText("Minha Equipe")).toBeInTheDocument();

    await setLang("en-US");
    expect(screen.getByText("News")).toBeInTheDocument();
    expect(screen.getByText("My Team")).toBeInTheDocument();
    expect(screen.getByText("Calendar")).toBeInTheDocument();
  });

  it("LeaveToMenuModal renderiza em EN", async () => {
    await setLang("en-US");
    render(
      <LeaveToMenuModal
        open
        isSaving={false}
        onSaveAndExit={() => {}}
        onExitWithoutSave={() => {}}
        onCancel={() => {}}
      />,
    );
    expect(screen.getByText("Exit to main menu?")).toBeInTheDocument();
    expect(screen.getByText("Save and exit")).toBeInTheDocument();
    expect(screen.getByText("Exit without saving")).toBeInTheDocument();
    expect(screen.getByText("Cancel")).toBeInTheDocument();
  });
});
