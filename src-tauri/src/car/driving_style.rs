//! Estilo de pilotagem do JOGADOR → modulação de desgaste (puro, testável).
//!
//! Só o JOGADOR passa por aqui (a IA não tem inputs reais). O `race_monitor` alimenta um
//! [`StyleAccumulator`] com os inputs do SDK a cada frame (acelerador, freio, rotação vs
//! redline, marcha, volante, zebra); no fim da corrida ele vira um **fator por peça** que
//! multiplica o desgaste do carro do jogador (por cima de pista × clima).
//!
//! ASSIMÉTRICO, por decisão do usuário: neutro = 1.0 (pilotagem de corrida normal cai aqui e
//! é fácil de manter); **economizar** desgaste dá desconto RESPONSIVO até `MIN_FACTOR` (0.75);
//! **abusar** só passa de 1.0 depois de uma ZONA MORTA e chega a `MAX_FACTOR` (1.30) — fácil
//! ganhar desconto, difícil tomar multa. O desgaste modulado PERSISTE (economizar = peça dura
//! mais = economia real; abusar = espiral). Cada sinal ataca a peça que ele castiga: motor
//! (limitador/short-shift), câmbio (troca no limitador), freios (frenagem forte), suspensão
//! (serrar volante / martelar zebra).
//!
//! ⚠️ Os limiares abaixo são um 1º corte — CALIBRAR com telemetria real na pista.
#![allow(dead_code)] // Consumido no wiring (monitor + economia do import).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::car::PartType;

// ───────────────────────── Fator assimétrico ─────────────────────────

/// Desconto MÁXIMO (economia perfeita) e acréscimo MÁXIMO (abuso total).
const MIN_FACTOR: f64 = 0.75;
const MAX_FACTOR: f64 = 1.30;
/// Zona morta do lado do ABUSO: um `score` de abuso abaixo disto não pune (o normal fica em
/// 1.0). O lado da economia NÃO tem zona morta (desconto responsivo).
const ABUSE_DEAD_ZONE: f64 = 0.35;

/// Mapeia um `score` de estilo assinado (−1 economia máx … 0 normal … +1 abuso máx) no fator.
fn style_factor(score: f64) -> f64 {
    let s = score.clamp(-1.0, 1.0);
    if s < 0.0 {
        // Economia: linear e responsivo (s=−1 → MIN_FACTOR).
        1.0 + s * (1.0 - MIN_FACTOR)
    } else {
        // Abuso: zona morta antes de punir (s≤dead_zone → 1.0; s=1 → MAX_FACTOR).
        let over = ((s - ABUSE_DEAD_ZONE) / (1.0 - ABUSE_DEAD_ZONE)).clamp(0.0, 1.0);
        1.0 + over * (MAX_FACTOR - 1.0)
    }
}

// ───────────────────────── Limiares de detecção (CALIBRAR na pista) ─────────────────────────

/// Fração do redline pra considerar "colado no limitador".
const LIMITER_FRAC: f64 = 0.98;
/// Rotação alta (pra WOT sustentado castigar o motor).
const HIGH_RPM_FRAC: f64 = 0.90;
/// Trocar de marcha ABAIXO disto (fração do redline) = short-shift (economia).
const SHORT_SHIFT_FRAC: f64 = 0.90;
/// Trocar de marcha ACIMA disto = no limitador (abuso de motor/câmbio).
const SHIFT_LIMITER_FRAC: f64 = 0.97;
/// Freio "pisado" (pra medir a fração de frenagem forte só quando há frenagem).
const BRAKE_ACTIVE: f64 = 0.05;
/// Frenagem FORTE (perto da trava).
const HARD_BRAKE: f64 = 0.90;
/// |Δ ângulo do volante| por tick que conta como "serrar" (rad).
const STEER_JERK_RAD: f64 = 0.15;
/// |aceleração vertical| de zebra/impacto (m/s²).
const KERB_G: f64 = 20.0;

