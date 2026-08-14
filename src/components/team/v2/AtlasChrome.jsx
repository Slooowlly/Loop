import { useTranslation } from "react-i18next";

import Tooltip from "../../ui/Tooltip";
import goldTrophy from "../../../assets/utilities/trophies/ouro.png";

// Cabeçalho, legenda e rodapé do Atlas v2 — a moldura em volta dos dados.

export function AtlasHeader({ title, families, selectedFamily, zoomYears, zoomLabel, zoomTitle, onSelectFamily, onToggleZoom }) {
  const { t } = useTranslation();
  return (
    <header className="flex shrink-0 items-end justify-between gap-6 px-5 py-3">
      <div className="min-w-0">
        <p className="text-[10px] font-semibold uppercase tracking-[0.26em] text-[#7fb4e8]">
          {t("globalTeams.historicAtlas")}
        </p>
        <h2 className="mt-1 truncate text-[28px] font-bold leading-tight text-slate-50">{title}</h2>
      </div>

      <div className="flex shrink-0 flex-nowrap items-center gap-1.5">
        <Tooltip texto={zoomTitle}>
          <button
            type="button"
            aria-pressed={zoomYears != null}
            onClick={onToggleZoom}
            className={`rounded-full border px-3.5 py-2 text-[11px] font-bold uppercase tracking-[0.12em] transition-colors ${
              zoomYears != null
                ? "border-[#f0a04b]/60 bg-[#f0a04b]/15 text-[#f0a04b]"
                : "border-[#2a3f5c] bg-white/[0.03] text-slate-400 hover:text-slate-200"
            }`}
          >
            {zoomLabel}
          </button>
        </Tooltip>
        {families.map((family) => {
          const isActive = family.id === selectedFamily;
          return (
            <button
              key={family.id}
              type="button"
              aria-pressed={isActive}
              onClick={() => onSelectFamily(family.id)}
              className={`rounded-full border px-4 py-2 text-[11px] font-bold uppercase tracking-[0.12em] transition-colors ${
                isActive
                  ? "border-[#f0a04b] bg-[#f0a04b]/18 text-[#f5b877] shadow-[0_0_14px_rgba(240,160,75,0.22)]"
                  : "border-[#2a3f5c] bg-white/[0.03] text-slate-400 hover:border-[#3a557a] hover:text-slate-200"
              }`}
            >
              {family.label}
            </button>
          );
        })}
      </div>
    </header>
  );
}

// Esta legenda explica o GRÁFICO e os dois símbolos que a coluna lateral usa. As
// setas de variação saíram junto com o indicador de movimentação: a linha do
// ranking mostra agora só os títulos da própria categoria, e explicar um símbolo
// que não existe mais era pior que não ter legenda.
export function AtlasLegend({ showLive = false }) {
  const { t } = useTranslation();
  const items = [
    { key: "entry", glyph: <span className="text-slate-300">○</span>, label: t("globalTeams.legendEntry") },
    {
      key: "titles",
      glyph: <img src={goldTrophy} alt="" draggable={false} className="h-3.5 w-3.5 object-contain" />,
      label: t("globalTeams.legendTitles"),
    },
    {
      key: "outside",
      glyph: (
        <span
          className="inline-block h-2.5 w-4 rounded-[2px] border border-white/[0.12]"
          style={{ background: "repeating-linear-gradient(135deg,rgba(148,163,184,0.22) 0 3px,rgba(148,163,184,0.04) 3px 6px)" }}
        />
      ),
      label: t("globalTeams.legendOutside"),
    },
  ];

  // A entrada "em andamento" só aparece quando há uma temporada em curso — numa
  // carreira entre temporadas ela explicaria um símbolo que não está na tela.
  if (showLive) {
    items.push({
      key: "live",
      // O glifo é o traço interrompido do gráfico, não o selo do card: quem lê a
      // legenda está olhando para as linhas.
      glyph: (
        <span
          className="inline-block h-0.5 w-4 rounded-full"
          style={{ background: "repeating-linear-gradient(90deg,#4ade80 0 5px,transparent 5px 9px)" }}
        />
      ),
      label: t("globalTeams.legendLive"),
    });
  }

  // Barra própria, encostada na base do card do gráfico: a legenda pertence ao
  // gráfico, então lê como rodapé dele e não como um texto solto na página.
  return (
    <div
      data-testid="atlas-v2-legend"
      className="mt-2 flex shrink-0 flex-wrap items-center justify-center gap-x-8 gap-y-1.5 rounded-lg border border-[#22364f]/70 bg-[#0a1524]/60 px-5 py-2.5 text-[12px] text-slate-400"
    >
      {items.map((item) => (
        <span key={item.key} className="flex items-center gap-2">
          <span aria-hidden="true" className="grid w-4 place-items-center text-[12px]">{item.glyph}</span>
          {item.label}
        </span>
      ))}
    </div>
  );
}

