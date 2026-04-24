import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import GlassCard from "../../components/ui/GlassCard";
import NewsScopeDrawers from "./NewsScopeDrawers";
import useCareerStore from "../../stores/useCareerStore";
import {
  buildFallbackPrimaryFilters,
  contextChipStyle,
  contextChipToneClass,
  getReadableTeamColor,
  isUpcomingRaceFilter,
  leadBadgeLabel,
  resolveStoryBlocks,
  storyToneBadgeClass,
  toneDotClass,
} from "./newsHelpers";

function NewsTab() {
  const careerId = useCareerStore((state) => state.careerId);
  const season = useCareerStore((state) => state.season);
  const [bootstrap, setBootstrap] = useState(null);
  const [snapshot, setSnapshot] = useState(null);
  const [requestState, setRequestState] = useState({
    scopeType: "category",
    scopeId: "",
    scopeClass: null,
    primaryFilter: null,
    contextType: null,
    contextId: null,
  });
  const [selectedStoryId, setSelectedStoryId] = useState(null);
  const [loadingBootstrap, setLoadingBootstrap] = useState(true);
  const [loadingSnapshot, setLoadingSnapshot] = useState(true);
  const [error, setError] = useState("");
  useEffect(() => {
    let mounted = true;

    async function loadBootstrap() {
      if (!careerId) {
        if (mounted) {
          setBootstrap(null);
          setSnapshot(null);
          setLoadingBootstrap(false);
          setLoadingSnapshot(false);
          setSelectedStoryId(null);
        }
        return;
      }

      setLoadingBootstrap(true);
      setError("");

      try {
        const data = await invoke("get_news_tab_bootstrap", { careerId });
        if (!mounted) return;

        setBootstrap(data);
        setRequestState({
          scopeType: data.default_scope_type,
          scopeId: data.default_scope_id,
          scopeClass: null,
          primaryFilter: data.default_primary_filter ?? null,
          contextType: data.default_context_type ?? null,
          contextId: data.default_context_id ?? null,
        });
        setSelectedStoryId(null);
      } catch (invokeError) {
        if (!mounted) return;
        setError(
          typeof invokeError === "string"
            ? invokeError
            : invokeError?.toString?.() ?? "Não foi possível carregar a central de notícias.",
        );
      } finally {
        if (mounted) {
          setLoadingBootstrap(false);
        }
      }
    }

    void loadBootstrap();

    return () => {
      mounted = false;
    };
  }, [careerId]);

  useEffect(() => {
    let mounted = true;

    async function loadSnapshot() {
      if (!careerId || !requestState.scopeId) {
        if (mounted) {
          setSnapshot(null);
          setLoadingSnapshot(false);
          setSelectedStoryId(null);
        }
        return;
      }

      setLoadingSnapshot(true);
      setError("");

      try {
        const data = await invoke("get_news_tab_snapshot", {
          careerId,
          request: {
            scope_type: requestState.scopeType,
            scope_id: requestState.scopeId,
            scope_class: requestState.scopeClass,
            primary_filter: requestState.primaryFilter,
            context_type: requestState.contextType,
            context_id: requestState.contextId,
          },
        });

        if (!mounted) return;
        setSnapshot(data);
      } catch (invokeError) {
        if (!mounted) return;
        setError(
          typeof invokeError === "string"
            ? invokeError
            : invokeError?.toString?.() ?? "Não foi possível atualizar a leitura editorial.",
        );
      } finally {
        if (mounted) {
          setLoadingSnapshot(false);
        }
      }
    }

    void loadSnapshot();

    return () => {
      mounted = false;
    };
  }, [
    careerId,
    requestState.scopeType,
    requestState.scopeId,
    requestState.scopeClass,
    requestState.primaryFilter,
    requestState.contextType,
    requestState.contextId,
    season?.numero,
    season?.rodada_atual,
  ]);

  useEffect(() => {
    setSelectedStoryId(snapshot?.stories?.[0]?.id ?? null);
  }, [snapshot]);

  const scopeTabs = bootstrap?.scopes ?? [];
  const activeScope = scopeTabs.find(
    (scope) => scope.id === requestState.scopeId && scope.scope_type === requestState.scopeType,
  );
  const primaryFilters = snapshot?.primary_filters ?? buildFallbackPrimaryFilters(requestState.scopeType);
  const contextualFilters = snapshot?.contextual_filters ?? [];
  const stories = snapshot?.stories ?? [];
  const selectedStory = stories.find((story) => story.id === selectedStoryId) ?? stories[0] ?? null;
  const hasActivePrimaryFilter = Boolean(requestState.primaryFilter);
  const isLoading = loadingBootstrap || loadingSnapshot;

  function handleScopeChange(nextSelection) {
    const scope = nextSelection?.scope ?? nextSelection;
    setRequestState((current) => {
      const allowedPrimaryFilterIds = buildFallbackPrimaryFilters(scope.scope_type).map((filter) => filter.id);
      const preservedPrimaryFilter =
        current.primaryFilter && allowedPrimaryFilterIds.includes(current.primaryFilter)
          ? current.primaryFilter
          : null;

      return {
        scopeType: scope.scope_type,
        scopeId: scope.id,
        scopeClass: nextSelection?.scopeClass ?? null,
        primaryFilter: preservedPrimaryFilter,
        contextType: null,
        contextId: null,
      };
    });
    setSelectedStoryId(null);
  }

  function handlePrimaryFilterChange(filterId) {
    setRequestState((current) => ({
      ...current,
      primaryFilter: current.primaryFilter === filterId ? null : filterId,
      contextType: null,
      contextId: null,
    }));
    setSelectedStoryId(null);
  }

  function handleContextFilterClick(filter) {
    const nextContextType = filter.kind ?? null;
    const isSameSelection = requestState.contextType === nextContextType && requestState.contextId === filter.id;

    setRequestState((current) => ({
      ...current,
      contextType: isSameSelection ? null : nextContextType,
      contextId: isSameSelection ? null : filter.id,
    }));
    setSelectedStoryId(null);
  }

  return (
    <div className="space-y-6">
      <section data-news-section="hero" className="mx-auto w-full max-w-[1400px]">
        <div className="rounded-[32px] overflow-hidden relative border border-white/[0.06] shadow-[0_8px_32px_rgba(0,0,0,0.4)]" style={{ background: "linear-gradient(180deg, rgba(22, 27, 34, 0.8) 0%, rgba(13, 17, 23, 0.8) 100%)", backdropFilter: "blur(20px)" }}>
          <div className="absolute inset-0 bg-[url('https://images.unsplash.com/photo-1541348263662-e068c2ee03e7?auto=format&fit=crop&q=80&w=1500')] bg-cover bg-center opacity-5 mix-blend-screen pointer-events-none"></div>

          <div className="px-6 pt-10 pb-6 sm:px-10 lg:pt-10 flex flex-col md:flex-row md:justify-between items-start border-b border-white/5 relative z-10">
            <div className="space-y-3">
               <div className="inline-flex items-center gap-2">
                  <div className="w-1.5 h-1.5 rounded-full bg-accent-primary"></div>
                  <h3 className="text-[10px] font-bold uppercase tracking-[0.2em] text-accent-primary">
                     {snapshot?.hero?.section_label ?? "Diretoria de Imprensa"}
                  </h3>
               </div>
               
               <h1 className="text-[2.2rem] sm:text-[2.5rem] font-extrabold leading-none tracking-tight text-white drop-shadow-sm">
                 {snapshot?.hero?.title ?? "Panorama do Campeonato"}
               </h1>
               <div className="flex items-center gap-3">
                  <span className="inline-flex items-center rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-semibold text-white">
                     Publicada em: {bootstrap?.pub_date_label ?? String(bootstrap?.season_year ?? "")}
                  </span>
                  <span className="text-sm font-medium text-text-secondary">
                     {stories.length} {stories.length === 1 ? "matéria na edição" : "matérias na edição"}
                  </span>
               </div>
            </div>

            <div className="rounded-2xl border border-white/10 bg-black/40 backdrop-blur-md px-6 py-4 text-right shadow-xl mt-6 md:mt-0">
               <p className="text-[10px] font-bold uppercase tracking-widest text-[#8b949e]">
                 Próxima Etapa • <span className="text-white">
                   {bootstrap ? `Etapa ${bootstrap.current_round} / ${bootstrap.total_rounds}` : ""}
                 </span>
               </p>
               <p className="text-[1.3rem] font-extrabold text-white mt-1 leading-tight">
                 {bootstrap?.next_race_date_label ?? "—"}
               </p>
               <p className="text-sm font-semibold text-text-secondary mt-0.5">
                 {bootstrap?.next_race_name ?? ""}
               </p>
            </div>
          </div>
            <div className="mt-2 w-full flex flex-col z-10">
              <NewsScopeDrawers
                scopeTabs={scopeTabs}
                requestState={requestState}
                showDrawer={Boolean(snapshot)}
                onScopeChange={handleScopeChange}
                renderPrimaryFilters={() => snapshot ? (
                  <>
                    {primaryFilters.map((filter) => {
                      const isActive = requestState.primaryFilter === filter.id;
                      return (
                        <button
                          key={filter.id}
                          type="button"
                          onClick={() => handlePrimaryFilterChange(filter.id)}
                          className={[
                            "flex items-center gap-2 rounded-full px-4 py-1.5 text-sm transition-all",
                            isActive
                              ? filter.id === "Mercado"
                                ? "font-bold text-status-yellow bg-status-yellow/10 shadow-[0_0_0_1px_rgba(210,153,34,0.3)]"
                                : "font-bold text-accent-primary bg-accent-primary/10 shadow-[0_0_0_1px_rgba(88,166,255,0.2)]"
                              : "font-medium text-text-secondary hover:text-white",
                          ].join(" ")}
                        >
                          {filter.label}
                        </button>
                      );
                    })}
                  </>
                ) : null}
                renderContextualFilters={() => snapshot && hasActivePrimaryFilter && contextualFilters.length > 0 ? (
                  <>
                    {contextualFilters.map((filter, index) => {
                      const isActive =
                        requestState.contextType === (filter.kind ?? null) && requestState.contextId === filter.id;
                      const isUpcomingRace =
                        requestState.primaryFilter === "Corridas"
                        && isUpcomingRaceFilter(filter, bootstrap)
                        && !isActive;
                      const chipToneClass = contextChipToneClass(filter, requestState.primaryFilter, index, isActive);
                      return (
                        <button
                          key={`${filter.kind ?? "context"}-${filter.id}`}
                          type="button"
                          disabled={isUpcomingRace}
                          onClick={() => handleContextFilterClick(filter)}
                          className={[
                            "rounded-full border px-4 py-1.5 text-[0.85rem] font-semibold text-center transition-glass",
                            chipToneClass
                              ? chipToneClass
                              : isActive
                              ? "border-accent-primary/30 bg-accent-primary/10 shadow-[0_0_12px_rgba(88,166,255,0.1)]"
                              : "border-white/10 bg-white/[0.04] hover:bg-white/10 hover:text-white border-transparent",
                            isUpcomingRace ? "cursor-not-allowed opacity-35" : "",
                          ].join(" ")}
                          style={contextChipStyle(filter, isActive)}
                        >
                          <div className="flex items-center justify-center gap-2.5">
                            {requestState.primaryFilter === "Corridas" ? null : (
                              <span className={["h-2 w-2 rounded-full", toneDotClass(filter.tone)].join(" ")} />
                            )}
                            <span
                              className={isActive ? "text-text-primary" : "text-text-secondary"}
                              style={filter.kind === "team" && filter.color_primary ? { color: getReadableTeamColor(filter.color_primary) } : undefined}
                            >
                              {filter.label}
                            </span>
                          </div>
                        </button>
                      );
                    })}
                  </>
                ) : null}
              />
            </div>
          </div>
      </section>

      {snapshot ? (
        <>
          <section data-news-section="main-reader" className="mx-auto w-full max-w-[1400px]">
            <div className="grid gap-6 xl:grid-cols-[1.6fr_1fr] items-stretch">
              
              {/* Leitura Principal (Lado Esquerdo) */}
              <div
                className="relative overflow-hidden rounded-[32px] border border-white/[0.06] shadow-[0_8px_32px_rgba(0,0,0,0.4)] flex flex-col group p-0"
                style={{ background: "linear-gradient(180deg, rgba(22, 27, 34, 0.8) 0%, rgba(13, 17, 23, 0.8) 100%)", backdropFilter: "blur(20px)" }}
              >
                <div className="absolute inset-0 bg-[url('https://images.unsplash.com/photo-1568605117036-5fe5e7bab0b7?auto=format&fit=crop&q=80')] bg-cover bg-center transition-transform duration-1000 group-hover:scale-105 opacity-60 mix-blend-luminosity"></div>
                <div className="absolute inset-0 bg-gradient-to-t from-[#0a0d14] via-[#0a0d14]/80 to-transparent"></div>
                
                <div className="relative z-10 flex-1 flex flex-col p-8 lg:p-10 justify-end min-h-[500px]">
                  {error ? (
                    <p className="text-sm font-semibold text-status-red">{error}</p>
                  ) : selectedStory ? (
                    <OpenStory story={selectedStory} />
                  ) : (
                    <ReaderEmptyState loading={isLoading} />
                  )}
                </div>
              </div>

              {/* Leituras do Recorte (Lado Direito) */}
              <div
                className="rounded-[32px] p-8 flex flex-col border border-white/5 transition-all"
                style={{ background: "linear-gradient(180deg, rgba(255, 255, 255, 0.03) 0%, rgba(255, 255, 255, 0.01) 100%)", backgroundColor: "#11161d" }}
              >
                <div className="mb-6 flex items-center justify-between">
                   <h3 className="text-xs font-bold uppercase tracking-[0.15em] text-text-muted">Acontecimentos Secundários</h3>
                   <span className="text-xs font-bold bg-white/10 text-white rounded-full px-2.5 py-0.5">{stories.length}</span>
                </div>
                
                <div className="space-y-4 overflow-y-auto pr-2 no-scrollbar flex-1 relative max-h-[600px]">
                  {stories.length > 0 ? (
                    stories.map((story) => (
                      <StoryListItem
                        key={story.id}
                        story={story}
                        active={selectedStory?.id === story.id}
                        onClick={() => setSelectedStoryId(story.id)}
                      />
                    ))
                  ) : (
                    <p className="text-sm text-text-muted">
                      {isLoading ? "Montando o briefing atual..." : "Não há histórias disponíveis neste recorte."}
                    </p>
                  )}
                </div>
              </div>

            </div>
          </section>
        </>
      ) : null}
    </div>
  );
}

