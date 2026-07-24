import FlagIcon from "../ui/FlagIcon";
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

export function DriverRankingRow({ row, relativeEntry, focusedDriverId, detailDriverId, onFocus, onOpenDriverDetail, onOpenTitles, onToggleFavorite }) {
  const isDetailDriver = row.id === detailDriverId;
  const isReranked = relativeEntry != null;
  return (
    <tr
      onClick={() => onFocus(row.id)}
      onDoubleClick={() => {
        onFocus(row.id);
        onOpenDriverDetail(row.id);
      }}
      className={[
        "cursor-pointer border-b border-white/6 last:border-0 transition-glass hover:bg-white/[0.04]",
        row.id === focusedDriverId ? "bg-accent-primary/12 ring-1 ring-accent-primary/40" : "",
        isDetailDriver ? "bg-accent-secondary/12 ring-2 ring-accent-secondary/60 shadow-[inset_4px_0_0_rgba(242,196,109,0.95)]" : "",
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
            <span
              title={row.lesao_ativa_tipo ? i18n.t("globalDrivers.row.injuredType", { type: row.lesao_ativa_tipo }) : i18n.t("globalDrivers.row.injured")}
              className="rounded-full border border-status-red/25 bg-status-red/10 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-status-red"
            >
              Lesionado
            </span>
          ) : null}
          {row.is_jogador ? null : (
            <button
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                onToggleFavorite?.(row.id);
              }}
              onDoubleClick={(event) => event.stopPropagation()}
              aria-pressed={Boolean(row.is_favorito)}
              title={row.is_favorito ? i18n.t("globalDrivers.row.removeFavorite") : i18n.t("globalDrivers.row.addFavorite")}
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
          <span className="truncate text-text-secondary" title={statusTitle(row)}>
            {teamCategoryLabel(row)}
          </span>
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
        aria-label={`Ver titulos de ${row.nome}`}
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
    <td
      className="px-4 py-3 font-mono text-text-primary cursor-help underline decoration-dotted decoration-white/25 underline-offset-4"
      title={podiumBreakdownTitle(row)}
    >
      {podios}
    </td>
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
      <td className="px-4 py-3 font-mono text-text-primary" title={i18n.t("globalDrivers.fame.title", { fama, charisma: carismaTitle })}>
        {fama}
      </td>
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
      <span className="inline-flex items-center gap-1.5" title={deltaTitle}>
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
    </td>
  );
}

function RankCell({ rank, delta, globalRank = null, scoped = false }) {
  const numericDelta = Number(delta ?? 0);
  const globalTitle = globalRank != null ? i18n.t("globalDrivers.rank.globalTitle", { rank: formatRank(globalRank) }) : undefined;

  if (!numericDelta) {
    return <span title={globalTitle}>{formatRank(rank)}</span>;
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
    <span className="inline-flex items-center gap-2" title={globalTitle}>
      <span>{formatRank(rank)}</span>
      <span
        title={title}
        className={[
          "whitespace-nowrap rounded-full border px-1.5 py-0.5 text-[10px] font-semibold leading-none",
          gained
            ? "border-status-green/25 bg-status-green/10 text-status-green"
            : "border-status-red/25 bg-status-red/10 text-status-red",
        ].join(" ")}
      >
        {label}
      </span>
    </span>
  );
}

export default DriverRankingRow;
