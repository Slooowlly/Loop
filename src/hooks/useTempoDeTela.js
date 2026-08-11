/**
 * Cronômetro de permanência nas telas que CUSTAM geração de IA.
 *
 * Três telas, e só três: notícias, briefing e debriefing. São as que consomem o
 * servidor de IA, e é por isso que o tempo nelas paga a conta de saber se o que foi
 * gerado chegou a ser lido. Calendário, tabela e ficha de piloto não custam nada, e
 * medir permanência nelas seria vigiar por vigiar.
 *
 * Reporta em dois momentos:
 *
 * 1. **Ao sair da tela** (unmount), com o tempo desde a última marca.
 * 2. **A cada minuto enquanto a tela está aberta.** Sem isto, quem lê o debriefing e
 *    fecha o Loop sem sair da tela levaria a leitura inteira embora — e é exatamente
 *    essa a leitura mais longa do jogo.
 *
 * O backend acumula em disco e manda um evento por rodada. Nada aqui sabe se a
 * telemetria está ligada: quem decide isso é o `telemetry.rs`, que descarta em
 * silêncio quando o jogador não consentiu.
 */
import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

/** De quanto em quanto tempo o parcial é descarregado enquanto a tela segue aberta. */
const INTERVALO_MS = 60_000;

/**
 * Abaixo disso não se reporta. Uma tela que abre e fecha no mesmo instante (troca de
 * aba com o dedo pesado, remontagem por `key`) não é leitura, e somar esses segundos
 * inflaria o número justamente onde ele precisa ser confiável.
 */
const MINIMO_SEGUNDOS = 2;

export default function useTempoDeTela(tela) {
  // Em ref, e não em estado: mudar isto não redesenha nada, e um `setState` por
  // minuto numa tela de leitura seria um redesenho por nada.
  const desde = useRef(null);

  useEffect(() => {
    if (!tela) return undefined;
    desde.current = Date.now();

    // Fecha o pedaço aberto e reancora o relógio. Devolve o que foi reportado, o que
    // torna o comportamento testável sem esperar um minuto de verdade.
    const descarregar = () => {
      if (desde.current == null) return 0;
      const segundos = Math.floor((Date.now() - desde.current) / 1000);
      desde.current = Date.now();
      if (segundos < MINIMO_SEGUNDOS) return 0;
      // Erro aqui morre em silêncio: telemetria não pode aparecer na cara do jogador.
      // O `then` defensivo existe porque este hook é montado dentro de telas que os
      // testes renderizam com o `invoke` trocado por um stub síncrono — e uma tela do
      // jogo não pode quebrar porque a telemetria esperava uma promessa.
      try {
        const envio = invoke("telemetria_tela", { tela, segundos });
        if (envio && typeof envio.catch === "function") envio.catch(() => {});
      } catch {
        // ignorado de propósito
      }
      return segundos;
    };

    const timer = setInterval(descarregar, INTERVALO_MS);
    return () => {
      clearInterval(timer);
      descarregar();
      desde.current = null;
    };
  }, [tela]);
}
