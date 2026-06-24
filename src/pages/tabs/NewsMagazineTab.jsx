import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import useCareerStore from "../../stores/useCareerStore";
import { categoryLabel } from "../../utils/formatters";
import { getTrackImageSrc } from "../../utils/trackImages";
import { getReadableTeamColor } from "./newsHelpers";

import "./NewsMagazineTab.css";

// ─────────────────────────────────────────────────────────────────────────────
// Construtores e edições agora são REAIS (vindos do backend). Pendentes:
//   • Texto/boletim da matéria → IA (Gemini) — por enquanto placeholder.
//   • Mensagens da caixa de entrada → mercado/empresário — mock abaixo.
// ─────────────────────────────────────────────────────────────────────────────

const MESSAGES = [
  {
    id: "vm",
    av: "g",
    ini: "VM",
    from: "Velocità Moderna",
    kind: "Interesse de equipe",
    time: "há 2 dias",
    subject: "Estamos de olho em você para 2030.",
    body: "<p>A diretoria da <b>Velocità Moderna</b> acompanhou sua evolução nas últimas etapas e quer abrir conversa para uma vaga em 2030. Nada oficial ainda — é um sinal de que você entrou no radar de uma equipe da ponta.</p>",
    actions: [
      { label: "Demonstrar interesse", type: "primary" },
      { label: "Agora não", type: "ghost" },
    ],
  },
  {
    id: "la",
    av: "y",
    ini: "LA",
    from: "Sua equipe",
    kind: "Expectativa · início de ano",
    time: "12 Mar",
    subject: "Meta da temporada: terminar no top 4.",
    body: "<p>No início da temporada a equipe definiu como meta terminar entre os <b>4 primeiros</b> do campeonato. Mantendo a média de pódios das últimas etapas, a meta é totalmente alcançável.</p>",
    actions: [],
  },
  {
    id: "cn",
    av: "p",
    ini: "GR",
    from: "Boletim do grid",
    kind: "Rival · já enfrentado",
    time: "há 5 dias",
    subject: "O nome a bater no campeonato.",
    body: "<p>O líder do campeonato é forte em classificação e gestão de pneu, mas vulnerável nas largadas — onde você já o superou. Fique de olho na briga direta nas próximas etapas.</p>",
    actions: [],
  },
];

// Logo da categoria (capa fechada quando a temporada ainda não teve corridas).
const CATEGORY_LOGOS = {
  mazda_rookie: "/utilities/categorias/MX5%20ROOKIE.png",
  toyota_rookie: "/utilities/categorias/GR%20ROOKIE.png",
  mazda_amador: "/utilities/categorias/MX5%20CUP.png",
  toyota_amador: "/utilities/categorias/GR%20CUP.png",
  bmw_m2: "/utilities/categorias/M2%20CUP.png",
  production_challenger: "/utilities/categorias/PRODUCTION.png",
  gt4: "/utilities/categorias/GT4.png",
  gt3: "/utilities/categorias/GT3.png",
  lmp2: "/utilities/categorias/LMP2.png",
  endurance: "/utilities/categorias/ENDURANCE.png",
};

