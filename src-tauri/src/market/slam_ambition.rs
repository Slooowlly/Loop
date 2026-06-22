//! Cérebro do "slam-chasing" da IA (lógica pura, sem DB).
//!
//! Dado o histórico de títulos + skill + se o piloto é ambicioso, decide se ele
//! persegue um slam de prestígio e QUE categoria ele quer correr na próxima
//! temporada. Essa preferência é levada ao mercado pelo chamador — como um
//! slam-chaser de elite é super-qualificado, qualquer equipe da categoria-alvo o
//! aceita, então a preferência quase sempre vira contrato.
//!
//! FSM (definida pelo usuário):
//! - Venceu a categoria: coleta a base. Sequência > 3 títulos → dinastia (fica).
//!   Perdeu a dinastia → +1 temporada; se falhar de novo → vai pra base diferente.
//!   Sem dinastia → move pra próxima base do slam.
//! - Não venceu (e sem dinastia ativa) → desiste do slam → sobe de categoria.
//! - Pode ir pra qualquer categoria (cima/lado/baixo); o mercado aceita.
//!
//! Os slams espelham `crown_bonus` em `commands/global_driver_rankings.rs`.
//! LMP2 não é categoria autônoma — só existe como classe da Endurance.

#![allow(dead_code)] // wiring no mercado vem na próxima camada

/// Um título conquistado (uma temporada): categoria-base + classe (multiclasse).
#[derive(Debug, Clone)]
pub struct TitleWin {
    pub category: String,
    pub class: Option<String>,
}

impl TitleWin {
    pub fn new(category: &str, class: Option<&str>) -> Self {
        Self {
            category: category.to_string(),
            class: class.map(str::to_string),
        }
    }
}

/// Os 5 slams, em ordem crescente de prestígio (mesmos pesos do `crown_bonus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slam {
    Cup,
    Gt,
    Production,
    GtSuper,
    Endurance,
}

/// Uma base de um slam: a categoria (e classe, p/ especiais) que precisa vencer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlamBase {
    pub category: String,
    pub class: Option<String>,
}

fn base(category: &str, class: Option<&str>) -> SlamBase {
    SlamBase {
        category: category.to_string(),
        class: class.map(str::to_string),
    }
}

impl Slam {
    /// Valor de completar o slam (prioridade de alvo; espelha `crown_bonus`).
    pub fn value(self) -> u32 {
        match self {
            Slam::Cup => 1500,
            Slam::Gt => 2500,
            Slam::Production => 3500,
            Slam::GtSuper => 5000,
            Slam::Endurance => 8000,
        }
    }

    /// As bases que o piloto precisa vencer pra completar o slam.
    pub fn bases(self) -> Vec<SlamBase> {
        match self {
            Slam::Cup => vec![
                base("mazda_amador", None),
                base("toyota_amador", None),
                base("bmw_m2", None),
            ],
            Slam::Gt => vec![base("gt4", None), base("gt3", None)],
            Slam::Production => vec![
                base("production_challenger", Some("mazda")),
                base("production_challenger", Some("toyota")),
                base("production_challenger", Some("bmw")),
            ],
            // LMP2 só existe como classe da Endurance.
            Slam::GtSuper => vec![
                base("gt4", None),
                base("gt3", None),
                base("endurance", Some("lmp2")),
            ],
            Slam::Endurance => vec![
                base("endurance", Some("gt4")),
                base("endurance", Some("gt3")),
                base("endurance", Some("lmp2")),
            ],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Slam::Cup => "Cup Slam",
            Slam::Gt => "GT Slam",
            Slam::Production => "Production Slam",
            Slam::GtSuper => "GT Super Slam",
            Slam::Endurance => "Endurance Slam",
        }
    }
}

const ALL_SLAMS: [Slam; 5] = [
    Slam::Cup,
    Slam::Gt,
    Slam::Production,
    Slam::GtSuper,
    Slam::Endurance,
];

/// Skill mínimo realista pra VENCER uma categoria-base (gate de "alcançável").
fn category_win_skill(category: &str) -> f64 {
    match category {
        "mazda_amador" | "toyota_amador" => 58.0,
        "bmw_m2" => 66.0,
        "production_challenger" => 70.0,
        "gt4" => 74.0,
        "gt3" => 82.0,
        "endurance" => 86.0,
        _ => 60.0,
    }
}

/// A base já foi conquistada? (classe `None` = qualquer título na categoria.)
fn base_collected(base: &SlamBase, history: &[TitleWin]) -> bool {
    history.iter().any(|title| {
        title.category == base.category
            && match &base.class {
                Some(class) => title.class.as_deref() == Some(class.as_str()),
                None => true,
            }
    })
}

