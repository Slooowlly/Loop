import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import TeamLogoMark from "../team/TeamLogoMark";
import RaceCharts from "./RaceCharts";
import useCareerStore from "../../stores/useCareerStore";
import { formatGap, formatLapTime } from "../../utils/formatters";

const CATEGORY_SUMMARY_LOGOS = {
  mazda: "/utilities/categorias/recortadas/MX5%20CUP.png",
  mazda_amador: "/utilities/categorias/recortadas/MX5%20CUP.png",
  mazda_rookie: "/utilities/categorias/recortadas/MX5%20ROOKIE.png",
  toyota: "/utilities/categorias/recortadas/GR%20CUP.png",
  toyota_amador: "/utilities/categorias/recortadas/GR%20CUP.png",
  toyota_rookie: "/utilities/categorias/recortadas/GR%20ROOKIE.png",
  bmw: "/utilities/categorias/recortadas/M2%20CUP.png",
  bmw_m2: "/utilities/categorias/recortadas/M2%20CUP.png",
  gt4: "/utilities/categorias/recortadas/GT4.png",
  gt3: "/utilities/categorias/recortadas/GT3.png",
  production_challenger: "/utilities/categorias/recortadas/PRODUCTION.png",
  endurance: "/utilities/categorias/recortadas/ENDURANCE.png",
  lmp2: "/utilities/categorias/recortadas/LMP2.png",
};

const CATEGORY_SUMMARY_FITS = {
  mazda: {
    frameClassName: "overflow-hidden",
    imageStyle: {
      clipPath: "inset(0 0 8% 0)",
    },
  },
  mazda_amador: {
    frameClassName: "overflow-hidden",
    imageStyle: {
      clipPath: "inset(0 0 8% 0)",
    },
  },
};

function weatherLabel(value) {
  if (value === "HeavyRain") return "Chuva forte";
  if (value === "Wet") return "Chuva";
  if (value === "Damp") return "Úmido";
  return "Seco";
}

// Avaliação do cérebro (race_eval) → rótulo + cor + emoji.
const ASSESSMENT = {
  MuitoAcima: { label: "Muito acima do esperado", color: "text-green-400", emoji: "🔥" },
  Acima: { label: "Acima do esperado", color: "text-green-400", emoji: "✅" },
  Dentro: { label: "Dentro do esperado", color: "text-[#58a6ff]", emoji: "🎯" },
  Abaixo: { label: "Abaixo do esperado", color: "text-amber-400", emoji: "⚠️" },
  MuitoAbaixo: { label: "Muito abaixo do esperado", color: "text-red-400", emoji: "🔻" },
};

// Caixa da nota colorida pela faixa.
function gradeBox(grade) {
  if (grade >= 7.5) return "border-green-500/30 bg-green-500/10 text-green-400";
  if (grade >= 6.0) return "border-[#58a6ff]/30 bg-[#58a6ff]/10 text-[#58a6ff]";
  if (grade >= 4.5) return "border-amber-500/30 bg-amber-500/10 text-amber-400";
  return "border-red-500/30 bg-red-500/10 text-red-400";
}

function ExpStat({ label, value, highlight }) {
  return (
    <div className="rounded-xl border border-white/10 bg-white/[0.03] px-4 py-3">
      <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">{label}</p>
      <p className={`mt-1 font-mono text-lg font-black ${highlight ? "text-white" : "text-gray-300"}`}>
        {value}
      </p>
    </div>
  );
}

// Delta em segundos a partir de ms (ex.: "+0.82s" / "-0.30s"). "—" se ~zero.
function fmtDeltaS(ms) {
  if (ms == null || Math.abs(ms) < 5) return "—";
  const s = (ms / 1000).toFixed(2);
  return `${ms > 0 ? "+" : ""}${s}s`;
}

// Chip de confiança da telemetria (alta/media/baixa).
const CONFIDENCE = {
  alta: { label: "Confiança alta", color: "text-green-400 border-green-500/30 bg-green-500/10" },
  media: { label: "Confiança média", color: "text-amber-400 border-amber-500/30 bg-amber-500/10" },
  baixa: { label: "Confiança baixa", color: "text-gray-400 border-white/15 bg-white/5" },
};

// Frase de cobertura: deixa claro quanto da corrida foi de fato analisado, para
// o jogador não achar que a análise cobriu a prova toda quando ele saiu cedo.
function coverageNote(t) {
  if (!t) return "";
  if (t.is_partial && t.last_lap_seen > 0) {
    return `Análise parcial — telemetria registrada até a volta ${t.last_lap_seen}.`;
  }
  if (t.race_laps > 0) {
    return `Análise baseada em ${t.laps_seen} de ${t.race_laps} voltas registradas.`;
  }
  return `Análise baseada em ${t.laps_seen} ${t.laps_seen === 1 ? "volta registrada" : "voltas registradas"}.`;
}

// Card de análise da telemetria (título + linhas de stat).
function AnalysisCard({ title, accent, children }) {
  return (
    <div className={`rounded-2xl border bg-white/[0.02] p-4 ${accent}`}>
      <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold mb-2">{title}</p>
      <div className="space-y-1.5">{children}</div>
    </div>
  );
}

function StatRow({ label, value, color }) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <span className="text-[11px] text-gray-400">{label}</span>
      <span className={`font-mono text-sm font-bold ${color || "text-gray-200"}`}>{value}</span>
    </div>
  );
}

