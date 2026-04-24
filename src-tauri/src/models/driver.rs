#![allow(dead_code)]

use std::collections::HashSet;

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::common::time::current_year;
use crate::models::driver_tags::get_attribute_tag;
use crate::models::enums::{DriverStatus, PrimaryPersonality, SecondaryPersonality};

pub use crate::models::driver_tags::{AttributeTag, TagLevel};

// TODO(migration): o schema atual já cobre todos os campos persistidos do Módulo 10.
// Se no futuro tags visíveis ou metadados de geração forem persistidos, novas colunas seriam necessárias.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverAttributes {
    pub skill: f64,
    pub consistencia: f64,
    pub racecraft: f64,
    pub defesa: f64,
    pub ritmo_classificacao: f64,
    pub gestao_pneus: f64,
    pub habilidade_largada: f64,
    pub adaptabilidade: f64,
    pub fator_chuva: f64,
    pub fitness: f64,
    pub experiencia: f64,
    pub desenvolvimento: f64,
    pub aggression: f64,
    pub smoothness: f64,
    pub midia: f64,
    pub mentalidade: f64,
    pub confianca: f64,
}

impl Default for DriverAttributes {
    fn default() -> Self {
        Self {
            skill: 50.0,
            consistencia: 50.0,
            racecraft: 50.0,
            defesa: 50.0,
            ritmo_classificacao: 50.0,
            gestao_pneus: 50.0,
            habilidade_largada: 50.0,
            adaptabilidade: 50.0,
            fator_chuva: 50.0,
            fitness: 50.0,
            experiencia: 50.0,
            desenvolvimento: 50.0,
            aggression: 50.0,
            smoothness: 50.0,
            midia: 50.0,
            mentalidade: 50.0,
            confianca: 50.0,
        }
    }
}

