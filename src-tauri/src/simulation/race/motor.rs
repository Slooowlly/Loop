//! O laço da corrida: percorre os cinco segmentos aplicando dano latente, incidentes, quebra de
//! peça e pontuação, e no fim monta o [`RaceResult`].

use rand::Rng;

use crate::constants::scoring::RACE_SCORE_TO_LAP_MS;

use crate::simulation::catalog::IncidentCatalog;
use crate::simulation::context::{SimDriver, SimulationContext};
use crate::simulation::incidents::process_segment_incidents_cfg;
use crate::simulation::qualifying::QualifyingResult;

use super::danos::process_pending_damage;
use super::pontuacao::{apply_physical_degradation, apply_tire_degradation, calculate_segment_score};
use super::resultados::{build_race_results, derive_caution_segments};
use super::tipos::{MechanicalOutcome, RaceResult, RaceSegment, RaceState};

/// Converte segundos parados no box em pontos de `cumulative_score`, a moeda da simulação.
/// É o INVERSO exato de [`RACE_SCORE_TO_LAP_MS`]: descontar isto do score faz o
/// `total_race_time_ms` do piloto sair de `build_race_results` `secs` mais lento. Sem esse
/// casamento o reparo custaria um número inventado de posições.
///
/// A equivalência é medida CONTRA O LÍDER (`build_race_results` ancora o tempo no vencedor).
/// Quando quem quebra é o próprio líder, a âncora se move junto e o custo aparente encolhe
/// pela margem que ele tinha — o que é o comportamento certo: ele só perde o que a margem
/// não cobria.
fn repair_secs_to_score(secs: u32, total_laps: i32) -> f64 {
    (secs as f64 * 1000.0) / (RACE_SCORE_TO_LAP_MS * total_laps.max(1) as f64)
}

/// Corrida simulada SEM quebra de peça — o caminho de sempre. Ver
/// [`simulate_race_with_breakdowns`] para o caminho com a Fase 7 ligada.
pub fn simulate_race(
    drivers: &[SimDriver],
    qualifying: &[QualifyingResult],
    ctx: &SimulationContext,
    catalog: &IncidentCatalog,
    is_endurance: bool,
    rng: &mut impl Rng,
) -> RaceResult {
    simulate_race_with_breakdowns(drivers, qualifying, ctx, catalog, is_endurance, None, rng)
}

