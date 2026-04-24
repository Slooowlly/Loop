#![allow(dead_code)]

use rand::Rng;

use crate::common::time::current_timestamp;
use serde::{Deserialize, Serialize};

use crate::constants::categories::get_category_config;
use crate::models::enums::{ContractStatus, ContractType, TeamRole};

// Formula de atratividade de proposta (uso futuro no modulo de mercado):
// score = (car_performance / 100) * 30
//       + (categoria_tier / 7) * 25
//       + bonus_papel (N1: 15, N2: 8)
//       + salario_normalizado * 15
//       + (reputacao / 100) * 10
//       + min(2, duracao_anos) * 2.5

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    pub id: String,
    pub piloto_id: String,
    pub piloto_nome: String,
    pub equipe_id: String,
    pub equipe_nome: String,
    pub temporada_inicio: i32,
    pub duracao_anos: i32,
    pub temporada_fim: i32,
    pub salario_anual: f64,
    pub papel: TeamRole,
    pub status: ContractStatus,
    pub tipo: ContractType,
    pub categoria: String,
    /// Classe específica em categorias multi-classe (ex: "gt3", "mazda").
    /// Preenchido apenas em contratos especiais; None em contratos regulares.
    #[serde(default)]
    pub classe: Option<String>,
    pub created_at: String,
}

impl Contract {
    pub fn new(
        id: String,
        piloto_id: String,
        piloto_nome: String,
        equipe_id: String,
        equipe_nome: String,
        temporada_inicio: i32,
        duracao_anos: i32,
        salario_anual: f64,
        papel: TeamRole,
        categoria: String,
    ) -> Self {
        let duracao_anos = duracao_anos.clamp(1, 3);
        Self {
            id,
            piloto_id,
            piloto_nome,
            equipe_id,
            equipe_nome,
            temporada_inicio,
            duracao_anos,
            temporada_fim: temporada_inicio + duracao_anos - 1,
            salario_anual,
            papel,
            status: ContractStatus::Ativo,
            tipo: ContractType::Regular,
            categoria,
            classe: None,
            created_at: current_timestamp(),
        }
    }

    pub fn is_ativo(&self) -> bool {
        self.status == ContractStatus::Ativo
    }

    pub fn expira_na_temporada(&self, temporada: i32) -> bool {
        self.temporada_fim == temporada
    }

    pub fn is_ultimo_ano(&self, temporada_atual: i32) -> bool {
        self.is_ativo() && self.temporada_fim == temporada_atual
    }

    pub fn anos_restantes(&self, temporada_atual: i32) -> i32 {
        (self.temporada_fim - temporada_atual).max(0)
    }

    pub fn expirar(&mut self) {
        self.status = ContractStatus::Expirado;
    }

    pub fn rescindir(&mut self) {
        self.status = ContractStatus::Rescindido;
    }
}

pub fn generate_initial_contract(
    contract_id: String,
    piloto_id: &str,
    piloto_nome: &str,
    equipe_id: &str,
    equipe_nome: &str,
    papel: TeamRole,
    categoria: &str,
    temporada: i32,
) -> Contract {
    let mut rng = rand::thread_rng();
    generate_initial_contract_with_rng(
        contract_id,
        piloto_id,
        piloto_nome,
        equipe_id,
        equipe_nome,
        papel,
        categoria,
        temporada,
        &mut rng,
    )
}

pub(crate) fn generate_initial_contract_with_rng(
    contract_id: String,
    piloto_id: &str,
    piloto_nome: &str,
    equipe_id: &str,
    equipe_nome: &str,
    papel: TeamRole,
    categoria: &str,
    temporada: i32,
    rng: &mut impl Rng,
) -> Contract {
    let tier = get_category_config(categoria)
        .map(|config| config.tier)
        .unwrap_or(0);
    let (base_min, base_max) = salary_range_for_tier(tier);
    let duracao_anos = weighted_duration(rng);
    let salary_multiplier = match papel {
        TeamRole::Numero1 => rng.gen_range(1.20..=1.40),
        TeamRole::Numero2 => rng.gen_range(1.00..=1.12),
    };
    let salario_base = rng.gen_range(base_min..=base_max);
    let salario_anual = (salario_base * salary_multiplier).round();

    Contract::new(
        contract_id,
        piloto_id.to_string(),
        piloto_nome.to_string(),
        equipe_id.to_string(),
        equipe_nome.to_string(),
        temporada,
        duracao_anos,
        salario_anual,
        papel,
        categoria.to_string(),
    )
}

fn weighted_duration(rng: &mut impl Rng) -> i32 {
    match rng.gen_range(0..100) {
        0..=39 => 1,
        40..=79 => 2,
        _ => 3,
    }
}

