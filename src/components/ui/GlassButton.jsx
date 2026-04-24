const variants = {
  primary:
    "bg-accent-primary text-slate-950 hover:bg-accent-hover focus-visible:ring-accent-primary",
  secondary:
    "glass-light text-text-primary hover:bg-app-card-hover focus-visible:ring-white/20",
  success:
    "bg-status-green text-slate-950 hover:brightness-110 focus-visible:ring-status-green",
  danger:
    "bg-status-red text-white hover:brightness-110 focus-visible:ring-status-red",
};

function GlassButton({
  children,
  className = "",
  variant = "primary",
  disabled = false,
  type = "button",
  ...props
}) {
  return (
    <button
      type={type}
      disabled={disabled}
      className={[
        "inline-flex min-h-11 items-center justify-center rounded-2xl px-5 py-3",
        "text-sm font-semibold tracking-[0.08em] transition-glass",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-0",
        disabled ? "cursor-not-allowed opacity-50" : "",
        variants[variant] ?? variants.primary,
        className,
      ].join(" ")}
      {...props}
    >
      {children}
    </button>
  );
}

export default GlassButton;
