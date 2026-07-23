import { Fragment, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import TeamLogoMark from "../../components/team/TeamLogoMark";
import RivalMarker from "../../components/driver/RivalMarker";
import useCareerStore from "../../stores/useCareerStore";
import { categoryLabel } from "../../utils/formatters";
import { getTrackImageSrc } from "../../utils/trackImages";
import { driverMentionClass, segmentDriverMentions } from "../../utils/driverMentions";
import { getTeamGlow } from "../../utils/teamColors";
import { getReadableTeamColor } from "./newsHelpers";
import { buildInboxMessages } from "./inboxMessages";
import { isPortuguese, localizedAiError } from "../../utils/aiFallback";

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
  const segments = segmentDriverMentions(text, mentionDrivers);
  if (!segments.length) {
    return colorizeTeams(text, teams);
  }
  return segments.map((seg, i) => {
    if (seg.type === "driver") {
      const isActive = hoveredDriverId === seg.id;
      return (
        <span
          key={i}
          onMouseEnter={() => onHover(seg.id)}
          onMouseLeave={() => onHover(null)}
          className={driverMentionClass(isActive, "text-[#58a6ff]", "text-white hover:text-[#58a6ff]")}
        >
          {seg.text}
          <RivalMarker driverId={seg.id} className="ml-0.5 align-middle" />
        </span>
      );
    }
    return <Fragment key={i}>{colorizeTeams(seg.text, teams)}</Fragment>;
  });
}

