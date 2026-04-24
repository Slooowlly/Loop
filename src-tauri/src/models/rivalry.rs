#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ── Tipo de origem da rivalidade ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RivalryType {
    Colisao,
    Companheiros,
    Campeonato,
    Pista,
}

impl RivalryType {
    pub fn as_str(&self) -> &str {
        match self {
            RivalryType::Colisao => "Colisao",
            RivalryType::Companheiros => "Companheiros",
            RivalryType::Campeonato => "Campeonato",
            RivalryType::Pista => "Pista",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Colisao" => RivalryType::Colisao,
            "Companheiros" => RivalryType::Companheiros,
            "Campeonato" => RivalryType::Campeonato,
            _ => RivalryType::Pista,
        }
    }
}

// ── Model de domínio ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rivalry {
    pub id: String,
    /// ID sempre ordenado: piloto1_id < piloto2_id (string)
    pub piloto1_id: String,
    pub piloto2_id: String,
    /// Peso acumulado ao longo da história — decai lentamente entre temporadas (0.0–100.0)
    pub historical_intensity: f64,
    /// Calor recente — aquece rápido, esfria com mais força entre temporadas (0.0–100.0)
    pub recent_activity: f64,
    pub tipo: RivalryType,
    pub criado_em: String,
    pub ultima_atualizacao: String,
    /// Número da temporada do último reforço — usado para decidir decaimento
    pub temporada_update: i32,
}

impl Rivalry {
    /// Intensidade percebida: combinação ponderada dos dois eixos.
    /// Recente tem peso 60% (atividade atual); histórico tem 40% (memória).
    pub fn perceived_intensity(&self) -> f64 {
        perceived_intensity(self.historical_intensity, self.recent_activity)
    }
}

/// Calcula intensidade percebida a partir dos dois eixos (0.0–100.0).
pub fn perceived_intensity(historical: f64, recent: f64) -> f64 {
    (historical * 0.4 + recent * 0.6).clamp(0.0, 100.0)
}

// ── Ciclo de vida ─────────────────────────────────────────────────────────────

/// Estado narrativo de uma rivalidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RivalryLifecycle {
    /// Atividade recente relevante ou percebida alta — aparece em notícias e fichas.
    Viva,
    /// Memória histórica presente mas calor recente baixo — "velhos rivais".
    Adormecida,
    /// Ambos os eixos muito baixos — pronta para remoção do banco.
    Extinta,
}

/// Classifica o ciclo de vida de uma rivalidade pelos dois eixos.
pub fn rivalry_lifecycle(historical: f64, recent: f64) -> RivalryLifecycle {
    let perceived = perceived_intensity(historical, recent);
    if recent >= 15.0 || perceived >= 20.0 {
        RivalryLifecycle::Viva
    } else if historical >= 10.0 || perceived >= 5.0 {
        RivalryLifecycle::Adormecida
    } else {
        RivalryLifecycle::Extinta
    }
}

// ── Normalização do par ───────────────────────────────────────────────────────

/// Par de IDs sempre com o menor em `piloto1_id`.
/// Retorna `None` se os dois IDs forem iguais.
pub struct NormalizedPair {
    pub piloto1_id: String,
    pub piloto2_id: String,
}

pub fn normalize_pair(a: &str, b: &str) -> Option<NormalizedPair> {
    if a == b {
        return None;
    }
    if a < b {
        Some(NormalizedPair {
            piloto1_id: a.to_string(),
            piloto2_id: b.to_string(),
        })
    } else {
        Some(NormalizedPair {
            piloto1_id: b.to_string(),
            piloto2_id: a.to_string(),
        })
    }
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_ordena_menor_primeiro() {
        let p = normalize_pair("P020", "P003").unwrap();
        assert_eq!(p.piloto1_id, "P003");
        assert_eq!(p.piloto2_id, "P020");
    }

    #[test]
    fn normalize_ja_ordenado_nao_inverte() {
        let p = normalize_pair("P003", "P020").unwrap();
        assert_eq!(p.piloto1_id, "P003");
        assert_eq!(p.piloto2_id, "P020");
    }

    #[test]
    fn normalize_mesmo_piloto_retorna_none() {
        assert!(normalize_pair("P010", "P010").is_none());
    }

    #[test]
    fn rivalry_type_roundtrip() {
        for t in [
            RivalryType::Colisao,
            RivalryType::Companheiros,
            RivalryType::Campeonato,
            RivalryType::Pista,
        ] {
            assert_eq!(RivalryType::from_str(t.as_str()), t);
        }
    }

    #[test]
    fn perceived_intensity_formula() {
        // 0.4 * 10 + 0.6 * 20 = 4.0 + 12.0 = 16.0
        let p = perceived_intensity(10.0, 20.0);
        assert!((p - 16.0).abs() < 1e-9);
    }

    #[test]
    fn perceived_intensity_clamp() {
        assert!((perceived_intensity(100.0, 100.0) - 100.0).abs() < 1e-9);
        assert!(perceived_intensity(0.0, 0.0).abs() < 1e-9);
    }

    #[test]
    fn lifecycle_viva_por_recent() {
        assert_eq!(rivalry_lifecycle(0.0, 15.0), RivalryLifecycle::Viva);
    }

    #[test]
    fn lifecycle_viva_por_perceived() {
        // h=30, r=20 → perceived = 0.4*30 + 0.6*20 = 12 + 12 = 24 >= 20
        assert_eq!(rivalry_lifecycle(30.0, 20.0), RivalryLifecycle::Viva);
    }

    #[test]
    fn lifecycle_adormecida_por_historical() {
        // r=0, h=10 → perceived = 4 < 5 mas historical >= 10
        assert_eq!(rivalry_lifecycle(10.0, 0.0), RivalryLifecycle::Adormecida);
    }

    #[test]
    fn lifecycle_adormecida_por_perceived() {
        // h=8, r=0 → perceived = 3.2 < 5; mas h < 10. → Extinta
        // h=10, r=2 → perceived = 0.4*10+0.6*2 = 4+1.2 = 5.2 >= 5 → Adormecida
        assert_eq!(rivalry_lifecycle(10.0, 2.0), RivalryLifecycle::Adormecida);
    }

    #[test]
    fn lifecycle_extinta_ambos_baixos() {
        assert_eq!(rivalry_lifecycle(0.0, 0.0), RivalryLifecycle::Extinta);
        // h=5, r=0 → perceived=2 < 5, h < 10 → Extinta
        assert_eq!(rivalry_lifecycle(5.0, 0.0), RivalryLifecycle::Extinta);
    }
}
