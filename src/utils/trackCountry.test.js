import { TRACK_COUNTRIES } from "./trackCountries";
import { trackCountryLabel } from "./trackCountry";

describe("trackCountryLabel", () => {
  it("resolve pelo nome exato", () => {
    expect(trackCountryLabel("Interlagos")).toBe(TRACK_COUNTRIES["Interlagos"]);
    expect(trackCountryLabel("Suzuka International Racing Course")).toBe(
      TRACK_COUNTRIES["Suzuka International Racing Course"],
    );
  });

  it("tolera acento e caixa divergentes do save", () => {
    expect(trackCountryLabel("autodromo jose carlos pace")).toBe(
      TRACK_COUNTRIES["Autódromo José Carlos Pace"],
    );
  });

  it("cai no nome do local quando o traçado não está no mapa", () => {
    // Existe "Charlotte Motor Speedway - Roval 2025", não "- Roval".
    expect(trackCountryLabel("Charlotte Motor Speedway - Roval")).toBe(
      TRACK_COUNTRIES["Charlotte Motor Speedway"],
    );
  });

  // Estas pistas têm arte de banner e ficavam SEM bandeira, porque a lista curta
  // que o Header mantinha à mão não as cobria.
  it.each([
    "Circuit Gilles Villeneuve",
    "Montreal",
    "Barber Motorsports Park",
    "Chicago Street Course",
    "Miami International Autodrome",
    "Adelaide Street Circuit",
    "Portland International Raceway",
    "The Bend Motorsport Park",
    "St. Petersburg Grand Prix",
    "Willow Springs International Raceway",
    "Knockhill Racing Circuit",
  ])("reconhece %s, que a lista antiga do Header não cobria", (nome) => {
    expect(trackCountryLabel(nome)).toBeTruthy();
  });

  it("devolve null em vez de arriscar bandeira errada", () => {
    expect(trackCountryLabel("Circuito Desconhecido XYZ")).toBeNull();
    expect(trackCountryLabel("")).toBeNull();
    expect(trackCountryLabel(null)).toBeNull();
  });
});
