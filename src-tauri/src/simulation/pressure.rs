//! Pressão de campeonato → ajuste de performance (clutch/choke). **Lógica pura.**
//!
//! Design (travado com o user). Pressão = INTENSIDADE × DIREÇÃO:
//! - **Intensidade** (o quanto está em jogo): 0 se o piloto não tem chance
//!   matemática de título ou se o título já está decidido. Nas ~5 últimas corridas
//!   sobe conforme acaba; **líder** que abriu vantagem → ×2 (defendendo); **última
//!   corrida** com título aberto → ×3 (pressão imensa, NÃO acumula com o ×2).
//! - **Direção** (ajuda ou atrapalha) pela RESILIÊNCIA = mentalidade + experiência
//!   (experiência vale metade). Resiliente (mental forte / veterano) → **clutch**
//!   (mais rápido, erra menos). Frágil (mental fraco / jovem) → **choke**.
//!
//! Mesma fonte na simulação offline E no export pro iRacing.

use serde::{Deserialize, Serialize};

/// Ponto neutro da resiliência: acima vira clutch, abaixo vira choke.
const NEUTRAL: f64 = 0.55;
/// Tamanho do swing de RITMO (pontos de skill) na intensidade máxima.
const PACE_K: f64 = 3.0;
/// Tamanho do swing na taxa de ERRO.
const ERROR_K: f64 = 0.15;
/// A partir de quantas corridas restantes a pressão começa.
const PRESSURE_WINDOW: u32 = 5;
/// Perseguidor (2º/3º com chance) tem o clutch um pouco mais fácil — baixa o neutro.
const CHASER_NEUTRAL_SHIFT: f64 = 0.07;

/// Situação do piloto na luta pelo título (da matemática do campeonato).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct TitleContext {
    /// Tem chance matemática de título (alcança o líder no que falta).
    pub in_contention: bool,
    /// É o líder SOLITÁRIO (abriu vantagem sobre todos).
    pub is_leader: bool,
    /// Título já decidido (o 2º não alcança o líder nem ganhando tudo).
    pub title_decided: bool,
}

/// Resiliência à pressão (0–1): mentalidade + experiência (experiência vale metade).
pub fn pressure_resilience(mentalidade: f64, experiencia: f64) -> f64 {
    ((mentalidade * 2.0 + experiencia) / 3.0).clamp(0.0, 100.0) / 100.0
}

/// Deriva a situação de título de um piloto a partir dos pontos do grid.
/// `all_points` = pontos de TODOS (incluindo ele); `max_points_per_race` = pontos
/// do vencedor; `races_left` = corridas restantes (incluindo a atual).
pub fn title_context(
    my_points: f64,
    all_points: &[f64],
    races_left: u32,
    max_points_per_race: f64,
) -> TitleContext {
    let max_gain = races_left as f64 * max_points_per_race;
    let leader = all_points.iter().cloned().fold(f64::MIN, f64::max);
    let second = all_points
        .iter()
        .cloned()
        .filter(|&p| p < leader - 1e-6)
        .fold(f64::MIN, f64::max);

    let in_contention = my_points + max_gain >= leader - 1e-6;
    // Líder solitário = ninguém tem pontos ≥ os meus além de mim (abri vantagem).
    let count_top = all_points
        .iter()
        .filter(|&&p| p >= my_points - 1e-6)
        .count();
    let is_leader = count_top == 1;
    let title_decided =
        races_left == 0 || (second != f64::MIN && second + max_gain < leader - 1e-6);

    TitleContext {
        in_contention,
        is_leader,
        title_decided,
    }
}

/// Intensidade da pressão (0 = nenhuma). Ver regras no topo do módulo.
pub fn pressure_intensity(ctx: &TitleContext, races_left: u32) -> f64 {
    if !ctx.in_contention || ctx.title_decided {
        return 0.0;
    }
    if races_left <= 1 {
        return 3.0; // última corrida = pressão imensa (não acumula com o ×2 do líder)
    }
    if races_left > PRESSURE_WINDOW {
        return 0.0; // ainda longe do fim
    }
    // 2..5 corridas: sobe conforme acaba; líder defende = ×2.
    let ramp = (PRESSURE_WINDOW as f64 + 1.0 - races_left as f64) / (PRESSURE_WINDOW as f64 - 1.0);
    ramp * if ctx.is_leader { 2.0 } else { 1.0 }
}

/// O efeito da pressão na performance.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PressureEffect {
    /// Pontos de skill a SOMAR (+ clutch, − choke).
    pub pace_delta: f64,
    /// Multiplicador da taxa de erro (<1 clutch, >1 choke).
    pub error_mult: f64,
}

impl PressureEffect {
    pub const NONE: PressureEffect = PressureEffect {
        pace_delta: 0.0,
        error_mult: 1.0,
    };
}

