import { useState } from "react";
import { useTranslation } from "react-i18next";

import i18n from "../../i18n/index.js";
import useExitToMenu from "../../hooks/useExitToMenu";
import LeaveToMenuModal from "./LeaveToMenuModal";
import useBannerDaEtapa from "./useBannerDaEtapa";
import useFluxoDeAvanco from "./useFluxoDeAvanco";
import {
  categoryLabel,
  extractNationalityCode,
  formatCompactDate,
  formatNextRaceCountdown,
  formatSurfaceSeasonLabel,
} from "../../utils/formatters";
import { getBannerImageFocus, getBannerImageSrc } from "../../utils/trackBanners";
import { trackCountryLabel } from "../../utils/trackCountry";
import { CLIMA_BANNER, weatherEmoji, weatherLabel as climaLabel } from "../../utils/weather";
import GlassButton from "../ui/GlassButton";
import FlagIcon from "../ui/FlagIcon";
import Tooltip from "../ui/Tooltip";
import TeamLogoMark from "../team/TeamLogoMark";
import TabNavigation from "./TabNavigation";

function Header({ activeTab, onTabChange }) {
  const { t } = useTranslation();
  // Todo o fluxo de temporada por trás do botão "Avançar" (sete destinos, decididos por
  // fase + etapa pendente + equipe + aba) mora em `useFluxoDeAvanco`. O cabeçalho desenha.
  const {
    season,
    playerTeam,
    nextRace,
    showRaceBriefing,
    visibleDate,
    visibleCountdown,
    viewingOwnCategory,
    viewedCategory,
    isFreeAgent,
    hasNoPendingRace,
    bannerOwnsAdvance,
    avancoEmCurso,
    avancar,
    rotuloDoAvanco,
    fecharBriefing,
  } = useFluxoDeAvanco({ activeTab, onTabChange });

  // As duas leituras de ponte do banner: o campeão do ano encerrado e a próxima etapa de
  // OUTRA categoria, quando o jogador abre uma série que não é a dele na tabela da Home.
  const { fimDeTemporada, categoryRace, fimDeTemporadaDaCategoria } = useBannerDaEtapa({
    activeTab,
    showRaceBriefing,
    viewingOwnCategory,
    viewedCategory,
    visibleDate,
    hasNoPendingRace,
    playerCategory: playerTeam?.categoria ?? null,
  });

  // Clicar no chip da equipe abre direto a pergunta de sair (salvando ou não).
  const { isSaving, exit, saveAndExit } = useExitToMenu();
  const [leaveConfirm, setLeaveConfirm] = useState(false);

  return (
    <header className="relative z-20 flex flex-col">
      <div className="shrink-0 px-3 py-2 sm:px-4 lg:px-5 xl:px-6">
        <div className="mx-auto flex w-full max-w-[1680px] items-center">
          <div className="flex min-w-0 flex-1 items-center gap-2">
            {!showRaceBriefing && (
              <Tooltip texto={t("nav.exitToMenu")}>
                <button
                  type="button"
                  onClick={() => setLeaveConfirm(true)}
                  className="flex items-center gap-2 rounded-xl px-1.5 py-1 transition-colors hover:bg-white/[0.08]"
                >
                  <img
                    src="/utilities/Logo%20sem%20fundo.webp"
                    alt="LOOP"
                    data-testid="header-app-logo"
                    className="h-7 w-auto shrink-0 object-contain"
                  />
                  <span className="kfx relative -top-[1px] truncate text-base font-black uppercase tracking-[0.16em] text-text-primary">
                    LOOP
                  </span>
                </button>
              </Tooltip>
            )}
          </div>

          {showRaceBriefing ? (
            <div className="flex items-center gap-3">
              <TeamLogoMark
                teamName={playerTeam?.nome}
                color={playerTeam?.cor_primaria ?? "#58a6ff"}
                size="md"
                testId="briefing-team-logo"
              />
              <span className="text-3xl font-bold tracking-[-0.035em] text-text-primary">
                {playerTeam?.nome ?? "-"}
              </span>
            </div>
          ) : (
            <TabNavigation activeTab={activeTab} onTabChange={onTabChange} />
          )}

          <div className="flex flex-1 justify-end">
            <div className="flex items-center gap-3 rounded-2xl border border-white/10 bg-white/[0.05] px-4 py-2 backdrop-blur-md">
              <div className="text-right">
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-secondary">
                  {t("nav.date")} {formatCompactDate(visibleDate)}
                </p>
                <p className="mt-1 text-xs font-semibold text-text-primary">
                  {formatNextRaceCountdown(visibleCountdown)}
                </p>
              </div>
              {showRaceBriefing ? (
                <GlassButton
                  variant="primary"
                  className="rounded-full px-5 py-2.5"
                  onClick={fecharBriefing}
                >
                  {t("nav.back")}
                </GlassButton>
              ) : bannerOwnsAdvance ? null : (
                <GlassButton
                  variant="primary"
                  disabled={avancoEmCurso}
                  className="rounded-full px-5 py-2.5"
                  onClick={avancar}
                >
                  {rotuloDoAvanco()}
                </GlassButton>
              )}
            </div>
          </div>
        </div>
      </div>

      {activeTab === "standings" && !showRaceBriefing && (
        !viewingOwnCategory ? (
          // Vendo OUTRA categoria: banner informativo (sem botão Avançar). Com etapa
          // pendente, a próxima corrida daquela categoria; com o ano dela encerrado, o
          // pôster do campeão dela. Enquanto as duas leituras carregam, segura a altura
          // do banner para não "pular" o layout.
          categoryRace?.pending && categoryRace.race ? (
            <NextRaceBanner
              nextRace={categoryRace.race}
              championship={categoryLabel(viewedCategory)}
              totalRodadas={categoryRace.totalRodadas}
              countdownDays={categoryRace.countdownDays}
            />
          ) : fimDeTemporadaDaCategoria ? (
            <SeasonFinishedBanner
              season={season}
              category={viewedCategory}
              resumo={fimDeTemporadaDaCategoria}
            />
          ) : (
            <BannerHeightPlaceholder />
          )
        ) : nextRace ? (
          <NextRaceBanner
            nextRace={nextRace}
            championship={categoryLabel(playerTeam?.categoria)}
            totalRodadas={season?.total_rodadas ?? null}
            countdownDays={visibleCountdown}
            onAdvance={avancar}
            advanceLabel={rotuloDoAvanco()}
            advanceDisabled={avancoEmCurso}
          />
        ) : hasNoPendingRace && playerTeam?.categoria ? (
          <SeasonFinishedBanner
            season={season}
            category={playerTeam.categoria}
            resumo={fimDeTemporada}
            onAdvance={avancar}
            advanceLabel={rotuloDoAvanco()}
            advanceDisabled={avancoEmCurso}
          />
        ) : (
          <div className="flex min-h-[110px] items-stretch h-[14vh]">
            <div className="mx-auto flex w-full max-w-[1680px] items-stretch px-3 sm:px-4 lg:px-5 xl:px-6">
              <p className="text-sm text-text-muted">
                {season
                  ? isFreeAgent
                    ? `${formatSurfaceSeasonLabel(season)} ${t("seasonBanner.noTeam")}`
                    : `${formatSurfaceSeasonLabel(season)} ${t("seasonBanner.noPendingRace")}`
                  : t("nav.loading")}
              </p>
            </div>
          </div>
        )
      )}

      <LeaveToMenuModal
        open={leaveConfirm}
        isSaving={isSaving}
        onSaveAndExit={saveAndExit}
        onExitWithoutSave={exit}
        onCancel={() => setLeaveConfirm(false)}
      />
    </header>
  );
}

