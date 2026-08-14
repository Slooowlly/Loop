import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { cacheEhDaEtapaAtual } from "../../../stores/career/helpers";
import { bestEffort } from "../../../utils/bestEffort";
import { AI_PREVIEW_MAX_WAIT_MS, PRE_RACE_READ_MS } from "./nextRaceHelpers";

// Prévia pré-corrida por IA + detecção de leitura da Sala de Estratégia.
export function usePreRaceAi({ careerId, nextRace, preRaceAi, briefing, isLoadingBriefing }) {
  // Prévia pré-corrida por IA (narrativa + voz da equipe, curtas). null → template.
  const [aiBriefing, setAiBriefing] = useState(null);
  // A prévia de IA está sendo buscada agora? Enquanto true, exibimos um skeleton no
  // lugar do template (evita o flash template→IA quando a IA chega logo em seguida).
  const [aiPending, setAiPending] = useState(false);
  // Reroll de debug da prévia por IA (força regenerar, ignora cache + cooldown).
  const [aiReroll, setAiReroll] = useState({ busy: false, status: null });
  // Debug: ver o template original mesmo quando há prévia de IA em cache. A IA fica
  // guardada em `aiBriefing`, então alternar de volta não regenera nada.
  const [showTemplate, setShowTemplate] = useState(false);

  // Prévia pré-corrida por IA: ao abrir a Sala de Estratégia, manda os fatos do
  // briefing ao servidor e troca a narrativa + voz da equipe pela versão da IA.
  // Cacheada por etapa no backend; em cooldown/erro mantém o template (aiBriefing
  // fica null). Mostra o template imediatamente e troca quando a IA chega.
  useEffect(() => {
    let active = true;
    const raceId = nextRace?.id;
    const facts = briefing.aiFacts;
    setAiBriefing(null);
    setAiPending(false);
    // Prefetch durante a animação de avanço já gerou esta etapa → usa direto (sem
    // novo fetch e sem flash; o render lê de `preRaceAi`). A carreira entra na conferência
    // porque a numeração das etapas recomeça em R001 a cada save.
    if (cacheEhDaEtapaAtual(preRaceAi, { careerId, raceId })) {
      return undefined;
    }
    // Só dispara com o contexto do briefing já carregado (standings/forma), senão
    // poderíamos cachear uma prévia com fatos incompletos.
    if (!careerId || !raceId || !facts || isLoadingBriefing) {
      return undefined;
    }
    // Busca em voo → skeleton no lugar do template até a IA chegar (ou o teto estourar).
    setAiPending(true);
    const maxWait = window.setTimeout(() => {
      if (active) setAiPending(false);
    }, AI_PREVIEW_MAX_WAIT_MS);
    // Best-effort de verdade: cooldown e erro do servidor de IA são o caso ESPERADO, e o
    // template já é a resposta pronta para eles (`aiBriefing` fica null e o render cai
    // nele sozinho). O que faltava era o rastro — sem ele, "a prévia nunca vem" e "a
    // prévia está em cooldown" chegam iguais ao suporte.
    bestEffort(invoke("pre_race_briefing_ai", { careerId, raceId, facts }), "pre_race_briefing_ai")
      .then((res) => {
        if (active && res?.narrative && res?.team_voice) {
          setAiBriefing({
            headline: res.headline ?? null,
            narrative: res.narrative,
            teamVoice: res.team_voice,
          });
        }
      })
      .finally(() => {
        if (active) setAiPending(false);
      });
    return () => {
      active = false;
      window.clearTimeout(maxWait);
    };
  }, [
    careerId,
    nextRace?.id,
    briefing.aiFacts,
    isLoadingBriefing,
    preRaceAi?.raceId,
    preRaceAi?.careerId,
  ]);

  // --- Detecção de leitura da prévia (alimenta o gate de engajamento da IA) ---
  // Cronometra o tempo na Sala de Estratégia por etapa. "Leu" = ficou ≥ PRE_RACE_READ_MS
  // (exportar não para o cronômetro; simular/sair antes conta como não-leu). Reporta no
  // simular ou ao trocar de corrida / sair da tela (cleanup). Guard evita report duplo.
  const viewStartRef = useRef(0);
  const readReportedRef = useRef(false);

  const reportPreRaceEngagement = useCallback(() => {
    if (readReportedRef.current) return;
    readReportedRef.current = true;
    if (!careerId) return;
    const read = Date.now() - viewStartRef.current >= PRE_RACE_READ_MS;
    // Telemetria de engajamento: nada na tela depende dela, e perder um report só
    // desloca de leve o gate da IA. Best-effort legítimo, com rastro para quando o gate
    // parecer travado sem motivo.
    bestEffort(
      invoke("report_pre_race_engagement", { careerId, read }),
      "report_pre_race_engagement",
    );
  }, [careerId]);

  useEffect(() => {
    if (!nextRace?.id) return undefined;
    viewStartRef.current = Date.now();
    readReportedRef.current = false;
    return () => {
      reportPreRaceEngagement();
    };
  }, [nextRace?.id, reportPreRaceEngagement]);

  // Reroll de debug: força o servidor a regenerar a prévia (ignora cache e cooldown)
  // e troca a narrativa + voz da equipe na hora. Útil para afinar fatos/prompt.
  async function handleRerollAi() {
    const raceId = nextRace?.id;
    const facts = briefing.aiFacts;
    if (!careerId || !raceId || !facts || aiReroll.busy) {
      return;
    }
    setAiReroll({ busy: true, status: null });
    try {
      const res = await invoke("pre_race_briefing_ai", { careerId, raceId, facts, force: true });
      if (res?.narrative && res?.team_voice) {
        setAiBriefing({
          headline: res.headline ?? null,
          narrative: res.narrative,
          teamVoice: res.team_voice,
        });
        setShowTemplate(false); // mostra o resultado novo da IA
        setAiReroll({ busy: false, status: res.status ?? "ok" });
      } else {
        setAiReroll({ busy: false, status: res?.status ?? "error" });
      }
    } catch (e) {
      setAiReroll({ busy: false, status: "error" });
    }
  }

  return {
    aiBriefing,
    aiPending,
    aiReroll,
    showTemplate,
    setShowTemplate,
    handleRerollAi,
    reportPreRaceEngagement,
  };
}
