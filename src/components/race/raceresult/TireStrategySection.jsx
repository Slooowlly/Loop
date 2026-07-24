// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
import { AnalysisCard, StintTrail } from "./primitives";

// ESTRATÉGIA DE PNEUS — inferida das paradas de box + clima (seco/chuva).
// Aparece mesmo se você saiu cedo: a IA corre até o fim. Só com paradas.
function TireStrategySection({ telemetry }) {
  if (!(telemetry?.tire_strategies?.length > 0)) return null;

  const all = telemetry.tire_strategies;
  const pt = telemetry.player_tire;
  const playerIdx = pt?.car_idx ?? -999;
  const others = all.filter((s) => s.car_idx !== playerIdx);
  const wetRace = all.some(
    (s) => s.start_compound === "Wet" || s.stints?.some((st) => st.compound === "Wet"),
  );
  const gambled = others.filter((s) => s.start_compound === "Wet");
  const wrong = others.filter((s) => s.wrong_tire);
  const changesGrid = others.reduce((n, s) => n + (s.tire_changes || 0), 0);

  return (
    <section className="mb-6 shrink-0 px-4">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <p className="text-[10px] uppercase tracking-[0.22em] text-gray-500 font-bold">
          Estratégia de pneus
        </p>
        <span className="text-[9px] uppercase tracking-widest font-bold px-2 py-0.5 rounded border border-gray-500/30 text-gray-400">
          {wetRace ? "Corrida de chuva" : "Corrida seca"} · estimado
        </span>
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        {/* Seus pneus */}
        <AnalysisCard title="🛞 Seus pneus" accent="border-[#58a6ff]/20">
          {pt ? (
            <>
              <p className="text-sm font-bold text-white">{pt.summary}</p>
              <StintTrail stints={pt.stints} />
              {pt.wrong_tire && (
                <p className="mt-2 text-[11px] text-amber-400">
                  ⚠️ Você terminou no pneu errado para a condição da pista.
                </p>
              )}
            </>
          ) : (
            <p className="text-sm text-gray-400">Sem dados da sua estratégia de pneu.</p>
          )}
        </AnalysisCard>

        {/* O grid */}
        <AnalysisCard title="🏁 O grid" accent="border-orange-500/20">
          {gambled.length > 0 && (
            <div className="mb-2">
              <p className="text-[11px] font-bold text-sky-300">
                Apostaram na chuva (largaram de wet):
              </p>
              <p className="text-[12px] text-gray-300">
                {gambled.map((s) => s.pilot_name).join(", ")}
              </p>
            </div>
          )}
          {wrong.length > 0 && (
            <div className="mb-2">
              <p className="text-[11px] font-bold text-amber-400">
                Ficaram no pneu errado:
              </p>
              <p className="text-[12px] text-gray-300">
                {wrong.map((s) => s.pilot_name).join(", ")}
              </p>
            </div>
          )}
          {gambled.length === 0 && wrong.length === 0 && (
            <p className="text-[12px] text-gray-400">
              {changesGrid > 0
                ? `${changesGrid} troca${changesGrid > 1 ? "s" : ""} de pneu no grid.`
                : "Grid largou de seco, sem trocas relevantes."}
            </p>
          )}
        </AnalysisCard>
      </div>
      <p className="mt-2 text-[10px] text-gray-600">
        Estimado pelas paradas de box + clima — o iRacing não informa o pneu da IA
        diretamente.
      </p>
    </section>
  );
}

export default TireStrategySection;
