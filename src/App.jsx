import { BrowserRouter, Routes, Route } from "react-router-dom";
import BootLogoScreen from "./pages/BootLogoScreen";
import SplashScreen from "./pages/SplashScreen";
import MainMenu from "./pages/MainMenu";
import NewCareer from "./pages/NewCareer";
import LoadSave from "./pages/LoadSave";
import Settings from "./pages/Settings";
import Dashboard from "./pages/Dashboard";
import WindowControlsDrawer from "./components/layout/WindowControlsDrawer";

function App() {
  return (
    <BrowserRouter>
      <WindowControlsDrawer />
      <Routes>
        <Route path="/" element={<BootLogoScreen />} />
        <Route path="/splash" element={<SplashScreen />} />
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
