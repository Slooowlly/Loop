//! HARNESS DE EXPERIMENTO (throwaway) — Monte Carlo do sistema de "quebra".
//!
//! NÃO é produção: não há wiring no jogo, não dispara comando nenhum. É só para
//! CALIBRAR os números antes de construir o sistema de verdade. Roda milhares de
//! corridas simuladas com o modelo acordado no design e reporta:
//!   - % de quebras por corrida (por tier: rico / médio / pobre / jogador);
//!   - chance por peça (qual peça mais quebra);
//!   - a condição das peças na hora (histograma do desgaste na falha);
//!   - o PORQUÊ (rastros de exemplo: entrou a X%, cruzou 95% na volta N, quebrou…);
//!   - mix de severidade (leve / grave / DNF) + exemplos de comando (!black / !dq);
//!   - varredura do botão global (para escolher o alvo olhando os resultados);
//!   - efeito do TAMANHO da corrida (sprint vs enduro).
//!
//! MODELO (do design travado):
//!   - desgaste sobe % FIXO por volta (corrida longa desgasta mais) + ruído de sorte;
//!   - risco só ABRE a 95% de desgaste; abaixo disso a peça é confiável;
//!   - dentro de [95%, 105%] cada volta é uma rolagem de SORTE (sobe rumo aos 105%);
//!   - 105% é a PAREDE (falha forçada); >100% não quebra automático — aguenta até 105%;
//!   - carro do jicador quebra um pouco menos (mais manutenção + desconto no risco).
//!
//! Rodar (debug):  cargo test -p loop breakdown_sim -- --nocapture
//! A aleatoriedade é semeada (StdRng) só para o EXPERIMENTO ser reproduzível; no jogo
//! real a sorte não é previsível por seed.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::car::parts::PartType;

// ───────────────────────── Parâmetros (os botões de calibração) ─────────────────────────

#[derive(Clone, Copy)]
struct Params {
    /// Comprimento de corrida em VOLTAS que ancora a durabilidade (que é "em corridas").
    /// vida_em_voltas(peça) = durability(peça) * ref_race_laps. Sprint típico.
    ref_race_laps: f64,
    /// Onde o risco ABRE (fração de desgaste).
    risk_open: f64,
    /// A PAREDE: falha forçada ao atingir/passar isto.
    hard_wall: f64,
    /// Risco por volta na borda de baixo da janela (em risk_open).
    hazard_at_open: f64,
    /// Risco por volta na borda de cima (perto da parede). Sobe linear entre as bordas.
    hazard_at_wall: f64,
    /// Ruído de sorte no desgaste por volta (±fração): volta "puxada" desgasta mais.
    wear_noise: f64,
    /// Botão GLOBAL: multiplica todo o risco da janela. É o alvo principal a calibrar.
    global: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            // Desgaste LENTO (decisão de design): a peça leva muitas voltas para atravessar
            // a janela [95%,105%] → a parede quase nunca é atingida numa corrida só; sorte e
            // fim da corrida é que decidem. vida_em_voltas = durability * ref_race_laps.
            // ROTA B (teste): ref ≈ tamanho típico de sprint → cadência de troca IGUAL à
            // economia de hoje (~3 corridas). Janela curta → risco por volta INTENSO pra
            // ainda dar dominância de sorte.
            ref_race_laps: 18.0,
            risk_open: 0.95,
            hard_wall: 1.05,
            hazard_at_open: 0.050,
            hazard_at_wall: 0.280,
            wear_noise: 0.30,
            global: 1.0,
        }
    }
}

// ───────────────────────── Tiers (quem mantém melhor o carro) ─────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Tier {
    Rich,
    Mid,
    Poor,
    Player,
}