// Pôster de fim de ano: o encerramento da temporada tem o mesmo peso visual da próxima
// etapa (mesma largura, mesma altura, mesmo botão), em vez de uma frase resumindo o que a
// tabela logo abaixo já mostra. O ouro só acende quando a taça é do jogador — com campeão
// de IA o card vira notícia, com a posição dele na linha de baixo.
function SeasonFinishedBanner({
  season,
  category,
  resumo,
  onAdvance = null,
  advanceLabel,
  advanceDisabled,
}) {
  const campeao = resumo?.campeao ?? null;
  const vice = resumo?.vice ?? null;
  const jogador = resumo?.jogador ?? null;

  const championName = campeao?.nome ?? i18n.t("seasonBanner.championTbd");
  const jogadorECampeao = Boolean(campeao?.is_jogador);
  const teamColor = campeao?.equipe_cor || "#f2c46d";
  const margem = campeao && vice ? Number(campeao.pontos) - Number(vice.pontos) : null;

  const kicker = season?.ano
    ? i18n.t("seasonBanner.finishedYear", { year: season.ano })
    : i18n.t("seasonBanner.finished");

  // Linha de apoio: existe SEMPRE que há campeão, para o pôster ter a mesma altura na
  // categoria do jogador e nas outras. O assunto é que muda — a posição dele quando ele
  // está naquela série e não levou a taça, e a conta do título no resto.
  const linhaDeApoio = (() => {
    if (!campeao) return null;
    if (jogador && !jogadorECampeao) {
      return i18n.t("seasonBanner.playerResult", {
        position: jogador.posicao_campeonato,
        points: jogador.pontos,
      });
    }
    if (vice?.nome && Number.isFinite(margem)) {
      return i18n.t("seasonBanner.clinch", { margin: margem, runnerUp: vice.nome });
    }
    return i18n.t("seasonBanner.clinchSolo", { points: campeao.pontos });
  })();

  const numeros = campeao
    ? [
        { chave: "points", valor: campeao.pontos, destaque: true },
        { chave: "wins", valor: campeao.vitorias },
        { chave: "podiums", valor: campeao.podios },
        ...(Number.isFinite(margem) ? [{ chave: "margin", valor: `+${margem}` }] : []),
      ]
    : [];

  return (
    // Mesmo enquadramento do banner de corrida: padding por fora do container de 1680px,
    // para o card bater com a largura das tabelas de baixo.
    <div className="px-3 pb-1 pt-0.5 sm:px-4 lg:px-5 xl:px-6">
      <div className="mx-auto w-full max-w-[1680px]">
        <div className="relative overflow-hidden rounded-[28px] border border-white/5 bg-[#03060f] shadow-[0_18px_45px_rgba(0,0,0,0.38)] min-h-[196px] md:h-[clamp(200px,21vh,230px)]">
          {/* Céu do pôster: o brilho grande é da cor da equipe campeã. */}
          <div
            className="pointer-events-none absolute inset-0 opacity-[0.22]"
            style={{
              background: `radial-gradient(900px 300px at 78% -12%, ${teamColor}, transparent 66%)`,
            }}
          />
          <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(700px_280px_at_16%_120%,rgba(242,196,109,0.20),transparent_70%)]" />

          {/* Traçado decorativo à direita — abstrato de propósito, para não afirmar um
              circuito que não é o do encerramento. */}
          <svg
            className="pointer-events-none absolute inset-y-0 right-0 h-full w-[58%] opacity-40"
            viewBox="0 0 700 210"
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            <path
              d="M60 196 C200 150 150 92 270 74 C380 58 350 20 470 18 C580 16 640 46 700 30"
              fill="none"
              stroke="#f2c46d"
              strokeWidth="2.5"
              opacity="0.38"
            />
            <path
              d="M60 206 C200 160 150 102 270 84 C380 68 350 30 470 28 C580 26 640 56 700 40"
              fill="none"
              stroke="#ffffff"
              strokeWidth="10"
              opacity="0.05"
            />
          </svg>

          <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(90deg,rgba(3,6,15,0.96)_28%,rgba(3,6,15,0.58)_62%,rgba(3,6,15,0.18)_100%)]" />

          <div className="relative z-10 flex h-full flex-col justify-end gap-4 p-5 sm:p-6 md:flex-row md:items-end md:justify-between md:p-7">
            <div className="flex min-w-0 flex-col">
              <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5">
                <p className="text-[11px] font-bold uppercase tracking-[0.28em] text-[#f2c46d]">
                  {kicker}
                </p>
                <span className="inline-flex items-center rounded-full border border-white/10 bg-white/[0.06] px-2.5 py-1 text-[11px] font-bold uppercase tracking-[0.12em] text-text-primary">
                  {categoryLabel(category)}
                </span>
              </div>

              <h2
                className={`mt-2 text-[clamp(2.2rem,5vw,3.4rem)] font-extrabold uppercase leading-[1.02] tracking-[-0.02em] ${
                  jogadorECampeao
                    ? "bg-[linear-gradient(180deg,#fff3d4_8%,#e8bd63_58%,#b8912f_100%)] bg-clip-text text-transparent"
                    : "text-white [text-shadow:0_2px_18px_rgba(0,0,0,0.55)]"
                }`}
              >
                {i18n.t("seasonBanner.championWord")}
              </h2>

              <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-sm">
                {campeao?.nacionalidade && <FlagIcon nacionalidade={campeao.nacionalidade} />}
                <span className="text-lg font-bold tracking-[-0.01em] text-text-primary sm:text-xl">
                  {championName}
                </span>
                {campeao?.equipe_nome && (
                  <>
                    <span className="text-white/25">•</span>
                    <span className="font-semibold uppercase tracking-[0.06em]" style={{ color: teamColor }}>
                      {campeao.equipe_nome}
                    </span>
                  </>
                )}
              </div>

              {linhaDeApoio && (
                <p className="mt-2 text-[13px] font-medium text-text-secondary">{linhaDeApoio}</p>
              )}
            </div>

            <div className="flex shrink-0 flex-wrap items-end justify-end gap-6 md:pb-1">
              {numeros.map((numero) => (
                <div key={numero.chave} className="text-right">
                  <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-text-secondary">
                    {i18n.t(`seasonBanner.stats.${numero.chave}`)}
                  </p>
                  <p
                    className={`text-[28px] font-extrabold leading-none tracking-[-0.03em] sm:text-[32px] ${
                      numero.destaque ? "text-[#f2c46d]" : "text-text-primary"
                    }`}
                  >
                    {numero.valor}
                  </p>
                </div>
              ))}

              {onAdvance ? (
                <button
                  type="button"
                  onClick={onAdvance}
                  disabled={advanceDisabled}
                  className="group/btn inline-flex h-12 items-center gap-2 rounded-xl bg-[#f2c46d] px-6 text-sm font-black uppercase tracking-[0.04em] text-[#2a1c05] shadow-[0_0_22px_rgba(242,196,109,0.35)] transition hover:bg-[#ffd98a] hover:shadow-[0_0_28px_rgba(242,196,109,0.5)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#f2c46d] active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-70 sm:h-[52px] sm:px-8 sm:text-base"
                >
                  <span>{advanceLabel}</span>
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2.6"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    className="h-4 w-4 transition-transform group-hover/btn:translate-x-0.5"
                    aria-hidden="true"
                  >
                    <path d="M5 12h14" />
                    <path d="m13 6 6 6-6 6" />
                  </svg>
                </button>
              ) : null}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// Banner cinematográfico da próxima corrida (Home). Protagonista da tela: imagem
