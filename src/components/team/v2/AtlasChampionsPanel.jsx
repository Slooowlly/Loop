import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import TeamLogoMark from "../TeamLogoMark";
import { bandAccent, ensureMinimumLuminance } from "./atlasV2Geometry";
import { getReadableWorldTeamColor } from "../worldTeamChartGeometry";
import bronzeTrophy from "../../../assets/utilities/trophies/bronze.png";
import goldTrophy from "../../../assets/utilities/trophies/ouro.png";
import silverTrophy from "../../../assets/utilities/trophies/prata.png";

// Pódio das dinastias: a taça diz a posição antes de o número ser lido — ouro
// grande para quem mais venceu, prata e bronze menores para o 2º e o 3º.
// Cor da equipe como TEXTO: a cor bruta serve para borda, logo e brilho, mas nomes
// escuros (Obsidian, Stratos, Stuttgart) somem sobre o azul-marinho. Mesmo par que
// o Atlas usa nos rótulos — clareia até o piso de luminosidade sem trocar o matiz.
const nomeColor = (cor) => ensureMinimumLuminance(getReadableWorldTeamColor(cor));

const DYNASTY_TROPHIES = [
  { src: goldTrophy, size: 62, opacity: 0.42 },
  { src: silverTrophy, size: 50, opacity: 0.36 },
  { src: bronzeTrophy, size: 40, opacity: 0.32 },
];