impl Tier {
    fn name(self) -> &'static str {
        match self {
            Tier::Rich => "Rico",
            Tier::Mid => "Médio",
            Tier::Poor => "Pobre",
            Tier::Player => "Jogador(pobre)",
        }
    }
    /// Probabilidade de, antes da corrida, TROCAR uma peça que entraria na zona de perigo
    /// (o análogo simplificado do cérebro de manutenção: rico repõe quase sempre; pobre
    /// estica/degrada — deixa passar). É a raiz de por que o pobre quebra mais.
    ///
    /// O JOGADOR herda o tier do time dele: em time rico/médio, use `Tier::Rich`/`Tier::Mid`
    /// (chances IDÊNTICAS às da IA). O tier `Player` aqui modela só o caso que DIFERE — o
    /// jogador num time POBRE, que é protegido (não sofre os 15-30% cheios do pobre).
    fn diligence(self) -> f64 {
        match self {
            Tier::Rich => 0.96,
            Tier::Mid => 0.88,
            Tier::Poor => 0.70,
            // Proteção do jogador em time POBRE: mesmos recursos, MAS o engenheiro é mais
            // cuidadoso (troca mais peças no limite). É via manutenção — que reduz TODAS as
            // quebras, inclusive as da parede — não por mágica no risco. Em time rico/médio o
            // jogador usa Rich/Mid (idêntico à IA), então não há diferença.
            Tier::Player => 0.90,
        }
    }
    /// (Sem desconto mágico no risco — a IA nunca tem, e o jogador é protegido só via
    /// diligência acima. Mantido em 1.0 para todos.)
    fn hazard_mult(self) -> f64 {
        1.0
    }
}

// ───────────────────────── Peça: desgaste por volta, fragilidade, severidade ─────────────────────────

/// % de desgaste que uma peça acumula por VOLTA (fixo — corrida longa desgasta mais).
fn wear_per_lap(pt: PartType, p: &Params) -> f64 {
    let life_laps = pt.durability() as f64 * p.ref_race_laps;
    1.0 / life_laps
}

/// Fragilidade relativa da peça: peça de vida curta falha mais (∝ 1/durabilidade,
/// normalizada para a mais durável = 0.5). Motor/câmbio/asas/freios/suspensão (vida 3)
/// = 1.0; eletrônica (vida 6) = 0.5.
fn fragility(pt: PartType) -> f64 {
    let min_dur = 3.0; // a menor durabilidade do catálogo
    (min_dur / pt.durability() as f64).clamp(0.5, 1.0)
}

/// Risco POR VOLTA de a peça quebrar, dado o desgaste dentro da janela [open, wall].
fn per_lap_hazard(pt: PartType, wear: f64, tier: Tier, p: &Params) -> f64 {
    if wear < p.risk_open {
        return 0.0;
    }
    let t = ((wear - p.risk_open) / (p.hard_wall - p.risk_open)).clamp(0.0, 1.0);
    let base = p.hazard_at_open + (p.hazard_at_wall - p.hazard_at_open) * t;
    (base * fragility(pt) * p.global * tier.hazard_mult()).clamp(0.0, 1.0)
}

#[derive(Clone, Copy, PartialEq)]
enum Severity {
    Light, // leve → !black #N (2-6s)
    Heavy, // grave → !black #N (8-15s)
    Dnf,   // terminal → !dq #N
}

/// Distribuição de severidade por peça (prob. de leve, grave; o resto é DNF).
fn severity_weights(pt: PartType) -> (f64, f64) {
    // Suavizado: menos peças terminam em DNF; a maioria vira penalidade (leve/grave).
    match pt {
        PartType::Engine => (0.20, 0.42), // resto 0.38 DNF (o mais terminal)
        PartType::Gearbox => (0.22, 0.44), // 0.34
        PartType::Suspension => (0.28, 0.47), // 0.25
        PartType::Chassis => (0.25, 0.45), // 0.30
        PartType::Brakes => (0.45, 0.45), // 0.10
        PartType::Cooling => (0.50, 0.42), // 0.08
        PartType::FrontWing => (0.60, 0.35), // 0.05
        PartType::RearWing => (0.60, 0.35), // 0.05
        PartType::Underbody => (0.72, 0.25), // 0.03
        PartType::Sidepods => (0.78, 0.20), // 0.02
        PartType::Electronics => (0.72, 0.25), // 0.03
    }
}

