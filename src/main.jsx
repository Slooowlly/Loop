import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import OverlayApp from "./overlay/OverlayApp";
import OverlayVersions from "./overlay/OverlayVersions";
import { EngineerRadioLive, EngineerRadioPreview } from "./overlay/EngineerRadio";
import RadioCanvasPreview from "./overlay/RadioCanvasPreview";
import "@fontsource-variable/space-grotesk";
import "./index.css";
import "./styles/kardust.css"; // Kardust: destaques (home, notícias, calendário)
import "./i18n"; // inicializa o i18next (UI estática, Fase 0 da tradução).
import useCareerStore from "./stores/useCareerStore";
import { estaNoTauri } from "./lib/tauri";

// A MESMA build serve a janela principal E a de overlay. Qual é qual:
//   • No app (Tauri): pela LABEL da janela. A janela criada pelo Rust chama-se
//     "overlay" e renderiza a torre AO VIVO transparente por cima do jogo; a
//     principal ("main") renderiza o app normal. (Usamos a label, não um hash na
//     URL, porque o `#` na WebviewUrl vira %23 e quebra a rota.)
//   • No navegador (sem Tauri): pelas rotas por HASH, só pra inspecionar —
//     `#overlay-versions` = 3 peles; `#overlay*` = torre com xadrez + mock.
const hash = window.location.hash;

let tauriLabel = null;
if (estaNoTauri()) {
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
  if (hash === "#radio-canvas") return <RadioCanvasPreview />;
  if (isEngineerWindow || hash === "#engineer") return <EngineerRadioLive />;
  if (isOverlayWindow) return <OverlayApp preview={false} />;
  if (hash.startsWith("#overlay")) return <OverlayApp preview={hash !== "#overlay"} />;
  return <App />;
}

// Aplica o idioma persistido (config.json) logo no boot, pra a UI já abrir no
// idioma certo antes de qualquer carreira carregar. Vale para TODAS as janelas —
// inclusive as de overlay, que também têm textos traduzíveis (rótulo do rádio,
// painel de posição VR).
void useCareerStore.getState().loadLanguage();

// O entry vive no grafo de HMR do Vite. Sem guarda, cada hot-reload re-executa
// esta linha e chama createRoot() DE NOVO no mesmo #root — dois roots brigam
// pelo mesmo DOM e o react-refresh estoura "removeChild ... not a child of this
// node". Cacheamos o root no próprio container: no primeiro boot cria; nos
// reloads seguintes só re-renderiza no root existente.
const container = document.getElementById("root");
const root = (container.__reactRoot ??= ReactDOM.createRoot(container));
root.render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
