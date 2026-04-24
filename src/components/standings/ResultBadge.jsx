function ResultBadge({ result }) {
  if (!result) {
    return (
      <span className="inline-flex h-7 w-10 items-center justify-center rounded-lg border border-dashed border-white/10 text-[11px] text-text-muted opacity-30">
        —
      </span>
    );
  }

  let content = null;

  if (result.is_dnf) {
    content = (
      <span className="inline-flex h-7 min-w-10 items-center justify-center rounded-lg border border-status-red/30 bg-status-red/15 px-1 text-[10px] font-bold text-status-red">
        DNF
      </span>
    );
  } else if (result.position === 1) {
    content = <Badge className="border-[#ffd700]/30 bg-[#ffd700]/15 text-[#ffd700]" label="P1" />;
  } else if (result.position === 2) {
    content = <Badge className="border-[#c0c0c0]/30 bg-[#c0c0c0]/15 text-[#c0c0c0]" label="P2" />;
  } else if (result.position === 3) {
    content = <Badge className="border-[#cd7f32]/30 bg-[#cd7f32]/15 text-[#cd7f32]" label="P3" />;
  } else if (result.position <= 10) {
    content = (
      <Badge className="border-white/10 bg-white/6 text-text-primary" label={`P${result.position}`} />
    );
  } else {
    content = (
      <span className="inline-flex h-7 w-10 items-center justify-center rounded-lg text-[11px] font-mono text-text-muted opacity-45">
        P{result.position}
      </span>
    );
  }

  if (!result.has_fastest_lap) {
    return content;
  }

  return (
    <span className="inline-flex items-center gap-1">
      {content}
      <span
        className="h-3 w-1.5 rounded-full border border-[#bc8cff]/35 bg-[#bc8cff] shadow-[0_0_12px_rgba(188,140,255,0.55)]"
        title="Volta mais rápida"
        aria-label="Volta mais rápida"
      />
    </span>
  );
}

function Badge({ className, label }) {
  return (
    <span
      className={[
        "inline-flex h-7 w-10 items-center justify-center rounded-lg border text-[11px] font-bold",
        className,
      ].join(" ")}
    >
      {label}
    </span>
  );
}

export default ResultBadge;
