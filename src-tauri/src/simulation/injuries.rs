use rand::Rng;
use uuid::Uuid;

use crate::models::enums::InjuryType;
use crate::models::injury::Injury;
use crate::simulation::incidents::{injury_base_chance, IncidentResult};

pub(crate) fn injury_name_pool(injury_type: InjuryType) -> &'static [&'static str] {
    match injury_type {
        InjuryType::Leve => &[
            "Dor no braço",
            "Dor no ombro",
            "Dor no pescoço",
            "Dor nas costas",
        ],
        InjuryType::Moderada => &[
            "Braço machucado",
            "Ombro machucado",
            "Pescoço travado",
            "Costas travadas",
            "Joelho machucado",
            "Pulso machucado",
            "Tornozelo machucado",
            "Dor forte nas costas",
        ],
        InjuryType::Grave | InjuryType::Critica => &[
            "Braço fraturado",
            "Costela fraturada",
            "Ombro lesionado",
            "Joelho lesionado",
            "Tornozelo lesionado",
            "Lesão nas costas",
            "Lesão no pescoço",
        ],
    }
}

fn select_injury_name(injury_type: InjuryType, rng: &mut impl Rng) -> String {
    let pool = injury_name_pool(injury_type);
    let index = rng.gen_range(0..pool.len());
    pool[index].to_string()
}

