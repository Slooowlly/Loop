//! HARNESS DE EXPERIMENTO (throwaway) — Monte Carlo do sistema de "quebra".
//!
//! NÃO é produção: não há wiring no jogo, não dispara comando nenhum. É só para
//! CALIBRAR os números antes de construir o sistema de verdade. Roda milhares de
//! corridas simuladas com o modelo acordado no design e reporta:
//!   - % de quebras por corrida (por tier: rico / médio / pobre / jogador);
//!   - chance por peça (qual peça mais quebra);
//!   - a condição das peças na hora (histograma do desgaste na falha);
//!   - o PORQUÊ (rastros de exemplo: entrou a X%, cruzou a abertura da janela, quebrou…);
//!   - mix de severidade (leve / grave / DNF) + exemplos de comando (!black / !dq);
//!   - varredura do botão global (para escolher o alvo olhando os resultados);
//!   - efeito do TAMANHO da corrida (sprint vs enduro).
//!
//! ## O MODELO É O DE PRODUÇÃO, e isso é o contrato deste arquivo
//!
//! Nada de curva, constante ou tabela de severidade mora aqui. Desgaste por volta, janela de
//! risco, parede, hazard, sorteio de severidade e piso de manutenção saem todos de
//! [`crate::car::breakdown`] e [`crate::car::wear`]. O que é do HARNESS é só a encenação: os
//! tiers de diligência, o sorteio do tamanho da corrida, o intervalo das paradas de enduro, o
//! botão `global` da varredura e a coleta de estatística.
//!
//! Isto não era assim. Até 12/08/2026 o arquivo carregava uma CÓPIA dos parâmetros — janela
//! abrindo a 95%, parede em 105%, hazard linear de 0.05 a 0.28 — que era o retrato do sistema
//! em 2026-07-18 e envelheceu calada quando a produção passou a 90%/120% com dois regimes. O
//! relatório continuou saindo bonito, medindo um jogo que não existe mais. O guard
//! `o_harness_retrata_a_producao_e_nao_uma_copia` é o que impede a divergência de voltar.
//!
//! O único desvio DELIBERADO é o botão `global`, e ele é explícito: a §6 do relatório varre
//! `global` em 0.5/1.0/2.0/4.0 para mostrar o efeito de mexer no risco. `global = 1.0` é a
//! produção; os outros valores são o CONTRAFACTUAL da varredura, e nada mais no arquivo os usa.
//!
//! Rodar (debug):  cargo test -p loop breakdown_sim -- --nocapture
//! A aleatoriedade é semeada (StdRng) só para o EXPERIMENTO ser reproduzível; no jogo
//! real a sorte não é previsível por seed.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::car::breakdown as prod;
use crate::car::parts::PartType;

// ───────────────────────── Parâmetros (os botões de calibração) ─────────────────────────

/// Os botões do experimento. **Nenhum deles é um valor novo**: cada um espelha a fonte de
/// produção, e o único que o relatório varre para longe dela é o `global`.
#[derive(Clone, Copy)]
struct Params {
    /// Comprimento de corrida em VOLTAS que ancora a durabilidade (que é "em corridas").
    /// Espelha [`crate::car::wear::REF_RACE_LAPS`].
    ref_race_laps: f64,
    /// Onde o risco ABRE (fração de desgaste). Espelha `breakdown::WEAR_RISK_OPEN`.
    risk_open: f64,
    /// A PAREDE: falha forçada ao atingir/passar isto. Espelha `breakdown::WEAR_HARD_WALL`.
    hard_wall: f64,
    /// Ruído de sorte no desgaste por volta (±fração). Espelha `breakdown::WEAR_RUIDO`.
    wear_noise: f64,
    /// Botão GLOBAL: multiplica todo o risco da janela. **É o único desvio deliberado do
    /// harness** — `1.0` é a produção, e a §6 do relatório varre o resto de propósito.
    global: f64,
}

