import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import useCareerStore from "../../stores/useCareerStore";
import FlagIcon from "../ui/FlagIcon";
import Tooltip from "../ui/Tooltip";
import TeamLogoMark from "../team/TeamLogoMark";
import RivalMarker from "../driver/RivalMarker";
import RaceTelemetryCockpit from "./RaceTelemetryCockpit";
import { buildMockBreakdowns, MOCK_TELEMETRY } from "./__mockTelemetry";
import {
  driverMentionClass,
  renderTextWithDriverMentions,
  segmentDriverMentions,
} from "../../utils/driverMentions";
import { capitalizar } from "../../utils/formatters";
import { currentLang } from "../../i18n/format.js";
import { getTeamGlow } from "../../utils/teamColors";
import { isPortuguese, localizedAiError } from "../../utils/aiFallback";
import { CLIMA_RESULTADO, weatherLabel as climaLabel } from "../../utils/weather";

// Tela pós-corrida REDESENHADA (v2), atrás de flag de dev. NÃO substitui a atual —
// renderizada em paralelo no Dashboard só quando a flag liga, para comparar lado a
// lado. Duas abas: DEBRIEF (tabela rica + debrief do engenheiro) e TELEMETRIA
// (cockpit). Recebe os MESMOS props da tela atual: result, evaluation, telemetry.

const ASSESSMENT = {
  MuitoAcima: { key: "muitoAcima", color: "#4ade80", emoji: "🔥" },
  Acima: { key: "acima", color: "#4ade80", emoji: "✅" },
  Dentro: { key: "dentro", color: "#58a6ff", emoji: "🎯" },
  Abaixo: { key: "abaixo", color: "#f59e0b", emoji: "⚠️" },
  MuitoAbaixo: { key: "muitoAbaixo", color: "#ef4444", emoji: "🔻" },
};

const PODIUM = { 1: "#f5c76d", 2: "#c9d1d9", 3: "#cd8a55" };

// O `font-mono` do app aponta pro mesmo Space Grotesk (não é mono). Para os números
// da corrida usamos uma mono de verdade do sistema — dígitos crus e alinhados.
const MONO =
  '"Cascadia Code", "Cascadia Mono", "JetBrains Mono", "SF Mono", Consolas, "Roboto Mono", ui-monospace, monospace';

const weatherLabel = (w) => climaLabel(w, CLIMA_RESULTADO);

// Glifo próprio desta tela: sem seletor de variação (fica alinhado com a numeração
// mono da tabela) e "Dry" → ☀, não o ⛅ do banner do Header — aqui o rótulo ao lado
// é "Seco", então um sol é o pareamento certo. Ver utils/weather.js.
function weatherIcon(w) {
  if (w === "HeavyRain") return "⛈";
  if (w === "Wet") return "🌧";
  if (w === "Damp") return "🌦";
  return "☀";
}

function formatLapMs(ms) {
  if (!Number.isFinite(ms) || ms <= 0) return "—";
  const s = ms / 1000;
  const m = Math.floor(s / 60);
  const rest = s - m * 60;
  return `${m}:${rest.toFixed(3).padStart(6, "0")}`;
}

function formatUSD(v) {
  const n = Math.round(v || 0);
  return `$${n.toLocaleString("en-US")}`;
}

// Tooltip de quebra: uma linha por peça que largou — volta, peça, o problema concreto e o que
// custou (segundos no box ou a corrida).
function breakdownTip(list, t) {
  return list
    .map((b) => {
      const cost = b.penalty_secs != null ? `+${b.penalty_secs}s` : t("raceResult.breakdowns.retired");
      return `${t("raceResult.breakdowns.lapTip", { lap: b.lap })} · ${b.part_name} — ${capitalizar(b.label)} (${cost})`;
    })
    .join("\n");
}

// Custo agregado das quebras de UM piloto: segundos somados e se alguma encerrou a corrida.
// É o que vira o rótulo ao lado da chave inglesa na tabela.
function breakdownCost(list) {
  return {
    secs: list.reduce((sum, b) => sum + (b.penalty_secs ?? 0), 0),
    retired: list.some((b) => b.severity === "dnf"),
  };
}

// Cor pelo pior desfecho do piloto: abandono > pesado > leve.
function breakdownColor(list) {
  if (list.some((b) => b.severity === "dnf")) return "#f0a3a3";
  if (list.some((b) => b.severity === "heavy")) return "#f0b37a";
  return "#e6d27a";
}

function formatGap(entry) {
  if (!entry) return "—";
  if (entry.finish_position === 1 && !entry.is_dnf) return "—";
  if (entry.is_dnf) return "—";
  const s = (entry.gap_to_winner_ms ?? 0) / 1000;
  if (s <= 0) return "—";
  return `+${s.toFixed(3)}`;
}

