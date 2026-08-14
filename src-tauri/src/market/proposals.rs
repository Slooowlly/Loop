#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::models::enums::TeamRole;

pub fn is_real_career_debut_category(category_id: &str) -> bool {
    matches!(category_id, "mazda_rookie" | "toyota_rookie")
}

pub fn is_rookie_market_candidate(
    target_category: &str,
    candidate_category: &str,
    career_races: u32,
    career_seasons: u32,
) -> bool {
    if !is_real_career_debut_category(target_category) {
        return false;
    }

    if career_races == 0 && career_seasons == 0 {
        return true;
    }

    candidate_category == target_category && career_seasons <= 2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketProposal {
    pub id: String,
    pub equipe_id: String,
    pub equipe_nome: String,
    pub piloto_id: String,
    pub piloto_nome: String,
    pub categoria: String,
    pub papel: TeamRole,
    pub salario_oferecido: f64,
    pub duracao_anos: i32,
    pub status: ProposalStatus,
    pub motivo_recusa: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProposalStatus {
    Pendente,
    Aceita,
    Recusada,
    Expirada,
}

impl ProposalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProposalStatus::Pendente => "Pendente",
            ProposalStatus::Aceita => "Aceita",
            ProposalStatus::Recusada => "Recusada",
            ProposalStatus::Expirada => "Expirada",
        }
    }

    pub fn from_str_strict(value: &str) -> Result<Self, String> {
        match value.trim() {
            "Pendente" => Ok(Self::Pendente),
            "Aceita" => Ok(Self::Aceita),
            "Recusada" => Ok(Self::Recusada),
            "Expirada" => Ok(Self::Expirada),
            other => Err(format!("ProposalStatus inválido: '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarketReport {
    pub contracts_expired: i32,
    pub contracts_renewed: i32,
    pub new_signings: Vec<SigningInfo>,
    pub retirements_replaced: i32,
    pub rookies_placed: i32,
    pub proposals_made: i32,
    pub proposals_accepted: i32,
    pub proposals_rejected: i32,
    pub player_proposals: Vec<MarketProposal>,
    pub unresolved_vacancies: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningInfo {
    pub driver_id: String,
    pub driver_name: String,
    pub team_id: String,
    pub team_name: String,
    pub categoria: String,
    pub papel: String,
    pub tipo: String,
}

#[derive(Debug, Clone)]
pub struct Vacancy {
    pub team_id: String,
    pub team_name: String,
    pub categoria: String,
    /// Classe da divisão competitiva (`categoria + classe`). `None` para
    /// categorias regulares; `Some(..)` para Production/Endurance multiclasse.
    pub classe: Option<String>,
    pub category_tier: u8,
    /// Força do carro da equipe em **0–100** ([`crate::models::team::Team::car_strength`]).
    /// Carregava a coluna legada em 0–16, e cada consumidor a re-normalizava do seu jeito.
    pub car_strength: f64,
    pub budget: f64,
    pub cash_balance: f64,
    pub debt_balance: f64,
    pub financial_state: String,
    pub reputacao: f64,
    pub papel_necessario: TeamRole,
    pub piloto_existente_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_as_duas_categorias_rookie_sao_estreia_de_carreira() {
        assert!(is_real_career_debut_category("mazda_rookie"));
        assert!(is_real_career_debut_category("toyota_rookie"));
        // O Amador é a segunda categoria da escada, não a estreia: um piloto gerado ali
        // nasceria por cima de quem subiu do Rookie.
        assert!(!is_real_career_debut_category("mazda_amador"));
        assert!(!is_real_career_debut_category("gt3"));
        assert!(!is_real_career_debut_category(""));
    }

    #[test]
    fn candidato_a_rookie_precisa_de_vaga_de_estreia() {
        // Fora da categoria de estreia ninguém é candidato a rookie, nem quem nunca correu.
        assert!(!is_rookie_market_candidate("gt3", "gt3", 0, 0));
        assert!(!is_rookie_market_candidate(
            "mazda_amador",
            "mazda_rookie",
            0,
            0
        ));
    }

    #[test]
    fn quem_nunca_correu_entra_em_qualquer_vaga_de_estreia() {
        // Sem corrida e sem temporada: o estreante puro, venha de onde vier.
        assert!(is_rookie_market_candidate("mazda_rookie", "", 0, 0));
        assert!(is_rookie_market_candidate("toyota_rookie", "gt3", 0, 0));
    }

    #[test]
    fn quem_ja_correu_so_fica_no_proprio_rookie_e_por_pouco_tempo() {
        // Mesma categoria e até 2 temporadas: o rookie que repete o ano.
        assert!(is_rookie_market_candidate(
            "mazda_rookie",
            "mazda_rookie",
            20,
            1
        ));
        assert!(is_rookie_market_candidate(
            "mazda_rookie",
            "mazda_rookie",
            40,
            2
        ));
        // Passou de 2 temporadas: não volta a ocupar assento de estreia.
        assert!(!is_rookie_market_candidate(
            "mazda_rookie",
            "mazda_rookie",
            60,
            3
        ));
        // Veio de outra categoria com carreira andada: o assento de estreia não é dele.
        assert!(!is_rookie_market_candidate(
            "mazda_rookie",
            "toyota_rookie",
            20,
            1
        ));
    }

    #[test]
    fn status_de_proposta_faz_ida_e_volta_pelo_texto() {
        for status in [
            ProposalStatus::Pendente,
            ProposalStatus::Aceita,
            ProposalStatus::Recusada,
            ProposalStatus::Expirada,
        ] {
            let texto = status.as_str();
            assert_eq!(
                ProposalStatus::from_str_strict(texto).expect("status conhecido"),
                status,
                "'{texto}' não volta ao mesmo status"
            );
        }
    }

    #[test]
    fn status_desconhecido_falha_em_vez_de_virar_pendente() {
        // O status vem do banco. Cair num default silencioso reabriria proposta recusada.
        let erro = ProposalStatus::from_str_strict("Sei la").expect_err("status invalido");
        assert!(erro.contains("Sei la"), "{erro}");
        assert!(ProposalStatus::from_str_strict("").is_err());
        // Espaço em volta é aparado — a coluna já veio com padding em saves antigos.
        assert_eq!(
            ProposalStatus::from_str_strict("  Aceita  ").expect("aparado"),
            ProposalStatus::Aceita
        );
    }

    #[test]
    fn relatorio_do_mercado_nasce_zerado() {
        // O report é acumulado por várias etapas da entressafra; começar com lixo faria a
        // semana nascer com assinaturas que não aconteceram.
        let report = MarketReport::default();
        assert_eq!(report.contracts_expired, 0);
        assert_eq!(report.contracts_renewed, 0);
        assert_eq!(report.rookies_placed, 0);
        assert_eq!(report.proposals_made, 0);
        assert_eq!(report.unresolved_vacancies, 0);
        assert!(report.new_signings.is_empty());
        assert!(report.player_proposals.is_empty());
    }

    #[test]
    fn proposta_do_jogador_faz_ida_e_volta_no_json() {
        // A proposta é persistida em JSON no plano da pré-temporada e relida na decisão.
        let proposal = MarketProposal {
            id: "PROP1".to_string(),
            equipe_id: "T1".to_string(),
            equipe_nome: "Equipe Um".to_string(),
            piloto_id: "P1".to_string(),
            piloto_nome: "Piloto Um".to_string(),
            categoria: "gt3".to_string(),
            papel: TeamRole::Numero1,
            salario_oferecido: 250_000.0,
            duracao_anos: 2,
            status: ProposalStatus::Pendente,
            motivo_recusa: None,
        };
        let texto = serde_json::to_string(&proposal).expect("serializa");
        let voltou: MarketProposal = serde_json::from_str(&texto).expect("desserializa");
        assert_eq!(voltou.id, proposal.id);
        assert_eq!(voltou.salario_oferecido, proposal.salario_oferecido);
        assert_eq!(voltou.status, ProposalStatus::Pendente);
        assert!(voltou.motivo_recusa.is_none());
    }
}
