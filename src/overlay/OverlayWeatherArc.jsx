import { useTranslation } from "react-i18next";

// Faixa COMPACTA do arco de chuva na torre AO VIVO. O `get_overlay_data` traz o mesmo
// clima determinístico da corrida (`session.weather.rainArc` = [{frac, event_type}] em
// frações 0..1) + `nowFrac` (progresso da prova). Só aparece quando há chuva a antecipar —
// aí a torre deixa de mostrar só a condição atual e revela o que vem pela frente.
//
// event_type: 0 sol, 1 quase limpo, 2 parcial, 3 encoberto, 6 garoa, 7 chuva, 8 forte.

const COND_COLOR = {
  0: "#f5b425",
  1: "#fcd34d",
  2: "#9fb1cb",
  3: "#74859e",
  6: "#7dd3fc",
  7: "#38bdf8",
  8: "#0284c7",
};
const RAIN_ICON = { 6: "🌦️", 7: "🌧️", 8: "⛈️" };
const colorOf = (et) => COND_COLOR[et] ?? COND_COLOR[0];
const isRain = (et) => et >= 6;
const clampPct = (v) => Math.max(2, Math.min(98, v));

export default function OverlayWeatherArc({ weather, width }) {
  const { t } = useTranslation();
  const pts = (weather?.rainArc ?? []).slice().sort((a, b) => a.frac - b.frac);
  if (pts.length < 1) return null;

  const now = typeof weather?.nowFrac === "number" ? weather.nowFrac : null;

  // Condição vigente AGORA (última entrada até `now`) e próxima virada pra chuva à frente.
  const current = now == null ? null : [...pts].reverse().find((p) => p.frac <= now);
  const rainingNow = current ? isRain(current.event_type) : false;
  const nextRain = (now == null ? pts : pts.filter((p) => p.frac >= now)).find((p) =>
    isRain(p.event_type),
  );

  const label = rainingNow
    ? t("overlay.weatherArc.raining")
    : nextRain
      ? t("overlay.weatherArc.incoming")
      : t("overlay.weatherArc.clearing");
  const leadIcon = rainingNow
    ? RAIN_ICON[current.event_type] ?? "🌧️"
    : nextRain
      ? RAIN_ICON[nextRain.event_type] ?? "🌧️"
      : "🌤️";

  const gid = "overlay-weather-arc";

  return (
    <div
      style={{
        width,
        boxSizing: "border-box",
        padding: "5px 8px 6px",
        background: "rgba(13,17,23,0.82)",
        borderRadius: 8,
        border: "1px solid rgba(255,255,255,0.1)",
        font: "600 11px system-ui, sans-serif",
        color: "#c9d1d9",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 4 }}>
        <span aria-hidden style={{ fontSize: 13, lineHeight: 1 }}>
          {leadIcon}
        </span>
        <span style={{ letterSpacing: "0.04em" }}>{label}</span>
      </div>

      <div style={{ position: "relative", height: 8 }}>
        <svg
          viewBox="0 0 1000 8"
          preserveAspectRatio="none"
          style={{ display: "block", width: "100%", height: 8 }}
        >
          <defs>
            <linearGradient id={gid} x1="0" y1="0" x2="1" y2="0">
              {pts.map((p, i) => (
                <stop
                  key={i}
                  offset={`${Math.max(0, Math.min(100, p.frac * 100)).toFixed(2)}%`}
                  stopColor={colorOf(p.event_type)}
                />
              ))}
            </linearGradient>
          </defs>
          <rect x="0" y="0" width="1000" height="8" rx="4" fill={`url(#${gid})`} />
        </svg>

        {now != null && (
          <div
            title={t("overlay.weatherArc.now")}
            style={{
              position: "absolute",
              top: -2,
              left: `${clampPct(now * 100)}%`,
              transform: "translateX(-50%)",
              width: 2,
              height: 12,
              background: "#fff",
              borderRadius: 1,
              boxShadow: "0 0 2px rgba(0,0,0,0.8)",
            }}
          />
        )}
      </div>
    </div>
  );
}
