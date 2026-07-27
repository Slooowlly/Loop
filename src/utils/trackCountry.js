// País de um circuito a partir do track_name.
//
// A fonte de verdade é utils/trackCountries.js, GERADO de constants/tracks.rs —
// 264 nomes, incluindo nome completo, nome do local (antes de " - ") e nome curto.
// O Header mantinha uma lista curta própria, escrita à mão, que não reconhecia
// Montreal, Barber, Chicago, Miami, Adelaide, Portland, The Bend, St. Pete,
// Willow Springs, Knockhill nem Coronado — pistas que têm arte de banner e
// apareciam sem bandeira. Usar o gerado resolve isso e não sai do ar de novo
// quando alguém adicionar pista no Rust.
//
// O valor devolvido vem no formato "🇺🇸 EUA" (emoji + nome curto), que é o que
// FlagIcon/extractNationalityCode já sabem consumir.
import { TRACK_COUNTRIES } from "./trackCountries";
import { normalizeTrackName } from "./trackImages";

const POR_NOME_NORMALIZADO = new Map(
  Object.entries(TRACK_COUNTRIES).map(([nome, pais]) => [normalizeTrackName(nome), pais]),
);

function buscar(nome) {
  if (!nome) return null;
  return TRACK_COUNTRIES[nome] ?? POR_NOME_NORMALIZADO.get(normalizeTrackName(nome)) ?? null;
}

// Três tentativas, da mais estrita para a mais tolerante — nenhuma delas por
// substring, para nunca devolver bandeira errada:
//   1. nome exato        ("Interlagos")
//   2. nome normalizado  (tolera acento/caixa divergentes do save)
//   3. nome do local     ("Charlotte Motor Speedway - Roval" → "Charlotte Motor Speedway"),
//      espelhando o split_track_name do backend.
export function trackCountryLabel(trackName) {
  const bruto = (trackName ?? "").trim();
  if (!bruto) return null;

  const direto = buscar(bruto);
  if (direto) return direto;

  const venue = bruto.split(" - ")[0];
  return venue === bruto ? null : buscar(venue);
}
