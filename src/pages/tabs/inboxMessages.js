// Voz da caixa de entrada. Recebe os FATOS reais do backend
// (`get_inbox_messages`) e monta as mensagens no formato que a casca da caixa já
// consome. Texto de template (i18n) — nada de IA. Read-only.

import i18n from "../../i18n/index.js";
import { ordinal } from "../../i18n/format.js";

// Atributos com frase de força/fraqueza; os demais não geram cláusula (null).
const KNOWN_ATTRS = new Set([
  "ritmo_classificacao",
  "gestao_pneus",
  "racecraft",
  "defesa",
  "habilidade_largada",
  "consistencia",
]);

const bold = (s) => `<b>${s}</b>`;

function attrClause(key, kind) {
  if (!KNOWN_ATTRS.has(key)) return null;
  return i18n.t(`inbox.attr.${key}.${kind}`);
}

// "Já cruzei com esse cara" — confronto direto com o rival mais enfrentado.
function headToHeadMessage(h) {
  if (!h || h.races_together <= 0) return null;
  const who = h.rival_team ? `${bold(h.rival_name)} (${h.rival_team})` : bold(h.rival_name);
  const n = h.races_together;
  let body = i18n.t("inbox.h2h.intro", { count: n, who });

  if (h.player_ahead <= 0) {
    body += i18n.t("inbox.h2h.lostAll");
  } else if (h.player_ahead >= n) {
    body += i18n.t("inbox.h2h.wonAll");
  } else {
    body += i18n.t("inbox.h2h.wonSome", { n: h.player_ahead });
    if (h.best_finish && h.best_track) {
      body += i18n.t("inbox.h2h.wonSomeBest", { ordinal: ordinal(h.best_finish), track: h.best_track });
    } else {
      body += i18n.t("inbox.h2h.wonSomeGeneric");
    }
  }

  return {
    id: "h2h",
    av: "p",
    ini: "GR",
    from: i18n.t("inbox.h2h.from"),
    kind: i18n.t("inbox.h2h.kind"),
    time: i18n.t("inbox.h2h.time"),
    subject: i18n.t("inbox.h2h.subject", { rival: h.rival_name }),
    body,
    actions: [],
  };
}

// "O favorito ao título" — o nome a bater na temporada.
function titleFavoriteMessage(f) {
  if (!f) return null;
  const who = f.driver_team ? `${bold(f.driver_name)} (${f.driver_team})` : bold(f.driver_name);

  // Perfil: veterano titulado vs. promessa.
  let profile;
  if (f.veteran && f.career_titles > 0) {
    profile = i18n.t("inbox.fav.profileVetTitles", { count: f.career_titles });
  } else if (f.veteran) {
    profile = i18n.t("inbox.fav.profileVet");
  } else {
    profile = i18n.t("inbox.fav.profileYoung");
  }

  // Situação na tabela.
  let standing;
  if (f.position === 0) {
    standing = i18n.t("inbox.fav.standingFavorite");
  } else if (f.position === 1) {
    standing =
      f.points_lead > 0
        ? i18n.t("inbox.fav.standingLead", { count: f.points_lead })
        : i18n.t("inbox.fav.standingTop");
  } else if (f.leads_player) {
    standing = i18n.t("inbox.fav.standingAhead", { ordinal: ordinal(f.position) });
  } else {
    standing = i18n.t("inbox.fav.standingBehind", { ordinal: ordinal(f.position) });
  }

  const strong = attrClause(f.strong_attr, "f");
  const weak = attrClause(f.weak_attr, "w");
  const traits =
    strong && weak
      ? i18n.t("inbox.fav.traits", {
          strong: `${strong[0].toUpperCase()}${strong.slice(1)}`,
          weak,
        })
      : "";

  const body = i18n.t("inbox.fav.body", { who, profile, standing, traits });

  return {
    id: "fav",
    av: "g",
    ini: "★",
    from: i18n.t("inbox.fav.from"),
    kind: f.position === 0 ? i18n.t("inbox.fav.kindFavorite") : i18n.t("inbox.fav.kindExpectation"),
    time: i18n.t("inbox.fav.time"),
    subject:
      f.position === 0
        ? i18n.t("inbox.fav.subjectFavorite", { name: f.driver_name })
        : i18n.t("inbox.fav.subjectContender", { name: f.driver_name }),
    body,
    actions: [],
  };
}

// Nível de fama (0–100) na mesma régua de 6 da ficha do piloto.
function famaLevel(v) {
  if (v <= 15) return i18n.t("inbox.fama.anonymous");
  if (v <= 30) return i18n.t("inbox.fama.discreet");
  if (v <= 50) return i18n.t("inbox.fama.known");
  if (v <= 70) return i18n.t("inbox.fama.strong");
  if (v <= 87) return i18n.t("inbox.fama.star");
  return i18n.t("inbox.fama.idol");
}

// "Times de olho em você" — interesse de equipes pela FAMA (Fase 2a do estrelato).
function teamInterestMessage(t) {
  if (!t || !Array.isArray(t.teams) || t.teams.length === 0) return null;
  const names = t.teams.map((x) => bold(x.team_name));
  const n = names.length;
  const list = n === 1 ? names[0] : `${names.slice(0, -1).join(", ")}${i18n.t("inbox.interest.and")}${names[n - 1]}`;
  const level = famaLevel(t.player_fama);

  const body =
    i18n.t("inbox.interest.p1", { count: n, list, level }) +
    i18n.t("inbox.interest.p2", { count: n });

  return {
    id: "interest",
    av: "g",
    ini: "◆",
    from: i18n.t("inbox.interest.from"),
    kind: n === 1 ? i18n.t("inbox.interest.kindOne") : i18n.t("inbox.interest.kindMany", { count: n }),
    time: i18n.t("inbox.interest.time"),
    subject:
      n === 1
        ? i18n.t("inbox.interest.subjectOne", { team: t.teams[0].team_name })
        : i18n.t("inbox.interest.subjectMany", { count: n }),
    body,
    actions: [],
  };
}

// Transforma os fatos do backend na lista de mensagens da caixa (na ordem de exibição).
export function buildInboxMessages(facts) {
  if (!facts) return [];
  return [
    teamInterestMessage(facts.team_interest),
    headToHeadMessage(facts.head_to_head),
    titleFavoriteMessage(facts.title_favorite),
  ].filter(Boolean);
}
