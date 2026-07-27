import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

// Aviso complementar para telas que ficaram SEM dados de telemetria.
//
// Uma tela zerada tem duas leituras opostas — "a corrida foi assim mesmo" e "o
// Loop nunca conseguiu ler o simulador" — e o jogador não tem como distinguir.
// Este aviso só aparece na segunda: quando o sampler não observou NENHUMA
// amostra desde que o app abriu. Aí a tela deixa de mentir por omissão e aponta
// para o diagnóstico.
function IracingSemDadosAviso() {
  const { t } = useTranslation();
  const [semAmostras, setSemAmostras] = useState(false);

  useEffect(() => {
    let vivo = true;
    invoke("iracing_diagnostico")
      .then((d) => {
        // Zero amostras desde o boot = o sim nunca foi lido. Com amostras, o
        // vazio é dado legítimo (corrida offline, importada) e calamos.
        if (vivo) setSemAmostras(d?.ticks_observados === 0);
      })
      .catch(() => {});
    return () => {
      vivo = false;
    };
  }, []);

  if (!semAmostras) return null;

  return (
    <p className="mt-3 text-[11px] leading-snug text-status-yellow/90">
      {t("iracingSemDados.aviso")}
    </p>
  );
}

export default IracingSemDadosAviso;