/// Combina intensidade + resiliência no efeito final. `is_chaser` = briga de baixo
/// (2º/3º com chance), que tem o clutch um pouco mais fácil que o líder.
pub fn pressure_effect(intensity: f64, resilience: f64, is_chaser: bool) -> PressureEffect {
    if intensity <= 0.0 {
        return PressureEffect::NONE;
    }
    let neutral = if is_chaser {
        NEUTRAL - CHASER_NEUTRAL_SHIFT
    } else {
        NEUTRAL
    };
    let dir = resilience - neutral; // + clutch, − choke
    PressureEffect {
        pace_delta: intensity * dir * PACE_K,
        error_mult: (1.0 - intensity * dir * ERROR_K).clamp(0.5, 2.0),
    }
}

/// Atalho: do contexto + atributos direto ao efeito.
pub fn pressure_for(
    ctx: &TitleContext,
    races_left: u32,
    mentalidade: f64,
    experiencia: f64,
) -> PressureEffect {
    let intensity = pressure_intensity(ctx, races_left);
    let is_chaser = ctx.in_contention && !ctx.is_leader;
    pressure_effect(
        intensity,
        pressure_resilience(mentalidade, experiencia),
        is_chaser,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experiencia_vale_metade() {
        // Mentalidade 100 sozinha → 0.667; experiência 100 sozinha → 0.333 (metade).
        assert!((pressure_resilience(100.0, 0.0) - 2.0 / 3.0).abs() < 1e-9);
        assert!((pressure_resilience(0.0, 100.0) - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn fora_de_briga_sem_pressao() {
        // 30 pts, líder 90, falta 1 corrida (25 max) → não alcança.
        let ctx = title_context(30.0, &[90.0, 60.0, 30.0], 1, 25.0);
        assert!(!ctx.in_contention);
        assert_eq!(pressure_intensity(&ctx, 1), 0.0);
    }

    #[test]
    fn titulo_decidido_sem_pressao() {
        // Líder 100, 2º 70, falta 1 (25) → 70+25 < 100 → decidido.
        let ctx = title_context(100.0, &[100.0, 70.0, 50.0], 1, 25.0);
        assert!(ctx.title_decided);
        assert_eq!(pressure_intensity(&ctx, 1), 0.0);
    }

    #[test]
    fn ultima_corrida_em_aberto_intensidade_3() {
        // Líder 100, 2º 85, falta 1 (25) → 85+25=110 ≥ 100 → aberto.
        let ctx = title_context(100.0, &[100.0, 85.0], 1, 25.0);
        assert!(ctx.in_contention && !ctx.title_decided);
        assert_eq!(pressure_intensity(&ctx, 1), 3.0);
    }

    #[test]
    fn lider_no_meio_dobra() {
        // Líder solitário, 3 corridas restantes, em aberto.
        let ctx = title_context(100.0, &[100.0, 90.0], 3, 25.0);
        assert!(ctx.is_leader && ctx.in_contention && !ctx.title_decided);
        let solo = title_context(90.0, &[100.0, 90.0], 3, 25.0); // o 2º (não líder)
        let i_leader = pressure_intensity(&ctx, 3);
        let i_chaser = pressure_intensity(&solo, 3);
        assert!(
            (i_leader - 2.0 * i_chaser).abs() < 1e-9,
            "{i_leader} vs {i_chaser}"
        );
    }

    #[test]
    fn clutch_vs_choke() {
        // Mesma intensidade: resiliente vira clutch (+ritmo, −erro), frágil choke.
        let clutch = pressure_effect(3.0, 1.0, false); // resiliência máxima
        let choke = pressure_effect(3.0, 0.1, false); // frágil
        assert!(
            clutch.pace_delta > 0.0 && clutch.error_mult < 1.0,
            "{clutch:?}"
        );
        assert!(
            choke.pace_delta < 0.0 && choke.error_mult > 1.0,
            "{choke:?}"
        );
    }

    #[test]
    fn mental_forte_quase_neutro() {
        // Líder (×2) com mental forte fica perto do neutro (resiliência ~NEUTRAL).
        let res = pressure_resilience(82.0, 50.0); // ~0.71
        let eff = pressure_effect(2.0, res, false);
        assert!(eff.pace_delta.abs() < 1.5, "deveria ser leve: {eff:?}");
        assert!(
            eff.pace_delta > 0.0,
            "mental forte = leve clutch, não choke"
        );
    }

    #[test]
    fn perseguidor_clutch_mais_facil() {
        // Mesma resiliência logo abaixo do neutro do líder: líder dá choke leve,
        // perseguidor (neutro mais baixo) já vira clutch leve.
        let res = 0.50;
        let lider = pressure_effect(3.0, res, false);
        let chaser = pressure_effect(3.0, res, true);
        assert!(
            lider.pace_delta < 0.0,
            "líder em 0.50 = leve choke: {lider:?}"
        );
        assert!(
            chaser.pace_delta > 0.0,
            "perseguidor em 0.50 = leve clutch: {chaser:?}"
        );
        assert!(chaser.pace_delta > lider.pace_delta);
    }
}