// Salão dos campeões de uma faixa. Abre pelo troféu do cabeçalho do card lateral e
// responde duas perguntas que o ranking sozinho não responde: quem ganhou em cada
// ano, e quem manda nesta categoria ao longo do tempo.
//
// Portal para o body pelo mesmo motivo do Atlas: `.tab-pane-fade` anima transform,
// e um ancestral transformado vira bloco de contenção de `position: fixed`. O z é
// acima do próprio Atlas (z-[85]) e da gaveta de equipe (z-[90]/[91]).
export function AtlasChampionsPanel({ careerId, band, onClose }) {
  const { t } = useTranslation();
  const [payload, setPayload] = useState(null);
  const [error, setError] = useState("");
  const bandKey = band?.key;

  useEffect(() => {
    if (!careerId || !bandKey) return undefined;
    let cancelled = false;
    setPayload(null);
    setError("");
    invoke("get_band_champions", { careerId, bandKey })
      .then((data) => {
        if (!cancelled) setPayload(data);
      })
      .catch((cause) => {
        if (!cancelled) setError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [careerId, bandKey]);

  // Esc fecha: é um painel modal, e sair dele não pode depender de acertar o X.
  useEffect(() => {
    const onKeyDown = (event) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  if (!band) return null;

  const accent = bandAccent(band);
  const seasons = payload?.seasons ?? [];
  const dynasties = payload?.dynasties ?? [];
  // A temporada mais recente é a leitura que o jogador procura primeiro: ganha
  // barra lateral, borda ciano e fundo mais claro. Max explícito porque a ordem
  // da lista é do backend, não uma garantia deste componente.
  const latestYear = seasons.length ? Math.max(...seasons.map((season) => season.year)) : null;

  return createPortal(
    <div
      data-testid="atlas-v2-champions"
      role="dialog"
      aria-modal="true"
      aria-label={t("globalTeams.championsTitle", { band: band.label })}
      className="fixed inset-0 z-[95] grid place-items-center bg-[rgba(2,6,14,0.72)] p-8"
      onClick={onClose}
    >
      <section
        className="flex max-h-full w-full max-w-[720px] flex-col overflow-hidden rounded-2xl border border-[#2b4266] bg-[linear-gradient(180deg,#0d1a2c_0%,#0a1421_100%)] shadow-[0_24px_60px_rgba(0,0,0,0.55)]"
        onClick={(event) => event.stopPropagation()}
      >
        <header
          className="flex shrink-0 items-center gap-4 border-b border-[#2b4266] px-6 py-5"
          style={{ background: `linear-gradient(180deg, color-mix(in srgb, ${accent} 13%, transparent) 0%, transparent 100%)` }}
        >
          <span
            aria-hidden="true"
            className="grid h-12 w-12 shrink-0 place-items-center rounded-xl border text-[22px] shadow-[inset_0_1px_0_rgba(255,255,255,0.09)]"
            style={{
              borderColor: `color-mix(in srgb, ${accent} 45%, transparent)`,
              background: `linear-gradient(180deg, color-mix(in srgb, ${accent} 26%, transparent) 0%, color-mix(in srgb, ${accent} 7%, transparent) 100%)`,
            }}
          >
            🏆
          </span>
          <div className="min-w-0 flex-1">
            <h2 className="truncate text-[21px] font-bold uppercase leading-tight tracking-[0.06em]" style={{ color: accent }}>
              {t("globalTeams.championsTitle", { band: payload?.band_label ?? band.label })}
            </h2>
            <p className="mt-1 truncate text-[12.5px] text-slate-400">{t("globalTeams.championsSubtitle")}</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t("globalTeams.championsClose")}
            className="grid h-10 w-10 shrink-0 place-items-center rounded-xl border border-[#2b4266] bg-[#0a1524] text-[17px] text-slate-400 transition-colors hover:border-[#3d5c85] hover:bg-white/[0.07] hover:text-slate-100"
          >
            <span aria-hidden="true" className="leading-none">✕</span>
          </button>
        </header>

        {/* O pódio das dinastias antes da lista: é o que responde "quem manda nesta
            categoria" sem obrigar a contar linha por linha. Só aparece quando há
            mais de uma campeã — com uma só, o agregado repete a lista. */}
        {dynasties.length > 1 ? (
          <div
            data-testid="atlas-v2-champions-dynasties"
            className="grid shrink-0 gap-3 border-b border-[#1c2f47] px-6 py-4"
            style={{ gridTemplateColumns: `repeat(${Math.min(dynasties.length, 3)}, minmax(0, 1fr))` }}
          >
            {dynasties.slice(0, 3).map((dynasty, rank) => (
              <div
                key={dynasty.team_id}
                className="relative flex items-center gap-3 overflow-hidden rounded-xl border px-3.5 py-3"
                style={{
                  borderColor: `color-mix(in srgb, ${dynasty.cor_primaria} 38%, transparent)`,
                  background: `linear-gradient(150deg, color-mix(in srgb, ${dynasty.cor_primaria} 12%, #0a1524) 0%, #0a1524 62%)`,
                  boxShadow: `inset 0 1px 0 rgba(255,255,255,0.05), 0 0 22px -10px color-mix(in srgb, ${dynasty.cor_primaria} 70%, transparent)`,
                }}
              >
                {/* Taça ao fundo, no metal e no tamanho da posição: ouro maior,
                    prata e bronze menores. Fica atrás do número, que continua
                    sendo a informação da vez. */}
                <img
                  src={DYNASTY_TROPHIES[rank].src}
                  alt=""
                  aria-hidden="true"
                  draggable={false}
                  className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 object-contain"
                  style={{
                    height: DYNASTY_TROPHIES[rank].size,
                    width: DYNASTY_TROPHIES[rank].size,
                    opacity: DYNASTY_TROPHIES[rank].opacity,
                  }}
                />
                <TeamLogoMark teamName={dynasty.nome} color={dynasty.cor_primaria} size="sm" />
                <div className="relative min-w-0 flex-1">
                  <span className="block truncate text-[12.5px] font-semibold leading-tight" style={{ color: nomeColor(dynasty.cor_primaria) }}>
                    {dynasty.nome}
                  </span>
                  <p className="mt-1 flex items-baseline gap-1.5 leading-none">
                    <span className="font-mono text-[24px] font-bold text-slate-50">{dynasty.titles}</span>
                    <span className="text-[11px] text-slate-400">{t("globalTeams.championsTitles")}</span>
                  </p>
                </div>
              </div>
            ))}
          </div>
        ) : null}

        <div className="grid shrink-0 grid-cols-[60px_minmax(0,1.1fr)_minmax(0,1.25fr)_78px] gap-5 border-b border-[#1c2f47] bg-[#0a1524] px-6 py-3 text-[10.5px] font-bold uppercase tracking-[0.14em] text-slate-500">
          <span>{t("globalTeams.championsYear")}</span>
          <span>{t("globalTeams.championsTeam")}</span>
          <span>{t("globalTeams.championsDriver")}</span>
          <span className="text-right">{t("globalTeams.championsWins")}</span>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {error ? <p className="px-6 py-8 text-center text-[12px] text-[#ff6b66]">{error}</p> : null}

          {!error && !payload ? (
            <p className="px-6 py-8 text-center text-[12px] text-slate-500">{t("globalTeams.championsLoading")}</p>
          ) : null}

          {payload && seasons.length === 0 ? (
            <p className="px-6 py-8 text-center text-[12px] text-slate-500">{t("globalTeams.championsEmpty")}</p>
          ) : null}

          {seasons.map((season, index) => {
            const isLatest = season.year === latestYear;
            return (
              <div
                key={`${season.year}-${season.team_id}`}
                data-testid={`atlas-v2-champion-${season.year}`}
                className={[
                  "relative grid grid-cols-[60px_minmax(0,1.1fr)_minmax(0,1.25fr)_78px] items-center gap-5 border-b border-[#16273c] px-6 py-3.5 last:border-b-0",
                  isLatest
                    ? "bg-[linear-gradient(90deg,rgba(56,189,248,0.14)_0%,rgba(56,189,248,0.04)_55%,transparent_100%)] ring-1 ring-inset ring-[#2f6f9e]"
                    : index % 2 === 1
                      ? "bg-white/[0.015]"
                      : "",
                ].join(" ")}
              >
                {isLatest ? (
                  <span aria-hidden="true" className="absolute inset-y-1.5 left-0 w-[3px] rounded-r-full bg-[#38bdf8] shadow-[0_0_10px_rgba(56,189,248,0.5)]" />
                ) : null}
                <span className={`font-mono text-[12.5px] ${isLatest ? "text-slate-100" : "text-slate-400"}`}>{season.year}</span>
                <span className="flex min-w-0 items-center gap-2.5">
                  <TeamLogoMark teamName={season.nome} color={season.cor_primaria} size="xs" />
                  <span className="truncate text-[12.5px] font-semibold" style={{ color: nomeColor(season.cor_primaria) }}>
                    {season.nome}
                  </span>
                </span>
                <ChampionDrivers drivers={season.drivers} />
                <span className={`text-right font-mono text-[13px] ${isLatest ? "font-bold text-slate-50" : "text-slate-400"}`}>{season.wins}</span>
              </div>
            );
          })}
        </div>
      </section>
    </div>,
    document.body,
  );
}

// A dupla da equipe naquele ano — quem ganhou o título COM ela. A estrela marca
// quem também levou o título de pilotos da categoria; os dois são independentes, e
// sem a marca não haveria como distinguir um do outro na mesma célula.
function ChampionDrivers({ drivers }) {
  const { t } = useTranslation();
  if (!drivers?.length) {
    return (
      <span className="text-[12px] text-slate-600" aria-hidden="true">
        —
      </span>
    );
  }
  return (
    <span className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-0.5">
      {drivers.map((driver) => (
        <span
          key={driver.driver_id}
          className={[
            "flex min-w-0 items-center gap-1.5 text-[12.5px]",
            driver.is_season_champion ? "font-semibold text-[#f2c46d]" : "text-slate-300",
          ].join(" ")}
        >
          <span className="truncate">{driver.nome}</span>
          {driver.is_season_champion ? (
            <img
              src={goldTrophy}
              alt=""
              draggable={false}
              className="h-3.5 w-3.5 shrink-0 object-contain drop-shadow-[0_0_8px_rgba(242,196,109,0.35)]"
              title={t("globalTeams.championsDriverTitle")}
            />
          ) : null}
        </span>
      ))}
    </span>
  );
}

export default AtlasChampionsPanel;
