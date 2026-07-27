import { getTrackImageSrc as getTrackImageSrcDeRace } from "./calendarShared";
import { getTrackImageSrc, getTrackThumbnailSrc, normalizeTrackName } from "./trackImages";

// Estes testes existem porque a normalização de nome é a CHAVE DE LOOKUP do
// arquivo de imagem: se ela mudar sem querer, uma pista some da tela sem erro
// nenhum no console. Antes deste arquivo, nada travava essa regressão.
describe("normalizeTrackName", () => {
  it("remove acento, mantém espaço e pontuação, e devolve minúscula", () => {
    expect(normalizeTrackName("Autódromo José Carlos Pace")).toBe("autodromo jose carlos pace");
    expect(normalizeTrackName("Nürburgring Nordschleife")).toBe("nurburgring nordschleife");
    expect(normalizeTrackName("Circuit de Lédenon")).toBe("circuit de ledenon");
    expect(normalizeTrackName("MotorLand Aragón - National")).toBe("motorland aragon - national");
    expect(normalizeTrackName("Portimão")).toBe("portimao");
  });

  it("trata nulo/indefinido como string vazia", () => {
    expect(normalizeTrackName(null)).toBe("");
    expect(normalizeTrackName(undefined)).toBe("");
  });
});

describe("getTrackThumbnailSrc", () => {
  it("casa por nome normalizado, mesmo com acento e sufixo de traçado", () => {
    expect(getTrackThumbnailSrc("Circuit de Lédenon")).toBe("/utilities/tracks/ledenon.webp");
    expect(getTrackThumbnailSrc("WeatherTech Raceway at Laguna Seca - 2026")).toBe(
      "/utilities/tracks/lagunaseca.webp",
    );
    expect(getTrackThumbnailSrc("Circuito de Navarra - Speed Circuit")).toBe(
      "/utilities/tracks/Navarra.webp",
    );
  });

  // A divergência que existia: o Header casava só "oulton park", enquanto os
  // nomes reais no calendário incluem "Oulton Fosters"/"Oulton Intl"/"Oulton Island".
  it.each(["Oulton Park Circuit", "Oulton Fosters", "Oulton Intl", "Oulton Island"])(
    "casa a variante de traçado %s",
    (nome) => {
      expect(getTrackThumbnailSrc(nome)).toBe("/utilities/tracks/oultonpark.jpeg");
    },
  );

  it("codifica espaço no caminho, venha o arquivo do nome ou do track_id", () => {
    expect(getTrackThumbnailSrc("Motorsport Arena Oschersleben")).toBe(
      "/utilities/tracks/motorsport%20arena.webp",
    );
    // 449 = Oschersleben; nome não reconhecido força o caminho do id.
    expect(getTrackThumbnailSrc("Pista Sem Nome", 449)).toBe(
      "/utilities/tracks/motorsport%20arena.webp",
    );
  });

  it("prioriza o casamento por nome sobre o track_id", () => {
    expect(getTrackThumbnailSrc("Tsukuba Circuit", 449)).toBe("/utilities/tracks/Tsukuba.webp");
  });

  it("por padrão chuta <nome>.webp quando nada casa", () => {
    expect(getTrackThumbnailSrc("Circuito Desconhecido XYZ")).toBe(
      "/utilities/tracks/Circuito%20Desconhecido%20XYZ.webp",
    );
  });

  it("devolve null quando nada casa e o chamador pediu aoFalhar: nulo", () => {
    expect(getTrackThumbnailSrc("Circuito Desconhecido XYZ", null, { aoFalhar: "nulo" })).toBeNull();
    expect(getTrackThumbnailSrc(null, null, { aoFalhar: "nulo" })).toBeNull();
  });
});

describe("getTrackImageSrc (assinatura por nome + id)", () => {
  it("delega para o resolvedor com a política de chute", () => {
    expect(getTrackImageSrc("Lime Rock Park", 353)).toBe("/utilities/tracks/limerock.jpeg");
    expect(getTrackImageSrc("Nada", null)).toBe("/utilities/tracks/Nada.webp");
  });
});

// O calendário é o único consumidor que trata "sem imagem" desenhando um
// placeholder (EventRow) — por isso a política dele é null, não chute.
describe("calendarShared.getTrackImageSrc (assinatura por corrida)", () => {
  it("resolve pelo track_name da corrida", () => {
    expect(getTrackImageSrcDeRace({ track_name: "Winton Motor Raceway" })).toBe(
      "/utilities/tracks/winton.jpeg",
    );
  });

  it("cai no track_id quando o nome não casa", () => {
    expect(getTrackImageSrcDeRace({ track_name: "Pista Nova", track_id: 586 })).toBe(
      "/utilities/tracks/lagunaseca.webp",
    );
  });

  it("devolve null (e não um caminho quebrado) quando não há imagem", () => {
    expect(getTrackImageSrcDeRace({ track_name: "Pista Nova" })).toBeNull();
    expect(getTrackImageSrcDeRace(null)).toBeNull();
  });
});
