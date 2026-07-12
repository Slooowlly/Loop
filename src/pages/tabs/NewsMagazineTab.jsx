import { Fragment, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import TeamLogoMark from "../../components/team/TeamLogoMark";
import useCareerStore from "../../stores/useCareerStore";
import { categoryLabel } from "../../utils/formatters";
import { getTrackImageSrc } from "../../utils/trackImages";
import { buildDriverMentionMatcher, driverMentionClass } from "../../utils/driverMentions";
import { getTeamGlow } from "../../utils/teamColors";
import { getReadableTeamColor } from "./newsHelpers";
import { buildInboxMessages } from "./inboxMessages";

import "./NewsMagazineTab.css";

// ─────────────────────────────────────────────────────────────────────────────
// Construtores e edições agora são REAIS (vindos do backend). Pendentes:
//   • Texto/boletim da matéria → IA (Gemini) — por enquanto placeholder.
//   • Mensagens da caixa de entrada → mercado/empresário — mock abaixo.
// ─────────────────────────────────────────────────────────────────────────────

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

// Renderiza um parágrafo do boletim combinando duas camadas: nomes de PILOTO viram
// spans interativos (hover acende o piloto/equipe na classificação ao lado) e o
// restante passa por `colorizeTeams` (nomes de EQUIPE na cor do time).
function renderBulletinParagraph(text, mentionDrivers, teams, hoveredDriverId, onHover) {
  const matcher = buildDriverMentionMatcher(mentionDrivers);
  if (!matcher) {
    return colorizeTeams(text, teams);
  }
  return text.split(matcher.regex).map((part, i) => {
    const driverId = matcher.byName.get(part);
    if (driverId) {
      const isActive = hoveredDriverId === driverId;
      return (
        <span
          key={i}
          onMouseEnter={() => onHover(driverId)}
          onMouseLeave={() => onHover(null)}
          className={driverMentionClass(isActive, "text-[#58a6ff]", "text-white hover:text-[#58a6ff]")}
        >
          {part}
        </span>
      );
    }
    return <Fragment key={i}>{colorizeTeams(part, teams)}</Fragment>;
  });
}

function NewsMagazineTab() {
  const careerId = useCareerStore((s) => s.careerId);
  const playerTeam = useCareerStore((s) => s.playerTeam);
  const season = useCareerStore((s) => s.season);

  const category = playerTeam?.categoria ?? null;
  const year = season?.ano ?? "";

  const [standings, setStandings] = useState([]);
  const [driverStandings, setDriverStandings] = useState([]);
  const [calendar, setCalendar] = useState([]);
  // Classificação ao lado do boletim: alterna entre pilotos (padrão) e equipes.
  const [standingsView, setStandingsView] = useState("pilotos");
  // Piloto realçado ao passar o mouse no nome dele no boletim → acende a equipe dele
  // (view construtores) ou ele mesmo (view pilotos) na classificação.
  const [hoveredDriverId, setHoveredDriverId] = useState(null);

  const [edIdx, setEdIdx] = useState(0);
  const [flipping, setFlipping] = useState(false);

  const [messages, setMessages] = useState([]);
  const [selectedId, setSelectedId] = useState(null);
  const [readIds, setReadIds] = useState(() => new Set());

  // ── Caixa de entrada real: fatos do save (confronto direto + favorito) → texto PT ──
  useEffect(() => {
    let mounted = true;
    if (!careerId) {
      setMessages([]);
      return undefined;
    }
    invoke("get_inbox_messages", { careerId })
      .then((facts) => {
        if (!mounted) return;
        const list = buildInboxMessages(facts);
        setMessages(list);
        setSelectedId((cur) => cur ?? list[0]?.id ?? null);
      })
      .catch(() => {
        if (mounted) setMessages([]);
      });
    return () => {
      mounted = false;
    };
  }, [careerId]);

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

  // ── Pilotos reais (para a tabela alternativa e para realçar nomes no boletim) ──
  useEffect(() => {
    let mounted = true;
    if (!careerId || !category) {
      setDriverStandings([]);
      return undefined;
    }
    invoke("get_drivers_by_category", { careerId, category })
      .then((rows) => {
        if (mounted) setDriverStandings(Array.isArray(rows) ? rows : []);
      })
      .catch(() => {
        if (mounted) setDriverStandings([]);
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
      id: t.id,
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

  // Pilotos: grid completo da categoria, por posição no campeonato.
  const pilotos = useMemo(() => {
    return [...driverStandings]
      .sort((a, b) => (a.posicao_campeonato ?? 999) - (b.posicao_campeonato ?? 999))
      .map((d) => ({
        id: d.id,
        pos: d.posicao_campeonato,
        name: d.nome,
        pts: d.pontos,
        teamName: d.equipe_nome,
        color: d.equipe_cor || "#888",
        me: d.is_jogador,
      }));
  }, [driverStandings]);

  // Nomes que o boletim pode mencionar + resolução do piloto realçado → equipe dele.
  const mentionDrivers = useMemo(
    () => driverStandings.map((d) => ({ id: d.id, nome: d.nome })),
    [driverStandings],
  );
  const hoveredTeamId = useMemo(
    () => driverStandings.find((d) => d.id === hoveredDriverId)?.equipe_id ?? null,
    [driverStandings, hoveredDriverId],
  );

  const unread = messages.filter((m) => !readIds.has(m.id)).length;
  const selected = messages.find((m) => m.id === selectedId) ?? null;

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
  const kicker = ed ? `${catLabel} · ${ed.track_name}` : catLabel;
  const footMeta = ed
    ? `Edição ${ed.rodada}${totalRounds ? ` de ${totalRounds}` : ""}${year ? ` · Temporada ${year}` : ""}`
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
                  .map((para, i) => (
                    <p key={i}>
                      {renderBulletinParagraph(
                        para,
                        mentionDrivers,
                        bulletin.teams,
                        hoveredDriverId,
                        setHoveredDriverId,
                      )}
                    </p>
                  ))
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
                <div className="nm-standings-head">
                  <h3 className="subhead">
                    {standingsView === "construtores" ? "Construtores" : "Pilotos"} · {year}
                  </h3>
                  <div className="nm-toggle">
                    <button
                      type="button"
                      className={`nm-toggle-btn${standingsView === "pilotos" ? " active" : ""}`}
                      onClick={() => setStandingsView("pilotos")}
                    >
                      Pilotos
                    </button>
                    <button
                      type="button"
                      className={`nm-toggle-btn${standingsView === "construtores" ? " active" : ""}`}
                      onClick={() => setStandingsView("construtores")}
                    >
                      Construtores
                    </button>
                  </div>
                </div>

                {standingsView === "construtores" ? (
                  construtores.length > 0 ? (
                    construtores.map((c) => {
                      const glow = hoveredTeamId != null && c.id === hoveredTeamId;
                      const tone = glow ? getTeamGlow(c.color) : null;
                      return (
                        <div
                          key={c.id ?? c.pos}
                          className={c.me ? "res-row me" : "res-row"}
                          style={
                            tone
                              ? { background: tone.soft, boxShadow: `inset 0 0 0 1.5px ${tone.solid}` }
                              : undefined
                          }
                        >
                          <span className="rp">{c.pos}</span>
                          <TeamLogoMark teamName={c.name} color={c.color} size="xs" testId="news-team-logo" />
                          <span className="rn">{c.name}</span>
                          <span className="rpts">{c.pts}</span>
                        </div>
                      );
                    })
                  ) : (
                    <p>Classificação de equipes indisponível.</p>
                  )
                ) : pilotos.length > 0 ? (
                  <div className={`res-list${pilotos.length > 12 ? " res-list--split" : ""}`}>
                    {pilotos.map((p) => {
                      const glow = hoveredDriverId != null && p.id === hoveredDriverId;
                      const tone = glow ? getTeamGlow(p.color) : null;
                      return (
                        <div
                          key={p.id ?? p.pos}
                          className={p.me ? "res-row me" : "res-row"}
                          style={
                            tone
                              ? { background: tone.soft, boxShadow: `inset 0 0 0 1.5px ${tone.solid}` }
                              : undefined
                          }
                          onMouseEnter={() => setHoveredDriverId(p.id)}
                          onMouseLeave={() => setHoveredDriverId(null)}
                        >
                          <span className="rp">{p.pos}</span>
                          <TeamLogoMark teamName={p.teamName} color={p.color} size="xs" testId="news-driver-team-logo" />
                          <span className="rn">{p.name}</span>
                          <span className="rpts">{p.pts}</span>
                        </div>
                      );
                    })}
                  </div>
                ) : (
                  <p>Classificação de pilotos indisponível.</p>
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
              <span className="mag-cover-title">{catLabel}</span>
            </div>
            <div className="mag-cover-side">
              {year ? <p className="mag-cover-cap">Temporada {year}</p> : null}
              <p className="mag-cover-sub">
                A revista abre quando você disputar a primeira corrida da temporada.
              </p>
            </div>
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
            {messages.map((m) => {
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
