// Voz (PT) da caixa de entrada. Recebe os FATOS reais do backend
// (`get_inbox_messages`) e monta as mensagens no formato que a casca da caixa já
// consome. Texto de template — nada de IA. Read-only.

const ATTR_PHRASE = {
  ritmo_classificacao: { f: "afiado na classificação", w: "vacila na classificação" },
  gestao_pneus: { f: "economiza muito bem os pneus", w: "sofre com a gestão de pneus" },
  racecraft: { f: "impecável no corpo a corpo", w: "frágil no corpo a corpo" },
  defesa: { f: "difícil de ultrapassar", w: "vulnerável quando atacado" },
  habilidade_largada: { f: "voa nas largadas", w: "perde posições na largada" },
  consistencia: { f: "muito consistente", w: "oscila demais de corrida a corrida" },
};

const ord = (n) => `${n}º`;
const plural = (n, one, many) => (n === 1 ? one : many);
const bold = (s) => `<b>${s}</b>`;

function attrClause(key, kind) {
  const e = ATTR_PHRASE[key];
  if (!e) return null;
  return kind === "f" ? e.f : e.w;
}

// "Já cruzei com esse cara" — confronto direto com o rival mais enfrentado.
function headToHeadMessage(h) {
  if (!h || h.races_together <= 0) return null;
  const who = h.rival_team ? `${bold(h.rival_name)} (${h.rival_team})` : bold(h.rival_name);
  const n = h.races_together;
  let body = `<p>Você e ${who} já largaram juntos ${n} ${plural(n, "vez", "vezes")} nesta categoria.`;

  if (h.player_ahead <= 0) {
    body += ` Ele levou a melhor em todas até aqui — está mais do que na hora de virar esse jogo.</p>`;
  } else if (h.player_ahead >= n) {
    body += ` Você terminou na frente em <b>todas</b> elas — uma vantagem psicológica que pesa no grid.</p>`;
  } else {
    body += ` Você terminou na frente em ${bold(h.player_ahead)}`;
    if (h.best_finish && h.best_track) {
      body += `, incluindo seu ${ord(h.best_finish)} em ${h.best_track}.</p>`;
    } else {
      body += ` desses duelos.</p>`;
    }
  }

  return {
    id: "h2h",
    av: "p",
    ini: "GR",
    from: "Boletim do grid",
    kind: "Rival · já enfrentado",
    time: "próxima prova",
    subject: `Você e ${h.rival_name} voltam a se cruzar.`,
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
    profile = `veterano com ${f.career_titles} ${plural(f.career_titles, "título", "títulos")} na carreira`;
  } else if (f.veteran) {
    profile = `um rodado que conhece a categoria como poucos`;
  } else {
    profile = `jovem em franca ascensão`;
  }

  // Situação na tabela.
  let standing;
  if (f.position === 0) {
    standing = `é apontado como favorito ao título desta temporada`;
  } else if (f.position === 1) {
    standing =
      f.points_lead > 0
        ? `lidera o campeonato por ${f.points_lead} ${plural(f.points_lead, "ponto", "pontos")}`
        : `está na ponta do campeonato`;
  } else if (f.leads_player) {
    standing = `aparece logo à sua frente, em ${ord(f.position)}`;
  } else {
    standing = `vem logo atrás de você, em ${ord(f.position)} — seu principal perseguidor`;
  }

  const strong = attrClause(f.strong_attr, "f");
  const weak = attrClause(f.weak_attr, "w");
  const traits = strong && weak ? ` ${strong[0].toUpperCase()}${strong.slice(1)}, mas ${weak}.` : "";

  const body = `<p>${who} é o nome a bater: ${profile}, ${standing}.${traits}</p>`;

  return {
    id: "fav",
    av: "g",
    ini: "★",
    from: "Prévia do campeonato",
    kind: f.position === 0 ? "Favorito ao título" : "Expectativa da temporada",
    time: "temporada",
    subject:
      f.position === 0
        ? `${f.driver_name}: o favorito ao título.`
        : `${f.driver_name} é o nome a bater.`,
    body,
    actions: [],
  };
}

// Transforma os fatos do backend na lista de mensagens da caixa (na ordem de exibição).
export function buildInboxMessages(facts) {
  if (!facts) return [];
  return [headToHeadMessage(facts.head_to_head), titleFavoriteMessage(facts.title_favorite)].filter(
    Boolean,
  );
}
