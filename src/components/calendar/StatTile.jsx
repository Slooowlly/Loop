const STAT_TONE = {
  blue: "#58a6ff",
  amber: "#e3b341",
  coral: "#f0785a",
  teal: "#3fc2a8",
  cyan: "#4bc4e0",
  purple: "#a371f7",
};

const STAT_ICON_PATHS = {
  calendar: (
    <>
      <rect x="3.5" y="4.5" width="17" height="16" rx="2.5" />
      <path d="M3.5 9.5h17M8 3v3M16 3v3" />
    </>
  ),
  trophy: (
    <>
      <path d="M8 21h8M12 17v4" />
      <path d="M7 4h10v5a5 5 0 0 1-10 0V4Z" />
      <path d="M7 5H4v2a3 3 0 0 0 3 3M17 5h3v2a3 3 0 0 1-3 3" />
    </>
  ),
  pin: (
    <>
      <path d="M12 21s6-5.4 6-10a6 6 0 1 0-12 0c0 4.6 6 10 6 10Z" />
      <circle cx="12" cy="11" r="2.3" />
    </>
  ),
  globe: (
    <>
      <circle cx="12" cy="12" r="8.2" />
      <path d="M3.8 12h16.4M12 3.8c2.2 2.4 3.4 5.2 3.4 8.2s-1.2 5.8-3.4 8.2c-2.2-2.4-3.4-5.2-3.4-8.2S9.8 6.2 12 3.8Z" />
    </>
  ),
  flag: (
    <>
      <path d="M5.5 21V4" />
      <path d="M5.5 4.5h11l-1.6 3.4 1.6 3.4h-11" />
    </>
  ),
  clock: (
    <>
      <circle cx="12" cy="12" r="8.2" />
      <path d="M12 7.5V12l3 2" />
    </>
  ),
  hourglass: (
    <>
      <path d="M6.5 3h11M6.5 21h11" />
      <path d="M7 3c0 4 4 5.4 4 9s-4 5-4 9M17 3c0 4-4 5.4-4 9s4 5 4 9" />
    </>
  ),
  rain: (
    <>
      <path d="M7 15.5a4.5 4.5 0 0 1 .5-8.98A5 5 0 0 1 17 7.5a3.5 3.5 0 0 1 .5 8" />
      <path d="M8 18l-1 2M12 18l-1 2M16 18l-1 2" />
    </>
  ),
  star: (
    <>
      <path d="M12 3.5l2.6 5.3 5.9.86-4.25 4.14 1 5.88L12 17l-5.25 2.76 1-5.88L3.5 9.66l5.9-.86Z" />
    </>
  ),
};

function StatTile({ icon, tone = "blue", value, label }) {
  const color = STAT_TONE[tone] ?? STAT_TONE.blue;
  return (
    <div className="flex items-center gap-2.5 rounded-2xl bg-white/[0.05] p-3.5">
      <span
        className="grid h-[34px] w-[34px] shrink-0 place-items-center rounded-xl"
        style={{ backgroundColor: `${color}1f`, color }}
      >
        <svg
          viewBox="0 0 24 24"
          width="17"
          height="17"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.7"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          {STAT_ICON_PATHS[icon]}
        </svg>
      </span>
      <div>
        <div className="kcal text-xl font-bold italic leading-none tabular-nums text-text-primary">{value}</div>
        <div className="mt-0.5 text-[10px] uppercase tracking-[0.06em] text-text-muted">{label}</div>
      </div>
    </div>
  );
}

export default StatTile;
