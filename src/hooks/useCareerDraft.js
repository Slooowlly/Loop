import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// A ponte do RASCUNHO de carreira (o mundo histórico simulado antes de o jogador entrar
// no grid). Cinco comandos, todos aqui: `get_career_draft`, `create_historical_career_draft`,
// `update_career_draft_identity`, `finalize_career_draft` e `discard_career_draft`.
//
// Ficam num hook por dois motivos. O primeiro é o padrão do projeto (invoke nos slices ou
// em hooks `use*`). O segundo é específico deste fluxo: um `create` custa 26 temporadas de
// simulação, e o wizard tem quatro caminhos que podem descartar o rascunho — trocar a
// dificuldade, resetar, finalizar, sair. Com as chamadas espalhadas pelo componente, qual
// delas engole o erro e qual o mostra virava decisão de cada call site.
//
// O estado do FORMULÁRIO continua no componente: o que mora aqui é o rascunho gravado no
// disco, que é o que o backend conhece.
export default function useCareerDraft() {
  const [draftState, setDraftState] = useState(null);

  /// Lê o rascunho em disco. Oportunista: sem rascunho (ou com erro de leitura) devolve
  /// `null` e a criação segue normalmente do zero.
  const carregar = useCallback(async () => {
    try {
      const state = await invoke("get_career_draft");
      return state?.exists ? state : null;
    } catch {
      return null;
    }
  }, []);

  /// Progresso da geração, enquanto o comando bloqueante roda. Best-effort por definição:
  /// é uma leitura paralela a uma escrita, e falhar nela só custa o ano na tela de espera.
  const lerProgresso = useCallback(async () => {
    try {
      const state = await invoke("get_career_draft");
      if (!state?.exists) return;
      setDraftState((atual) => ({
        ...(atual ?? state),
        progress_year: state.progress_year ?? atual?.progress_year ?? null,
        error: state.error ?? atual?.error ?? null,
        lifecycle_status: state.lifecycle_status ?? atual?.lifecycle_status,
      }));
    } catch {
      // Silencioso: a geração de verdade é quem reporta falha.
    }
  }, []);

  /// Gera o mundo histórico. É a chamada cara do wizard (26 temporadas) — o erro SEMPRE
  /// sobe, porque seguir sem mundo deixaria o jogador escolhendo equipe de uma lista vazia.
  const gerar = useCallback(async ({ playerName, nationality, age, difficulty }) => {
    const state = await invoke("create_historical_career_draft", {
      input: {
        player_name: playerName,
        player_nationality: nationality,
        player_age: Number(age),
        difficulty,
      },
    });
    setDraftState(state);
    return state;
  }, []);

  /// Grava no rascunho a identidade que está na tela. Substitui o descarte que existia
  /// antes: o jogador corrige o próprio nome e as temporadas já simuladas continuam de pé.
  ///
  /// `strict` decide o destino do erro, e a distinção é o motivo de esta função existir:
  /// na navegação entre passos, falhar é só perda de rascunho; na finalização, seguir
  /// adiante criaria a carreira com a identidade ANTIGA.
  const gravarIdentidade = useCallback(
    async ({ playerName, nationality, age }, { strict = false } = {}) => {
      const alvo = draftState;
      if (!alvo?.career_id || alvo.lifecycle_status !== "draft") return null;

      const nome = (playerName ?? "").trim();
      if (!nome) return null;

      const inalterado =
        nome === (alvo.player_name ?? "").trim() &&
        nationality === (alvo.player_nationality ?? "") &&
        Number(age) === Number(alvo.player_age);
      if (inalterado) return null;

      try {
        const state = await invoke("update_career_draft_identity", {
          input: {
            career_id: alvo.career_id,
            player_name: nome,
            player_nationality: nationality,
            player_age: Number(age),
          },
        });
        if (state?.exists) setDraftState(state);
        return state ?? null;
      } catch (falha) {
        if (strict) throw falha;
        return null;
      }
    },
    [draftState],
  );

  /// Fecha o rascunho e cria a carreira de verdade. O erro sobe: sem carreira não há
  /// para onde navegar.
  const finalizar = useCallback(
    async ({ category, teamId }) => {
      if (!draftState?.career_id) return null;
      return invoke("finalize_career_draft", {
        input: { career_id: draftState.career_id, category, team_id: teamId },
      });
    },
    [draftState],
  );

  /// Joga o rascunho fora. Nunca sobe erro: o descarte é limpeza, e o `create`/`finalize`
  /// seguinte é quem reporta uma sujeira persistente.
  const descartar = useCallback(async () => {
    // Zera ANTES de esperar o backend: quem descarta já mudou algo que invalida o mundo
    // (a dificuldade, por exemplo), e deixar o rascunho velho de pé durante a ida ao
    // backend faria a tela oferecer, por um instante, equipes de um mundo que morreu.
    setDraftState(null);
    try {
      await invoke("discard_career_draft");
    } catch {
      // Ver acima.
    }
  }, []);

  return {
    draftState,
    setDraftState,
    carregar,
    lerProgresso,
    gerar,
    gravarIdentidade,
    finalizar,
    descartar,
  };
}
