import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// O que o jogador já viveu com cada piloto que ficou sem vaga: confronto direto e
// papel de rival. Dado local desta tela — não vive no store, some quando o modal
// fecha.
//
// Best-effort de propósito: se a consulta falhar, o modal continua listando os
// pilotos sem os marcadores. Perder o histórico não pode impedir o jogador de
// iniciar a temporada.
export default function useDisplacedContext(careerId, driverIds) {
  const [byDriver, setByDriver] = useState({});
  // A lista de ids chega como array novo a cada render; a chave em string é o que
  // impede o efeito de rodar em loop.
  const key = driverIds.join(",");

  useEffect(() => {
    if (!careerId || !key) {
      setByDriver({});
      return undefined;
    }
    let ativo = true;
    invoke("get_displaced_driver_context", { careerId, driverIds: key.split(",") })
      .then((rows) => {
        if (!ativo) return;
        setByDriver(Object.fromEntries((rows ?? []).map((row) => [row.driver_id, row])));
      })
      .catch(() => {
        if (ativo) setByDriver({});
      });
    return () => {
      ativo = false;
    };
  }, [careerId, key]);

  return byDriver;
}
