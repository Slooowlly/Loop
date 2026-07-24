//! Suíte de testes do modelo de quebras (extraída de `breakdown.rs`).
//!
//! Continua sendo o mesmo módulo `tests` de antes: `use super::*` enxerga o
//! módulo `breakdown` inteiro, incluindo os itens privados.

use super::*;

/// Qualidade neutra de pit crew para os testes que não medem esse eixo.
const PIT_NEUTRO: f64 = 50.0;
/// Pista equilibrada (sem influência: todos os multiplicadores = 1.0).
const TRACK_NEUTRO: (f64, f64, f64) = (1.0, 1.0, 1.0);
/// Pistas peaked para os testes de influência.
const TRACK_POWER: (f64, f64, f64) = (0.70, 0.15, 0.15);
const TRACK_HANDLING: (f64, f64, f64) = (0.15, 0.70, 0.15);
/// Clima: neutro (25°C, seco), aguaceiro, e dia quente máx (32°C — a faixa real do iRacing).
const WEATHER_NEUTRO: Weather = Weather::NEUTRAL;
const WEATHER_RAIN: Weather = Weather {
    wetness: 1.0,
    temperature: 20.0,
    humidity: 90.0,
    wind_kmh: 25.0,
};
const WEATHER_HOT: Weather = Weather {
    wetness: 0.0,
    temperature: 32.0,
    humidity: 45.0,
    wind_kmh: 18.0,
};

fn car_with(part: PartType, wear: f64) -> Car {
    let mut car = Car::uniform(3); // demais peças em wear 0.0
    car.set_wear(part, wear);
    car
}

// -------- Primitivo ao vivo (LiveBreakdown) --------

