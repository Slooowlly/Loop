import i18n from "../i18n/index.js";
import { weatherLabel as labelCalendario } from "./calendarShared";
import { CLIMA_BANNER, weatherEmoji, weatherLabel } from "./weather";

const idiomaOriginal = i18n.language;

afterEach(async () => {
  await i18n.changeLanguage(idiomaOriginal);
});

describe("weatherLabel", () => {
  it("traduz as condições no namespace do calendário", () => {
    expect(labelCalendario("HeavyRain")).toBe("Chuva Forte");
    expect(labelCalendario("Wet")).toBe("Chuva");
    expect(labelCalendario("Damp")).toBe("Úmido");
    expect(labelCalendario("Dry")).toBe("Seco");
  });

  // Divergência INTENCIONAL: no banner o rótulo pareia com o ⛅, não com um sol.
  it("mantém o default do banner em Parcialmente Nublado", () => {
    expect(weatherLabel("Dry", CLIMA_BANNER)).toBe("Parcialmente Nublado");
    expect(weatherLabel("HeavyRain", CLIMA_BANNER)).toBe("Chuva Forte");
  });

  // Havia aqui um par de casos sobre `weatherLabel` de `race/raceresult/helpers.js`, a versão
  // em caixa baixa da tela de resultado V1. A árvore V1 inteira saiu do repositório em
  // 11/08/2026 (Dashboard usa RaceResultViewV2 desde sempre e nada mais importava aquele
  // módulo), então os dois casos protegiam código que não existe mais. A regressão que eles
  // travavam — prosa PT hardcoded num arquivo `.js`, fora do alcance do hook de pre-commit —
  // continua coberta pelo caso de en-US logo abaixo e pelo auditor de i18n.

  it("traduz o calendário em en-US", async () => {
    await i18n.changeLanguage("en-US");
    expect(labelCalendario("HeavyRain")).toBe(i18n.t("weather.heavyRain"));
    expect(labelCalendario("Dry")).toBe(i18n.t("weather.dry"));
  });

  it("trata condição desconhecida como tempo seco", () => {
    expect(labelCalendario("Clear")).toBe("Seco");
    expect(labelCalendario(null)).toBe("Seco");
  });
});

describe("weatherEmoji", () => {
  it("pareia o glifo com a condição", () => {
    expect(weatherEmoji("HeavyRain")).toBe("\u{26C8}\u{FE0F}");
    expect(weatherEmoji("Wet")).toBe("\u{1F327}\u{FE0F}");
    expect(weatherEmoji("Damp")).toBe("\u{1F326}\u{FE0F}");
    expect(weatherEmoji("Dry")).toBe("\u{26C5}");
  });
});
