//! Cérebro do Sistema de Quebra — puro, determinístico-com-sorte, testável.
//!
//! Dado o carro (com o desgaste que cada peça carrega da economia), o nº de voltas da
//! corrida, a semente e a qualidade do pit crew da equipe, faz o **pré-roll** da corrida
//! inteira: simula o desgaste volta a volta, joga a sorte na janela de perigo (95%→105%),
//! e devolve os EVENTOS de quebra (qual peça, em que volta, severidade, e quantos segundos
//! de penalidade ou DNF). O mesmo pré-roll alimenta o disparo ao vivo (`!black`/`!dq` na
//! volta-alvo) E o aviso pré-corrida. Não toca DB, SDK nem economia — é wiring da Fase 3.
//!
//! **Cada peça** define sua chance de leve/grave/DNF (§11) E as faixas de tempo de conserto
//! de leve e de grave (câmbio demora mais que uma asa); a **qualidade do pit crew** modula
//! esse tempo (equipe boa perde menos). Params calibrados (Rota B): ver
//! `docs/superpowers/specs/2026-07-18-car-breakdown-system.md` §3, §4, §7, §11.
#![allow(dead_code)] // Fase 2: cérebro puro; o wiring ao vivo (disparo + aviso) vem na Fase 3.

use serde::Serialize;

use crate::car::wear::wear_per_lap;
use crate::car::{Car, PartType};

// ───────────────────────── Parâmetros calibrados (Rota B) ─────────────────────────

/// Desgaste em que a janela de perigo ABRE. Abaixo disto a peça é confiável (risco 0).
const RISK_OPEN: f64 = 0.95;
/// A PAREDE: ao atingir/passar, a peça acabou (falha forçada). O carro aguenta até aqui.
const HARD_WALL: f64 = 1.05;
/// Risco por volta na borda de baixo da janela (em 95%).
const HAZARD_OPEN: f64 = 0.05;
/// Risco por volta perto da parede (em 105%). Intenso o bastante pra a SORTE matar cedo.
const HAZARD_WALL: f64 = 0.28;
/// Ruído de sorte no desgaste por volta (±fração): volta "puxada" gasta mais.
const WEAR_NOISE: f64 = 0.30;
/// Botão global da taxa do grid (análogo ao `IRACER_SALARY_SHARE`).
const GLOBAL: f64 = 1.0;

/// Numa manutenção em box (enduro), troca a peça acima deste desgaste. Ver [`roll_race_breakdowns`].
const SERVICE_WEAR_FLOOR: f64 = 0.60;

/// Canais da RNG determinística (descorrelaciona as rolagens do mesmo `(peça, volta)`).
const CH_NOISE: u64 = 1;
const CH_HAZARD: u64 = 2;
const CH_SEVERITY: u64 = 3;
const CH_TIME: u64 = 4;

// ───────────────────────── Tipos de saída ─────────────────────────

/// Gravidade da quebra → o comando de admin que ela vira no iRacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Severity {
    /// Penalidade curta no box (`!black`), tempo próprio da peça.
    Light,
    /// Penalidade longa no box (`!black`), tempo próprio da peça.
    Heavy,
    /// Retirada (`!dq`) — encerra a corrida do carro.
    Dnf,
}

/// Um evento de quebra pré-rolado para um carro numa corrida.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BreakdownEvent {
    /// Peça culpada (nome legível via [`PartType::display_name`] na narrativa).
    pub part: PartType,
    /// Volta em que a peça larga (1-based). O disparo ao vivo acontece aqui.
    pub lap: u32,
    /// Gravidade da quebra.
    pub severity: Severity,
    /// Segundos de penalidade no box (`!black`); `None` = DNF (`!dq`).
    pub penalty_secs: Option<u32>,
    /// Desgaste da peça ao LARGAR a corrida (pra narrativa/aviso).
    pub entered_wear: f64,
    /// Desgaste no momento da falha (sempre ≥ 95%).
    pub wear_at_fail: f64,
    /// `true` se foi na parede (105%), `false` se foi por sorte na janela.
    pub forced: bool,
}

impl BreakdownEvent {
    /// A quebra tira o carro da corrida?
    pub fn is_dnf(&self) -> bool {
        self.severity == Severity::Dnf
    }

    /// O comando de admin do iRacing para este evento, dado o nº do carro na sessão.
    pub fn command(&self, car_number: u32) -> String {
        match self.penalty_secs {
            Some(secs) => format!("!black #{car_number} {secs}"),
            None => format!("!dq #{car_number}"),
        }
    }
}

// ───────────────────────── RNG determinística (splitmix64) ─────────────────────────

/// Finalizador splitmix64 → f64 uniforme em [0, 1). Puro, sem estado, reproduzível.
fn hash_to_unit(mut x: u64) -> f64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 11) as f64 / ((1u64 << 53) as f64)
}