/// Bases ainda não conquistadas de um slam (na ordem canônica).
fn needed_bases(slam: Slam, history: &[TitleWin]) -> Vec<SlamBase> {
    slam.bases()
        .into_iter()
        .filter(|base| !base_collected(base, history))
        .collect()
}

fn slam_complete(slam: Slam, history: &[TitleWin]) -> bool {
    needed_bases(slam, history).is_empty()
}

/// O slam é alcançável dada a skill? (precisa conseguir vencer todas as bases.)
fn slam_achievable(slam: Slam, skill: f64) -> bool {
    slam.bases()
        .iter()
        .all(|base| skill >= category_win_skill(&base.category))
}

/// Slam-alvo = o INCOMPLETO ALCANÇÁVEL de maior valor. `None` se já fez todos os
/// que consegue (ou não alcança nenhum).
pub fn target_slam(history: &[TitleWin], skill: f64) -> Option<Slam> {
    ALL_SLAMS
        .into_iter()
        .filter(|&slam| slam_achievable(slam, skill) && !slam_complete(slam, history))
        .max_by_key(|&slam| slam.value())
}

/// Decisão de carreira para o mercado.
#[derive(Debug, Clone, PartialEq)]
pub enum SlamDecision {
    /// Quer correr esta categoria (classe, se especial) pra coletar uma base.
    Chase {
        target: Slam,
        category: String,
        class: Option<String>,
        rationale: &'static str,
    },
    /// Fica na categoria atual (dinastia ou tentativa extra pós-derrota).
    Stay {
        target: Slam,
        rationale: &'static str,
    },
}

/// Resumo do histórico do piloto na categoria ATUAL (derivado pelo chamador a
/// partir do archive): resultado de campeão por temporada, do mais antigo ao mais
/// recente, só das temporadas em que correu a categoria atual.
fn trailing_run(results: &[bool], want: bool) -> u32 {
    results
        .iter()
        .rev()
        .take_while(|&&champ| champ == want)
        .count() as u32
}

