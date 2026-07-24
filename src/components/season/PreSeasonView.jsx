import { useState, useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { currentLang } from "../../i18n/format.js";
import i18n from "../../i18n/index.js";
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
  CATEGORIES,
  CLASS_LABELS,
  CLASS_PRIORITY,
  MULTICLASS_ORDER,
  FREE_AGENT_ORDER,
  LEVEL_BANDS,
  bandForTier,
  MARKET_TIER_BY_CATEGORY,
  CATEGORY_TIER,
  shortCatName,
  subcatLabel,
  subcatColor,
  brandOf,
  is_regular_market_category,
  playerCatToFilter,
  buildWeeklyClosingGroups,
} from "./preSeasonFormatters.js";

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

  // Ofertas agrupadas por categoria (N1/N2 dentro). Ordem: MARCA do jogador primeiro
  // (ex.: Mazda antes de Toyota) e, dentro de cada marca, tier maior primeiro (Cup antes
  // de Rookie). As demais marcas vêm depois, agrupadas, também por tier decrescente.
  const offersByCategory = useMemo(() => {
    const groups = new Map();
    for (const offer of playerOffers) {
      const baseCat = offer.category || "outras";
      // Production/Endurance dividem por CLASSE (carro): chave "categoria:classe".
      const isMulti =
        (baseCat === "production_challenger" || baseCat === "endurance") && offer.class;
      const key = isMulti ? `${baseCat}:${offer.class}` : baseCat;
      if (!groups.has(key)) {
        groups.set(key, {
          cat: key,
          baseCat,
          classe: isMulti ? offer.class : null,
          tier: offer.category_tier ?? CATEGORY_TIER[baseCat] ?? 0,
          label: isMulti
            ? `${shortCatName(baseCat)} · ${CLASS_LABELS[offer.class] ?? offer.class.toUpperCase()}`
            : offer.category_label || subcatLabel(baseCat),
          n1: [],
          n2: [],
        });
      }
      const g = groups.get(key);
      if (offer.role === "N1") g.n1.push(offer);
      else g.n2.push(offer);
    }
    // Bucket de ordenação: 0 = PROMOÇÃO (tier acima do jogador, sempre no topo),
    // 1 = marca do jogador, 2 = demais marcas. Usa a categoria BASE (não a classe).
    // Tier de EXIBIÇÃO usa CATEGORY_TIER (distingue Production=3 de BMW=2, que no
    // backend são ambos tier 2).
    const bucketOf = (g) => {
      if (playerTier != null && g.tier > playerTier) return 0;
      if (playerBrand && brandOf(g.baseCat) === playerBrand) return 1;
      return 2;
    };
    for (const g of groups.values()) g.bucket = bucketOf(g);
    const dispTier = (g) => CATEGORY_TIER[g.baseCat] ?? g.tier;
    return [...groups.values()].sort((a, b) => {
      if (a.bucket !== b.bucket) return a.bucket - b.bucket;
      // Ordena por NÍVEL da categoria: maior no topo, rookies no fundo.
      // (GT3 > GT4 > Production > BMW/Cup > Rookie.)
      const dt = dispTier(b) - dispTier(a);
      if (dt !== 0) return dt;
      // Mesmo nível, mesma categoria multiclasse → ordem MULTICLASS_ORDER das classes.
      if (a.baseCat === b.baseCat && a.classe && b.classe) {
        const order = MULTICLASS_ORDER[a.baseCat] ?? [];
        const ia = order.indexOf(a.classe);
        const ib = order.indexOf(b.classe);
        if (ia !== ib) return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
      }
      // Mesmo nível, marcas diferentes (ex.: BMW vs Cups) → desempate por marca
      // (bmw < mazda < toyota), deixando o BMW acima das cups.
      const na = brandOf(a.baseCat) ?? "";
      const nb = brandOf(b.baseCat) ?? "";
      if (na !== nb) return na < nb ? -1 : 1;
      return 0;
    });
  }, [playerOffers, playerBrand, playerTier]);

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
        const dbIds = new Set();
        if (selectedCat === "all") {
          CATEGORIES.filter((c) => !c.isSeparator && c.id !== "all").forEach((c) =>
            c.dbIds?.forEach((id) => dbIds.add(id)),
          );
        } else {
          const cfg = CATEGORIES.find((c) => c.id === selectedCat);
          if (cfg) cfg.dbIds?.forEach((id) => dbIds.add(id));
        }

        // Busca PARALELA por categoria (era sequencial → grid demorava a refletir as
        // assinaturas após avançar a semana). Tag cada equipe com o dbId usado.
        const perCategory = await Promise.all(
          [...dbIds].map((dbId) =>
            invoke("get_teams_standings", { careerId, category: dbId })
              .then((teams) => teams.map((t) => ({ ...t, _categoria: dbId })))
              .catch(() => []),
          ),
        );
        const all = perCategory.flat();

        // Filtrar por classe quando categoria tem filterClass
        let final = all;
        if (selectedCat !== "all") {
          const cfg = CATEGORIES.find((c) => c.id === selectedCat);
          if (cfg?.filterClass) {
            final = all.filter((t) => {
              if (t.classe === cfg.filterClass) return true;
              if (t._categoria?.startsWith(cfg.filterClass)) return true;
              if (cfg.filterClass === "bmw" && t._categoria === "bmw_m2") return true;
              return false;
            });
          }
        }

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
  const groupedTeams = useMemo(() => {
    const grouped = {};
    gridData.forEach((team) => {
      const key = team._categoria === "endurance" || team._categoria === "production_challenger"
        ? team._categoria
        : team.classe || team._categoria || "outras";
      grouped[key] = grouped[key] ?? [];
      grouped[key].push(team);
    });
    return grouped;
  }, [gridData]);

  const sortedClasses = useMemo(() => {
    return Object.keys(groupedTeams).sort((a, b) => {
      const pa = CLASS_PRIORITY.indexOf(a);
      const pb = CLASS_PRIORITY.indexOf(b);
      if (pa !== -1 && pb !== -1) return pa - pb;
      if (pa !== -1) return -1;
      if (pb !== -1) return 1;
      return a.localeCompare(b);
    });
  }, [groupedTeams]);

  // ── Free agents agrupados por FAIXA DE NÍVEL (onde correm hoje) ─────────────
  // Chave = banda do tier (market_tier), não a categoria/carteira. Dentro da banda,
  // pilotos "frescos" primeiro e os "parados" no fim (marcador de inatividade).
  const freeAgentsByBand = useMemo(() => {
    // Filtro do topo também recorta a coluna: mostra só quem pode pegar vaga na
    // categoria selecionada (interseção com eligible_categories, vindo do backend).
    const filterCfg = selectedCat === "all" ? null : CATEGORIES.find((c) => c.id === selectedCat);
    const filterDbIds = filterCfg?.dbIds ? new Set(filterCfg.dbIds) : null;
    const grouped = {};
    (preseasonFreeAgents ?? []).forEach((d) => {
      const cat = d.categoria || "outras";
      if (!is_regular_market_category(cat)) return;
      if (filterDbIds && !(d.eligible_categories ?? []).some((id) => filterDbIds.has(id))) return;
      const band = bandForTier(d.market_tier);
      (grouped[band.key] = grouped[band.key] ?? []).push(d);
    });
    Object.values(grouped).forEach((list) =>
      list.sort((a, b) => {
        // 1) Agrupa por marca/categoria dentro da banda: Toyota e Mazda têm a mesma cor,
        //    então intercalá-los confunde — cada marca vira uma sequência contígua.
        const pa = FREE_AGENT_ORDER.indexOf(a.categoria);
        const pb = FREE_AGENT_ORDER.indexOf(b.categoria);
        const oa = pa === -1 ? 999 : pa;
        const ob = pb === -1 ? 999 : pb;
        if (oa !== ob) return oa - ob;
        // 2) Dentro da marca: fresco antes do parado.
        const ia = a.seasons_idle ?? 0;
        const ib = b.seasons_idle ?? 0;
        if (ia !== ib) return ia - ib;
        // 3) Por nome.
        return (a.driver_name ?? "").localeCompare(b.driver_name ?? "");
      }),
    );
    return grouped;
  }, [preseasonFreeAgents, selectedCat]);

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

  const displacedVeteransByCategory = useMemo(() => {
    const grouped = {};

    displacedVeterans.forEach((driver) => {
      const category = driver.categoria || "outras";
      if (!is_regular_market_category(category)) return;
      grouped[category] = grouped[category] ?? [];
      grouped[category].push(driver);
    });

    return Object.entries(grouped)
      .sort(([a], [b]) => {
        const pa = FREE_AGENT_ORDER.indexOf(a);
        const pb = FREE_AGENT_ORDER.indexOf(b);
        if (pa !== -1 && pb !== -1) return pa - pb;
        if (pa !== -1) return -1;
        if (pb !== -1) return 1;
        return a.localeCompare(b);
      })
      .map(([category, drivers]) => ({
        category,
        color: subcatColor(category),
        label: subcatLabel(category),
        drivers,
      }));
  }, [displacedVeterans]);

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
