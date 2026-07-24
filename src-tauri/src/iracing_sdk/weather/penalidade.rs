//! Penalidade de skill da IA na chuva: intensidade + curva por `fator_chuva`.

use serde::{Deserialize, Serialize};

/// Intensidade da chuva na corrida (caráter geral, mesmo que varie no tempo).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RainIntensity {
    /// Seco — sem penalidade.
    None,
    /// Garoa/leve.
    Light,
    /// Chuva "decente" (a referência de 30→8).
    Decent,
    /// Chuva forte.
    Heavy,
    /// Chuva MUITO forte — penaliza até os bons (40→14).
    VeryHeavy,
}

impl RainIntensity {
    /// Âncoras `(fator_chuva, penalidade)` que definem a curva da penalidade, em
    /// ordem de fator. Interpolação LINEAR POR PARTES entre elas (assim cravamos os
    /// números que o user deu, mesmo quando não caem numa reta). Os de chuva forte
    /// têm um joelho em fator 90 (o "ás da chuva" só vira resiliente lá no topo).
    fn anchors(self) -> &'static [(f64, f64)] {
        match self {
            RainIntensity::None => &[(0.0, 0.0), (100.0, 0.0)],
            RainIntensity::Light => &[(0.0, 18.0), (100.0, 5.0)],
            // Decente: 0→30, 90→10, 100→8 (reta — os três são colineares).
            RainIntensity::Decent => &[(0.0, 30.0), (100.0, 8.0)],
            // Forte: intermediária (interpolada — user só fixou decente e muito forte).
            RainIntensity::Heavy => &[(0.0, 35.0), (90.0, 15.0), (100.0, 11.0)],
            // Muito forte: 0→40, 90→20, 100→14 (pontos exatos do user; curva).
            RainIntensity::VeryHeavy => &[(0.0, 40.0), (90.0, 20.0), (100.0, 14.0)],
        }
    }
}

/// Interpolação linear por partes de `y` num conjunto de âncoras `(x, y)` ordenado.
fn interp(anchors: &[(f64, f64)], x: f64) -> f64 {
    if x <= anchors[0].0 {
        return anchors[0].1;
    }
    for w in anchors.windows(2) {
        let (x0, y0) = w[0];
        let (x1, y1) = w[1];
        if x <= x1 {
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        }
    }
    anchors[anchors.len() - 1].1
}

/// Penalidade de skill (pontos a SUBTRAIR) de um piloto numa corrida molhada, dada
/// a habilidade dele na chuva (`fator_chuva` 0–100) e a intensidade. Interpola entre
/// as âncoras da intensidade. Seco → 0.
pub fn rain_skill_penalty(fator_chuva: f64, intensity: RainIntensity) -> i64 {
    let f = fator_chuva.clamp(0.0, 100.0);
    interp(intensity.anchors(), f).round() as i64
}