// Colore os nomes de equipes citados no boletim de IA com a cor do time.
// `teams` é o mapa nome→cor (hex) das equipes da corrida.
function colorizeTeams(text, teams) {
  if (!teams || typeof teams !== "object") return text;
  const names = Object.keys(teams)
    .filter(Boolean)
    .sort((a, b) => b.length - a.length);
  if (!names.length) return text;
  const escaped = names.map((n) => n.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const re = new RegExp(`(${escaped.join("|")})`, "g");
  return text.split(re).map((part, i) =>
    teams[part] ? (
      <span key={i} style={{ color: getReadableTeamColor(teams[part]), fontWeight: 600 }}>
        {part}
      </span>
    ) : (
      part
    ),
  );
}

function NewsMagazineTab() {
  const careerId = useCareerStore((s) => s.careerId);
  const playerTeam = useCareerStore((s) => s.playerTeam);
  const season = useCareerStore((s) => s.season);

  const category = playerTeam?.categoria ?? null;
  const year = season?.ano ?? "";

  const [standings, setStandings] = useState([]);
  const [calendar, setCalendar] = useState([]);

  const [edIdx, setEdIdx] = useState(0);
  const [flipping, setFlipping] = useState(false);

  const [selectedId, setSelectedId] = useState(MESSAGES[0]?.id ?? null);
  const [readIds, setReadIds] = useState(() => new Set());

  const [bulletin, setBulletin] = useState(null);

  // ── Construtores reais ──
  useEffect(() => {
    let mounted = true;
    if (!careerId || !category) {
      setStandings([]);
      return undefined;
    }
    invoke("get_teams_standings", { careerId, category })
      .then((rows) => {
        if (mounted) setStandings(Array.isArray(rows) ? rows : []);
      })
      .catch(() => {
        if (mounted) setStandings([]);
      });
    return () => {
      mounted = false;
    };
  }, [careerId, category]);

  // ── Calendário real (para montar as edições das corridas disputadas) ──
  useEffect(() => {
    let mounted = true;
    if (!careerId || !category) {
      setCalendar([]);
      return undefined;
    }
    invoke("get_calendar_for_category", { careerId, category })
      .then((rows) => {
        if (mounted) setCalendar(Array.isArray(rows) ? rows : []);
      })
      .catch(() => {
        if (mounted) setCalendar([]);
      });
    return () => {
      mounted = false;
    };
  }, [careerId, category]);

  // Edições = corridas concluídas, da mais recente para a mais antiga.
  const editions = useMemo(() => {
    return calendar
      .filter((r) => r.status === "Concluida")
      .sort((a, b) => (b.rodada ?? 0) - (a.rodada ?? 0));
  }, [calendar]);

  // Próxima corrida (primeira pendente por rodada).
  const nextRace = useMemo(() => {
    const pending = calendar
      .filter((r) => r.status !== "Concluida")
      .sort((a, b) => (a.rodada ?? 0) - (b.rodada ?? 0));
    return pending[0] ?? null;
  }, [calendar]);

  const totalRounds = calendar.length;
  const safeIdx = Math.min(edIdx, Math.max(0, editions.length - 1));
  const ed = editions[safeIdx] ?? null;

  // Boletim de IA da edição atual (corrida do jogador): resolve o news_id da rodada
  // e pede o boletim (cacheado/prewarmed no fim da corrida). Sem boletim → placeholder.
  useEffect(() => {
    let active = true;
    const roundNo = ed?.rodada;
    const seasonId = season?.id;
    if (!careerId || roundNo == null || !seasonId) {
      setBulletin(null);
      return undefined;
    }
    setBulletin({ loading: true });
    invoke("player_race_news_id", { careerId, seasonId, rodada: roundNo })
      .then((newsId) => {
        if (!newsId) {
          if (active) setBulletin({ loading: false, story: null });
          return null;
        }
        return invoke("enrich_race_news_ai", {
          careerId,
          newsId,
          readingSeconds: null,
        }).then((res) => {
          if (active) {
            setBulletin({
              loading: false,
              story: res?.story ?? null,
              teams: res?.teams ?? null,
              status: res?.status ?? null,
            });
          }
        });
      })
      .catch(() => {
        if (active) setBulletin({ loading: false, story: null });
      });
    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [careerId, ed?.rodada, season?.id]);

  // Construtores: top 6, garantindo que a equipe do jogador apareça.
  const construtores = useMemo(() => {
    const mapped = standings.map((t) => ({
      pos: t.posicao,
      name: t.nome,
      pts: t.pontos,
      color: t.cor_primaria || "#888",
      me: playerTeam?.id != null && t.id === playerTeam.id,
    }));
    let top = mapped.slice(0, 6);
    if (!top.some((t) => t.me)) {
      const meTeam = mapped.find((t) => t.me);
      if (meTeam) top = [...mapped.slice(0, 5), meTeam];
    }
    return top;
  }, [standings, playerTeam]);

  const unread = MESSAGES.filter((m) => !readIds.has(m.id)).length;
  const selected = MESSAGES.find((m) => m.id === selectedId) ?? null;

  function goEdition(delta) {
    const next = safeIdx + delta;
    if (next < 0 || next >= editions.length || flipping) return;
    setFlipping(true);
    setTimeout(() => {
      setEdIdx(next);
      setFlipping(false);
    }, 200);
  }

  function selectMessage(id) {
    setSelectedId(id);
    setReadIds((prev) => {
      const nextSet = new Set(prev);
      nextSet.add(id);
      return nextSet;
    });
  }

  const catLabel = category ? categoryLabel(category) : "";
  const catLogo = category ? CATEGORY_LOGOS[category] ?? null : null;
  const kicker = ed
    ? `${catLabel} · Etapa ${ed.rodada} · ${ed.track_name}`
    : catLabel;
  const footMeta = ed
    ? `Etapa ${ed.rodada}${totalRounds ? ` de ${totalRounds}` : ""} · ${ed.display_date ?? ""}` +
      (nextRace ? ` · Próxima: ${nextRace.track_name}, ${nextRace.display_date ?? ""}` : "")
    : "";

  return (
    <div className="newsmag">
      {/* ═══════════ A REVISTA ═══════════ */}
      <article className={`mag${flipping ? " flipping" : ""}${ed ? "" : " mag--cover"}`}>
        {ed ? (
        <>
        <div className="spread">
          {/* PÁGINA ESQUERDA */}
          <div className="page page-l">
            <div className="flag" />
            <div className="kicker">{kicker}</div>
            <h1 className="display">
              <span className="l1">{ed ? ed.track_name : "Sem corridas"}</span>
              <span className="l2">
                {ed ? `Etapa ${ed.rodada} · Temporada ${year}` : "ainda nesta temporada"}
              </span>
            </h1>
            <span className="ai-tag">
              {bulletin?.loading
                ? "✦ Gerando boletim…"
                : bulletin?.story
                ? "✦ Boletim"
                : "✦ Boletim por IA · em breve"}
            </span>

            <div className="prose-cols">
              <h3 className="subhead">Boletim da corrida</h3>
              {bulletin?.story ? (
                bulletin.story
                  .split(/\n\s*\n/)
                  .filter(Boolean)
                  .map((para, i) => <p key={i}>{colorizeTeams(para, bulletin.teams)}</p>)
              ) : bulletin?.loading ? (
                <p>Gerando o boletim desta etapa…</p>
              ) : (
                <>
                  <p>
                    O relato completo desta etapa será gerado pela IA a partir do que aconteceu na pista — sua
                    largada, ultrapassagens, disputa pela ponta e o resultado final.
                  </p>
                  <p>
                    Por enquanto, acompanhe ao lado a{" "}
                    <span className="teamname">classificação de construtores</span> atualizada e, abaixo, as
                    mensagens diretas a você na caixa de entrada.
                  </p>
                </>
              )}
            </div>
          </div>

          {/* PÁGINA DIREITA */}
          <div className="page page-r">
            <div className="credits">
              Reportagem de <b>Diretoria de Imprensa</b>
              <br />
              {catLabel ? <>Temporada de <b>{catLabel}</b></> : null}
            </div>
            {ed ? (
              <img
                className="photo"
                src={getTrackImageSrc(ed.track_name, ed.track_id)}
                alt={ed.track_name}
              />
            ) : (
              <div className="photo" />
            )}
            <p className="cap">
              {ed
                ? `${ed.track_name}${ed.display_date ? ` — ${ed.display_date}` : ""}`
                : "As corridas que você disputar aparecem aqui como edições."}
            </p>

            <div className="r-grid r-grid-single">
              <div>
                <h3 className="subhead">Construtores · {year}</h3>
                {construtores.length > 0 ? (
                  construtores.map((c) => (
                    <div key={c.pos} className={c.me ? "res-row me" : "res-row"}>
                      <span className="rp">{c.pos}</span>
                      <span className="chip" style={{ background: c.color }} />
                      <span className="rn">{c.name}</span>
                      <span className="rpts">{c.pts}</span>
                    </div>
                  ))
                ) : (
                  <p>Classificação de equipes indisponível.</p>
                )}
              </div>
            </div>
          </div>
        </div>

        <div className="mag-foot">
          <div className="foot-left">
            <div className="brand">
              GRID<span>·</span>MAGAZINE
            </div>
            <div className="mag-nav">
              <button
                type="button"
                className="navbtn"
                onClick={() => goEdition(1)}
                disabled={safeIdx >= editions.length - 1}
                title="Edição anterior"
                aria-label="Edição anterior"
              >
                ‹
              </button>
              <button
                type="button"
                className="navbtn"
                onClick={() => goEdition(-1)}
                disabled={safeIdx <= 0}
                title="Próxima edição"
                aria-label="Próxima edição"
              >
                ›
              </button>
            </div>
          </div>
          <div className="foot-meta">{footMeta}</div>
        </div>
        </>
        ) : (
          <div className="mag-cover">
            <div className="mag-cover-frame">
              <img
                className="mag-cover-book"
                src="/utilities/news/magazine-cover.png"
                alt=""
                draggable={false}
              />
              {catLogo && (
                <img className="mag-cover-logo" src={catLogo} alt={catLabel} draggable={false} />
              )}
            </div>
            <p className="mag-cover-cap">
              {catLabel}
              {year ? ` · Temporada ${year}` : ""}
            </p>
            <p className="mag-cover-sub">
              A revista abre quando você disputar a primeira corrida da temporada.
            </p>
          </div>
        )}
      </article>

      {/* ═══════════ CAIXA DE E-MAIL ═══════════ */}
      <section className="mailbox">
        <div className="mb-head">
          <span className="mb-icon">✉</span>
          <span className="mb-title">Caixa de entrada</span>
          {unread > 0 && <span className="mb-count">{unread}</span>}
        </div>

        <div className="mb-split">
          <div className="mb-list">
            {MESSAGES.map((m) => {
              const classes = ["mrow"];
              if (readIds.has(m.id)) classes.push("read");
              if (m.id === selectedId) classes.push("active");
              return (
                <div
                  key={m.id}
                  className={classes.join(" ")}
                  onClick={() => selectMessage(m.id)}
                  role="button"
                  tabIndex={0}
                >
                  <span className={`mava ${m.av}`}>{m.ini}</span>
                  <div className="m-main">
                    <span className="mfrom">
                      {m.from}
                      <small>{m.kind}</small>
                    </span>
                  </div>
                  <span className="mright">
                    <span className="mtime">{m.time}</span>
                    <span className="ndot" />
                  </span>
                </div>
              );
            })}
          </div>

          {selected ? (
            <div className="mb-reader">
              <div className="reader-head">
                <span className={`mava ${selected.av}`}>{selected.ini}</span>
                <span className="reader-from">
                  {selected.from}
                  <small>{selected.kind}</small>
                </span>
                <span className="reader-time">{selected.time}</span>
              </div>
              <h3 className="reader-subject">{selected.subject}</h3>
              <div className="reader-body" dangerouslySetInnerHTML={{ __html: selected.body }} />
              {selected.actions.length > 0 && (
                <div className="reader-actions">
                  {selected.actions.map((a) => (
                    <button key={a.label} type="button" className={`mbtn ${a.type}`}>
                      {a.label}
                    </button>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <div className="mb-reader empty">
              <div>
                <div className="ph-ic">✉</div>
                Selecione uma mensagem para ler
              </div>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}

export default NewsMagazineTab;
