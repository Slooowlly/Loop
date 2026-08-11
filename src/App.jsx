import { Suspense, lazy } from "react";
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
import SpotterVoiceAuto from "./overlay/SpotterVoiceAuto";
import EngenheiroVozAuto from "./overlay/EngenheiroVozAuto";
import EngenheiroPttAuto from "./overlay/EngenheiroPttAuto";
import PoachAuctionHost from "./components/season/PoachAuctionHost";
import SeasonChampionOverlay from "./components/season/SeasonChampionOverlay";
import UpdaterProvider from "./components/system/UpdaterProvider";
import UpdateGate from "./components/system/UpdateGate";
import UpdateChangelogModal from "./components/system/UpdateChangelogModal";
import TelemetryConsentGate from "./components/system/TelemetryConsentGate";

// Bancada da POC de TTS (docs/tts-poc-latencia.md). `import.meta.env.DEV` vira a
// constante `false` no build de produção, então o import dinâmico morre junto com o
// ramo — nada da POC entra no bundle do jogador.
const TtsPocPage = import.meta.env.DEV ? lazy(() => import("./dev/tts/TtsPocPage")) : null;
// Bancada do push-to-talk: captura do microfone e, adiante, o botão do volante.
const PttPocPage = import.meta.env.DEV ? lazy(() => import("./dev/ptt/PttPocPage")) : null;

// Atalho para as bancadas. Elas só existem dentro do WebView2 do Tauri — o microfone e
// o botão de volante não têm como ser testados no navegador —, e a janela do app não tem
// barra de endereço. Sem isto, chegar lá dependeria de abrir o devtools e trocar a URL
// na mão. Morre no build de produção junto com as páginas.
// i18n-ignore
function DevLabs() {
  if (!import.meta.env.DEV) return null;
  return (
    <div style={{ position: "fixed", left: 8, bottom: 6, zIndex: 99999, display: "flex", gap: 10 }}>
      {["/dev/tts", "/dev/ptt"].map((r) => (
        <a
          key={r}
          href={r}
          style={{ fontSize: 10, color: "#8b949e", opacity: 0.45, textDecoration: "none" }}
        >
          {r}
        </a>
      ))}
    </div>
  );
}

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
        <SpotterVoiceAuto />
        <EngenheiroVozAuto />
        <EngenheiroPttAuto />
        <PoachAuctionHost />
        <SeasonChampionOverlay />
        <UpdateGate />
        <UpdateChangelogModal />
        <TelemetryConsentGate />
        <DevLabs />
        <Routes>
          <Route path="/" element={<MainMenu intro />} />
          <Route path="/menu" element={<MainMenu />} />
          <Route path="/new-career" element={<NewCareer />} />
          <Route path="/load-save" element={<LoadSave />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/dashboard" element={<Dashboard />} />
          {TtsPocPage ? (
            <Route
              path="/dev/tts"
              element={
                <Suspense fallback={null}>
                  <TtsPocPage />
                </Suspense>
              }
            />
          ) : null}
          {PttPocPage ? (
            <Route
              path="/dev/ptt"
              element={
                <Suspense fallback={null}>
                  <PttPocPage />
                </Suspense>
              }
            />
          ) : null}
        </Routes>
      </UpdaterProvider>
    </BrowserRouter>
  );
}

export default App;
