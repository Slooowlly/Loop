import { BrowserRouter, Routes, Route } from "react-router-dom";
import { useTranslation } from "react-i18next";
import MainMenu from "./pages/MainMenu";
import NewCareer from "./pages/NewCareer";
import LoadSave from "./pages/LoadSave";
import Settings from "./pages/Settings";
import Dashboard from "./pages/Dashboard";
import WindowControlsDrawer from "./components/layout/WindowControlsDrawer";
import OverlayVrWriter from "./overlay/OverlayVrWriter";
import EngineerVrWriter from "./overlay/EngineerVrWriter";
import OverlayPositionPanel from "./overlay/OverlayPositionPanel";
import OverlayMonitorAuto from "./overlay/OverlayMonitorAuto";
import PoachAuctionHost from "./components/season/PoachAuctionHost";
import UpdaterProvider from "./components/system/UpdaterProvider";
import UpdateGate from "./components/system/UpdateGate";
import UpdateChangelogModal from "./components/system/UpdateChangelogModal";

function App() {
  // Assina as trocas de idioma: quando o locale muda (Settings), a árvore inteira
  // re-renderiza, então os formatters que leem i18n.t() atualizam ao vivo mesmo em
  // componentes que não usam useTranslation diretamente.
  useTranslation();

  return (
    <BrowserRouter>
      <UpdaterProvider>
        <WindowControlsDrawer />
        <OverlayVrWriter />
        <EngineerVrWriter />
        <OverlayPositionPanel />
        <OverlayMonitorAuto />
        <PoachAuctionHost />
        <UpdateGate />
        <UpdateChangelogModal />
        <Routes>
          <Route path="/" element={<MainMenu intro />} />
          <Route path="/menu" element={<MainMenu />} />
          <Route path="/new-career" element={<NewCareer />} />
          <Route path="/load-save" element={<LoadSave />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/dashboard" element={<Dashboard />} />
        </Routes>
      </UpdaterProvider>
    </BrowserRouter>
  );
}

export default App;
