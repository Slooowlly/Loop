import { useLayoutEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

// Auto-ajusta o título da capa: teto de 57px (calibrado p/ todas as
// categorias reais), encolhendo só se algum nome futuro estourar a coluna.
const COVER_TITLE_BASE_PX = 57;

// Capa da revista ("livro fechado"): aparece quando não há edição aberta nem
// matéria de pré-temporada.
function MagazineCover({ catLabel, year }) {
  const { t } = useTranslation();
  const coverTitleRef = useRef(null);
  const coverBookRef = useRef(null);

  useLayoutEffect(() => {
    const el = coverTitleRef.current;
    if (!el) return;
    const fit = () => {
      // Enquanto o livro (imagem) não carrega, a coluna tem largura ~0 e a
      // medição sairia errada (título minúsculo). Deixa no 57px do CSS e
      // espera o ResizeObserver disparar quando a imagem definir a largura.
      if (el.clientWidth < 2) return;
      let size = COVER_TITLE_BASE_PX;
      el.style.fontSize = `${size}px`;
      while (el.scrollWidth > el.clientWidth + 1 && size > 12) {
        size -= 2;
        el.style.fontSize = `${size}px`;
      }
    };
    fit();
    // Re-mede quando a imagem do livro e a fonte web terminam de carregar
    // (é aí que a largura da coluna passa a existir de verdade).
    let ro;
    if (typeof ResizeObserver !== "undefined" && coverBookRef.current) {
      ro = new ResizeObserver(fit);
      ro.observe(coverBookRef.current);
    }
    if (document?.fonts?.ready) document.fonts.ready.then(fit).catch(() => {});
    window.addEventListener("resize", fit);
    return () => {
      window.removeEventListener("resize", fit);
      if (ro) ro.disconnect();
    };
  }, [catLabel]);

  return (
    <div className="mag-cover">
      <div className="mag-cover-frame">
        <img
          className="mag-cover-book"
          ref={coverBookRef}
          src="/utilities/news/magazine-cover.webp"
          alt=""
          draggable={false}
        />
        <span className="mag-cover-title" ref={coverTitleRef}>
          {catLabel.split(/\s+/).map((word, i) => (
            <span className="mag-cover-word" key={i}>
              {word}
            </span>
          ))}
        </span>
      </div>
      <div className="mag-cover-side">
        {year ? <p className="mag-cover-cap">{t("newsMagazine.cover.seasonYear", { year })}</p> : null}
        <p className="mag-cover-sub">{t("newsMagazine.cover.sub")}</p>
      </div>
    </div>
  );
}

export default MagazineCover;
