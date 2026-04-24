const tabs = [
  { id: "standings", label: "Piloto" },
  { id: "news", label: "Notícias" },
  { id: "my-team", label: "Minha Equipe" },
  { id: "calendar", label: "Calendário" },
];

function TabNavigation({ activeTab, onTabChange }) {
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
            {tab.label}
          </button>
        );
      })}
    </nav>
  );
}

export default TabNavigation;
