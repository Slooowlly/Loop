import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

import Tooltip from "../ui/Tooltip";

// Tipo de tempo (event_type do backend) → cor, ícone e rótulo. Tons da esquerda
// (encoberto/parcial) propositalmente FRIOS (azulados) p/ não "vazar" amarelo no começo.
const COND = {
  0: { c: "#f5b425", icon: "☀️", labelKey: "sol" },
  1: { c: "#fcd34d", icon: "🌤️", labelKey: "quaseLimpo" },
  2: { c: "#9fb1cb", icon: "⛅", labelKey: "parcial" },
  3: { c: "#74859e", icon: "☁️", labelKey: "encoberto" },
  6: { c: "#7dd3fc", icon: "🌦️", labelKey: "garoa" },
  7: { c: "#38bdf8", icon: "🌧️", labelKey: "chuva" },
  8: { c: "#0284c7", icon: "⛈️", labelKey: "chuvaForte" },
};
const condOf = (et) => COND[et] ?? COND[0];
const clampPct = (v) => Math.max(3, Math.min(97, v));

/**
 * Faixa do clima da corrida (Largada → Bandeira): um gradiente que transita pelas
 * condições + ícones de tempo nas mudanças. Reconstrói o clima determinístico via
 * `get_race_weather_timeline`.
 *
 * @param careerId  id da carreira
 * @param raceId    id da corrida (calendar.id)
 * @param markers   [{frac, icon, label, isPlayer}] opcional (pós-corrida)
 * @param forecast  true = previsão (sala de estratégia); false = revisão (pós-corrida)
 */
export default function WeatherTimelineChart({ careerId, raceId, markers = [], forecast = false, mockData = null }) {
  const { t } = useTranslation();
  const [data, setData] = useState(mockData);
  const [state, setState] = useState(mockData ? "ok" : "loading"); // loading | ok | error

  useEffect(() => {
    let alive = true;
    // Dev: dados fake injetados → pula o backend.
    if (mockData) {
      setData(mockData);
      setState("ok");
      return undefined;
    }
    if (!careerId || !raceId) {
      setState("error");
      return;
    }
    setState("loading");
    invoke("get_race_weather_timeline", { careerId, raceId })
      .then((res) => {
        if (!alive) return;
        setData(res);
        setState("ok");
      })
      .catch(() => alive && setState("error"));
    return () => {
      alive = false;
    };
  }, [careerId, raceId, mockData]);

  if (state === "error") {
    return (
      <div className="text-sm text-gray-500 py-8 text-center">
        {t("weatherTimeline.error")}
      </div>
    );
  }
  if (state === "loading" || !data) {
    return <div className="rounded-2xl border border-white/10 bg-white/[0.02] h-[140px] animate-pulse" />;
  }

  const pts = (data.points ?? []).slice().sort((a, b) => a.frac - b.frac);
  if (!pts.length) return null;

  // Gradiente da faixa via SVG (preciso, sem o artefato de canto do linear-gradient CSS).
  const gid = `wbar-${raceId}`;

  // Ícones só nas MUDANÇAS de condição (1º sempre) — evita repetir o mesmo tempo.
  const icons = pts.filter((p, i) => i === 0 || p.event_type !== pts[i - 1].event_type);

  return (
    <div>
      <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span className="text-sm font-bold uppercase tracking-[0.16em] text-gray-200">
            {forecast ? t("weatherTimeline.forecastTitle") : t("weatherTimeline.raceTitle")}
          </span>
          <span
            className={`text-[10px] uppercase tracking-widest font-bold px-2 py-0.5 rounded border ${
              data.is_wet_race
                ? "border-sky-500/40 text-sky-300 bg-sky-500/10"
                : "border-amber-500/30 text-amber-300 bg-amber-500/10"
            }`}
          >
            {data.intensity}
          </span>
        </div>
        <span className="text-[13px] font-semibold text-gray-300">{data.scenario}</span>
      </div>

      {/* Ícones de tempo (acima da faixa) */}
      <div className="relative h-12">
        {icons.map((p, i) => {
          const cond = condOf(p.event_type);
          return (
            <div
              key={i}
              className="absolute top-0 -translate-x-1/2 flex flex-col items-center"
              style={{ left: `${clampPct(p.frac * 100)}%` }}
            >
              <span className="text-2xl leading-none drop-shadow">{cond.icon}</span>
              <span className="mt-1 text-[10px] font-semibold text-gray-400 whitespace-nowrap">
                {t(`weatherTimeline.conditions.${cond.labelKey}`)}
              </span>
            </div>
          );
        })}
      </div>

      {/* A faixa do clima (SVG: gradiente preciso, sem artefato de canto) */}
      <svg viewBox="0 0 1000 48" preserveAspectRatio="none" className="block w-full h-12">
        <defs>
          <linearGradient id={gid} x1="0" y1="0" x2="1" y2="0">
            {pts.map((p, i) => (
              <stop
                key={i}
                offset={`${Math.max(0, Math.min(100, p.frac * 100)).toFixed(2)}%`}
                stopColor={condOf(p.event_type).c}
              />
            ))}
          </linearGradient>
        </defs>
        <rect
          x="0.5"
          y="0.5"
          width="999"
          height="47"
          rx="10"
          fill={`url(#${gid})`}
          stroke="rgba(255,255,255,0.1)"
        />
      </svg>

      {/* Marcações (pós-corrida) + extremos */}
      <div className="relative mt-1 h-6">
        {markers.map((m, i) => (
          <Tooltip key={i} texto={m.label || ""}>
            <div
              className="absolute top-0 -translate-x-1/2 flex flex-col items-center"
              style={{ left: `${clampPct(m.frac * 100)}%` }}
            >
              <span className={`text-[11px] leading-none ${m.isPlayer ? "" : "opacity-60"}`}>▾</span>
              {typeof m.icon === "string" && m.icon.startsWith("/") ? (
                <img src={m.icon} alt="" className="h-4 w-4 object-contain" />
              ) : (
                <span className="text-[12px] leading-none">{m.icon}</span>
              )}
            </div>
          </Tooltip>
        ))}
      </div>
      <div className="flex items-center justify-between text-[12px] font-bold text-gray-400">
        <span>{t("weatherTimeline.start")}</span>
        <span>{t("weatherTimeline.finish")}</span>
      </div>

      <p className="mt-2 text-[10px] text-gray-600">
        {forecast
          ? t("weatherTimeline.captionForecast")
          : t("weatherTimeline.captionReview")}
        {markers.some((m) => m.isPlayer) ? t("weatherTimeline.markersNote") : ""}
      </p>
    </div>
  );
}
