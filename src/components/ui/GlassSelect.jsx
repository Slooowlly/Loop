function GlassSelect({ className = "", children, ...props }) {
  return (
    <select
      className={[
        // color-scheme:dark → o dropdown NATIVO (renderizado pelo WebView2) usa o
        // esquema escuro; sem isso, as opções saem com texto claro sobre o fundo
        // claro padrão do navegador (ilegíveis). As cores sólidas das <option> vêm
        // da regra global `select option` em index.css.
        "[color-scheme:dark]",
        "glass-light min-h-12 w-full rounded-2xl border border-white/10 px-4 py-3",
        "text-sm text-text-primary outline-none transition-glass",
        "focus:border-accent-primary",
        "focus:shadow-[0_0_0_1px_rgba(88,166,255,0.5),0_0_20px_rgba(88,166,255,0.12)]",
        className,
      ].join(" ")}
      {...props}
    >
      {children}
    </select>
  );
}

export default GlassSelect;