function RaceResultViewV2({ result, evaluation, telemetry, maintenance, repercussion, onDismiss }) {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const season = useCareerStore((state) => state.season);
  const lastRaceId = useCareerStore((state) => state.lastRaceId);
  const language = useCareerStore((state) => state.language);
  const [tab, setTab] = useState("debrief");
  // Debrief por IA (engenheiro → você). Lazy: gera ao abrir, cacheado por corrida.
  // Fallback pro texto determinístico do cérebro enquanto carrega / se falhar.
  const [aiDebrief, setAiDebrief] = useState(null);
  // Começa "carregando" (true) pra NÃO piscar o template determinístico antes do
  // effect rodar — o engenheiro fica "no rádio" e o texto só aparece quando chega.
  const [aiLoading, setAiLoading] = useState(true);
  // DEV: injeta telemetria fake pra conferir o cockpit sem correr no iRacing.
  const [mockTelem, setMockTelem] = useState(false);
  const telemetryForCockpit = mockTelem ? MOCK_TELEMETRY : telemetry;
  const [drivers, setDrivers] = useState([]);
  // Clicar em "melhor volta" reordena pela volta mais rápida; volta ao normal em 5s.
  const [sortByLap, setSortByLap] = useState(false);
  const lapSortTimer = useRef(null);
  // Hover: destaca a linha sob o mouse + companheiro(s) de equipe, na cor da equipe.
  const [hoverTeam, setHoverTeam] = useState(null);
  // Piloto realçado ao passar o mouse no nome dele no texto do engenheiro → acende a
  // linha correspondente na tabela de resultados (na cor da equipe).
  const [hoveredDriverId, setHoveredDriverId] = useState(null);
  // Card da repercussão: estado em vez de `group-hover` porque o card mora FORA da
  // faixa (que recorta), então gatilho e painel não são pai/filho.
  const [repercussionOpen, setRepercussionOpen] = useState(false);
  // A fatura da etapa, buscada pelo comando `get_stage_invoice`. `null` enquanto carrega
  // ou quando a rodada nao moveu o caixa — a celula cai na decomposicao legada.
  const [invoice, setInvoice] = useState(null);

  function handleSortByLap() {
    setSortByLap(true);
    if (lapSortTimer.current) clearTimeout(lapSortTimer.current);
    lapSortTimer.current = setTimeout(() => setSortByLap(false), 5000);
  }
  useEffect(() => () => {
    if (lapSortTimer.current) clearTimeout(lapSortTimer.current);
  }, []);

  // Busca o debrief do engenheiro por IA (lazy + cache no backend por race_id).
  useEffect(() => {
    let active = true;
    if (!careerId || !lastRaceId) {
      setAiLoading(false);
      return undefined;
    }
    setAiLoading(true);
    invoke("post_race_debrief_ai", { careerId, raceId: lastRaceId })
      .then((r) => {
        if (active && r && (r.headline || r.body)) {
          setAiDebrief({ headline: r.headline || null, body: r.body || null });
        }
      })
      .catch(() => {})
      .finally(() => {
        if (active) setAiLoading(false);
      });
    return () => {
      active = false;
    };
  }, [careerId, lastRaceId]);

  // A FATURA da etapa: sete linhas físicas com quantidade e preço, os canais de receita
  // e o custo fixo do ano no rodapé. Vem por comando próprio (e não no payload da
  // corrida) porque ela é remontada do ledger — o dinheiro que de fato saiu do caixa —,
  // então uma tela reaberta meses depois lê a mesma fatura.
  useEffect(() => {
    let active = true;
    setInvoice(null);
    if (!careerId || !lastRaceId) return undefined;
    invoke("get_stage_invoice", { careerId, raceId: lastRaceId })
      .then((data) => {
        if (active && data) setInvoice(data);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [careerId, lastRaceId]);

  // Nacionalidade e cor de equipe não vêm no resultado da corrida — puxamos do
  // elenco da categoria (mesmo caminho da tela atual) e cruzamos por id/nome.
  useEffect(() => {
    let active = true;
    if (!careerId || !playerTeam?.categoria) return undefined;
    invoke("get_drivers_by_category", { careerId, category: playerTeam.categoria })
      .then((data) => {
        if (active) setDrivers(Array.isArray(data) ? data : []);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [careerId, playerTeam?.categoria]);

  // Quebras de peça da corrida (Peça 3): quem teve problema, qual peça, e o desfecho.
  // Resumo no Debrief + detalhe por piloto na Telemetria. Cruzado por pilot_id/nome.
  const [breakdowns, setBreakdowns] = useState([]);
  useEffect(() => {
    let active = true;
    if (!careerId || !lastRaceId) return undefined;
    invoke("get_race_breakdowns", { careerId, raceId: lastRaceId })
      .then((data) => {
        if (active) setBreakdowns(Array.isArray(data) ? data : []);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [careerId, lastRaceId]);

  // DEV: com os dados fake ligados, as quebras vêm de uma receita montada sobre o grid REAL da
  // tela (ver `buildMockBreakdowns`) — é o que permite conferir a UI de quebra sem depender de
  // uma corrida em que alguém de fato quebrou. Daqui pra baixo tudo lê `shownBreakdowns`.
  const shownBreakdowns = useMemo(
    () => (mockTelem ? buildMockBreakdowns(result?.race_results) : breakdowns),
    [mockTelem, result, breakdowns],
  );

  // Agrupa as quebras por piloto (um piloto pode ter mais de uma peça largando).
  const breakdownsByDriver = useMemo(() => {
    const m = {};
    for (const b of shownBreakdowns) {
      (m[b.driver_id] ||= []).push(b);
    }
    return m;
  }, [shownBreakdowns]);

  // O que a quebra custou AO JOGADOR — é a leitura que o debrief dele precisa dar. Tempo de
  // box somado; se uma peça encerrou a corrida, o custo não é tempo, é a corrida inteira.
  const playerBreakdowns = useMemo(
    () => shownBreakdowns.filter((b) => b.is_player),
    [shownBreakdowns],
  );
  const playerTimeLost = useMemo(
    () => playerBreakdowns.reduce((sum, b) => sum + (b.penalty_secs ?? 0), 0),
    [playerBreakdowns],
  );
  const playerRetiredByPart = playerBreakdowns.some((b) => b.severity === "dnf");

  const natById = useMemo(() => {
    const m = {};
    for (const d of drivers) if (d?.id) m[d.id] = d.nacionalidade;
    return m;
  }, [drivers]);

  const teamColorByName = useMemo(() => {
    const m = {};
    for (const d of drivers) if (d?.equipe_nome && d?.equipe_cor) m[d.equipe_nome] = d.equipe_cor;
    return m;
  }, [drivers]);

  // Paradas (aprox.) por nome de piloto, da telemetria — refinado no passo do painel
  // de pneus (inclui fuel-only). Aqui conta trocas de pneu como nº de paradas.
  const pitByName = useMemo(() => {
    const m = {};
    for (const t of telemetry?.tire_strategies ?? []) {
      if (t?.pilot_name) m[t.pilot_name] = t.tire_changes ?? 0;
    }
    return m;
  }, [telemetry]);

  const sortedResults = useMemo(() => {
    const rows = [...(result?.race_results ?? [])];
    if (sortByLap) {
      const lapVal = (e) =>
        Number.isFinite(e.best_lap_time_ms) && e.best_lap_time_ms > 0 ? e.best_lap_time_ms : Infinity;
      rows.sort((a, b) => lapVal(a) - lapVal(b));
      return rows;
    }
    rows.sort((a, b) => {
      if (!!a.is_dnf !== !!b.is_dnf) return a.is_dnf ? 1 : -1;
      return (a.finish_position ?? 999) - (b.finish_position ?? 999);
    });
    return rows;
  }, [result, sortByLap]);

  const playerEntry = useMemo(
    () => sortedResults.find((e) => e.is_jogador) ?? null,
    [sortedResults],
  );

  // Pilotos que o texto do engenheiro pode mencionar: os que correram esta etapa
  // (id/nome vêm dos próprios resultados — auto-consistente com a tabela).
  const mentionDrivers = useMemo(
    () => (result?.race_results ?? []).map((e) => ({ id: e.pilot_id, nome: e.pilot_name })),
    [result],
  );

  // Companheiro de equipe (mesmo team_id, não-jogador) — padrão do Gap na telemetria.
  const teammateName = useMemo(() => {
    if (!playerEntry) return null;
    const mate = (result?.race_results ?? []).find(
      (r) => r.team_id === playerEntry.team_id && !r.is_jogador,
    );
    return mate?.pilot_name ?? null;
  }, [result, playerEntry]);

  const assess = evaluation ? ASSESSMENT[evaluation.assessment] ?? ASSESSMENT.Dentro : null;
  const gained = playerEntry?.positions_gained ?? 0;
  const accent = assess?.color || "#58a6ff";

  return (
    <div style={{ color: "#e6edf3" }} className="font-sans min-h-[calc(100vh-3.5rem)] flex flex-col justify-center">
      <div
        style={{
          background: "linear-gradient(160deg, rgba(10,15,22,0.62) 0%, rgba(8,13,20,0.74) 100%)",
          border: "1px solid rgba(255,255,255,0.08)",
          backdropFilter: "blur(40px)",
          WebkitBackdropFilter: "blur(40px)",
          boxShadow: "0 26px 70px rgba(0,0,0,0.5), inset 0 1px 0 rgba(255,255,255,0.05)",
        }}
        className="rounded-[20px] overflow-hidden"
      >
        {/* Cabeçalho + abas */}
        <div
          style={{ background: "rgba(255,255,255,0.025)", borderBottom: "1px solid rgba(255,255,255,0.07)" }}
          className="flex items-center justify-between px-5 py-3.5"
        >
          {/* `relative` para ancorar o card da repercussão FORA da faixa: a faixa é
              `overflow-hidden` (as pontas arredondadas cortam os segmentos), então um
              painel flutuante dentro dela seria recortado. */}
          <div className="relative">
          <div className="glass-light flex items-stretch h-[76px] rounded-2xl overflow-hidden">
            {/* Bloco do clima */}
            <Tooltip texto={weatherLabel(result?.weather)}>
              <div
                className="flex items-center justify-center"
                style={{ width: "78px", background: "rgba(255,255,255,0.045)", borderRight: "1px solid rgba(255,255,255,0.08)" }}
              >
                <span className="text-[36px] leading-none">{weatherIcon(result?.weather)}</span>
              </div>
            </Tooltip>
            {/* Pista + condição */}
            <div className="flex flex-col justify-center px-6 min-w-0" style={{ borderRight: "1px solid rgba(255,255,255,0.08)" }}>
              <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.18em] leading-none mb-1.5">{t("raceResult.header.finalClassification")}</div>
              <div className="text-[25px] font-semibold text-white leading-none truncate">{result?.track_name || t("raceResult.header.raceFallback")}</div>
              <div style={{ color: "#8b949e" }} className="text-[13px] mt-2 leading-none">{weatherLabel(result?.weather)}</div>
            </div>
            {/* Segmentos de stat */}
            <div className="flex flex-col justify-center px-7 text-center" style={{ borderRight: "1px solid rgba(255,255,255,0.08)" }}>
              <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.16em] leading-none mb-2.5">{t("raceResult.header.laps")}</div>
              <div style={{ color: "#fff", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[24px] font-medium leading-none">{result?.total_laps ?? 0}</div>
            </div>
            {season?.ano && (
              <div
                className="flex flex-col justify-center px-7 text-center"
                style={repercussion ? { borderRight: "1px solid rgba(255,255,255,0.08)" } : undefined}
              >
                <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.16em] leading-none mb-2.5">{t("raceResult.header.season")}</div>
                <div style={{ color: "#fff", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[24px] font-medium leading-none">{season.ano}</div>
              </div>
            )}
            {/* Repercussão do EVENTO — fecha a faixa de contexto (pista, voltas, temporada).
                Fica aqui, e não na régua do debrief, porque não é métrica do piloto: é o
                porte que a corrida alcançou. Só entra quando o backend mandou o dado. */}
            {repercussion && (
              <RepercussionSegment
                repercussion={repercussion}
                onHover={setRepercussionOpen}
                open={repercussionOpen}
              />
            )}
          </div>
          {repercussion && <RepercussionCard repercussion={repercussion} open={repercussionOpen} />}
          </div>
          <div className="flex items-center gap-2">
            {/* Vale nas DUAS abas: os dados fake alimentam o cockpit da Telemetria E a UI de
                quebra do Debrief (chips, 🔧 na tabela, tempo perdido na régua). */}
            {import.meta.env.DEV && (
              <Tooltip texto={t("raceResult.dev.mockTitle")}>
                <button
                  type="button"
                  onClick={() => setMockTelem((v) => !v)}
                  style={mockTelem ? { background: "#a855f733", color: "#d6bcfa", border: "0.5px solid #a855f755" } : { border: "0.5px solid rgba(255,255,255,0.12)", color: "#8b949e" }}
                  className="text-[11px] px-3 py-[7px] rounded-lg"
                >
                  {mockTelem ? `🧪 ${t("raceResult.dev.fakeOn")}` : `🧪 ${t("raceResult.dev.fakeData")}`}
                </button>
              </Tooltip>
            )}
            <span
              style={{ background: "#0b0f16", border: "0.5px solid rgba(255,255,255,0.08)" }}
              className="flex gap-1.5 rounded-[10px] p-[3px]"
            >
              <button
                type="button"
                onClick={() => setTab("debrief")}
                style={tab === "debrief" ? { background: "#58a6ff22", color: "#58a6ff" } : { color: "#8b949e" }}
                className="text-[12px] px-[15px] py-[6px] rounded-lg uppercase tracking-wide"
              >
                {t("raceResult.tabs.debrief")}
              </button>
              <button
                type="button"
                onClick={() => setTab("telemetry")}
                style={tab === "telemetry" ? { background: "#58a6ff22", color: "#58a6ff" } : { color: "#8b949e" }}
                className="text-[12px] px-[15px] py-[6px] rounded-lg uppercase tracking-wide"
              >
                {t("raceResult.tabs.telemetry")}
              </button>
            </span>
            {onDismiss && (
              <button
                type="button"
                onClick={onDismiss}
                style={{ border: "0.5px solid rgba(255,255,255,0.12)", color: "#c9d1d9" }}
                className="text-[12px] px-3 py-[7px] rounded-lg hover:bg-white/5"
              >
                {t("raceResult.tabs.continue")}
              </button>
            )}
          </div>
        </div>

        <div className="p-3.5 flex flex-col gap-3">
          {tab === "debrief" ? (
            <>
              {/* Tabela rica de resultados */}
              <div style={{ background: "rgba(0,0,0,0.20)", border: "1px solid rgba(255,255,255,0.06)" }} className="rounded-2xl overflow-hidden">
                <table className="w-full border-collapse" style={{ tableLayout: "fixed" }}>
                  <colgroup>
                    <col style={{ width: "64px" }} />
                    <col style={{ width: "54px" }} />
                    <col style={{ width: "40px" }} />
                    <col />
                    <col />
                    <col style={{ width: "58px" }} />
                    <col style={{ width: "50px" }} />
                    <col style={{ width: "112px" }} />
                    <col style={{ width: "86px" }} />
                    <col style={{ width: "64px" }} />
                  </colgroup>
                  <thead>
                    <tr style={{ background: "rgba(255,255,255,0.025)" }}>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-1 text-center uppercase tracking-wider">{t("raceResult.table.pos")}</th>
                      <th className="py-2.5"></th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-1 text-center uppercase tracking-wider">{t("raceResult.table.nac")}</th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-2 text-left uppercase tracking-wider">{t("raceResult.table.driver")}</th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-2 text-left uppercase tracking-wider">{t("raceResult.table.team")}</th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-1 text-center uppercase tracking-wider">{t("raceResult.table.laps")}</th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-1 text-center uppercase tracking-wider">{t("raceResult.table.pits")}</th>
                      <Tooltip texto={t("raceResult.table.bestLapSortTitle")}>
                        <th
                          onClick={handleSortByLap}
                          style={{ color: sortByLap ? "#d6bcfa" : "#8b949e" }}
                          className="text-[11px] font-normal py-2.5 px-1.5 text-right uppercase tracking-wider cursor-pointer select-none hover:text-white"
                        >
                          {t("raceResult.table.bestLap")} {sortByLap ? "↓" : ""}
                        </th>
                      </Tooltip>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-1 text-right uppercase tracking-wider">{t("raceResult.table.gap")}</th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 pr-5 text-right uppercase tracking-wider">{t("raceResult.table.points")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {sortedResults.map((e) => {
                      const isPlayer = e.is_jogador;
                      const teamColor = teamColorByName[e.team_name] || "#5f5e5a";
                      const pits = pitByName[e.pilot_name] ?? 0;
                      const isHoverTeam = hoverTeam && e.team_name === hoverTeam;
                      const isMentionHovered = hoveredDriverId && e.pilot_id === hoveredDriverId;
                      const baseRow = { transition: "background 120ms", height: "46px" };
                      const mentionTone = isMentionHovered ? getTeamGlow(teamColor) : null;
                      const rowStyle = mentionTone
                        ? {
                            ...baseRow,
                            background: mentionTone.soft,
                            boxShadow: `inset 0 0 0 1.5px ${mentionTone.solid}`,
                          }
                        : isHoverTeam
                          ? { ...baseRow, background: withAlpha(teamColor, 0.2) || "rgba(255,255,255,0.06)" }
                          : e.is_dnf
                            ? { ...baseRow, background: "rgba(239,68,68,0.09)" }
                            : isPlayer
                              ? { ...baseRow, background: "rgba(45,212,191,0.08)" }
                              : baseRow;
                      const txt = e.is_dnf ? "#f0a3a3" : isPlayer ? "#ffffff" : "#e6edf3";
                      const isPodium = !e.is_dnf && PODIUM[e.finish_position];
                      return (
                        <tr
                          key={e.pilot_id}
                          style={rowStyle}
                          onMouseEnter={() => setHoverTeam(e.team_name)}
                          onMouseLeave={() => setHoverTeam(null)}
                        >
                          <td
                            className="text-center py-2"
                            style={{ borderTop: "0.5px solid rgba(255,255,255,0.05)", borderLeft: `3px solid ${teamColor}`, paddingLeft: "18px", paddingRight: "8px" }}
                          >
                            {e.is_dnf ? (
                              <Tooltip texto={e.dnf_reason ? capitalizar(e.dnf_reason) : undefined}>
                                <span
                                  style={{ background: "rgba(239,68,68,0.18)", color: "#fca5a5", border: "1px solid rgba(239,68,68,0.3)", fontFamily: MONO }}
                                  className={`text-[10px] px-1.5 py-0.5 rounded${e.dnf_reason ? " cursor-help" : ""}`}
                                >
                                  DNF
                                </span>
                              </Tooltip>
                            ) : isPodium ? (
                              <span
                                style={{
                                  background: PODIUM[e.finish_position],
                                  color: "#1a1407",
                                  boxShadow: `0 2px 8px ${PODIUM[e.finish_position]}55`,
                                  fontFamily: MONO,
                                  fontVariantNumeric: "tabular-nums",
                                }}
                                className="inline-flex items-center justify-center w-7 h-7 rounded-md text-[13.5px] font-semibold"
                              >
                                {e.finish_position}
                              </span>
                            ) : (
                              <span className="text-[15px]" style={{ color: isPlayer ? "#fff" : "#c9d1d9", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }}>
                                {e.finish_position}
                              </span>
                            )}
                          </td>
                          <td className="text-center text-[13px] py-2" style={{ borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {e.is_dnf ? (
                              <span style={{ color: "#8b949e" }}>—</span>
                            ) : gainedCell(e.positions_gained)}
                          </td>
                          <td className="text-center py-2" style={{ borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            <FlagIcon nacionalidade={natById[e.pilot_id]} className="inline-block" />
                          </td>
                          <td className="text-left px-2 py-2 text-[13px] truncate" style={{ color: txt, borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {e.pilot_name}
                            {e.has_fastest_lap && (
                              <Tooltip texto={t("raceResult.table.fastestLapTitle")}>
                                <span style={{ color: "#d6bcfa" }} className="ml-1">⚡</span>
                              </Tooltip>
                            )}
                            {/* Quebra de peça: a chave inglesa marca quem teve problema e o
                                rótulo diz o que custou. A peça e a volta ficam no tooltip —
                                a linha da tabela só carrega a consequência. */}
                            {breakdownsByDriver[e.pilot_id] && (() => {
                              const list = breakdownsByDriver[e.pilot_id];
                              const { secs, retired } = breakdownCost(list);
                              return (
                                <Tooltip texto={breakdownTip(list, t)}>
                                  <span
                                    style={{ color: breakdownColor(list) }}
                                    className="ml-1.5 cursor-help whitespace-nowrap text-[12px]"
                                  >
                                    🔧
                                    {secs > 0 && (
                                      <span style={{ fontFamily: MONO }} className="ml-1 tabular-nums">
                                        +{secs}s
                                      </span>
                                    )}
                                    {secs === 0 && retired && (
                                      <span style={{ fontFamily: MONO }} className="ml-1">DNF</span>
                                    )}
                                  </span>
                                </Tooltip>
                              );
                            })()}
                            {/* Sem marcador "você": a linha do jogador já vem realçada em
                                turquesa e o nome é o dele — o rótulo só repetia. */}
                            <RivalMarker driverId={e.pilot_id} className="ml-1 inline-block" />
                          </td>
                          <td className="text-left px-2 py-2" style={{ borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            <span className="flex items-center gap-1.5">
                              <TeamLogoMark teamName={e.team_name} color={teamColor} size="xs" testId="v2-team-logo" />
                              <span className="text-[12.5px] truncate" style={{ color: "#c9d1d9" }}>{e.team_name}</span>
                            </span>
                          </td>
                          <td className="text-center py-2 text-[12.5px] tabular-nums" style={{ fontFamily: MONO, color: e.is_dnf ? "#e08a8a" : "#c9d1d9", borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {e.laps_completed ?? 0}
                          </td>
                          <td className="text-center py-2 text-[12.5px] tabular-nums" style={{ fontFamily: MONO, color: "#c9d1d9", borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {pits > 0 ? pits : <span style={{ color: "#444" }}>—</span>}
                          </td>
                          <td className="text-right pr-1.5 py-2" style={{ borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {e.has_fastest_lap ? (
                              <span style={{ background: "rgba(168,85,247,0.2)", color: "#d6bcfa", fontFamily: MONO }} className="text-[12.5px] px-1.5 py-0.5 rounded tabular-nums">{formatLapMs(e.best_lap_time_ms)}</span>
                            ) : (
                              <span className="text-[12.5px] tabular-nums" style={{ fontFamily: MONO, color: e.is_dnf ? "#e08a8a" : "#c9d1d9" }}>{formatLapMs(e.best_lap_time_ms)}</span>
                            )}
                          </td>
                          <td className="text-right pr-1 py-2 text-[12.5px] tabular-nums" style={{ fontFamily: MONO, color: e.is_dnf ? "#f0a3a3" : "#8b949e", borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {formatGap(e)}
                          </td>
                          <td className="text-right pr-5 py-2 text-[14px] tabular-nums font-semibold" style={{ fontFamily: MONO, color: txt, borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {e.points_earned > 0 ? e.points_earned : <span style={{ color: "#444" }}>—</span>}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>

              {/* A quebra NÃO tem painel próprio: ela vive na linha do piloto (🔧 + o que
                  custou), com peça e volta no tooltip. Um resumo separado repetia a mesma
                  informação e afastava a consequência de quem a sofreu. O detalhe volta a
                  volta continua na aba Telemetria. */}

              {/* Debrief do engenheiro — painel completo (placeholder até a IA pós-corrida) */}
              {evaluation && (
                <div
                  style={{ background: "rgba(255,255,255,0.03)", border: "1px solid rgba(255,255,255,0.06)", borderLeft: `3px solid ${accent}` }}
                  className="rounded-2xl"
                >
                  {/* Cabeçalho: engenheiro · assessment · nota */}
                  <div className="flex items-center justify-between px-6 pt-4">
                    <div style={{ color: "#8b949e" }} className="text-[11px] uppercase tracking-[0.14em] flex items-center gap-2">
                      🎧 {t("raceResult.debrief.engineerTitle")}
                    </div>
                    {assess && (
                      <div className="flex items-center gap-3 whitespace-nowrap">
                        <span style={{ background: `${accent}1f`, color: accent }} className="inline-flex items-center gap-1.5 text-[12.5px] rounded-full px-3 py-[5px]">
                          {assess.emoji} {t(`raceResult.assessment.${assess.key}`)}
                        </span>
                        {Number.isFinite(evaluation.grade) && (
                          <span className="flex items-baseline gap-1.5">
                            <span style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-wider">{t("raceResult.debrief.grade")}</span>
                            <span style={{ color: accent, fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[27px] font-bold leading-none">
                              {evaluation.grade.toFixed(1)}
                            </span>
                          </span>
                        )}
                      </div>
                    )}
                  </div>

                  {/* Manchete + leitura da equipe */}
                  <div className="px-6 pt-3.5 pb-5">
                    <EngineerSpeech
                      loading={aiLoading && !aiDebrief}
                      ai={aiDebrief}
                      fallbackHeadline={evaluation.headline}
                      fallbackBody={evaluation.team_read}
                      language={language}
                      accent={accent}
                      mentionDrivers={mentionDrivers}
                      hoveredDriverId={hoveredDriverId}
                      onMentionHover={setHoveredDriverId}
                    />
                  </div>

                  {/* Régua de métricas */}
                  <div className="flex" style={{ borderTop: "1px solid rgba(255,255,255,0.07)" }}>
                    <DebriefMetric label={t("raceResult.metrics.startToFinish")} divider>
                      P{playerEntry?.grid_position ?? "—"}{" → "}
                      {playerEntry?.is_dnf ? "DNF" : `P${playerEntry?.finish_position ?? "—"}`}
                      {gained !== 0 && (
                        <span style={{ color: gained > 0 ? "#4ade80" : "#f87171" }} className="text-[14px] ml-1.5">
                          {gained > 0 ? "▲" : "▼"}{Math.abs(gained)}
                        </span>
                      )}
                    </DebriefMetric>
                    <DebriefMetric label={t("raceResult.metrics.raceTarget")} divider>
                      P{evaluation.target_low}{evaluation.target_high !== evaluation.target_low ? `–P${evaluation.target_high}` : ""}
                    </DebriefMetric>
                    <DebriefMetric label={t("raceResult.metrics.bestLap")} divider>
                      {formatLapMs(playerEntry?.best_lap_time_ms)}
                    </DebriefMetric>
                    <DebriefMetric label={t("raceResult.metrics.incidents")} divider>
                      {playerEntry?.incidents_count ?? 0}
                    </DebriefMetric>
                    {/* Só entra quando uma peça do jogador de fato largou — numa corrida em que
                        o carro aguentou não há nada a reportar, e a régua fica com 5 células. */}
                    {playerBreakdowns.length > 0 && (
                      <DebriefMetric
                        label={t("raceResult.metrics.timeLost")}
                        title={breakdownTip(playerBreakdowns, t)}
                        divider
                      >
                        <span style={{ color: playerRetiredByPart ? "#f0a3a3" : "#e0a458" }}>
                          {playerRetiredByPart ? "DNF" : `+${playerTimeLost}s`}
                        </span>
                      </DebriefMetric>
                    )}
                    <MaintenanceMetric maintenance={maintenance} invoice={invoice} />
                  </div>
                </div>
              )}
            </>
          ) : (
            <RaceTelemetryCockpit
              telemetry={telemetryForCockpit}
              teammateName={teammateName}
              breakdowns={shownBreakdowns}
            />
          )}
        </div>
      </div>
    </div>
  );
}

function withAlpha(hex, a) {
  if (typeof hex === "string" && /^#[0-9a-fA-F]{6}$/.test(hex)) {
    return `${hex}${Math.round(a * 255).toString(16).padStart(2, "0")}`;
  }
  return undefined;
}

// Revela um texto palavra por palavra, como fala chegando. Espaços preservados;
// cada palavra entra com um pequeno atraso escalonado (CSS .speech-word). Se `mentions`
// for passado, nomes de piloto viram um único "word" animado + interativo (hover
// acende o piloto na tabela de resultados).
function SpeechWords({ text, delayStep = 30, startDelay = 0, mentions = null }) {
  const segments = mentions?.drivers
    ? segmentDriverMentions(String(text), mentions.drivers)
    : [{ type: "text", text: String(text) }];
  const nodes = [];
  let wi = 0;
  segments.forEach((seg, si) => {
    if (seg.type === "driver") {
      const delay = startDelay + wi * delayStep;
      wi += 1;
      const isActive = mentions.hoveredDriverId === seg.id;
      nodes.push(
        <span key={`s${si}`} className="speech-word" style={{ animationDelay: `${delay}ms` }}>
          <span
            onMouseEnter={() => mentions.onHover(seg.id)}
            onMouseLeave={() => mentions.onHover(null)}
            className={driverMentionClass(isActive, "text-[#58a6ff]", "text-white hover:text-[#58a6ff]")}
          >
            {seg.text}
          </span>
        </span>,
      );
      return;
    }
    seg.text.split(/(\s+)/).forEach((tok, ti) => {
      if (tok === "") return;
      if (/^\s+$/.test(tok)) {
        nodes.push(tok);
        return;
      }
      const delay = startDelay + wi * delayStep;
      wi += 1;
      nodes.push(
        <span key={`s${si}w${ti}`} className="speech-word" style={{ animationDelay: `${delay}ms` }}>
          {tok}
        </span>,
      );
    });
  });
  return nodes;
}

// Bloco da fala do engenheiro: enquanto a IA gera, mostra o equalizer de rádio
// (texto "tapado"); quando chega, revela manchete+corpo com animação de fala. Se a
// IA falhar, cai no texto determinístico do cérebro (sem animação, nunca vazio).
function EngineerSpeech({
  loading,
  ai,
  fallbackHeadline,
  fallbackBody,
  language,
  accent,
  mentionDrivers,
  hoveredDriverId,
  onMentionHover,
}) {
  const { t } = useTranslation();
  if (loading) {
    return (
      <div className="flex items-center gap-3.5 py-2.5">
        <span className="eq" style={{ "--eq-color": accent }} aria-hidden="true">
          <i /><i /><i /><i /><i />
        </span>
        <span style={{ color: "#9aa5b1" }} className="text-[14.5px] italic">{t("raceResult.debrief.onRadio")}</span>
      </div>
    );
  }
  const animated = !!ai;
  // Sem IA: em português cai no texto determinístico do cérebro; em outro idioma
  // mostra "erro na geração de texto" localizado (não despeja PT para não-PT).
  const noAi = !ai?.headline && !ai?.body;
  if (noAi && !isPortuguese(language)) {
    return (
      <p style={{ color: "#6b7280" }} className="text-[14.5px] italic leading-relaxed">
        {localizedAiError(language)}
      </p>
    );
  }
  const headline = ai?.headline || fallbackHeadline;
  const body = ai?.body || fallbackBody;
  const mentions = { drivers: mentionDrivers, hoveredDriverId, onHover: onMentionHover };
  // `key` por conteúdo: ao trocar de loading→texto, remonta e dispara a animação.
  return (
    <div key={animated ? "ai" : "fallback"}>
      {headline && (
        <div style={{ color: "#fff" }} className="text-[24px] font-semibold leading-snug tracking-tight max-w-[82%]">
          {animated ? (
            <SpeechWords text={headline} delayStep={45} startDelay={0} mentions={mentions} />
          ) : (
            renderTextWithDriverMentions(headline, mentionDrivers, hoveredDriverId, onMentionHover)
          )}
        </div>
      )}
      {body && (
        <p style={{ color: "#9aa5b1" }} className="text-[14.5px] leading-relaxed mt-2.5 mb-0">
          {animated ? (
            <SpeechWords text={body} delayStep={26} startDelay={420} mentions={mentions} />
          ) : (
            renderTextWithDriverMentions(body, mentionDrivers, hoveredDriverId, onMentionHover)
          )}
        </p>
      )}
    </div>
  );
}

function DebriefMetric({ label, divider, title, children }) {
  return (
    <Tooltip texto={title}>
      <div
        className={`flex-1 px-5 py-3.5${title ? " cursor-help" : ""}`}
        style={divider ? { borderRight: "1px solid rgba(255,255,255,0.06)" } : undefined}
      >
        <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.12em] leading-none">{label}</div>
        <div style={{ color: "#fff", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[19px] mt-2 leading-none">
          {children}
        </div>
      </div>
    </Tooltip>
  );
}

// Verde quando a corrida entregou mais do que prometia, vermelho quando entregou
// menos, neutro no empate. A cor sai do SINAL do delta — nada de faixa inventada.
const repercussionTone = (delta) => (delta > 0 ? "#4ade80" : delta < 0 ? "#f87171" : "#8b949e");

const formatAudience = (v, t) =>
  `${(v ?? 0).toLocaleString(currentLang())} ${t("raceResult.repercussion.audienceUnit")}`;

// Segmento "Repercussão" da faixa de contexto do cabeçalho: o tier que o evento
// ALCANÇOU e o saldo contra o que se esperava antes da largada — "esta corrida entregou
// mais do que prometia" é a leitura que interessa.
//
// TODOS os números e rótulos vêm do backend (`EventRepercussionSummary`): o front não
// recalcula tier nem traduz label.
function RepercussionSegment({ repercussion, onHover, open }) {
  const { t } = useTranslation();
  const delta = repercussion.delta_display_value ?? 0;
  const deltaText = `${delta > 0 ? "▲" : "▼"}${Math.abs(delta).toLocaleString(currentLang())}`;
  return (
    <div
      className="flex flex-col justify-center px-5 text-center cursor-help transition-colors duration-150"
      style={{ background: open ? "rgba(255,255,255,0.05)" : undefined }}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
      data-testid="repercussion-segment"
    >
      <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.16em] leading-none mb-1.5">
        {t("raceResult.metrics.repercussion")}
      </div>
      {/* O PÚBLICO é o número — mesma tipografia de Voltas/Temporada, e CENTRADO como
          eles. O delta fica à direita dele, e um espelho INVISÍVEL do mesmo texto ocupa
          a esquerda: com os dois flancos de largura igual, o número cai no eixo exato do
          segmento. Sem o espelho ele escorregava — e escorregava um tanto diferente a
          cada corrida, porque "▼225" e "▲6.500" não têm a mesma largura. Posicionar o
          delta absoluto no canto resolvia o eixo, mas ele batia no rótulo: o segmento é
          estreito demais para ter canto livre. */}
      <div className="flex items-baseline justify-center gap-1 leading-none whitespace-nowrap">
        {delta !== 0 && (
          <span
            aria-hidden="true"
            style={{ fontFamily: MONO, fontVariantNumeric: "tabular-nums" }}
            className="invisible text-[11px]"
          >
            {deltaText}
          </span>
        )}
        <span
          style={{ color: "#fff", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }}
          className="text-[24px] font-medium"
        >
          {(repercussion.final_display_value ?? 0).toLocaleString(currentLang())}
        </span>
        {delta !== 0 && (
          <span
            style={{ color: repercussionTone(delta), fontFamily: MONO, fontVariantNumeric: "tabular-nums" }}
            className="text-[11px]"
          >
            {deltaText}
          </span>
        )}
      </div>
      <div style={{ color: "#8b949e" }} className="text-[13px] mt-2 leading-none whitespace-nowrap">
        {repercussion.final_tier_label}
      </div>
    </div>
  );
}

// Card do confronto esperado × entregue. Mesma linguagem visual do tooltip da
// Manutenção (vidro escuro, borda de 1px, sombra funda) — é o idioma de painel
// flutuante desta tela. Renderizado FORA da faixa, ancorado no wrapper `relative`.
function RepercussionCard({ repercussion, open }) {
  const { t } = useTranslation();
  const delta = repercussion.delta_display_value ?? 0;
  const tone = repercussionTone(delta);
  return (
    <div
      style={{
        background: "rgba(13,19,30,0.97)",
        border: "1px solid rgba(255,255,255,0.1)",
        boxShadow: "0 16px 40px rgba(0,0,0,0.55)",
        backdropFilter: "blur(8px)",
      }}
      className={`absolute top-full right-0 mt-2 z-40 rounded-xl px-4 py-3 min-w-[300px] pointer-events-none transition-all duration-150 ${
        open ? "opacity-100 translate-y-0" : "opacity-0 -translate-y-1"
      }`}
      data-testid="repercussion-card"
    >
      <div style={{ color: "#8b949e" }} className="text-[10px] uppercase tracking-[0.14em] mb-2.5">
        {t("raceResult.repercussion.tooltipTitle")}
      </div>
      <div className="flex flex-col gap-2">
        <RepercussionRow
          label={t("raceResult.repercussion.expected")}
          value={repercussion.expected_tier_label}
          hint={formatAudience(repercussion.expected_display_value, t)}
        />
        <RepercussionRow
          label={t("raceResult.repercussion.delivered")}
          value={repercussion.final_tier_label}
          hint={formatAudience(repercussion.final_display_value, t)}
        />
      </div>
      <div
        style={{ borderTop: "1px solid rgba(255,255,255,0.08)" }}
        className="flex items-center justify-between gap-6 mt-2.5 pt-2.5"
      >
        <span style={{ color: "#8b949e" }} className="text-[11px] uppercase tracking-wide">
          {t("raceResult.repercussion.delta")}
        </span>
        <span
          style={{ color: tone, fontFamily: MONO, fontVariantNumeric: "tabular-nums" }}
          className="text-[13px]"
        >
          {delta > 0 ? "+" : ""}
          {delta.toLocaleString(currentLang())}
        </span>
      </div>
      <div className="flex items-center justify-between gap-6 mt-1.5">
        <span style={{ color: "#8b949e" }} className="text-[11px] uppercase tracking-wide">
          {t("raceResult.repercussion.headline")}
        </span>
        <span style={{ color: "#fff" }} className="text-[12.5px]">{repercussion.headline_strength_label}</span>
      </div>
    </div>
  );
}

function RepercussionRow({ label, value, hint }) {
  return (
    <div className="flex items-baseline justify-between gap-6">
      <span style={{ color: "#8b949e" }} className="text-[11px] uppercase tracking-wide">{label}</span>
      <span className="text-right">
        <span style={{ color: "#fff" }} className="text-[12.5px]">{value}</span>
        <span
          style={{ color: "#6e7681", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }}
          className="block text-[10.5px] mt-0.5"
        >
          {hint}
        </span>
      </span>
    </div>
  );
}

// Ordem dos blocos na fatura do fim de semana. Itens sem `group` (telas salvas antes dos
// blocos existirem) caem num bloco sem cabeçalho, no fim.
const MAINTENANCE_GROUPS = ["carro", "logistica", "equipe", "reparo"];

// Ordem dos blocos da FATURA nova. Despesa primeiro, receita por último: a fatura é uma
// prestação de contas, e o saldo é a última coisa que se lê.
const INVOICE_BLOCKS = ["corrida", "logistica", "equipe", "reparo", "receita"];

// Quantidade física do expandir: sem casas quando a unidade CONTA coisas (não existe meia
// pessoa nem meia diária), uma casa quando é medida contínua (litro, km).
const UNIDADES_CONTAVEIS = ["pessoa", "pessoa_ano", "pessoa_dia", "pessoa_noite", "carro", "ano"];
function formatQty(q, unit) {
  return UNIDADES_CONTAVEIS.includes(unit)
    ? Math.round(q).toLocaleString(currentLang())
    : q.toFixed(1);
}

// Preço UNITÁRIO, que é o número mais sensível da fatura: arredondar para o dólar inteiro
// escreve "$4" onde o litro custa $3,40 e — pior — "$0" onde o quilômetro de revisão custa
// $0,48. Uma linha que diz "198 km × $0" e cobra $96 é exatamente a falsa precisão que este
// redesign existe para remover, só que ao contrário. Abaixo de $10 vão duas casas.
function formatUnitPrice(v) {
  const n = Math.abs(v || 0);
  if (n === 0) return "$0";
  if (n < 10) return `$${(v || 0).toFixed(2)}`;
  return formatUSD(v);
}

// O expandir de uma linha: "173 L × $3,40". É o que responde a "esse número é absurdo?" —
// e é o motivo de o preço, e nunca o total, absorver qualquer ajuste no Rust.
function InvoiceDetail({ detail, t }) {
  return (
    <div className="flex flex-col gap-0.5 mt-0.5 mb-1">
      {detail.map((d) => (
        <div key={d.key} className="flex items-center justify-between gap-6 pl-3">
          <span style={{ color: "#6e7681" }} className="text-[10.5px]">
            {t(`raceResult.invoice.lines.${d.key}`, { defaultValue: d.key })}
          </span>
          <span
            style={{ color: "#6e7681", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }}
            className="text-[10.5px] whitespace-nowrap"
          >
            {formatQty(d.quantity, d.unit)} {t(`raceResult.invoice.units.${d.unit}`, { defaultValue: d.unit })}
            {" × "}
            {formatUnitPrice(d.unitPrice)}
          </span>
        </div>
      ))}
    </div>
  );
}

// Célula "Custos da corrida" da régua: total sempre visível + a FATURA no hover — sete
// linhas físicas, os canais de receita, o conserto se houve e, no pé, o custo fixo do ano.
// A cor é informação: âmbar SÓ quando houve conserto — fim de semana limpo é custo de
// rotina, não alerta.
function MaintenanceMetric({ maintenance, invoice }) {
  const { t } = useTranslation();
  // A fatura nova manda quando existe; sem ela (corrida de bloco especial, save antigo) a
  // célula cai na decomposição legada em vez de sumir.
  const total = invoice ? invoice.expenseTotal : maintenance?.total ?? 0;
  const hasRepair = (invoice?.repairTotal ?? maintenance?.repair_total ?? 0) > 0;
  const totalColor = hasRepair ? "#e0a458" : "#c9d1d9";

  const items = Array.isArray(maintenance?.items) ? maintenance.items : [];
  const legadoAgrupado = MAINTENANCE_GROUPS.map((g) => [g, items.filter((it) => it.group === g)])
    .concat([[null, items.filter((it) => !MAINTENANCE_GROUPS.includes(it.group))]])
    .filter(([, list]) => list.length > 0);

  const lines = Array.isArray(invoice?.lines) ? invoice.lines : [];
  const blocos = INVOICE_BLOCKS.map((b) => [b, lines.filter((l) => l.block === b)]).filter(
    ([, list]) => list.length > 0,
  );
  const temConteudo = blocos.length > 0 || legadoAgrupado.length > 0;

  return (
    <div className="flex-1 px-5 py-3.5 relative group">
      <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.12em] leading-none">{t("raceResult.metrics.maintenance")}</div>
      <div
        style={{ color: totalColor, fontFamily: MONO, fontVariantNumeric: "tabular-nums" }}
        className="text-[19px] mt-2 leading-none inline-flex items-center gap-1.5 cursor-help"
      >
        {formatUSD(total)}
        {temConteudo && <span style={{ color: "#6e7681" }} className="text-[11px]">ⓘ</span>}
      </div>
      {temConteudo && (
        <div
          style={{
            background: "rgba(13,19,30,0.97)",
            border: "1px solid rgba(255,255,255,0.1)",
            boxShadow: "0 16px 40px rgba(0,0,0,0.55)",
            backdropFilter: "blur(8px)",
          }}
          // `pointer-events-auto` no hover: a fatura é mais alta que o card e precisa
          // rolar. Com `pointer-events-none` fixo, o rodapé e os totais existiam no DOM e
          // eram inalcançáveis — o card é filho do `.group`, então deixá-lo receber o
          // mouse mantém o hover vivo enquanto o jogador rola dentro dele.
          className="absolute bottom-full right-4 mb-2 z-30 rounded-xl px-4 py-3 w-[330px] max-h-[62vh] overflow-y-auto opacity-0 translate-y-1 pointer-events-none group-hover:opacity-100 group-hover:translate-y-0 group-hover:pointer-events-auto transition-all duration-150"
        >
          <div style={{ color: "#8b949e" }} className="text-[10px] uppercase tracking-[0.14em] mb-2.5">{t("raceResult.maintenance.invoiceTitle")}</div>

          {blocos.length > 0 ? (
            <>
              <div className="flex flex-col gap-3">
                {blocos.map(([bloco, list]) => (
                  <div key={bloco} className="flex flex-col gap-1">
                    <div
                      style={{ color: bloco === "reparo" ? "#e0a458" : "#6e7681" }}
                      className="text-[9.5px] uppercase tracking-[0.14em]"
                    >
                      {t(`raceResult.invoice.blocks.${bloco}`)}
                    </div>
                    {list.map((l) => (
                      <div key={l.key}>
                        <div className="flex items-center justify-between gap-6">
                          <span style={{ color: "#c9d1d9" }} className="text-[12.5px]">
                            {t(`raceResult.invoice.lines.${l.key}`, { defaultValue: l.key })}
                          </span>
                          <span
                            style={{
                              color: bloco === "receita" ? "#4ade80" : "#fff",
                              fontFamily: MONO,
                              fontVariantNumeric: "tabular-nums",
                            }}
                            className="text-[12.5px]"
                          >
                            {bloco === "receita" ? "+" : ""}{formatUSD(l.total)}
                          </span>
                        </div>
                        {/* `expandable`, não `detail.length`: as linhas que são só
                            dinheiro (os canais de receita, a peça comprada) carregam um
                            detalhe sintético só para o total fechar com a soma. Renderizá-lo
                            escrevia "1 ano × $9.162" embaixo do prêmio da corrida — um
                            rótulo que o número não cumpre, dentro do expandir que existe
                            para cumpri-lo. */}
                        {l.expandable && <InvoiceDetail detail={l.detail} t={t} />}
                      </div>
                    ))}
                  </div>
                ))}
              </div>

              <div style={{ borderTop: "1px solid rgba(255,255,255,0.08)" }} className="mt-3 pt-2.5 flex flex-col gap-1">
                <div className="flex items-center justify-between gap-6">
                  <span style={{ color: "#8b949e" }} className="text-[11px] uppercase tracking-wide">{t("raceResult.invoice.expense")}</span>
                  <span style={{ color: totalColor, fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[13px]">
                    {formatUSD(invoice.expenseTotal)}
                  </span>
                </div>
                {invoice.incomeTotal > 0 && (
                  <>
                    <div className="flex items-center justify-between gap-6">
                      <span style={{ color: "#8b949e" }} className="text-[11px] uppercase tracking-wide">{t("raceResult.invoice.income")}</span>
                      <span style={{ color: "#4ade80", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[13px]">
                        {formatUSD(invoice.incomeTotal)}
                      </span>
                    </div>
                    <div className="flex items-center justify-between gap-6">
                      <span style={{ color: "#c9d1d9" }} className="text-[11px] uppercase tracking-wide">{t("raceResult.invoice.result")}</span>
                      <span
                        style={{
                          color: invoice.result >= 0 ? "#4ade80" : "#f87171",
                          fontFamily: MONO,
                          fontVariantNumeric: "tabular-nums",
                        }}
                        className="text-[14px]"
                      >
                        {formatUSD(invoice.result)}
                      </span>
                    </div>
                  </>
                )}
              </div>

              {/* Rodapé da decisão 10: folha e sede não variam por corrida, então elas
                  não são linha da etapa — só contexto, dito uma vez, no ano inteiro. */}
              {invoice.fixedCost && (
                <div style={{ borderTop: "1px solid rgba(255,255,255,0.08)" }} className="mt-2.5 pt-2.5">
                  <div className="flex items-center justify-between gap-6">
                    <span style={{ color: "#6e7681" }} className="text-[10.5px] uppercase tracking-wide">{t("raceResult.invoice.fixedCost")}</span>
                    <span style={{ color: "#8b949e", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[12px]">
                      {formatUSD(invoice.fixedCostAnnual)}
                    </span>
                  </div>
                  <div style={{ color: "#5a616b" }} className="text-[10px] leading-snug mt-1">
                    {t("raceResult.invoice.fixedCostNote", { share: formatUSD(invoice.fixedCost.total) })}
                  </div>
                </div>
              )}
            </>
          ) : (
            <>
              <div className="flex flex-col gap-3">
                {legadoAgrupado.map(([grupo, list]) => (
                  <div key={grupo ?? "outros"} className="flex flex-col gap-1.5">
                    {grupo && (
                      <div
                        style={{ color: grupo === "reparo" ? "#e0a458" : "#6e7681" }}
                        className="text-[9.5px] uppercase tracking-[0.14em]"
                      >
                        {t(`raceResult.maintenance.groups.${grupo}`)}
                      </div>
                    )}
                    {list.map((it) => (
                      <div key={it.key} className="flex items-center justify-between gap-6">
                        <span style={{ color: "#c9d1d9" }} className="text-[12.5px]">{it.label}</span>
                        <span style={{ color: "#fff", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[12.5px]">{formatUSD(it.cost)}</span>
                      </div>
                    ))}
                  </div>
                ))}
              </div>
              <div style={{ borderTop: "1px solid rgba(255,255,255,0.08)" }} className="flex items-center justify-between gap-6 mt-3 pt-2.5">
                <span style={{ color: "#8b949e" }} className="text-[11px] uppercase tracking-wide">{t("raceResult.maintenance.total")}</span>
                <span style={{ color: totalColor, fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[13px]">{formatUSD(total)}</span>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function gainedCell(g) {
  if (!g) return <span style={{ color: "#8b949e" }}>—</span>;
  const up = g > 0;
  return (
    <span style={{ color: up ? "#4ade80" : "#f87171", fontWeight: 500 }}>
      {up ? "▲" : "▼"}
      {Math.abs(g)}
    </span>
  );
}

export default RaceResultViewV2;
