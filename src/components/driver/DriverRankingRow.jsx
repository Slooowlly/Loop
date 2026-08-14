import FlagIcon from "../ui/FlagIcon";
import Tooltip from "../ui/Tooltip";
import TeamLogoMark from "../team/TeamLogoMark";
import i18n from "../../i18n/index.js";
import { formatMoneyCompact } from "../../utils/formatters";
import {
  formatIndex,
  formatRank,
  formatYears,
  podiumBreakdownTitle,
  statusClass,
  statusTitle,
  teamCategoryLabel,
} from "./globalDriverFormatters";

// Gravidades de lesão que o backend manda (`lesao_ativa_tipo`, o enum `InjuryType`). O valor
// que chegava aqui era a grafia do BANCO e ia CRU pro tooltip — português fixo, fora do i18n.
const INJURY_SEVERITIES = new Set(["light", "moderate", "severe", "critical"]);

// Rótulo da gravidade da lesão no idioma ativo. Gravidade que não conhecemos (save antigo,
// cache velho) some do tooltip em vez de aparecer como chave técnica.
function injurySeverityLabel(severity) {
  if (!severity || !INJURY_SEVERITIES.has(severity)) {
    return null;
  }
  return i18n.t(`globalDrivers.row.injurySeverity.${severity}`);
}

// Cabeçalho clicável de uma coluna do ranking. O marcador mostra a direção atual.
export function SortableHeader({ label, sortKey, sort, onSort, className = "px-4 py-3" }) {
  const active = sort.key === sortKey;
  const marker = active ? (sort.direction === "asc" ? "↑" : "↓") : "↕";
  return (
    <th className={className}>
      <button
        type="button"
        onClick={() => onSort(sortKey)}
        className="inline-flex items-center gap-1 rounded-lg text-left transition-glass hover:text-text-primary"
      >
        <span>{label}</span>
        <span className={active ? "text-accent-primary" : "text-text-muted"}>{marker}</span>
      </button>
    </th>
  );
}

// Faixa divisória entre "atualmente na categoria" e "já passaram por ela".
export function CategorySectionRow({ label }) {
  return (
    <tr className="border-y border-accent-primary/15 bg-accent-primary/[0.06]">
      <td colSpan={17} className="px-4 py-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent-primary">
        {label}
      </td>
    </tr>
  );
}

