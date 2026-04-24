#![allow(dead_code)]

use rand::Rng;

use crate::common::time::current_timestamp;
use crate::finance::planning::{category_finance_scale, derive_budget_index_from_money};
use crate::market::pit_strategy::{
    seed_pit_crew_quality_from_team, seed_pit_strategy_risk_from_team,
};
use serde::{Deserialize, Serialize};

use crate::constants::categories::{get_category_config, is_especial};
use crate::constants::teams::{get_team_templates, TeamTemplate};
use crate::simulation::car_build::CarBuildProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamHierarchyClimate {
    Estavel,
    Competitivo,
    Tensao,
    Reavaliacao,
    Inversao,
    Crise,
}

impl TeamHierarchyClimate {
    pub fn as_str(&self) -> &'static str {
        match self {
            TeamHierarchyClimate::Estavel => "estavel",
            TeamHierarchyClimate::Competitivo => "competitivo",
            TeamHierarchyClimate::Tensao => "tensao",
            TeamHierarchyClimate::Reavaliacao => "reavaliacao",
            TeamHierarchyClimate::Inversao => "inversao",
            TeamHierarchyClimate::Crise => "crise",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "competitivo" => TeamHierarchyClimate::Competitivo,
            "tensao" => TeamHierarchyClimate::Tensao,
            "reavaliacao" => TeamHierarchyClimate::Reavaliacao,
            "inversao" => TeamHierarchyClimate::Inversao,
            "crise" => TeamHierarchyClimate::Crise,
            // Compatibilidade com o schema antigo.
            "n1" | "n2" | "independente" | "estavel" | "claro" => TeamHierarchyClimate::Estavel,
            _ => TeamHierarchyClimate::Estavel,
        }
    }

    pub fn from_str_strict(value: &str) -> Result<Self, String> {
        match value.trim().to_lowercase().as_str() {
            "estavel" | "n1" | "n2" | "independente" | "claro" => Ok(TeamHierarchyClimate::Estavel),
            "competitivo" => Ok(TeamHierarchyClimate::Competitivo),
            "tensao" => Ok(TeamHierarchyClimate::Tensao),
            "reavaliacao" => Ok(TeamHierarchyClimate::Reavaliacao),
            "inversao" => Ok(TeamHierarchyClimate::Inversao),
            "crise" => Ok(TeamHierarchyClimate::Crise),
            other => Err(format!("TeamHierarchyClimate invalido: '{other}'")),
        }
    }

    pub fn from_tensao(tensao: f64) -> Self {
        let tensao = tensao.clamp(0.0, 100.0);
        if tensao < 20.0 {
            TeamHierarchyClimate::Estavel
        } else if tensao < 40.0 {
            TeamHierarchyClimate::Competitivo
        } else if tensao < 60.0 {
            TeamHierarchyClimate::Tensao
        } else if tensao < 75.0 {
            TeamHierarchyClimate::Reavaliacao
        } else if tensao < 90.0 {
            TeamHierarchyClimate::Inversao
        } else {
            TeamHierarchyClimate::Crise
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub nome: String,
    pub nome_curto: String,
    pub cor_primaria: String,
    pub cor_secundaria: String,
    pub pais_sede: String,
    pub ano_fundacao: i32,
    pub categoria: String,
    pub ativa: bool,
    pub marca: Option<String>,
    pub classe: Option<String>,
    pub piloto_1_id: Option<String>,
    pub piloto_2_id: Option<String>,
    pub car_performance: f64,
    pub car_build_profile: CarBuildProfile,
    pub confiabilidade: f64,
    pub pit_strategy_risk: f64,
    pub pit_crew_quality: f64,
    pub budget: f64,
    pub cash_balance: f64,
    pub debt_balance: f64,
    pub financial_state: String,
    pub season_strategy: String,
    pub last_round_income: f64,
    pub last_round_expenses: f64,
    pub last_round_net: f64,
    pub parachute_payment_remaining: f64,
    pub facilities: f64,
    pub engineering: f64,
    pub reputacao: f64,
    pub morale: f64,
    pub aerodinamica: f64,
    pub motor: f64,
    pub chassi: f64,
    pub hierarquia_n1_id: Option<String>,
    pub hierarquia_n2_id: Option<String>,
    pub hierarquia_status: String,
    pub hierarquia_tensao: f64,
    pub hierarquia_duelos_total: i32,
    pub hierarquia_duelos_n2_vencidos: i32,
    pub hierarquia_sequencia_n2: i32,
    pub hierarquia_sequencia_n1: i32,
    pub hierarquia_inversoes_temporada: i32,
    pub stats_vitorias: i32,
    pub stats_podios: i32,
    pub stats_poles: i32,
    pub stats_pontos: i32,
    pub stats_melhor_resultado: i32,
    pub historico_vitorias: i32,
    pub historico_podios: i32,
    pub historico_poles: i32,
    pub historico_pontos: i32,
    pub historico_titulos_pilotos: i32,
    pub historico_titulos_construtores: i32,
    pub temporada_atual: i32,
    pub created_at: String,
    pub updated_at: String,
    // Campos ja existentes no schema atual.
    pub is_player_team: bool,
    pub parent_team_id: Option<String>,
    pub aceita_rookies: bool,
    pub meta_posicao: i32,
    pub temp_posicao: i32,
    /// Categoria da equipe na temporada anterior (Some se foi promovida/rebaixada, None caso contrário).
    pub categoria_anterior: Option<String>,
}

impl Team {
    pub fn from_template(
        template: &TeamTemplate,
        category_id: &str,
        team_id: String,
        temporada: i32,
    ) -> Team {
        let mut rng = rand::thread_rng();
        Self::from_template_with_rng(template, category_id, team_id, temporada, &mut rng)
    }

    pub(crate) fn from_template_with_rng(
        template: &TeamTemplate,
        category_id: &str,
        team_id: String,
        temporada: i32,
        rng: &mut impl Rng,
    ) -> Team {
        let timestamp = current_timestamp();
        let car_performance = clamp_f64(
            template.car_performance_base + rng.gen_range(-2.0..=2.0),
            -5.0,
            16.0,
        );
        let budget_seed = clamp_f64(template.budget_base + rng.gen_range(-5.0..=5.0), 0.0, 100.0);
        let finance_scale = category_finance_scale(category_id);
        let cash_balance = finance_scale.cash_min
            + (finance_scale.cash_max - finance_scale.cash_min) * (budget_seed / 100.0);
        let facilities = clamp_f64(50.0 + rng.gen_range(-10.0..=15.0), 0.0, 100.0);
        let engineering = clamp_f64(50.0 + rng.gen_range(-10.0..=15.0), 0.0, 100.0);
        let mut team = Team {
            id: team_id,
            nome: template.nome.to_string(),
            nome_curto: template.nome_curto.to_string(),
            cor_primaria: template.cor_primaria.to_string(),
            cor_secundaria: template.cor_secundaria.to_string(),
            pais_sede: template.pais_sede.to_string(),
            ano_fundacao: temporada - rng.gen_range(5..=20),
            categoria: category_id.to_string(),
            ativa: true,
            marca: template.marca.map(str::to_string),
            classe: template.classe.map(str::to_string),
            piloto_1_id: None,
            piloto_2_id: None,
            car_performance,
            car_build_profile: CarBuildProfile::Balanced,
            confiabilidade: clamp_f64(60.0 + rng.gen_range(-10.0..=10.0), 0.0, 100.0),
            pit_strategy_risk: 50.0,
            pit_crew_quality: 50.0,
            budget: 0.0,
            cash_balance,
            debt_balance: 0.0,
            financial_state: "stable".to_string(),
            season_strategy: "balanced".to_string(),
            last_round_income: 0.0,
            last_round_expenses: 0.0,
            last_round_net: 0.0,
            parachute_payment_remaining: 0.0,
            facilities,
            engineering,
            reputacao: clamp_f64(
                template.reputacao_base + rng.gen_range(-3.0..=3.0),
                0.0,
                100.0,
            ),
            morale: 1.0,
            aerodinamica: clamp_f64(50.0 + rng.gen_range(-10.0..=15.0), 0.0, 100.0),
            motor: clamp_f64(50.0 + rng.gen_range(-10.0..=15.0), 0.0, 100.0),
            chassi: clamp_f64(50.0 + rng.gen_range(-10.0..=15.0), 0.0, 100.0),
            hierarquia_n1_id: None,
            hierarquia_n2_id: None,
            hierarquia_status: TeamHierarchyClimate::Estavel.as_str().to_string(),
            hierarquia_tensao: 0.0,
            hierarquia_duelos_total: 0,
            hierarquia_duelos_n2_vencidos: 0,
            hierarquia_sequencia_n2: 0,
            hierarquia_sequencia_n1: 0,
            hierarquia_inversoes_temporada: 0,
            stats_vitorias: 0,
            stats_podios: 0,
            stats_poles: 0,
            stats_pontos: 0,
            stats_melhor_resultado: 0,
            historico_vitorias: 0,
            historico_podios: 0,
            historico_poles: 0,
            historico_pontos: 0,
            historico_titulos_pilotos: 0,
            historico_titulos_construtores: 0,
            temporada_atual: temporada,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            is_player_team: false,
            parent_team_id: None,
            aceita_rookies: true,
            meta_posicao: 10,
            temp_posicao: 0,
            categoria_anterior: None,
        };
        team.budget = derive_budget_index_from_money(&team);
        team.pit_strategy_risk = seed_pit_strategy_risk_from_team(&team);
        team.pit_crew_quality = seed_pit_crew_quality_from_team(&team);
        team.budget = derive_budget_index_from_money(&team);
        team
    }
}

pub fn generate_teams_for_category<F>(
    category_id: &str,
    temporada: i32,
    id_generator: &mut F,
) -> Vec<Team>
where
    F: FnMut() -> String,
{
    let mut rng = rand::thread_rng();
    generate_teams_for_category_with_rng(category_id, temporada, id_generator, &mut rng)
}

fn generate_teams_for_category_with_rng<F, R>(
    category_id: &str,
    temporada: i32,
    id_generator: &mut F,
    rng: &mut R,
) -> Vec<Team>
where
    F: FnMut() -> String,
    R: Rng,
{
    let templates = get_team_templates(category_id);
    let teams: Vec<Team> = templates
        .into_iter()
        .map(|template| {
            Team::from_template_with_rng(template, category_id, id_generator(), temporada, rng)
        })
        .collect();

    if let Some(config) = get_category_config(category_id) {
        let expected_team_count = if is_especial(category_id) {
            config
                .classes
                .iter()
                .filter(|class| get_category_config(class.car_categoria).is_none())
                .map(|class| class.num_equipes as usize)
                .sum()
        } else {
            config.num_equipes as usize
        };

        assert_eq!(
            teams.len(),
            expected_team_count,
            "Quantidade de equipes persistentes gerada para '{}' difere da configuracao da categoria",
            category_id
        );
    }

    teams
}

pub fn hierarchy_status_from_tensao(tensao: f64) -> TeamHierarchyClimate {
    TeamHierarchyClimate::from_tensao(tensao)
}

pub fn placeholder_team_from_db(
    id: String,
    nome: String,
    categoria: String,
    created_at: String,
) -> Team {
    Team {
        id,
        nome_curto: nome.clone(),
        nome,
        cor_primaria: String::new(),
        cor_secundaria: String::new(),
        pais_sede: String::new(),
        ano_fundacao: 0,
        categoria,
        ativa: true,
        marca: None,
        classe: None,
        piloto_1_id: None,
        piloto_2_id: None,
        car_performance: 50.0,
        car_build_profile: CarBuildProfile::Balanced,
        confiabilidade: 50.0,
        pit_strategy_risk: 50.0,
        pit_crew_quality: 50.0,
        budget: 0.0,
        cash_balance: 0.0,
        debt_balance: 0.0,
        financial_state: "stable".to_string(),
        season_strategy: "balanced".to_string(),
        last_round_income: 0.0,
        last_round_expenses: 0.0,
        last_round_net: 0.0,
        parachute_payment_remaining: 0.0,
        facilities: 0.0,
        engineering: 0.0,
        reputacao: 50.0,
        morale: 1.0,
        aerodinamica: 0.0,
        motor: 0.0,
        chassi: 0.0,
        hierarquia_n1_id: None,
        hierarquia_n2_id: None,
        hierarquia_status: TeamHierarchyClimate::Estavel.as_str().to_string(),
        hierarquia_tensao: 0.0,
        hierarquia_duelos_total: 0,
        hierarquia_duelos_n2_vencidos: 0,
        hierarquia_sequencia_n2: 0,
        hierarquia_sequencia_n1: 0,
        hierarquia_inversoes_temporada: 0,
        stats_vitorias: 0,
        stats_podios: 0,
        stats_poles: 0,
        stats_pontos: 0,
        stats_melhor_resultado: 0,
        historico_vitorias: 0,
        historico_podios: 0,
        historico_poles: 0,
        historico_pontos: 0,
        historico_titulos_pilotos: 0,
        historico_titulos_construtores: 0,
        temporada_atual: 0,
        created_at: created_at.clone(),
        updated_at: created_at,
        is_player_team: false,
        parent_team_id: None,
        aceita_rookies: true,
        meta_posicao: 10,
        temp_posicao: 0,
        categoria_anterior: None,
    }
}

fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    value.clamp(min, max)
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_team_from_template_basic_fields() {
        let template = get_team_templates("gt3")[0];
        let mut rng = StdRng::seed_from_u64(11);
        let team =
            Team::from_template_with_rng(template, "gt3", "T001".to_string(), 2026, &mut rng);

        assert_eq!(team.id, "T001");
        assert_eq!(team.nome, template.nome);
        assert_eq!(team.nome_curto, template.nome_curto);
        assert_eq!(team.cor_primaria, template.cor_primaria);
        assert_eq!(team.cor_secundaria, template.cor_secundaria);
        assert_eq!(team.pais_sede, template.pais_sede);
        assert_eq!(team.categoria, "gt3");
        assert_eq!(team.marca.as_deref(), template.marca);
        assert_eq!(team.classe.as_deref(), template.classe);
    }

    #[test]
    fn test_team_from_template_performance_has_variation() {
        let template = get_team_templates("gt3")[0];
        let mut rng = StdRng::seed_from_u64(22);
        let team =
            Team::from_template_with_rng(template, "gt3", "T002".to_string(), 2026, &mut rng);

        assert!(team.car_performance >= template.car_performance_base - 2.0);
        assert!(team.car_performance <= template.car_performance_base + 2.0);
    }

    #[test]
    fn test_team_default_values() {
        let template = get_team_templates("mazda_rookie")[0];
        let mut rng = StdRng::seed_from_u64(33);
        let team = Team::from_template_with_rng(
            template,
            "mazda_rookie",
            "T003".to_string(),
            2026,
            &mut rng,
        );

        assert!(team.ativa);
        assert_eq!(team.morale, 1.0);
        assert_eq!(team.hierarquia_status, "estavel");
        assert_eq!(team.hierarquia_tensao, 0.0);
        assert_eq!(team.car_build_profile, CarBuildProfile::Balanced);
        assert!((0.0..=100.0).contains(&team.pit_strategy_risk));
        assert!((0.0..=100.0).contains(&team.pit_crew_quality));
        assert_eq!(team.stats_vitorias, 0);
        assert_eq!(team.stats_podios, 0);
        assert_eq!(team.stats_poles, 0);
        assert_eq!(team.stats_pontos, 0);
        assert_eq!(team.historico_vitorias, 0);
        assert_eq!(team.historico_titulos_construtores, 0);
    }

    #[test]
    fn test_team_initial_cash_respects_category_money_scale() {
        let template = get_team_templates("mazda_rookie")[0];
        let mut rng = StdRng::seed_from_u64(34);
        let team = Team::from_template_with_rng(
            template,
            "mazda_rookie",
            "T004".to_string(),
            2026,
            &mut rng,
        );
        let scale = crate::finance::planning::category_finance_scale("mazda_rookie");
        let expected_budget = crate::finance::planning::derive_budget_index_from_money(&team);

        assert!(team.cash_balance >= scale.cash_min);
        assert!(team.cash_balance <= scale.cash_max);
        assert!((team.budget - expected_budget).abs() < 0.0001);
    }

    #[test]
    fn test_generate_teams_for_category_correct_count() {
        let mut rng = StdRng::seed_from_u64(44);
        let mut seq = 1_u32;
        let mut next_id = || {
            let id = format!("T{:03}", seq);
            seq += 1;
            id
        };

        let teams = generate_teams_for_category_with_rng("gt3", 2026, &mut next_id, &mut rng);

        assert_eq!(teams.len(), 14);
        assert_eq!(teams.first().map(|team| team.id.as_str()), Some("T001"));
        assert_eq!(teams.last().map(|team| team.id.as_str()), Some("T014"));
    }

    #[test]
    fn test_hierarchy_status_from_tensao() {
        assert_eq!(
            hierarchy_status_from_tensao(0.0),
            TeamHierarchyClimate::Estavel
        );
        assert_eq!(
            hierarchy_status_from_tensao(25.0),
            TeamHierarchyClimate::Competitivo
        );
        assert_eq!(
            hierarchy_status_from_tensao(50.0),
            TeamHierarchyClimate::Tensao
        );
        assert_eq!(
            hierarchy_status_from_tensao(70.0),
            TeamHierarchyClimate::Reavaliacao
        );
        assert_eq!(
            hierarchy_status_from_tensao(85.0),
            TeamHierarchyClimate::Inversao
        );
        assert_eq!(
            hierarchy_status_from_tensao(95.0),
            TeamHierarchyClimate::Crise
        );
    }
}
