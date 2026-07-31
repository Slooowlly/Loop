// Cor de equipe legível como TEXTO sobre fundo escuro: cores com luminância baixa
// são misturadas com branco até voltarem a ser lidas. Cada tela passa seu próprio
// `fallback` (o que mostrar quando não há cor de equipe) e, se precisar, seu `mix`.
// Os defaults aqui são os de 4 dos 6 pontos de uso — quem diverge diz isso na chamada.
export function getReadableTeamColor(color, { fallback = "#c9d1d9", mix = 0.58 } = {}) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) {
    return fallback;
  }

  const hex = color.slice(1);
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;

  if (luminance < 0.32) {
    const boost = (channel) => Math.round(channel + (255 - channel) * mix);
    return `rgb(${boost(r)}, ${boost(g)}, ${boost(b)})`;
  }

  return color;
}

// Destaque (glow) por cor de equipe. Cores muito escuras (ex.: Thunderline Academy)
// ficam quase invisíveis como brilho sobre um fundo escuro, então clareamos para um
// tom claro/branco quando a luminância é baixa. Retorna variações prontas:
//   solid → borda / linha (opaco)
//   soft  → fundo tênue da linha realçada
//   glow  → brilho externo (box-shadow) de cards
export function getTeamGlow(color) {
  let r = 201;
  let g = 209;
  let b = 217; // fallback cinza-claro (#c9d1d9) quando a cor é inválida/ausente
  if (typeof color === "string" && /^#([0-9a-f]{6})$/i.test(color)) {
    r = parseInt(color.slice(1, 3), 16);
    g = parseInt(color.slice(3, 5), 16);
    b = parseInt(color.slice(5, 7), 16);
    const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
    if (luminance < 0.35) {
      const mix = 0.6; // mistura com branco: clareia sem perder totalmente o matiz
      r = Math.round(r + (255 - r) * mix);
      g = Math.round(g + (255 - g) * mix);
      b = Math.round(b + (255 - b) * mix);
    }
  }
  return {
    solid: `rgb(${r}, ${g}, ${b})`,
    soft: `rgba(${r}, ${g}, ${b}, 0.20)`,
    glow: `rgba(${r}, ${g}, ${b}, 0.45)`,
  };
}

// Fundo dos cards escuros do dossiê. É contra ele que o contraste é medido —
// medir contra o preto puro aprovaria cores que somem sobre o card real.
const DARK_SURFACE = [15, 28, 43];
// 4.5:1 é o mínimo de texto normal do WCAG AA. Uma linha de gráfico de 2px pede
// pelo menos o mesmo que uma letra.
const MIN_CONTRAST = 4.5;
// Abaixo disto a cor não tem matiz para resgatar — cinza é cinza, e forçar
// saturação inventaria uma identidade que a equipe não tem.
const ACHROMATIC = 0.08;
// Acima disto a cor já é viva o bastante; mexer só a deixaria berrante.
const VIVID_ENOUGH = 0.45;
const SATURATION_FLOOR = 0.55;

function relativeLuminance([r, g, b]) {
  const canal = (v) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * canal(r) + 0.7152 * canal(g) + 0.0722 * canal(b);
}

function contrastRatio(a, b) {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

function rgbToHsl([r, g, b]) {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h;
  if (max === rn) h = ((gn - bn) / d + (gn < bn ? 6 : 0)) / 6;
  else if (max === gn) h = ((bn - rn) / d + 2) / 6;
  else h = ((rn - gn) / d + 4) / 6;
  return [h, s, l];
}

function hslToRgb([h, s, l]) {
  if (s === 0) {
    const v = Math.round(l * 255);
    return [v, v, v];
  }
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const canal = (t) => {
    let tt = t;
    if (tt < 0) tt += 1;
    if (tt > 1) tt -= 1;
    if (tt < 1 / 6) return p + (q - p) * 6 * tt;
    if (tt < 1 / 2) return q;
    if (tt < 2 / 3) return p + (q - p) * (2 / 3 - tt) * 6;
    return p;
  };
  return [canal(h + 1 / 3), canal(h), canal(h - 1 / 3)].map((v) => Math.round(v * 255));
}

// Cor da equipe pronta para VIRAR DADO num gráfico escuro: linha, ponto, área.
//
// `getTeamGlow` clareia o que está escuro, e isso basta para uma borda. Não basta
// para um gráfico: clarear um azul-quase-preto (#2f3542, a Thunderline) produz
// cinza claro — legível, e indistinguível da grade, dos rótulos e de todo o resto
// do cromo da interface, que também é cinza. A linha de dados deixava de ser
// achável mesmo estando visível.
//
// Aqui a saturação sobe junto com a luminância, então o matiz que a equipe TEM
// volta a aparecer. Quem já é vivo o bastante e já contrasta não é tocado — o
// ciano da Track Day Heroes sai daqui igual ao que entrou.
export function getVividTeamColor(color, { fallback = "#c9d1d9" } = {}) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) return fallback;
  const rgb = [
    parseInt(color.slice(1, 3), 16),
    parseInt(color.slice(3, 5), 16),
    parseInt(color.slice(5, 7), 16),
  ];
  const [h, s, l] = rgbToHsl(rgb);
  // Cinza de verdade não ganha matiz inventado; só é clareado até contrastar.
  const saturacao = s > ACHROMATIC && s < VIVID_ENOUGH ? SATURATION_FLOOR : s;
  let luz = l;
  let atual = hslToRgb([h, saturacao, luz]);
  // Sobe a luminância em passos até passar do mínimo. O teto de 0.82 evita que
  // uma cor escura termine em branco e perca o matiz que acabou de recuperar.
  while (contrastRatio(atual, DARK_SURFACE) < MIN_CONTRAST && luz < 0.82) {
    luz = Math.min(luz + 0.02, 0.82);
    atual = hslToRgb([h, saturacao, luz]);
  }
  return `rgb(${atual[0]}, ${atual[1]}, ${atual[2]})`;
}