// ampla do circuito ao fundo, gradientes escuros para legibilidade, dados 100%
// dinâmicos (mesmas variáveis do bloco antigo) e o botão "Avançar calendário"
// reaproveitando o handler existente (onAdvance = o `avancar` de `useFluxoDeAvanco`).
function NextRaceBanner({
  nextRace,
  championship,
  totalRodadas = null,
  countdownDays,
  onAdvance = null,
  advanceLabel,
  advanceDisabled,
}) {
  const trackName = nextRace.track_name;
  // "🇧🇷 Brasil" — o rótulo cru alimenta o FlagIcon; o código ISO vira a chave i18n.
  const country = trackCountryLabel(trackName);
  const countryCode = extractNationalityCode(country);
  const bannerDate = formatBannerDate(nextRace.display_date);
  const hasWeather = nextRace.clima != null || nextRace.temperatura != null;
  const countdown = compactCountdown(countdownDays);

  return (
    // Padding POR FORA do container de 1680px (igual à barra de nav e ao <main>),
    // para o card do banner ficar exatamente da mesma largura dos cards de baixo
    // ("GT3 Championship"/"Construtores").
    <div className="px-3 pb-1 pt-0.5 sm:px-4 lg:px-5 xl:px-6">
      <div className="mx-auto w-full max-w-[1680px]">
        <div className="group relative overflow-hidden rounded-[28px] border border-accent-primary/20 bg-[#03060f] shadow-[0_18px_45px_rgba(0,0,0,0.38)] min-h-[196px] md:h-[clamp(200px,21vh,230px)]">
        {/* Base decorativa (sempre atrás da imagem): quando a pista não tem foto,
            este fundo azul-marinho premium aparece no lugar de um preto vazio. */}
        <div
          className="pointer-events-none absolute inset-0"
          style={{
            background:
              "radial-gradient(120% 130% at 82% 0%, rgba(88,166,255,0.12), transparent 55%), linear-gradient(135deg, #0a1424 0%, #03060f 70%)",
          }}
        />
        {/* Imagem do circuito (fade-in ao carregar; cai na base acima se faltar). */}
        <BannerTrackImage trackName={trackName} />

        {/* Camada 1 — gradiente horizontal: escuro só à ESQUERDA (atrás do texto),
            transparente na metade direita para a foto aparecer nítida. */}
        <div
          className="pointer-events-none absolute inset-0"
          style={{
            background:
              "linear-gradient(90deg, rgba(3,8,18,0.95) 0%, rgba(3,8,18,0.78) 30%, rgba(3,8,18,0.32) 58%, rgba(3,8,18,0) 82%)",
          }}
        />
        {/* Camada 2 — gradiente vertical suave: só assenta a base (linha de infos). */}
        <div
          className="pointer-events-none absolute inset-0"
          style={{
            background:
              "linear-gradient(0deg, rgba(3,8,18,0.72) 0%, rgba(3,8,18,0) 56%)",
          }}
        />
        {/* Brilho decorativo discreto (canto superior direito) — sem mapa de pista
            para não arriscar um contorno incorreto. */}
        <div className="pointer-events-none absolute -right-16 -top-20 h-56 w-56 rounded-full bg-[radial-gradient(circle,rgba(88,166,255,0.14),transparent_68%)]" />

        {/* Conteúdo */}
        <div className="nrb-rise relative z-10 flex h-full flex-col justify-end gap-4 p-5 sm:p-6 md:flex-row md:items-end md:justify-between md:p-7">
          <div className="flex min-w-0 max-w-full flex-col md:max-w-[62%]">
            <p className="text-[11px] font-bold uppercase tracking-[0.28em] text-accent-primary">
              {totalRodadas
                ? i18n.t("raceBanner.raceNofTotal", { round: nextRace.rodada, total: totalRodadas })
                : i18n.t("raceBanner.raceN", { round: nextRace.rodada })}
            </p>

            {/* Sem tooltip: o nome quebra linha em vez de cortar, então o balão
                só repetiria em miúdo o que já está em 2,9rem na tela. */}
            <h2 className="mt-2 break-words text-[clamp(1.9rem,4vw,2.9rem)] font-extrabold uppercase leading-[1.02] tracking-[-0.01em] text-white [text-shadow:0_2px_18px_rgba(0,0,0,0.55)]">
              {trackName}
            </h2>

            <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-sm">
              {country && (
                <>
                  <span className="flex items-center gap-2">
                    <FlagIcon nacionalidade={country} />
                    <span className="font-semibold uppercase tracking-[0.06em] text-text-primary">
                      {i18n.t(`header.trackCountry.${countryCode}`, { defaultValue: country })}
                    </span>
                  </span>
                  <span className="text-white/25">•</span>
                </>
              )}
              <span className="font-semibold uppercase tracking-[0.06em] text-accent-primary">
                {championship}
              </span>
            </div>

            <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-[13px] font-medium text-text-secondary">
              {bannerDate && (
                <span className="flex items-center gap-1.5">
                  <CalendarGlyph />
                  <span className="text-text-primary/90">{bannerDate}</span>
                </span>
              )}
              {nextRace.horario && (
                <span className="flex items-center gap-1.5">
                  <ClockGlyph />
                  <span className="text-text-primary/90">{nextRace.horario} {i18n.t("raceBanner.local")}</span>
                </span>
              )}
              {hasWeather && (
                <span className="flex items-center gap-1.5">
                  <span className="sr-only">{i18n.t("raceBanner.weather")}</span>
                  <span aria-hidden="true" className="text-base leading-none">
                    {weatherEmoji(nextRace.clima)}
                  </span>
                  {nextRace.temperatura != null && (
                    <span className="text-text-primary/90">
                      {Math.round(nextRace.temperatura)}°C
                    </span>
                  )}
                  <span className="hidden text-text-secondary sm:inline">
                    {climaLabel(nextRace.clima, CLIMA_BANNER)}
                  </span>
                </span>
              )}
              {countdown && (
                <span className="inline-flex items-center gap-1.5 rounded-full border border-accent-primary/30 bg-accent-primary/10 px-2.5 py-1 text-[11px] font-bold uppercase tracking-[0.12em] text-accent-primary">
                  {countdown}
                </span>
              )}
            </div>
          </div>

          {/* Ações — botão principal reaproveita o handler de avanço do calendário.
              Só aparece na próxima corrida DO JOGADOR; vendo outra categoria o banner
              é puramente informativo (o botão global volta à barra do topo). */}
          {onAdvance ? (
          <div className="flex shrink-0 items-center justify-end md:pb-1">
            <button
              type="button"
              onClick={onAdvance}
              disabled={advanceDisabled}
              className="group/btn inline-flex h-12 items-center gap-2 rounded-xl bg-accent-primary px-6 text-sm font-black uppercase tracking-[0.04em] text-[#03060f] shadow-[0_0_22px_rgba(88,166,255,0.4)] transition hover:bg-accent-hover hover:shadow-[0_0_28px_rgba(88,166,255,0.55)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-primary active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-70 sm:h-[52px] sm:px-8 sm:text-base"
            >
              <span>{advanceLabel}</span>
              <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.6"
                strokeLinecap="round"
                strokeLinejoin="round"
                className="h-4 w-4 transition-transform group-hover/btn:translate-x-0.5"
                aria-hidden="true"
              >
                <path d="M5 12h14" />
                <path d="m13 6 6 6-6 6" />
              </svg>
            </button>
          </div>
          ) : null}
        </div>
        </div>
      </div>
    </div>
  );
}

