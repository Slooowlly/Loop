import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

// Chave de preferência do layout da pré-temporada. Mora no localStorage e não no
// save: é escolha de quem está olhando a tela, não estado da carreira — trocar de
// save não deve devolver o jogador ao layout que ele já descartou.
const STORAGE_KEY = "loop.preseason.layout";

function readStoredLayout() {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    return stored === "v1" || stored === "v2" ? stored : "v2";
  } catch {
    return "v2";
  }
}

export function usePreSeasonLayout() {
  const [layout, setLayout] = useState(readStoredLayout);

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, layout);
    } catch {
      // Sem localStorage (janela isolada, modo privado): a escolha vale só na sessão.
    }
  }, [layout]);

  const toggle = useCallback(() => {
    setLayout((current) => (current === "v2" ? "v1" : "v2"));
  }, []);

  return { layout, setLayout, toggle };
}

// Botão discreto no canto: o v1 continua a um clique de distância enquanto o v2
// não estiver aprovado.
export default function LayoutSwitch({ layout, onToggle }) {
  const { t } = useTranslation();
  const isV2 = layout === "v2";
  return (
    <button
      type="button"
      onClick={onToggle}
      data-testid="preseason-layout-switch"
      title={isV2 ? t("preSeason.v2.layout.backToOld") : t("preSeason.v2.layout.tryNew")}
      className="transition-glass glass-light fixed right-3 top-2 z-40 flex items-center gap-1.5 rounded-lg px-2.5 py-1 text-[9px] font-black uppercase tracking-[0.16em] text-[color:var(--text-secondary)] hover:text-[color:var(--text-primary)]"
    >
      <span
        className="h-1.5 w-1.5 rounded-full"
        style={{ background: isV2 ? "var(--accent-primary)" : "var(--text-muted)" }}
      />
      {isV2 ? t("preSeason.v2.layout.badgeNew") : t("preSeason.v2.layout.badgeOld")}
    </button>
  );
}