/// O loop manual do `LiveBreakdown` (mesmo clima toda volta) é IDÊNTICO ao pré-roll —
/// o disparo ao vivo e o pré-roll compartilham o mesmo cérebro.
#[test]
fn live_step_equivale_ao_preroll() {
    for seed in 0..400u64 {
        // Frota realista: desgaste de entrada variado por peça.
        let mut car = Car::uniform(3);
        for (i, &pt) in PartType::ALL.iter().enumerate() {
            let u = roll(seed ^ 0xBEEF, i, 1, 7);
            car.set_wear(pt, 0.30 + 0.67 * u * u);
        }
        let batch =
            roll_race_breakdowns(&car, 22, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
        let mut state = LiveBreakdown::new(&car, seed, PIT_NEUTRO, TRACK_NEUTRO);
        let mut live = Vec::new();
        for lap in 1..=22 {
            live.extend(state.advance_lap(lap, WEATHER_NEUTRO));
            if state.is_out() {
                break;
            }
        }
        assert_eq!(batch, live, "seed {seed}: live != pré-roll");
    }
}

// -------- Análise: taxa de quebra por corrida (diagnóstico, não regressão) --------

/// Clima BRUTAL pro motor/arrefecimento: dia quente MÁX + úmido (amplifica a térmica) +
/// vento forte (estressa suspensão/asas). O pior cenário realista da faixa do iRacing.
const WEATHER_BRUTAL: Weather = Weather {
    wetness: 0.0,
    temperature: 32.0,
    humidity: 95.0,
    wind_kmh: 42.0,
};

/// Monta um carro com um PERFIL de desgaste de entrada que reflete o que o cérebro de
/// manutenção deixa cada tipo de time levar pra pista (ver `car_maintenance::needs_decision`,
/// que troca a peça quando `wear + wear_per_race >= 1.0`).
fn perfil(kind: &str) -> Car {
    use crate::car::wear::wear_per_race;
    let mut car = Car::uniform(3);
    // Limiar em que o time DECIDE trocar (peça cruzaria 100% na próxima corrida).
    let limiar = |pt: PartType| 1.0 - wear_per_race(pt);
    for &pt in &PartType::ALL {
        let frag = pt.durability() <= 3; // motor/câmbio/freios/asas/suspensão
        let w = match kind {
            // Time rico: repõe no limiar → entra, em média, na METADE da vida útil.
            "rico_saudavel" => limiar(pt) / 2.0,
            // Time rico no PIOR momento do ciclo: logo antes de repor.
            "rico_limitrofe" => (limiar(pt) - 0.02).max(0.0),
            // Time pobre: não repôs as frágeis → esticou até o limiar; resto na metade.
            "pobre_esticando" => if frag { limiar(pt) } else { limiar(pt) / 2.0 },
            // Time quebrado: frágeis DEGRADADAS além da vida; resto no limiar.
            "pobre_degradado" => if frag { 0.98 } else { limiar(pt) - 0.02 },
            _ => 0.0,
        };
        car.set_wear(pt, w);
    }
    car
}

/// Roda o MC real (`roll_race_breakdowns`) e mede, POR CORRIDA:
/// (P(≥1 quebra qualquer), P(≥1 quebra que custa tempo), P(DNF do carro)).
fn medir(car: &Car, laps: u32, track: (f64, f64, f64), weather: Weather, samples: u32) -> (f64, f64, f64) {
    let base = 0x00C0_FFEE_u64;
    let (mut any, mut costly, mut dnf) = (0u32, 0u32, 0u32);
    for i in 0..samples {
        let seed = splitmix64(base ^ splitmix64(i as u64));
        let evs = roll_race_breakdowns(car, laps, seed, PIT_NEUTRO, track, weather, &[]);
        if !evs.is_empty() {
            any += 1;
        }
        if evs.iter().any(|e| e.severity == Severity::Heavy || e.is_dnf()) {
            costly += 1;
        }
        if evs.iter().any(|e| e.is_dnf()) {
            dnf += 1;
        }
    }
    let n = samples as f64;
    (any as f64 / n, costly as f64 / n, dnf as f64 / n)
}

/// "1 a cada N corridas" a partir da probabilidade por corrida (∞ se ~0).
fn cada(p: f64) -> String {
    if p < 1e-4 {
        "     —".to_string()
    } else {
        format!("1/{:>4.1}", 1.0 / p)
    }
}

/// DIAGNÓSTICO (rode com: `cargo test analise_taxa_quebra -- --ignored --nocapture`).
/// Não é regressão — só imprime a taxa esperada de quebra por corrida pra calibração.
#[test]
#[ignore]
fn analise_taxa_quebra() {
    const N: u32 = 20_000;
    let perfis = ["rico_saudavel", "rico_limitrofe", "pobre_esticando", "pobre_degradado"];
    let cenarios: [(&str, (f64, f64, f64), Weather); 2] = [
        ("NEUTRO  (pista equilibrada, 25C seco)", TRACK_NEUTRO, WEATHER_NEUTRO),
        ("BRUTAL  (pista de potencia, 32C umido)", TRACK_POWER, WEATHER_BRUTAL),
    ];

    for laps in [18u32, 30] {
        println!("\n================ CORRIDA DE {laps} VOLTAS ================");
        for (nome_cen, track, weather) in cenarios {
            println!("\n  -- {nome_cen} --");
            println!(
                "  {:<18} {:>10} {:>12} {:>10}",
                "perfil do time", "qq quebra", "custa tempo", "DNF"
            );
            for kind in perfis {
                let car = perfil(kind);
                let (a, c, d) = medir(&car, laps, track, weather, N);
                println!(
                    "  {:<18} {:>5.1}% {:>6}  {:>5.1}% {:>6}  {:>5.1}% {}",
                    kind,
                    a * 100.0,
                    cada(a),
                    c * 100.0,
                    cada(c),
                    d * 100.0,
                    cada(d),
                );
            }
        }
    }

    // Curva: risco de UMA peça frágil (motor, durab 3) sozinha, por desgaste de entrada.
    println!("\n================ CURVA: MOTOR (durab 3) SOZINHO, 18 voltas ================");
    println!("  desgaste_entrada   qq quebra    custa tempo    DNF   (NEUTRO / BRUTAL)");
    for pct in [50, 60, 67, 75, 85, 95, 100, 103] {
        let w = pct as f64 / 100.0;
        let car = car_with(PartType::Engine, w);
        let (an, cn, dn) = medir(&car, 18, TRACK_NEUTRO, WEATHER_NEUTRO, N);
        let (ab, cb, db) = medir(&car, 18, TRACK_POWER, WEATHER_BRUTAL, N);
        println!(
            "  {:>3}%   N:{:>5.1}%/{:>5.1}%/{:>5.1}%   B:{:>5.1}%/{:>5.1}%/{:>5.1}%",
            pct,
            an * 100.0, cn * 100.0, dn * 100.0,
            ab * 100.0, cb * 100.0, db * 100.0,
        );
    }
    println!();
}

/// Métricas profundas de UMA corrida (por peça-que-falha), agregadas no Monte Carlo.
struct DistQuebra {
    /// P(≥1 quebra qualquer), P(quebra que custa tempo), P(DNF do carro).
    any: f64,
    costly: f64,
    dnf: f64,
    /// Distribuição do nº de peças que falharam numa corrida: 0, 1, 2, ≥3.
    p0: f64,
    p1: f64,
    p2: f64,
    p3plus: f64,
    /// P(≥2 quebras | ≥1 quebra) — "erros SEGUIDOS na MESMA corrida".
    cond_2plus: f64,
    /// Média de peças que falham por corrida.
    mean: f64,
}

/// Roda o MC real e agrega a DISTRIBUIÇÃO do nº de quebras por corrida (não só "teve/não
/// teve"). `roll_race_breakdowns` já para no 1º DNF, então o nº de eventos é o nº de peças
/// que largaram antes (ou até) o carro sair.
fn medir_profundo(
    car: &Car,
    laps: u32,
    track: (f64, f64, f64),
    weather: Weather,
    samples: u32,
) -> DistQuebra {
    let base = 0x00C0_FFEE_u64;
    let (mut any, mut costly, mut dnf) = (0u32, 0u32, 0u32);
    let (mut c0, mut c1, mut c2, mut c3) = (0u32, 0u32, 0u32, 0u32);
    let mut total_events = 0u64;
    for i in 0..samples {
        let seed = splitmix64(base ^ splitmix64(i as u64));
        let evs = roll_race_breakdowns(car, laps, seed, PIT_NEUTRO, track, weather, &[]);
        let n = evs.len();
        total_events += n as u64;
        match n {
            0 => c0 += 1,
            1 => c1 += 1,
            2 => c2 += 1,
            _ => c3 += 1,
        }
        if n >= 1 {
            any += 1;
        }
        if evs.iter().any(|e| e.severity == Severity::Heavy || e.is_dnf()) {
            costly += 1;
        }
        if evs.iter().any(|e| e.is_dnf()) {
            dnf += 1;
        }
    }
    let s = samples as f64;
    let two_plus = (c2 + c3) as f64;
    DistQuebra {
        any: any as f64 / s,
        costly: costly as f64 / s,
        dnf: dnf as f64 / s,
        p0: c0 as f64 / s,
        p1: c1 as f64 / s,
        p2: c2 as f64 / s,
        p3plus: c3 as f64 / s,
        cond_2plus: if any > 0 { two_plus / any as f64 } else { 0.0 },
        mean: total_events as f64 / s,
    }
}

/// DIAGNÓSTICO PROFUNDO — Perguntas 1 e 3 (rode com:
/// `cargo test analise_profunda_quebras -- --ignored --nocapture`).
/// (1) QUÃO FREQUENTES são as quebras por corrida, por perfil de time e cenário.
/// (3) Com que frequência caem QUEBRAS SEGUIDAS na MESMA corrida (≥2 peças) — a coluna
///     `P(≥2|≥1)` é "dado que quebrou, a chance de ter sido MAIS de uma peça".
/// A recorrência ENTRE corridas (mesma peça na próxima) está em
/// `car_maintenance::tests::analise_recorrencia_entre_corridas`.
#[test]
#[ignore]
fn analise_profunda_quebras() {
    const N: u32 = 40_000;
    let perfis = ["rico_saudavel", "rico_limitrofe", "pobre_esticando", "pobre_degradado"];
    let cenarios: [(&str, (f64, f64, f64), Weather); 2] = [
        ("NEUTRO (pista equilibrada, 25C seco)", TRACK_NEUTRO, WEATHER_NEUTRO),
        ("BRUTAL (pista de potencia, 32C umido+vento)", TRACK_POWER, WEATHER_BRUTAL),
    ];

    for laps in [18u32, 30] {
        println!("\n================ CORRIDA DE {laps} VOLTAS ================");
        for (cen, track, weather) in cenarios {
            println!("\n  -- cenário {cen} --");
            println!(
                "  {:<18} {:>6} {:>6} {:>6}  |  {:>6} {:>6} {:>6} {:>6}  {:>9} {:>6}",
                "perfil", "≥1qb", "custa", "DNF", "0qb", "1qb", "2qb", "≥3qb", "P(≥2|≥1)", "média"
            );
            for kind in perfis {
                let car = perfil(kind);
                let m = medir_profundo(&car, laps, track, weather, N);
                println!(
                    "  {:<18} {:>5.1}% {:>5.1}% {:>5.1}%  |  {:>5.1}% {:>5.1}% {:>5.1}% {:>5.1}%  {:>8.1}% {:>6.2}",
                    kind,
                    m.any * 100.0,
                    m.costly * 100.0,
                    m.dnf * 100.0,
                    m.p0 * 100.0,
                    m.p1 * 100.0,
                    m.p2 * 100.0,
                    m.p3plus * 100.0,
                    m.cond_2plus * 100.0,
                    m.mean,
                );
            }
        }
    }
    println!();
}

// -------- Diretor do disparo ao vivo --------

#[test]
fn diretor_dispara_peca_no_limite() {
    let mut car = Car::uniform(3);
    car.set_wear(PartType::Engine, 1.22); // além da parede (1.20) → falha forçada na volta 1
    let live = LiveBreakdown::new(&car, 42, PIT_NEUTRO, TRACK_NEUTRO);
    let mut dir = BreakdownDirector::new();
    dir.add_car(7, live, vec![]);
    let evs = dir.on_lap(7, 1, WEATHER_NEUTRO);
    assert_eq!(evs.len(), 1, "deveria disparar 1 evento na volta 1");
    assert_eq!(evs[0].part, PartType::Engine);
    let cmd = evs[0].command(7);
    assert!(cmd.starts_with("!black #7") || cmd == "!dq #7", "comando inesperado: {cmd}");
}

#[test]
fn diretor_ignora_carro_fora_do_grid() {
    let mut dir = BreakdownDirector::new();
    assert!(dir.is_empty());
    assert!(dir.on_lap(99, 5, WEATHER_NEUTRO).is_empty(), "carro não montado → nada");
}

#[test]
fn diretor_nao_redispara_a_mesma_volta() {
    let mut car = Car::uniform(3);
    car.set_wear(PartType::Brakes, 1.22); // além da parede (HARD_WALL 1.20) → falha forçada garantida
    let live = LiveBreakdown::new(&car, 1, PIT_NEUTRO, TRACK_NEUTRO);
    let mut dir = BreakdownDirector::new();
    dir.add_car(3, live, vec![]);
    assert!(!dir.on_lap(3, 1, WEATHER_NEUTRO).is_empty(), "1ª chamada deveria disparar");
    assert!(dir.on_lap(3, 1, WEATHER_NEUTRO).is_empty(), "reprocessar a volta 1 não redispara");
}

#[test]
fn diretor_avanca_multiplas_voltas_sem_duplicar() {
    // Se o monitor "pula" pra volta 40, avança tudo de uma vez; reprocessar não faz nada.
    let car = Car::uniform(3);
    let live = LiveBreakdown::new(&car, 5, PIT_NEUTRO, TRACK_NEUTRO);
    let mut dir = BreakdownDirector::new();
    dir.add_car(1, live, vec![]);
    let _ = dir.on_lap(1, 40, WEATHER_NEUTRO);
    assert!(dir.on_lap(1, 40, WEATHER_NEUTRO).is_empty(), "reprocessar não redispara");
}

#[test]
fn diretor_para_apos_dnf() {
    // Acha um seed que dá DNF numa peça na parede; confirma que o carro para de disparar.
    let seed = (0..500u64)
        .find(|&s| {
            let mut c = Car::uniform(3);
            c.set_wear(PartType::Engine, 1.22);
            let mut lb = LiveBreakdown::new(&c, s, PIT_NEUTRO, TRACK_NEUTRO);
            lb.advance_lap(1, WEATHER_NEUTRO)
                .iter()
                .any(|e| e.severity == Severity::Dnf)
        })
        .expect("algum seed deveria dar DNF no motor na parede");
    let mut car = Car::uniform(3);
    car.set_wear(PartType::Engine, 1.22);
    let live = LiveBreakdown::new(&car, seed, PIT_NEUTRO, TRACK_NEUTRO);
    let mut dir = BreakdownDirector::new();
    dir.add_car(2, live, vec![]);
    let first = dir.on_lap(2, 1, WEATHER_NEUTRO);
    assert!(first.iter().any(|e| e.severity == Severity::Dnf), "volta 1 deveria dar DNF");
    assert!(dir.on_lap(2, 30, WEATHER_NEUTRO).is_empty(), "após DNF o carro não dispara mais");
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
        dnf(PartType::Engine) > dnf(PartType::Electronics) + 0.15,
        "motor deveria dar muito mais DNF que eletrônica"
    );
}