impl Default for Params {
    /// O retrato da PRODUÇÃO. Toda linha aqui é uma referência, não um número: é isso que
    /// impede o harness de voltar a medir o sistema de uma versão anterior.
    fn default() -> Self {
        Params {
            ref_race_laps: crate::car::wear::REF_RACE_LAPS,
            risk_open: prod::WEAR_RISK_OPEN,
            hard_wall: prod::WEAR_HARD_WALL,
            wear_noise: prod::WEAR_RUIDO,
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

/// % de desgaste que uma peça acumula por VOLTA.
///
/// Com `ref_race_laps` no valor de produção isto é exatamente
/// [`crate::car::wear::wear_per_lap`] — o guard cobra a igualdade. A fórmula fica escrita
/// porque `ref_race_laps` é um botão do experimento e a função precisa responder a ele.
fn wear_per_lap(pt: PartType, p: &Params) -> f64 {
    let life_laps = pt.durability() as f64 * p.ref_race_laps;
    1.0 / life_laps
}

/// Risco POR VOLTA de a peça quebrar — a curva de PRODUÇÃO, com o botão `global` por cima.
///
/// A fragilidade da peça e os dois regimes (em serviço / sobreuso) já estão dentro de
/// `prod::hazard_por_volta`. O harness tinha uma segunda curva, linear, de uma versão anterior
/// do sistema, e era ela que fazia o relatório mentir.
fn per_lap_hazard(pt: PartType, wear: f64, tier: Tier, p: &Params) -> f64 {
    (prod::hazard_por_volta(pt, wear) * p.global * tier.hazard_mult()).clamp(0.0, 1.0)
}

/// A severidade é a de produção — enum, tabela por peça e regra da parede inclusive.
///
/// O harness mantinha uma cópia dos pesos de antes de a fatia de DNF das peças estruturais ser
/// cortada pela metade, e promovia `Grave→DNF` na parede, coisa que a produção deixou de fazer
/// justamente porque esvaziava a grade. `is_enduro = false` aqui: o cenário longo do relatório
/// é "corrida de 40-60 voltas", e não o enduro com o abrandamento do §7 do design.
use prod::Severity;

/// Recebe a rolagem em vez do `rng` para o guard poder varrer `r` de 0 a 1 e comparar a REGRA,
/// não a fonte do número.
fn sample_severity(pt: PartType, forced: bool, r: f64) -> Severity {
    prod::severidade_da_falha(pt, forced, r, false)
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
/// janela, cada volta é uma rolagem de sorte; na parede a falha é forçada. Uma peça
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
                                      // O piso é o de PRODUÇÃO (`LiveBreakdown::service_pit`), não um número do harness: o 0.80
                                      // que estava aqui deixava passar a faixa que a produção troca.
    const SERVICE_WEAR_FLOOR: f64 = prod::WEAR_PISO_DE_SERVICO;

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
                let severity = sample_severity(pt, forced, rng.gen::<f64>());
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
    /// Histograma do desgaste na falha. As faixas saem de [`bordas_do_histograma`].
    wear_bucket: [u64; 6],
}

/// Bordas do histograma de desgaste na falha, DERIVADAS dos marcos de produção: a abertura da
/// janela, o meio do regime em serviço, o fim da vida nominal, o meio do sobreuso e a parede.
///
/// Estavam cravadas em 0.95/0.97/1.00/1.03/1.05 — a mesma cópia defasada dos parâmetros, só
/// que na saída: com a janela real indo de 90% a 120%, o relatório carimbava metade das falhas
/// na coluna "=105% (parede)" que já não era parede nenhuma.
fn bordas_do_histograma() -> [f64; 5] {
    [
        prod::WEAR_RISK_OPEN,
        (prod::WEAR_RISK_OPEN + prod::WEAR_OVERUSE) / 2.0,
        prod::WEAR_OVERUSE,
        (prod::WEAR_OVERUSE + prod::WEAR_HARD_WALL) / 2.0,
        prod::WEAR_HARD_WALL,
    ]
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
            // Quantas bordas este desgaste já passou = o balde. Abaixo da primeira → 0
            // ("não deveria"); acima da última → 5 (a parede).
            let b = bordas_do_histograma().iter().filter(|&&e| w >= e).count();
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
        "Parâmetros (os de PRODUÇÃO): ref_race_laps={} risco@{:.0}% nominal@{:.0}% parede@{:.0}% ruído±{:.0}% GLOBAL={}",
        p.ref_race_laps,
        p.risk_open * 100.0,
        prod::WEAR_OVERUSE * 100.0,
        p.hard_wall * 100.0,
        p.wear_noise * 100.0,
        p.global
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
    let b = bordas_do_histograma();
    let pct = |x: f64| (x * 100.0).round() as i64;
    let labels = [
        format!("<{}% (não deveria)", pct(b[0])),
        format!("{}-{}%", pct(b[0]), pct(b[1])),
        format!("{}-{}%", pct(b[1]), pct(b[2])),
        format!("{}-{}%", pct(b[2]), pct(b[3])),
        format!("{}-{}%", pct(b[3]), pct(b[4])),
        format!("≥{}% (parede)", pct(b[4])),
    ];
    for (i, lbl) in labels.iter().enumerate() {
        let pct = poor.wear_bucket[i] as f64 / poor.total_failures.max(1) as f64 * 100.0;
        let bar = "█".repeat((pct / 2.0).round() as usize);
        println!("{:<20} {:>6.1}%  {}", lbl, pct, bar);
    }
    println!(
        "Forçadas na parede ({:.0}%): {:.1}%  |  por SORTE na janela: {:.1}%",
        prod::WEAR_HARD_WALL * 100.0,
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
    println!("(muda só QUANDO/SE quebra por sorte na janela; a parede é sempre certa)");
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
                let how = if f.forced { "parede" } else { "sorte" };
                println!(
                    "• {:<11} entrou {:>5.0}% · entrou na janela e quebrou na volta {:>2}/{:<2} a {:>5.0}% ({}, {}) → {}",
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

// ───────────────────────── Guard: o harness retrata a produção ─────────────────────────

/// **O harness não pode voltar a ter uma cópia dos parâmetros.**
///
/// Este arquivo carregou, de 2026-07-18 a 12/08/2026, uma segunda versão do modelo: janela
/// abrindo a 95%, parede em 105% e um hazard linear. A produção mudou para 90%/120% em dois
/// regimes e ninguém tocou aqui — o relatório continuou saindo, bonito e errado, medindo um
/// sistema que já não existia. O modo de falha é esse: não há sintoma, só um número que descreve
/// outro jogo.
///
/// O guard confere as três frentes por onde a cópia voltaria: os marcos do desgaste, a curva de
/// risco e a fórmula de desgaste por volta. O único desvio autorizado é o botão `global` da §6,
/// e ele aparece aqui como a exigência de que `1.0` seja a produção.
#[test]
fn o_harness_retrata_a_producao_e_nao_uma_copia() {
    let p = Params::default();

    assert_eq!(p.risk_open, prod::WEAR_RISK_OPEN, "janela de risco");
    assert_eq!(p.hard_wall, prod::WEAR_HARD_WALL, "parede");
    assert_eq!(p.wear_noise, prod::WEAR_RUIDO, "ruído de desgaste");
    assert_eq!(
        p.ref_race_laps,
        crate::car::wear::REF_RACE_LAPS,
        "corrida de referência"
    );
    assert_eq!(p.global, 1.0, "o retrato de produção não é a varredura");

    for &pt in PartType::ALL.iter() {
        // Desgaste por volta: a fórmula do harness responde a `ref_race_laps`, mas no valor de
        // produção ela tem de dar exatamente o mesmo número da economia.
        assert!(
            (wear_per_lap(pt, &p) - crate::car::wear::wear_per_lap(pt)).abs() < 1e-12,
            "desgaste por volta divergiu em {}",
            pt.as_str()
        );

        // A curva inteira, e não só as pontas: 60 amostras de antes da janela até depois da
        // parede. Um regime a menos aqui é o defeito que existia.
        for k in 0..=60 {
            let wear = 0.80 + k as f64 * 0.01;
            let esperado = prod::hazard_por_volta(pt, wear).clamp(0.0, 1.0);
            let obtido = per_lap_hazard(pt, wear, Tier::Mid, &p);
            assert!(
                (obtido - esperado).abs() < 1e-12,
                "hazard divergiu em {} a {wear:.2}: harness {obtido} × produção {esperado}",
                pt.as_str()
            );
        }
    }

    // O piso de manutenção do enduro também é o de produção — ver `run_race`.
    assert_eq!(prod::WEAR_PISO_DE_SERVICO, 0.60);

    // O histograma acompanha os marcos. Se a parede subir de novo, a última coluna sobe junto,
    // em vez de virar uma faixa que nunca é atingida.
    let b = bordas_do_histograma();
    assert_eq!(b[0], prod::WEAR_RISK_OPEN);
    assert_eq!(b[2], prod::WEAR_OVERUSE);
    assert_eq!(b[4], prod::WEAR_HARD_WALL);
    assert!(b.windows(2).all(|w| w[0] < w[1]), "bordas fora de ordem");
}

/// **A severidade também é a de produção.** O harness promovia `Grave→DNF` na parede e usava a
/// tabela de pesos de antes de a fatia de DNF das estruturais ser cortada pela metade — as duas
/// coisas que a produção desfez porque esvaziavam a grade.
///
/// Varre a rolagem inteira em vez de sortear: uma tabela local reintroduzida aqui divergiria em
/// alguma faixa de `r`, e sorteando é possível não cair nela.
#[test]
fn a_severidade_do_harness_e_a_de_producao() {
    let mut divergiu = Vec::new();
    for &pt in PartType::ALL.iter() {
        for forced in [false, true] {
            for k in 0..1000 {
                let r = k as f64 / 1000.0;
                if sample_severity(pt, forced, r) != prod::severidade_da_falha(pt, forced, r, false)
                {
                    divergiu.push(format!("{} forced={forced} r={r}", pt.as_str()));
                }
            }
        }
    }
    assert!(divergiu.is_empty(), "severidade divergiu em: {divergiu:?}");

    // E a regra que o harness tinha invertida: a parede sobe Leve→Grave e PARA aí. Um `Grave`
    // que virasse `DNF` por ter batido na parede é o que esvaziava a grade.
    for &pt in PartType::ALL.iter() {
        for k in 0..1000 {
            let r = k as f64 / 1000.0;
            if sample_severity(pt, false, r) == Severity::Heavy {
                assert_eq!(
                    sample_severity(pt, true, r),
                    Severity::Heavy,
                    "a parede promoveu Grave→DNF em {} (r={r})",
                    pt.as_str()
                );
            }
        }
    }
}