// Uma linha do breakdown de posições: seta + rótulo + valor com sinal.
function FlowRow({ icon, label, value, color }) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg bg-white/[0.03] px-3 py-2">
      <span className="flex items-center gap-2 text-[12px] text-gray-300">
        <span className={color}>{icon}</span>
        {label}
      </span>
      <span className={`font-mono text-sm font-black ${color}`}>{value}</span>
    </div>
  );
}

// Aba do painel direito (Resultados / Campeonato / Gráficos).
function PanelTab({ active, onClick, children }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        "rounded-lg px-3 py-1.5 text-[11px] font-bold uppercase tracking-widest transition",
        active ? "bg-[#58a6ff]/20 text-[#58a6ff]" : "text-gray-400 hover:text-white",
      ].join(" ")}
    >
      {children}
    </button>
  );
}

// Banner de "momento" da corrida (melhor momento / erro mais caro).
function MomentBanner({ label, card, confidence }) {
  if (!card) return null;
  return (
    <div className={`flex items-start gap-3 rounded-2xl border bg-white/[0.02] p-4 ${card.accent}`}>
      <span className="text-2xl mt-0.5">{card.icon}</span>
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">{label}</p>
          {confidence === "media" && (
            <span className="text-[8px] uppercase tracking-widest text-gray-500 border border-white/10 rounded px-1.5 py-0.5">
              estimado
            </span>
          )}
        </div>
        <p className={`mt-1 text-sm font-extrabold ${card.color}`}>{card.title}</p>
        <p className="mt-0.5 text-[13px] leading-relaxed text-gray-400">{card.desc}</p>
      </div>
    </div>
  );
}

// Erro mais caro (2b-2) → título + frase + visual, a partir de kind + números.
// Sempre "estimado"; nunca promete mais certeza do que temos.
function mistakeCard(m) {
  if (!m) return null;
  const lapTag = m.lap > 0 ? `Volta ${m.lap} — ` : "";
  const sec = (m.time_lost_ms / 1000).toFixed(1);
  const pos = m.positions_lost;
  switch (m.kind) {
    case "incident": {
      const parts = [];
      if (m.time_lost_ms >= 500) parts.push(`${sec}s`);
      if (pos > 0) parts.push(`${pos} posição${pos > 1 ? "ões" : ""}`);
      const tail = parts.length ? `Perda estimada de ${parts.join(" e ")}.` : "Contato que comprometeu a volta.";
      return { title: `${lapTag}Incidente custoso`, desc: tail, accent: "border-red-500/30", icon: "💥", color: "text-red-400" };
    }
    case "position_loss":
      return {
        title: `${lapTag}Perda de posição`,
        desc: `Você perdeu ${pos} posição${pos > 1 ? "ões" : ""} em uma única volta.`,
        accent: "border-amber-500/30",
        icon: "⬇️",
        color: "text-amber-400",
      };
    case "dnf":
      return {
        title: `${lapTag}Corrida comprometida`,
        desc: "O abandono encerrou sua análise de ritmo e limitou o resultado final.",
        accent: "border-red-500/30",
        icon: "🏁",
        color: "text-red-400",
      };
    case "pace_drop":
    default:
      return {
        title: `${lapTag}Queda de ritmo`,
        desc: `Você ficou ${sec}s acima do seu ritmo limpo estimado.`,
        accent: "border-amber-500/30",
        icon: "📉",
        color: "text-amber-400",
      };
  }
}

// Melhor momento (2b-3) → título + frase + visual, a partir de kind + números.
function bestMomentCard(b) {
  if (!b) return null;
  const lapTag = b.lap > 0 ? `Volta ${b.lap} — ` : "";
  const sec = (b.time_gain_ms / 1000).toFixed(1);
  const pos = b.positions_gained;
  switch (b.kind) {
    case "position_gain":
      return {
        title: `${lapTag}Ataque decisivo`,
        desc: `Você ganhou ${pos} posição${pos > 1 ? "ões" : ""} em uma única volta.`,
        accent: "border-green-500/30",
        icon: "🚀",
        color: "text-green-400",
      };
    case "rival_beaten":
      return {
        title: "Rival superado",
        desc: `Você venceu a disputa direta contra ${b.rival_name} após ${b.streak} voltas em batalha.`,
        accent: "border-orange-500/30",
        icon: "⚔️",
        color: "text-orange-300",
      };
    case "recovery":
      return {
        title: "Boa reação",
        desc:
          pos > 0
            ? `Depois do ponto mais caro da corrida, você recuperou ritmo e ganhou ${pos} posição${pos > 1 ? "ões" : ""} de volta.`
            : "Depois do ponto mais caro da corrida, você recuperou o ritmo e voltou a brigar.",
        accent: "border-[#58a6ff]/30",
        icon: "🔄",
        color: "text-[#58a6ff]",
      };
    case "clean_streak":
      return {
        title: "Sequência forte",
        desc: `Você encaixou ${b.streak} voltas boas seguidas, mantendo ritmo constante.`,
        accent: "border-green-500/30",
        icon: "📊",
        color: "text-green-400",
      };
    case "best_lap":
    default:
      return {
        title: `${lapTag}Melhor ritmo da corrida`,
        desc: `Sua melhor volta veio ${sec}s abaixo do seu ritmo limpo — confirmou seu potencial de ritmo.`,
        accent: "border-purple-500/30",
        icon: "🎯",
        color: "text-purple-300",
      };
  }
}