#[test]
fn parede_agrava_a_severidade() {
    let r = 0.05; // dentro da fatia "leve" do motor (light=0.20)
    assert_eq!(sample_severity(PartType::Engine, false, r, false), Severity::Light);
    assert_eq!(sample_severity(PartType::Engine, true, r, false), Severity::Heavy);
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
        problem: 0,
    };
    assert_eq!(pen.command(7), "!black #7 9");
    assert!(!pen.is_dnf());
    let dnf = BreakdownEvent { severity: Severity::Dnf, penalty_secs: None, ..pen };
    assert_eq!(dnf.command(7), "!dq #7");
    assert!(dnf.is_dnf());
}

#[test]
fn catalogo_de_problemas_cobre_todas_as_pecas_e_severidades() {
    // Toda combinação (peça × modo × severidade) devolve uma frase não-vazia e o modo
    // além do range faz wrap (nunca entra no braço impossível de panic).
    for &pt in &PartType::ALL {
        for mode in 0..(FAILURE_MODES + 2) {
            for sev in [Severity::Light, Severity::Heavy, Severity::Dnf] {
                assert!(!problem_text(pt, mode, sev).is_empty());
            }
        }
    }
    // O rótulo do evento combina peça + modo + severidade (DNF do motor cita "fundiu").
    let ev = BreakdownEvent {
        part: PartType::Engine,
        lap: 3,
        severity: Severity::Dnf,
        penalty_secs: None,
        entered_wear: 0.98,
        wear_at_fail: 1.06,
        forced: true,
        problem: 0,
    };
    assert_eq!(ev.problem_label(), "motor fundiu por superaquecimento");
}

