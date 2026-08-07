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
    /// Histórico tem peso 60% (memória); recente tem 40% (calor atual).
    pub fn perceived_intensity(&self) -> f64 {
        perceived_intensity(self.historical_intensity, self.recent_activity)
    }
}

/// Calcula intensidade percebida a partir dos dois eixos (0.0–100.0).
///
/// A MEMÓRIA PESA MAIS QUE O CALOR, e por muito tempo foi o contrário (0.4 histórico
/// + 0.6 recente). A inversão veio de uma observação simples sobre os dois eixos: o
/// recente é o que cai pela metade todo fim de temporada, o histórico é o que acumula.
/// Dar 60% ao eixo volátil fazia a fórmula pesar mais o que ela mesma apaga — uma dupla
/// com doze anos de história e uma temporada quieta lia mais baixo que dois pilotos que
/// se tocaram duas vezes no mês passado. Para uma coisa chamada rivalidade, isso está
/// de costas: rivalidade é justamente o que sobra depois que o calor passa.
///
/// Os limiares semânticos (`rivalry::intensity_level`) e os portões que dependem deles
/// (Nemesis em 40, transbordo para equipe em 60) não mudaram — o que mudou é qual eixo
/// leva a rivalidade até lá.
pub fn perceived_intensity(historical: f64, recent: f64) -> f64 {
    (historical * 0.6 + recent * 0.4).clamp(0.0, 100.0)
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
        // 0.6 * 10 + 0.4 * 20 = 6.0 + 8.0 = 14.0
        let p = perceived_intensity(10.0, 20.0);
        assert!((p - 14.0).abs() < 1e-9);
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
        // h=30, r=20 → perceived = 0.6*30 + 0.4*20 = 18 + 8 = 26 >= 20
        assert_eq!(rivalry_lifecycle(30.0, 20.0), RivalryLifecycle::Viva);
    }

    #[test]
    fn lifecycle_adormecida_por_historical() {
        // r=0, h=10 → perceived = 6 < 20 (nao e Viva); historical >= 10 → Adormecida
        assert_eq!(rivalry_lifecycle(10.0, 0.0), RivalryLifecycle::Adormecida);
    }

    #[test]
    fn lifecycle_adormecida_por_perceived() {
        // h=8, r=0 → perceived = 4.8 < 5; e h < 10 → Extinta
        // h=10, r=2 → perceived = 0.6*10+0.4*2 = 6+0.8 = 6.8 >= 5 → Adormecida
        assert_eq!(rivalry_lifecycle(10.0, 2.0), RivalryLifecycle::Adormecida);
    }

    #[test]
    fn lifecycle_extinta_ambos_baixos() {
        assert_eq!(rivalry_lifecycle(0.0, 0.0), RivalryLifecycle::Extinta);
        // h=5, r=0 → perceived=3 < 5, h < 10 → Extinta
        assert_eq!(rivalry_lifecycle(5.0, 0.0), RivalryLifecycle::Extinta);
    }
}
