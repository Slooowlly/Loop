//! Fechamento da corrida: ordenar quem terminou e quem abandonou, publicar o relógio
//! ancorado no vencedor, estimar as voltas do abandono e derivar em que segmentos houve
//! bandeira amarela.

use std::collections::HashMap;

use rand::Rng;

use crate::simulation::context::{SimDriver, SimulationContext};
use crate::simulation::incidents::IncidentResult;
use crate::simulation::qualifying::QualifyingResult;

use super::tipos::{ClassificationStatus, RaceDriverResult, RaceSegment, RaceState};

/// Segmentos em que a corrida foi neutralizada por bandeira amarela, derivados dos
/// incidentes que realmente aconteceram. Um carro batido e parado traz a amarela; um
/// susto leve ou uma pane em que o carro recolhe sozinho, não. Vários incidentes no
/// mesmo segmento contam como UMA neutralização — seria a mesma bandeira.
///
/// Determinístico e sem efeito no resultado. Fazer a amarela agrupar o pelotão e mudar
/// quem ganha é outro trabalho, que exigiria recalibrar o balanceamento.
pub fn derive_caution_segments<'a>(
    incidents: impl IntoIterator<Item = &'a IncidentResult>,
) -> Vec<String> {
    let mut segs: Vec<String> = Vec::new();
    for inc in incidents {
        // Predicado em fonte ÚNICA (`estrategia::traz_bandeira_amarela`): o registro aqui e a
        // consequência do safety car no motor leem do mesmo lugar. Duplicar o critério seria a
        // maneira mais fácil de eles divergirem sem ninguém notar.
        if super::estrategia::traz_bandeira_amarela(inc) && !segs.contains(&inc.segment) {
            segs.push(inc.segment.clone());
        }
    }
    segs
}