#[test]
fn previsao_de_risco_reflete_o_desgaste_de_entrada() {
    // Carro novo (desgaste baixo) → risco ~zero num sprint.
    let sadio = Car::uniform(3);
    let f0 = forecast_breakdown_risk(&sadio, 18, 42, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], 300, false, true);
    assert!(f0.dnf_prob < 0.02, "carro novo não deveria ter risco de DNF: {}", f0.dnf_prob);

    // Motor entrando na zona de perigo → motor vira a peça de MAIOR risco e o risco sobe.
    let mut gasto = Car::uniform(3);
    gasto.set_wear(PartType::Engine, 0.98);
    let f1 = forecast_breakdown_risk(&gasto, 18, 42, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], 300, false, true);
    assert!(f1.dnf_prob > f0.dnf_prob, "carro gasto deveria ter mais risco");
    assert!(!f1.parts.is_empty());
    assert_eq!(f1.parts[0].part, PartType::Engine, "o motor no fio deveria liderar o risco");
    assert!(f1.parts[0].any_prob > 0.3, "motor a 98% deveria largar com frequência: {}", f1.parts[0].any_prob);

    // Determinístico: mesma entrada → mesma previsão.
    let f2 = forecast_breakdown_risk(&gasto, 18, 42, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], 300, false, true);
    assert_eq!(f1, f2);
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
fn fleet_breaks(track: (f64, f64, f64), weather: Weather) -> ([u32; 11], u32) {
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

/// Clima só com wetness+temp movidos (umidade/vento neutros) — pra isolar um eixo.
fn wx(wetness: f64, temperature: f64) -> Weather {
    Weather { wetness, temperature, humidity: 45.0, wind_kmh: WIND_TYPICAL_KMH }
}

#[test]
fn clima_neutro_nao_influencia() {
    for &pt in &PartType::ALL {
        assert!((weather_wear_mult(pt, WEATHER_NEUTRO) - 1.0).abs() < 1e-9);
    }
}

#[test]
fn chuva_estressa_eletronica_e_alivia_a_termica() {
    let elec = weather_wear_mult(PartType::Electronics, wx(1.0, 20.0));
    let motor = weather_wear_mult(PartType::Engine, wx(1.0, 20.0));
    let cooling = weather_wear_mult(PartType::Cooling, wx(1.0, 20.0));
    assert!(elec > 1.0, "chuva deveria estressar a eletrônica ({elec})");
    assert!(motor < 1.0 && cooling < 1.0, "chuva deveria aliviar motor/arrefecimento");
    // Peça sem relação com chuva/vento não muda.
    assert!((weather_wear_mult(PartType::Brakes, wx(1.0, 20.0)) - 1.0).abs() < 1e-9);
}

#[test]
fn calor_agrava_e_frio_alivia_o_motor() {
    // Modelo CENTRADO em 25°C: 32° (máx real) agrava, 18° (mín real) alivia.
    let motor_neutro = weather_wear_mult(PartType::Engine, wx(0.0, 25.0));
    let motor_quente = weather_wear_mult(PartType::Engine, wx(0.0, 32.0));
    let motor_frio = weather_wear_mult(PartType::Engine, wx(0.0, 18.0));
    assert!((motor_neutro - 1.0).abs() < 1e-9, "25° deveria ser neutro");
    assert!(motor_quente > 1.0, "dia quente (32°) deveria agravar o motor ({motor_quente})");
    assert!(motor_frio < 1.0, "dia frio (18°) deveria aliviar o motor ({motor_frio})");
    assert!((weather_wear_mult(PartType::Brakes, wx(0.0, 32.0)) - 1.0).abs() < 1e-9, "calor não mexe nos freios");
}

#[test]
fn umidade_amplifica_o_calor() {
    let seco = Weather { wetness: 0.0, temperature: 32.0, humidity: 10.0, wind_kmh: WIND_TYPICAL_KMH };
    let umido = Weather { wetness: 0.0, temperature: 32.0, humidity: 95.0, wind_kmh: WIND_TYPICAL_KMH };
    let motor_seco = weather_wear_mult(PartType::Engine, seco);
    let motor_umido = weather_wear_mult(PartType::Engine, umido);
    assert!(motor_umido > motor_seco, "dia quente ÚMIDO deveria castigar mais o motor ({motor_umido} vs {motor_seco})");
    // Em dia FRIO a umidade não vira alívio extra (só amplifica o lado quente).
    let frio_seco = weather_wear_mult(PartType::Engine, Weather { wetness: 0.0, temperature: 18.0, humidity: 10.0, wind_kmh: WIND_TYPICAL_KMH });
    let frio_umido = weather_wear_mult(PartType::Engine, Weather { wetness: 0.0, temperature: 18.0, humidity: 95.0, wind_kmh: WIND_TYPICAL_KMH });
    assert!((frio_seco - frio_umido).abs() < 1e-9, "umidade não deveria mexer no dia frio");
}

#[test]
fn vento_estressa_suspensao_e_asas() {
    let calmo = Weather { wetness: 0.0, temperature: 25.0, humidity: 45.0, wind_kmh: 2.0 };
    let vendaval = Weather { wetness: 0.0, temperature: 25.0, humidity: 45.0, wind_kmh: 48.0 };
    for pt in [PartType::Suspension, PartType::FrontWing, PartType::RearWing] {
        assert!(weather_wear_mult(pt, vendaval) > weather_wear_mult(pt, calmo),
            "{pt:?} deveria sofrer mais no vendaval");
    }
    // Motor não sente o vento.
    assert!((weather_wear_mult(PartType::Engine, vendaval) - weather_wear_mult(PartType::Engine, calmo)).abs() < 1e-9);
}

#[test]
fn clima_redistribui_e_dia_quente_agrava_frio_alivia() {
    let idx = |pt: PartType| PartType::ALL.iter().position(|&x| x == pt).unwrap();
    const WEATHER_COOL: Weather = Weather { wetness: 0.0, temperature: 18.0, humidity: 45.0, wind_kmh: WIND_TYPICAL_KMH };
    let (seco, ts) = fleet_breaks(TRACK_NEUTRO, WEATHER_NEUTRO);
    let (chuva, _tc) = fleet_breaks(TRACK_NEUTRO, WEATHER_RAIN);
    let (forno, tf) = fleet_breaks(TRACK_NEUTRO, WEATHER_HOT);
    let (frio, tfr) = fleet_breaks(TRACK_NEUTRO, WEATHER_COOL);

    println!("\n── Influência do clima (frota realista) ──");
    println!("Taxa total: neutro {ts} · quente {tf} · frio {tfr}");

    // Chuva: eletrônica quebra mais; motor menos.
    assert!(chuva[idx(PartType::Electronics)] > seco[idx(PartType::Electronics)],
        "chuva deveria quebrar mais eletrônica");
    assert!(chuva[idx(PartType::Engine)] < seco[idx(PartType::Engine)],
        "chuva deveria quebrar menos motor");
    // Dia quente (32°) agrava o motor E sobe o total; dia frio (18°) alivia.
    assert!(forno[idx(PartType::Engine)] > seco[idx(PartType::Engine)],
        "dia quente deveria quebrar mais motor");
    assert!(tf > ts, "dia quente deveria subir o total: {tf} vs {ts}");
    assert!(tfr < ts, "dia frio deveria aliviar o total: {tfr} vs {ts}");
}

// -------- Mults por corrida que alimentam a ECONOMIA (persistência) --------

/// Corrida neutra (pista equilibrada, clima neutro) → todos os mults = 1.0: a economia
/// calibrada NÃO muda ao introduzir a modulação por condições.
#[test]
fn conditions_mults_neutro_sao_um() {
    let neutro = conditions_wear_mults((1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0), Weather::NEUTRAL);
    for &pt in &PartType::ALL {
        assert!(
            (neutro[&pt] - 1.0).abs() < 1e-9,
            "{pt:?} neutro deveria ser 1.0, deu {}",
            neutro[&pt]
        );
    }
}

/// O mult combinado leva o clima corretamente por peça: calor agrava motor/arrefecimento;
/// chuva estressa a eletrônica e alivia o motor.
#[test]
fn conditions_mults_levam_o_clima_por_peca() {
    let bal = (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0);
    let quente = conditions_wear_mults(bal, wx(0.0, 32.0));
    assert!(quente[&PartType::Engine] > 1.0, "calor deveria agravar o motor");
    assert!(quente[&PartType::Cooling] > 1.0, "calor deveria agravar o arrefecimento");
    let chuva = conditions_wear_mults(bal, wx(1.0, 20.0));
    assert!(chuva[&PartType::Electronics] > 1.0, "chuva deveria estressar a eletrônica");
    assert!(chuva[&PartType::Engine] < 1.0, "chuva deveria aliviar o motor");
    assert!((chuva[&PartType::Brakes] - 1.0).abs() < 1e-9, "clima não mexe nos freios");
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
    // sintética é arbitrária; o valor fino se calibra no wiring com desgaste real). Com a
    // janela de risco alargada (RISK_OPEN 0.87), o mesmo alívio de 5% cobre proporção menor
    // da zona → a redução relativa cai de ~20% pra ~19%; a banda acompanha.
    assert!(protegido < sem * 85 / 100, "proteção fraca demais: {protegido} vs {sem}");
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

// ========== ENDURO: DNF raro + rampa de fim + economia ==========

/// Eixo 1: no enduro, a MAIORIA dos DNFs de peça não-estrutural vira Grave. Numa parede
/// (peça a 1.06) o motor ainda pode morrer, mas a asa/eletrônica travam em Grave.
#[test]
fn enduro_rebaixa_a_maioria_dos_dnfs_a_grave() {
    // Conta DNFs numa frota de motores no fio, sprint vs enduro.
    let dnf_rate = |is_enduro: bool| -> usize {
        (0..2000u64)
            .filter(|&s| {
                let car = car_with(PartType::Engine, 0.98);
                roll_race_breakdowns_cfg(&car, 40, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], is_enduro, true)
                    .iter()
                    .any(|e| e.is_dnf())
            })
            .count()
    };
    let sprint = dnf_rate(false);
    let enduro = dnf_rate(true);
    assert!(
        enduro * 2 < sprint,
        "enduro deveria dar bem menos DNF de motor (sprint={sprint}, enduro={enduro})"
    );
}

/// Peça não-estrutural (asa) NUNCA tira o carro no enduro, nem na parede.
#[test]
fn enduro_peca_nao_estrutural_nunca_da_dnf() {
    for seed in 0..1000u64 {
        let car = car_with(PartType::FrontWing, 1.08); // dentro da janela → a asa vai quebrar
        for ev in roll_race_breakdowns_cfg(&car, 40, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], true, true) {
            // O invariante é sobre a ASA (não-estrutural). Numa corrida de 40 voltas, peças
            // frágeis frescas (motor/câmbio) também entram na zona e, por serem estruturais,
            // PODEM dar DNF no enduro — então filtramos só os eventos da asa.
            if ev.part == PartType::FrontWing {
                assert!(!ev.is_dnf(), "asa não deveria dar DNF no enduro (seed {seed})");
            }
        }
    }
}