fn has_dynasty(results: &[bool]) -> bool {
    let mut run = 0u32;
    for &champ in results {
        if champ {
            run += 1;
            if run > 3 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// Decide a preferência de carreira do piloto-IA. `None` = sem preferência de slam
/// (não é ambicioso, esgotou os slams alcançáveis, ou falhou e deve subir normal).
///
/// - `history`: todos os títulos de carreira (categoria + classe).
/// - `current_category`: onde corre hoje.
/// - `skill`, `ambitious`: gate de quem persegue slam.
/// - `current_results`: campeão-ou-não por temporada na categoria atual (antigo→recente).
pub fn decide(
    history: &[TitleWin],
    current_category: &str,
    skill: f64,
    ambitious: bool,
    current_results: &[bool],
) -> Option<SlamDecision> {
    if !ambitious {
        return None;
    }
    let target = target_slam(history, skill)?;
    let needed = needed_bases(target, history);
    if needed.is_empty() {
        return None;
    }

    let current_is_base = target
        .bases()
        .iter()
        .any(|base| base.category == current_category);
    let won_last = current_results.last() == Some(&true);
    let streak = trailing_run(current_results, true);
    let seasons_since_title = trailing_run(current_results, false);
    let dynasty = has_dynasty(current_results);

    // Primeira base ainda faltante DIFERENTE da categoria atual (pra "mover").
    let next_base = needed
        .iter()
        .find(|base| base.category != current_category)
        .or_else(|| needed.first());
    let chase = |base: &SlamBase, rationale: &'static str| {
        Some(SlamDecision::Chase {
            target,
            category: base.category.clone(),
            class: base.class.clone(),
            rationale,
        })
    };

    if current_is_base {
        if won_last {
            if streak > 3 {
                // Dinastia: continua dominando a mesma categoria.
                return Some(SlamDecision::Stay {
                    target,
                    rationale: "dinastia (>3 títulos seguidos)",
                });
            }
            // Venceu a base → vai pra próxima base faltante.
            return next_base.and_then(|base| chase(base, "venceu a base, vai pra próxima"));
        }
        // Não venceu na categoria atual.
        if dynasty && seasons_since_title == 1 {
            // Perdeu o título de uma dinastia → tenta mais 1 temporada.
            return Some(SlamDecision::Stay {
                target,
                rationale: "tentativa extra após perder o título",
            });
        }
        if dynasty {
            // Já tentou a temporada extra e falhou → vai pra base diferente.
            return next_base.and_then(|base| chase(base, "abandona dinastia, base diferente"));
        }
        // Falhou sem dinastia → desiste do slam, sobe normal.
        return None;
    }

    // Não está numa base do slam → vai pra primeira base faltante.
    needed
        .first()
        .and_then(|base| chase(base, "entra na primeira base do slam"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wins(items: &[(&str, Option<&str>)]) -> Vec<TitleWin> {
        items.iter().map(|(c, k)| TitleWin::new(c, *k)).collect()
    }

    #[test]
    fn non_ambitious_never_chases() {
        let decision = decide(&[], "gt4", 90.0, false, &[]);
        assert!(decision.is_none());
    }

    #[test]
    fn target_scales_with_skill() {
        assert_eq!(target_slam(&[], 60.0), None); // nem o Cup (bmw_m2 precisa 66)
        assert_eq!(target_slam(&[], 67.0), Some(Slam::Cup));
        assert_eq!(target_slam(&[], 72.0), Some(Slam::Production));
        assert_eq!(target_slam(&[], 90.0), Some(Slam::Endurance));
    }

    #[test]
    fn high_skill_targets_endurance() {
        assert_eq!(target_slam(&[], 90.0), Some(Slam::Endurance));
    }

    #[test]
    fn won_base_moves_to_next_needed_base() {
        // Caçando Cup, já tem mazda, está no mazda_amador e venceu → vai pra outra base.
        let history = wins(&[("mazda_amador", None)]);
        let decision = decide(&history, "mazda_amador", 67.0, true, &[true]);
        match decision {
            Some(SlamDecision::Chase {
                target, category, ..
            }) => {
                assert_eq!(target, Slam::Cup);
                assert!(category == "toyota_amador" || category == "bmw_m2");
            }
            other => panic!("esperava Chase, veio {other:?}"),
        }
    }

    #[test]
    fn dynasty_stays() {
        let history = wins(&[("mazda_amador", None)]);
        // 4 títulos seguidos na categoria atual = dinastia.
        let decision = decide(
            &history,
            "mazda_amador",
            67.0,
            true,
            &[true, true, true, true],
        );
        assert!(matches!(decision, Some(SlamDecision::Stay { .. })));
    }

    #[test]
    fn dynasty_lost_one_season_tries_again() {
        let history = wins(&[("mazda_amador", None)]);
        // Dinastia (4) e perdeu 1 temporada → tentativa extra.
        let decision = decide(
            &history,
            "mazda_amador",
            67.0,
            true,
            &[true, true, true, true, false],
        );
        assert!(matches!(decision, Some(SlamDecision::Stay { .. })));
    }

    #[test]
    fn dynasty_lost_two_seasons_moves_to_different_base() {
        let history = wins(&[("mazda_amador", None)]);
        let decision = decide(
            &history,
            "mazda_amador",
            67.0,
            true,
            &[true, true, true, true, false, false],
        );
        match decision {
            Some(SlamDecision::Chase { category, .. }) => assert_ne!(category, "mazda_amador"),
            other => panic!("esperava Chase pra base diferente, veio {other:?}"),
        }
    }

    #[test]
    fn failed_without_dynasty_gives_up_and_promotes() {
        // Está numa base, nunca venceu (sem dinastia) → desiste (None = sobe normal).
        let decision = decide(&[], "mazda_amador", 67.0, true, &[false]);
        assert!(decision.is_none());
    }

    #[test]
    fn enters_first_base_when_not_in_a_slam_category() {
        // Skill p/ Cup, está no rookie (não é base), nada coletado → entra na 1ª base.
        let decision = decide(&[], "mazda_rookie", 67.0, true, &[true]);
        match decision {
            Some(SlamDecision::Chase {
                target, category, ..
            }) => {
                assert_eq!(target, Slam::Cup);
                assert_eq!(category, "mazda_amador");
            }
            other => panic!("esperava Chase, veio {other:?}"),
        }
    }

    #[test]
    fn lmp2_collected_via_endurance_class() {
        // GtSuper precisa de gt4 + gt3 + endurance:lmp2.
        let history = wins(&[("gt4", None), ("gt3", None), ("endurance", Some("lmp2"))]);
        // GtSuper completo; com skill alto o alvo passa a ser Endurance (incompleto).
        assert!(slam_complete(Slam::GtSuper, &history));
        assert_eq!(target_slam(&history, 90.0), Some(Slam::Endurance));
    }

    #[test]
    fn completed_target_advances_to_next_value() {
        // Cup completo → alvo vira o próximo alcançável de maior valor.
        let history = wins(&[
            ("mazda_amador", None),
            ("toyota_amador", None),
            ("bmw_m2", None),
        ]);
        assert!(slam_complete(Slam::Cup, &history));
        // skill 72 alcança Production (70) mas não GT (gt3=82).
        assert_eq!(target_slam(&history, 72.0), Some(Slam::Production));
    }
}