function OpenStory({ story }) {
  const storyHeadline = story.headline ?? story.title ?? "Leitura atual";
  const storyDeck = story.deck ?? story.summary ?? "";
  const storyBlocks = resolveStoryBlocks(story);

  return (
    <div className="space-y-4 max-w-2xl flex flex-col justify-end">
      {/* Badges */}
      <div className="flex items-center gap-2">
         {story.news_type && (
           <span className={["inline-flex items-center px-3 py-1 rounded-md text-[11px] font-bold uppercase tracking-wider backdrop-blur-md border shadow-sm", storyToneBadgeClass(story.accent_tone)].join(" ")}>
             {story.news_type}
           </span>
         )}
         {story.importance && (
           <span className="text-[11px] font-semibold text-white bg-black/50 px-3 py-1 rounded-md backdrop-blur-md border border-white/10 uppercase tracking-widest">
             {leadBadgeLabel(story.importance)}
           </span>
         )}
      </div>
      
      {/* Título & Resumo */}
      <h2 className="text-[2rem] sm:text-4xl font-extrabold text-white leading-[1.1] tracking-tight drop-shadow-md">
         {storyHeadline}
      </h2>
      {storyDeck && (
         <p className="text-[1.05rem] font-medium text-gray-300 leading-relaxed mt-4 drop-shadow-md">
           {storyDeck}
         </p>
      )}
      
      {/* Chapters/Blocks if available */}
      {storyBlocks.length > 0 && (
        <div className="mt-4 pt-4 border-t border-white/10 space-y-3">
          {storyBlocks.map((block) => (
            <StoryChapter key={`${block.label}-${block.text}`} label={block.label} text={block.text} />
          ))}
        </div>
      )}
    </div>
  );
}