/// Sorteia a severidade. Falha FORÇADA na parede (105%) sobe um degrau (a peça foi ao limite).
fn sample_severity(pt: PartType, forced: bool, rng: &mut StdRng) -> Severity {
    let (light, heavy) = severity_weights(pt);
    let roll: f64 = rng.gen();
    let base = if roll < light {
        Severity::Light
    } else if roll < light + heavy {
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

/// Comando de exemplo que a severidade viraria no iRacing.
fn example_command(sev: Severity, car: u32, rng: &mut StdRng) -> String {
    match sev {
        Severity::Light => format!("!black #{car} {}", rng.gen_range(2..=6)),
        Severity::Heavy => format!("!black #{car} {}", rng.gen_range(8..=15)),
        Severity::Dnf => format!("!dq #{car}"),
    }
}

// ───────────────────────── Simulação ─────────────────────────

struct Failure {
    part: PartType,
    tier: Tier,
    entered_wear: f64, // desgaste da peça ao LARGAR a corrida
    lap: u32,
    total_laps: u32,
    wear_at_fail: f64,
    forced: bool,
    severity: Severity,
}

/// Manutenção antes da corrida: para cada peça que ENTRARIA na zona de perigo nesta corrida
/// (desgaste previsto ≥ risk_open), o time tenta repor (zera) com prob. = diligência. Se não
/// repõe, a peça "roda" (estica/degrada) — carrega o desgaste alto para dentro da corrida.
fn maintain(wear: &mut [f64], laps: u32, tier: Tier, p: &Params, rng: &mut StdRng) {
    for (i, &pt) in PartType::ALL.iter().enumerate() {
        let predicted = wear[i] + wear_per_lap(pt, p) * laps as f64;
        if predicted >= p.risk_open && rng.gen::<f64>() < tier.diligence() {
            wear[i] = 0.0;
        }
    }
}

/// Roda uma corrida volta a volta. Cada peça acumula desgaste (com ruído); ao entrar na
/// janela, cada volta é uma rolagem de sorte; na parede (105%) a falha é forçada. Uma peça
/// que já quebrou para de rodar. Devolve as falhas ocorridas nesta corrida.
fn run_race(
    wear: &mut [f64],
    laps: u32,
    tier: Tier,
    p: &Params,
    rng: &mut StdRng,
    car_num: u32,
) -> Vec<Failure> {
    let mut failures = Vec::new();
    let mut broken = [false; 11];
    let entered: Vec<f64> = wear.to_vec();

    // GAP 2 — manutenção em box durante o ENDURO: corrida longa (≥ min) faz paradas em
    // INTERVALOS regulares (≈ uma distância de sprint cada), zerando as peças mais gastas —
    // é o que os times fazem de verdade num enduro. Sem isso, desgaste-por-volta × corrida
    // longa = DNF absurdo. Sprint é curto demais pra parar.
    const ENDURO_SERVICE_MIN: u32 = 30; // só corridas longas param
    const SERVICE_INTERVAL: u32 = 20; // voltas entre paradas
    const SERVICE_WEAR_FLOOR: f64 = 0.80; // só troca peça JÁ bem gasta (deixa a de meia-vida)

    for lap in 1..=laps {
        if laps >= ENDURO_SERVICE_MIN && lap % SERVICE_INTERVAL == 0 && lap < laps {
            for (i, w) in wear.iter_mut().enumerate() {
                if !broken[i] && *w >= SERVICE_WEAR_FLOOR {
                    *w = 0.0; // peça trocada no pit (custa no jogo real; aqui ignorado)
                }
            }
        }
        for (i, &pt) in PartType::ALL.iter().enumerate() {
            if broken[i] {
                continue;
            }
            // Desgaste da volta com ruído de sorte (±wear_noise).
            let noise = 1.0 + rng.gen_range(-p.wear_noise..p.wear_noise);
            wear[i] += wear_per_lap(pt, p) * noise;

            let mut failed = false;
            let mut forced = false;
            if wear[i] >= p.hard_wall {
                failed = true;
                forced = true;
            } else if wear[i] >= p.risk_open {
                if rng.gen::<f64>() < per_lap_hazard(pt, wear[i], tier, p) {
                    failed = true;
                }
            }
            if failed {
                broken[i] = true;
                let severity = sample_severity(pt, forced, rng);
                failures.push(Failure {
                    part: pt,
                    tier,
                    entered_wear: entered[i],
                    lap,
                    total_laps: laps,
                    wear_at_fail: wear[i],
                    forced,
                    severity,
                });
                // A peça que quebrou é trocada para a próxima corrida.
                wear[i] = 0.0;
                let _ = car_num; // (o nº do carro só é usado nos exemplos de comando)
                                 // Um DNF (!dq) ENCERRA a corrida do carro — ele está fora, as demais
                                 // peças não importam mais. Falha leve/grave: pita, resolve e segue.
                if severity == Severity::Dnf {
                    return failures;
                }
            }
        }
    }
    failures
}

/// Sorteia o comprimento (voltas) de uma corrida do cenário.
fn race_laps(sprint: bool, rng: &mut StdRng) -> u32 {
    if sprint {
        rng.gen_range(14..=22)
    } else {
        rng.gen_range(40..=60)
    }
}

// ───────────────────────── Coleta de estatísticas ─────────────────────────

#[derive(Default)]
struct Stats {
    car_races: u64,
    races_with_break: u64,
    total_failures: u64,
    dnfs: u64,
    forced: u64,
    by_part: [u64; 11],
    sev_light: u64,
    sev_heavy: u64,
    sev_dnf: u64,
    // Histograma do desgaste na falha.
    wear_bucket: [u64; 6], // <0.95 | 0.95-0.97 | 0.97-1.00 | 1.00-1.03 | 1.03-1.05 | =1.05(parede)
}

impl Stats {
    fn record(&mut self, race_failures: &[Failure]) {
        self.car_races += 1;
        if !race_failures.is_empty() {
            self.races_with_break += 1;
        }
        for f in race_failures {
            self.total_failures += 1;
            if f.forced {
                self.forced += 1;
            }
            let idx = PartType::ALL.iter().position(|&x| x == f.part).unwrap();
            self.by_part[idx] += 1;
            match f.severity {
                Severity::Light => self.sev_light += 1,
                Severity::Heavy => self.sev_heavy += 1,
                Severity::Dnf => {
                    self.sev_dnf += 1;
                    self.dnfs += 1;
                }
            }
            let w = f.wear_at_fail;
            let b = if w < 0.95 {
                0
            } else if w < 0.97 {
                1
            } else if w < 1.00 {
                2
            } else if w < 1.03 {
                3
            } else if w < 1.05 {
                4
            } else {
                5
            };
            self.wear_bucket[b] += 1;
        }
    }
    fn break_rate(&self) -> f64 {
        self.races_with_break as f64 / self.car_races.max(1) as f64
    }
    fn dnf_rate(&self) -> f64 {
        self.dnfs as f64 / self.car_races.max(1) as f64
    }
}

/// Roda uma temporada por carro, acumulando estatísticas do tier.
fn simulate_tier(
    tier: Tier,
    cars: u32,
    races: u32,
    sprint: bool,
    p: &Params,
    rng: &mut StdRng,
) -> Stats {
    let mut stats = Stats::default();
    for car in 0..cars {
        let mut wear = [0.0_f64; 11];
        for _ in 0..races {
            let laps = race_laps(sprint, rng);
            maintain(&mut wear, laps, tier, p, rng);
            let fails = run_race(&mut wear, laps, tier, p, rng, car % 60 + 1);
            stats.record(&fails);
        }
    }
    stats
}

// ───────────────────────── Relatório ─────────────────────────

#[test]
fn report() {
    let mut rng = StdRng::seed_from_u64(0xB0_0B_15);
    let p = Params::default();
    let cars = 3000;
    let races = 16;

    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║  EXPERIMENTO: sistema de QUEBRA — Monte Carlo (throwaway)          ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!(
        "Parâmetros: ref_race_laps={} risco@{:.0}% parede@{:.0}% hazard[{:.2}→{:.2}]/volta ruído±{:.0}% GLOBAL={}",
        p.ref_race_laps, p.risk_open * 100.0, p.hard_wall * 100.0,
        p.hazard_at_open, p.hazard_at_wall, p.wear_noise * 100.0, p.global
    );
    println!("Amostra: {cars} carros × {races} corridas (sprint 14-22 voltas) por tier\n");

    // ── 1) Taxa por tier ──
    println!("── 1) TAXA DE QUEBRA POR TIER ─────────────────────────────────────────");
    println!("Alvos DNF/corr: Rico 2-3% · Médio 5-7% · Pobre 15-30% · Jogador-em-time-pobre 5-7%");
    println!("(jogador em time rico/médio = IDÊNTICO à IA daquele tier)");
    println!(
        "{:<15} {:>12} {:>12} {:>14} {:>12}",
        "Tier", "quebra/corr", "DNF/corr", "falhas/corr", "n corr"
    );
    let tiers = [Tier::Rich, Tier::Mid, Tier::Poor, Tier::Player];
    let mut mid_stats = None;
    for &tier in &tiers {
        let s = simulate_tier(tier, cars, races, true, &p, &mut rng);
        println!(
            "{:<15} {:>11.1}% {:>11.1}% {:>14.3} {:>12}",
            tier.name(),
            s.break_rate() * 100.0,
            s.dnf_rate() * 100.0,
            s.total_failures as f64 / s.car_races.max(1) as f64,
            s.car_races
        );
        if tier == Tier::Mid {
            mid_stats = Some(rebuild(tier, cars, races, true, &p));
        }
    }

    // Estatística detalhada no tier POBRE (onde a quebra vive).
    let poor = rebuild(Tier::Poor, cars, races, true, &p);
    println!("\n── 2) CHANCE POR PEÇA (tier Pobre) ────────────────────────────────────");
    let mut parts: Vec<(PartType, u64)> = PartType::ALL
        .iter()
        .enumerate()
        .map(|(i, &pt)| (pt, poor.by_part[i]))
        .collect();
    parts.sort_by(|a, b| b.1.cmp(&a.1));
    for (pt, n) in parts {
        let pct = n as f64 / poor.total_failures.max(1) as f64 * 100.0;
        let bar = "█".repeat((pct / 2.0).round() as usize);
        println!("{:<12} {:>6.1}%  {}", pt.as_str(), pct, bar);
    }

    println!("\n── 3) CONDIÇÃO DA PEÇA NA FALHA (tier Pobre) ──────────────────────────");
    let labels = [
        "<95% (não deveria)",
        "95-97%",
        "97-100%",
        "100-103%",
        "103-105%",
        "=105% (parede)",
    ];
    for (i, lbl) in labels.iter().enumerate() {
        let pct = poor.wear_bucket[i] as f64 / poor.total_failures.max(1) as f64 * 100.0;
        let bar = "█".repeat((pct / 2.0).round() as usize);
        println!("{:<20} {:>6.1}%  {}", lbl, pct, bar);
    }
    println!(
        "Forçadas na parede (105%): {:.1}%  |  por SORTE na janela: {:.1}%",
        poor.forced as f64 / poor.total_failures.max(1) as f64 * 100.0,
        (poor.total_failures - poor.forced) as f64 / poor.total_failures.max(1) as f64 * 100.0
    );

    println!("\n── 4) SEVERIDADE (tier Pobre) ─────────────────────────────────────────");
    let tot = poor.total_failures.max(1) as f64;
    println!(
        "Leve (penalidade curta): {:.1}%",
        poor.sev_light as f64 / tot * 100.0
    );
    println!(
        "Grave (penalidade longa): {:.1}%",
        poor.sev_heavy as f64 / tot * 100.0
    );
    println!(
        "DNF (!dq):                {:.1}%",
        poor.sev_dnf as f64 / tot * 100.0
    );

    // ── 5) PORQUÊ: rastros de exemplo ──
    println!("\n── 5) PORQUÊ / COMO ACONTECEU (10 exemplos, tier Pobre) ───────────────");
    print_traces(Tier::Poor, &p);

    // ── 6) Varredura do GLOBAL ──
    println!("\n── 6) VARREDURA DO BOTÃO GLOBAL (quebra/corr, tier Pobre) ──────────────");
    println!("(muda só QUANDO/SE quebra por sorte na janela; a parede 105% é sempre certa)");
    println!("{:>8} {:>14} {:>12}", "GLOBAL", "quebra/corr", "DNF/corr");
    for &g in &[0.5, 1.0, 2.0, 4.0] {
        let mut pg = p;
        pg.global = g;
        let s = rebuild(Tier::Poor, 1000, races, true, &pg);
        println!(
            "{:>8.1} {:>13.1}% {:>11.1}%",
            g,
            s.break_rate() * 100.0,
            s.dnf_rate() * 100.0
        );
    }

    // ── 7) Efeito do tamanho da corrida ──
    println!("\n── 7) EFEITO DO TAMANHO DA CORRIDA (tier Médio) ───────────────────────");
    let sprint = rebuild_scenario(Tier::Mid, 2000, races, true, &p);
    let enduro = rebuild_scenario(Tier::Mid, 2000, races, false, &p);
    println!(
        "Sprint (14-22 voltas):  quebra/corr {:.1}%  |  DNF/corr {:.1}%",
        sprint.break_rate() * 100.0,
        sprint.dnf_rate() * 100.0
    );
    println!(
        "Enduro (40-60 voltas):  quebra/corr {:.1}%  |  DNF/corr {:.1}%",
        enduro.break_rate() * 100.0,
        enduro.dnf_rate() * 100.0
    );

    let _ = mid_stats;
    println!("\n(experimento reproduzível; ajuste os Params e rode de novo para recalibrar)\n");
}

/// Reexecuta um tier do zero com RNG própria (para relatórios detalhados independentes).
fn rebuild(tier: Tier, cars: u32, races: u32, sprint: bool, p: &Params) -> Stats {
    let mut rng = StdRng::seed_from_u64(0x5EED_00 ^ tier as u64 ^ ((p.global * 1000.0) as u64));
    simulate_tier(tier, cars, races, sprint, p, &mut rng)
}

fn rebuild_scenario(tier: Tier, cars: u32, races: u32, sprint: bool, p: &Params) -> Stats {
    let mut rng = StdRng::seed_from_u64(0x5CE_00 ^ (sprint as u64));
    simulate_tier(tier, cars, races, sprint, p, &mut rng)
}

/// Imprime alguns rastros humanos de "como aconteceu".
fn print_traces(tier: Tier, p: &Params) {
    let mut rng = StdRng::seed_from_u64(0x71ACE5);
    let mut shown = 0;
    let mut car: u32 = 1;
    while shown < 10 {
        let mut wear = [0.0_f64; 11];
        for _ in 0..12 {
            let laps = race_laps(true, &mut rng);
            maintain(&mut wear, laps, tier, p, &mut rng);
            // roda registrando o desgaste de entrada
            let entered: Vec<f64> = wear.to_vec();
            let fails = run_race(&mut wear, laps, tier, p, &mut rng, car);
            for f in &fails {
                if shown >= 10 {
                    break;
                }
                let i = PartType::ALL.iter().position(|&x| x == f.part).unwrap();
                let sev = match f.severity {
                    Severity::Light => "leve",
                    Severity::Heavy => "grave",
                    Severity::Dnf => "DNF",
                };
                let cmd = example_command(f.severity, car % 60 + 1, &mut rng);
                let how = if f.forced { "parede 105%" } else { "sorte" };
                println!(
                    "• {:<11} entrou {:>5.0}% · cruzou 95% e quebrou na volta {:>2}/{:<2} a {:>5.0}% ({}, {}) → {}",
                    f.part.as_str(),
                    entered[i] * 100.0,
                    f.lap,
                    f.total_laps,
                    f.wear_at_fail * 100.0,
                    how,
                    sev,
                    cmd
                );
                shown += 1;
            }
            car += 1;
        }
    }
}
