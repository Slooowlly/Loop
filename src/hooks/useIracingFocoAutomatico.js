import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

// Gatilho inverso do iRacing: quando o simulador acaba de fechar, o Loop se traz à frente.
//
// A checagem é atômica no backend e barata, então roda mais rápido que o poller de
// importação de resultado. Fica num hook (e não solto num `useEffect` do Dashboard) para o
// intervalo e o nome do comando morarem no mesmo lugar — a tela só declara "enquanto esta
// carreira estiver aberta".
const INTERVALO_MS = 1500;

export default function useIracingFocoAutomatico(ativo) {
  useEffect(() => {
    if (!ativo) return undefined;
    const handle = setInterval(() => {
      // Silencioso de propósito: fora do Windows (e sem o iRacing aberto) o comando
      // simplesmente não tem o que fazer, e um erro por segundo e meio afogaria o console.
      invoke("iracing_focus_self_if_closed").catch(() => {});
    }, INTERVALO_MS);
    return () => clearInterval(handle);
  }, [ativo]);
}