pub(crate) fn build_race_results(
    drivers: &[SimDriver],
    qualifying: &[QualifyingResult],
    ctx: &SimulationContext,
    states: &[RaceState],
    rng: &mut impl Rng,
) -> Vec<RaceDriverResult> {
    // Lookup maps para evitar O(n²)
    let driver_map: HashMap<&str, &SimDriver> =
        drivers.iter().map(|d| (d.id.as_str(), d)).collect();
    let quali_map: HashMap<&str, &QualifyingResult> = qualifying
        .iter()
        .map(|q| (q.pilot_id.as_str(), q))
        .collect();

    // Separar finishers e DNFs para ordenação correta
    let mut finishers: Vec<&RaceState> = states.iter().filter(|s| !s.is_dnf).collect();
    let mut dnfs: Vec<&RaceState> = states.iter().filter(|s| s.is_dnf).collect();

    // Quem terminou: por TEMPO acumulado, menor na frente.
    finishers.sort_by(|a, b| {
        a.tempo_acumulado_ms
            .partial_cmp(&b.tempo_acumulado_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // DNFs: segmento mais tardio primeiro; desempate por tempo acumulado (menor na frente).
    dnfs.sort_by(|a, b| {
        let seg_ord_b = b.dnf_segment.map(|s| s.ordinal()).unwrap_or(0);
        let seg_ord_a = a.dnf_segment.map(|s| s.ordinal()).unwrap_or(0);
        seg_ord_b.cmp(&seg_ord_a).then_with(|| {
            a.tempo_acumulado_ms
                .partial_cmp(&b.tempo_acumulado_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    let ordered: Vec<&RaceState> = finishers.into_iter().chain(dnfs).collect();

    // O relógio publicado é ancorado no vencedor: o `RITMO_DE_REFERENCIA` embutido no tempo
    // acumulado cancela nesta subtração, e o que sobra é o gap real entre os carros.
    let tempo_do_vencedor_ms = ordered.first().map(|s| s.tempo_acumulado_ms).unwrap_or(0.0);
    let winner_lap_time_ms = ctx.base_lap_time_ms;
    let winner_total_time_ms = winner_lap_time_ms * ctx.total_laps as f64;

    ordered
        .iter()
        .enumerate()
        .filter_map(|(finish_idx, state)| {
            let driver = driver_map.get(state.driver_id.as_str())?;
            let qualifying_result = quali_map.get(state.driver_id.as_str())?;

            // O atraso deste carro para o vencedor JÁ é tempo — não há mais conversão de
            // moeda aqui, só uma subtração.
            let atraso_ms = (state.tempo_acumulado_ms - tempo_do_vencedor_ms).max(0.0);
            let lap_time_ms = ctx.base_lap_time_ms + atraso_ms / ctx.total_laps.max(1) as f64;
            let best_lap_factor = rng.gen_range(0.97..=1.0);
            let best_lap_time_ms = lap_time_ms * best_lap_factor;

            let laps_completed = if state.is_dnf {
                estimate_laps_at_dnf(state.dnf_segment, ctx.total_laps)
            } else {
                ctx.total_laps
            };

            let total_race_time_ms = if state.is_dnf {
                // Tempo proporcional às voltas completadas + pequeno overhead
                winner_total_time_ms * (laps_completed as f64 / ctx.total_laps as f64) * 1.05
            } else {
                winner_total_time_ms + atraso_ms
            };

            // gap sempre >= 0
            let gap_to_winner_ms = (total_race_time_ms - winner_total_time_ms).max(0.0);

            // Incidente mais importante para campo de conveniência
            let notable_incident = state
                .incidents
                .iter()
                .filter(|i| i.narrative_importance_hint >= 2)
                .max_by_key(|i| i.narrative_importance_hint)
                .map(|i| i.description.clone());

            let dnf_incident = state.incidents.iter().find(|i| i.is_dnf);
            let dnf_catalog_id = dnf_incident.and_then(|i| i.catalog_id.clone());
            let damage_origin_segment = dnf_incident.and_then(|i| i.damage_origin_segment.clone());

            let classification_status = if state.is_dnf {
                ClassificationStatus::Dnf
            } else {
                ClassificationStatus::Finished
            };

            let finish_position = finish_idx as i32 + 1;

            Some(RaceDriverResult {
                pilot_id: driver.id.clone(),
                pilot_name: driver.nome.clone(),
                team_id: driver.team_id.clone(),
                team_name: driver.team_name.clone(),
                grid_position: qualifying_result.position,
                finish_position,
                positions_gained: qualifying_result.position - finish_position,
                best_lap_time_ms,
                total_race_time_ms,
                gap_to_winner_ms,
                is_dnf: state.is_dnf,
                dnf_reason: state.dnf_reason.clone(),
                dnf_segment: state.dnf_segment.map(|s| s.as_str().to_string()),
                incidents_count: state.incidents.len() as i32,
                incidents: state.incidents.clone(),
                has_fastest_lap: false,
                points_earned: 0,
                is_jogador: driver.is_jogador,
                laps_completed,
                final_tire_wear: state.tire_wear,
                final_physical: state.physical_condition,
                classification_status,
                notable_incident,
                dnf_catalog_id,
                damage_origin_segment,
                // Dado cru de posição na pista, direto do estado vivo da corrida.
                posicoes_por_segmento: state.trafego.posicoes_por_segmento.clone(),
                gaps_para_da_frente_ms: state.trafego.gaps_para_da_frente_ms.clone(),
                segmentos_em_ar_sujo: state.trafego.segmentos_em_ar_sujo,
                tentativas_ultrapassagem: state.trafego.tentativas_ultrapassagem,
                ultrapassagens_concluidas: state.trafego.ultrapassagens_concluidas,
                tentativas_sofridas: state.trafego.tentativas_sofridas,
                maior_sequencia_preso: state.trafego.maior_sequencia_preso,
                // Dado cru de estratégia, direto do estado vivo da corrida.
                volta_da_parada: state.paradas.voltas.clone(),
                posicao_antes_da_parada: state.paradas.posicao_antes.clone(),
                posicao_depois: state.paradas.posicao_depois.clone(),
                estrategia_id: state.paradas.estrategia_id.clone(),
            })
        })
        .collect()
}

pub(crate) fn estimate_laps_at_dnf(segment: Option<RaceSegment>, total_laps: i32) -> i32 {
    let fraction = match segment {
        Some(RaceSegment::Start) => 0.10,
        Some(RaceSegment::Early) => 0.30,
        Some(RaceSegment::Mid) => 0.50,
        Some(RaceSegment::Late) => 0.70,
        Some(RaceSegment::Finish) => 0.90,
        None => 0.05,
    };
    ((total_laps as f64 * fraction) as i32).max(1)
}
