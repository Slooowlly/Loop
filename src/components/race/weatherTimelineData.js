import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Fonte única da escala de tempo da linha do tempo do clima. Nasceu dentro do
// `WeatherTimelineChart` e saiu de lá quando a tira compacta do card de Condição
// de Pista passou a desenhar o MESMO gradiente: duas cópias da tabela seriam duas
// paletas divergindo em silêncio na primeira condição nova.
//
// Tipo de tempo (`event_type` do backend) → cor, ícone e chave de rótulo. Tons da
// esquerda (encoberto/parcial) propositalmente FRIOS (azulados) p/ não "vazar"
// amarelo no começo.
export const COND = {
  0: { c: "#f5b425", icon: "☀️", labelKey: "sol" },
  1: { c: "#fcd34d", icon: "🌤️", labelKey: "quaseLimpo" },
  2: { c: "#9fb1cb", icon: "⛅", labelKey: "parcial" },
  3: { c: "#74859e", icon: "☁️", labelKey: "encoberto" },
  6: { c: "#7dd3fc", icon: "🌦️", labelKey: "garoa" },
  7: { c: "#38bdf8", icon: "🌧️", labelKey: "chuva" },
  8: { c: "#0284c7", icon: "⛈️", labelKey: "chuvaForte" },
};

export const condOf = (et) => COND[et] ?? COND[0];

// Mantém o ícone dentro da faixa: a 0% e a 100% o `-translate-x-1/2` jogaria
// metade dele para fora do card.
export const clampPct = (v) => Math.max(3, Math.min(97, v));

/** Pontos ordenados por fração da corrida. Lista vazia quando não há dado. */
export function pontosOrdenados(data) {
  return (data?.points ?? []).slice().sort((a, b) => a.frac - b.frac);
}

/** Só as MUDANÇAS de condição (a primeira sempre entra) — evita repetir o mesmo tempo. */
export function mudancasDeCondicao(pts) {
  return pts.filter((p, i) => i === 0 || p.event_type !== pts[i - 1].event_type);
}

/**
 * Carrega a linha do tempo determinística do clima da corrida.
 *
 * @returns {{ data: object|null, state: "loading"|"ok"|"error" }}
 */
export function useWeatherTimeline(careerId, raceId, mockData = null) {
  const [data, setData] = useState(mockData);
  const [state, setState] = useState(mockData ? "ok" : "loading");

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
      return undefined;
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

  return { data, state };
}