fn salary_range_for_tier(tier: u8) -> (f64, f64) {
    match tier {
        0 => (5_000.0, 15_000.0),
        1 => (15_000.0, 40_000.0),
        2 => (30_000.0, 80_000.0),
        3 => (60_000.0, 150_000.0),
        4 => (100_000.0, 300_000.0),
        5 => (80_000.0, 250_000.0),
        _ => (5_000.0, 15_000.0),
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_contract_new_calculates_temporada_fim() {
        let contract = Contract::new(
            "C001".to_string(),
            "P001".to_string(),
            "Piloto".to_string(),
            "T001".to_string(),
            "Equipe".to_string(),
            1,
            3,
            100_000.0,
            TeamRole::Numero1,
            "gt3".to_string(),
        );

        assert_eq!(contract.temporada_fim, 3);
    }

    #[test]
    fn test_contract_is_ativo() {
        let mut contract = sample_contract();
        assert!(contract.is_ativo());

        contract.expirar();
        assert!(!contract.is_ativo());
    }

    #[test]
    fn test_contract_expira_na_temporada() {
        let contract = sample_contract_with_duration(1, 3);
        assert!(contract.expira_na_temporada(3));
        assert!(!contract.expira_na_temporada(2));
    }

    #[test]
    fn test_contract_is_ultimo_ano() {
        let contract = sample_contract_with_duration(1, 3);
        assert!(contract.is_ultimo_ano(3));
        assert!(!contract.is_ultimo_ano(2));
    }

    #[test]
    fn test_contract_anos_restantes() {
        let contract = sample_contract_with_duration(3, 3);
        assert_eq!(contract.anos_restantes(3), 2);

        let contract_final = sample_contract_with_duration(1, 3);
        assert_eq!(contract_final.anos_restantes(3), 0);
    }

    #[test]
    fn test_contract_expirar() {
        let mut contract = sample_contract();
        contract.expirar();
        assert_eq!(contract.status, ContractStatus::Expirado);
    }

    #[test]
    fn test_contract_rescindir() {
        let mut contract = sample_contract();
        contract.rescindir();
        assert_eq!(contract.status, ContractStatus::Rescindido);
    }

    #[test]
    fn test_generate_initial_contract_salary_range() {
        let mut rng = StdRng::seed_from_u64(101);
        let contract = generate_initial_contract_with_rng(
            "C001".to_string(),
            "P001",
            "Piloto",
            "T001",
            "Equipe",
            TeamRole::Numero2,
            "gt3",
            1,
            &mut rng,
        );

        assert!(contract.salario_anual >= 100_000.0);
        assert!(contract.salario_anual <= 336_000.0);
    }

    #[test]
    fn test_generate_initial_contract_n1_earns_more() {
        let mut total_n1 = 0.0;
        let mut total_n2 = 0.0;

        for seed in 1..=200 {
            let mut rng_n1 = StdRng::seed_from_u64(seed);
            let mut rng_n2 = StdRng::seed_from_u64(seed + 10_000);

            total_n1 += generate_initial_contract_with_rng(
                format!("C{:03}", seed),
                "P001",
                "Piloto 1",
                "T001",
                "Equipe",
                TeamRole::Numero1,
                "gt4",
                1,
                &mut rng_n1,
            )
            .salario_anual;

            total_n2 += generate_initial_contract_with_rng(
                format!("C{:03}", seed + 500),
                "P002",
                "Piloto 2",
                "T001",
                "Equipe",
                TeamRole::Numero2,
                "gt4",
                1,
                &mut rng_n2,
            )
            .salario_anual;
        }

        assert!(total_n1 / 200.0 > total_n2 / 200.0);
    }

    #[test]
    fn test_generate_initial_contract_duration_range() {
        for seed in 1..=50 {
            let mut rng = StdRng::seed_from_u64(seed);
            let contract = generate_initial_contract_with_rng(
                format!("C{:03}", seed),
                "P001",
                "Piloto",
                "T001",
                "Equipe",
                TeamRole::Numero2,
                "mazda_rookie",
                1,
                &mut rng,
            );

            assert!((1..=3).contains(&contract.duracao_anos));
        }
    }

    fn sample_contract() -> Contract {
        sample_contract_with_duration(1, 2)
    }

    fn sample_contract_with_duration(temporada_inicio: i32, duracao: i32) -> Contract {
        Contract::new(
            "C001".to_string(),
            "P001".to_string(),
            "Piloto".to_string(),
            "T001".to_string(),
            "Equipe".to_string(),
            temporada_inicio,
            duracao,
            100_000.0,
            TeamRole::Numero1,
            "gt3".to_string(),
        )
    }
}