// Referências de "normal" e escalas de abuso/economia (frações), por peça.
const MOTOR_LIMITER_NORMAL: f64 = 0.02; // ~2% da corrida encostando no limitador é normal.
const MOTOR_LIMITER_ABUSE: f64 = 0.15; // ~15% = abuso cheio.
const SHORT_SHIFT_FULL: f64 = 0.40; // 40% das trocas curtas = economia cheia.
const GEARBOX_LIMITER_SHIFT_FULL: f64 = 0.50; // metade das trocas no limitador = abuso cheio.
const BRAKE_HARD_NORMAL: f64 = 0.15; // ~15% da frenagem forte é normal.
const BRAKE_HARD_ABUSE: f64 = 0.50; // 50% = abuso cheio.
const SUSP_ABUSE_SUM: f64 = 0.30; // (serra + zebra) somados = abuso cheio.
const SUSP_SMOOTH_REF: f64 = 0.05; // abaixo disto de "serra" = mãos suaves (economia).

// ───────────────────────── Amostra e acumulador ─────────────────────────

/// Um frame de telemetria relevante pro estilo (do SDK, via `race_monitor`).
#[derive(Debug, Clone, Copy)]
pub struct StyleSample {
    /// Acelerador 0..1.
    pub throttle: f64,
    /// Freio 0..1.
    pub brake: f64,
    /// Rotação do motor.
    pub rpm: f64,
    /// Redline do carro (referência). ≤0 = desconhecido → sinais de rotação ignorados no tick.
    pub redline: f64,
    /// Marcha (>0 = engrenada; ≤0 neutro/ré, ignorado pra troca).
    pub gear: i32,
    /// Ângulo do volante (rad).
    pub steering_rad: f64,
    /// Aceleração vertical (m/s²) — zebra/impacto.
    pub vert_accel: f64,
}

/// Fatores de desgaste POR PEÇA prontos do estilo (só as 4 peças com sinal; 1.0 = neutro).
/// `Copy` e serializável — fácil de carregar do monitor até a economia do import.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct StyleFactors {
    pub engine: f64,
    pub gearbox: f64,
    pub brakes: f64,
    pub suspension: f64,
}

impl Default for StyleFactors {
    fn default() -> Self {
        Self {
            engine: 1.0,
            gearbox: 1.0,
            brakes: 1.0,
            suspension: 1.0,
        }
    }
}

impl StyleFactors {
    /// Fator desta peça (1.0 para as peças sem sinal de estilo).
    pub fn factor_for(&self, pt: PartType) -> f64 {
        match pt {
            PartType::Engine => self.engine,
            PartType::Gearbox => self.gearbox,
            PartType::Brakes => self.brakes,
            PartType::Suspension => self.suspension,
            _ => 1.0,
        }
    }

    /// Cautela UNIFORME (mecânica): babá as 4 peças com sinal por igual. Usado quando um time
    /// DESCONFIA do carro (DNF mecânico recente) e poupa as peças — o loop emergente da quebra
    /// (quebrou → desconfia → poupa → quebra menos). `f` < 1.0 = menos desgaste.
    pub fn uniform(f: f64) -> Self {
        Self {
            engine: f,
            gearbox: f,
            brakes: f,
            suspension: f,
        }
    }

    /// Nenhum fator se afasta de 1.0 (nada a aplicar — corrida sem estilo capturado).
    pub fn is_neutral(&self) -> bool {
        [self.engine, self.gearbox, self.brakes, self.suspension]
            .iter()
            .all(|f| (f - 1.0).abs() < 1e-9)
    }
}

/// Acumuladores do estilo ao longo da corrida (somados tick a tick — barato, sem alocação).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StyleAccumulator {
    samples: u64,
    // Motor
    limiter_ticks: u64,  // rpm_frac > LIMITER_FRAC
    wot_high_ticks: u64, // throttle>0.95 && rpm_frac > HIGH_RPM_FRAC
    // Câmbio (upshifts classificados)
    upshifts: u64,
    limiter_shifts: u64, // upshift com rpm_frac > SHIFT_LIMITER_FRAC
    short_shifts: u64,   // upshift com rpm_frac < SHORT_SHIFT_FRAC
    // Freios
    brake_ticks: u64,      // brake > BRAKE_ACTIVE
    hard_brake_ticks: u64, // brake > HARD_BRAKE
    // Suspensão
    steer_jerk_ticks: u64, // |Δsteering| > STEER_JERK_RAD
    kerb_ticks: u64,       // |vert_accel| > KERB_G
    // Estado da última amostra (derivadas).
    last_gear: i32,
    last_steering: f64,
    has_last: bool,
}

