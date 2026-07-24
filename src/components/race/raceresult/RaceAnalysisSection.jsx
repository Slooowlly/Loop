// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
import { formatLapTime } from "../../../utils/formatters";
import { CONFIDENCE } from "./constants";
import { bestMomentCard, coverageNote, fmtDeltaS, mistakeCard } from "./helpers";
import { AnalysisCard, MomentBanner, StatRow } from "./primitives";

// ANÁLISE DA CORRIDA (Fase 2) — só com telemetria (você correu a prova).
// Cada card respeita um critério mínimo: ritmo (>=2 voltas), consistência
// (>=3), vs grid (amostra do campo), rival (disputa real).
function RaceAnalysisSection({ telemetry }) {
  if (!telemetry?.has_telemetry) return null;

  return (
    <section className="mb-6 shrink-0 px-4">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <p className="text-[10px] uppercase tracking-[0.22em] text-gray-500 font-bold">
          Análise da corrida
        </p>
        <div className="flex items-center gap-3">
          <span className="text-[11px] text-gray-500">{coverageNote(telemetry)}</span>
          {telemetry.confidence && CONFIDENCE[telemetry.confidence] && (
            <span className={`text-[9px] uppercase tracking-widest font-bold px-2 py-0.5 rounded border ${CONFIDENCE[telemetry.confidence].color}`}>
              {CONFIDENCE[telemetry.confidence].label}
            </span>
          )}
        </div>
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        {telemetry.pace && (
          <AnalysisCard title="🏎️ Ritmo" accent="border-[#58a6ff]/20">
            <StatRow label="Melhor volta" value={formatLapTime(telemetry.pace.best_lap_ms)} color="text-purple-300" />
            <StatRow label="Ritmo limpo" value={formatLapTime(telemetry.pace.clean_avg_ms)} />
            {telemetry.pace.vs_grid_reliable && (
              <StatRow
                label="vs média do grid"
                value={fmtDeltaS(telemetry.pace.vs_grid_ms)}
                color={telemetry.pace.vs_grid_ms < 0 ? "text-green-400" : "text-amber-400"}
              />
            )}
          </AnalysisCard>
        )}
        {telemetry.pace?.consistency_reliable && (
          <AnalysisCard title="📊 Consistência" accent="border-green-500/20">
            <StatRow
              label="Voltas boas"
              value={`${telemetry.pace.good_laps}/${telemetry.pace.total_laps}`}
              color="text-green-400"
            />
            <StatRow label="Perdido por volta" value={fmtDeltaS(telemetry.pace.lost_per_lap_ms)} color="text-amber-400" />
            <StatRow label="Ritmo médio real" value={formatLapTime(telemetry.pace.real_avg_ms)} />
          </AnalysisCard>
        )}
        {telemetry.rival && (
          <AnalysisCard title="⚔️ Rival da corrida" accent="border-orange-500/20">
            <p className="text-sm font-bold text-white">{telemetry.rival.pilot_name}</p>
            <StatRow label="Voltas em disputa" value={`${telemetry.rival.laps_battled}`} />
            <StatRow label="Gap médio" value={`${telemetry.rival.avg_gap_s.toFixed(1)}s`} />
          </AnalysisCard>
        )}
      </div>

      {/* MELHOR MOMENTO (2b-3) + ERRO MAIS CARO (2b-2) — espelhos. Cada um só
          aparece se houve destaque/momento custoso real (confiança >= média
          no backend); corrida sem nada forte não mostra. */}
      {(telemetry.best_moment || telemetry.mistake) && (
        <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2">
          {telemetry.best_moment && (
            <MomentBanner
              label="Melhor momento"
              card={bestMomentCard(telemetry.best_moment)}
              confidence={telemetry.best_moment.confidence}
            />
          )}
          {telemetry.mistake && (
            <MomentBanner
              label="Erro mais caro"
              card={mistakeCard(telemetry.mistake)}
              confidence={telemetry.mistake.confidence}
            />
          )}
        </div>
      )}
    </section>
  );
}

export default RaceAnalysisSection;