/// Corrida simulada cobrando também os desfechos de QUEBRA DE PEÇA pré-rolados (Fase 7).
///
/// `mechanicals` é a chave de FONTE ÚNICA da pane mecânica:
/// - `Some(..)` — o Sistema de Quebra está no comando. Os desfechos vêm do cérebro
///   `car::breakdown`, rolados sobre o desgaste REAL do carro de cada time (por isso o time
///   pobre, que estica peça por falta de caixa, é o que quebra), e a pane genérica do catálogo
///   de incidentes é DESLIGADA. Lista vazia é válido: significa "a quebra rodou e ninguém
///   quebrou", não "sistema desligado".
/// - `None` — a quebra não roda nesta corrida (rascunho histórico, grid sintético sem carro no
///   banco). A pane do catálogo continua sendo a fonte de falha mecânica.
#[allow(clippy::too_many_arguments)]
pub fn simulate_race_with_breakdowns(
    drivers: &[SimDriver],
    qualifying: &[QualifyingResult],
    ctx: &SimulationContext,
    catalog: &IncidentCatalog,
    is_endurance: bool,
    mechanicals: Option<&[MechanicalOutcome]>,
    rng: &mut impl Rng,
) -> RaceResult {
    let catalog_mechanical = mechanicals.is_none();
    let mechanicals = mechanicals.unwrap_or(&[]);
    let total_drivers = qualifying.len() as i32;
    let mut applied_mechanicals: Vec<usize> = Vec::new();
    let mut states: Vec<RaceState> = qualifying
        .iter()
        .map(|result| RaceState {
            driver_id: result.pilot_id.clone(),
            tire_wear: 1.0,
            physical_condition: 1.0,
            cumulative_score: (total_drivers - result.position + 1) as f64 * 2.0,
            is_dnf: false,
            current_position: result.position,
            incidents: Vec::new(),
            dnf_reason: None,
            dnf_segment: None,
            pending_damage: Vec::new(),
        })
        .collect();

    for segment in [
        RaceSegment::Start,
        RaceSegment::Early,
        RaceSegment::Mid,
        RaceSegment::Late,
        RaceSegment::Finish,
    ] {
        if ctx.incidents_enabled {
            // Processar danos latentes ANTES dos rolls normais do segmento
            process_pending_damage(
                &mut states,
                segment,
                drivers,
                catalog,
                ctx.vehicle_class,
                is_endurance,
                rng,
            );

            let result = process_segment_incidents_cfg(
                drivers,
                &states,
                segment,
                ctx.weather,
                ctx.is_championship_deciding,
                ctx.incident_rate_multiplier,
                ctx.start_chaos_multiplier,
                ctx.pack_density_factor,
                catalog,
                ctx.vehicle_class,
                is_endurance,
                catalog_mechanical,
                rng,
            );

            for incident in result.incidents {
                if let Some(state) = states.iter_mut().find(|s| s.driver_id == incident.pilot_id) {
                    if incident.is_dnf {
                        state.is_dnf = true;
                        state.dnf_reason = Some(incident.description.clone());
                        state.dnf_segment = Some(segment);
                    }
                    state.incidents.push(incident);
                }
            }

            // Aplicar novos danos latentes gerados neste segmento
            for (driver_id, pd) in result.new_pending_damage {
                if let Some(state) = states.iter_mut().find(|s| s.driver_id == driver_id) {
                    state.pending_damage.push(pd);
                }
            }
        }

        // QUEBRA DE PEÇA (Fase 7): o desfecho pré-rolado cobra o preço nesta altura da corrida.
        // Roda FORA do `incidents_enabled` de propósito — quebra não é incidente de pilotagem;
        // é consequência do desgaste que o time carregou pra cá, e vale mesmo com incidentes
        // desligados. DNF encerra o carro; leve/grave desconta os segundos do box na moeda da
        // sim, então a perda de posição sai da física do resultado, não de um chute.
        for (idx, m) in mechanicals.iter().enumerate() {
            if RaceSegment::from_lap(m.lap, ctx.total_laps) != segment {
                continue;
            }
            let Some(state) = states.iter_mut().find(|s| s.driver_id == m.pilot_id) else {
                continue;
            };
            // Carro já fora (batida antes): a peça largaria num carro que não estava mais lá.
            if state.is_dnf {
                continue;
            }
            if m.is_dnf {
                state.is_dnf = true;
                state.dnf_reason = Some(m.label.clone());
                state.dnf_segment = Some(segment);
            } else {
                state.cumulative_score = (state.cumulative_score
                    - repair_secs_to_score(m.penalty_secs, ctx.total_laps))
                .max(0.0);
            }
            applied_mechanicals.push(idx);
        }

        let seg_str = segment.as_str();
        for state in &mut states {
            if state.is_dnf {
                continue;
            }

            if let Some(driver) = drivers.iter().find(|driver| driver.id == state.driver_id) {
                let mut segment_score = calculate_segment_score(driver, state, segment, ctx, rng);
                let penalty: f64 = state
                    .incidents
                    .iter()
                    .filter(|incident| incident.segment == seg_str && !incident.is_dnf)
                    .map(|incident| incident.positions_lost as f64 * 2.0)
                    .sum();
                segment_score = (segment_score - penalty).max(0.0);
                state.cumulative_score += segment_score;
                apply_tire_degradation(state, driver, ctx);
                apply_physical_degradation(state, driver, ctx);
            }
        }

        states.sort_by(|a, b| match (a.is_dnf, b.is_dnf) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            _ => b
                .cumulative_score
                .partial_cmp(&a.cumulative_score)
                .unwrap_or(std::cmp::Ordering::Equal),
        });

        for (index, state) in states.iter_mut().enumerate() {
            state.current_position = index as i32 + 1;
        }
    }

    let mut race_results = build_race_results(drivers, qualifying, ctx, &states, rng);
    let pole_sitter_id = qualifying
        .first()
        .map(|result| result.pilot_id.clone())
        .unwrap_or_default();
    let winner_id = race_results
        .first()
        .map(|result| result.pilot_id.clone())
        .unwrap_or_default();
    let total_incidents: i32 = race_results.iter().map(|r| r.incidents_count).sum();
    let total_dnfs = race_results.iter().filter(|r| r.is_dnf).count() as i32;

    // Aggregate narrative fields
    let main_incident_count: i32 = race_results
        .iter()
        .flat_map(|r| &r.incidents)
        .filter(|i| i.narrative_importance_hint >= 1)
        .count() as i32;

    let notable_incident_pilot_ids: Vec<String> = race_results
        .iter()
        .filter(|r| r.notable_incident.is_some())
        .map(|r| r.pilot_id.clone())
        .collect();

    let most_positions_gained_id = race_results
        .iter()
        .filter(|r| !r.is_dnf && r.positions_gained > 0)
        .max_by_key(|r| r.positions_gained)
        .map(|r| r.pilot_id.clone());

    let caution_segments = derive_caution_segments(race_results.iter().flat_map(|r| &r.incidents));

    RaceResult {
        qualifying_results: qualifying.to_vec(),
        race_results: std::mem::take(&mut race_results),
        pole_sitter_id,
        winner_id,
        fastest_lap_id: String::new(),
        total_laps: ctx.total_laps,
        weather: ctx.weather.as_str().to_string(),
        track_name: ctx.track_name.clone(),
        total_incidents,
        total_dnfs,
        main_incident_count,
        notable_incident_pilot_ids,
        most_positions_gained_id,
        caution_segments,
        applied_mechanicals,
    }
}
