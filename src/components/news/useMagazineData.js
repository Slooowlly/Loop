import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Hooks de carga da revista: cada um cuida de um `invoke` e do seu estado. Todos
// seguem o mesmo contrato — em qualquer falha caem no vazio/placeholder, nunca
// quebram a página.

// ── Construtores reais ──
export function useTeamsStandings(careerId, category) {
  const [standings, setStandings] = useState([]);
  useEffect(() => {
    let mounted = true;
    if (!careerId || !category) {
      setStandings([]);
      return undefined;
    }
    invoke("get_teams_standings", { careerId, category })
      .then((rows) => {
        if (mounted) setStandings(Array.isArray(rows) ? rows : []);
      })
      .catch(() => {
        if (mounted) setStandings([]);
      });
    return () => {
      mounted = false;
    };
  }, [careerId, category]);
  return standings;
}

// ── Pilotos reais (para a tabela alternativa e para realçar nomes no boletim) ──
export function useDriverStandings(careerId, category) {
  const [driverStandings, setDriverStandings] = useState([]);
  useEffect(() => {
    let mounted = true;
    if (!careerId || !category) {
      setDriverStandings([]);
      return undefined;
    }
    invoke("get_drivers_by_category", { careerId, category })
      .then((rows) => {
        if (mounted) setDriverStandings(Array.isArray(rows) ? rows : []);
      })
      .catch(() => {
        if (mounted) setDriverStandings([]);
      });
    return () => {
      mounted = false;
    };
  }, [careerId, category]);
  return driverStandings;
}

// ── Calendário real (para montar as edições das corridas disputadas) ──
// Devolve `loaded` junto porque calendário vazio é ambíguo: pode ser "ainda não
// voltou" ou "temporada sem corrida disputada" — e os dois abrem spreads
// diferentes. Sem essa distinção a revista abre no spread errado por alguns
// frames ao entrar na aba.
export function useCategoryCalendar(careerId, category) {
  const [state, setState] = useState({ calendar: [], loaded: false });
  useEffect(() => {
    let mounted = true;
    if (!careerId || !category) {
      setState({ calendar: [], loaded: true });
      return undefined;
    }
    setState({ calendar: [], loaded: false });
    invoke("get_calendar_for_category", { careerId, category })
      .then((rows) => {
        if (mounted) setState({ calendar: Array.isArray(rows) ? rows : [], loaded: true });
      })
      .catch(() => {
        if (mounted) setState({ calendar: [], loaded: true });
      });
    return () => {
      mounted = false;
    };
  }, [careerId, category]);
  return state;
}

// ── Rodapé "notícias do mundo": notinhas sobre ex-equipes e ex-companheiros do
// jogador, só da categoria atual (crise, dívida, clima pesado, nova diretoria).
export function useWorldNotes(careerId) {
  const [worldNotes, setWorldNotes] = useState([]);
  useEffect(() => {
    let mounted = true;
    if (!careerId) {
      setWorldNotes([]);
      return undefined;
    }
    // Mostra o texto determinístico na hora; a IA (se disponível no servidor) troca
    // as notas depois, sem bloquear a abertura da revista. Em qualquer falha —
    // inclusive o endpoint /world-notes ainda não existir — mantém o template.
    invoke("get_world_footer", { careerId })
      .then((res) => {
        if (!mounted) return;
        setWorldNotes(Array.isArray(res?.notes) ? res.notes : []);
        invoke("enrich_world_footer_ai", { careerId })
          .then((ai) => {
            if (mounted && ai?.source === "ai" && Array.isArray(ai?.notes)) {
              setWorldNotes(ai.notes);
            }
          })
          .catch(() => {});
      })
      .catch(() => {
        if (mounted) setWorldNotes([]);
      });
    return () => {
      mounted = false;
    };
  }, [careerId]);
  return worldNotes;
}

// Boletim de IA da edição atual (corrida do jogador): resolve o news_id da rodada
// e pede o boletim (cacheado/prewarmed no fim da corrida). Sem boletim → placeholder.
export function useRaceBulletin(careerId, seasonId, rodada) {
  const [bulletin, setBulletin] = useState(null);
  useEffect(() => {
    let active = true;
    if (!careerId || rodada == null || !seasonId) {
      setBulletin(null);
      return undefined;
    }
    setBulletin({ loading: true });
    invoke("player_race_news_id", { careerId, seasonId, rodada })
      .then((newsId) => {
        if (!newsId) {
          if (active) setBulletin({ loading: false, story: null });
          return null;
        }
        return invoke("enrich_race_news_ai", {
          careerId,
          newsId,
          readingSeconds: null,
        }).then((res) => {
          if (active) {
            setBulletin({
              loading: false,
              story: res?.story ?? null,
              teams: res?.teams ?? null,
              status: res?.status ?? null,
            });
          }
        });
      })
      .catch(() => {
        if (active) setBulletin({ loading: false, story: null });
      });
    return () => {
      active = false;
    };
  }, [careerId, rodada, seasonId]);
  return bulletin;
}

// Pré-temporada: só há matéria de expectativas enquanto NENHUMA corrida foi disputada.
export function useSeasonPreview(careerId, category, seasonId, isPreseason) {
  const [preview, setPreview] = useState(null);
  useEffect(() => {
    let active = true;
    if (!careerId || !category || !seasonId || !isPreseason) {
      setPreview(null);
      return undefined;
    }
    setPreview({ loading: true });
    invoke("enrich_season_preview_ai", { careerId })
      .then((res) => {
        if (!active) return;
        setPreview({
          loading: false,
          headline: res?.headline ?? null,
          standfirst: res?.standfirst ?? null,
          body: res?.body ?? null,
          teams: res?.teams ?? null,
          source: res?.source ?? null,
        });
      })
      .catch(() => {
        if (active) setPreview({ loading: false, body: null });
      });
    return () => {
      active = false;
    };
  }, [careerId, category, seasonId, isPreseason]);
  return preview;
}
