// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
import WeatherTimelineChart from "../WeatherTimelineChart";

// Aba "Clima": linha do tempo do clima com marcadores das paradas de box.
function WeatherPanel({ careerId, raceId, result, telemetry }) {
  const totalLaps = telemetry?.race_laps || result?.total_laps || 0;
  const markers = [];
  if (telemetry?.tire_strategies?.length && totalLaps > 0) {
    const pIdx = telemetry.player_tire?.car_idx ?? -999;
    (telemetry.player_tire?.stops || []).forEach((d) => {
      markers.push({
        frac: d.lap / totalLaps,
        icon: d.tire_change ? "🔧" : "⛽",
        label: `Pit V${d.lap}`,
        isPlayer: true,
      });
    });
    telemetry.tire_strategies
      .filter((s) => s.car_idx !== pIdx)
      .forEach((s) =>
        (s.stops || [])
          .filter((d) => d.tire_change)
          .forEach((d) =>
            markers.push({ frac: d.lap / totalLaps, icon: "🔧", isPlayer: false }),
          ),
      );
  }

  return (
    <div className="animate-fade-in pr-2">
        <WeatherTimelineChart careerId={careerId} raceId={raceId} markers={markers} />
    </div>
  );
}

export default WeatherPanel;
