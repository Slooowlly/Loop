// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
import { CONFIDENCE } from "./constants";
import { breakdownSentence } from "./helpers";
import { FlowRow } from "./primitives";

// SALDO DE POSIÇÕES (2b-1) — Nível 1 sempre (tabela oficial); Nível 2
// enriquece com a telemetria. Tudo "estimado", nada de "ultrapassagem
// limpa". Aparece sempre que há resultado do jogador; nunca quebra.
function PositionBreakdownSection({ positionBreakdown, telemetry }) {
  if (!positionBreakdown) return null;

  return (
    <section className="mb-6 shrink-0 px-4">
      <div className="rounded-3xl border border-white/10 bg-gradient-to-br from-[#0a0f16]/90 to-[#080d14]/50 p-6 shadow-xl">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">
            Saldo de posições
          </p>
          <span
            className={`text-[9px] uppercase tracking-widest font-bold px-2 py-0.5 rounded border ${
              positionBreakdown.hasFlow && telemetry?.confidence && CONFIDENCE[telemetry.confidence]
                ? CONFIDENCE[telemetry.confidence].color
                : "text-gray-400 border-white/15 bg-white/5"
            }`}
          >
            {positionBreakdown.hasFlow
              ? `${CONFIDENCE[telemetry.confidence]?.label ?? "Estimado"} · telemetria`
              : "Nível 1 · tabela oficial"}
          </span>
        </div>

        <div className="mt-4 flex flex-col gap-4 lg:flex-row lg:items-stretch">
          {/* Largou → Chegou + saldo */}
          <div className="flex items-center gap-4 lg:w-2/5 shrink-0">
            <div className="text-center">
              <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">Largou</p>
              <p className="mt-1 font-mono text-2xl font-black text-gray-300">P{positionBreakdown.grid}</p>
            </div>
            <span className="text-2xl text-gray-600">→</span>
            <div className="text-center">
              <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">Chegou</p>
              <p className="mt-1 font-mono text-2xl font-black text-white">
                {positionBreakdown.isDnf ? "DNF" : `P${positionBreakdown.finish}`}
              </p>
            </div>
            {!positionBreakdown.isDnf && (
              <div className="ml-auto flex h-14 w-16 flex-col items-center justify-center rounded-xl border border-white/10 bg-white/5">
                <span
                  className={`text-xl font-black leading-none ${
                    positionBreakdown.net > 0
                      ? "text-green-400"
                      : positionBreakdown.net < 0
                        ? "text-red-400"
                        : "text-gray-400"
                  }`}
                >
                  {positionBreakdown.net > 0 ? `+${positionBreakdown.net}` : positionBreakdown.net}
                </span>
                <span className="text-[8px] uppercase tracking-widest text-gray-500">Saldo</span>
              </div>
            )}
          </div>

          {/* Decomposição estimada */}
          {!positionBreakdown.isDnf && (
            <div className="flex-1 space-y-1.5 lg:border-l lg:border-white/10 lg:pl-6">
              {positionBreakdown.onTrack > 0 && (
                <FlowRow
                  icon="▲"
                  label="Ganhas na pista (estimado)"
                  value={`+${positionBreakdown.onTrack}`}
                  color="text-green-400"
                />
              )}
              {positionBreakdown.inherited > 0 && (
                <FlowRow
                  icon="⬆"
                  label="Herdadas por DNF (provável)"
                  value={`+${positionBreakdown.inherited}`}
                  color="text-[#58a6ff]"
                />
              )}
              {positionBreakdown.lostOnTrack > 0 && (
                <FlowRow
                  icon="▼"
                  label={positionBreakdown.hasFlow ? "Perdidas na pista (estimado)" : "Perdidas"}
                  value={`-${positionBreakdown.lostOnTrack}`}
                  color="text-red-400"
                />
              )}
              {positionBreakdown.onTrack === 0 &&
                positionBreakdown.inherited === 0 &&
                positionBreakdown.lostOnTrack === 0 && (
                  <p className="text-[12px] text-gray-500">Você manteve a posição da largada.</p>
                )}
            </div>
          )}
        </div>

        <p className="mt-4 text-[13px] leading-relaxed text-gray-400">
          {breakdownSentence(positionBreakdown)}
        </p>
      </div>
    </section>
  );
}

export default PositionBreakdownSection;