export function DriverRankingRow({ row, relativeEntry, focusedDriverId, detailDriverId, glowing = false, onFocus, onOpenDriverDetail, onOpenTitles, onToggleFavorite }) {
  const isDetailDriver = row.id === detailDriverId;
  const isReranked = relativeEntry != null;
  return (
    <tr
      // Âncora para a tabela rolar até o piloto que chegou em foco vindo da
      // ficha — a linha certa está a 200 posições do topo em ranking de 610.
      data-driver-id={row.id}
      data-testid={glowing ? "driver-row-glow" : undefined}
      onClick={() => onFocus(row.id)}
      onDoubleClick={() => {
        onFocus(row.id);
        onOpenDriverDetail(row.id);
      }}
      className={[
        "cursor-pointer border-b border-white/[0.06] last:border-0 transition-glass hover:bg-white/[0.04]",
        // O halo da chegada vem ANTES dos realces de estado na lista de classes
        // só por leitura; quem decide é a animação, que sobrepõe o `bg` das
        // outras enquanto corre e devolve a linha ao normal quando acaba.
        glowing ? "animate-driver-row-glow" : "",
        row.id === focusedDriverId ? "bg-accent-primary/[0.12] ring-1 ring-accent-primary/40" : "",
        isDetailDriver ? "bg-accent-secondary/[0.12] ring-2 ring-accent-secondary/60 shadow-[inset_4px_0_0_rgba(242,196,109,0.95)]" : "",
        row.is_jogador ? "border-l-2 border-l-accent-primary/70" : "",
        row.status === "Livre" && !isDetailDriver ? "opacity-60" : "",
        row.status === "Aposentado" && !isDetailDriver ? "opacity-50" : "",
      ].join(" ")}
    >
      <td className="py-3 pr-4 font-mono text-xs text-text-muted">
        <RankCell
          rank={isReranked ? relativeEntry.rank : row.historical_rank}
          delta={isReranked ? relativeEntry.delta : row.historical_rank_delta}
          globalRank={isReranked ? row.historical_rank : null}
          scoped={isReranked}
        />
      </td>
      <td className="px-4 py-3">
        <div className="flex min-w-[190px] items-center gap-2">
          {row.nacionalidade ? <FlagIcon nacionalidade={row.nacionalidade} /> : null}
          <span
            onDoubleClick={(event) => {
              event.stopPropagation();
              onOpenDriverDetail(row.id);
            }}
            className={row.is_jogador ? "font-semibold text-accent-primary" : "font-semibold text-text-primary"}
          >
            {row.nome}
          </span>
          {row.is_jogador ? (
            <span className="rounded-full border border-accent-primary/30 bg-accent-primary/10 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.14em] text-accent-primary">
              Voce
            </span>
          ) : null}
          {row.is_lesionado ? (
            <Tooltip
              texto={injurySeverityLabel(row.lesao_ativa_tipo) ? i18n.t("globalDrivers.row.injuredType", { type: injurySeverityLabel(row.lesao_ativa_tipo) }) : i18n.t("globalDrivers.row.injured")}
            >
              <span className="rounded-full border border-status-red/25 bg-status-red/10 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-status-red">
                Lesionado
              </span>
            </Tooltip>
          ) : null}
          {row.is_jogador ? null : (
            <Tooltip
              texto={row.is_favorito ? i18n.t("globalDrivers.row.removeFavorite") : i18n.t("globalDrivers.row.addFavorite")}
            >
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  onToggleFavorite?.(row.id);
                }}
                onDoubleClick={(event) => event.stopPropagation()}
                aria-pressed={Boolean(row.is_favorito)}
                aria-label={row.is_favorito ? i18n.t("globalDrivers.row.removeFavorite") : i18n.t("globalDrivers.row.addFavorite")}
                className={[
                  "ml-auto text-[15px] leading-none transition-colors",
                  row.is_favorito
                    ? "text-[#fbbf24] drop-shadow-[0_0_4px_rgba(251,191,36,0.5)]"
                    : "text-text-muted/50 hover:text-[#fbbf24]",
                ].join(" ")}
              >
                {row.is_favorito ? "★" : "☆"}
              </button>
            </Tooltip>
          )}
        </div>
      </td>
      <td className="px-4 py-3">
        <span className={statusClass(row.status)}>
          {row.status}
        </span>
      </td>
      <td className="px-4 py-3">
        <div className="flex min-w-[170px] items-center gap-2">
          {row.equipe_nome ? (
            <TeamLogoMark teamName={row.equipe_nome} color={row.equipe_cor_primaria} size="xs" />
          ) : null}
          <Tooltip texto={statusTitle(row)}>
            <span className="truncate text-text-secondary">{teamCategoryLabel(row)}</span>
          </Tooltip>
        </div>
      </td>
      <MetricCell value={row.idade || "-"} />
      <td className="px-4 py-3 font-mono text-text-primary">{formatYears(row.anos_carreira)}</td>
      <td className="px-4 py-3 font-mono text-text-primary">{row.salario_anual ? formatMoneyCompact(row.salario_anual) : "-"}</td>
      <FamaCell row={row} />
      <td className="px-4 py-3 font-mono text-text-primary">{formatIndex(row.historical_index)}</td>
      <TitleMetricCell row={row} onOpenTitles={onOpenTitles} />
      <MetricCell value={row.vitorias} />
      <PodiumMetricCell row={row} />
      <MetricCell value={row.poles} />
      <MetricCell value={row.pontos} />
      <MetricCell value={row.corridas} />
      <MetricCell value={row.dnfs} />
      <MetricCell value={row.lesoes} />
    </tr>
  );
}

function MetricCell({ value }) {
  return <td className="px-4 py-3 font-mono text-text-primary">{value ?? 0}</td>;
}

function TitleMetricCell({ row, onOpenTitles }) {
  if (!row.titulos || row.titulos <= 0) {
    return <MetricCell value={row.titulos} />;
  }

  return (
    <td className="px-4 py-3 font-mono text-text-primary">
      <button
        type="button"
        aria-label={i18n.t("globalDrivers.row.viewTitlesAria", { name: row.nome })}
        onClick={(event) => {
          event.stopPropagation();
          onOpenTitles(row);
        }}
        onDoubleClick={(event) => event.stopPropagation()}
        className="rounded-md px-2 py-1 font-mono text-accent-primary underline decoration-accent-primary/40 underline-offset-4 transition-glass hover:bg-accent-primary/10 hover:text-accent-secondary"
      >
        {row.titulos}
      </button>
    </td>
  );
}

