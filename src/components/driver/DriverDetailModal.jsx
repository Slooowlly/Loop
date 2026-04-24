import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { createPortal } from "react-dom";

import GlassButton from "../ui/GlassButton";
import FlagIcon from "../ui/FlagIcon";
import useCareerStore from "../../stores/useCareerStore";
import { formatSalary } from "../../utils/formatters";
import {
  CareerSection as CareerSectionContent,
  FormSection as FormSectionContent,
  MarketSection as MarketSectionContent,
  PerformanceSection as PerformanceSectionContent,
  formatMoment,
} from "./DriverDetailModalSections";

const DOSSIER_TABS = [
  { id: "atual", label: "Atual" },
  { id: "forma", label: "Forma" },
  { id: "carreira", label: "Carreira" },
  { id: "mercado", label: "Mercado" },
];

function Section({ title, headerLeft = null, headerRight = null, children, className = "" }) {
  return (
    <section className={["mb-5", className].join(" ")}>
      <div className="relative mb-3 min-h-[26px] border-b border-[#21262d] pb-1.5">
        <div className="relative z-[1] flex items-center gap-2 pr-8">
          <h3 className="text-[10px] font-bold uppercase tracking-[0.24em] text-[#7d8590]">
            {title}
          </h3>
          {headerLeft}
        </div>
        {headerRight ? (
          <div className="absolute inset-x-0 top-1/2 flex -translate-y-1/2 justify-center px-4 sm:px-8">
            {headerRight}
          </div>
        ) : null}
      </div>
      {children}
    </section>
  );
}

function nationalityForFlag(perfil, detail) {
  if (perfil?.bandeira && perfil?.nacionalidade) {
    return `${perfil.bandeira} ${perfil.nacionalidade}`;
  }

  if (perfil?.nacionalidade) {
    return perfil.nacionalidade;
  }

  return detail?.nacionalidade || "";
}

function BadgePill({ badge }) {
  const variants = {
    player: "bg-[#58a6ff]/18 text-[#58a6ff]",
    success: "bg-[#3fb950]/18 text-[#3fb950]",
    warning: "bg-[#d29922]/18 text-[#d29922]",
    info: "bg-white/10 text-[#c9d1d9]",
  };

  return (
    <span
      className={[
        "rounded-full px-2.5 py-1 text-[10px] font-bold uppercase tracking-[0.18em]",
        variants[badge?.variant] || variants.info,
      ].join(" ")}
    >
      {badge?.label}
    </span>
  );
}

function DossierTabs({ activeTab, onChange }) {
  return (
    <div className="sticky top-0 z-10 -mx-1 mb-5 border-b border-white/6 bg-[rgba(13,17,23,0.88)] px-1 pb-4 pt-2 backdrop-blur-xl">
      <div className="flex flex-wrap gap-2">
        {DOSSIER_TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            onClick={() => onChange(tab.id)}
            className={[
              "rounded-full border px-3 py-1.5 text-[11px] font-bold uppercase tracking-[0.18em] transition-all",
              activeTab === tab.id
                ? "border-[#58a6ff]/30 bg-[#58a6ff]/14 text-[#58a6ff]"
                : "border-white/10 bg-white/5 text-[#7d8590] hover:border-white/20 hover:text-[#c9d1d9]",
            ].join(" ")}
          >
            {tab.label}
          </button>
        ))}
      </div>
    </div>
  );
}

function PersonalityCard({ personality }) {
  if (!personality) {
    return (
      <div className="glass-light rounded-xl p-3">
        <p className="text-xs text-[#7d8590]">Sem tracos publicos visiveis.</p>
      </div>
    );
  }

  return (
    <div className="glass-light flex items-start gap-3 rounded-xl p-3">
      <span className="text-lg leading-none">{personality.emoji}</span>
      <div>
        <div className="text-sm font-semibold text-[#e6edf3]">{personality.tipo}</div>
        <div className="mt-1 text-[11px] text-[#7d8590]">{personality.descricao}</div>
      </div>
    </div>
  );
}

