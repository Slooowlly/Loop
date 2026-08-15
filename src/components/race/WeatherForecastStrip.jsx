import { useTranslation } from "react-i18next";

import {
  clampPct,
  condOf,
  mudancasDeCondicao,
  pontosOrdenados,
  useWeatherTimeline,
} from "./weatherTimelineData";

// Prévia compacta da previsão, para o lado direito do card de Condição de Pista.
//
// O card inteiro é um botão que abre o gráfico do clima, e até aqui metade dele
// era vazio: o jogador clicava num espaço em branco para descobrir o que havia
// atrás. A tira mostra um pedaço do que o clique abre — o mesmo gradiente, os
// mesmos ícones de mudança, a mesma escala de `weatherTimelineData` — e assim o
// clique passa a ser oferecido em vez de adivinhado.
//
// O que ela deixa de fora em relação ao gráfico grande: os rótulos de cada
// condição, as marcações de corrida e a legenda. Nessa largura eles viram ruído,
// e o detalhe é exatamente o que o modal entrega.
function WeatherForecastStrip({ careerId, raceId, mockData = null }) {
  const { t } = useTranslation();
  const { data, state } = useWeatherTimeline(careerId, raceId, mockData);

  // Erro não vira mensagem aqui dentro: o card tem dono (o clima) e a tira é um
  // acessório. Sem dado ela some, e o card volta a ser o que era antes.
  if (state === "error") return null;
  if (state === "loading" || !data) {
    return (
      <div
        data-testid="weather-forecast-strip-skeleton"
        className="hidden sm:block w-40 xl:w-48 shrink-0 h-[52px] animate-pulse rounded-xl bg-white/[0.04]"
      />
    );
  }

  const pts = pontosOrdenados(data);
  if (!pts.length) return null;

  const icons = mudancasDeCondicao(pts);
  const gid = `wstrip-${raceId}`;

  return (
    <div
      data-testid="weather-forecast-strip"
      className="hidden sm:block w-40 xl:w-48 shrink-0"
    >
      {/* Ícones das mudanças de condição, na posição em que elas acontecem. */}
      <div className="relative h-[18px]">
        {icons.map((p, i) => (
          <span
            key={i}
            className="absolute top-0 -translate-x-1/2 text-[13px] leading-none drop-shadow"
            style={{ left: `${clampPct(p.frac * 100)}%` }}
          >
            {condOf(p.event_type).icon}
          </span>
        ))}
      </div>

      {/* A faixa, no mesmo SVG do gráfico grande: o linear-gradient do CSS deixa
          artefato de canto num raio desse tamanho. */}
      <svg viewBox="0 0 1000 24" preserveAspectRatio="none" className="mt-1 block h-2.5 w-full">
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
          height="23"
          rx="8"
          fill={`url(#${gid})`}
          stroke="rgba(255,255,255,0.1)"
        />
      </svg>

      <div className="mt-1 flex items-center justify-between text-[9px] font-bold uppercase tracking-wider text-gray-500">
        <span>{t("weatherTimeline.start")}</span>
        <span>{t("weatherTimeline.finish")}</span>
      </div>
    </div>
  );
}

export default WeatherForecastStrip;