/// Peça ESTRUTURAL (motor) ainda pode dar DNF no enduro (só fica mais raro).
#[test]
fn enduro_estrutural_ainda_pode_dar_dnf() {
    let algum_dnf = (0..1000u64).any(|s| {
        let car = car_with(PartType::Engine, 1.06);
        roll_race_breakdowns_cfg(&car, 40, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], true, true)
            .iter()
            .any(|e| e.is_dnf())
    });
    assert!(algum_dnf, "motor deveria conseguir dar DNF mesmo no enduro");
}

/// Estrutural no enduro mantém só ~[`ENDURO_DNF_SCALE`] da fatia de DNF; não-estrutural
/// nunca vira DNF. O filtro é determinístico e vale igual pra sorte ou parede.
#[test]
fn enduro_estrutural_mantem_so_a_fracao_de_dnf() {
    // Motor (estrutural), todos os `r` na fatia de DNF, sem parede: ~25% permanecem DNF.
    let (light, heavy) = severity_weights(PartType::Engine); // fonte única — não fica stale
    let n = 4000;
    let dnf = (0..n)
        .filter(|k| {
            let r = (light + heavy) + (1.0 - light - heavy) * (*k as f64 / n as f64);
            sample_severity(PartType::Engine, false, r, true) == Severity::Dnf
        })
        .count();
    let frac = dnf as f64 / n as f64;
    assert!((frac - ENDURO_DNF_SCALE).abs() < 0.08, "estrutural deveria manter ~25% de DNF, deu {frac}");
    // Não-estrutural NUNCA vira DNF no enduro, nem na fatia de DNF nem na parede.
    for k in 0..1000 {
        let r = 0.96 + 0.04 * (k as f64 / 1000.0);
        assert_ne!(sample_severity(PartType::FrontWing, false, r, true), Severity::Dnf);
        assert_ne!(sample_severity(PartType::FrontWing, true, r, true), Severity::Dnf);
    }
    // Sem enduro, a fatia inteira do motor é DNF.
    assert_eq!(sample_severity(PartType::Engine, false, 0.95, false), Severity::Dnf);
}

