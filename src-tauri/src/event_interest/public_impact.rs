#![allow(dead_code)]

use std::collections::HashMap;

use crate::event_interest::models::{InterestTier, RealizedEventInterest};
use crate::models::injury::Injury;

// ── Tipos públicos ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum MediaImpactReason {
    Win,
    Pole,
    Podium,
    MainIncident,
    Injury,
}

/// Impacto consolidado de mídia pública para um driver após uma corrida.
/// `reasons` preserva todos os papéis que contribuíram, em ordem de processamento,
/// com deduplicação (sem repetição do mesmo reason por piloto).
#[derive(Debug, Clone)]
pub struct DriverMediaImpact {
    pub driver_id: String,
    pub delta: f64,
    pub reasons: Vec<MediaImpactReason>,
}

/// Contexto mínimo e explícito da corrida para cálculo de impacto público.
///
/// `winner_id` e `pole_sitter_id` são strings canônicas (sempre válidas em corrida concluída).
/// `podium_ids`: apenas P2 e P3 elegíveis (!dnf). `winner_id` deve ser excluído do slice
/// pelo call site (garantia de mutuidade Win/Podium).
/// `main_incident_pilot_id`: curadoria editorial v1 — apenas o piloto central do incidente
/// narrativamente principal. Escolha de um único piloto é deliberada, não perda acidental.
/// `excluded_driver_id`: excluído por dupla aplicação (já recebe tratamento player-facing),
/// não por ausência do mundo simulado.
#[derive(Debug, Clone)]
pub struct RaceEventContext<'a> {
    pub winner_id: &'a str,
    pub pole_sitter_id: &'a str,
    pub podium_ids: &'a [&'a str],
    pub main_incident_pilot_id: Option<&'a str>,
    pub excluded_driver_id: &'a str,
}

// ── Cálculo de domínio puro ───────────────────────────────────────────────────

fn tier_multiplier(tier: &InterestTier) -> f64 {
    match tier {
        InterestTier::Baixo => 0.3,
        InterestTier::Moderado => 0.7,
        InterestTier::Alto => 1.0,
        InterestTier::MuitoAlto => 1.5,
        InterestTier::EventoPrincipal => 2.5,
    }
}

