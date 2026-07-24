// i18n-ignore-file — tela V1 APOSENTADA (RaceResultViewV2 é a oficial); código morto,
// não vale traduzir. Se um dia voltar a ser usada, remova esta linha e traduza.
import { ASSESSMENT } from "./constants";
import { ExpStat } from "./primitives";

// LEITURA DE CARREIRA (Fase 1) — só aparece com avaliação; nunca quebra sem.
function CareerReadSection({ evaluation, playerResult }) {
  if (!evaluation) return null;

  return (
    <section className="mb-6 shrink-0 px-4">
      <div className="rounded-3xl border border-white/10 bg-gradient-to-br from-[#0a0f16]/90 to-[#080d14]/50 p-6 shadow-xl">
        <div className="flex flex-col gap-5 lg:flex-row">
          {/* Avaliação + frase */}
          <div className="flex items-start gap-4 lg:w-2/5 shrink-0">
            <span className="text-3xl mt-0.5">{ASSESSMENT[evaluation.assessment]?.emoji}</span>
            <div>
              <p className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">Avaliação da corrida</p>
              <p className={`text-xl font-extrabold ${ASSESSMENT[evaluation.assessment]?.color}`}>
                {ASSESSMENT[evaluation.assessment]?.label}
              </p>
            </div>
          </div>
          <p className="flex-1 text-sm leading-relaxed text-gray-200 lg:border-l lg:border-white/10 lg:pl-6">
            {evaluation.headline}
          </p>
        </div>

        {/* Meta da corrida vs Resultado (o potencial fica oculto — é interno). */}
        <div className="mt-5 grid grid-cols-1 gap-3 border-t border-white/10 pt-5 sm:grid-cols-2">
          <ExpStat label="Meta da corrida" value={`P${evaluation.target_low}–P${evaluation.target_high}`} />
          <ExpStat
            label="Resultado"
            value={playerResult ? (playerResult.is_dnf ? "DNF" : `P${playerResult.finish_position}`) : "—"}
            highlight
          />
        </div>

        {/* Leitura da equipe */}
        <p className="mt-4 text-[13px] leading-relaxed text-gray-400">
          <span className="text-[10px] uppercase tracking-widest font-bold text-gray-500">Leitura da equipe: </span>
          {evaluation.team_read}
        </p>
      </div>
    </section>
  );
}

export default CareerReadSection;