function PodiumMetricCell({ row }) {
  const podios = row.podios ?? 0;
  if (podios <= 0) {
    return <td className="px-4 py-3 font-mono text-text-primary">0</td>;
  }
  return (
    <Tooltip texto={podiumBreakdownTitle(row)}>
      <td className="px-4 py-3 font-mono text-text-primary cursor-help underline decoration-dotted decoration-white/25 underline-offset-4">
        {podios}
      </td>
    </Tooltip>
  );
}

// Célula de Fama (estrelato). Mostra a fama atual (0–100) e, quando houve
// movimento desde a temporada passada, um selo ▲/▼ com o tamanho da subida —
// é a vitrine "quem está em alta". Aposentados/sem fama caem no traço.
function FamaCell({ row }) {
  const fama = Number(row.fama ?? 0);
  const delta = Number(row.fama_delta ?? 0);
  const carismaTitle = row.carisma != null ? i18n.t("globalDrivers.fame.charisma", { carisma: row.carisma }) : "";

  if (fama <= 0) {
    return <td className="px-4 py-3 font-mono text-text-muted">-</td>;
  }

  if (!delta) {
    return (
      <Tooltip texto={i18n.t("globalDrivers.fame.title", { fama, charisma: carismaTitle })}>
        <td className="px-4 py-3 font-mono text-text-primary">{fama}</td>
      </Tooltip>
    );
  }

  const gained = delta > 0;
  const amount = Math.abs(delta);
  const deltaTitle = i18n.t("globalDrivers.fame.deltaTitle", {
    direction: gained ? i18n.t("globalDrivers.fame.up") : i18n.t("globalDrivers.fame.down"),
    amount,
    charisma: carismaTitle,
  });

  return (
    <td className="px-4 py-3 font-mono text-text-primary">
      <Tooltip texto={deltaTitle}>
        <span className="inline-flex items-center gap-1.5">
          <span>{fama}</span>
          <span
            className={[
              "whitespace-nowrap rounded-full border px-1.5 py-0.5 text-[10px] font-semibold leading-none",
              gained
                ? "border-status-green/25 bg-status-green/10 text-status-green"
                : "border-status-red/25 bg-status-red/10 text-status-red",
            ].join(" ")}
          >
            {`${gained ? "▲" : "▼"}${amount}`}
          </span>
        </span>
      </Tooltip>
    </td>
  );
}

function RankCell({ rank, delta, globalRank = null, scoped = false }) {
  const numericDelta = Number(delta ?? 0);
  const globalTitle = globalRank != null ? i18n.t("globalDrivers.rank.globalTitle", { rank: formatRank(globalRank) }) : undefined;

  if (!numericDelta) {
    return (
      <Tooltip texto={globalTitle}>
        <span>{formatRank(rank)}</span>
      </Tooltip>
    );
  }

  const gained = numericDelta > 0;
  const amount = Math.abs(numericDelta);
  const label = `${gained ? "↑" : "↓"}${amount}`;
  const scope = scoped ? i18n.t("globalDrivers.rank.scopeFiltered") : "";
  const deltaTitle = i18n.t("globalDrivers.rank.deltaTitle", {
    count: amount,
    direction: gained ? i18n.t("globalDrivers.rank.up") : i18n.t("globalDrivers.rank.down"),
    scope,
  });
  const title = globalTitle ? `${deltaTitle}. ${globalTitle}` : deltaTitle;

  return (
    <Tooltip texto={globalTitle}>
      <span className="inline-flex items-center gap-2">
        <span>{formatRank(rank)}</span>
        {/* O balão da variação ganha do balão da linha: quem mira a pílula quer
            saber quanto subiu, e o texto dela já carrega a posição global. */}
        <Tooltip texto={title}>
          <span
            className={[
              "whitespace-nowrap rounded-full border px-1.5 py-0.5 text-[10px] font-semibold leading-none",
              gained
                ? "border-status-green/25 bg-status-green/10 text-status-green"
                : "border-status-red/25 bg-status-red/10 text-status-red",
            ].join(" ")}
          >
            {label}
          </span>
        </Tooltip>
      </span>
    </Tooltip>
  );
}

export default DriverRankingRow;