// Segura a altura do banner enquanto o calendário de outra categoria é buscado, para
// a Home não "pular" quando o banner informativo entra logo em seguida.
function BannerHeightPlaceholder() {
  return (
    <div className="px-3 pb-1 pt-0.5 sm:px-4 lg:px-5 xl:px-6">
      <div className="mx-auto w-full max-w-[1680px]">
        <div className="relative overflow-hidden rounded-[28px] border border-white/5 bg-[#03060f] min-h-[196px] md:h-[clamp(200px,21vh,230px)]" />
      </div>
    </div>
  );
}

// Imagem de fundo do banner: fundo escuro enquanto carrega, fade-in + zoom lento
// ao terminar, e some (mantendo o gradiente) caso o arquivo não exista. A resolução
// (panorâmica → miniatura → chute) mora em utils/trackBanners.js.
function BannerTrackImage({ trackName }) {
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);

  return (
    <img
      src={getBannerImageSrc(trackName)}
      alt={trackName}
      loading="lazy"
      onLoad={() => setLoaded(true)}
      onError={() => setFailed(true)}
      style={{ objectPosition: getBannerImageFocus(trackName) }}
      className={[
        "absolute inset-0 h-full w-full object-cover transition-opacity duration-700",
        "[filter:saturate(0.96)_contrast(1.05)_brightness(1.02)]",
        loaded && !failed ? "opacity-100 nrb-zoom" : "opacity-0",
      ].join(" ")}
    />
  );
}

