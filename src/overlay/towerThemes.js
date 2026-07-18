// Três "peles" da torre. MESMA estrutura (colunas, tamanhos, janela dos 15,
// pins, pilha de pneus, pontos) — o que muda é o visual. `drawTower` recebe uma
// delas; a escolhida serve monitor e VR igual, sem retrabalho.
//
// Como escolher depois: fixar `DEFAULT_THEME = THEMES.<nome>`.

const COMMON = {
  text: "#f0f6fc",
  textMuted: "#6e7681",
  posColor: "#e6edf3",
  playerColor: "#3fb950", // nome do jogador
  teammateColor: "#58a6ff", // nome do companheiro
  purple: "#bc8cff", // volta mais rápida
  gainGreen: "#7ee787", // ganho de pontos
  up: "#3fb950",
  down: "#f85149",
};

export const THEMES = {
  // 1) BROADCAST — o que já temos. Painel escuro sólido, tint em degradê da cor
  //    da equipe, aba de classe. Densa e "TV timing".
  broadcast: {
    ...COMMON,
    key: "broadcast",
    label: "Broadcast",
    panelBg: "rgba(13,16,19,0.96)",
    sessionBg: "rgba(6,8,10,0.98)",
    sessionTop: "rgba(18,22,28,0.98)",
    colHeadBg: "rgba(13,16,19,0.96)",
    classBg: "rgba(0,0,0,0.35)",
    blockRadius: 0,
    rowStyle: "gradient",
    accentWidth: 3,
    classStyle: "tab",
    rowAlpha: 0.22,
    sheen: false,
  },

  // 2) CARBON GLASS — painel mais translúcido (vê o jogo por baixo), blocos com
  //    cantos arredondados, brilho sutil no topo da linha, texto mais claro,
  //    classe com sublinhado colorido. Mais "HUD premium".
  glass: {
    ...COMMON,
    key: "glass",
    label: "Carbon Glass",
    text: "#ffffff",
    panelBg: "rgba(18,22,28,0.72)",
    sessionBg: "rgba(10,14,20,0.82)",
    sessionTop: "rgba(28,36,48,0.85)",
    colHeadBg: "rgba(18,22,28,0.5)",
    classBg: "rgba(255,255,255,0.06)",
    blockRadius: 9,
    rowStyle: "glow",
    accentWidth: 2,
    classStyle: "underline",
    rowAlpha: 0.18,
    sheen: true,
  },

  // 3) STRIPE — chrome mínimo, fundo quase transparente, a POSIÇÃO vira um chip
  //    sólido na cor da equipe (acento e número num só bloco), nomes em branco
  //    forte, classe como rótulo grande. Feito pra legibilidade num relance (VR).
  stripe: {
    ...COMMON,
    key: "stripe",
    label: "Stripe",
    panelBg: "rgba(8,10,12,0.75)",
    sessionBg: "rgba(8,10,12,0.92)",
    sessionTop: "rgba(20,26,34,0.94)",
    colHeadBg: "rgba(0,0,0,0)",
    classBg: "rgba(0,0,0,0)",
    blockRadius: 0,
    rowStyle: "block",
    accentWidth: 0,
    classStyle: "band",
    rowAlpha: 0.14,
    sheen: false,
  },
};

export const DEFAULT_THEME = THEMES.stripe;
export const THEME_LIST = [THEMES.broadcast, THEMES.glass, THEMES.stripe];
