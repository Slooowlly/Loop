import { useTranslation } from "react-i18next";

// Rodapé "notícias do mundo": notinhas sobre ex-equipes e ex-companheiros do jogador.
function WorldNotes({ notes }) {
  const { t } = useTranslation();
  if (!notes.length) return null;
  return (
    <div className="world-notes" aria-label={t("newsMagazine.world.ariaLabel")}>
      <div className="wn-head">{t("newsMagazine.world.heading")}</div>
      <div className="wn-list">
        {/* O backend já emite estas notas no idioma ativo (rust-i18n), então o
            template determinístico aparece direto; a IA só reescreve por cima. */}
        {notes.map((n) => (
          <div key={n.id} className={`wn-item wn-${n.tone || "neutro"}`}>
            <span className="wn-tag">{n.tag}</span>
            <span className="wn-text">{n.text}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

export default WorldNotes;