function HeaderPersonalityList({ competitivo }) {
  const personalities = [
    competitivo?.personalidade_primaria,
    competitivo?.personalidade_secundaria,
  ].filter(Boolean);

  if (!personalities.length) return null;

  return (
    <div className="mt-3 grid gap-2 lg:mt-auto lg:pt-2">
      {personalities.map((personality, index) => (
        <div key={`${personality.tipo}-${index}`} className="flex items-start gap-2.5">
          <span className="pt-0.5 text-sm leading-none">{personality.emoji}</span>
          <div className="min-w-0">
            <div className="text-sm font-semibold leading-tight text-[#e6edf3]">
              {personality.tipo}
            </div>
            <div className="mt-0.5 text-[11px] leading-snug text-[#7d8590]">
              {personality.descricao}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function TagRow({ tag }) {
  return (
    <div className="flex items-center gap-2 py-1">
      <span
        className="h-2 w-2 flex-shrink-0 rounded-full"
        style={{ backgroundColor: tag.color }}
      />
      <span className="text-sm text-[#e6edf3]">{tag.tag_text}</span>
      <span className="ml-auto text-[10px] italic text-[#6e7681]">
        {formatAttributeName(tag.attribute_name)}
      </span>
    </div>
  );
}

function ProsConsPanel({ competitivo, className = "" }) {
  return (
    <div
      className={[
        "flex h-[138px] min-h-0 flex-col lg:h-full",
        className,
      ].join(" ")}
    >
      <div className="grid min-h-0 flex-1 grid-cols-2 gap-5">
        <div className="min-h-0 overflow-y-auto pr-1">
          <div className="mb-2 text-[10px] font-bold uppercase tracking-[0.16em] text-[#7d8590]">
            Qualidades
          </div>
          {competitivo?.qualidades?.length ? (
            competitivo.qualidades.map((tag) => (
              <TagRow key={`${tag.attribute_name}-${tag.level}`} tag={tag} />
            ))
          ) : (
            <p className="text-xs text-[#7d8590]">Sem qualidades visiveis.</p>
          )}
        </div>

        <div className="min-h-0 overflow-y-auto border-l border-white/8 pl-5 pr-1">
          <div className="mb-2 text-[10px] font-bold uppercase tracking-[0.16em] text-[#7d8590]">
            Pontos de atencao
          </div>
          {competitivo?.defeitos?.length ? (
            competitivo.defeitos.map((tag) => (
              <TagRow key={`${tag.attribute_name}-${tag.level}`} tag={tag} />
            ))
          ) : competitivo?.neutro ? (
            <p className="text-xs italic text-[#7d8590]">
              Piloto equilibrado, sem fraquezas gritantes.
            </p>
          ) : (
            <p className="text-xs text-[#7d8590]">Sem defeitos visiveis.</p>
          )}
        </div>
      </div>
    </div>
  );
}

function MotivationBar({ value, compact = false, className = "" }) {
  const normalized = Number.isFinite(value) ? value : 0;
  const color = normalized >= 70 ? "#3fb950" : normalized >= 40 ? "#d29922" : "#f85149";

  if (compact) {
    return (
      <div
        className={[
          "flex items-center gap-2.5 rounded-full bg-transparent px-0 py-1",
          className,
        ].join(" ")}
      >
        <span className="text-[10px] font-bold uppercase tracking-[0.14em] text-[#7d8590]">
          Motivacao
        </span>
        <div className="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-[#21262d]">
          <div
            className="h-full rounded-full transition-all duration-700"
            style={{ width: `${normalized}%`, backgroundColor: color }}
          />
        </div>
      </div>
    );
  }

  return (
    <div className={["glass-light flex items-center gap-3 rounded-xl border border-white/6 px-4 py-3", className].join(" ")}>
      <span className="w-20 text-xs text-[#7d8590]">Motivacao</span>
      <div className="h-2 flex-1 overflow-hidden rounded-full bg-[#21262d]">
        <div
          className="h-full rounded-full transition-all duration-700"
          style={{ width: `${normalized}%`, backgroundColor: color }}
        />
      </div>
      <span className="w-10 text-right font-mono text-xs" style={{ color }}>
        {normalized}%
      </span>
    </div>
  );
}

function NavChevron({ direction }) {
  const path = direction === "up" ? "M3 7.5 6 4.5l3 3" : "m3 4.5 3 3 3-3";

  return (
    <svg
      viewBox="0 0 12 12"
      aria-hidden="true"
      className="h-3.5 w-3.5 flex-shrink-0"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d={path} />
    </svg>
  );
}

function DriverNavigatorButton({ label, direction, disabled, onClick }) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      className={[
        "flex h-10 w-10 items-center justify-center rounded-2xl border backdrop-blur-md transition-all duration-200 ease-out",
        disabled
          ? "cursor-not-allowed border-white/[0.05] bg-[#11151b]/90 text-[#5b616b]"
          : "border-white/[0.09] bg-[#161b22]/96 text-[#c9d1d9] shadow-[0_14px_34px_rgba(0,0,0,0.28)] hover:border-white/[0.14] hover:bg-[#1c2128] hover:text-[#e6edf3] focus-visible:border-white/[0.14] focus-visible:bg-[#1c2128] focus-visible:text-[#e6edf3]",
      ].join(" ")}
    >
      <NavChevron direction={direction} />
    </button>
  );
}

function DriverEdgeNavigator({
  drawerWidth,
  viewportWidth,
  previousDriverId,
  nextDriverId,
  onSelectDriver,
  visible,
  isClosing,
}) {
  if (!onSelectDriver || viewportWidth < 768 || !visible) return null;

  const railRight = drawerWidth + 14;

  return (
    <div
      className="animate-edge-rail-in pointer-events-auto fixed top-24 z-[61] flex flex-col gap-2 sm:top-28"
      style={{ right: `${railRight}px` }}
    >
      <DriverNavigatorButton
        label="Anterior"
        direction="up"
        disabled={!previousDriverId || isClosing}
        onClick={() => onSelectDriver(previousDriverId)}
      />
      <DriverNavigatorButton
        label="Proximo"
        direction="down"
        disabled={!nextDriverId || isClosing}
        onClick={() => onSelectDriver(nextDriverId)}
      />
    </div>
  );
}

function formatContractRole(role) {
  if (role === "Numero1" || role === "N1" || role === "Piloto N1") return "Piloto N1";
  if (role === "Numero2" || role === "N2" || role === "Piloto N2") return "Piloto N2";
  return role || "-";
}

function DetailRow({ label, value, valueClassName = "text-[#e6edf3]" }) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-white/6 py-2 last:border-b-0 last:pb-0">
      <span className="text-[11px] uppercase tracking-[0.16em] text-[#7d8590]">{label}</span>
      <span className={["text-right text-sm font-medium", valueClassName].join(" ")}>{value}</span>
    </div>
  );
}

function PerformanceSection({ title, stats }) {
  return <PerformanceSectionContent SectionComponent={Section} title={title} stats={stats} />;
}

function CurrentMomentSection({ forma, moment, contract }) {
  return (
    <Section title="Momento Atual">
      <div className="grid gap-4 lg:grid-cols-[1.1fr_0.9fr]">
        <div className="glass-light rounded-xl p-4">
          <div className="mb-2 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            Forma recente
          </div>
          <div className="grid gap-3">
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">
                Media recente
              </div>
              <div className="mt-1 flex items-center gap-2 text-2xl font-bold text-[#e6edf3]">
                <span>{formatAverage(forma?.media_chegada)}</span>
                <span className="text-xl text-[#7d8590]">{forma?.tendencia || "->"}</span>
              </div>
            </div>
            <div className="rounded-xl border border-white/6 bg-black/10 p-3">
              <DetailRow label="Status de forma" value={moment.label} valueClassName={moment.color} />
              <DetailRow label="Tendencia" value={forma?.tendencia || "->"} />
            </div>
          </div>
        </div>

        <div className="glass-light rounded-xl p-4">
          <div className="mb-2 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            Situacao contratual
          </div>
          {contract ? (
            <div className="grid gap-1">
              <DetailRow label="Equipe" value={contract.equipe_nome || "-"} />
              <DetailRow
                label="Funcao"
                value={formatContractRole(contract.papel)}
              />
              <DetailRow label="Salario anual" value={formatSalary(contract.salario_anual)} />
              <DetailRow
                label="Expira em"
                value={`${contract.anos_restantes} ano${contract.anos_restantes !== 1 ? "s" : ""}`}
              />
              <DetailRow
                label="Vigencia"
                value={`Temporada ${contract.temporada_inicio} ate ${contract.temporada_fim}`}
              />
            </div>
          ) : (
            <p className="text-sm text-[#7d8590]">Sem contrato ativo no momento.</p>
          )}
        </div>
      </div>
    </Section>
  );
}

function FormSection({ detail, moment }) {
  return <FormSectionContent SectionComponent={Section} detail={detail} moment={moment} />;
}

function CareerSection({ detail, trajetoria }) {
  return <CareerSectionContent SectionComponent={Section} detail={detail} trajetoria={trajetoria} />;
}

function MarketSection({ detail, market }) {
  return <MarketSectionContent SectionComponent={Section} detail={detail} market={market} />;
}

function formatAttributeName(name) {
  const map = {
    skill: "Velocidade",
    consistencia: "Consistência",
    racecraft: "Racecraft",
    defesa: "Defesa",
    ritmo_classificacao: "Classificação",
    gestao_pneus: "Pneus",
    habilidade_largada: "Largada",
    adaptabilidade: "Adaptabilidade",
    fator_chuva: "Chuva",
    fitness: "Forma Fisica",
    experiencia: "Experiencia",
    desenvolvimento: "Desenvolvimento",
    aggression: "Agressividade",
    smoothness: "Suavidade",
    midia: "Midia",
    mentalidade: "Mentalidade",
    confianca: "Confianca",
  };

  return map[name] || name;
}

function formatAverage(value) {
  if (value === null || value === undefined) return "-";
  return value.toFixed(1);
}

export default function DriverDetailModal({
  driverId,
  driverIds = [],
  onSelectDriver = null,
  onClose,
}) {
  const CLOSE_ANIMATION_MS = 280;
  const careerId = useCareerStore((state) => state.careerId);
  const [detail, setDetail] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [activeTab, setActiveTab] = useState("atual");
  const [isClosing, setIsClosing] = useState(false);
  const [showEdgeNavigator, setShowEdgeNavigator] = useState(false);
  const [viewportWidth, setViewportWidth] = useState(() => window.innerWidth);
  const closeTimeoutRef = useRef(null);
  const edgeNavigatorTimeoutRef = useRef(null);
  const hasShownEdgeNavigatorRef = useRef(false);

  useEffect(() => {
    let active = true;

    async function fetchDetail() {
      if (!driverId || !careerId) {
        if (active) {
          setLoading(false);
          setError("");
          setDetail(null);
        }
        return;
      }

      setLoading(true);
      setError("");
      setDetail(null);

      try {
        const data = await invoke("get_driver_detail", { careerId, driverId });
        if (active) setDetail(data);
      } catch (fetchError) {
        if (active) {
          setError(
            typeof fetchError === "string"
              ? fetchError
              : fetchError?.toString?.() ?? "Erro ao carregar piloto.",
          );
        }
      } finally {
        if (active) setLoading(false);
      }
    }

    fetchDetail();
    return () => {
      active = false;
    };
  }, [careerId, driverId]);

  useEffect(() => {
    function handleEsc(event) {
      if (event.key === "Escape") requestClose();
    }

    window.addEventListener("keydown", handleEsc);
    return () => window.removeEventListener("keydown", handleEsc);
  }, [isClosing, onClose]);

  useEffect(() => {
    function handleResize() {
      setViewportWidth(window.innerWidth);
    }

    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  useEffect(() => {
    setActiveTab("atual");
    setIsClosing(false);
    window.clearTimeout(closeTimeoutRef.current);
    window.clearTimeout(edgeNavigatorTimeoutRef.current);

    if (!hasShownEdgeNavigatorRef.current) {
      setShowEdgeNavigator(false);
      edgeNavigatorTimeoutRef.current = window.setTimeout(() => {
        hasShownEdgeNavigatorRef.current = true;
        setShowEdgeNavigator(true);
      }, CLOSE_ANIMATION_MS);
    } else {
      setShowEdgeNavigator(true);
    }

    return () => {
      window.clearTimeout(closeTimeoutRef.current);
      window.clearTimeout(edgeNavigatorTimeoutRef.current);
    };
  }, [driverId]);

  function requestClose() {
    if (isClosing) return;

    setIsClosing(true);
    setShowEdgeNavigator(false);
    window.clearTimeout(closeTimeoutRef.current);
    window.clearTimeout(edgeNavigatorTimeoutRef.current);
    closeTimeoutRef.current = window.setTimeout(() => {
      onClose();
    }, CLOSE_ANIMATION_MS);
  }

  function selectAdjacentDriver(targetDriverId) {
    if (!targetDriverId || !onSelectDriver || isClosing) return;
    onSelectDriver(targetDriverId);
  }

  const perfil = detail?.perfil;
  const competitivo = detail?.competitivo;
  const forma = detail ? detail.forma : null;
  const trajetoria = detail ? detail.trajetoria : null;
  const contract = detail?.contrato_mercado?.contrato;
  const market = detail?.contrato_mercado?.mercado;
  const moment = formatMoment(detail ? detail.forma?.momento : null);
  const titleCount = trajetoria?.titulos ?? 0;
  const hasChampionship = Boolean(trajetoria?.foi_campeao);
  const licenseLevelBadge = detail?.perfil?.licenca?.nivel
    ? {
        label: detail.perfil.licenca.nivel,
        variant: "info",
      }
    : null;
  const visibleBadges = perfil?.badges?.filter((badge) => badge.label !== "ROOKIE") || [];
  const currentDriverIndex = driverIds.indexOf(driverId);
  const previousDriverId = currentDriverIndex > 0 ? driverIds[currentDriverIndex - 1] : null;
  const nextDriverId =
    currentDriverIndex >= 0 && currentDriverIndex < driverIds.length - 1
      ? driverIds[currentDriverIndex + 1]
      : null;
  const profileHeaderMeta = (
    <div className="flex w-full items-center justify-center">
      <MotivationBar
        value={competitivo?.motivacao}
        compact
        className="min-w-[220px] sm:min-w-[320px] lg:min-w-[420px]"
      />
    </div>
  );
  const drawerWidth =
    viewportWidth >= 1280
      ? Math.floor(viewportWidth * 0.5)
      : viewportWidth >= 768
        ? Math.floor(viewportWidth * 0.72)
        : viewportWidth;

  const drawerContent = (
    <div className="pointer-events-none fixed inset-0 z-[60]">
      <button
        type="button"
        className={[
          "pointer-events-auto fixed inset-0 bg-black/18 backdrop-blur-[1px]",
          isClosing ? "animate-fade-out" : "animate-fade-in",
        ].join(" ")}
        onClick={requestClose}
        aria-label="Fechar ficha do piloto"
      />

      <DriverEdgeNavigator
        drawerWidth={drawerWidth}
        viewportWidth={viewportWidth}
        previousDriverId={previousDriverId}
        nextDriverId={nextDriverId}
        onSelectDriver={selectAdjacentDriver}
        visible={showEdgeNavigator && !isClosing}
        isClosing={isClosing}
      />

      <div
        className={[
          "glass-strong pointer-events-auto fixed inset-y-0 right-0 overflow-y-auto border-l border-white/10 shadow-[-24px_0_60px_rgba(0,0,0,0.34)]",
          isClosing ? "animate-drawer-out" : "animate-drawer-in",
        ].join(" ")}
        onClick={(event) => event.stopPropagation()}
        style={{ width: `${drawerWidth}px` }}
      >
        {perfil ? (
          <div
            className="h-1"
            style={{
              background: perfil.equipe_cor_primaria || "#21262d",
            }}
          />
        ) : null}

        {loading ? (
          <div className="p-12 text-center">
            <div className="mb-4 text-4xl animate-pulse">🏎️</div>
            <p className="text-[#7d8590]">Carregando dados do piloto...</p>
          </div>
        ) : null}

        {!loading && error ? (
          <div className="p-8 text-center">
            <p className="mb-4 text-[#f85149]">❌ {error}</p>
            <GlassButton variant="secondary" onClick={requestClose}>
              Fechar
            </GlassButton>
          </div>
        ) : null}

        {!loading && !error && detail ? (
          <div className="relative min-h-full p-6 sm:p-7">
            <button
              type="button"
              onClick={requestClose}
              className="absolute right-4 top-4 flex h-8 w-8 items-center justify-center rounded-lg text-lg text-[#7d8590] transition-colors hover:bg-[#21262d] hover:text-[#e6edf3]"
              aria-label="Fechar modal do piloto"
            >
              ✕
            </button>

            <Section
              title="Perfil"
              headerLeft={licenseLevelBadge ? <BadgePill badge={licenseLevelBadge} /> : null}
              headerRight={profileHeaderMeta}
            >
              <div className="pr-8">
                <div className="grid gap-4 lg:min-h-[146px] lg:h-[146px] lg:grid-cols-[300px_minmax(0,1fr)] lg:items-start">
                  <div className="min-w-0 lg:flex lg:h-full lg:max-w-[300px] lg:flex-col">
                    <div className="mb-2 flex flex-wrap items-center gap-2">
                      <FlagIcon
                        nacionalidade={nationalityForFlag(perfil, detail)}
                        className="h-6 w-9 rounded-md"
                      />
                      <div className="inline-flex items-center gap-2 self-center leading-none">
                        <h2 className="text-2xl font-bold leading-none text-[#e6edf3]">
                          {detail.nome}
                        </h2>
                        <span className="relative top-[3px] self-center text-sm leading-none text-[#7d8590]">
                          {perfil?.idade ?? detail.idade} anos
                        </span>
                      </div>
                    </div>

                    <div className="mb-3 text-sm text-[#c9d1d9]">
                      {detail.papel === "Numero1"
                        ? "N1"
                        : detail.papel === "Numero2"
                          ? "N2"
                          : detail.papel || "-"}
                      {perfil?.equipe_nome ? ` - ${perfil.equipe_nome}` : " - Sem equipe"}
                    </div>

                    {visibleBadges.length ? (
                      <div className="mb-3 flex flex-wrap gap-2">
                        {visibleBadges.map((badge) => (
                          <BadgePill key={`${badge.label}-${badge.variant}`} badge={badge} />
                        ))}
                      </div>
                    ) : null}

                    <HeaderPersonalityList competitivo={competitivo} />
                  </div>

                  <div className="lg:flex lg:h-full lg:items-start lg:pt-4">
                    <ProsConsPanel competitivo={competitivo} className="h-[118px] w-full lg:h-[118px]" />
                  </div>
                </div>
              </div>
            </Section>

            <DossierTabs activeTab={activeTab} onChange={setActiveTab} />

            {activeTab === "atual" ? (
              <>
                <PerformanceSection title="Temporada" stats={detail.performance?.temporada} />
                <CurrentMomentSection forma={forma} moment={moment} contract={contract} />
              </>
            ) : null}

            {activeTab === "forma" ? <FormSection detail={detail} moment={moment} /> : null}

            {activeTab === "carreira" ? (
              <CareerSection detail={detail} trajetoria={trajetoria} />
            ) : null}

            {activeTab === "mercado" ? (
              <MarketSection detail={detail} market={market} />
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );

  if (typeof document === "undefined") return null;
  return createPortal(drawerContent, document.body);
}
