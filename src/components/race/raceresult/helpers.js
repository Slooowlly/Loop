// Funções puras de formatação/copy da tela de resultado V1 (RaceResultView).
import { CATEGORY_SUMMARY_FITS, CATEGORY_SUMMARY_LOGOS } from "./constants";

export function weatherLabel(value) {
  if (value === "HeavyRain") return "Chuva forte";
  if (value === "Wet") return "Chuva";
  if (value === "Damp") return "Úmido";
  return "Seco";
}

// Caixa da nota colorida pela faixa.
export function gradeBox(grade) {
  if (grade >= 7.5) return "border-green-500/30 bg-green-500/10 text-green-400";
  if (grade >= 6.0) return "border-[#58a6ff]/30 bg-[#58a6ff]/10 text-[#58a6ff]";
  if (grade >= 4.5) return "border-amber-500/30 bg-amber-500/10 text-amber-400";
  return "border-red-500/30 bg-red-500/10 text-red-400";
}

// Delta em segundos a partir de ms (ex.: "+0.82s" / "-0.30s"). "—" se ~zero.
export function fmtDeltaS(ms) {
  if (ms == null || Math.abs(ms) < 5) return "—";
  const s = (ms / 1000).toFixed(2);
  return `${ms > 0 ? "+" : ""}${s}s`;
}

// Frase de cobertura: deixa claro quanto da corrida foi de fato analisado, para
// o jogador não achar que a análise cobriu a prova toda quando ele saiu cedo.
export function coverageNote(t) {
  if (!t) return "";
  if (t.is_partial && t.last_lap_seen > 0) {
    return `Análise parcial — telemetria registrada até a volta ${t.last_lap_seen}.`;
  }
  if (t.race_laps > 0) {
    return `Análise baseada em ${t.laps_seen} de ${t.race_laps} voltas registradas.`;
  }
  return `Análise baseada em ${t.laps_seen} ${t.laps_seen === 1 ? "volta registrada" : "voltas registradas"}.`;
}

// Erro mais caro (2b-2) → título + frase + visual, a partir de kind + números.
// Sempre "estimado"; nunca promete mais certeza do que temos.
export function mistakeCard(m) {
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
export function bestMomentCard(b) {
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
export function breakdownSentence(b) {
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

export function getCategorySummaryLogo(categoryId) {
  return typeof categoryId === "string" ? CATEGORY_SUMMARY_LOGOS[categoryId] ?? null : null;
}

export function getCategorySummaryFit(categoryId) {
  if (typeof categoryId !== "string") {
    return { frameClassName: "", imageStyle: {} };
  }

  return CATEGORY_SUMMARY_FITS[categoryId] ?? { frameClassName: "", imageStyle: {} };
}

// Breakdown de posições (2b-1). Nível 1 = só tabela oficial (sempre funciona,
// honesto). Nível 2 = enriquecido com a trajetória da telemetria (estimado).
// O SALDO (grid → chegada) é sempre oficial; o resto é estimativa.
export function computePositionBreakdown(playerResult, result, telemetry) {
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
}