/// Rolagem determinística em [0,1) para `(semente, peça, volta, canal)`.
fn roll(seed: u64, part_idx: usize, lap: u32, channel: u64) -> f64 {
    const P: u64 = 0x0000_0100_0000_01B3; // FNV-ish
    let mut x = seed;
    x = x.wrapping_mul(P).wrapping_add(part_idx as u64 + 1);
    x = x.wrapping_mul(P).wrapping_add(lap as u64 + 1);
    x = x.wrapping_mul(P).wrapping_add(channel);
    hash_to_unit(x)
}

// ───────────────────────── Modelo de risco (quando quebra) ─────────────────────────

/// Fragilidade relativa: peça de vida curta falha mais (∝ 1/durabilidade, normalizada à
/// mais durável = 0.5). Motor/câmbio/asas/freios/suspensão (vida 3) = 1.0; eletrônica = 0.5.
fn fragility(pt: PartType) -> f64 {
    (3.0 / pt.durability() as f64).clamp(0.5, 1.0)
}

/// Risco POR VOLTA de a peça quebrar, dado o desgaste dentro da janela [95%, 105%].
fn per_lap_hazard(pt: PartType, wear: f64) -> f64 {
    if wear < RISK_OPEN {
        return 0.0;
    }
    let t = ((wear - RISK_OPEN) / (HARD_WALL - RISK_OPEN)).clamp(0.0, 1.0);
    let base = HAZARD_OPEN + (HAZARD_WALL - HAZARD_OPEN) * t;
    (base * fragility(pt) * GLOBAL).clamp(0.0, 1.0)
}

// ───────────────────────── Influência da pista (qual peça sofre) ─────────────────────────

/// Força "média" do efeito da pista (~±35% nas pistas mais peaked). Calibrável.
const TRACK_STRESS_K: f64 = 1.4;

/// Alinhamento (centrado) da peça com a demanda PHA da pista: positivo = a peça puxa PARA o
/// atributo que a pista cobra (é estressada ali); negativo = puxa pra longe. Ambos os vetores
/// em frações centradas em 1/3, então pista equilibrada → ~0 para todas as peças.
fn track_alignment(pt: PartType, track_pha: (f64, f64, f64)) -> f64 {
    let dir = |(p, h, a): (f64, f64, f64)| {
        let t = p + h + a;
        if t > 0.0 {
            (p / t, h / t, a / t)
        } else {
            (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0)
        }
    };
    let (pp, ph, pa) = dir(pt.pha_per_level());
    let (dp, dh, da) = dir(track_pha);
    let t = 1.0 / 3.0;
    (pp - t) * (dp - t) + (ph - t) * (dh - t) + (pa - t) * (da - t)
}

/// Multiplicador de desgaste POR VOLTA da peça nesta pista, **centrado em ~1.0** (subtraindo
/// o `mean_align` das 11 peças) pra não inflar a taxa total do grid: peça alinhada com a
/// pista gasta mais (chega na zona de perigo antes), a contrária gasta menos. Clampado pra
/// não explodir em pistas extremas. É assim que a pista decide QUAL peça tende a quebrar.
fn track_wear_mult(pt: PartType, track_pha: (f64, f64, f64), mean_align: f64) -> f64 {
    let a = track_alignment(pt, track_pha) - mean_align;
    (1.0 + TRACK_STRESS_K * a).clamp(0.6, 1.5)
}

// ───────────────────────── Influência do CLIMA (chuva/calor) ─────────────────────────

/// Chuva: +% de risco na ELETRÔNICA (curto/sensores). Cheio no aguaceiro (wetness=1).
const RAIN_ELEC_STRESS: f64 = 0.60;
/// Chuva ALIVIA a térmica: −% no motor/arrefecimento (o carro esfria).
const RAIN_THERMAL_RELIEF: f64 = 0.25;
/// Temperatura base (neutra) e de "forno" (efeito de calor cheio), em °C.
const HEAT_BASE_C: f64 = 25.0;
const HEAT_FULL_C: f64 = 42.0;
/// Calor: +% no motor/arrefecimento no forno. **INFLA o total** (escolha A) — dia quente
/// quebra mais motores; nada é aliviado. É o único tempero que muda a taxa total do grid.
const HEAT_THERMAL_STRESS: f64 = 0.40;

/// Intensidade do calor: 0 (≤ base) → 1 (≥ forno).
fn heat_factor(temperature: f64) -> f64 {
    ((temperature - HEAT_BASE_C) / (HEAT_FULL_C - HEAT_BASE_C)).clamp(0.0, 1.0)
}

/// Multiplicador de desgaste por volta pelo CLIMA. Chuva redistribui (eletrônica ↑; motor/
/// arrefecimento ↓ pelo resfriamento → ~neutro no total). Calor AGRAVA a térmica sem aliviar
/// ninguém → infla o total. As demais peças não sentem o clima.
fn weather_wear_mult(pt: PartType, wetness: f64, temperature: f64) -> f64 {
    let w = wetness.clamp(0.0, 1.0);
    let heat = heat_factor(temperature);
    match pt {
        PartType::Electronics => 1.0 + w * RAIN_ELEC_STRESS,
        PartType::Engine | PartType::Cooling => {
            (1.0 - w * RAIN_THERMAL_RELIEF) * (1.0 + heat * HEAT_THERMAL_STRESS)
        }
        _ => 1.0,
    }
}