// Frase honesta do breakdown (sem prometer mais certeza do que temos).
function breakdownSentence(b) {
  if (!b) return "";
  if (b.isDnf) return `Você abandonou a prova (largou em P${b.grid}).`;
  if (b.net === 0) return `Você terminou onde largou (P${b.grid}).`;
  if (b.net < 0) {
    return `Você perdeu ${b.lost} posição${b.lost > 1 ? "ões" : ""} em relação à largada (P${b.grid} → P${b.finish}).`;
  }
  if (b.inherited > 0) {
    return `Você ganhou ${b.gained} posição${b.gained > 1 ? "ões" : ""} no resultado final. Destas, até ${b.inherited} pode${b.inherited > 1 ? "m" : ""} ter vindo de abandonos de pilotos que largaram à frente.`;
  }
  return `Você ganhou ${b.gained} posição${b.gained > 1 ? "ões" : ""} no resultado final — nenhum abandono relevante à sua frente.`;
}

function getCategorySummaryLogo(categoryId) {
  return typeof categoryId === "string" ? CATEGORY_SUMMARY_LOGOS[categoryId] ?? null : null;
}

function getCategorySummaryFit(categoryId) {
  if (typeof categoryId !== "string") {
    return { frameClassName: "", imageStyle: {} };
  }

  return CATEGORY_SUMMARY_FITS[categoryId] ?? { frameClassName: "", imageStyle: {} };
}

