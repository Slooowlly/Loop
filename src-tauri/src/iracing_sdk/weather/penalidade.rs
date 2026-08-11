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
    /// Âncoras `(fator_chuva, penalidade)` que definem a curva da penalidade, em ordem de
    /// fator. Interpolação LINEAR POR PARTES entre elas. Os de chuva forte têm um joelho em
    /// fator 90 (o "ás da chuva" só vira resiliente lá no topo).
    ///
    /// **A curva ANCORA POR BAIXO.** O problema real da chuva no iRacing é que a IA não
    /// erra: ela repete o mesmo tempo volta após volta e, quando vem atrás, põe uma pressão
    /// que o humano não sustenta na pista molhada. O jogador tira o pé para não rodar, a IA
    /// não. Então o debuff GERAL é o grosso da punição: ele é o que faz a IA andar com
    /// cuidado e é o que JUSTIFICA ela não errar. Quem é bom de chuva sobe um pouco a partir
    /// desse fundo, sem escapar dele.
    ///
    /// Por isso o topo de cada curva (fator 0) manteve os números originais e o fundo (fator
    /// 100) subiu muito: o pior caso não piorou, e o ás da chuva deixou de correr quase de
    /// graça. Antes o melhor de chuva levava 8 a 14 pontos numa prova molhada, quase nada;
    /// agora leva de 13 a 30, e a diferença entre ele e o pior do grid ficou em 5 a 10
    /// pontos. A punição do pelotão passou a dominar a diferenciação, que é o que se quer.
    ///
    /// A severidade certa MUDA COM A PISTA, e isso ainda não tem alavanca no export.
    fn anchors(self) -> &'static [(f64, f64)] {
        match self {
            RainIntensity::None => &[(0.0, 0.0), (100.0, 0.0)],
            RainIntensity::Light => &[(0.0, 18.0), (100.0, 13.0)],
            RainIntensity::Decent => &[(0.0, 30.0), (100.0, 22.0)],
            // Forte: intermediária (interpolada — user só fixou decente e muito forte).
            RainIntensity::Heavy => &[(0.0, 35.0), (90.0, 27.0), (100.0, 25.0)],
            RainIntensity::VeryHeavy => &[(0.0, 40.0), (90.0, 32.0), (100.0, 30.0)],
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
