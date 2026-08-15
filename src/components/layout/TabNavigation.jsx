import { useTranslation } from "react-i18next";

// labelKey resolvido no render (i18n) — não congela no idioma do boot.
// A ordem é a do zoom: o mundo (Home), o que o mundo diz (Notícias), a minha
// equipe, o que vem (Calendário).
//
// "Carreira" saiu daqui em 14/08/2026, e a `pages/tabs/carreira/` foi apagada
// junto. Ela nasceu para dar um LUGAR ao assunto "eu", e as cinco seções dela
// (piloto, história, troféus, rivais, mercado) liam o mesmo `get_driver_detail`
// que a ficha do piloto já lê. A ficha cresceu e passou a responder tudo:
// Habilidade é o dossiê, Histórico cobre também os primeiros marcos, o auge, a
// confiabilidade e os eventos especiais que a sala de troféus listava, e Rivais e
// Mercado são as mesmas seções. Duas portas para a mesma resposta é o custo que a
// aba cobrava.
//
// A porta que fica é a de sempre: clicar no próprio nome na Home abre a ficha.
//
// O único conteúdo da aba que a ficha não tinha era o F-01: as vagas abertas do
// mundo com o veredito de elegibilidade e o "quem está de olho em você". Os dois
// mudaram de casa no mesmo dia, para `components/driver/v2/MercadoDoJogador.jsx`,
// no fim da aba Mercado da ficha. O resto do código da aba está no commit 4892aa8,
// em `src/pages/tabs/carreira/`.
const tabs = [
  { id: "standings", labelKey: "nav.tab.standings" },
  { id: "news", labelKey: "nav.tab.news" },
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
