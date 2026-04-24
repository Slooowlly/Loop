import { forwardRef } from "react";

export const GlassCard = forwardRef(function GlassCard(
  {
    children,
    className = "",
    hover = true,
    selected = false,
    darkBg = false,
    onClick,
    as: Component = "div",
    ...props
  },
  ref,
) {
  const interactive = Boolean(onClick) || hover;

  return (
    <Component
      {...props}
      ref={ref}
      onClick={onClick}
      className={[
        "rounded-3xl border p-6 shadow-[0_18px_50px_rgba(0,0,0,0.24)]",
        darkBg
          ? "bg-app-card/70 backdrop-blur-[12px]"
          : "glass",
        "transition-glass",
        interactive ? "card-hover cursor-pointer" : "",
        selected
          ? `border-accent-primary glow-blue ${darkBg ? "bg-accent-primary/20" : "bg-accent-primary/10"}`
          : "border-white/10",
        className,
      ].join(" ")}
    >
      {children}
    </Component>
  );
});

export default GlassCard;
