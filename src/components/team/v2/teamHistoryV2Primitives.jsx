import { Award, BarChart3, CalendarDays, Crown, Medal, TrendingUp } from "lucide-react";
import goldTrophy from "../../../assets/utilities/trophies/ouro.png";
import i18n from "../../../i18n/index.js";

// Primitivas de desenho do dossiê de equipe v2: rótulo de bloco, legenda de
// medalha, par rótulo-valor e o ícone de métrica.
//
// Extraídas de `TeamHistoryDrawerV2.jsx` em 11/08/2026, no mesmo movimento que
// tirou de lá os painéis por seção. Elas são o vocabulário compartilhado desses
// painéis: cada uma aparece em três ou mais deles, então viver dentro de um
// único painel deixaria os outros importando de um vizinho arbitrário.
//
// Nada aqui decide conteúdo. Recebem o texto já traduzido e o valor já formatado
// e devolvem a caixa — o que muda o que se lê mora em [teamHistoryV2Logic.js] e
// em [teamHistoryV2Labels.js].

// Ícone por métrica, escolhido pelo `id` do record — nunca pelo rótulo, que é
// texto traduzido. Métrica desconhecida não desenha nada: um ícone genérico
// seria pior que nenhum.
const METRIC_ICONS = {
  titles: Award,
  wins: Crown,
  podiums: Medal,
  podium_rate: BarChart3,
  win_rate: TrendingUp,
  seasons: CalendarDays,
};

export function HistoryStateMessage({ dossier }) {
  const message = dossier.historyStatus === "error" ? dossier.historyError : i18n.t("myTeamTab.history.loading");
  return <div className="mb-3 rounded-xl border border-white/10 bg-[#08111f]/95 px-4 py-2.5 text-[11px] text-text-secondary">{message}</div>;
}

export function MedalKey({ color, label }) {
  return (
    <span className="flex items-center gap-1.5">
      <span className="h-2 w-2 rounded-sm" style={{ backgroundColor: color }} />
      {label}
    </span>
  );
}

// Rótulo de bloco — a receita vale para TODO rótulo do dossiê.
//
// Era `text-[10px] uppercase tracking-[0.15em] text-text-muted`, e essa
// combinação é o pior caso possível de legibilidade: a caixa alta apaga a
// silhueta da palavra (sem ascendente nem descendente o olho perde a forma que
// usa para reconhecê-la), o espaçamento largo desmancha o que sobrou em letras
// soltas, e o cinza apagado num corpo pequeno não tem contraste para compensar.
// Cada um sozinho passaria; os quatro juntos obrigavam a soletrar.
//
// As chaves de i18n já vêm em caixa de frase, então parar de forçar `uppercase`
// devolve a capitalização certa de graça. A hierarquia não depende disso — o
// rótulo continua menor e cinza contra um valor branco e maior.
//
// NÃO reintroduza caixa alta em rótulo pequeno aqui. Se precisar de mais
// separação entre rótulo e valor, mexa no peso ou no espaçamento vertical.
export function BlockLabel({ children }) {
  return <span className="block text-[11px] font-semibold text-text-secondary">{children}</span>;
}

export function MiniMetric({ label, value }) {
  return (
    <div className="rounded-xl bg-[#0f1c2b] px-3.5 py-3">
      <span className="block truncate text-[11px] font-semibold text-text-secondary">{label}</span>
      <strong className="mt-1 block font-mono text-lg leading-none text-text-primary">{value}</strong>
    </div>
  );
}

export function InfoCard({ label, value, detail = "" }) {
  return (
    <div className="rounded-xl border border-white/10 bg-[#0c1626]/95 px-4 py-3">
      <div className="flex items-start justify-between gap-3">
        <strong className="text-xs text-text-primary">{label}</strong>
        <span className="text-right font-mono text-[11px] font-semibold text-status-yellow">{value}</span>
      </div>
      {detail ? <p className="mt-1.5 text-[11px] leading-5 text-text-secondary">{detail}</p> : null}
    </div>
  );
}

export function MetricIcon({ name, size = 15 }) {
  const Icon = METRIC_ICONS[name];
  if (!Icon) return null;
  return <Icon size={size} strokeWidth={1.5} aria-hidden="true" className="shrink-0" />;
}

// Marca-d'água dos cards de destaque: o troféu de ouro do jogo, o MESMO que a
// classificação usa no TrophyBadge. Antes era o ícone de contorno do Lucide —
// correto como ícone de 16px, magro e sem peso como ornamento de 80px. Arte de
// verdade preenche, e a tela já tinha a dela.
//
// Puro ornamento, então `alt=""` e `aria-hidden`: quem lê por leitor de tela não
// perde nada.
export function HighlightTrophy() {
  return (
    <img
      src={goldTrophy}
      alt=""
      aria-hidden="true"
      className="pointer-events-none absolute -right-4 top-1/2 h-[84px] w-[84px] -translate-y-1/2 object-contain opacity-[0.16] [filter:saturate(1.4)]"
    />
  );
}
