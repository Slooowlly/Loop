// Sistema de "menção de piloto": realça nomes de pilotos conhecidos dentro de um
// texto (narrativa da IA, boletim, debrief) e, ao passar o mouse num nome, dispara
// um callback com o id do piloto para acendê-lo em outra parte da tela (favoritos,
// tabela do campeonato, resultados, construtores…). Compartilhado entre a Sala de
// Estratégia, o News e o Debriefing pós-corrida.

export function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// Monta o matcher de nomes a partir de uma lista de pilotos `{ id, nome }`. Retorna
// `{ regex, byName }` (byName: nome → id) ou null quando não há nomes utilizáveis.
// Além do nome completo, aceitamos formas abreviadas (sobrenome e primeiro nome),
// porque a IA muitas vezes cita o piloto só pelo sobrenome ("Ruiz", "Carvalho") em
// segunda menção — sem isso, esses nomes não realçam. Aliases abreviados só entram
// quando são inequívocos (apontam para um único piloto): se dois pilotos dividem o
// sobrenome "Silva", esse alias fica de fora para nunca acender o piloto errado.
// Nomes mais longos vêm primeiro para "Ramiro Ruiz" casar antes de "Ruiz"; limites
// por letra unicode evitam casar pedaço de outra palavra.
export function buildDriverMentionMatcher(drivers) {
  const named = (Array.isArray(drivers) ? drivers : [])
    .filter((driver) => driver?.id && typeof driver?.nome === "string" && driver.nome.trim().length > 1)
    .map((driver) => ({ id: driver.id, name: driver.nome.trim() }));
  if (named.length === 0) {
    return null;
  }

  // Conta quantos ids cada alias abreviado geraria; só os de id único sobrevivem.
  const aliasIds = new Map();
  const addAlias = (raw, id) => {
    const alias = raw.trim();
    if (alias.length <= 1) return;
    if (!aliasIds.has(alias)) aliasIds.set(alias, new Set());
    aliasIds.get(alias).add(id);
  };
  for (const { id, name } of named) {
    const tokens = name.split(/\s+/).filter(Boolean);
    if (tokens.length >= 2) {
      addAlias(tokens[tokens.length - 1], id); // sobrenome
      addAlias(tokens[0], id); // primeiro nome
    }
  }

  // Nome completo tem prioridade absoluta; aliases inequívocos que não colidem com
  // um nome completo já mapeado entram em seguida.
  const byName = new Map();
  for (const { id, name } of named) {
    byName.set(name, id);
  }
  for (const [alias, ids] of aliasIds) {
    if (ids.size === 1 && !byName.has(alias)) {
      byName.set(alias, [...ids][0]);
    }
  }

  const pattern = [...byName.keys()]
    .sort((left, right) => right.length - left.length)
    .map((name) => escapeRegExp(name))
    .join("|");
  const regex = new RegExp(`(?<!\\p{L})(${pattern})(?!\\p{L})`, "gu");
  return { regex, byName };
}

// Classe do nome realçado. `activeClass` (quando o piloto está com hover) e
// `idleClass` são parametrizáveis para as telas com fundo/acento próprios.
export function driverMentionClass(isActive, activeClass, idleClass) {
  return [
    "cursor-default font-semibold underline decoration-dotted decoration-[#58a6ff]/40 underline-offset-2 transition",
    isActive ? activeClass : idleClass,
  ].join(" ");
}

// Renderiza `text` como um array de nós React onde cada nome de piloto vira um
// <span> interativo. `onHover(id | null)` acende/apaga o piloto correspondente.
export function renderTextWithDriverMentions(
  text,
  drivers,
  hoveredDriverId,
  onHover,
  { activeClass = "text-[#58a6ff]", idleClass = "text-white hover:text-[#58a6ff]" } = {},
) {
  if (typeof text !== "string" || !text) {
    return text;
  }
  const matcher = buildDriverMentionMatcher(drivers);
  if (!matcher) {
    return text;
  }
  return text.split(matcher.regex).map((part, index) => {
    const driverId = matcher.byName.get(part);
    if (!driverId) {
      return part;
    }
    const isActive = hoveredDriverId === driverId;
    return (
      <span
        key={index}
        onMouseEnter={() => onHover(driverId)}
        onMouseLeave={() => onHover(null)}
        className={driverMentionClass(isActive, activeClass, idleClass)}
      >
        {part}
      </span>
    );
  });
}
