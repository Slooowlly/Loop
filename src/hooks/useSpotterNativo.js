import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// O interruptor do spotter NATIVO do iRacing (seção [SPCC] do app.ini).
//
// Não passa pelo `handleToggle` genérico do config porque a preferência e o `app.ini`
// precisam mudar juntos, e quem faz as duas coisas é um comando dedicado. Aqui em vez de
// inline na tela porque o tratamento de erro tem uma regra própria, e ela é fácil de
// perder de vista no meio do JSX: o interruptor NÃO volta quando a escrita falha. O
// backend grava a preferência ANTES de tocar no arquivo, então ela está salva mesmo com o
// sim aberto ou o app.ini ausente — voltar o interruptor faria a tela discordar do disco.
export default function useSpotterNativo() {
  const [status, setStatus] = useState(null);
  const [ocupado, setOcupado] = useState(false);

  // Leitura única na abertura, para a tela mostrar o que o `app.ini` diz hoje.
  useEffect(() => {
    let ativo = true;
    invoke("iracing_spotter_status")
      .then((s) => {
        if (ativo) setStatus(s);
      })
      .catch(() => {});
    return () => {
      ativo = false;
    };
  }, []);

  /// Liga/desliga. Devolve a mensagem de erro (string) quando falha, ou `null` no sucesso —
  /// quem desenha decide onde mostrar.
  const alternar = useCallback(async (ligado) => {
    setOcupado(true);
    try {
      setStatus(await invoke("iracing_spotter_set", { enabled: ligado }));
      return null;
    } catch (falha) {
      // O erro aparece e o boot seguinte tenta de novo; o estado da tela segue o disco.
      invoke("iracing_spotter_status").then(setStatus).catch(() => {});
      return String(falha);
    } finally {
      setOcupado(false);
    }
  }, []);

  // Sem `app.ini` não há o que alternar — a tela desabilita o controle por este sinal.
  const disponivel = Boolean(status?.app_ini_found);

  return { status, ocupado, disponivel, alternar };
}