function RaceResultView({ result, evaluation, telemetry, onDismiss }) {
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const otherCategoriesResult = useCareerStore((state) => state.otherCategoriesResult);
  // Painel direito: 'results' (tabela oficial) | 'championship' | 'charts'.
  const [rightView, setRightView] = useState("results");
  const hasCharts = !!telemetry?.charts;
  const [championship, setChampionship] = useState([]);
  const [teamColors, setTeamColors] = useState({});
  const [loadingChampionship, setLoadingChampionship] = useState(false);
  const [championshipError, setChampionshipError] = useState("");

  const playerResult = useMemo(
    () => result?.race_results?.find((entry) => entry.is_jogador) ?? null,
    [result],
  );
  
  const winner = useMemo(
    () => result?.race_results?.find((entry) => entry.finish_position === 1) ?? null,
    [result],
  );
  
  const poleSitter = useMemo(
    () => result?.qualifying_results?.find((entry) => entry.is_pole) ?? null,
    [result],
  );
  
  const fastestLap = useMemo(
    () => result?.race_results?.find((entry) => entry.has_fastest_lap) ?? null,
    [result],
  );
  
  const biggestGainer = useMemo(() => {
    const activeResults = result?.race_results?.filter((entry) => !entry.is_dnf) ?? [];
    if (activeResults.length === 0) return null;
    return activeResults.reduce((best, entry) =>
      entry.positions_gained > best.positions_gained ? entry : best,
    activeResults[0]);
  }, [result]);

  // Breakdown de posições (2b-1). Nível 1 = só tabela oficial (sempre funciona,
  // honesto). Nível 2 = enriquecido com a trajetória da telemetria (estimado).
  // O SALDO (grid → chegada) é sempre oficial; o resto é estimativa.
  const positionBreakdown = useMemo(() => {
    if (!playerResult) return null;
    const grid = playerResult.grid_position || 0;
    const entries = result?.race_results ?? [];
    // Pilotos que largaram À FRENTE do jogador e abandonaram (fonte das herdadas).
    const dnfAhead = entries.filter(
      (e) => !e.is_jogador && e.is_dnf && e.grid_position > 0 && grid > 0 && e.grid_position < grid,
    ).length;

    if (playerResult.is_dnf) {
      return { isDnf: true, grid, dnfAhead };
    }

    const finish = playerResult.finish_position;
    const net = playerResult.positions_gained; // grid - finish
    const gained = Math.max(net, 0);
    const lost = Math.max(-net, 0);
    const inherited = Math.min(dnfAhead, gained);

    const flow = telemetry?.position_flow ?? null;
    // Ganhas na pista (estimado): subidas brutas menos as herdadas, nunca < 0.
    const onTrack = flow
      ? Math.max(flow.gained_on_track - inherited, 0)
      : Math.max(gained - inherited, 0);
    const lostOnTrack = flow ? flow.lost_on_track : lost;

    return {
      isDnf: false,
      grid,
      finish,
      net,
      gained,
      lost,
      inherited,
      onTrack,
      lostOnTrack,
      dnfAhead,
      hasFlow: !!flow,
    };
  }, [playerResult, result, telemetry]);

  useEffect(() => {
    let mounted = true;
    async function fetchChampionship() {
      if (!careerId || !playerTeam?.categoria) return;
      setLoadingChampionship(true);
      setChampionshipError("");
      try {
        const data = await invoke("get_drivers_by_category", {
          careerId,
          category: playerTeam.categoria,
        });
        if (mounted) {
          setChampionship(data);
          
          const colors = {};
          data.forEach(d => {
            if (d.equipe_nome && d.equipe_cor) {
              colors[d.equipe_nome] = d.equipe_cor;
            }
          });
          setTeamColors(colors);
        }
      } catch (error) {
        if (mounted) {
          setChampionshipError(
            typeof error === "string" ? error : "Não foi possível carregar o campeonato."
          );
        }
      } finally {
        if (mounted) setLoadingChampionship(false);
      }
    }
    fetchChampionship();
    return () => { mounted = false; };
  }, [careerId, playerTeam?.categoria]);

  if (!result) return null;

  return (
    <div className="relative z-10 flex h-[calc(100vh-4rem)] w-full flex-col overflow-y-auto custom-scrollbar rounded-[32px] border border-white/5 bg-[#080d14]/40 p-2 animate-fade-in shadow-[0_10px_50px_rgba(0,0,0,0.5)] backdrop-blur-3xl lg:p-4">
      
      {/* HEADER */}
      <header className="flex flex-col lg:flex-row justify-between items-end mb-6 border-b border-white/10 pb-6 shrink-0 px-4 pt-4">
        <div>
          <p className="text-[11px] uppercase font-black text-[#58a6ff] tracking-[0.3em] mb-2 shadow-text">Classificação Final</p>
          <h1 className="text-4xl lg:text-5xl font-extrabold text-white tracking-tight">{result.track_name}</h1>
          <p className="text-gray-400 mt-2 font-mono text-sm capitalize">{weatherLabel(result.weather)} • {result.total_laps} Voltas Completadas</p>
        </div>
        
        <div className="mt-6 lg:mt-0 bg-[#0a0f16]/80 border border-white/10 px-6 py-4 rounded-2xl flex items-center gap-6 shadow-xl">
          <div>
            <p className="text-[10px] uppercase tracking-widest text-[#58a6ff] font-bold">Seu Desempenho</p>
            <p className="text-3xl font-black text-white leading-none mt-1 drop-shadow-md">
              {playerResult ? (playerResult.is_dnf ? "DNF" : `P${playerResult.finish_position}`) : "—"}
            </p>
          </div>
          <div className="text-right">
             <p className={`text-xs font-bold px-2 py-0.5 rounded uppercase tracking-wider shadow-sm ${playerResult && playerResult.positions_gained >= 0 ? 'text-green-400 bg-green-500/10' : 'text-red-400 bg-red-500/10'}`}>
                {playerResult ? (playerResult.positions_gained > 0 ? `+${playerResult.positions_gained}` : playerResult.positions_gained) : "-"} Var
             </p>
             <p className="text-[10px] text-gray-400 mt-1 uppercase tracking-widest font-bold">Grid: {playerResult ? `${playerResult.grid_position}º` : "—"}</p>
          </div>
          {evaluation && (
            <>
              <div className="h-10 w-[1px] bg-white/10 mx-2"></div>
              <div className={`flex h-14 w-14 flex-col items-center justify-center rounded-xl border ${gradeBox(evaluation.grade)}`}>
                <span className="text-xl font-black leading-none">{evaluation.grade.toFixed(1)}</span>
                <span className="text-[8px] uppercase tracking-widest opacity-70">Nota</span>
              </div>
            </>
          )}
          <div className="h-10 w-[1px] bg-white/10 mx-2"></div>
          <button onClick={onDismiss} className="px-6 py-3 bg-[#58a6ff] hover:bg-blue-400 text-[#05080c] font-black uppercase tracking-widest rounded-xl transition text-xs shadow-[0_0_20px_rgba(88,166,255,0.2)]">
            Voltar Aos Boxes
          </button>
        </div>
      </header>

      {/* LEITURA DE CARREIRA (Fase 1) — só aparece com avaliação; nunca quebra sem. */}
      {evaluation && (
        <section className="mb-6 shrink-0 px-4">
          <div className="rounded-3xl border border-white/10 bg-gradient-to-br from-[#0a0f16]/90 to-[#080d14]/50 p-6 shadow-xl">
            <div className="flex flex-col gap-5 lg:flex-row">
              {/* Avaliação + frase */}
              <div className="flex items-start gap-4 lg:w-2/5 shrink-0">
                <span className="text-3xl mt-0.5">{ASSESSMENT[evaluation.assessment]?.emoji}</span>
                <div>
                  <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">Avaliação da corrida</p>
                  <p className={`text-xl font-extrabold ${ASSESSMENT[evaluation.assessment]?.color}`}>
                    {ASSESSMENT[evaluation.assessment]?.label}
                  </p>
                </div>
              </div>
              <p className="flex-1 text-sm leading-relaxed text-gray-200 lg:border-l lg:border-white/10 lg:pl-6">
                {evaluation.headline}
              </p>
            </div>

            {/* Meta da corrida vs Resultado (o potencial fica oculto — é interno). */}
            <div className="mt-5 grid grid-cols-1 gap-3 border-t border-white/10 pt-5 sm:grid-cols-2">
              <ExpStat label="Meta da corrida" value={`P${evaluation.target_low}–P${evaluation.target_high}`} />
              <ExpStat
                label="Resultado"
                value={playerResult ? (playerResult.is_dnf ? "DNF" : `P${playerResult.finish_position}`) : "—"}
                highlight
              />
            </div>

            {/* Leitura da equipe */}
            <p className="mt-4 text-[13px] leading-relaxed text-gray-400">
              <span className="text-[10px] uppercase tracking-widest font-bold text-gray-500">Leitura da equipe: </span>
              {evaluation.team_read}
            </p>
          </div>
        </section>
      )}

      {/* SALDO DE POSIÇÕES (2b-1) — Nível 1 sempre (tabela oficial); Nível 2
          enriquece com a telemetria. Tudo "estimado", nada de "ultrapassagem
          limpa". Aparece sempre que há resultado do jogador; nunca quebra. */}
      {positionBreakdown && (
        <section className="mb-6 shrink-0 px-4">
          <div className="rounded-3xl border border-white/10 bg-gradient-to-br from-[#0a0f16]/90 to-[#080d14]/50 p-6 shadow-xl">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">
                Saldo de posições
              </p>
              <span
                className={`text-[9px] uppercase tracking-widest font-bold px-2 py-0.5 rounded border ${
                  positionBreakdown.hasFlow && telemetry?.confidence && CONFIDENCE[telemetry.confidence]
                    ? CONFIDENCE[telemetry.confidence].color
                    : "text-gray-400 border-white/15 bg-white/5"
                }`}
              >
                {positionBreakdown.hasFlow
                  ? `${CONFIDENCE[telemetry.confidence]?.label ?? "Estimado"} · telemetria`
                  : "Nível 1 · tabela oficial"}
              </span>
            </div>

            <div className="mt-4 flex flex-col gap-4 lg:flex-row lg:items-stretch">
              {/* Largou → Chegou + saldo */}
              <div className="flex items-center gap-4 lg:w-2/5 shrink-0">
                <div className="text-center">
                  <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">Largou</p>
                  <p className="mt-1 font-mono text-2xl font-black text-gray-300">P{positionBreakdown.grid}</p>
                </div>
                <span className="text-2xl text-gray-600">→</span>
                <div className="text-center">
                  <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">Chegou</p>
                  <p className="mt-1 font-mono text-2xl font-black text-white">
                    {positionBreakdown.isDnf ? "DNF" : `P${positionBreakdown.finish}`}
                  </p>
                </div>
                {!positionBreakdown.isDnf && (
                  <div className="ml-auto flex h-14 w-16 flex-col items-center justify-center rounded-xl border border-white/10 bg-white/5">
                    <span
                      className={`text-xl font-black leading-none ${
                        positionBreakdown.net > 0
                          ? "text-green-400"
                          : positionBreakdown.net < 0
                            ? "text-red-400"
                            : "text-gray-400"
                      }`}
                    >
                      {positionBreakdown.net > 0 ? `+${positionBreakdown.net}` : positionBreakdown.net}
                    </span>
                    <span className="text-[8px] uppercase tracking-widest text-gray-500">Saldo</span>
                  </div>
                )}
              </div>

              {/* Decomposição estimada */}
              {!positionBreakdown.isDnf && (
                <div className="flex-1 space-y-1.5 lg:border-l lg:border-white/10 lg:pl-6">
                  {positionBreakdown.onTrack > 0 && (
                    <FlowRow
                      icon="▲"
                      label="Ganhas na pista (estimado)"
                      value={`+${positionBreakdown.onTrack}`}
                      color="text-green-400"
                    />
                  )}
                  {positionBreakdown.inherited > 0 && (
                    <FlowRow
                      icon="⬆"
                      label="Herdadas por DNF (provável)"
                      value={`+${positionBreakdown.inherited}`}
                      color="text-[#58a6ff]"
                    />
                  )}
                  {positionBreakdown.lostOnTrack > 0 && (
                    <FlowRow
                      icon="▼"
                      label={positionBreakdown.hasFlow ? "Perdidas na pista (estimado)" : "Perdidas"}
                      value={`-${positionBreakdown.lostOnTrack}`}
                      color="text-red-400"
                    />
                  )}
                  {positionBreakdown.onTrack === 0 &&
                    positionBreakdown.inherited === 0 &&
                    positionBreakdown.lostOnTrack === 0 && (
                      <p className="text-[12px] text-gray-500">Você manteve a posição da largada.</p>
                    )}
                </div>
              )}
            </div>

            <p className="mt-4 text-[13px] leading-relaxed text-gray-400">
              {breakdownSentence(positionBreakdown)}
            </p>
          </div>
        </section>
      )}

      {/* ANÁLISE DA CORRIDA (Fase 2) — só com telemetria (você correu a prova).
          Cada card respeita um critério mínimo: ritmo (>=2 voltas), consistência
          (>=3), vs grid (amostra do campo), rival (disputa real). */}
      {telemetry?.has_telemetry && (
        <section className="mb-6 shrink-0 px-4">
          <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
            <p className="text-[10px] uppercase tracking-[0.22em] text-gray-500 font-bold">
              Análise da corrida
            </p>
            <div className="flex items-center gap-3">
              <span className="text-[11px] text-gray-500">{coverageNote(telemetry)}</span>
              {telemetry.confidence && CONFIDENCE[telemetry.confidence] && (
                <span className={`text-[9px] uppercase tracking-widest font-bold px-2 py-0.5 rounded border ${CONFIDENCE[telemetry.confidence].color}`}>
                  {CONFIDENCE[telemetry.confidence].label}
                </span>
              )}
            </div>
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
            {telemetry.pace && (
              <AnalysisCard title="🏎️ Ritmo" accent="border-[#58a6ff]/20">
                <StatRow label="Melhor volta" value={formatLapTime(telemetry.pace.best_lap_ms)} color="text-purple-300" />
                <StatRow label="Ritmo limpo" value={formatLapTime(telemetry.pace.clean_avg_ms)} />
                {telemetry.pace.vs_grid_reliable && (
                  <StatRow
                    label="vs média do grid"
                    value={fmtDeltaS(telemetry.pace.vs_grid_ms)}
                    color={telemetry.pace.vs_grid_ms < 0 ? "text-green-400" : "text-amber-400"}
                  />
                )}
              </AnalysisCard>
            )}
            {telemetry.pace?.consistency_reliable && (
              <AnalysisCard title="📊 Consistência" accent="border-green-500/20">
                <StatRow
                  label="Voltas boas"
                  value={`${telemetry.pace.good_laps}/${telemetry.pace.total_laps}`}
                  color="text-green-400"
                />
                <StatRow label="Perdido por volta" value={fmtDeltaS(telemetry.pace.lost_per_lap_ms)} color="text-amber-400" />
                <StatRow label="Ritmo médio real" value={formatLapTime(telemetry.pace.real_avg_ms)} />
              </AnalysisCard>
            )}
            {telemetry.rival && (
              <AnalysisCard title="⚔️ Rival da corrida" accent="border-orange-500/20">
                <p className="text-sm font-bold text-white">{telemetry.rival.pilot_name}</p>
                <StatRow label="Voltas em disputa" value={`${telemetry.rival.laps_battled}`} />
                <StatRow label="Gap médio" value={`${telemetry.rival.avg_gap_s.toFixed(1)}s`} />
              </AnalysisCard>
            )}
          </div>

          {/* MELHOR MOMENTO (2b-3) + ERRO MAIS CARO (2b-2) — espelhos. Cada um só
              aparece se houve destaque/momento custoso real (confiança >= média
              no backend); corrida sem nada forte não mostra. */}
          {(telemetry.best_moment || telemetry.mistake) && (
            <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2">
              {telemetry.best_moment && (
                <MomentBanner
                  label="Melhor momento"
                  card={bestMomentCard(telemetry.best_moment)}
                  confidence={telemetry.best_moment.confidence}
                />
              )}
              {telemetry.mistake && (
                <MomentBanner
                  label="Erro mais caro"
                  card={mistakeCard(telemetry.mistake)}
                  confidence={telemetry.mistake.confidence}
                />
              )}
            </div>
          )}
        </section>
      )}

      {/* CONTEÚDO */}
      <div className="grid grid-cols-12 gap-6 min-h-[620px] px-4 pb-4">
        
        {/* Esquerda: Destaques */}
        <div className="col-span-12 lg:col-span-3 flex flex-col gap-4 overflow-y-auto pr-2 custom-scrollbar">
            
            {/* Vencedor */}
            <div className="relative rounded-2xl p-6 text-center border border-yellow-500/20 bg-yellow-500/5 shadow-inner">
                <span className="text-yellow-500 text-3xl mb-2 block drop-shadow-[0_0_15px_rgba(234,179,8,0.5)]">🏆</span>
                <p className="text-[10px] uppercase font-bold text-gray-400 tracking-wider">Vencedor</p>
                <p className="text-xl font-bold text-white mt-1 relative">{winner?.pilot_name || "—"}</p>
                <p className="text-[10px] font-black tracking-widest text-yellow-500 uppercase mt-1 opacity-80">{winner?.team_name || "—"}</p>
            </div>
            
            {/* Fastest Lap */}
            <div className="rounded-2xl p-5 border border-purple-500/20 bg-purple-500/5 shadow-inner flex flex-col justify-center">
                <p className="text-[10px] uppercase font-bold text-purple-400 tracking-wider">Volta Mais Rápida</p>
                <div className="flex justify-between items-end mt-1">
                    <p className="text-lg font-bold text-white truncate max-w-[130px] pr-2">{fastestLap?.pilot_name || "—"}</p>
                    <p className="text-sm font-mono font-bold text-purple-300 drop-shadow-md">{fastestLap ? formatLapTime(fastestLap.best_lap_time_ms) : "—"}</p>
                </div>
            </div>

            {/* Pole Position */}
            <div className="rounded-2xl p-5 border border-white/10 bg-white/5 shadow-inner flex flex-col justify-center">
                <p className="text-[10px] uppercase font-bold text-gray-400 tracking-wider">Pole Position</p>
                <div className="flex justify-between items-end mt-1">
                    <p className="text-lg font-bold text-white truncate max-w-[130px] pr-2">{poleSitter?.pilot_name || "—"}</p>
                    <p className="text-sm font-mono text-gray-400">{poleSitter ? formatLapTime(poleSitter.best_lap_time_ms) : "—"}</p>
                </div>
            </div>

            {/* Escalada */}
            <div className="rounded-2xl p-5 border border-green-500/20 bg-green-500/5 shadow-inner flex items-center justify-between">
                <div>
                    <p className="text-[10px] uppercase font-bold text-green-400 tracking-wider">Maior Escalada</p>
                    <p className="text-lg font-bold text-white mt-1 truncate max-w-[120px]">{biggestGainer?.pilot_name || "—"}</p>
                </div>
                {biggestGainer && (
                    <span className="bg-green-500/20 text-green-400 border border-green-500/30 px-3 py-1 rounded font-black text-sm drop-shadow-sm">
                        {biggestGainer.positions_gained > 0 ? `+${biggestGainer.positions_gained}` : biggestGainer.positions_gained}
                    </span>
                )}
            </div>
            
            {/* Outras Categorias Mini-Resumo */}
            {otherCategoriesResult?.total_races_simulated > 0 && (
                <div className="mt-auto rounded-2xl border border-white/5 bg-[#05080c] p-4 relative overflow-hidden group">
                    <div>
                        <div>
                            <p className="text-[10px] uppercase tracking-widest font-bold text-gray-500">Outras Categorias</p>
                            <p className="mt-1 text-sm font-bold text-[#58a6ff]">
                                {otherCategoriesResult.total_races_simulated} Corrida{otherCategoriesResult.total_races_simulated > 1 ? 's' : ''} Processada{otherCategoriesResult.total_races_simulated > 1 ? 's' : ''}
                            </p>
                        </div>
                    </div>
                    <div
                        className="mt-3 flex flex-wrap items-center justify-center gap-x-6 gap-y-4"
                        data-testid="other-categories-logo-strip"
                    >
                        {otherCategoriesResult.categories_simulated.map((cat) => {
                            const logoSrc = getCategorySummaryLogo(cat.category_id);
                            const logoFit = getCategorySummaryFit(cat.category_id);

                            if (!logoSrc) {
                                return (
                                    <span key={cat.category_id} className="text-[9px] uppercase font-bold tracking-widest border border-white/10 bg-white/5 px-2 py-0.5 rounded text-gray-400">
                                        {cat.category_name}
                                    </span>
                                );
                            }

                            return (
                                <span
                                    key={cat.category_id}
                                    className={[
                                        "flex h-24 w-[320px] items-center justify-center sm:h-28 sm:w-[360px]",
                                        logoFit.frameClassName,
                                    ].join(" ").trim()}
                                >
                                    <img
                                        src={logoSrc}
                                        alt={cat.category_name}
                                        className="h-full w-full object-contain"
                                        style={logoFit.imageStyle}
                                        draggable={false}
                                    />
                                </span>
                            );
                        })}
                    </div>
                </div>
            )}
        </div>

        {/* Direita: Tabela de Resultados (100% dinâmica com scroll perfeito) */}
        <div className="col-span-12 lg:col-span-9 rounded-3xl p-6 overflow-hidden flex flex-col bg-[#060a10] border border-white/5 shadow-inner relative">
             
             {/* Gradient glow interno no topo para suavizar */}
             <div className="absolute top-0 left-0 right-0 h-16 bg-gradient-to-b from-[#58a6ff]/5 to-transparent pointer-events-none"></div>

             <div className="flex justify-between items-center mb-4 border-b border-white/10 pb-4 shrink-0 px-2 relative z-10">
                 <h3 className="text-sm font-bold text-white uppercase tracking-widest opacity-90 drop-shadow-sm">
                     {rightView === "championship"
                       ? "Classificação Geral do Campeonato"
                       : rightView === "charts"
                         ? "Gráficos da Corrida"
                         : "Tabela Oficial da Prova"}
                 </h3>
                 <div className="flex items-center gap-1.5 rounded-xl border border-white/10 bg-white/5 p-1">
                     <PanelTab active={rightView === "results"} onClick={() => setRightView("results")}>Resultados</PanelTab>
                     <PanelTab active={rightView === "championship"} onClick={() => setRightView("championship")}>Campeonato</PanelTab>
                     {hasCharts && (
                       <PanelTab active={rightView === "charts"} onClick={() => setRightView("charts")}>Gráficos</PanelTab>
                     )}
                 </div>
             </div>
             
             <div className="flex-1 overflow-y-auto custom-scrollbar pr-2 relative z-10">
                 {rightView === "charts" ? (
                     <div className="animate-fade-in pr-2">
                         <RaceCharts
                             charts={telemetry.charts}
                             mistakeLap={telemetry?.mistake?.lap ?? 0}
                             bestMomentLap={telemetry?.best_moment?.lap ?? 0}
                         />
                     </div>
                 ) : rightView === "championship" ? (
                     <div className="animate-fade-in pr-2">
                         {loadingChampionship ? (
                             <div className="py-10 text-center">
                                 <p className="text-sm text-gray-400 font-mono tracking-widest uppercase animate-pulse">Consultando Federação...</p>
                             </div>
                         ) : championshipError ? (
                             <div className="bg-red-500/10 border border-red-500/30 text-red-400 px-4 py-3 rounded-xl text-sm font-mono text-center">
                                 {championshipError}
                             </div>
                         ) : (
                             <table className="w-full text-left">
                                <thead className="text-[10px] uppercase tracking-[0.2em] text-gray-500 sticky top-0 bg-[#060a10] z-10 shadow-sm">
                                    <tr>
                                        <th className="py-4 px-2 w-[80px] text-center border-b border-white/5">POS</th>
                                        <th className="py-4 px-2 border-b border-white/5">PILOTO</th>
                                        <th className="py-4 px-2 w-[180px] border-b border-white/5">EQUIPE</th>
                                        <th className="py-4 px-2 w-24 text-center border-b border-white/5">VITÓRIAS</th>
                                        <th className="py-4 px-2 w-20 text-right pr-4 border-b border-white/5">PTS</th>
                                    </tr>
                                </thead>
                                <tbody className="text-sm font-medium divide-y divide-white/5">
                                    {championship.map((driver) => (
                                        <tr key={driver.id} className={`hover:bg-white/5 transition ${driver.is_jogador ? 'bg-[#58a6ff]/10 relative shadow-[inset_4px_0_0_#58a6ff]' : ''}`}>
                                            <td className={`py-4 px-2 text-center text-lg font-black ${driver.posicao_campeonato === 1 ? 'text-yellow-500' : driver.posicao_campeonato === 2 ? 'text-gray-300' : driver.posicao_campeonato === 3 ? 'text-orange-400' : 'text-gray-500'}`}>
                                                {driver.posicao_campeonato}
                                            </td>
                                            <td className={`py-4 px-2 font-bold ${driver.is_jogador ? 'text-[#58a6ff]' : 'text-gray-200'}`}>
                                                {driver.is_jogador ? `▶ ${driver.nome} ◀` : driver.nome}
                                            </td>
                                            <td className="py-4 px-2 text-[10px] font-bold uppercase tracking-widest text-gray-400 opacity-90">
                                                <div className="flex items-center gap-2">
                                                    {driver.equipe_cor && (
                                                        <div className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: driver.equipe_cor, boxShadow: `0 0 8px ${driver.equipe_cor}80` }}></div>
                                                    )}
                                                    <span className={`truncate max-w-[140px] ${driver.is_jogador ? 'text-[#58a6ff]' : ''}`}>{driver.equipe_nome || "-"}</span>
                                                </div>
                                            </td>
                                            <td className="py-4 px-2 text-center font-mono font-bold text-gray-400">{driver.vitorias}</td>
                                            <td className="py-4 px-2 text-right font-black font-mono text-white text-base pr-4">{driver.pontos}</td>
                                        </tr>
                                    ))}
                                </tbody>
                             </table>
                         )}
                     </div>
                 ) : (
                     <table className="w-full text-left">
                         <thead className="text-[10px] uppercase tracking-[0.16em] text-gray-500 border-b border-white/10 sticky top-0 bg-[#060a10] z-10 shadow-sm">
                             <tr>
                                 <th className="py-4 px-2 w-[110px] text-center">POS (VAR)</th>
                                 <th className="py-4 px-2 w-[240px]">PILOTO</th>
                                 <th className="py-4 px-2 w-[200px]">EQUIPE</th>
                                 <th className="py-4 px-2 text-right pr-6">TEMPO / GAP</th>
                             </tr>
                         </thead>
                         <tbody className="text-[13px] font-medium divide-y divide-white/5">
                             {result.race_results.map((entry) => {
                                 let posColor = "text-gray-500";
                                 let posSize = "text-base";
                                 if (entry.finish_position === 1) { posColor = "text-yellow-500"; posSize = "text-lg"; }
                                 else if (entry.finish_position === 2) { posColor = "text-gray-300"; posSize = "text-[17px]"; }
                                 else if (entry.finish_position === 3) { posColor = "text-orange-400"; posSize = "text-base"; }
                                 
                                 const isJogador = entry.is_jogador;
                                 if (isJogador) posColor = "text-[#58a6ff]";

                                 // Delta ao lado da Posição
                                 const delta = entry.positions_gained;
                                 let deltaStr = delta === 0 ? "-" : (delta > 0 ? `+${delta}` : `${delta}`);
                                 let deltaColor = delta === 0 ? "text-gray-600 font-medium" : (delta > 0 ? "text-green-400 font-bold" : "text-red-400/80 font-bold");

                                 return (
                                     <tr key={entry.pilot_id} className={`hover:bg-white/5 transition ${isJogador ? 'bg-[#58a6ff]/10 relative shadow-[inset_4px_0_0_#58a6ff]' : entry.is_dnf ? 'bg-red-500/5 opacity-80' : 'bg-white/[0.01]'}`}>
                                         
                                         {/* Coluna combinada POS + Delta */}
                                         <td className="py-4 px-2 text-center align-middle">
                                            <div className="flex items-center justify-center gap-2">
                                                <span className={`font-black w-6 text-right ${entry.is_dnf ? 'text-red-500 text-xs tracking-widest uppercase' : posColor + ' ' + posSize}`}>
                                                    {entry.is_dnf ? 'DNF' : entry.finish_position}
                                                </span>
                                                {!entry.is_dnf && (
                                                    <span className={`text-[10px] min-w-[20px] text-left ${deltaColor}`}>
                                                        {delta > 0 ? `▲${deltaStr.replace('+','')}` : delta < 0 ? `▼${deltaStr.replace('-','')}` : '—'}
                                                    </span>
                                                )}
                                            </div>
                                         </td>
                                         
                                         <td className={`py-4 px-2 font-bold flex items-center gap-2 ${entry.is_dnf ? 'line-through text-gray-500' : isJogador ? 'text-[#58a6ff] text-sm' : 'text-gray-200 text-sm'}`}>
                                            {entry.has_fastest_lap && !entry.is_dnf && <span className="animate-pulse drop-shadow-md pb-[2px]" title="Volta mais rápida">⚡</span>}
                                            {isJogador ? `▶ ${entry.pilot_name} ◀` : entry.pilot_name}
                                         </td>
                                         
                                         <td className={`py-4 px-2 text-[11px] uppercase tracking-widest ${isJogador ? 'font-black text-[#58a6ff] opacity-80' : 'text-gray-400 font-bold'}`}>
                                            <div className="flex items-center gap-2">
                                                <TeamLogoMark
                                                    teamName={entry.team_name}
                                                    color={teamColors[entry.team_name] ?? null}
                                                    size="xs"
                                                    testId="official-race-team-logo"
                                                />
                                                <span className="truncate max-w-[170px]">{entry.team_name}</span>
                                            </div>
                                         </td>
                                         
                                         <td className={`py-4 px-2 text-right font-mono pr-6 ${entry.is_dnf ? 'text-red-500 text-[10px] font-bold tracking-widest uppercase' : entry.finish_position === 1 ? 'text-yellow-500 font-bold' : isJogador ? 'text-white font-bold' : 'text-gray-400'}`}>
                                             {entry.is_dnf 
                                                ? "Abandonou" 
                                                : entry.finish_position === 1 
                                                    ? formatLapTime(entry.total_race_time_ms) 
                                                    : formatGap(entry.gap_to_winner_ms)}
                                         </td>

                                     </tr>
                                 );
                             })}
                         </tbody>
                     </table>
                 )}
             </div>
        </div>

      </div>

    </div>
  );
}

export default RaceResultView;
