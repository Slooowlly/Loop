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

// ── Tags explícitas de menção (contrato com o servidor de IA) ────────────────
// O casamento por nome sozinho não consegue ligar um APELIDO ("o líder", "o novato
// da Northgate") ao piloto — o texto visível não contém o nome. Para isso, a IA
// marca CADA referência com a identidade canônica, em sintaxe estilo wiki:
//
//   [[Nathaniel Turner|o líder]]   → mostra "o líder", liga ao piloto Nathaniel Turner
//   [[Nathaniel Turner]]           → mostra e liga "Nathaniel Turner"
//
// `canônico` é o nome do piloto EXATAMENTE como veio nos fatos (resolve o id via
// `byName`). Só o texto visível é renderizado; os delimitadores somem. É opcional e
// retrocompatível: texto sem tags cai no casamento por nome de sempre.
const MENTION_TAG = /\[\[([^\][]+?)\]\]/g;

function parseMentionTag(inner) {
  const bar = inner.indexOf("|");
  if (bar === -1) {
    const name = inner.trim();
    return { canonical: name, visible: name };
  }
  return {
    canonical: inner.slice(0, bar).trim(),
    visible: inner.slice(bar + 1).trim(),
  };
}

// Casa nomes "crus" (sem tag) dentro de um trecho e empurra os segmentos resultantes.
function pushBareSegments(chunk, matcher, out) {
  if (!chunk) return;
  for (const part of chunk.split(matcher.regex)) {
    if (part === "") continue;
    const id = matcher.byName.get(part);
    out.push(id ? { type: "driver", id, text: part } : { type: "text", text: part });
  }
}

// Quebra `text` numa lista de segmentos { type: "driver", id, text } | { type: "text", text }.
// Primeiro extrai as tags explícitas da IA (que resolvem apelidos → id); o texto entre e
// fora das tags ainda passa pelo casamento por nome, para nomes citados sem tag continuarem
// acendendo (legado + resiliência caso a IA marque só parte das menções). Os renderizadores
// (boletim, debrief, prévia) constroem o JSX em cima disto — a segmentação mora aqui.
export function segmentDriverMentions(text, drivers) {
  if (typeof text !== "string" || !text) return [];
  const matcher = buildDriverMentionMatcher(drivers);
  if (!matcher) return [{ type: "text", text }];

  const segments = [];
  let last = 0;
  MENTION_TAG.lastIndex = 0;
  let match;
  while ((match = MENTION_TAG.exec(text)) !== null) {
    if (match.index > last) {
      pushBareSegments(text.slice(last, match.index), matcher, segments);
    }
    const { canonical, visible } = parseMentionTag(match[1]);
    const id = matcher.byName.get(canonical);
    if (id && visible) {
      segments.push({ type: "driver", id, text: visible });
    } else {
      // Nome não reconhecido (ou visível vazio): nunca mostra os colchetes crus —
      // ainda tenta casar um nome dentro do próprio texto visível.
      pushBareSegments(visible || match[1], matcher, segments);
    }
    last = MENTION_TAG.lastIndex;
  }
  if (last < text.length) {
    pushBareSegments(text.slice(last), matcher, segments);
  }
  return segments;
}

// Renderiza `text` como um array de nós React onde cada menção de piloto — nome OU
// apelido marcado pela IA — vira um <span> interativo. `onHover(id | null)` acende/apaga
// o piloto correspondente.
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
  const segments = segmentDriverMentions(text, drivers);
  return segments.map((seg, index) => {
    if (seg.type !== "driver") {
      return seg.text;
    }
    const isActive = hoveredDriverId === seg.id;
    return (
      <span
        key={index}
        onMouseEnter={() => onHover(seg.id)}
        onMouseLeave={() => onHover(null)}
        className={driverMentionClass(isActive, activeClass, idleClass)}
      >
        {seg.text}
      </span>
    );
  });
}