impl DriverAttributes {
    pub fn entries(&self) -> Vec<(&'static str, f64)> {
        vec![
            ("skill", self.skill),
            ("consistencia", self.consistencia),
            ("racecraft", self.racecraft),
            ("defesa", self.defesa),
            ("ritmo_classificacao", self.ritmo_classificacao),
            ("gestao_pneus", self.gestao_pneus),
            ("habilidade_largada", self.habilidade_largada),
            ("adaptabilidade", self.adaptabilidade),
            ("fator_chuva", self.fator_chuva),
            ("fitness", self.fitness),
            ("experiencia", self.experiencia),
            ("desenvolvimento", self.desenvolvimento),
            ("aggression", self.aggression),
            ("smoothness", self.smoothness),
            ("midia", self.midia),
            ("mentalidade", self.mentalidade),
            ("confianca", self.confianca),
        ]
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriverSeasonStats {
    pub pontos: f64,
    pub vitorias: u32,
    pub podios: u32,
    pub poles: u32,
    pub corridas: u32,
    pub dnfs: u32,
    pub posicao_media: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriverCareerStats {
    pub pontos_total: f64,
    pub vitorias: u32,
    pub podios: u32,
    pub poles: u32,
    pub corridas: u32,
    pub temporadas: u32,
    pub titulos: u32,
    pub dnfs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Driver {
    pub id: String,
    pub nome: String,
    pub is_jogador: bool,
    pub idade: u32,
    pub nacionalidade: String,
    pub genero: String,
    pub categoria_atual: Option<String>,
    #[serde(default)]
    pub categoria_especial_ativa: Option<String>,
    pub status: DriverStatus,
    pub personalidade_primaria: Option<PrimaryPersonality>,
    pub personalidade_secundaria: Option<SecondaryPersonality>,
    pub ano_inicio_carreira: u32,
    #[serde(default)]
    pub atributos: DriverAttributes,
    #[serde(default)]
    pub stats_temporada: DriverSeasonStats,
    #[serde(default)]
    pub stats_carreira: DriverCareerStats,
    #[serde(default = "default_motivation")]
    pub motivacao: f64,
    #[serde(default = "default_track_history")]
    pub historico_circuitos: serde_json::Value,
    #[serde(default = "default_recent_results")]
    pub ultimos_resultados: serde_json::Value,
    pub melhor_resultado_temp: Option<u32>,
    pub temporadas_na_categoria: u32,
    pub corridas_na_categoria: u32,
    pub temporadas_motivacao_baixa: u32,
}

impl Driver {
    pub fn new(
        id: String,
        nome: String,
        nacionalidade: String,
        genero: String,
        idade: u32,
        ano_inicio_carreira: u32,
    ) -> Self {
        Self {
            id,
            nome,
            is_jogador: false,
            idade,
            nacionalidade,
            genero,
            categoria_atual: None,
            categoria_especial_ativa: None,
            status: DriverStatus::Ativo,
            personalidade_primaria: None,
            personalidade_secundaria: None,
            ano_inicio_carreira,
            atributos: DriverAttributes::default(),
            stats_temporada: DriverSeasonStats::default(),
            stats_carreira: DriverCareerStats::default(),
            motivacao: default_motivation(),
            historico_circuitos: default_track_history(),
            ultimos_resultados: default_recent_results(),
            melhor_resultado_temp: None,
            temporadas_na_categoria: 0,
            corridas_na_categoria: 0,
            temporadas_motivacao_baixa: 0,
        }
    }

    pub fn new_player(
        id: String,
        nome: String,
        nacionalidade: String,
        idade: u32,
        current_year: u32,
    ) -> Self {
        let mut driver = Self::new(
            id,
            nome,
            nacionalidade,
            "M".to_string(),
            idade,
            current_year.saturating_sub(idade.saturating_sub(16)),
        );
        driver.is_jogador = true;
        driver.personalidade_primaria = None;
        driver.personalidade_secundaria = None;
        driver.atributos = DriverAttributes::default();
        driver.motivacao = 70.0;
        driver
    }

    pub fn create_player(id: String, nome: String, nacionalidade: String, idade: i32) -> Self {
        Self::new_player(
            id,
            nome,
            nacionalidade,
            idade.clamp(16, 60) as u32,
            current_year(),
        )
    }

    pub fn generate_for_category(
        category_id: &str,
        category_tier: u8,
        difficulty: &str,
        count: usize,
        existing_names: &mut HashSet<String>,
        rng: &mut impl Rng,
    ) -> Vec<Self> {
        crate::models::driver_generation::generate_for_category(
            category_id,
            category_tier,
            difficulty,
            count,
            existing_names,
            rng,
        )
    }

    pub(crate) fn generate_for_category_with_id_factory<F, R>(
        category_id: &str,
        category_tier: u8,
        difficulty: &str,
        count: usize,
        existing_names: &mut HashSet<String>,
        id_factory: &mut F,
        rng: &mut R,
    ) -> Vec<Self>
    where
        F: FnMut() -> String,
        R: Rng,
    {
        crate::models::driver_generation::generate_for_category_with_id_factory(
            category_id,
            category_tier,
            difficulty,
            count,
            existing_names,
            id_factory,
            rng,
        )
    }

    pub fn get_visible_tags(&self) -> Vec<AttributeTag> {
        self.atributos
            .entries()
            .into_iter()
            .filter_map(|(attribute_name, value)| get_attribute_tag(attribute_name, value))
            .collect()
    }

    pub fn get_all_tags(&self) -> Vec<AttributeTag> {
        self.get_visible_tags()
    }

    pub fn attribute_tag(attribute_name: &'static str, value: f64) -> Option<AttributeTag> {
        get_attribute_tag(attribute_name, value)
    }

    pub fn reset_season_stats(&mut self) {
        self.stats_temporada = DriverSeasonStats::default();
        self.melhor_resultado_temp = None;
        self.ultimos_resultados = default_recent_results();
    }

    pub fn accumulate_career_stats(&mut self) {
        let season = &self.stats_temporada;
        let career = &mut self.stats_carreira;
        career.pontos_total += season.pontos;
        career.vitorias += season.vitorias;
        career.podios += season.podios;
        career.poles += season.poles;
        career.corridas += season.corridas;
        career.dnfs += season.dnfs;
        career.temporadas += 1;
    }
}

fn default_motivation() -> f64 {
    75.0
}

fn default_track_history() -> serde_json::Value {
    serde_json::json!({})
}

fn default_recent_results() -> serde_json::Value {
    serde_json::json!([])
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::constants::skill_ranges;

    #[test]
    fn test_get_visible_tags_returns_only_extreme_attributes() {
        let mut driver = Driver::new(
            "DRV-1".to_string(),
            "Teste".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            22,
            2024,
        );
        driver.atributos.skill = 97.0;
        driver.atributos.consistencia = 20.0;

        let tags = driver.get_visible_tags();
        assert_eq!(tags.len(), 2);
        assert!(tags
            .iter()
            .any(|tag| tag.attribute_name == "skill" && tag.tag_text == "Alien"));
        assert!(tags
            .iter()
            .any(|tag| tag.attribute_name == "consistencia" && tag.tag_text == "Inconsistente"));
    }

    #[test]
    fn test_player_has_personalities_none() {
        let player = Driver::new_player(
            "PLY-1".to_string(),
            "Jogador".to_string(),
            "Brasil".to_string(),
            20,
            2024,
        );
        assert!(player.personalidade_primaria.is_none());
        assert!(player.personalidade_secundaria.is_none());
        assert_eq!(player.atributos.skill, 50.0);
        assert_eq!(player.motivacao, 70.0);
        assert!(player
            .atributos
            .entries()
            .into_iter()
            .all(|(_, value)| value == 50.0));
    }

    #[test]
    fn test_generate_for_category_uses_real_names() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut existing_names = HashSet::new();
        let drivers =
            Driver::generate_for_category("gt4", 3, "medio", 8, &mut existing_names, &mut rng);

        assert!(!drivers.is_empty());
        assert!(drivers
            .iter()
            .all(|driver| driver.nome.split_whitespace().count() >= 2));
        assert!(drivers.iter().all(|driver| driver.nome != "IA"));
    }

    #[test]
    fn test_generate_for_category_no_name_collisions() {
        let mut rng = StdRng::seed_from_u64(7);
        let mut existing_names = HashSet::new();
        let drivers =
            Driver::generate_for_category("gt3", 4, "medio", 40, &mut existing_names, &mut rng);

        let unique_names: HashSet<_> = drivers.iter().map(|driver| driver.nome.clone()).collect();
        assert_eq!(unique_names.len(), drivers.len());
    }

    #[test]
    fn test_generate_for_category_rookie_has_prodigies() {
        let mut rng = StdRng::seed_from_u64(99);
        let mut existing_names = HashSet::new();
        let drivers = Driver::generate_for_category(
            "mazda_rookie",
            0,
            "medio",
            12,
            &mut existing_names,
            &mut rng,
        );

        let prodigies = drivers
            .iter()
            .filter(|driver| driver.atributos.skill >= 60.0 && driver.idade <= 19)
            .count();
        assert!(prodigies >= 2);
    }

    #[test]
    fn test_generate_for_category_skill_within_range() {
        let mut rng = StdRng::seed_from_u64(17);
        let mut existing_names = HashSet::new();
        let drivers =
            Driver::generate_for_category("gt4", 3, "medio", 25, &mut existing_names, &mut rng);

        let range = skill_ranges::get_skill_range_by_tier(3).expect("tier 3 range should exist");
        assert!(drivers.iter().all(|driver| {
            driver.atributos.skill >= range.skill_min as f64
                && driver.atributos.skill <= range.skill_max as f64
        }));
    }

    #[test]
    fn test_create_player_attributes_at_50() {
        let player = Driver::create_player(
            "P001".to_string(),
            "Jogador Teste".to_string(),
            "BR".to_string(),
            20,
        );

        assert!(player.is_jogador);
        assert!(player.personalidade_primaria.is_none());
        assert!(player.personalidade_secundaria.is_none());
        assert_eq!(player.motivacao, 70.0);
        assert!(player
            .atributos
            .entries()
            .into_iter()
            .all(|(_, value)| value == 50.0));
    }
}