// ───────────────────────── Proteção do JOGADOR (só o jogador) ─────────────────────────

/// Alívio MÁXIMO de desgaste de entrada do carro do jogador (no time mais fraco possível).
/// Botão da proteção — a IA NUNCA recebe isto. Primeiro corte: com `0.05`, num teste de
/// frota pobre, o jogador quebra ~8.6%/corrida vs ~15.8% do poor-AI (≈metade). O número
/// FINAL se calibra contra o desgaste REAL de time pobre no wiring (frota sintética é
/// hipersensível à distribuição de entrada).
const PLAYER_MAX_RELIEF: f64 = 0.05;

/// Fração de alívio no desgaste de entrada do carro do JOGADOR — a equipe dele cuida melhor
/// do carro. Escala com a FRAQUEZA do time (via `pit_crew_quality` 0-100): time forte
/// (crew alto) → ~0 (chances IDÊNTICAS à IA); time pobre (crew baixo) → alívio maior. É
/// "via manutenção", não desconto mágico no risco. Só o jogador chama isto.
fn player_wear_relief(pit_crew_quality: f64) -> f64 {
    let weakness = 1.0 - pit_crew_quality.clamp(0.0, 100.0) / 100.0;
    PLAYER_MAX_RELIEF * weakness
}

/// Cópia do carro do JOGADOR com o desgaste aliviado pela proteção (ver [`player_wear_relief`]).
/// O wiring aplica isto ANTES do pré-roll, só para o carro do jogador.
pub fn player_protected_car(car: &Car, pit_crew_quality: f64) -> Car {
    let keep = 1.0 - player_wear_relief(pit_crew_quality);
    let mut c = car.clone();
    for p in c.parts.iter_mut() {
        p.wear *= keep;
    }
    c
}

// ───────────────────────── Consequência: severidade por peça ─────────────────────────

/// Distribuição de severidade por peça: `(prob. leve, prob. grave)`; o resto é DNF.
/// **Percentuais aprovados** — estrutural/mecânica tira você da corrida; aero/eletrônica
/// quase sempre é só penalidade. Ver spec §11.
fn severity_weights(pt: PartType) -> (f64, f64) {
    match pt {
        PartType::Engine => (0.20, 0.42),      // 0.38 DNF
        PartType::Gearbox => (0.22, 0.44),     // 0.34
        PartType::Suspension => (0.28, 0.47),  // 0.25
        PartType::Chassis => (0.25, 0.45),     // 0.30
        PartType::Brakes => (0.45, 0.45),      // 0.10
        PartType::Cooling => (0.50, 0.42),     // 0.08
        PartType::FrontWing => (0.60, 0.35),   // 0.05
        PartType::RearWing => (0.60, 0.35),    // 0.05
        PartType::Underbody => (0.72, 0.25),   // 0.03
        PartType::Electronics => (0.72, 0.25), // 0.03
        PartType::Sidepods => (0.78, 0.20),    // 0.02
    }
}

/// Sorteia a severidade. Falha FORÇADA na parede (105%) sobe um degrau (a peça foi ao limite).
fn sample_severity(pt: PartType, forced: bool, r: f64) -> Severity {
    let (light, heavy) = severity_weights(pt);
    let base = if r < light {
        Severity::Light
    } else if r < light + heavy {
        Severity::Heavy
    } else {
        Severity::Dnf
    };
    if !forced {
        return base;
    }
    match base {
        Severity::Light => Severity::Heavy,
        Severity::Heavy => Severity::Dnf,
        Severity::Dnf => Severity::Dnf,
    }
}

// ───────────────────────── Consequência: tempo por peça × pit crew ─────────────────────────

/// Faixa de tempo de conserto (segundos), **por peça e por severidade** — condizente com o
/// tamanho do serviço: câmbio/motor demoram; asa/eletrônica são rápidos; grave > leve. Ainda
/// SEM o efeito do pit crew (aplicado em [`repair_secs`]). Não usada para DNF.
fn repair_secs_range(pt: PartType, sev: Severity) -> (u32, u32) {
    match sev {
        Severity::Light => match pt {
            PartType::Gearbox => (6, 9),
            PartType::Engine => (6, 9),
            PartType::Chassis => (5, 8),
            PartType::Suspension => (5, 8),
            PartType::Cooling => (4, 7),
            PartType::Brakes => (3, 6),
            PartType::RearWing => (3, 5),
            PartType::FrontWing => (2, 5),
            PartType::Underbody => (2, 4),
            PartType::Sidepods => (2, 4),
            PartType::Electronics => (2, 3),
        },
        Severity::Heavy => match pt {
            PartType::Gearbox => (14, 20),
            PartType::Engine => (13, 19),
            PartType::Chassis => (11, 17),
            PartType::Suspension => (10, 15),
            PartType::Cooling => (9, 13),
            PartType::Brakes => (7, 11),
            PartType::RearWing => (6, 9),
            PartType::FrontWing => (5, 8),
            PartType::Underbody => (5, 8),
            PartType::Sidepods => (4, 6),
            PartType::Electronics => (3, 5),
        },
        Severity::Dnf => (0, 0), // não usado
    }
}