function NewsMagazineTab() {
  const { t } = useTranslation();
  const careerId = useCareerStore((s) => s.careerId);
  const playerTeam = useCareerStore((s) => s.playerTeam);
  const season = useCareerStore((s) => s.season);
  const language = useCareerStore((s) => s.language);

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
  // "O Que Esperar": matéria de PRÉ-TEMPORADA. Só existe antes da 1ª corrida —
  // substitui o "livro fechado" por um spread aberto. Ver docs/season-preview-design.md.
  const [preview, setPreview] = useState(null);

  // ── Rodapé "notícias do mundo": notinhas sobre ex-equipes e ex-companheiros do
  // jogador, só da categoria atual (crise, dívida, clima pesado, nova diretoria).
  // Determinístico hoje; migra pra IA quando o endpoint /world-notes existir.
  const [worldNotes, setWorldNotes] = useState([]);
  // As notas vieram da IA (idioma certo) ou do template determinístico (PT)?
  useEffect(() => {
    let mounted = true;
    if (!careerId) {
      setWorldNotes([]);
      return undefined;
    }
    // Mostra o texto determinístico na hora; a IA (se disponível no servidor) troca
    // as notas depois, sem bloquear a abertura da revista. Em qualquer falha —
    // inclusive o endpoint /world-notes ainda não existir — mantém o template.
    invoke("get_world_footer", { careerId })
      .then((res) => {
        if (!mounted) return;
        setWorldNotes(Array.isArray(res?.notes) ? res.notes : []);
        invoke("enrich_world_footer_ai", { careerId })
          .then((ai) => {
            if (mounted && ai?.source === "ai" && Array.isArray(ai?.notes)) {
              setWorldNotes(ai.notes);
            }
          })
          .catch(() => {});
      })
      .catch(() => {
        if (mounted) setWorldNotes([]);
      });
    return () => {
      mounted = false;
    };
  }, [careerId]);

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

  // Pré-temporada: só há matéria de expectativas enquanto NENHUMA corrida foi disputada.
  // Assim que a 1ª etapa conclui, `editions` deixa de ser vazio e este bloco some.
  const isPreseason = editions.length === 0;

  useEffect(() => {
    let active = true;
    if (!careerId || !category || !season?.id || !isPreseason) {
      setPreview(null);
      return undefined;
    }
    setPreview({ loading: true });
    invoke("enrich_season_preview_ai", { careerId })
      .then((res) => {
        if (!active) return;
        setPreview({
          loading: false,
          headline: res?.headline ?? null,
          standfirst: res?.standfirst ?? null,
          body: res?.body ?? null,
          teams: res?.teams ?? null,
          source: res?.source ?? null,
        });
      })
      .catch(() => {
        if (active) setPreview({ loading: false, body: null });
      });
    return () => {
      active = false;
    };
  }, [careerId, category, season?.id, isPreseason]);

  // Pista de abertura (menor rodada do calendário) — foto e legenda do spread.
  const openingRace = useMemo(() => {
    if (!calendar.length) return null;
    return [...calendar].sort((a, b) => (a.rodada ?? 0) - (b.rodada ?? 0))[0] ?? null;
  }, [calendar]);

  // Coluna direita da pré-temporada: o grid por POTENCIAL (o campeonato ainda está
  // zerado). Aqui número é permitido — é data-viz, não a prosa da matéria.
  const favorites = useMemo(() => {
    return [...driverStandings]
      .sort((a, b) => (b.skill ?? 0) - (a.skill ?? 0))
      .slice(0, 12)
      .map((d, i) => ({
        id: d.id,
        pos: i + 1,
        name: d.nome,
        skill: Math.round(d.skill ?? 0),
        teamName: d.equipe_nome,
        color: d.equipe_cor || "#888",
        me: d.is_jogador,
      }));
  }, [driverStandings]);

  // Construtores: grid completo da categoria, por posição no campeonato.
  const construtores = useMemo(() => {
    return standings.map((t) => ({
      id: t.id,
      pos: t.posicao,
      name: t.nome,
      pts: t.pontos,
      color: t.cor_primaria || "#888",
      me: playerTeam?.id != null && t.id === playerTeam.id,
    }));
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

  // Auto-ajusta o título da capa: teto de 57px (calibrado p/ todas as
  // categorias reais), encolhendo só se algum nome futuro estourar a coluna.
  const COVER_TITLE_BASE_PX = 57;
  const coverTitleRef = useRef(null);
  const coverBookRef = useRef(null);
  useLayoutEffect(() => {
    if (ed) return; // só na capa (sem edição aberta)
    const el = coverTitleRef.current;
    if (!el) return;
    const fit = () => {
      // Enquanto o livro (imagem) não carrega, a coluna tem largura ~0 e a
      // medição sairia errada (título minúsculo). Deixa no 57px do CSS e
      // espera o ResizeObserver disparar quando a imagem definir a largura.
      if (el.clientWidth < 2) return;
      let size = COVER_TITLE_BASE_PX;
      el.style.fontSize = `${size}px`;
      while (el.scrollWidth > el.clientWidth + 1 && size > 12) {
        size -= 2;
        el.style.fontSize = `${size}px`;
      }
    };
    fit();
    // Re-mede quando a imagem do livro e a fonte web terminam de carregar
    // (é aí que a largura da coluna passa a existir de verdade).
    let ro;
    if (typeof ResizeObserver !== "undefined" && coverBookRef.current) {
      ro = new ResizeObserver(fit);
      ro.observe(coverBookRef.current);
    }
    if (document?.fonts?.ready) document.fonts.ready.then(fit).catch(() => {});
    window.addEventListener("resize", fit);
    return () => {
      window.removeEventListener("resize", fit);
      if (ro) ro.disconnect();
    };
  }, [ed, catLabel]);
  const footMeta = ed
    ? `${t("newsMagazine.foot.edition", { round: ed.rodada })}${totalRounds ? t("newsMagazine.foot.ofTotal", { total: totalRounds }) : ""}${year ? t("newsMagazine.foot.seasonSuffix", { year }) : ""}`
    : "";

  // Antes da 1ª corrida a revista abre com "O Que Esperar" (spread aberto, não o
  // "livro fechado"). Só recai no livro fechado se nem categoria houver (edge).
  const showPreseason = isPreseason && !!category;

  return (
    <div className="newsmag">
      {/* ═══════════ A REVISTA ═══════════ */}
      <article className={`mag${flipping ? " flipping" : ""}${ed || showPreseason ? "" : " mag--cover"}`}>
        {ed ? (
        <>
        <div className="spread">
          {/* PÁGINA ESQUERDA */}
          <div className="page page-l">
            <div className="flag" />
            <div className="kicker">{kicker}</div>
            <h1 className="display">
              <span className="l1">{ed ? ed.track_name : t("newsMagazine.page.noRaces")}</span>
              <span className="l2">
                {ed
                  ? t("newsMagazine.page.stageSeasonLine", { round: ed.rodada, year })
                  : t("newsMagazine.page.stillThisSeason")}
              </span>
            </h1>
            <span className="ai-tag">
              {bulletin?.loading
                ? t("newsMagazine.aiTag.generating")
                : bulletin?.story
                ? t("newsMagazine.aiTag.bulletin")
                : t("newsMagazine.aiTag.comingSoon")}
            </span>

            <div className="prose-cols">
              <h3 className="subhead">{t("newsMagazine.page.bulletinHead")}</h3>
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
                <p>{t("newsMagazine.page.generatingBulletin")}</p>
              ) : !isPortuguese(language) ? (
                <p style={{ fontStyle: "italic", opacity: 0.6 }}>{localizedAiError(language)}</p>
              ) : (
                <>
                  <p>
                    O relato completo desta etapa será gerado pela IA a partir do que aconteceu na pista — sua
                    largada, ultrapassagens, disputa pela ponta e o resultado final.
                  </p>
                  <p>
                    Por enquanto, acompanhe ao lado a{" "}
                    {/* i18n-ignore — placeholder PT-only do gate isPortuguese: só renderiza quando o idioma É português */}
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
              {t("newsMagazine.credits.reportedBy")} <b>{t("newsMagazine.credits.pressOffice")}</b>
              <br />
              {catLabel ? <>{t("newsMagazine.credits.seasonOf")} <b>{catLabel}</b></> : null}
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
                : t("newsMagazine.caption.racesAppearHere")}
            </p>

            <div className="r-grid r-grid-single">
              <div>
                <div className="nm-standings-head">
                  <h3 className="subhead">
                    {standingsView === "construtores"
                      ? t("newsMagazine.standings.constructors")
                      : t("newsMagazine.standings.drivers")}{" "}
                    · {year}
                  </h3>
                  <div className="nm-toggle">
                    <button
                      type="button"
                      className={`nm-toggle-btn${standingsView === "pilotos" ? " active" : ""}`}
                      onClick={() => setStandingsView("pilotos")}
                    >
                      {t("newsMagazine.standings.drivers")}
                    </button>
                    <button
                      type="button"
                      className={`nm-toggle-btn${standingsView === "construtores" ? " active" : ""}`}
                      onClick={() => setStandingsView("construtores")}
                    >
                      {t("newsMagazine.standings.constructors")}
                    </button>
                  </div>
                </div>

                {standingsView === "construtores" ? (
                  construtores.length > 0 ? (
                    <div className={`res-list${construtores.length > 12 ? " res-list--split" : ""}`}>
                      {construtores.map((c) => {
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
                      })}
                    </div>
                  ) : (
                    <p>{t("newsMagazine.standings.teamsUnavailable")}</p>
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
                  <p>{t("newsMagazine.standings.driversUnavailable")}</p>
                )}
              </div>
            </div>
          </div>
        </div>

        {worldNotes.length > 0 && (
          <div className="world-notes" aria-label={t("newsMagazine.world.ariaLabel")}>
            <div className="wn-head">{t("newsMagazine.world.heading")}</div>
            <div className="wn-list">
              {/* O backend já emite estas notas no idioma ativo (rust-i18n), então o
                  template determinístico aparece direto; a IA só reescreve por cima. */}
              {worldNotes.map((n) => (
                <div key={n.id} className={`wn-item wn-${n.tone || "neutro"}`}>
                  <span className="wn-tag">{n.tag}</span>
                  <span className="wn-text">{n.text}</span>
                </div>
              ))}
            </div>
          </div>
        )}

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
                title={t("newsMagazine.foot.prevEdition")}
                aria-label={t("newsMagazine.foot.prevEdition")}
              >
                ‹
              </button>
              <button
                type="button"
                className="navbtn"
                onClick={() => goEdition(-1)}
                disabled={safeIdx <= 0}
                title={t("newsMagazine.foot.nextEdition")}
                aria-label={t("newsMagazine.foot.nextEdition")}
              >
                ›
              </button>
            </div>
          </div>
          <div className="foot-meta">{footMeta}</div>
        </div>
        </>
        ) : showPreseason ? (
        <>
        <div className="spread">
          {/* PÁGINA ESQUERDA — a matéria "O Que Esperar" */}
          <div className="page page-l">
            <div className="flag" />
            <div className="kicker">
              {catLabel} · {t("newsMagazine.preseason.kicker")}
            </div>
            <h1 className="display">
              <span className="l1">
                {preview?.headline || t("newsMagazine.preseason.titleLine")}
              </span>
              <span className="l2">
                {preview?.standfirst || t("newsMagazine.preseason.seasonLine", { year })}
              </span>
            </h1>
            <span className="ai-tag">
              {preview?.loading
                ? t("newsMagazine.preseason.aiTag.generating")
                : preview?.body
                ? t("newsMagazine.preseason.aiTag.ready")
                : t("newsMagazine.preseason.aiTag.comingSoon")}
            </span>

            <div className="prose-cols">
              <h3 className="subhead">{t("newsMagazine.preseason.articleHead")}</h3>
              {preview?.body ? (
                preview.body
                  .split(/\n\s*\n/)
                  .filter(Boolean)
                  .map((para, i) => (
                    <p key={i}>
                      {renderBulletinParagraph(
                        para,
                        mentionDrivers,
                        preview.teams,
                        hoveredDriverId,
                        setHoveredDriverId,
                      )}
                    </p>
                  ))
              ) : preview?.loading ? (
                <p>{t("newsMagazine.preseason.generating")}</p>
              ) : (
                <p>{t("newsMagazine.preseason.placeholder")}</p>
              )}
            </div>
          </div>

          {/* PÁGINA DIREITA — pista de abertura + grid por potencial */}
          <div className="page page-r">
            <div className="credits">
              {t("newsMagazine.credits.reportedBy")} <b>{t("newsMagazine.credits.pressOffice")}</b>
              <br />
              {catLabel ? <>{t("newsMagazine.credits.seasonOf")} <b>{catLabel}</b></> : null}
            </div>
            {openingRace ? (
              <img
                className="photo"
                src={getTrackImageSrc(openingRace.track_name, openingRace.track_id)}
                alt={openingRace.track_name}
              />
            ) : (
              <div className="photo" />
            )}
            <p className="cap">
              {openingRace
                ? t("newsMagazine.preseason.caption", { track: openingRace.track_name })
                : t("newsMagazine.preseason.captionGeneric")}
            </p>

            <div className="r-grid r-grid-single">
              <div>
                <div className="nm-standings-head">
                  <h3 className="subhead">
                    {t("newsMagazine.preseason.favoritesHead")}
                    {year ? <> · {year}</> : null}
                  </h3>
                </div>
                {favorites.length > 0 ? (
                  <div className={`res-list${favorites.length > 12 ? " res-list--split" : ""}`}>
                    {favorites.map((p) => {
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
                          <TeamLogoMark teamName={p.teamName} color={p.color} size="xs" testId="news-favorite-team-logo" />
                          <span className="rn">{p.name}</span>
                          <span className="rpts">{p.skill}</span>
                        </div>
                      );
                    })}
                  </div>
                ) : (
                  <p>{t("newsMagazine.standings.driversUnavailable")}</p>
                )}
              </div>
            </div>
          </div>
        </div>

        {worldNotes.length > 0 && (
          <div className="world-notes" aria-label={t("newsMagazine.world.ariaLabel")}>
            <div className="wn-head">{t("newsMagazine.world.heading")}</div>
            <div className="wn-list">
              {worldNotes.map((n) => (
                <div key={n.id} className={`wn-item wn-${n.tone || "neutro"}`}>
                  <span className="wn-tag">{n.tag}</span>
                  <span className="wn-text">{n.text}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        <div className="mag-foot">
          <div className="foot-left">
            <div className="brand">
              GRID<span>·</span>MAGAZINE
            </div>
          </div>
          <div className="foot-meta">
            {year
              ? t("newsMagazine.preseason.footSeason", { year })
              : t("newsMagazine.preseason.foot")}
          </div>
        </div>
        </>
        ) : (
          <div className="mag-cover">
            <div className="mag-cover-frame">
              <img
                className="mag-cover-book"
                ref={coverBookRef}
                src="/utilities/news/magazine-cover.webp"
                alt=""
                draggable={false}
              />
              <span className="mag-cover-title" ref={coverTitleRef}>
                {catLabel.split(/\s+/).map((word, i) => (
                  <span className="mag-cover-word" key={i}>
                    {word}
                  </span>
                ))}
              </span>
            </div>
            <div className="mag-cover-side">
              {year ? (
                <p className="mag-cover-cap">{t("newsMagazine.cover.seasonYear", { year })}</p>
              ) : null}
              <p className="mag-cover-sub">{t("newsMagazine.cover.sub")}</p>
            </div>
          </div>
        )}
      </article>

      {/* ═══════════ CAIXA DE E-MAIL ═══════════ */}
      <section className="mailbox">
        <div className="mb-head">
          <span className="mb-icon">✉</span>
          <span className="mb-title">{t("newsMagazine.mailbox.inbox")}</span>
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
                {t("newsMagazine.mailbox.selectPrompt")}
              </div>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}

export default NewsMagazineTab;