/// Eixo 2 (rampa de fim): a mesma peça no fio quebra MAIS no clímax (progress alto) que no
/// começo (progress baixo) no enduro — a rampa de desgaste morde da metade pro fim.
#[test]
fn enduro_rampa_agrava_o_fim_da_corrida() {
    assert!(enduro_late_ramp(0.0) < enduro_late_ramp(1.0));
    assert!((enduro_late_ramp(0.25) - 1.0).abs() < 1e-9, "1ª metade não deveria ter rampa");
    assert!((enduro_late_ramp(0.5) - 1.0).abs() < 1e-9, "rampa começa na metade");
    assert!((enduro_late_ramp(1.0) - (1.0 + ENDURO_LATE_RAMP_EXTRA)).abs() < 1e-9);
}

// -------- Economia do enduro (custo + alívio de parada) --------

#[test]
fn gate_de_enduro_por_duracao() {
    assert!(!is_enduro_duration(30));
    assert!(!is_enduro_duration(40)); // no gate ainda é sprint
    assert!(is_enduro_duration(41));
    assert!(is_enduro_duration(60));
}

#[test]
fn sprint_nao_tem_sobrecusto_de_peca() {
    for d in [0u8, 15, 25, 30, 40] {
        assert!((enduro_economy_wear_mult(d, 0) - 1.0).abs() < 1e-9, "sprint {d}min deveria ser 1.0");
    }
}

