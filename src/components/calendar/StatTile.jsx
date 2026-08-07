import { CalendarDays, Clock, CloudRain, Flag, Globe, Hourglass, MapPin, Star, Trophy } from "lucide-react";

const STAT_TONE = {
  blue: "#58a6ff",
  amber: "#e3b341",
  coral: "#f0785a",
  teal: "#3fc2a8",
  cyan: "#4bc4e0",
  purple: "#a371f7",
};

const STAT_ICONS = {
  calendar: CalendarDays,
  trophy: Trophy,
  pin: MapPin,
  globe: Globe,
  flag: Flag,
  clock: Clock,
  hourglass: Hourglass,
  rain: CloudRain,
  star: Star,
};

function StatTile({ icon, tone = "blue", value, label }) {
  const color = STAT_TONE[tone] ?? STAT_TONE.blue;
  const Icon = STAT_ICONS[icon];
  return (
    <div className="flex items-center gap-2.5 rounded-2xl bg-white/[0.05] p-3.5">
      <span
        className="grid h-[34px] w-[34px] shrink-0 place-items-center rounded-xl"
        style={{ backgroundColor: `${color}1f`, color }}
      >
        {Icon ? <Icon size={17} strokeWidth={1.7} aria-hidden="true" /> : null}
      </span>
      <div>
        <div className="kcal text-xl font-bold italic leading-none tabular-nums text-text-primary">{value}</div>
        <div className="mt-0.5 text-[10px] uppercase tracking-[0.06em] text-text-muted">{label}</div>
      </div>
    </div>
  );
}

export default StatTile;