const BANNER_MONTHS_PT = [
  "JAN",
  "FEV",
  "MAR",
  "ABR",
  "MAI",
  "JUN",
  "JUL",
  "AGO",
  "SET",
  "OUT",
  "NOV",
  "DEZ",
];

// Data compacta em caixa alta ("06 FEV 2042"), lida direto da string YYYY-MM-DD
// para evitar deslocamento de fuso do `new Date`.
function formatBannerDate(displayDate) {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(displayDate ?? "");
  if (!match) return null;
  const [, year, month, day] = match;
  const monthLabel = BANNER_MONTHS_PT[Number(month) - 1] ?? month;
  return `${day} ${monthLabel} ${year}`;
}

// Contagem regressiva curtíssima ("EM 7 DIAS") a partir dos mesmos dados do header.
function compactCountdown(days) {
  if (days == null) return null;
  const value = Number(days);
  if (!Number.isFinite(value)) return null;
  if (value <= 0) return i18n.t("raceBanner.countdown.today");
  if (value === 1) return i18n.t("raceBanner.countdown.tomorrow");
  if (value <= 27) return i18n.t("raceBanner.countdown.days", { count: value });
  if (value < 56) {
    const weeks = Math.max(1, Math.floor(value / 7));
    return i18n.t("raceBanner.countdown.weeks", { count: weeks });
  }
  const months = Math.max(2, Math.floor(value / 28));
  return i18n.t("raceBanner.countdown.months", { count: months });
}

function CalendarGlyph() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      className="h-4 w-4 text-accent-primary"
      aria-hidden="true"
    >
      <rect x="3" y="4.5" width="18" height="16" rx="2.5" />
      <path d="M3 9h18M8 2.5v4M16 2.5v4" />
    </svg>
  );
}

function ClockGlyph() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      className="h-4 w-4 text-accent-primary"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7.5V12l3 2" />
    </svg>
  );
}

export default Header;