// Rodapé: marca e metadados à esquerda, contexto e controles à direita.
//
// O "voltar" já foi tratado como secundário — um ícone de 28px, cinza-escuro, com
// o rótulo escondido no `title`. Não é secundário coisa nenhuma: o Atlas ocupa a
// viewport inteira, cobre o Dashboard e não tem outra saída na tela. Um ícone que
// só se explica depois de meio segundo de hover é a diferença entre sair daqui e
// ficar preso.
export function AtlasFooter({ teamCount, bandCount, familyLabel, lastDataYear, updatedAt, currentSeason, onBack }) {
  const { t } = useTranslation();
  return (
    <footer className="flex h-12 shrink-0 items-center justify-between gap-4 border-t border-[#22364f] bg-[linear-gradient(180deg,#0a1524_0%,#070f1b_100%)] px-5">
      <div className="flex min-w-0 items-center gap-4">
        {/* Rótulo visível, borda e alvo de ~32px de altura: a saída tem de ser
            legível em repouso, sem depender de hover nem de tooltip. */}
        <button
          type="button"
          data-testid="atlas-v2-back"
          onClick={onBack}
          className="flex shrink-0 cursor-pointer items-center gap-2 rounded-lg border border-[#2a3f5c] bg-white/[0.04] px-3 py-1.5 text-[11.5px] font-bold uppercase tracking-[0.1em] text-slate-300 transition-colors hover:border-[#3a557a] hover:bg-white/[0.08] hover:text-slate-100"
        >
          <span aria-hidden="true" className="text-[14px] leading-none">←</span>
          {t("globalTeams.backToStandings")}
        </button>

        {/* i18n-ignore — marca do painel, igual nos dois idiomas. */}
        <span aria-hidden="true" className="shrink-0 text-[13px] font-bold uppercase tracking-[0.42em] text-slate-200">
          Atlas
        </span>

        <span aria-hidden="true" className="h-4 w-px shrink-0 bg-[#22364f]" />

        <span className="truncate text-[11.5px] text-slate-500">
          {t("globalTeams.footerUpdated")}{" "}
          <strong className="font-mono text-slate-300">{updatedAt || lastDataYear}</strong>
        </span>

      </div>

      <div className="flex shrink-0 items-center gap-4">
        {/* O resumo é o único dado do rodapé que fala do conteúdo da tela — quantas
            equipes e quantos campeonatos estão desenhados. Fica junto do nome da
            categoria, que é o contexto que ele qualifica. */}
        <span className="truncate text-[13px] font-medium uppercase tracking-[0.08em] text-slate-300">
          {t("globalTeams.footerSummary", { teams: teamCount, bands: bandCount })}
        </span>
        <span aria-hidden="true" className="h-4 w-px bg-[#22364f]" />
        <span className="truncate text-[13px] font-semibold uppercase tracking-[0.1em] text-slate-300">{familyLabel}</span>
        <span aria-hidden="true" className="h-4 w-px bg-[#22364f]" />
        <span className="text-[11.5px] text-slate-500">
          {t("globalTeams.currentSeason")}
          <strong className="ml-1.5 font-mono text-[12.5px] text-slate-100">{currentSeason}</strong>
        </span>
      </div>
    </footer>
  );
}
