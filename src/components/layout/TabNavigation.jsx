import { useTranslation } from "react-i18next";

// labelKey resolvido no render (i18n) — não congela no idioma do boot.
// A ordem é a do zoom: o mundo (Home), o que o mundo diz (Notícias), eu
// (Carreira), a minha equipe, o que vem (Calendário). "Carreira" entrou no meio,
// e não no fim, porque ela é sobre o protagonista: encostada no Calendário ela se
// leria como mais uma consulta de agenda.
const tabs = [
  { id: "standings", labelKey: "nav.tab.standings" },
  { id: "news", labelKey: "nav.tab.news" },
  { id: "carreira", labelKey: "nav.tab.carreira" },
  { id: "my-team", labelKey: "nav.tab.myTeam" },
  { id: "calendar", labelKey: "nav.tab.calendar" },
];

function TabNavigation({ activeTab, onTabChange }) {
  const { t } = useTranslation();
  return (
    <nav className="inline-flex items-center gap-1 rounded-full bg-white/5 backdrop-blur-md border border-white/10 px-1">
      {tabs.map((tab) => {
        const isActive = activeTab === tab.id;
        return (
          <button
            key={tab.id}
            type="button"
            onClick={() => onTabChange?.(tab.id)}
            className={[
              "px-5 py-[11px] text-sm font-semibold tracking-[0.06em] rounded-full transition-glass",
              isActive
                ? "bg-accent-primary/20 text-accent-primary shadow-[inset_0_1px_0_rgba(255,255,255,0.1)]"
                : "text-text-secondary hover:text-text-primary hover:bg-white/5",
            ].join(" ")}
          >
            {t(tab.labelKey)}
          </button>
        );
      })}
    </nav>
  );
}

export default TabNavigation;