/// Calcula impactos de mídia pública para pilotos AI relevantes de uma corrida.
///
/// Sem `RealizedEventInterest`, este bloco não deve ser chamado — a dependência
/// semântica é explícita: sem importância pública calculada, não há impacto público persistente.
///
/// O `excluded_driver_id` (jogador) é omitido de todos os papéis para evitar dupla aplicação
/// com o pipeline player-facing de media/motivação já existente.
///
/// Base deltas (antes do multiplicador de tier):
/// - Win: +3.0
/// - Pole (somente se polesitter ≠ vencedor): +1.5
/// - Podium P2/P3: +1.0
/// - MainIncident: +1.5
/// - Injury: +1.0
pub fn compute_public_media_impacts(
    ctx: &RaceEventContext<'_>,
    injuries: &[Injury],
    realized: &RealizedEventInterest,
) -> Vec<DriverMediaImpact> {
    let mult = tier_multiplier(&realized.final_tier);
    let mut accum: HashMap<String, (f64, Vec<MediaImpactReason>)> = HashMap::new();

    // Acumula delta e reason apenas se o reason ainda não foi contabilizado para este piloto.
    // Isso evita dupla contagem quando o mesmo reason ocorre múltiplas vezes (ex.: dois
    // registros de Injury para o mesmo pilot_id): o primeiro conta, os demais são ignorados.
    let mut add = |id: &str, base: f64, reason: MediaImpactReason| {
        if id.is_empty() || id == ctx.excluded_driver_id {
            return;
        }
        let entry = accum.entry(id.to_string()).or_insert((0.0, Vec::new()));
        if !entry.1.contains(&reason) {
            entry.0 += base * mult;
            entry.1.push(reason);
        }
    };

    // Win
    add(ctx.winner_id, 3.0, MediaImpactReason::Win);

    // Pole — somente se diferente do vencedor (curadoria: vitória já é o ápice do evento)
    if ctx.pole_sitter_id != ctx.winner_id {
        add(ctx.pole_sitter_id, 1.5, MediaImpactReason::Pole);
    }

    // Podium P2 e P3
    for &id in ctx.podium_ids {
        add(id, 1.0, MediaImpactReason::Podium);
    }

    // Incidente principal — piloto central, curadoria editorial v1
    if let Some(id) = ctx.main_incident_pilot_id {
        add(id, 1.5, MediaImpactReason::MainIncident);
    }

    // Lesões novas da corrida
    for injury in injuries {
        add(&injury.pilot_id, 1.0, MediaImpactReason::Injury);
    }

    // Converter em Vec ordenado deterministicamente por driver_id
    let mut result: Vec<DriverMediaImpact> = accum
        .into_iter()
        .map(|(driver_id, (delta, reasons))| DriverMediaImpact {
            driver_id,
            delta,
            reasons,
        })
        .collect();
    result.sort_by(|a, b| a.driver_id.cmp(&b.driver_id));
    result
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_interest::models::{HeadlineStrength, InterestTier, RealizedEventInterest};
    use crate::models::enums::InjuryType;
    use crate::models::injury::Injury;

    fn make_realized(tier: InterestTier) -> RealizedEventInterest {
        RealizedEventInterest {
            expected_display_value: 0,
            expected_tier: InterestTier::Baixo,
            final_score: 0.0,
            final_display_value: 0,
            final_tier: tier,
            delta_vs_expected: 0.0,
            media_delta_modifier: 1.0,
            motivation_delta_modifier: 1.0,
            news_importance_bias: 0,
            headline_strength: HeadlineStrength::Normal,
        }
    }

    fn make_injury(pilot_id: &str) -> Injury {
        Injury {
            id: format!("INJ_{pilot_id}"),
            pilot_id: pilot_id.to_string(),
            injury_type: InjuryType::Leve,
            injury_name: "Dor no braço".to_string(),
            modifier: 0.95,
            races_total: 2,
            races_remaining: 2,
            skill_penalty: 0.05,
            season: 1,
            race_occurred: "R01".to_string(),
            active: true,
        }
    }

    fn ctx_simple<'a>(
        winner: &'a str,
        pole: &'a str,
        podium: &'a [&'a str],
        incident: Option<&'a str>,
        excluded: &'a str,
    ) -> RaceEventContext<'a> {
        RaceEventContext {
            winner_id: winner,
            pole_sitter_id: pole,
            podium_ids: podium,
            main_incident_pilot_id: incident,
            excluded_driver_id: excluded,
        }
    }

    #[test]
    fn test_win_low_vs_high_interest_different_delta() {
        let ctx = ctx_simple("P001", "P002", &["P003", "P004"], None, "PLAYER");
        let low = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Baixo));
        let high =
            compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::EventoPrincipal));

        let winner_low = low.iter().find(|d| d.driver_id == "P001").unwrap();
        let winner_high = high.iter().find(|d| d.driver_id == "P001").unwrap();
        assert!(winner_high.delta > winner_low.delta);
    }

    #[test]
    fn test_polesitter_winner_only_gets_win() {
        // Mesmo piloto ganhou a pole e a corrida → recebe apenas Win, sem Pole
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], None, "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        let winner = impacts.iter().find(|d| d.driver_id == "P001").unwrap();
        assert!(winner.reasons.contains(&MediaImpactReason::Win));
        assert!(!winner.reasons.contains(&MediaImpactReason::Pole));
    }

    #[test]
    fn test_pole_different_pilot_separate_impact() {
        let ctx = ctx_simple("P001", "P002", &["P003", "P004"], None, "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        let winner = impacts.iter().find(|d| d.driver_id == "P001").unwrap();
        let poler = impacts.iter().find(|d| d.driver_id == "P002").unwrap();
        assert!(winner.reasons.contains(&MediaImpactReason::Win));
        assert!(poler.reasons.contains(&MediaImpactReason::Pole));
        // Pole delta < Win delta ao mesmo tier
        assert!(winner.delta > poler.delta);
    }

    #[test]
    fn test_winner_not_in_podium_role() {
        // call site garante que winner não está em podium_ids, mas verificamos que
        // mesmo que estivesse, Win e Podium não se duplicam incorretamente
        let ctx = ctx_simple("P001", "P099", &["P002", "P003"], None, "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        let winner = impacts.iter().find(|d| d.driver_id == "P001").unwrap();
        assert!(winner.reasons.contains(&MediaImpactReason::Win));
        assert!(!winner.reasons.contains(&MediaImpactReason::Podium));
    }

    #[test]
    fn test_main_incident_pilot_receives_impact() {
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], Some("P005"), "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        let inc_pilot = impacts.iter().find(|d| d.driver_id == "P005").unwrap();
        assert!(inc_pilot.reasons.contains(&MediaImpactReason::MainIncident));
        assert!(inc_pilot.delta > 0.0);
    }

    #[test]
    fn test_injury_generates_impact() {
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], None, "PLAYER");
        let injury = make_injury("P006");
        let impacts =
            compute_public_media_impacts(&ctx, &[injury], &make_realized(InterestTier::Alto));

        let injured = impacts.iter().find(|d| d.driver_id == "P006").unwrap();
        assert!(injured.reasons.contains(&MediaImpactReason::Injury));
        assert!(injured.delta > 0.0);
    }

    #[test]
    fn test_excluded_driver_absent_from_all_roles() {
        // O jogador é o vencedor — não deve aparecer no Vec
        let ctx = ctx_simple(
            "PLAYER",
            "PLAYER",
            &["P002", "P003"],
            Some("PLAYER"),
            "PLAYER",
        );
        let injury = make_injury("PLAYER");
        let impacts =
            compute_public_media_impacts(&ctx, &[injury], &make_realized(InterestTier::Alto));

        assert!(impacts.iter().all(|d| d.driver_id != "PLAYER"));
    }

    #[test]
    fn test_excluded_main_incident_absent() {
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], Some("PLAYER"), "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        assert!(impacts.iter().all(|d| d.driver_id != "PLAYER"));
    }

    #[test]
    fn test_multiple_reasons_preserved() {
        // Mesmo piloto: vence a corrida e está lesionado
        let ctx = ctx_simple("P001", "P099", &["P002", "P003"], None, "PLAYER");
        let injury = make_injury("P001");
        let impacts =
            compute_public_media_impacts(&ctx, &[injury], &make_realized(InterestTier::Alto));

        let pilot = impacts.iter().find(|d| d.driver_id == "P001").unwrap();
        assert!(pilot.reasons.contains(&MediaImpactReason::Win));
        assert!(pilot.reasons.contains(&MediaImpactReason::Injury));
        // Delta acumulado de Win + Injury
        let win_delta = 3.0 * tier_multiplier(&InterestTier::Alto);
        let inj_delta = 1.0 * tier_multiplier(&InterestTier::Alto);
        assert!((pilot.delta - (win_delta + inj_delta)).abs() < 1e-9);
    }

    #[test]
    fn test_duplicate_injury_counts_once() {
        // Se injuries contiver dois registros para o mesmo pilot_id,
        // o Injury deve ser contabilizado apenas uma vez (delta e reason).
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], None, "PLAYER");
        let injuries = vec![make_injury("P006"), make_injury("P006")];
        let impacts =
            compute_public_media_impacts(&ctx, &injuries, &make_realized(InterestTier::Alto));

        let injured = impacts.iter().find(|d| d.driver_id == "P006").unwrap();
        let expected_delta = 1.0 * tier_multiplier(&InterestTier::Alto);
        assert!(
            (injured.delta - expected_delta).abs() < 1e-9,
            "delta duplicado: esperado {expected_delta}, obtido {}",
            injured.delta
        );
        assert_eq!(
            injured
                .reasons
                .iter()
                .filter(|r| **r == MediaImpactReason::Injury)
                .count(),
            1,
            "Injury deve aparecer apenas uma vez em reasons"
        );
    }

    #[test]
    fn test_main_incident_and_injury_same_pilot() {
        // MainIncident e Injury são papéis distintos e independentes.
        // Um piloto que foi o incidente principal E ficou lesionado deve ter ambos
        // os papéis preservados em reasons e os deltas acumulados corretamente.
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], Some("P006"), "PLAYER");
        let injury = make_injury("P006");
        let impacts =
            compute_public_media_impacts(&ctx, &[injury], &make_realized(InterestTier::Alto));

        let pilot = impacts.iter().find(|d| d.driver_id == "P006").unwrap();
        assert!(pilot.reasons.contains(&MediaImpactReason::MainIncident));
        assert!(pilot.reasons.contains(&MediaImpactReason::Injury));
        // Delta = MainIncident (1.5) + Injury (1.0) × tier_multiplier(Alto=1.0)
        let expected_delta = (1.5 + 1.0) * tier_multiplier(&InterestTier::Alto);
        assert!(
            (pilot.delta - expected_delta).abs() < 1e-9,
            "delta esperado {expected_delta}, obtido {}",
            pilot.delta
        );
    }

    #[test]
    fn test_only_eligible_roles_impacted() {
        // Fixture controlado: 4 pilotos com papéis conhecidos, nenhum extra
        let ctx = ctx_simple("P001", "P002", &["P003", "P004"], Some("P005"), "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        let ids: Vec<&str> = impacts.iter().map(|d| d.driver_id.as_str()).collect();
        assert!(ids.contains(&"P001")); // Win
        assert!(ids.contains(&"P002")); // Pole
        assert!(ids.contains(&"P003")); // Podium P2
        assert!(ids.contains(&"P004")); // Podium P3
        assert!(ids.contains(&"P005")); // MainIncident
                                        // Nenhum piloto fora dos papéis definidos
        assert_eq!(impacts.len(), 5);
    }
}
