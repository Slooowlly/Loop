import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import useCareerStore from "../stores/useCareerStore";

// O config.json do app, com carga, gravação e as duas formas de mexer nele.
//
// Estava inline na tela de Configurações, junto de mais dez assuntos. Aqui fica sozinho e
// testável: `saveConfig` manda o objeto INTEIRO ao backend (é assim que o merge do lado
// Rust espera receber — ver `commands::config::merge_config_do_front`), e a tela otimista
// só recua para o disco quando a gravação falha.
export function useConfiguracaoDoApp() {
  const [config, setConfig] = useState(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");

  const loadConfig = useCallback(async () => {
    try {
      const cfg = await invoke("get_config");
      setConfig(cfg);
    } catch (err) {
      console.error("Falha ao carregar config:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  const saveConfig = useCallback(
    async (newCfg) => {
      setSaving(true);
      setErrorMessage("");
      try {
        await invoke("update_config", { newConfig: newCfg });
      } catch (err) {
        console.error("Falha ao salvar config:", err);
        setErrorMessage(err.toString());
        // A tela mostrou o valor novo antes de saber se gravou; se não gravou, ela volta
        // ao que está no disco em vez de mentir para o jogador.
        loadConfig();
      } finally {
        setSaving(false);
      }
    },
    [loadConfig],
  );

  const handleToggle = useCallback(
    (field) => {
      setConfig((atual) => {
        if (!atual) return atual;
        const newCfg = { ...atual, [field]: !atual[field] };
        saveConfig(newCfg);
        return newCfg;
      });
    },
    [saveConfig],
  );

  const handleChange = useCallback(
    (field, value) => {
      setConfig((atual) => {
        if (!atual) return atual;
        const newCfg = { ...atual, [field]: value };
        saveConfig(newCfg);
        return newCfg;
      });
      // Reflete a troca de idioma no store na hora (decide fallback PT vs "erro"
      // localizado nas telas de narrativa), sem exigir reload da carreira.
      if (field === "language") {
        useCareerStore.getState().setLanguage(value);
      }
    },
    [saveConfig],
  );

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  return {
    config,
    setConfig,
    loading,
    saving,
    errorMessage,
    setErrorMessage,
    loadConfig,
    saveConfig,
    handleToggle,
    handleChange,
  };
}

export default useConfiguracaoDoApp;
