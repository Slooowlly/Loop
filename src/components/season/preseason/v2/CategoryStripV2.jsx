import { useTranslation } from "react-i18next";
import Tooltip from "../../../ui/Tooltip";
import {
  subcatLabel,
  subcatColor,
  subcatLogo,
  subcatLogoFit,
} from "../../preSeasonFormatters.js";

// Cabeçalho de categoria do v2.
//
// A faixa fina que estava aqui era eficiente e sem graça. O pôster do v1 é o que
// dá presença à tela: o logo grande é o que diz "você está olhando a Mazda Cup"
// antes de qualquer texto. Ele volta — em altura menor, porque nove categorias
// empilhadas na visão "Todas" não podem custar uma rolagem inteira cada uma —
// e agora carrega os números que o v1 não tinha.
//
// O logo usa os MESMOS presets de enquadramento do v1 (`subcatLogoFit`). Cada arte
// tem margem interna e proporção próprias; a tabela de correção já existia e é ela
// que faz todas ocuparem o mesmo espaço óptico. Só a moldura encolhe.
const COMPACT_FRAME = "h-24 lg:h-28";

export default function CategoryStripV2({ categoryKey, teamCount, seatTotal, seatsOpen, seatsAtRisk }) {
  const { t } = useTranslation();
  const label = subcatLabel(categoryKey);
  const color = subcatColor(categoryKey);
  const logo = subcatLogo(categoryKey);
  const logoFit = subcatLogoFit(categoryKey);
  // Três fatias da mesma barra. `safe` é o que sobra: assento ocupado por contrato
  // que não vence e piloto que não se aposenta. O max(0) protege contra um save em
  // que o backend conte um assento nas duas listas — a barra encolhe, não estoura.
  const total = seatTotal > 0 ? seatTotal : 1;
  const safeSeats = Math.max(0, seatTotal - seatsOpen - seatsAtRisk);
  const safePct = (safeSeats / total) * 100;
  const riskPct = (Math.min(seatsAtRisk, total) / total) * 100;
  const openPct = (Math.min(seatsOpen, total) / total) * 100;

  return (
    <div
      data-testid={`preseason-category-strip-${categoryKey}`}
      className="mb-5 flex flex-col items-center gap-3 rounded-2xl px-5 py-5"
      style={{
        background: `linear-gradient(135deg, ${color}22 0%, ${color}0a 100%)`,
        borderLeft: `3px solid ${color}`,
        boxShadow: `0 0 18px ${color}18`,
      }}
    >
      {logo ? (
        <div className={`flex w-full items-start justify-center overflow-hidden ${COMPACT_FRAME}`}>
          <img
            data-testid="preseason-category-logo"
            src={logo}
            alt={label}
            className="h-full w-auto max-w-none object-contain"
            style={logoFit.imageStyle}
            draggable={false}
          />
        </div>
      ) : (
        <span
          className="text-[22px] font-black uppercase tracking-[0.18em]"
          style={{ color }}
        >
          {label}
        </span>
      )}

      {/* A barra É o número.
          Três segmentos na ordem em que o grid se desfaz: ocupado firme na cor da
          categoria, tracejado âmbar no que pode abrir, verde no que já está livre.
          Um bloco de dígitos ao lado ("0 +13?") obrigava o jogador a converter dois
          números numa imagem que a barra já desenha — a legenda embaixo nomeia as
          cores e pronto. */}
      <div className="flex w-full max-w-[620px] flex-col items-center gap-2">
        <div className="flex w-full items-center gap-4">
          <span className="shrink-0 text-[10px] font-bold uppercase tracking-[0.14em] text-[color:var(--text-muted)]">
            {t("preSeason.grid.teamCount", { count: teamCount })}
          </span>
          <div className="flex h-[8px] min-w-0 flex-1 overflow-hidden rounded-sm bg-black/35">
            <div
              style={{
                width: `${safePct}%`,
                background: `linear-gradient(90deg, ${color}cc, ${color}77)`,
              }}
            />
            <Tooltip texto={t("preSeason.v2.grid.mayOpenTooltip", { count: seatsAtRisk })}>
              <span
                className="block h-full"
                style={{
                  width: `${riskPct}%`,
                  background:
                    "repeating-linear-gradient(90deg, rgba(210,153,34,0.95) 0 4px, rgba(210,153,34,0.25) 4px 8px)",
                }}
              />
            </Tooltip>
            <div
              style={{
                width: `${openPct}%`,
                background: "linear-gradient(90deg, rgba(63,185,80,1), rgba(63,185,80,0.7))",
              }}
            />
          </div>
        </div>

        <div className="flex items-center gap-4 text-[9.5px] font-bold">
          <span className="flex items-center gap-1.5 text-[color:var(--text-muted)]">
            <span className="h-2 w-2 rounded-[2px]" style={{ background: `${color}cc` }} />
            {t("preSeason.v2.grid.seatsFilled", { filled: seatTotal - seatsOpen, total: seatTotal })}
          </span>
          {seatsAtRisk > 0 && (
            <span className="flex items-center gap-1.5" style={{ color: "var(--status-yellow)" }}>
              <span
                className="h-2 w-2 rounded-[2px]"
                style={{
                  background:
                    "repeating-linear-gradient(90deg, rgba(210,153,34,0.95) 0 2px, rgba(210,153,34,0.25) 2px 4px)",
                }}
              />
              {t("preSeason.v2.grid.atRiskCount", { count: seatsAtRisk })}
            </span>
          )}
          <span
            className="flex items-center gap-1.5"
            style={{ color: seatsOpen > 0 ? "var(--status-green)" : "var(--text-muted)" }}
          >
            <span
              className="h-2 w-2 rounded-[2px]"
              style={{ background: seatsOpen > 0 ? "var(--status-green)" : "rgba(255,255,255,0.14)" }}
            />
            {/* "0 vaga" (o plural do português trata zero como singular) lê como erro
                de digitação numa legenda. Zero tem rótulo próprio. */}
            {seatsOpen > 0
              ? t("preSeason.grid.vacancies", { count: seatsOpen })
              : t("preSeason.v2.grid.noVacancy")}
          </span>
        </div>
      </div>
    </div>
  );
}
