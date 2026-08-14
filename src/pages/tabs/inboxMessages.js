// Voz da caixa de entrada. Recebe os FATOS reais do backend
// (`get_inbox_messages`) e monta as mensagens no formato que a casca da caixa já
// consome. Texto de template (i18n) — nada de IA. Read-only.
//
// O corpo de cada mensagem é uma lista de PARÁGRAFOS, e cada parágrafo é uma lista
// de trechos tipados: `{ tipo: "texto" | "forte", texto }`. Não existe HTML aqui.
// A razão é direta: nome de piloto, de equipe e de pista vem do banco, e o banco
// guarda o que o jogador digitou na criação da carreira. Enquanto o corpo era uma
// string de HTML concatenado, um nome com marcação virava marcação de verdade na
// tela. Agora o negrito é decidido por nós, no texto do locale, e o dado do banco
// entra sempre como trecho de texto — a caixa desenha cada trecho como filho do
// React, que escapa qualquer coisa sozinho.

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

const texto = (valor) => ({ tipo: "texto", texto: String(valor) });
const forte = (valor) => ({ tipo: "forte", texto: String(valor) });

// Marca de posição dos valores do banco dentro do texto do locale. O i18next
// interpola a MARCA no lugar do valor; o divisor troca a marca pelos trechos já
// prontos. O dado do banco nunca chega a passar pelo divisor, e por isso não tem
// como forjar uma marca: quando ele aparece, o divisor já terminou.
const marca = (nome) => `[[${nome}]]`;
const DIVISOR = /<b>([\s\S]*?)<\/b>|\[\[([a-zA-Z]+)\]\]/g;

// Quebra o texto do locale em trechos. Só o `<b>` do NOSSO texto vira negrito;
// tudo o mais segue como texto puro, inclusive `<b>`, `<script>` ou `<img>` que
// venham dentro de um valor do banco, porque esses entram por `campos` e não
// chegam a ser lidos aqui.
function dividir(modelo, campos) {
  const trechos = [];
  let cursor = 0;
  DIVISOR.lastIndex = 0;
  for (let achado = DIVISOR.exec(modelo); achado; achado = DIVISOR.exec(modelo)) {
    if (achado.index > cursor) trechos.push(texto(modelo.slice(cursor, achado.index)));
    if (achado[1] !== undefined) trechos.push(forte(achado[1]));
    else trechos.push(...(campos[achado[2]] ?? []));
    cursor = achado.index + achado[0].length;
  }
  if (cursor < modelo.length) trechos.push(texto(modelo.slice(cursor)));
  return trechos;
}

// Traduz `chave` e devolve os trechos. `valores` são interpolações do nosso próprio
// texto (números que formatamos, cláusulas vindas de outra chave); `campos` são os
// valores do banco, que entram já como lista de trechos.
function frase(chave, valores = {}, campos = {}) {
  const marcas = {};
  for (const nome of Object.keys(campos)) marcas[nome] = marca(nome);
  return dividir(i18n.t(chave, { ...valores, ...marcas }), campos);
}

// Nome do piloto em negrito, com a equipe entre parênteses quando existe.
function quem(nome, equipe) {
  return equipe ? [forte(nome), texto(` (${equipe})`)] : [forte(nome)];
}

function attrClause(key, kind) {
  if (!KNOWN_ATTRS.has(key)) return null;
  return i18n.t(`inbox.attr.${key}.${kind}`);
}

// "Já cruzei com esse cara" — confronto direto com o rival mais enfrentado.
function headToHeadMessage(h) {
  if (!h || h.races_together <= 0) return null;
  const n = h.races_together;
  const paragrafo = frase("inbox.h2h.intro", { count: n }, { who: quem(h.rival_name, h.rival_team) });

  if (h.player_ahead <= 0) {
    paragrafo.push(...frase("inbox.h2h.lostAll"));
  } else if (h.player_ahead >= n) {
    paragrafo.push(...frase("inbox.h2h.wonAll"));
  } else {
    paragrafo.push(...frase("inbox.h2h.wonSome", { n: h.player_ahead }));
    if (h.best_finish && h.best_track) {
      paragrafo.push(
        ...frase(
          "inbox.h2h.wonSomeBest",
          { ordinal: ordinal(h.best_finish) },
          { track: [texto(h.best_track)] },
        ),
      );
    } else {
      paragrafo.push(...frase("inbox.h2h.wonSomeGeneric"));
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
    paragrafos: [paragrafo],
    actions: [],
  };
}

// "O favorito ao título" — o nome a bater na temporada.
function titleFavoriteMessage(f) {
  if (!f) return null;

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

  const paragrafo = frase(
    "inbox.fav.body",
    { profile, standing, traits },
    { who: quem(f.driver_name, f.driver_team) },
  );

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
    paragrafos: [paragrafo],
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
  const n = t.teams.length;

  // A lista de equipes: "Alpha, Beta e Gama", cada nome em negrito e os separadores
  // por nossa conta.
  const list = [];
  t.teams.forEach((equipe, i) => {
    if (i > 0) list.push(texto(i === n - 1 ? i18n.t("inbox.interest.and") : ", "));
    list.push(forte(equipe.team_name));
  });

  const level = famaLevel(t.player_fama);

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
    paragrafos: [
      frase("inbox.interest.p1", { count: n, level }, { list }),
      frase("inbox.interest.p2", { count: n }),
    ],
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
