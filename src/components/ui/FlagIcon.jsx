import { extractFlag, extractNationalityCode } from "../../utils/formatters";

const flagModules = import.meta.glob("../../assets/utilities/flag-icons/*.png", {
  eager: true,
  import: "default",
});

const FLAG_IMAGES = Object.fromEntries(
  Object.entries(flagModules).map(([path, url]) => {
    const filename = path.split("/").pop()?.replace(".png", "");
    return [filename, url];
  }),
);

// As artes têm proporções DIFERENTES entre si — de 1:1 (Suíça) a 2:1 (Austrália,
// Canadá, Reino Unido, Hungria), passando por 3:2 e 4:3. Não é defeito dos
// arquivos: bandeira nacional tem proporção oficial, e elas não batem.
//
// Por isso `object-contain`, e não `object-cover`. Com `cover`, a caixa fixa
// cortava o que sobrava: a Suíça perdia um quarto da altura e a Austrália perdia
// as laterais, com o Union Jack decepado. A CAIXA não muda de tamanho com esta
// troca — só o desenho lá dentro deixa de ser recortado —, então nenhum layout
// que já existia se mexe.
const BASE_BOXED = "h-4 w-6 object-contain";

// `variant` decide quem manda no tamanho.
//
// "boxed" (padrão) é o que tabela e lista usam: caixa fixa de 16x24, o que varia
// é só a bandeira dentro dela. É o que mantém uma coluna de bandeiras alinhada.
//
// "natural" é para quem desenha a bandeira GRANDE — a ficha do piloto. Nesse
// tamanho a tarja transparente do `contain` fica visível e o anel de 1px passa a
// contornar a caixa em vez da bandeira. Aqui o chamador dá só a ALTURA e a
// largura sai da proporção real da arte.
function FlagIcon({ nacionalidade, className = "", variant = "boxed" }) {
  const code = extractNationalityCode(nacionalidade);
  const src = code ? FLAG_IMAGES[code] : null;
  const fallback = extractFlag(nacionalidade);

  if (!src) {
    return (
      <span className={["inline-flex items-center justify-center text-base", className].join(" ")}>
        {fallback}
      </span>
    );
  }

  return (
    <img
      src={src}
      alt={nacionalidade || "Bandeira"}
      className={[
        variant === "natural" ? "" : BASE_BOXED,
        "rounded-[3px] shadow-[0_0_0_1px_rgba(255,255,255,0.08)]",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
      loading="lazy"
    />
  );
}

export default FlagIcon;
