// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
import TeamLogoMark from "../../team/TeamLogoMark";
import { TIRE_EST_SECS } from "./constants";

// Tabela DEDICADA de tempos de pit (tracking próprio, ênfase na EQUIPE). Uma linha
// por piloto = a parada mais rápida dele (menor tempo na caixa). A divisão
// pneu/combustível é estimada (troca ≈ 20–22 s; resto = abastecimento).
function PitTimesTable({ telemetry, teamColors }) {
  const strategies = telemetry?.tire_strategies;
  if (!strategies?.length) return null;

  const playerIdx = telemetry.player_tire?.car_idx ?? -999;

  // Melhor (menor) parada de cada carro. Equipe vem resolvida do backend (car_idx).
  const best = {};
  strategies.forEach((s) => {
    (s.stops || []).forEach((d) => {
      const cur = best[s.car_idx];
      if (!cur || d.box_secs < cur.box_secs) {
        best[s.car_idx] = {
          ...d,
          pilot_name: s.pilot_name,
          team: s.team_name || "—",
          car_idx: s.car_idx,
          isPlayer: s.car_idx === playerIdx,
        };
      }
    });
  });
  const rows = Object.values(best).sort((a, b) => a.box_secs - b.box_secs);
  if (!rows.length) return null;

  return (
    <section className="px-4 mt-4 mb-8">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="text-sm font-bold uppercase tracking-[0.18em] text-gray-200">Tempos de pit</h3>
        <span className="text-[11px] text-gray-500">
          Tempo na caixa (estafe dos boxes) · pneu/combustível estimado
        </span>
      </div>
      <div className="rounded-2xl border border-white/10 bg-white/[0.02] overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-[#0d1420] text-gray-500 text-[11px] uppercase tracking-wider">
            <tr>
              <th className="px-3 py-3 text-center w-10">#</th>
              <th className="px-3 py-3 text-left">Equipe</th>
              <th className="px-3 py-3 text-left">Piloto</th>
              <th className="px-3 py-3 text-right">Tempo de pit</th>
              <th className="px-3 py-3 text-center">Volta</th>
              <th className="px-3 py-3 text-center">🛞 Pneus</th>
              <th className="px-3 py-3 text-center">⛽ Comb.</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => {
              const fuel = r.tire_change ? Math.max(0, r.box_secs - TIRE_EST_SECS) : r.box_secs;
              const tire = r.tire_change ? r.box_secs - fuel : 0;
              return (
                <tr
                  key={r.car_idx}
                  className={`border-t border-white/5 ${r.isPlayer ? "bg-[#58a6ff]/10" : "hover:bg-white/[0.03]"}`}
                >
                  <td className="px-3 py-3 text-center font-black text-gray-500">{i + 1}</td>
                  <td className="px-3 py-3">
                    <div className="flex items-center gap-2.5">
                      <TeamLogoMark teamName={r.team} color={teamColors[r.team] ?? null} size="sm" />
                      <span
                        className={`font-bold uppercase tracking-wide text-[13px] truncate max-w-[190px] ${r.isPlayer ? "text-[#58a6ff]" : "text-gray-100"}`}
                      >
                        {r.team}
                      </span>
                    </div>
                  </td>
                  <td className={`px-3 py-3 ${r.isPlayer ? "font-bold text-[#58a6ff]" : "text-gray-300"}`}>
                    <span className="flex items-center gap-1.5">
                      {r.pilot_name}
                      {r.track_wet && <span title="Pista molhada">🌧️</span>}
                    </span>
                  </td>
                  <td className="px-3 py-3 text-right font-mono font-bold text-white">
                    {r.box_secs.toFixed(2)}
                  </td>
                  <td className="px-3 py-3 text-center text-gray-400">{r.lap}</td>
                  <td className="px-3 py-3 text-center text-gray-300">
                    {r.tire_change ? `~${Math.round(tire)}s` : "—"}
                  </td>
                  <td className="px-3 py-3 text-center text-gray-300">
                    {fuel >= 2 ? `~${Math.round(fuel)}s` : "—"}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <p className="mt-2 text-[10px] text-gray-600">
        Tempo parado na caixa por parada (o menor de cada piloto) — reflete o estafe dos boxes da
        equipe. A divisão pneu/combustível é estimada: a troca de pneus leva ~20–22 s, o resto é
        abastecimento.
      </p>
    </section>
  );
}

export default PitTimesTable;