#[test]
fn enduro_custa_mais_e_escala_com_a_duracao() {
    let m60 = enduro_economy_wear_mult(60, 0);
    let m80 = enduro_economy_wear_mult(80, 0);
    assert!((m60 - 2.0).abs() < 1e-9, "60min sem parada deveria ser 2.0×, deu {m60}");
    assert!(m80 > m60, "corrida mais longa deveria custar mais ({m80} vs {m60})");
}

#[test]
fn parada_alivia_o_sobrecusto_com_teto_de_30() {
    let base = enduro_economy_wear_mult(60, 0); // 2.0
    let uma = enduro_economy_wear_mult(60, 1); // −10% do sobrecusto
    let tres = enduro_economy_wear_mult(60, 3); // −30% (teto)
    let cinco = enduro_economy_wear_mult(60, 5); // ainda −30% (teto)
    assert!(uma < base && tres < uma, "cada parada deveria aliviar ({base} → {uma} → {tres})");
    assert!((tres - cinco).abs() < 1e-9, "o alívio deveria travar em 30% (3+ paradas)");
    // O alívio nunca leva abaixo de 1.0 (só corta o sobrecusto).
    assert!(cinco > 1.0, "enduro com muitas paradas ainda custa mais que sprint ({cinco})");
    assert!((enduro_pit_relief(3) - 0.30).abs() < 1e-9);
    assert!((enduro_pit_relief(10) - 0.30).abs() < 1e-9);
}

