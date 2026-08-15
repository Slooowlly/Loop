import { useTranslation } from "react-i18next";

// Rodapé "notícias do mundo": notinhas sobre ex-equipes e ex-companheiros do jogador.
// A diagramação é de coluna de breves impressa — fio no lugar de caixa, numeração
// contínua e tarja de editoria. As notas descem por coluna (01-03 à esquerda,
// 04-06 à direita), então a divisão é feita aqui e não pelo fluxo do grid: é o que
// permite a primeira nota de cada página abrir sem fio em cima.
function WorldNotes({ notes }) {
  const { t } = useTranslation();
  if (!notes.length) return null;

  const meio = Math.ceil(notes.length / 2);
  const colunas = [notes.slice(0, meio), notes.slice(meio)].filter(
    (c) => c.length,
  );
  // Nada atravessa a dobra: o cabeçalho é feito de duas peças, uma por folha,
  // cada uma com o seu fio. Um fio único cruzando o vão solda as duas páginas e
  // desmancha a ilusão de papel. Com uma nota só a folha direita não recebe
  // peça nenhuma — fio vazio ali promete conteúdo que não existe.
  const solo = notes.length === 1;

  return (
    <div
      className={`world-notes${solo ? " world-notes--solo" : ""}`}
      aria-label={t("newsMagazine.world.ariaLabel")}
    >
      <div className="wn-head">
        <div className="wn-head-pg">
          <span className="wn-head-title">
            {t("newsMagazine.world.heading")}
          </span>
          <span className="wn-head-sub">
            {t("newsMagazine.world.subheading")}
          </span>
          <span className="wn-head-rule" aria-hidden="true" />
          {solo ? (
            <span className="wn-head-count">
              {t("newsMagazine.world.count", { count: notes.length })}
            </span>
          ) : null}
        </div>
        {solo ? null : (
          <div className="wn-head-pg wn-head-pg--dir">
            <span className="wn-head-rule" aria-hidden="true" />
            <span className="wn-head-count">
              {t("newsMagazine.world.count", { count: notes.length })}
            </span>
          </div>
        )}
      </div>
      <div className="wn-list">
        {/* O backend já emite estas notas no idioma ativo (rust-i18n), então o
            template determinístico aparece direto; a IA só reescreve por cima. */}
        {colunas.map((coluna, ci) => (
          <div className="wn-col" key={ci}>
            {coluna.map((n, i) => (
              <div key={n.id} className={`wn-item wn-${n.tone || "neutro"}`}>
                <span className="wn-num" aria-hidden="true">
                  {String(ci * meio + i + 1).padStart(2, "0")}
                </span>
                <span className="wn-body">
                  <span className="wn-kick">
                    <span className="wn-tag">{n.tag}</span>
                    <span className="wn-fio" aria-hidden="true" />
                  </span>
                  <span className="wn-text">{n.text}</span>
                </span>
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

export default WorldNotes;