/// Generates a persistent Injury from a simulated incident.
/// Uses `injury_risk_multiplier` as the source of truth for incident eligibility.
/// Returns None if the incident is not eligible or if the driver gets lucky.
pub fn generate_injury_from_incident(
    incident: &IncidentResult,
    season: i32,
    race_id: &str,
    rng: &mut impl Rng,
) -> Option<Injury> {
    if incident.injury_risk_multiplier <= 0.0 {
        return None;
    }

    let base_chance = injury_base_chance(incident.incident_type);
    let chance = (base_chance * incident.injury_risk_multiplier).min(0.70);

    if rng.gen_bool(chance) {
        let roll = rng.gen_range(1..=100);
        let (injury_type, modifier, races_total, skill_penalty) = if roll <= 60 {
            // 60% Leve
            (InjuryType::Leve, 0.95, 2, 0.05)
        } else if roll <= 90 {
            // 30% Moderada
            (InjuryType::Moderada, 0.88, 4, 0.10)
        } else {
            // 10% Grave
            (InjuryType::Grave, 0.75, 8, 0.15)
        };

        Some(Injury {
            id: Uuid::new_v4().to_string(),
            pilot_id: incident.pilot_id.clone(),
            injury_name: select_injury_name(injury_type.clone(), rng),
            injury_type,
            modifier,
            races_total,
            races_remaining: races_total,
            skill_penalty,
            season,
            race_occurred: race_id.to_string(),
            active: true,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use crate::simulation::incidents::{IncidentSeverity, IncidentType};

    use super::*;

    fn make_incident(incident_type: IncidentType, severity: IncidentSeverity) -> IncidentResult {
        use crate::simulation::incidents::IncidentResult;
        let irm = match (incident_type, severity) {
            (IncidentType::Collision, IncidentSeverity::Critical) => 1.5,
            (IncidentType::Collision, IncidentSeverity::Major) => 0.45,
            (IncidentType::DriverError, IncidentSeverity::Critical) => 1.0,
            (IncidentType::Mechanical, IncidentSeverity::Critical) => 0.6,
            _ => 0.0,
        };
        IncidentResult {
            pilot_id: "P1".to_string(),
            incident_type,
            severity,
            segment: "Start".to_string(),
            positions_lost: 0,
            is_dnf: true,
            description: "test".to_string(),
            linked_pilot_id: None,
            is_two_car_incident: false,
            injury_risk_multiplier: irm,
            narrative_importance_hint: if severity == IncidentSeverity::Critical {
                2
            } else {
                0
            },
            catalog_id: None,
            damage_origin_segment: None,
        }
    }

    #[test]
    fn test_irm_zero_returns_none() {
        let incident = make_incident(IncidentType::Collision, IncidentSeverity::Minor);
        let mut rng = StdRng::seed_from_u64(1);
        for _ in 0..100 {
            assert!(generate_injury_from_incident(&incident, 2026, "R001", &mut rng).is_none());
        }
    }

    #[test]
    fn test_non_critical_driver_error_returns_none() {
        let incident = make_incident(IncidentType::DriverError, IncidentSeverity::Major);
        let mut rng = StdRng::seed_from_u64(2);
        for _ in 0..100 {
            assert!(generate_injury_from_incident(&incident, 2026, "R001", &mut rng).is_none());
        }
    }

    #[test]
    fn test_collision_critical_higher_injury_rate_than_mechanical_critical() {
        let collision = make_incident(IncidentType::Collision, IncidentSeverity::Critical);
        let mechanical = make_incident(IncidentType::Mechanical, IncidentSeverity::Critical);

        let mut rng = StdRng::seed_from_u64(42);
        let mut collision_injuries = 0;
        let mut mechanical_injuries = 0;
        let runs = 1000;

        for _ in 0..runs {
            if generate_injury_from_incident(&collision, 2026, "R001", &mut rng).is_some() {
                collision_injuries += 1;
            }
            if generate_injury_from_incident(&mechanical, 2026, "R001", &mut rng).is_some() {
                mechanical_injuries += 1;
            }
        }

        assert!(
            collision_injuries > mechanical_injuries,
            "collision injuries={collision_injuries} should > mechanical injuries={mechanical_injuries}"
        );
    }

    #[test]
    fn test_injury_chance_capped_at_70_percent() {
        // Collision+Critical: base=0.50 * irm=1.5 = 0.75, capped to 0.70
        // Over many runs, rate should not exceed 75%
        let incident = make_incident(IncidentType::Collision, IncidentSeverity::Critical);
        let mut rng = StdRng::seed_from_u64(99);
        let mut injured = 0;
        let runs = 1000;
        for _ in 0..runs {
            if generate_injury_from_incident(&incident, 2026, "R001", &mut rng).is_some() {
                injured += 1;
            }
        }
        // Should be around 70%, definitely not above 85%
        assert!(injured < 850, "injury rate {injured}/1000 seems uncapped");
    }

    #[test]
    fn test_positive_irm_non_critical_incident_can_generate_injury() {
        let mut incident = make_incident(IncidentType::Collision, IncidentSeverity::Major);
        incident.injury_risk_multiplier = 0.4;

        let mut rng = StdRng::seed_from_u64(777);
        let mut injured = 0;
        for _ in 0..200 {
            if generate_injury_from_incident(&incident, 2026, "R001", &mut rng).is_some() {
                injured += 1;
            }
        }

        assert!(
            injured > 0,
            "positive IRM should allow injuries for eligible incidents, injured={injured}"
        );
    }

    #[test]
    fn test_collision_critical_generates_more_injuries_than_collision_major() {
        let collision_critical = make_incident(IncidentType::Collision, IncidentSeverity::Critical);
        let collision_major = make_incident(IncidentType::Collision, IncidentSeverity::Major);

        let mut rng = StdRng::seed_from_u64(20260501);
        let mut critical_injuries = 0;
        let mut major_injuries = 0;
        let runs = 1000;

        for _ in 0..runs {
            if generate_injury_from_incident(&collision_critical, 2026, "R001", &mut rng).is_some()
            {
                critical_injuries += 1;
            }
            if generate_injury_from_incident(&collision_major, 2026, "R001", &mut rng).is_some() {
                major_injuries += 1;
            }
        }

        assert!(
            major_injuries > 0,
            "major collision should now be eligible for injuries, major_injuries={major_injuries}"
        );
        assert!(
            critical_injuries > major_injuries,
            "critical injuries={critical_injuries} should stay above major injuries={major_injuries}"
        );
    }

    #[test]
    fn test_injury_name_pools_are_separated_by_severity() {
        let light_pool = injury_name_pool(InjuryType::Leve);
        let moderate_pool = injury_name_pool(InjuryType::Moderada);
        let grave_pool = injury_name_pool(InjuryType::Grave);

        assert!(light_pool.contains(&"Dor no braço"));
        assert!(moderate_pool.contains(&"Braço machucado"));
        assert!(grave_pool.contains(&"Braço fraturado"));
        assert!(!light_pool.contains(&"Braço fraturado"));
        assert!(!grave_pool.contains(&"Dor no braço"));
    }

    #[test]
    fn test_generated_injury_receives_specific_name_from_its_pool() {
        let incident = make_incident(IncidentType::Collision, IncidentSeverity::Critical);
        let mut rng = StdRng::seed_from_u64(20260502);

        let injury = (0..100)
            .find_map(|_| generate_injury_from_incident(&incident, 2026, "R001", &mut rng))
            .expect("expected deterministic seed to produce an injury");

        assert!(!injury.injury_name.is_empty());
        assert!(injury_name_pool(injury.injury_type).contains(&injury.injury_name.as_str()));
    }
}