#[test]
fn paradas_da_ia_sao_modeladas_pela_duracao() {
    assert_eq!(modeled_ai_pits(30), 0, "sprint: sem parada modelada");
    assert_eq!(modeled_ai_pits(60), 2, "60min ≈ 2 stints");
    assert_eq!(modeled_ai_pits(90), 3);
}

// -------- Tenda de durabilidade por NÍVEL (§4.8) --------

#[test]
fn tenda_de_nivel_tem_pico_no_5_e_e_simetrica() {
    use crate::car::wear::level_durability_mult as m;
    assert!(m(5) > m(4) && m(5) > m(6), "pico de vida no nível 5");
    assert!((m(4) - m(6)).abs() < 1e-9, "4 e 6 = normal (iguais)");
    assert!(
        (m(3) - m(7)).abs() < 1e-9 && (m(2) - m(8)).abs() < 1e-9 && (m(1) - m(9)).abs() < 1e-9,
        "curva simétrica em torno do 5"
    );
    assert!(m(1) < m(2) && m(2) < m(3) && m(3) < m(4) && m(4) < m(5), "sobe até o 5");
    assert!(m(9) < m(8) && m(8) < m(7) && m(7) < m(6), "cai depois do 5");
}

#[test]
fn peca_nivel_alto_quebra_mais_que_nivel_5() {
    // MESMA peça (motor), MESMO desgaste de entrada — só o NÍVEL muda. Nível 8 (de ponta,
    // frágil) quebra MAIS que o nível 5 (o ponto confiável): o tradeoff desempenho×confiab.
    let breaks = |level: u8| -> usize {
        (0..1500u64)
            .filter(|&s| {
                let mut car = Car::uniform(level);
                car.set_wear(PartType::Engine, 0.80);
                !roll_race_breakdowns(&car, 18, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[])
                    .is_empty()
            })
            .count()
    };
    let n5 = breaks(5);
    let n8 = breaks(8);
    assert!(n8 > n5 + 50, "nível 8 deveria quebrar bem mais que o 5 (5={n5}, 8={n8})");
}

#[test]
fn categoria_spec_ignora_a_tenda_de_nivel() {
    // Guard do §4.8: sem a tenda (`apply_tent=false`, categoria spec) o NÍVEL não muda a vida
    // — o carro do iniciante (tudo nível 1) NÃO é penalizado. Com a tenda, nível 1 quebra mais.
    let breaks = |level: u8, tent: bool| -> usize {
        (0..1500u64)
            .filter(|&s| {
                let mut car = Car::uniform(level);
                car.set_wear(PartType::Engine, 0.80);
                !roll_race_breakdowns_cfg(
                    &car, 18, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], false, tent,
                )
                .is_empty()
            })
            .count()
    };
    assert!(breaks(1, true) > breaks(5, true), "com tenda, nível 1 (0.60×) quebra mais que 5");
    assert_eq!(breaks(1, false), breaks(5, false), "sem tenda (spec), o nível é irrelevante");
}