/// Fator de tempo do pit pela qualidade da equipe (0-100). Boa equipe perde menos tempo:
/// qualidade 0 → 1.20× (lenta), 50 → 1.00× (neutra), 100 → 0.80× (rápida).
fn pit_time_factor(pit_crew_quality: f64) -> f64 {
    let q = pit_crew_quality.clamp(0.0, 100.0);
    1.20 - 0.40 * (q / 100.0)
}

/// Segundos de conserto: faixa da peça/severidade (com sorte) escalada pela qualidade do pit.
fn repair_secs(pt: PartType, sev: Severity, pit_crew_quality: f64, r: f64) -> u32 {
    let (lo, hi) = repair_secs_range(pt, sev);
    let span = (hi - lo + 1) as f64;
    let raw = lo + ((r * span) as u32).min(hi - lo); // lo..=hi
    ((raw as f64) * pit_time_factor(pit_crew_quality))
        .round()
        .max(1.0) as u32
}

// ───────────────────────── Pré-roll da corrida ─────────────────────────

/// Pré-rola a corrida inteira para UM carro e devolve os eventos de quebra, em ordem de
/// volta. Determinístico dado `(car.wear, laps, seed, pit_crew_quality, service_laps)` — o
/// mesmo resultado no pré-corrida (aviso) e ao vivo (disparo). A SORTE é a semente.
///
/// - Cada volta, cada peça acumula `wear_per_lap × (1 ± ruído)`.
/// - Ao cruzar 95%, cada volta é uma rolagem de sorte; a 105% a falha é forçada.
/// - Ao falhar: sorteia leve/grave/DNF (por peça; a parede agrava um degrau). Leve/grave
///   viram penalidade de tempo **por peça** modulada pelo `pit_crew_quality` da equipe.
/// - Um **DNF encerra** a corrida do carro.
/// - `track_pha`: demanda P/H/A da pista (de `get_track_simulation_data`) — inclina QUAL peça
///   se desgasta mais (pista de potência cobra o motor; técnica cobra freios/suspensão), sem
///   inflar a taxa total (multiplicador centrado em ~1.0).
/// - `weather = (wetness 0..1, temperature °C)`: chuva estressa a eletrônica e alivia a
///   térmica (~neutro); calor agrava motor/arrefecimento e **infla** o total.
/// - `service_laps`: voltas em que o carro para no box (enduro) e troca as peças acima de
///   [`SERVICE_WEAR_FLOOR`] — vazio para sprints (dial do gap-2, a estratégia de pit decide).
pub fn roll_race_breakdowns(
    car: &Car,
    laps: u32,
    seed: u64,
    pit_crew_quality: f64,
    track_pha: (f64, f64, f64),
    weather: (f64, f64),
    service_laps: &[u32],
) -> Vec<BreakdownEvent> {
    let mut wear: Vec<f64> = PartType::ALL
        .iter()
        .map(|&pt| car.part(pt).map(|p| p.wear).unwrap_or(0.0))
        .collect();
    let entered = wear.clone();
    let mut broken = [false; 11];
    let mut events = Vec::new();

    // Média do alinhamento das 11 peças com a pista — subtraída em cada peça pra CENTRAR os
    // multiplicadores em ~1.0 (redistribui o desgaste sem inflar a taxa total do grid).
    let mean_align =
        PartType::ALL.iter().map(|&pt| track_alignment(pt, track_pha)).sum::<f64>() / 11.0;

    for lap in 1..=laps {
        if service_laps.contains(&lap) {
            for (i, w) in wear.iter_mut().enumerate() {
                if !broken[i] && *w >= SERVICE_WEAR_FLOOR {
                    *w = 0.0;
                }
            }
        }
        for (i, &pt) in PartType::ALL.iter().enumerate() {
            if broken[i] {
                continue;
            }
            let noise = 1.0 + (roll(seed, i, lap, CH_NOISE) * 2.0 - 1.0) * WEAR_NOISE;
            wear[i] += wear_per_lap(pt)
                * noise
                * track_wear_mult(pt, track_pha, mean_align)
                * weather_wear_mult(pt, weather.0, weather.1);

            let (failed, forced) = if wear[i] >= HARD_WALL {
                (true, true)
            } else if wear[i] >= RISK_OPEN
                && roll(seed, i, lap, CH_HAZARD) < per_lap_hazard(pt, wear[i])
            {
                (true, false)
            } else {
                (false, false)
            };

            if failed {
                broken[i] = true;
                let severity = sample_severity(pt, forced, roll(seed, i, lap, CH_SEVERITY));
                let penalty = match severity {
                    Severity::Dnf => None,
                    _ => Some(repair_secs(
                        pt,
                        severity,
                        pit_crew_quality,
                        roll(seed, i, lap, CH_TIME),
                    )),
                };
                events.push(BreakdownEvent {
                    part: pt,
                    lap,
                    severity,
                    penalty_secs: penalty,
                    entered_wear: entered[i],
                    wear_at_fail: wear[i],
                    forced,
                });
                wear[i] = 0.0; // a peça é trocada dali em diante
                if severity == Severity::Dnf {
                    return events; // carro fora — encerra
                }
            }
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Qualidade neutra de pit crew para os testes que não medem esse eixo.
    const PIT_NEUTRO: f64 = 50.0;
    /// Pista equilibrada (sem influência: todos os multiplicadores = 1.0).
    const TRACK_NEUTRO: (f64, f64, f64) = (1.0, 1.0, 1.0);
    /// Pistas peaked para os testes de influência.
    const TRACK_POWER: (f64, f64, f64) = (0.70, 0.15, 0.15);
    const TRACK_HANDLING: (f64, f64, f64) = (0.15, 0.70, 0.15);
    /// Clima: seco a 25°C (neutro), aguaceiro, e forno de 42°C.
    const WEATHER_NEUTRO: (f64, f64) = (0.0, 25.0);
    const WEATHER_RAIN: (f64, f64) = (1.0, 20.0);
    const WEATHER_HOT: (f64, f64) = (0.0, 42.0);

    fn car_with(part: PartType, wear: f64) -> Car {
        let mut car = Car::uniform(3); // demais peças em wear 0.0
        car.set_wear(part, wear);
        car
    }

    // -------- Confiabilidade da peça sadia --------

    #[test]
    fn peca_sadia_nunca_quebra_num_sprint() {
        let car = Car::uniform(5);
        for seed in 0..1000 {
            let ev = roll_race_breakdowns(&car, 20, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
            assert!(ev.is_empty(), "carro novo não deveria quebrar em 20 voltas (seed {seed})");
        }
    }

    #[test]
    fn nenhuma_quebra_abaixo_de_95_por_cento() {
        for w in [0.0, 0.5, 0.80, 0.90, 0.94] {
            let car = car_with(PartType::Engine, w);
            for seed in 0..200 {
                for ev in roll_race_breakdowns(&car, 18, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]) {
                    assert!(
                        ev.wear_at_fail >= RISK_OPEN,
                        "quebra a {:.3} < 95% (peça entrou {w})",
                        ev.wear_at_fail
                    );
                }
            }
        }
    }

    // -------- Peça no limite quebra por sorte --------

    #[test]
    fn peca_no_limite_quebra_com_frequencia() {
        let car = car_with(PartType::Engine, 0.97);
        let quebrou = (0..1000)
            .filter(|&s| !roll_race_breakdowns(&car, 18, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]).is_empty())
            .count();
        assert!(quebrou > 700, "esperado muitas quebras, deu {quebrou}/1000");
    }

    #[test]
    fn a_culpada_e_a_peca_no_limite() {
        let car = car_with(PartType::Gearbox, 0.98);
        for seed in 0..300 {
            for ev in roll_race_breakdowns(&car, 18, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]) {
                assert_eq!(ev.part, PartType::Gearbox, "só o câmbio deveria quebrar aqui");
            }
        }
    }

    // -------- Determinismo e sorte --------

    #[test]
    fn e_deterministico() {
        let car = car_with(PartType::Engine, 0.96);
        for seed in [0u64, 7, 42, 1000, 999_999] {
            let a = roll_race_breakdowns(&car, 22, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
            let b = roll_race_breakdowns(&car, 22, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
            assert_eq!(a, b, "mesmos inputs deveriam dar o mesmo resultado (seed {seed})");
        }
    }

    #[test]
    fn a_sorte_varia_o_desfecho() {
        let car = car_with(PartType::Engine, 0.96);
        let mut voltas = std::collections::HashSet::new();
        for seed in 0..200 {
            if let Some(ev) = roll_race_breakdowns(&car, 22, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]).first() {
                voltas.insert(ev.lap);
            }
        }
        assert!(voltas.len() > 3, "a volta da quebra deveria variar com a sorte, deu {voltas:?}");
    }

    // -------- Parede vs sorte --------

    #[test]
    fn parede_forca_a_falha() {
        let car = car_with(PartType::Engine, 1.04);
        for seed in 0..300 {
            let ev = roll_race_breakdowns(&car, 18, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
            assert!(!ev.is_empty(), "peça a 104% deveria quebrar (seed {seed})");
        }
    }

    // -------- DNF encerra a corrida --------

    #[test]
    fn dnf_e_o_ultimo_e_unico() {
        let car = car_with(PartType::Engine, 1.04);
        for seed in 0..500 {
            let ev = roll_race_breakdowns(&car, 30, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
            let dnfs = ev.iter().filter(|e| e.is_dnf()).count();
            assert!(dnfs <= 1, "no máximo 1 DNF por corrida");
            if let Some(pos) = ev.iter().position(|e| e.is_dnf()) {
                assert_eq!(pos, ev.len() - 1, "DNF deveria ser o último evento");
            }
            // DNF não tem tempo de penalidade; leve/grave têm.
            for e in &ev {
                assert_eq!(e.is_dnf(), e.penalty_secs.is_none());
            }
        }
    }

    // -------- Severidade por peça --------

    #[test]
    fn motor_termina_em_dnf_mais_que_eletronica() {
        let dnf = |pt| {
            let (l, h) = severity_weights(pt);
            1.0 - l - h
        };
        assert!(
            dnf(PartType::Engine) > dnf(PartType::Electronics) + 0.2,
            "motor deveria dar muito mais DNF que eletrônica"
        );
    }

    #[test]
    fn parede_agrava_a_severidade() {
        let r = 0.05; // dentro da fatia "leve" do motor (light=0.20)
        assert_eq!(sample_severity(PartType::Engine, false, r), Severity::Light);
        assert_eq!(sample_severity(PartType::Engine, true, r), Severity::Heavy);
    }

    // -------- Tempo de conserto CONDIZENTE com a peça e a severidade --------

    #[test]
    fn grave_demora_mais_que_leve_na_mesma_peca() {
        let leve = repair_secs(PartType::Gearbox, Severity::Light, PIT_NEUTRO, 0.5);
        let grave = repair_secs(PartType::Gearbox, Severity::Heavy, PIT_NEUTRO, 0.5);
        assert!(grave > leve, "câmbio grave ({grave}s) deveria demorar mais que leve ({leve}s)");
    }

    #[test]
    fn peca_grande_demora_mais_que_pequena() {
        let g = |pt| repair_secs(pt, Severity::Heavy, PIT_NEUTRO, 0.5);
        assert!(g(PartType::Gearbox) > g(PartType::Suspension));
        assert!(g(PartType::Suspension) > g(PartType::Brakes));
        assert!(g(PartType::Brakes) > g(PartType::FrontWing));
        assert!(g(PartType::FrontWing) >= g(PartType::Electronics));
    }

    #[test]
    fn cambio_leve_e_grave_nos_intervalos_esperados() {
        // Pit neutro (fator 1.0) → tempo cru dentro da faixa da tabela.
        for r in [0.0, 0.3, 0.6, 0.99] {
            let leve = repair_secs(PartType::Gearbox, Severity::Light, PIT_NEUTRO, r);
            assert!((6..=9).contains(&leve), "câmbio leve fora de 6-9: {leve}");
            let grave = repair_secs(PartType::Gearbox, Severity::Heavy, PIT_NEUTRO, r);
            assert!((14..=20).contains(&grave), "câmbio grave fora de 14-20: {grave}");
        }
    }

    // -------- Variância pela qualidade do pit crew --------

    #[test]
    fn pit_crew_melhor_conserta_mais_rapido() {
        let ruim = repair_secs(PartType::Gearbox, Severity::Heavy, 20.0, 0.5);
        let bom = repair_secs(PartType::Gearbox, Severity::Heavy, 95.0, 0.5);
        assert!(bom < ruim, "pit bom ({bom}s) deveria ser mais rápido que ruim ({ruim}s)");
    }

    #[test]
    fn fator_de_pit_neutro_em_50() {
        assert!((pit_time_factor(50.0) - 1.0).abs() < 1e-9);
        assert!(pit_time_factor(0.0) > 1.0);
        assert!(pit_time_factor(100.0) < 1.0);
    }

    // -------- Comandos --------

    #[test]
    fn comando_black_para_penalidade_dq_para_dnf() {
        let pen = BreakdownEvent {
            part: PartType::Brakes,
            lap: 5,
            severity: Severity::Heavy,
            penalty_secs: Some(9),
            entered_wear: 0.96,
            wear_at_fail: 0.99,
            forced: false,
        };
        assert_eq!(pen.command(7), "!black #7 9");
        assert!(!pen.is_dnf());
        let dnf = BreakdownEvent { severity: Severity::Dnf, penalty_secs: None, ..pen };
        assert_eq!(dnf.command(7), "!dq #7");
        assert!(dnf.is_dnf());
    }

    // -------- Influência da pista (qual peça sofre) --------

    #[test]
    fn pista_neutra_nao_influencia() {
        // Pista equilibrada → multiplicador 1.0 para toda peça.
        let mean = PartType::ALL
            .iter()
            .map(|&pt| track_alignment(pt, TRACK_NEUTRO))
            .sum::<f64>()
            / 11.0;
        for &pt in &PartType::ALL {
            assert!((track_wear_mult(pt, TRACK_NEUTRO, mean) - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn pista_de_potencia_estressa_o_motor_nao_os_freios() {
        let mean = PartType::ALL
            .iter()
            .map(|&pt| track_alignment(pt, TRACK_POWER))
            .sum::<f64>()
            / 11.0;
        let motor = track_wear_mult(PartType::Engine, TRACK_POWER, mean);
        let freios = track_wear_mult(PartType::Brakes, TRACK_POWER, mean);
        assert!(motor > 1.0, "motor deveria sofrer mais na pista de potência ({motor})");
        assert!(freios < 1.0, "freios deveriam sofrer menos ({freios})");
        assert!(motor > freios);
    }

    #[test]
    fn pista_de_handling_estressa_os_freios_nao_o_motor() {
        let mean = PartType::ALL
            .iter()
            .map(|&pt| track_alignment(pt, TRACK_HANDLING))
            .sum::<f64>()
            / 11.0;
        let motor = track_wear_mult(PartType::Engine, TRACK_HANDLING, mean);
        let freios = track_wear_mult(PartType::Brakes, TRACK_HANDLING, mean);
        assert!(freios > motor, "na pista técnica os freios ({freios}) deveriam sofrer mais que o motor ({motor})");
    }

    /// Roda uma frota REALISTA (desgaste de entrada variado por peça: maioria baixo, poucas
    /// perto da zona) e conta as quebras por peça.
    fn fleet_breaks(track: (f64, f64, f64), weather: (f64, f64)) -> ([u32; 11], u32) {
        let mut by_part = [0u32; 11];
        let mut total = 0;
        for seed in 0..8000u64 {
            let mut car = Car::uniform(3);
            for (i, &pt) in PartType::ALL.iter().enumerate() {
                // Desgaste de entrada determinístico e enviesado pra baixo (cauda até ~0.97).
                let u = roll(seed ^ 0xA5A5_5A5A, i, 1, 99);
                car.set_wear(pt, 0.30 + 0.67 * u * u);
            }
            let ev = roll_race_breakdowns(&car, 20, seed, PIT_NEUTRO, track, weather, &[]);
            if !ev.is_empty() {
                total += 1;
            }
            for e in &ev {
                let i = PartType::ALL.iter().position(|&x| x == e.part).unwrap();
                by_part[i] += 1;
            }
        }
        (by_part, total)
    }

    #[test]
    fn taxa_total_quase_invariante_e_distribuicao_inclina_por_pista() {
        let (neutro, tn) = fleet_breaks(TRACK_NEUTRO, WEATHER_NEUTRO);
        let (power, tp) = fleet_breaks(TRACK_POWER, WEATHER_NEUTRO);
        let (handling, th) = fleet_breaks(TRACK_HANDLING, WEATHER_NEUTRO);

        // Relatório (visível com --nocapture).
        let idx = |pt: PartType| PartType::ALL.iter().position(|&x| x == pt).unwrap();
        println!("\n── Influência da pista (frota com desgaste realista) ──");
        println!("Taxa total (carros com quebra): neutro {tn} · power {tp} · handling {th}");
        println!(
            "{:<12} {:>8} {:>8} {:>8}",
            "peça", "neutro", "power", "handling"
        );
        for &pt in &PartType::ALL {
            let i = idx(pt);
            println!("{:<12} {:>8} {:>8} {:>8}", pt.as_str(), neutro[i], power[i], handling[i]);
        }

        // (a) A taxa total quase não muda com o tipo de pista (redistribui, não infla).
        let max = tn.max(tp).max(th) as f64;
        let min = tn.min(tp).min(th) as f64;
        assert!((max - min) / max < 0.15, "taxa total variou demais: {tn}/{tp}/{th}");

        // (b) O motor sofre mais na pista de potência; os freios, na técnica.
        assert!(power[idx(PartType::Engine)] > handling[idx(PartType::Engine)],
            "motor deveria quebrar mais na pista de potência");
        assert!(handling[idx(PartType::Brakes)] > power[idx(PartType::Brakes)],
            "freios deveriam quebrar mais na pista técnica");
    }

    // -------- Influência do clima (chuva/calor) --------

    #[test]
    fn clima_neutro_nao_influencia() {
        for &pt in &PartType::ALL {
            assert!((weather_wear_mult(pt, WEATHER_NEUTRO.0, WEATHER_NEUTRO.1) - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn chuva_estressa_eletronica_e_alivia_a_termica() {
        let elec = weather_wear_mult(PartType::Electronics, 1.0, 20.0);
        let motor = weather_wear_mult(PartType::Engine, 1.0, 20.0);
        let cooling = weather_wear_mult(PartType::Cooling, 1.0, 20.0);
        assert!(elec > 1.0, "chuva deveria estressar a eletrônica ({elec})");
        assert!(motor < 1.0 && cooling < 1.0, "chuva deveria aliviar motor/arrefecimento");
        // Peça sem relação com chuva não muda.
        assert!((weather_wear_mult(PartType::Brakes, 1.0, 20.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn calor_agrava_motor_e_arrefecimento() {
        let motor_frio = weather_wear_mult(PartType::Engine, 0.0, 25.0);
        let motor_forno = weather_wear_mult(PartType::Engine, 0.0, 42.0);
        assert!(motor_forno > motor_frio, "forno deveria agravar o motor ({motor_forno} vs {motor_frio})");
        assert!((weather_wear_mult(PartType::Brakes, 0.0, 42.0) - 1.0).abs() < 1e-9, "calor não mexe nos freios");
    }

    #[test]
    fn chuva_inclina_pra_eletronica_e_calor_infla_o_total() {
        let idx = |pt: PartType| PartType::ALL.iter().position(|&x| x == pt).unwrap();
        let (seco, ts) = fleet_breaks(TRACK_NEUTRO, WEATHER_NEUTRO);
        let (chuva, tc) = fleet_breaks(TRACK_NEUTRO, WEATHER_RAIN);
        let (forno, tf) = fleet_breaks(TRACK_NEUTRO, WEATHER_HOT);

        println!("\n── Influência do clima (frota realista) ──");
        println!("Taxa total: seco {ts} · chuva {tc} · forno {tf}");
        println!(
            "eletrônica: seco {} · chuva {} · forno {}",
            seco[idx(PartType::Electronics)], chuva[idx(PartType::Electronics)], forno[idx(PartType::Electronics)]
        );
        println!(
            "motor:      seco {} · chuva {} · forno {}",
            seco[idx(PartType::Engine)], chuva[idx(PartType::Engine)], forno[idx(PartType::Engine)]
        );

        // Chuva: eletrônica quebra mais; motor menos.
        assert!(chuva[idx(PartType::Electronics)] > seco[idx(PartType::Electronics)],
            "chuva deveria quebrar mais eletrônica");
        assert!(chuva[idx(PartType::Engine)] < seco[idx(PartType::Engine)],
            "chuva deveria quebrar menos motor");
        // Calor (escolha A): motor quebra mais E o total sobe.
        assert!(forno[idx(PartType::Engine)] > seco[idx(PartType::Engine)],
            "forno deveria quebrar mais motor");
        assert!(tf > ts, "forno deveria INFLAR o total (escolha A): forno {tf} vs seco {ts}");
    }

    // -------- Proteção do jogador (só o jogador, via manutenção) --------

    #[test]
    fn time_forte_nao_da_protecao_ao_jogador() {
        // Crew no topo (100) → alívio 0 → carro idêntico → chances iguais à IA.
        let mut car = Car::uniform(4);
        car.set_wear(PartType::Engine, 0.90);
        let protegido = player_protected_car(&car, 100.0);
        assert_eq!(protegido, car, "time forte não deveria mudar o carro do jogador");
    }

    #[test]
    fn protecao_reduz_quebras_do_jogador_em_time_fraco() {
        // Frota pobre (desgaste alto). Sem proteção (crew 100) vs protegido (crew fraco 45).
        // Frota pobre NÃO-saturada (taxa moderada) pra o efeito da proteção aparecer.
        let n = 8000u64;
        let total = |crew: f64| -> usize {
            let mut c = 0;
            for seed in 0..n {
                let mut raw = Car::uniform(3);
                for (i, &pt) in PartType::ALL.iter().enumerate() {
                    let u = roll(seed ^ 0xBEEF, i, 1, 7);
                    raw.set_wear(pt, 0.20 + 0.48 * u); // 0.20–0.68 (poucas peças perto da zona)
                }
                let car = player_protected_car(&raw, crew);
                if !roll_race_breakdowns(&car, 18, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[])
                    .is_empty()
                {
                    c += 1;
                }
            }
            c
        };
        let sem = total(100.0); // crew máximo = sem proteção (poor-AI)
        let protegido = total(45.0); // time pobre do jogador = protegido
        let pct = |x: usize| x as f64 / n as f64 * 100.0;
        println!(
            "\n── Proteção do jogador ── quebra/corrida: poor-AI {:.1}% · jogador-protegido {:.1}% (razão {:.2})",
            pct(sem), pct(protegido), protegido as f64 / sem as f64
        );
        // Banda sã: protege de verdade, mas sem tornar o jogador quase-imune (a frota
        // sintética é arbitrária; o valor fino se calibra no wiring com desgaste real).
        assert!(protegido < sem * 4 / 5, "proteção fraca demais: {protegido} vs {sem}");
        assert!(protegido > sem / 5, "proteção forte demais (quase imune): {protegido} vs {sem}");
    }

    // -------- Manutenção em box do enduro (gap 2) --------

    #[test]
    fn parada_de_box_reduz_quebras_no_enduro() {
        let car = car_with(PartType::Engine, 0.5);
        let sem: usize = (0..500)
            .filter(|&s| !roll_race_breakdowns(&car, 50, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]).is_empty())
            .count();
        let com: usize = (0..500)
            .filter(|&s| !roll_race_breakdowns(&car, 50, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[16, 32]).is_empty())
            .count();
        assert!(com < sem, "a parada de box deveria reduzir quebras (sem={sem}, com={com})");
    }
}