impl StyleAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Quantos frames foram ingeridos (0 = sem dado → fatores neutros).
    pub fn samples(&self) -> u64 {
        self.samples
    }

    /// Ingere um frame do SDK.
    pub fn ingest(&mut self, s: StyleSample) {
        self.samples += 1;

        let rpm_frac = if s.redline > 0.0 {
            Some(s.rpm / s.redline)
        } else {
            None
        };

        // Motor: tempo no limitador + WOT em alta rotação.
        if let Some(f) = rpm_frac {
            if f > LIMITER_FRAC {
                self.limiter_ticks += 1;
            }
            if s.throttle > 0.95 && f > HIGH_RPM_FRAC {
                self.wot_high_ticks += 1;
            }
        }

        // Câmbio: classifica UPSHIFTS pela rotação no momento da troca.
        if self.has_last && s.gear > self.last_gear && s.gear > 0 && self.last_gear > 0 {
            self.upshifts += 1;
            if let Some(f) = rpm_frac {
                if f > SHIFT_LIMITER_FRAC {
                    self.limiter_shifts += 1;
                } else if f < SHORT_SHIFT_FRAC {
                    self.short_shifts += 1;
                }
            }
        }

        // Freios: fração de frenagem forte (só quando há frenagem).
        if s.brake > BRAKE_ACTIVE {
            self.brake_ticks += 1;
            if s.brake > HARD_BRAKE {
                self.hard_brake_ticks += 1;
            }
        }

        // Suspensão: serrar o volante + martelar zebra.
        if self.has_last && (s.steering_rad - self.last_steering).abs() > STEER_JERK_RAD {
            self.steer_jerk_ticks += 1;
        }
        if s.vert_accel.abs() > KERB_G {
            self.kerb_ticks += 1;
        }

        self.last_gear = s.gear;
        self.last_steering = s.steering_rad;
        self.has_last = true;
    }

    /// Score assinado (−1 economia … +1 abuso) de cada peça com sinal de estilo. Peça sem
    /// sinal fica fora do mapa (fator neutro no consumidor).
    fn scores(&self) -> HashMap<PartType, f64> {
        let mut out = HashMap::new();
        if self.samples == 0 {
            return out;
        }
        let n = self.samples as f64;
        let frac = |x: u64, d: f64| if d > 0.0 { x as f64 / d } else { 0.0 };
        let norm = |v: f64, lo: f64, hi: f64| ((v - lo) / (hi - lo)).clamp(0.0, 1.0);

        let shifts = self.upshifts as f64;
        let short_shift_frac = frac(self.short_shifts, shifts);
        let economy_shift = (short_shift_frac / SHORT_SHIFT_FULL).clamp(0.0, 1.0);

        // Motor: limitador (abuso) vs short-shift (economia).
        let limiter_frac = frac(self.limiter_ticks, n);
        let motor_abuse = norm(limiter_frac, MOTOR_LIMITER_NORMAL, MOTOR_LIMITER_ABUSE);
        out.insert(
            PartType::Engine,
            (motor_abuse - economy_shift).clamp(-1.0, 1.0),
        );

        // Câmbio: trocas no limitador (abuso) vs short-shift (economia).
        let limiter_shift_frac = frac(self.limiter_shifts, shifts);
        let gearbox_abuse = (limiter_shift_frac / GEARBOX_LIMITER_SHIFT_FULL).clamp(0.0, 1.0);
        out.insert(
            PartType::Gearbox,
            (gearbox_abuse - economy_shift).clamp(-1.0, 1.0),
        );

        // Freios: fração de frenagem forte, centrada no normal (frenagem leve = economia).
        // Sem NENHUMA frenagem no dado (brake_ticks=0) = sem sinal → neutro (não economia).
        if self.brake_ticks > 0 {
            let hard_brake_frac = frac(self.hard_brake_ticks, self.brake_ticks as f64);
            let brakes_abuse = norm(hard_brake_frac, BRAKE_HARD_NORMAL, BRAKE_HARD_ABUSE);
            let brakes_economy = norm(BRAKE_HARD_NORMAL - hard_brake_frac, 0.0, BRAKE_HARD_NORMAL);
            out.insert(
                PartType::Brakes,
                (brakes_abuse - brakes_economy).clamp(-1.0, 1.0),
            );
        }

        // Suspensão: serrar + zebra (abuso) vs mãos suaves (economia).
        let jerk_frac = frac(self.steer_jerk_ticks, n);
        let kerb_frac = frac(self.kerb_ticks, n);
        let susp_abuse = ((jerk_frac + kerb_frac) / SUSP_ABUSE_SUM).clamp(0.0, 1.0);
        let susp_economy = norm(SUSP_SMOOTH_REF - jerk_frac, 0.0, SUSP_SMOOTH_REF);
        out.insert(
            PartType::Suspension,
            (susp_abuse - susp_economy).clamp(-1.0, 1.0),
        );

        out
    }

    /// Fator de desgaste POR PEÇA (0.75–1.30). Peça sem sinal de estilo → 1.0. Sem dado
    /// (nenhum frame) → todos 1.0. É o que o consumidor multiplica no desgaste do jogador.
    pub fn wear_factors(&self) -> HashMap<PartType, f64> {
        self.scores()
            .into_iter()
            .map(|(pt, score)| (pt, style_factor(score)))
            .collect()
    }

    /// Fator de uma peça específica (1.0 se sem sinal). Conveniência pro consumidor.
    pub fn factor_for(&self, pt: PartType) -> f64 {
        self.scores()
            .get(&pt)
            .map(|&s| style_factor(s))
            .unwrap_or(1.0)
    }

    /// Fatores das 4 peças com estilo, na forma `Copy` pra carregar até a economia.
    pub fn factors(&self) -> StyleFactors {
        StyleFactors {
            engine: self.factor_for(PartType::Engine),
            gearbox: self.factor_for(PartType::Gearbox),
            brakes: self.factor_for(PartType::Brakes),
            suspension: self.factor_for(PartType::Suspension),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constrói uma amostra "normal" (rotação média, sem freio forte, volante parado).
    fn cruise() -> StyleSample {
        StyleSample {
            throttle: 0.6,
            brake: 0.0,
            rpm: 6000.0,
            redline: 8000.0, // 0.75 do redline
            gear: 4,
            steering_rad: 0.0,
            vert_accel: 0.0,
        }
    }

    fn run(samples: impl IntoIterator<Item = StyleSample>) -> StyleAccumulator {
        let mut acc = StyleAccumulator::new();
        for s in samples {
            acc.ingest(s);
        }
        acc
    }

    #[test]
    fn mapa_do_fator_e_assimetrico() {
        assert!((style_factor(0.0) - 1.0).abs() < 1e-9, "normal = 1.0");
        assert!(
            (style_factor(-1.0) - MIN_FACTOR).abs() < 1e-9,
            "economia máx = piso"
        );
        assert!(
            (style_factor(1.0) - MAX_FACTOR).abs() < 1e-9,
            "abuso máx = teto"
        );
        // Economia responsiva; abuso com zona morta.
        assert!(style_factor(-0.2) < 1.0, "economia leve já desconta");
        assert!(
            (style_factor(0.2) - 1.0).abs() < 1e-9,
            "abuso leve (na zona morta) não pune"
        );
        assert!(style_factor(0.7) > 1.0, "abuso claro pune");
    }

    #[test]
    fn sem_dado_e_neutro() {
        let acc = StyleAccumulator::new();
        for &pt in &PartType::ALL {
            assert!(
                (acc.factor_for(pt) - 1.0).abs() < 1e-9,
                "{pt:?} sem dado deveria ser 1.0"
            );
        }
    }

    #[test]
    fn pilotagem_normal_fica_perto_de_um() {
        // Pilotagem NORMAL realista: trocas na banda 0.90–0.97, zonas de frenagem com fração
        // típica de frenagem forte (~15%), e volante com movimento moderado (~5% de "serra").
        let mut samples = Vec::new();
        for i in 0..2000 {
            let mut s = cruise();
            if i % 200 == 0 {
                s.rpm = 7400.0; // 0.925 do redline → troca normal
                s.gear = 5;
            }
            if i % 10 < 3 {
                // ~30% da corrida freando; ~1 em 6 dessas é forte (~fração normal).
                s.brake = if i % 20 == 0 { 0.95 } else { 0.5 };
            }
            // Volante muda a cada 20 ticks → ~5% de "serra" (o normal, na zona morta).
            s.steering_rad = if (i / 20) % 2 == 0 { 0.0 } else { 0.3 };
            samples.push(s);
        }
        let acc = run(samples);
        for pt in [
            PartType::Engine,
            PartType::Gearbox,
            PartType::Brakes,
            PartType::Suspension,
        ] {
            let f = acc.factor_for(pt);
            assert!(
                (0.90..=1.10).contains(&f),
                "{pt:?} normal deveria ~1.0, deu {f}"
            );
        }
    }

    #[test]
    fn economizar_desconta_o_motor() {
        // Short-shifting agressivo: sempre troca cedo (~0.80 do redline).
        let mut samples = Vec::new();
        let mut gear = 3;
        for i in 0..2000 {
            let mut s = cruise();
            s.rpm = 5000.0; // 0.625 do redline (baixa)
            if i % 20 == 0 {
                gear += 1;
                if gear > 6 {
                    gear = 3;
                }
                s.rpm = 6300.0; // 0.79 do redline → short-shift
            }
            s.gear = gear;
            samples.push(s);
        }
        let acc = run(samples);
        assert!(
            acc.factor_for(PartType::Engine) < 0.95,
            "short-shift deveria descontar o motor ({})",
            acc.factor_for(PartType::Engine)
        );
        assert!(
            acc.factor_for(PartType::Gearbox) < 0.95,
            "short-shift deveria descontar o câmbio"
        );
    }

    #[test]
    fn abusar_do_limitador_castiga_motor_e_cambio() {
        // Sempre colado no limitador e trocando lá em cima.
        let mut samples = Vec::new();
        let mut gear = 3;
        for i in 0..2000 {
            let mut s = cruise();
            s.throttle = 1.0;
            s.rpm = 7950.0; // 0.994 do redline → no limitador
            if i % 20 == 0 {
                gear += 1;
                if gear > 6 {
                    gear = 3;
                }
                // troca acontece no limitador (rpm alto no tick da troca)
            }
            s.gear = gear;
            samples.push(s);
        }
        let acc = run(samples);
        assert!(
            acc.factor_for(PartType::Engine) > 1.05,
            "limitador deveria castigar o motor ({})",
            acc.factor_for(PartType::Engine)
        );
        assert!(
            acc.factor_for(PartType::Gearbox) > 1.05,
            "trocar no limitador deveria castigar o câmbio"
        );
    }

    #[test]
    fn frenagem_forte_castiga_os_freios() {
        let suave = run((0..2000).map(|_| StyleSample {
            brake: 0.4,
            ..cruise()
        }));
        let bruta = run((0..2000).map(|_| StyleSample {
            brake: 1.0,
            ..cruise()
        }));
        assert!(
            suave.factor_for(PartType::Brakes) < 1.0,
            "frenagem leve deveria descontar"
        );
        assert!(
            bruta.factor_for(PartType::Brakes) > 1.05,
            "frenagem no talo deveria castigar ({})",
            bruta.factor_for(PartType::Brakes)
        );
    }

    #[test]
    fn serrar_o_volante_e_zebra_castigam_a_suspensao() {
        // Alterna o volante bruscamente todo tick + bate zebra.
        let mut samples = Vec::new();
        for i in 0..2000 {
            samples.push(StyleSample {
                steering_rad: if i % 2 == 0 { 0.5 } else { -0.5 }, // Δ=1.0 rad/tick
                vert_accel: 30.0,
                ..cruise()
            });
        }
        let acc = run(samples);
        assert!(
            acc.factor_for(PartType::Suspension) > 1.05,
            "serrar+zebra deveria castigar a suspensão ({})",
            acc.factor_for(PartType::Suspension)
        );
        // Mãos suaves (volante parado, sem zebra) descontam.
        let suave = run((0..2000).map(|_| cruise()));
        assert!(
            suave.factor_for(PartType::Suspension) < 1.0,
            "mãos suaves deveriam descontar"
        );
    }

    #[test]
    fn redline_desconhecido_ignora_rotacao() {
        // Sem redline (≤0): sinais de rotação são ignorados → motor neutro.
        let acc = run((0..1000).map(|_| StyleSample {
            redline: 0.0,
            rpm: 9999.0,
            throttle: 1.0,
            ..cruise()
        }));
        assert!(
            (acc.factor_for(PartType::Engine) - 1.0).abs() < 1e-9,
            "sem redline o motor deveria ficar neutro"
        );
    }
}
