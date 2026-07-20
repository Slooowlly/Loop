import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import OverlayApp from "./overlay/OverlayApp";
import OverlayVersions from "./overlay/OverlayVersions";
import { EngineerRadioLive, EngineerRadioPreview } from "./overlay/EngineerRadio";
import "@fontsource-variable/space-grotesk";
import "./index.css";
import "./i18n"; // inicializa o i18next (UI estática, Fase 0 da tradução).
import useCareerStore from "./stores/useCareerStore";

// A MESMA build serve a janela principal E a de overlay. Qual é qual:
//   • No app (Tauri): pela LABEL da janela. A janela criada pelo Rust chama-se
//     "overlay" e renderiza a torre AO VIVO transparente por cima do jogo; a
//     principal ("main") renderiza o app normal. (Usamos a label, não um hash na
//     URL, porque o `#` na WebviewUrl vira %23 e quebra a rota.)
//   • No navegador (sem Tauri): pelas rotas por HASH, só pra inspecionar —
//     `#overlay-versions` = 3 peles; `#overlay*` = torre com xadrez + mock.
const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const hash = window.location.hash;

let tauriLabel = null;
if (IN_TAURI) {
  try {
    tauriLabel = getCurrentWindow().label;
  } catch {
    /* fora do Tauri / API indisponível */
  }
}
const isOverlayWindow = tauriLabel === "overlay";
const isEngineerWindow = tauriLabel === "engineer";
const isRealOverlay = isOverlayWindow || isEngineerWindow || hash === "#overlay" || hash === "#engineer";

// A janela real precisa ser TRANSPARENTE de verdade: o body tem fundo sólido
// (var(--app-bg)); zera aqui pra o clique-atravessa mostrar só a torre.
if (isRealOverlay) {
  document.documentElement.style.background = "transparent";
  document.body.style.background = "transparent";
}

function Root() {
  if (hash === "#overlay-versions") return <OverlayVersions />;
  if (hash === "#engineer-preview") return <EngineerRadioPreview />;
  if (isEngineerWindow || hash === "#engineer") return <EngineerRadioLive />;
  if (isOverlayWindow) return <OverlayApp preview={false} />;
  if (hash.startsWith("#overlay")) return <OverlayApp preview={hash !== "#overlay"} />;
  return <App />;
}

// Aplica o idioma persistido (config.json) logo no boot da janela principal, pra
// menu/Settings já abrirem no idioma certo antes de qualquer carreira carregar.
// As janelas de overlay não têm UI de menu — não precisam.
if (!isRealOverlay) {
  void useCareerStore.getState().loadLanguage();
}

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
