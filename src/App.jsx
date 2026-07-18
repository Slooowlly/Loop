import { BrowserRouter, Routes, Route } from "react-router-dom";
import MainMenu from "./pages/MainMenu";
import NewCareer from "./pages/NewCareer";
import LoadSave from "./pages/LoadSave";
import Settings from "./pages/Settings";
import Dashboard from "./pages/Dashboard";
import WindowControlsDrawer from "./components/layout/WindowControlsDrawer";
import OverlayVrWriter from "./overlay/OverlayVrWriter";
import OverlayPositionPanel from "./overlay/OverlayPositionPanel";
import OverlayMonitorAuto from "./overlay/OverlayMonitorAuto";
import PoachAuctionHost from "./components/season/PoachAuctionHost";

function App() {
  return (
    <BrowserRouter>
      <WindowControlsDrawer />
      <OverlayVrWriter />
      <OverlayPositionPanel />
      <OverlayMonitorAuto />
      <PoachAuctionHost />
      <Routes>
        <Route path="/" element={<MainMenu intro />} />
        <Route path="/menu" element={<MainMenu />} />
        <Route path="/new-career" element={<NewCareer />} />
        <Route path="/load-save" element={<LoadSave />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/dashboard" element={<Dashboard />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
