import { useState } from "react";
import { useTranslation } from "react-i18next";

import useCareerStore from "../../stores/useCareerStore";
import { isLegacySeasonPhase } from "../../utils/seasonPhases";

// O FLUXO DE TEMPORADA por trás do botão "Avançar".
//
// Um botão só, e por trás dele sete destinos: animar o calendário até a próxima etapa,
// pular a temporada de um agente livre, abrir a janela de convocação e encerrar o bloco
// especial (os dois legado 9D), desviar pelas Notícias no fim do campeonato, e virar o
// ano. Qual deles vale depende da fase da temporada, de haver etapa pendente, de o
// jogador ter equipe e da aba aberta.
//
// Isso é regra de temporada, e morava dentro de um componente de LAYOUT — vinte seletores
// do store e três funções de decisão no meio do JSX do cabeçalho. Aqui em cima, o Header
// volta a ser o que ele é: desenho.
export default function useFluxoDeAvanco({ activeTab, onTabChange }) {
  const { t } = useTranslation();

  const playerTeam = useCareerStore((state) => state.playerTeam);
  const season = useCareerStore((state) => state.season);
  const nextRace = useCareerStore((state) => state.nextRace);
  const homeCategory = useCareerStore((state) => state.homeCategory);
  const temporalSummary = useCareerStore((state) => state.temporalSummary);
  const calendarDisplayDate = useCareerStore((state) => state.calendarDisplayDate);
  const displayDaysUntilNextEvent = useCareerStore((state) => state.displayDaysUntilNextEvent);
  const isCalendarAdvancing = useCareerStore((state) => state.isCalendarAdvancing);
  const isAdvancing = useCareerStore((state) => state.isAdvancing);
  const isConvocating = useCareerStore((state) => state.isConvocating);
  const showRaceBriefing = useCareerStore((state) => state.showRaceBriefing);
  const startCalendarAdvance = useCareerStore((state) => state.startCalendarAdvance);
  const advanceSeason = useCareerStore((state) => state.advanceSeason);
  const skipAllPendingRaces = useCareerStore((state) => state.skipAllPendingRaces);
  const runConvocationWindow = useCareerStore((state) => state.runConvocationWindow);
  const finishSpecialBlock = useCareerStore((state) => state.finishSpecialBlock);
  const closeRaceBriefing = useCareerStore((state) => state.closeRaceBriefing);

  // Temporada cujo "Avançar" já desviou pelas Notícias. Só no FIM DO CAMPEONATO o botão
  // gasta um clique levando o jogador ao fechamento do ano antes de abrir o mercado — sem
  // isso, quem vem no piloto automático pula as notícias de encerramento sem ver. Um
  // clique por temporada; o segundo avança de verdade.
  const [newsDetourSeason, setNewsDetourSeason] = useState(null);

  const visibleDate = calendarDisplayDate ?? temporalSummary?.current_display_date;
  const visibleCountdown = displayDaysUntilNextEvent ?? temporalSummary?.days_until_next_event;
  const hasNoPendingRace = !nextRace;
  const isFreeAgent = !playerTeam;
  // Categoria em exibição na Home: a do jogador (padrão) ou outra que ele abriu na tabela.
  const viewingOwnCategory = !homeCategory || homeCategory === playerTeam?.categoria;
  const viewedCategory = homeCategory ?? playerTeam?.categoria ?? null;
  const phase = season?.fase;
  const isLegacyPhase = isLegacySeasonPhase(phase);
  const hasPendingLegacyRegularRaces =
    isLegacyPhase && phase === "BlocoRegular" && (temporalSummary?.pending_in_phase ?? 0) > 0;
  const canAdvanceCalendar = Boolean(nextRace) || (!isFreeAgent && hasPendingLegacyRegularRaces);
  // O clique que encerra o ano e abre o mercado. É o único que ganha o desvio de uma volta
  // pelas Notícias (ver `newsDetourSeason`).
  const isSeasonEndAdvance =
    !canAdvanceCalendar &&
    !isFreeAgent &&
    !isLegacyPhase &&
    (phase === "Encerramento" || (hasNoPendingRace && phase === "Temporada"));
  // Quem já está na aba de Notícias não precisa ser levado até lá — o desvio existe para
  // quem avançaria sem passar por elas.
  const seasonEndNeedsNewsDetour =
    isSeasonEndAdvance &&
    season?.numero != null &&
    newsDetourSeason !== season.numero &&
    activeTab !== "news";
  // No Home (standings), com uma corrida marcada, o botão "Avançar calendário" vive DENTRO
  // do banner cinematográfico — então o duplicado da barra superior some. O banner só
  // "dona" o botão quando é a próxima corrida DO JOGADOR; vendo outra categoria ele é
  // informativo e o botão global volta à barra.
  const bannerOwnsAdvance =
    activeTab === "standings" && !showRaceBriefing && Boolean(nextRace) && viewingOwnCategory;

  const avancoEmCurso = isCalendarAdvancing || isAdvancing || isConvocating;

  function avancarCalendario() {
    // Leva o jogador para o Calendário (com fade) para ele ver a animação dos dias
    // passando — MAS só quando há dias a passar. Se a corrida é HOJE, avançar abre direto
    // a sala de estratégia; piscar o calendário antes seria ruim.
    const daysUntilRace = Number(visibleCountdown);
    if (Number.isFinite(daysUntilRace) && daysUntilRace > 0) {
      onTabChange?.("calendar");
    }
    void Promise.resolve(startCalendarAdvance?.()).catch((error) => {
      console.error("Erro ao avançar calendário pelo header:", error);
    });
  }

  async function avancarTemporada() {
    try {
      // Fim do campeonato: o primeiro clique só leva às Notícias do encerramento. O
      // jogador volta a clicar em "Avançar" e aí sim entra no mercado.
      if (seasonEndNeedsNewsDetour) {
        setNewsDetourSeason(season.numero);
        onTabChange?.("news");
        return;
      }

      if (isFreeAgent && hasNoPendingRace) {
        await skipAllPendingRaces?.();
        return;
      }

      // LEGADO 9D: convocação e bloco especial só existem para saves pré-v33 em voo.
      if (isLegacyPhase && hasNoPendingRace && phase === "BlocoRegular") {
        await runConvocationWindow?.();
        return;
      }

      if (isLegacyPhase && hasNoPendingRace && phase === "BlocoEspecial") {
        await finishSpecialBlock?.();
        return;
      }

      await advanceSeason?.();
    } catch (error) {
      console.error("Erro ao avançar temporada pelo header:", error);
    }
  }

  /// O clique único do botão. Qual dos dois caminhos vale é decisão desta camada, não de
  /// quem desenha — o botão da barra e o do banner são o mesmo botão.
  function avancar() {
    if (canAdvanceCalendar) {
      avancarCalendario();
      return;
    }
    void avancarTemporada();
  }

  function rotuloDoAvanco() {
    if (avancoEmCurso) {
      return t("nav.advance.advancing");
    }

    if (canAdvanceCalendar) {
      return t("nav.advance.calendar");
    }

    // O clique do desvio anuncia o que faz: ler o fechamento do ano, não avançar.
    if (seasonEndNeedsNewsDetour) {
      return t("nav.advance.seasonNews");
    }

    if (isFreeAgent && hasNoPendingRace) {
      return t("nav.advance.skipSeason");
    }

    // LEGADO 9D: estes labels só aparecem em saves pré-v33 em voo.
    if (isLegacyPhase && hasNoPendingRace && phase === "BlocoRegular") {
      return t("nav.advance.toCallup");
    }

    if (isLegacyPhase && hasNoPendingRace && phase === "BlocoEspecial") {
      return t("nav.advance.skipSpecial");
    }

    if (isLegacyPhase && hasNoPendingRace && phase === "PosEspecial") {
      return t("nav.advance.endSeason");
    }

    if (phase === "Encerramento" || (hasNoPendingRace && phase === "Temporada")) {
      return t("nav.advance.toPreseason");
    }

    if (phase === "PreTemporada") {
      return t("nav.advance.openMarket");
    }

    return t("nav.advance.advanceSeason");
  }

  return {
    // Estado de tela que o cabeçalho desenha.
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
    // Botão de avanço.
    bannerOwnsAdvance,
    avancoEmCurso,
    avancar,
    rotuloDoAvanco,
    // Volta da Sala de Estratégia para a aba de origem.
    fecharBriefing: () => closeRaceBriefing?.(),
  };
}
