function SeasonSectionHeader({ title, color, detail, titleTestId }) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-3">
        <div
          className="h-px flex-1"
          style={{
            background: `linear-gradient(to right, transparent, ${color}88)`,
          }}
        />
        <p
          data-testid={titleTestId}
          className="min-w-[120px] text-center text-[18px] font-black uppercase tracking-[0.22em]"
          style={{ color }}
        >
          {title}
        </p>
        <div
          className="h-px flex-1"
          style={{
            background: `linear-gradient(to left, transparent, ${color}88)`,
          }}
        />
      </div>
      {detail ? (
        <p className="text-center text-[10px] font-semibold uppercase tracking-[0.14em] text-[color:var(--text-muted)]">
          {detail}
        </p>
      ) : null}
    </div>
  );
}

export default SeasonSectionHeader;
