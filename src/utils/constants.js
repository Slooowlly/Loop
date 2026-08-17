// Passos do wizard como slugs estáveis; o rótulo exibido vem de
// `newCareer.wizardSteps.<slug>` resolvido no NewCareer (que tem o hook `t`).
export const WIZARD_STEPS = ["driver", "history", "category", "team", "confirm"];

// O vocabulário de dificuldade que o backend aceita. O wizard deixou de oferecer a
// escolha em 16/08/2026, porque a dificuldade da IA passou a ser adaptativa: aqui
// sobrou só a validação do valor que vem gravado num draft retomado.
export const DIFFICULTIES = [
  { id: "facil" },
  { id: "medio" },
  { id: "dificil" },
  { id: "lendario" },
];

// A dificuldade que todo mundo histórico novo recebe, já que ninguém mais a escolhe.
export const DIFFICULTY_PADRAO = "medio";

// O rótulo (bandeira + gentílico) vem de `newCareer.nationality.<id>` /
// `<id>_fem`, resolvido no render. O emoji da bandeira mora no valor traduzido.
export const NATIONALITIES = [
  { id: "br" },
  { id: "gb" },
  { id: "de" },
  { id: "fr" },
  { id: "it" },
  { id: "es" },
  { id: "nl" },
  { id: "au" },
  { id: "jp" },
  { id: "us" },
  { id: "mx" },
  { id: "ar" },
  { id: "fi" },
  { id: "be" },
  { id: "pt" },
  { id: "ca" },
  { id: "at" },
  { id: "ch" },
  { id: "dk" },
  { id: "se" },
  { id: "no" },
  { id: "pl" },
  { id: "cn" },
];

export const STARTING_CATEGORIES = [
  {
    id: "mazda_rookie",
    name: "Mazda Rookie",
    car: "Mazda MX-5",
    logo: new URL("../assets/utilities/source-images/Categorias/MX5 ROOKIE.webp", import.meta.url).href,
    teams: 6,
    races: 5,
    drivers: 12,
    // `description` migrou para `newCareer.category.<id>.description` (i18n).
  },
  {
    id: "toyota_rookie",
    name: "Toyota Rookie",
    car: "Toyota GR86",
    logo: new URL("../assets/utilities/source-images/Categorias/GR ROOKIE.webp", import.meta.url).href,
    teams: 6,
    races: 5,
    drivers: 12,
    // `description` migrou para `newCareer.category.<id>.description` (i18n).
  },
];

export const TEAM_PREVIEWS = {
  mazda_rookie: [
    { index: 0, name: "Racing Academy Red", shortName: "RAR", primaryColor: "#e63946", secondaryColor: "#e63946", country: "🇺🇸 EUA", performanceRating: 68 },
    { index: 1, name: "Rolling Thunder Academy", shortName: "RTA", primaryColor: "#2f3542", secondaryColor: "#2f3542", country: "🇺🇸 EUA", performanceRating: 74 },
    { index: 2, name: "Grid Start Racing School", shortName: "GSR", primaryColor: "#f6c90e", secondaryColor: "#f6c90e", country: "🇬🇧 Reino Unido", performanceRating: 60 },
    { index: 3, name: "First Gear Motorsport", shortName: "FGM", primaryColor: "#3a86ff", secondaryColor: "#3a86ff", country: "🇩🇪 Alemanha", performanceRating: 71 },
    { index: 4, name: "Apex Academy Racing", shortName: "AAR", primaryColor: "#2ec4b6", secondaryColor: "#2ec4b6", country: "🇫🇷 Franca", performanceRating: 78 },
    { index: 5, name: "Rookie Squad Racing", shortName: "RSQ", primaryColor: "#9b5de5", secondaryColor: "#9b5de5", country: "🇯🇵 Japao", performanceRating: 64 },
  ],
  toyota_rookie: [
    { index: 0, name: "Sakura Driver Academy", shortName: "SDA", primaryColor: "#d90429", secondaryColor: "#d90429", country: "🇯🇵 Japao", performanceRating: 74 },
    { index: 1, name: "Kanzen Racing School", shortName: "KRS", primaryColor: "#264653", secondaryColor: "#264653", country: "🇯🇵 Japao", performanceRating: 78 },
    { index: 2, name: "Open Road Academy", shortName: "ORA", primaryColor: "#8b5e34", secondaryColor: "#8b5e34", country: "🇺🇸 EUA", performanceRating: 64 },
    { index: 3, name: "Speed Lab Rookies", shortName: "SLR", primaryColor: "#fb5607", secondaryColor: "#fb5607", country: "🇺🇸 EUA", performanceRating: 68 },
    { index: 4, name: "Rising Stars Motorsport", shortName: "RSM", primaryColor: "#ffbe0b", secondaryColor: "#ffbe0b", country: "🇬🇧 Reino Unido", performanceRating: 71 },
    { index: 5, name: "Fresh Start Racing", shortName: "FSR", primaryColor: "#80b918", secondaryColor: "#80b918", country: "🇩🇪 Alemanha", performanceRating: 60 },
  ],
};

// De quanto em quanto tempo a mensagem do overlay de carregamento troca. Os TEXTOS não moram
// aqui: eles são as chaves `newCareer.loadingMessages.msg<i>` do locale, e a tela cicla por
// elas. Havia uma cópia dos 75 textos em português neste arquivo, usada só pelo `.length` —
// duas listas que ninguém garantia iguais, e a que o jogador lia era a outra.
export const LOADING_MESSAGE_INTERVAL_MS = 2000;