function StoryListItem({ story, active, onClick }) {
  const storyHeadline = story.headline ?? story.title ?? "Leitura atual";
  const storyDeck = story.deck ?? story.summary ?? "";
  const toneClass = storyToneBadgeClass(story.accent_tone);

  return (
    <div 
        onClick={onClick} 
        className={["group relative rounded-2xl border bg-white/[0.02] p-5 hover:bg-white/[0.04] transition-all cursor-pointer text-left block w-full", active ? "border-white/20 bg-white/[0.08]" : "border-white/5"].join(" ")}
    >
      {story.news_type && (
        <span className={["px-2 py-1 text-[10px] font-bold uppercase tracking-wider rounded w-fit mb-3 block border", toneClass].join(" ")}>
          {story.news_type}
        </span>
      )}
      <h4 className="text-lg font-bold text-white mb-1.5 leading-snug group-hover:text-accent-primary transition-colors line-clamp-2">
        {storyHeadline}
      </h4>
      <p className="text-sm font-medium text-text-secondary line-clamp-2">
        {storyDeck}
      </p>
    </div>
  );
}

function ReaderEmptyState({ loading }) {
  return (
    <div className="flex min-h-[320px] flex-col justify-end">
      <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">Leitura principal</p>
      <h3 className="mt-3 text-3xl font-semibold tracking-[-0.05em] text-text-primary">
        {loading ? "Preparando a leitura do recorte atual" : "Sem leitura disponível"}
      </h3>
      <p className="mt-3 max-w-2xl text-sm leading-7 text-text-secondary">
        {loading
          ? "A matéria principal e a lista navegável vão aparecer aqui assim que o briefing for montado."
          : "Troque o escopo ou ative outro filtro para abrir uma nova edição."}
      </p>
    </div>
  );
}


function StoryChapter({ label, text }) {
  return (
    <div className="border-t border-white/8 py-4 first:border-t-0 first:pt-0 last:pb-0">
      <p className="text-[10px] uppercase tracking-[0.22em] text-text-muted">{label}</p>
      <p className="mt-2 text-[0.95rem] leading-8 text-text-secondary">{text}</p>
    </div>
  );
}


export default NewsTab;
