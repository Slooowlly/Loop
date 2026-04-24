function GlassSelect({ className = "", children, ...props }) {
  return (
    <select
      className={[
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
