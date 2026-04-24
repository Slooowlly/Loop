import { extractFlag, extractNationalityCode } from "../../utils/formatters";

const flagModules = import.meta.glob("../../assets/flag-icons/*.png", {
  eager: true,
  import: "default",
});

const FLAG_IMAGES = Object.fromEntries(
  Object.entries(flagModules).map(([path, url]) => {
    const filename = path.split("/").pop()?.replace(".png", "");
    return [filename, url];
  }),
);

function FlagIcon({ nacionalidade, className = "" }) {
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
      className={["h-4 w-6 rounded-[3px] object-cover shadow-[0_0_0_1px_rgba(255,255,255,0.08)]", className].join(" ")}
      loading="lazy"
    />
  );
}

export default FlagIcon;
