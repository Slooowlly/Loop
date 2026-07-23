// Mapa índice→composto do `CarIdxTireCompound` do iRacing (a mesma info que o RaceLab
// e a torre usam). O SDK entrega um ÍNDICE 0-based POR SÉRIE (0 = 1º composto, 1 = 2º…),
// -1 = desconhecido/ainda não escolhido. Antes o overlay achatava tudo em seco/molhado
// (`>=1 → wet`); aqui centralizamos a leitura num mapa por categoria, com FALLBACK seguro.
//
// Regra de ouro (segurança): NUNCA rotular o pneu de CHUVA como um slick. No iRacing o
// composto de chuva é um composto separado, sempre ACIMA dos secos. Então cada categoria
// declara só a ORDEM DOS SECOS por índice; o wet é o índice logo após essa lista.
//
// Hoje as categorias do jogo rodam 1 slick + chuva (nenhuma expõe macio/médio/duro
// selecionável), então o DEFAULT reproduz exatamente o comportamento antigo: idx 0 = seco,
// idx ≥1 = chuva. Quando uma série ganhar opção de composto, basta listar a ordem dos secos
// em `COMPOUND_SCHEMES` (ex.: `gt3: ["soft", "medium", "hard"]`) — o wet vira o índice
// seguinte automaticamente, sem tocar em mais nada.

// 1 slick só. idx 0 = seco; qualquer índice acima = chuva.
const DEFAULT_SCHEME = ["dry"];

// Ordem dos compostos SECOS por índice, por id de categoria do evento. Ausente → DEFAULT.
const COMPOUND_SCHEMES = {
  // (sem séries multi-composto por enquanto — ver nota acima)
};

// Ordem canônica dos "kinds" (macio → duro → seco → chuva) para cores/ícones estáveis.
export const COMPOUND_KINDS = ["soft", "medium", "hard", "dry", "wet"];

/**
 * Resolve o COMPOSTO nomeado de um índice de `CarIdxTireCompound`.
 * @param {string|null|undefined} category  id da categoria do evento (ex.: "gt3")
 * @param {number} index  índice 0-based do SDK; -1/negativo = desconhecido
 * @returns {"soft"|"medium"|"hard"|"dry"|"wet"|null}  null = desconhecido
 */
export function tireCompoundKind(category, index) {
  if (typeof index !== "number" || index < 0) return null; // desconhecido/não escolhido
  const dry = COMPOUND_SCHEMES[category] ?? DEFAULT_SCHEME;
  if (index < dry.length) return dry[index]; // composto seco nomeado
  return "wet"; // acima dos secos = chuva
}

/**
 * Classificação binária seco/molhado — para camadas que só distinguem os dois mundos
 * (ícones de pneu da coluna de paradas, telemetria). null = desconhecido.
 * @returns {"dry"|"wet"|null}
 */
export function tireCompoundDryWet(category, index) {
  const kind = tireCompoundKind(category, index);
  if (kind == null) return null;
  return kind === "wet" ? "wet" : "dry";
}
