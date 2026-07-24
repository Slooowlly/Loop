import { useState, useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { currentLang } from "../../i18n/format.js";
import { invoke } from "@tauri-apps/api/core";
import useCareerStore from "../../stores/useCareerStore";
import WeeklyClosingMovement from "./preseason/WeeklyClosingMovement";
import MarketCategoryHeader from "./preseason/MarketCategoryHeader";
import TeamDriverRow from "./preseason/TeamDriverRow";
import FreeAgentCard from "./preseason/FreeAgentCard";
import OfferCategoryRow from "./preseason/OfferCategoryRow";
import OfferCardRich from "./preseason/OfferCardRich";
import TeamGridCard from "./preseason/TeamGridCard";
import ProposalCard from "./preseason/ProposalCard";
import OffersModal from "./preseason/OffersModal";
import ContractModal from "./preseason/ContractModal";
import TeamHistoryOverlay from "./preseason/TeamHistoryOverlay";
import DisplacedDriversModal from "./preseason/DisplacedDriversModal";
import TransferDetailModal from "./preseason/TransferDetailModal";
import FreeAgentWarningModal from "./preseason/FreeAgentWarningModal";
import PreSeasonHeader from "./preseason/PreSeasonHeader";
import FreeAgentsPanel from "./preseason/FreeAgentsPanel";
import TeamsGridPanel from "./preseason/TeamsGridPanel";
import DecisionsPanel from "./preseason/DecisionsPanel";

import {
  LEVEL_BANDS,
  bandForTier,
  MARKET_TIER_BY_CATEGORY,
  brandOf,
  playerCatToFilter,
  buildWeeklyClosingGroups,
} from "./preSeasonFormatters.js";
import {
  buildOffersByCategory,
  fetchGridTeams,
  groupTeamsByClass,
  sortTeamClasses,
  buildFreeAgentsByBand,
  buildDisplacedByCategory,
} from "./preseason/selectors.js";

// ─── Main Component ───────────────────────────────────────────────────────────

export default function PreSeasonView() {
  const { t } = useTranslation();
  const careerId             = useCareerStore((s) => s.careerId);
  const preseasonState       = useCareerStore((s) => s.preseasonState);
  const lastMarketWeekResult = useCareerStore((s) => s.lastMarketWeekResult);
  const playerProposals      = useCareerStore((s) => s.playerProposals);
  const respondToProposal    = useCareerStore((s) => s.respondToProposal);
  const transferWindow       = useCareerStore((s) => s.transferWindow);
  const preseasonFreeAgents  = useCareerStore((s) => s.preseasonFreeAgents);
  const isAdvancingWeek      = useCareerStore((s) => s.isAdvancingWeek);
  const advanceMarketWeek    = useCareerStore((s) => s.advanceMarketWeek);
  const finalizePreseason    = useCareerStore((s) => s.finalizePreseason);
  const playerTeam           = useCareerStore((s) => s.playerTeam);

  const [selectedCat, setSelectedCat]           = useState(() => playerCatToFilter(playerTeam?.categoria));
  const [gridData, setGridData]                 = useState([]);
  const [loadingGrid, setLoadingGrid]           = useState(false);
  const [showDisplacedModal, setShowDisplacedModal] = useState(false);
  const [transferDetail, setTransferDetail] = useState(null);
  const [showFreeAgentWarning, setShowFreeAgentWarning] = useState(false);
  const [startError, setStartError] = useState("");
  const [paintToast, setPaintToast] = useState("");
  const [showOffersModal, setShowOffersModal] = useState(false);
  // Categoria filtrada no modal de ofertas (null = mostrar todas).
  const [offersModalCat, setOffersModalCat] = useState(null);
  // Oferta cujo "contrato" (tela detalhada de assinatura) está aberto (null = fechado).
  const [contractOffer, setContractOffer] = useState(null);
  // Animação de assinatura: enquanto true, o nome do piloto é "escrito" na linha
  // antes de a oferta ser efetivada.
  const [isSigning, setIsSigning] = useState(false);
  // Equipe do grid cujo Histórico mundial de equipes (atlas) está aberto — duplo clique no card.
  const [historyTeam, setHistoryTeam] = useState(null);
  // Categoria do piloto livre sob o cursor → acende as equipes dela no grid central.
  const [hoveredFreeAgentCat, setHoveredFreeAgentCat] = useState(null);

  const freeAgentContainerRef = useRef(null);
  const freeAgentSectionRefs  = useRef({});
  const mainGridRef           = useRef(null);
  // Scroll a restaurar após avançar a semana (mantém o usuário onde estava, em vez
  // de jogar os painéis pro topo enquanto o grid recarrega). null = nada a restaurar.
  const preserveScrollRef     = useRef(null);
  // Auto-scroll pra categoria do jogador só na 1ª carga (não brigar com o restore).
  const didInitialScrollRef   = useRef(false);

  // Semana atual e total
  const currentWeek = Math.min(preseasonState?.current_week ?? 1, preseasonState?.total_weeks ?? 1);
  const totalWeeks  = preseasonState?.total_weeks ?? 1;
  const isComplete  = preseasonState?.is_complete ?? false;
  const isMarketOpen = !isComplete;
  const weekProgress = Math.min(100, (currentWeek / totalWeeks) * 100);

  // Ofertas que a Janela de Transferências mandou ao jogador nesta semana.
  const playerOffers = transferWindow?.player_offers ?? [];
  const playerSignedThisWindow = preseasonState?.player_has_team ?? false;

  // Categoria/tier efetivos do jogador (vêm do backend — funciona mesmo como agente
  // livre, quando não há playerTeam). Tier na convenção do backend (= offer.category_tier).
  const playerCategory = transferWindow?.player_category ?? playerTeam?.categoria ?? null;
  const playerTier = transferWindow?.player_tier ?? null;
  const playerBrand = brandOf(playerCategory);
  const playerName = transferWindow?.player_name ?? null;

  // Ofertas agrupadas por categoria (N1/N2 dentro), ordenadas por bucket/nível.
  const offersByCategory = useMemo(
    () => buildOffersByCategory(playerOffers, playerBrand, playerTier),
    [playerOffers, playerBrand, playerTier],
  );

  const totalOffers = playerOffers.length;

  // Três andares de importância (offersByCategory já vem ordenado por bucket):
  // 0 = promoção (destaque), 1 = marca atual do jogador, 2 = demais marcas.
  const promoOfferGroups = offersByCategory.filter((g) => g.bucket === 0);
  const brandOfferGroups = offersByCategory.filter((g) => g.bucket === 1);
  const otherOfferGroups = offersByCategory.filter((g) => g.bucket === 2);

  // Abre o modal de ofertas já filtrado pela categoria (null = todas).
  const openOffersFor = (cat) => { setOffersModalCat(cat); setShowOffersModal(true); };

  // Abre a ficha de contrato de uma oferta (assinatura sempre começa zerada).
  const handleViewContract = (offer) => { setIsSigning(false); setContractOffer(offer); };

  // Assinar: escreve a assinatura (~1.25s) e só então efetiva a oferta.
  const handleSignContract = (offer) => {
    if (isSigning) return;
    setIsSigning(true);
    setTimeout(() => {
      setContractOffer(null);
      setShowOffersModal(false);
      handleAcceptOffer(offer);
    }, 1550);
  };

  const currentDateLabel = useMemo(
    () => {
      const preseasonDate = preseasonState?.current_display_date;
      if (!preseasonDate) return "-";

      return new Intl.DateTimeFormat(currentLang(), {
        day: "numeric",
        month: "long",
      }).format(new Date(`${preseasonDate}T12:00:00`));
    },
    [preseasonState?.current_display_date],
  );

  // ── Fetch grid ──────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!careerId) return;
    let mounted = true;

    async function fetchGrid() {
      setLoadingGrid(true);
      try {
        const final = await fetchGridTeams(careerId, selectedCat);
        if (mounted) setGridData(final);
      } finally {
        if (mounted) setLoadingGrid(false);
      }
    }

    fetchGrid();
    return () => { mounted = false; };
    // Semana CRUA (não clampada) + resultado da semana → o grid reflete as
    // assinaturas aplicadas a cada avanço, inclusive além do teto de exibição.
  }, [careerId, selectedCat, preseasonState?.current_week, lastMarketWeekResult]);

  // ── Agrupamento e ordenação ─────────────────────────────────────────────────
  const groupedTeams = useMemo(() => groupTeamsByClass(gridData), [gridData]);

  const sortedClasses = useMemo(() => sortTeamClasses(groupedTeams), [groupedTeams]);

  // ── Free agents agrupados por FAIXA DE NÍVEL (onde correm hoje) ─────────────
  const freeAgentsByBand = useMemo(
    () => buildFreeAgentsByBand(preseasonFreeAgents, selectedCat),
    [preseasonFreeAgents, selectedCat],
  );

  // Bandas presentes, do mais prestigioso pro menos (ordem de LEVEL_BANDS).
  const freeAgentBandOrder = useMemo(
    () => LEVEL_BANDS.filter((b) => freeAgentsByBand[b.key]?.length),
    [freeAgentsByBand],
  );

  // Total visível (após o filtro do topo) — alimenta o contador do cabeçalho.
  const visibleFreeAgentCount = useMemo(
    () => freeAgentBandOrder.reduce((n, b) => n + freeAgentsByBand[b.key].length, 0),
    [freeAgentBandOrder, freeAgentsByBand],
  );

  const displacedVeterans = useMemo(
    () => (preseasonFreeAgents ?? []).filter((d) => !d.is_rookie),
    [preseasonFreeAgents],
  );

  const displacedVeteransByCategory = useMemo(
    () => buildDisplacedByCategory(displacedVeterans),
    [displacedVeterans],
  );

  const weeklyClosingGroups = useMemo(
    () => buildWeeklyClosingGroups(lastMarketWeekResult),
    [lastMarketWeekResult],
  );

  // ── Auto-scroll para a BANDA do jogador ao carregar ────────────────────────
  useEffect(() => {
    if (didInitialScrollRef.current) return; // só na 1ª carga; depois preserva o scroll do usuário
    if (!freeAgentBandOrder.length || !playerTeam?.categoria) return;
    const playerBand = bandForTier(MARKET_TIER_BY_CATEGORY[playerTeam.categoria]).key;
    const el = freeAgentSectionRefs.current[playerBand];
    const container = freeAgentContainerRef.current;
    if (el && container) {
      didInitialScrollRef.current = true;
      requestAnimationFrame(() => {
        container.scrollTop = Math.max(0, el.offsetTop - container.offsetTop - 8);
      });
    }
  }, [freeAgentBandOrder.length]); // dispara quando a lista carrega

  // ── Restaura o scroll dos painéis após avançar a semana ────────────────────
  // O grid recarrega de forma assíncrona (mostra "Carregando grid..." e esvazia o
  // conteúdo, o que zera o scrollTop). Guardamos a posição antes de avançar e a
  // devolvemos quando o grid termina de recarregar.
  useEffect(() => {
    if (loadingGrid) return;             // espera o grid voltar a ter conteúdo
    const saved = preserveScrollRef.current;
    if (!saved) return;
    preserveScrollRef.current = null;
    requestAnimationFrame(() => {
      if (mainGridRef.current) mainGridRef.current.scrollTop = saved.main;
      if (freeAgentContainerRef.current) freeAgentContainerRef.current.scrollTop = saved.aside;
    });
  }, [loadingGrid]);

  // ── Ações ───────────────────────────────────────────────────────────────────
  const handleAdvanceWeek = async () => {
    if (isAdvancingWeek) return;
    setStartError("");
    if (isComplete) {
      if (playerProposals.length > 0) return;
      if (displacedVeterans.length > 0) {
        setShowDisplacedModal(true);
        return;
      }
      // Jogador sem equipe: exibe aviso antes de confirmar
      if (!preseasonState?.player_has_team) {
        setShowFreeAgentWarning(true);
        return;
      }
      try {
        await finalizePreseason();
      } catch (e) {
        setStartError(typeof e === "string" ? e : e?.message ?? t("preSeason.errors.startSeason"));
      }
    } else {
      // Guarda o scroll atual dos painéis pra restaurar quando o grid recarregar
      // (senão a tela pula pro topo a cada avanço de semana).
      preserveScrollRef.current = {
        main: mainGridRef.current?.scrollTop ?? 0,
        aside: freeAgentContainerRef.current?.scrollTop ?? 0,
      };
      try { await advanceMarketWeek(); } catch (e) {
        preserveScrollRef.current = null;
        console.error(e);
      }
    }
  };

  const handleConfirmStartSeason = async () => {
    setShowDisplacedModal(false);
    setStartError("");
    try { await finalizePreseason(); } catch (e) {
      setStartError(typeof e === "string" ? e : e?.message ?? t("preSeason.errors.startSeason"));
    }
  };

  const handleConfirmFreeAgentStart = async () => {
    setShowFreeAgentWarning(false);
    setStartError("");
    try {
      await finalizePreseason();
    } catch (e) {
      setStartError(typeof e === "string" ? e : e?.message ?? t("preSeason.errors.startSeason"));
    }
  };

  // Janela de Transferências: aceitar uma oferta fecha a semana do jogador e assina.
  // Ao assinar com a equipe nova, repinta o carro do jogador na cor dela (o ID do
  // iRacing já foi capturado/vinculado) — silencioso, só com um toast discreto.
  const handleAcceptOffer = async (offer) => {
    if (isAdvancingWeek) return;
    setStartError("");
    try {
      await advanceMarketWeek(offer?.seat_id);
    } catch (e) {
      console.error(e);
      return;
    }
    if (!offer?.team_color) return;
    try {
      const res = await invoke("iracing_apply_market_paint", {
        careerId,
        teamColor: offer.team_color,
        category: offer.category ?? offer.category_label ?? "",
      });
      if (res) {
        setPaintToast(t("preSeason.toast.paintUpdated", { team: offer.team_name ?? t("preSeason.toast.newTeamFallback") }));
        setTimeout(() => setPaintToast(""), 6000);
      }
    } catch (e) {
      console.error("[paint] falha ao repintar no mercado:", e);
    }
  };

  // Propostas formais ("Proposta recebida"): aceitar assina (respond_to_proposal);
  // recusar dispensa. Ao aceitar, repinta o carro na cor da nova equipe (como nas ofertas).
  const handleRespondProposal = async (proposalId, accept, teamColor, category, teamName) => {
    if (isAdvancingWeek) return;
    setStartError("");
    try {
      await respondToProposal(proposalId, accept);
    } catch (e) {
      console.error(e);
      return;
    }
    if (!accept || !teamColor) return;
    try {
      const res = await invoke("iracing_apply_market_paint", {
        careerId,
        teamColor,
        category: category ?? "",
      });
      if (res) {
        setPaintToast(t("preSeason.toast.paintUpdated", { team: teamName ?? t("preSeason.toast.newTeamFallback") }));
        setTimeout(() => setPaintToast(""), 6000);
      }
    } catch (e) {
      console.error("[paint] falha ao repintar apos proposta:", e);
    }
  };

  // ── Render ──────────────────────────────────────────────────────────────────
  return (
    <div className="app-shell relative h-screen w-full overflow-hidden text-[color:var(--text-primary)]">
      <div className="app-backdrop pointer-events-none absolute inset-0" />

      {/* Toast: cor do carro atualizada ao assinar com a nova equipe */}
      {paintToast && (
        <div className="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-xl border border-[#58a6ff44] bg-[#0d1117] px-4 py-2.5 text-sm font-semibold text-[color:var(--text-primary)] shadow-2xl">
          {paintToast}
        </div>
      )}


      <div className="relative z-10 mx-auto flex h-full max-w-[1680px] flex-col px-3 pb-3 pt-3 sm:px-4 lg:px-5 xl:px-6">

        {/* ══ HEADER ══ */}
        <PreSeasonHeader
          isComplete={isComplete}
          isMarketOpen={isMarketOpen}
          playerOffers={playerOffers}
          playerProposals={playerProposals}
          selectedCat={selectedCat}
          setSelectedCat={setSelectedCat}
          currentWeek={currentWeek}
          totalWeeks={totalWeeks}
          weekProgress={weekProgress}
          currentDateLabel={currentDateLabel}
          isAdvancingWeek={isAdvancingWeek}
          handleAdvanceWeek={handleAdvanceWeek}
          startError={startError}
        />

        {/* ══ 3 COLUNAS ══ */}
        <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 xl:grid-cols-[20%_62%_18%]">

          {/* ── ESQUERDA: Mercado de Pilotos ── */}
          <FreeAgentsPanel
            freeAgentContainerRef={freeAgentContainerRef}
            freeAgentSectionRefs={freeAgentSectionRefs}
            preseasonFreeAgents={preseasonFreeAgents}
            selectedCat={selectedCat}
            visibleFreeAgentCount={visibleFreeAgentCount}
            freeAgentBandOrder={freeAgentBandOrder}
            freeAgentsByBand={freeAgentsByBand}
            setHoveredFreeAgentCat={setHoveredFreeAgentCat}
          />

          {/* ── CENTRO: Grid de Equipes ── */}
          <TeamsGridPanel
            mainGridRef={mainGridRef}
            loadingGrid={loadingGrid}
            gridData={gridData}
            sortedClasses={sortedClasses}
            groupedTeams={groupedTeams}
            hoveredFreeAgentCat={hoveredFreeAgentCat}
            setHistoryTeam={setHistoryTeam}
          />

          {/* ── DIREITA: Decisões Pendentes ── */}
          <DecisionsPanel
            playerProposals={playerProposals}
            playerOffers={playerOffers}
            playerSignedThisWindow={playerSignedThisWindow}
            playerBrand={playerBrand}
            isComplete={isComplete}
            isAdvancingWeek={isAdvancingWeek}
            totalOffers={totalOffers}
            promoOfferGroups={promoOfferGroups}
            brandOfferGroups={brandOfferGroups}
            otherOfferGroups={otherOfferGroups}
            weeklyClosingGroups={weeklyClosingGroups}
            handleRespondProposal={handleRespondProposal}
            openOffersFor={openOffersFor}
            setTransferDetail={setTransferDetail}
          />

        </div>
      </div>

      {/* ══ MODAL: Suas ofertas (fichas das equipes) ══ */}
      {showOffersModal && totalOffers > 0 && (
        <OffersModal
          offersByCategory={offersByCategory}
          offersModalCat={offersModalCat}
          totalOffers={totalOffers}
          playerTier={playerTier}
          isAdvancingWeek={isAdvancingWeek}
          onClose={() => setShowOffersModal(false)}
          onClearCat={() => setOffersModalCat(null)}
          onViewContract={handleViewContract}
        />
      )}

      {/* ══ MODAL: Contrato — documento A4 de assinatura ══ */}
      {contractOffer && (
        <ContractModal
          offer={contractOffer}
          playerName={playerName}
          isSigning={isSigning}
          isAdvancingWeek={isAdvancingWeek}
          onClose={() => setContractOffer(null)}
          onSign={handleSignContract}
        />
      )}

      {/* ══ OVERLAY: Histórico mundial de equipes (atlas, duplo clique no card) ══ */}
      {historyTeam ? (
        <TeamHistoryOverlay team={historyTeam} onClose={() => setHistoryTeam(null)} />
      ) : null}

      {/* ══ MODAL: Pilotos sem vaga ══ */}
      {showDisplacedModal && (
        <DisplacedDriversModal
          groups={displacedVeteransByCategory}
          totalCount={displacedVeterans.length}
          onClose={() => setShowDisplacedModal(false)}
          onConfirm={handleConfirmStartSeason}
        />
      )}

      {/* ══ MODAL: Detalhe da transferência ══ */}
      {transferDetail && (
        <TransferDetailModal event={transferDetail} onClose={() => setTransferDetail(null)} />
      )}

      {/* ── Modal: Iniciar temporada sem equipe ── */}
      {showFreeAgentWarning && (
        <FreeAgentWarningModal
          onClose={() => setShowFreeAgentWarning(false)}
          onConfirm={handleConfirmFreeAgentStart}
        />
      )}

    </div>
  );
}
