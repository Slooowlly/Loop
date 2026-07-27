import i18n from "../i18n/index.js";
import { weatherLabel as labelCalendario } from "./calendarShared";
import { weatherLabel as labelResultado } from "../components/race/raceresult/helpers";
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

  it("usa a copy em caixa baixa no resultado de corrida", () => {
    expect(labelResultado("HeavyRain")).toBe("Chuva forte");
    expect(labelResultado("Wet")).toBe("Chuva");
    expect(labelResultado("Damp")).toBe("Úmido");
    expect(labelResultado("Dry")).toBe("Seco");
  });

  // A regressão que este arquivo trava: a versão do resultado tinha a prosa PT
  // hardcoded e vazava para en-US. Como o arquivo é .js, o hook de pre-commit
  // (que só olha .jsx) não pegava.
  it("traduz o resultado de corrida em en-US", async () => {
    await i18n.changeLanguage("en-US");
    expect(labelResultado("HeavyRain")).toBe("Heavy rain");
    expect(labelResultado("Wet")).toBe("Rain");
    expect(labelResultado("Damp")).toBe("Damp");
    expect(labelResultado("Dry")).toBe("Dry");
  });

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
