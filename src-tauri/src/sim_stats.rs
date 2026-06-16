//! Harness de estatísticas de simulação (Monte Carlo).
//!
//! Roda N carreiras independentes, cada uma avançando M temporadas completas,
//! e agrega métricas populacionais sobre os pilotos da IA:
//!   • Lesões: % de pilotos que se machucam por temporada, por gravidade.
//!   • Evolução: % que SOBE / DESCE / ESTAGNA (delta de atributos por idade).
//!   • Aposentadorias: % por temporada, idade média, causas.
//!   • Promoções/Rebaixamentos: pilotos que sobem/descem de categoria.
//!
//! Não é um teste de invariante — é um coletor. Sempre "passa"; o valor está
//! no relatório impresso. Rode com:
//!   cargo test --release sim_stats::monte_carlo -- --nocapture --ignored
//!
//! Escala configurável por env:
//!   IRACER_MC_RUNS=10  IRACER_MC_SEASONS=10  cargo test ...

#![cfg(test)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::commands::career::{
    advance_market_week_in_base_dir, advance_season_in_base_dir, create_career_in_base_dir,
    finalize_preseason_in_base_dir, skip_all_pending_races_in_base_dir, CreateCareerInput,
};
use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::market::preseason::PreSeasonPhase;
use crate::promotion::{MovementType, PilotEffectType};

// ── Parâmetros de classificação ───────────────────────────────────────────────

/// 10 atributos "treináveis" — média deles é o nosso indicador de evolução.
const CORE_ATTRS: &[&str] = &[
    "skill",
    "consistencia",
    "racecraft",
    "defesa",
    "ritmo_classificacao",
    "gestao_pneus",
    "adaptabilidade",
    "mentalidade",
    "confianca",
    "smoothness",
];

/// Margem (em pontos de atributo médio) abaixo da qual consideramos "estagnou"
/// numa única temporada (mudanças são graduais).
const STAGNATION_THRESHOLD: f64 = 0.5;

/// Margem para a trajetória de CARREIRA inteira (primeiro vs último ano).
const CAREER_THRESHOLD: f64 = 1.0;

fn age_bucket(age: i32) -> &'static str {
    match age {
        i32::MIN..=20 => "<=20",
        21..=24 => "21-24",
        25..=28 => "25-28",
        29..=32 => "29-32",
        _ => "33+",
    }
}

// ── Snapshot de pilotos ───────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct DriverSnap {
    age: i32,
    overall: f64,
}

/// Snapshot de todos os pilotos da IA ativos (não aposentados).
fn snapshot_drivers(db_path: &Path) -> HashMap<String, DriverSnap> {
    let db = Database::open_existing(db_path).expect("db");
    let cols = CORE_ATTRS.join(", ");
    let sql = format!(
        "SELECT id, idade, {cols} FROM drivers \
         WHERE is_jogador = 0 AND status != 'Aposentado'"
    );
    let mut stmt = db.conn.prepare(&sql).expect("prepare snapshot");
    let n_attrs = CORE_ATTRS.len();
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let age: i32 = row.get(1)?;
            let mut sum = 0.0f64;
            for i in 0..n_attrs {
                let v: f64 = row.get(2 + i)?;
                sum += v;
            }
            Ok((
                id,
                DriverSnap {
                    age,
                    overall: sum / n_attrs as f64,
                },
            ))
        })
        .expect("query snapshot");
    rows.filter_map(Result::ok).collect()
}

/// Mapa id_lesao -> (pilot_id, gravidade) de todas as lesões registradas.
fn snapshot_injuries(db_path: &Path) -> HashMap<String, (String, String)> {
    let db = Database::open_existing(db_path).expect("db");
    let mut stmt = db
        .conn
        .prepare("SELECT id, pilot_id, type FROM injuries")
        .expect("prepare injuries");
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let pilot: String = row.get(1)?;
            let tipo: String = row.get(2)?;
            Ok((id, (pilot, tipo)))
        })
        .expect("query injuries");
    rows.filter_map(Result::ok).collect()
}

/// Stats de desempenho da temporada que acabou de rodar (ler ANTES do advance).
struct SeasonPerf {
    vitorias: i64,
    podios: i64,
    corridas: i64,
    dnfs: i64,
    motivacao: f64,
}

/// Lê as stats `temp_*` dos pilotos da IA (preenchidas pela simulação da temporada).
fn snapshot_season_perf(db_path: &Path) -> Vec<SeasonPerf> {
    let db = Database::open_existing(db_path).expect("db");
    let mut stmt = db
        .conn
        .prepare(
            "SELECT temp_vitorias, temp_podios, \
             temp_corridas, temp_dnfs, motivacao \
             FROM drivers WHERE is_jogador = 0 AND status != 'Aposentado'",
        )
        .expect("prepare perf");
    let rows = stmt
        .query_map([], |row| {
            Ok(SeasonPerf {
                vitorias: row.get(0)?,
                podios: row.get(1)?,
                corridas: row.get(2)?,
                dnfs: row.get(3)?,
                motivacao: row.get(4)?,
            })
        })
        .expect("query perf");
    rows.filter_map(Result::ok).collect()
}

/// Snapshot de uma equipe ativa.
struct TeamSnap {
    id: String,
    categoria: String,
    car_performance: f64,
    confiabilidade: f64,
    reputacao: f64,
    facilities: f64,
    engineering: f64,
    morale: f64,
    cash_balance: f64,
    debt_balance: f64,
    financial_state: String,
}

fn snapshot_teams(db_path: &Path) -> Vec<TeamSnap> {
    let db = Database::open_existing(db_path).expect("db");
    let mut stmt = db
        .conn
        .prepare(
            "SELECT id, categoria, car_performance, confiabilidade, reputacao, facilities, \
             engineering, morale, cash_balance, debt_balance, \
             COALESCE(financial_state,'?') \
             FROM teams WHERE ativa = 1 AND is_player_team = 0",
        )
        .expect("prepare teams");
    let rows = stmt
        .query_map([], |row| {
            Ok(TeamSnap {
                id: row.get(0)?,
                categoria: row.get(1)?,
                car_performance: row.get(2)?,
                confiabilidade: row.get(3)?,
                reputacao: row.get(4)?,
                facilities: row.get(5)?,
                engineering: row.get(6)?,
                morale: row.get(7)?,
                cash_balance: row.get(8)?,
                debt_balance: row.get(9)?,
                financial_state: row.get(10)?,
            })
        })
        .expect("query teams");
    rows.filter_map(Result::ok).collect()
}

/// Lê um contador agregado de eventos de resgate de equipe (0 se ausente).
fn rescue_counter(db_path: &Path, key: &str) -> i64 {
    let db = Database::open_existing(db_path).expect("db");
    db.conn
        .query_row(
            "SELECT value FROM team_rescue_counters WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .unwrap_or(0)
}

/// Títulos de construtores acumulados por equipe (via archive). team_id -> total.
fn constructor_titles_by_team(db_path: &Path) -> Vec<i64> {
    let db = Database::open_existing(db_path).expect("db");
    let mut stmt = db
        .conn
        .prepare(
            "SELECT team_id, SUM(titulos_construtores) \
             FROM team_season_archive GROUP BY team_id",
        )
        .expect("prepare titles");
    let rows = stmt
        .query_map([], |row| {
            let _id: String = row.get(0)?;
            let n: i64 = row.get(1)?;
            Ok(n)
        })
        .expect("query titles");
    rows.filter_map(Result::ok).collect()
}

/// Salários anuais de contratos ativos (IA), por categoria.
fn snapshot_salaries(db_path: &Path) -> Vec<(String, f64)> {
    let db = Database::open_existing(db_path).expect("db");
    let mut stmt = db
        .conn
        .prepare(
            "SELECT c.categoria, c.salario_anual FROM contracts c \
             JOIN drivers d ON d.id = c.piloto_id \
             WHERE c.status = 'Ativo' AND d.is_jogador = 0 AND c.salario_anual > 0",
        )
        .expect("prepare salaries");
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query salaries");
    rows.filter_map(Result::ok).collect()
}

/// Ranking de estados financeiros (0 = pior). Usado para detectar recuperação.
fn state_rank(state: &str) -> u8 {
    match state {
        "collapse" => 0,
        "crisis" => 1,
        "pressured" => 2,
        "stable" => 3,
        "healthy" => 4,
        "elite" => 5,
        _ => 3, // desconhecido tratado como neutro
    }
}

/// Trajetória financeira de uma equipe ao longo das temporadas de uma run.
#[derive(Default)]
struct TeamStateTrack {
    ever_collapse: bool,
    seasons_in_collapse: u32,
    /// Índice da temporada em que entrou em colapso pela 1ª vez.
    first_collapse_season: Option<usize>,
    /// Atingiu "stable" ou melhor DEPOIS de ter colapsado.
    recovered: bool,
    /// Saiu do colapso (qualquer estado > collapse) depois de ter colapsado.
    escaped: bool,
    /// Temporada em que se recuperou (stable+) pela 1ª vez após colapso.
    recover_season: Option<usize>,
    final_state_rank: u8,
}

fn tier_of(cat: &str) -> u8 {
    crate::constants::categories::get_category_config(cat)
        .map(|c| c.tier)
        .unwrap_or(99)
}

// ── Helpers de ciclo de temporada (não-assertivos) ────────────────────────────

fn career_db_path(base_dir: &Path) -> PathBuf {
    let config = AppConfig::load_or_default(base_dir);
    config
        .saves_dir()
        .join("career_001")
        .join("career.db")
}

fn career_dir(base_dir: &Path) -> PathBuf {
    let config = AppConfig::load_or_default(base_dir);
    config.saves_dir().join("career_001")
}

/// Pré-temporada → Temporada, sem asserts (para rodar muitas temporadas seguidas).
fn run_preseason_to_temporada(base_dir: &Path) {
    let db_path = career_db_path(base_dir);
    let career_dir = career_dir(base_dir);

    let _ = advance_market_week_in_base_dir(base_dir, "career_001");

    // Limpa propostas pendentes
    {
        let db = Database::open_existing(&db_path).expect("db");
        let _ = db.conn.execute(
            "UPDATE market_proposals SET status = 'Rejeitada' WHERE status = 'Pendente'",
            [],
        );
    }

    // Força conclusão do plano de pré-temporada
    if let Ok(Some(mut plan)) = crate::market::preseason::load_preseason_plan(&career_dir) {
        plan.state.is_complete = true;
        plan.state.current_week = plan.state.total_weeks + 1;
        plan.state.phase = PreSeasonPhase::Complete;
        plan.state.player_has_pending_proposals = false;
        crate::market::preseason::save_preseason_plan(&career_dir, &plan).expect("salvar plano");
    }

    finalize_preseason_in_base_dir(base_dir, "career_001").expect("finalizar pré-temporada");
}

// ── Acumuladores ──────────────────────────────────────────────────────────────

#[derive(Default)]
struct Totals {
    // Denominador: pilotos-temporada observados (soma de pilotos ativos no início de cada temporada)
    driver_seasons: u64,
    // Evolução (apenas sobreviventes presentes antes e depois)
    survivors: u64,
    sobe: u64,
    desce: u64,
    estagna: u64,
    // Por faixa etária: (sobe, desce, estagna, soma_delta, n)
    by_age: BTreeMap<&'static str, [f64; 5]>, // [sobe, desce, estagna, soma_delta, n]
    // Lesões
    injured_drivers: u64,
    inj_leve: u64,
    inj_moderada: u64,
    inj_grave: u64,
    inj_critica: u64,
    // Aposentadorias
    retirements: u64,
    retire_age_sum: u64,
    retire_reasons: BTreeMap<String, u64>,
    // Promoções/Rebaixamentos de pilotos
    promoted: u64,
    relegated: u64,
    freed_no_license: u64,
    // Por temporada-ciclo: taxas (para média Monte Carlo)
    inj_rate_samples: Vec<f64>,
    retire_rate_samples: Vec<f64>,
    sobe_rate_samples: Vec<f64>,
    desce_rate_samples: Vec<f64>,
    estagna_rate_samples: Vec<f64>,
    // Trajetória de carreira (primeiro vs último overall observado por piloto,
    // entre pilotos vistos em >= 2 temporadas)
    traj_count: u64,
    traj_sobe: u64,
    traj_desce: u64,
    traj_estagna: u64,
    traj_delta_sum: f64,
    traj_by_age: BTreeMap<&'static str, [f64; 5]>, // [sobe, desce, estagna, soma_delta, n]

    // ── Desempenho de pilotos na temporada ──
    total_starts: i64, // soma de corridas
    total_dnfs: i64,
    drivers_raced: u64, // piloto-temporada com >=1 corrida
    win_0: u64,
    win_1_2: u64,
    win_3_5: u64,
    win_6p: u64,
    with_podium: u64,
    motiv_sum: f64,
    motiv_n: u64,
    motiv_lt20: u64, // pilotos em zona de risco
    // Duração de carreira ao aposentar
    retire_career_len_sum: u64,
    retire_career_len_n: u64,

    // ── Equipes ──
    team_seasons: u64,
    fin_state: BTreeMap<String, u64>,
    team_insolvent: u64, // cash<0 ou debt>0
    cash_sum: f64,
    debt_sum: f64,
    car_perf_by_tier: BTreeMap<u8, [f64; 2]>, // [soma, n]
    team_attr_sum: [f64; 5],                   // facilities, engineering, reputacao, morale, confiabilidade
    team_promoted: u64,
    team_relegated: u64,
    // Concentração de títulos de construtores (consolidado por run)
    title_top_share_sum: f64,
    title_runs: u64,
    title_teams_with_any_sum: f64,

    // ── Salários por tier ── [soma, n, min, max]
    salary_by_tier: BTreeMap<u8, [f64; 4]>,

    // ── Recuperação de equipes em colapso (trajetória por equipe) ──
    teams_ever_collapse: u64,
    teams_recovered: u64,      // chegaram a stable+ depois do colapso
    teams_escaped: u64,        // saíram do colapso (qualquer estado melhor) depois
    teams_stuck: u64,          // colapsaram e TERMINARAM em colapso
    recover_time_sum: u64,     // temporadas do colapso até recuperação (stable+)
    recover_time_n: u64,
    collapse_seasons_sum: u64, // total de temporada-em-colapso (para média)
    // Desfecho dos episódios de colapso (contadores de produção)
    episodes_self_rescued: u64, // salvaram-se no all-in, sem venda
    episodes_sold: u64,         // precisaram ser vendidas
}

/// Trajetória de um piloto dentro de uma run: primeiro e último overall observados.
#[derive(Clone, Copy)]
struct Trajectory {
    first_overall: f64,
    last_overall: f64,
    first_age: i32,
    seasons_seen: u32,
}

fn pct(num: u64, den: u64) -> f64 {
    if den == 0 {
        0.0
    } else {
        100.0 * num as f64 / den as f64
    }
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

fn stddev(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = mean(v);
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64;
    var.sqrt()
}

fn minmax(v: &[f64]) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &x in v {
        lo = lo.min(x);
        hi = hi.max(x);
    }
    if v.is_empty() {
        (0.0, 0.0)
    } else {
        (lo, hi)
    }
}

// ── O experimento ─────────────────────────────────────────────────────────────

#[test]
#[ignore = "harness de estatística; rode manualmente com --nocapture --ignored"]
fn monte_carlo() {
    let runs: usize = std::env::var("IRACER_MC_RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let seasons: usize = std::env::var("IRACER_MC_SEASONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  MONTE CARLO — estatísticas de carreira iRacer                ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Runs (carreiras): {runs}   Temporadas por run: {seasons}   Total ciclos: {}", runs * seasons);
    println!("Threshold de estagnação: ±{STAGNATION_THRESHOLD} pts (média de {} atributos)\n", CORE_ATTRS.len());

    let mut t = Totals::default();
    let start = Instant::now();

    for run in 0..runs {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!("iracer_mc_{run}_{nanos}"));
        std::fs::create_dir_all(&base_dir).expect("base dir");

        let input = CreateCareerInput {
            player_name: "MC Bot".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "mazda_rookie".to_string(),
            team_index: 0,
            difficulty: "medio".to_string(),
        };
        create_career_in_base_dir(&base_dir, input).expect("criar carreira");
        let db_path = career_db_path(&base_dir);

        // Trajetória de carreira: primeiro/último overall por piloto nesta run
        let mut traj: HashMap<String, Trajectory> = HashMap::new();
        let mut last_after: HashMap<String, DriverSnap> = HashMap::new();
        // Trajetória financeira por equipe nesta run (para medir recuperação)
        let mut team_states: HashMap<String, TeamStateTrack> = HashMap::new();

        for season in 0..seasons {
            // Snapshot ANTES da temporada
            let before = snapshot_drivers(&db_path);

            // Alimenta a trajetória com o estado de início de temporada
            for (id, snap) in &before {
                traj.entry(id.clone())
                    .and_modify(|tr| {
                        tr.last_overall = snap.overall;
                        tr.seasons_seen += 1;
                    })
                    .or_insert(Trajectory {
                        first_overall: snap.overall,
                        last_overall: snap.overall,
                        first_age: snap.age,
                        seasons_seen: 1,
                    });
            }
            let inj_before: HashSet<String> =
                snapshot_injuries(&db_path).into_keys().collect();
            let active_at_start = before.len() as u64;
            t.driver_seasons += active_at_start;

            // Salários ativos no início da temporada (por tier)
            for (cat, sal) in snapshot_salaries(&db_path) {
                let tier = tier_of(&cat);
                let e = t.salary_by_tier.entry(tier).or_insert([0.0, 0.0, f64::INFINITY, 0.0]);
                e[0] += sal;
                e[1] += 1.0;
                e[2] = e[2].min(sal);
                e[3] = e[3].max(sal);
            }

            // Roda todas as corridas (gera resultados, lesões, finanças)
            skip_all_pending_races_in_base_dir(&base_dir, "career_001")
                .expect("skip all pending");

            // ── Desempenho da temporada (ler ANTES do advance, que arquiva/zera temp_*) ──
            for p in snapshot_season_perf(&db_path) {
                t.total_starts += p.corridas;
                t.total_dnfs += p.dnfs;
                if p.corridas > 0 {
                    t.drivers_raced += 1;
                    match p.vitorias {
                        0 => t.win_0 += 1,
                        1..=2 => t.win_1_2 += 1,
                        3..=5 => t.win_3_5 += 1,
                        _ => t.win_6p += 1,
                    }
                    if p.podios > 0 {
                        t.with_podium += 1;
                    }
                }
                t.motiv_sum += p.motivacao;
                t.motiv_n += 1;
                if p.motivacao < 20.0 {
                    t.motiv_lt20 += 1;
                }
            }

            // Fecha a temporada (aplica growth/decline/lesão/aposentadoria/promoção)
            let result = advance_season_in_base_dir(&base_dir, "career_001")
                .expect("advance season");

            // ── Equipes: snapshot pós-temporada ──
            for tm in snapshot_teams(&db_path) {
                t.team_seasons += 1;
                *t.fin_state.entry(tm.financial_state.clone()).or_insert(0) += 1;
                if tm.cash_balance < 0.0 || tm.debt_balance > 0.0 {
                    t.team_insolvent += 1;
                }
                t.cash_sum += tm.cash_balance;
                t.debt_sum += tm.debt_balance;
                let tier = tier_of(&tm.categoria);
                let e = t.car_perf_by_tier.entry(tier).or_insert([0.0, 0.0]);
                e[0] += tm.car_performance;
                e[1] += 1.0;
                t.team_attr_sum[0] += tm.facilities;
                t.team_attr_sum[1] += tm.engineering;
                t.team_attr_sum[2] += tm.reputacao;
                t.team_attr_sum[3] += tm.morale;
                t.team_attr_sum[4] += tm.confiabilidade;

                // Trajetória financeira: detecta colapso e recuperação posterior
                let rank = state_rank(&tm.financial_state);
                let track = team_states.entry(tm.id.clone()).or_default();
                track.final_state_rank = rank;
                if tm.financial_state == "collapse" {
                    track.seasons_in_collapse += 1;
                    if !track.ever_collapse {
                        track.ever_collapse = true;
                        track.first_collapse_season = Some(season);
                    }
                } else if track.ever_collapse {
                    // Já colapsou antes e agora está fora do colapso
                    track.escaped = true;
                    if rank >= state_rank("stable") && !track.recovered {
                        track.recovered = true;
                        track.recover_season = Some(season);
                    }
                }
            }
            // Movimentos de EQUIPE (promoção/rebaixamento de times)
            for m in &result.promotion_result.movements {
                match m.movement_type {
                    MovementType::Promocao => t.team_promoted += 1,
                    MovementType::Rebaixamento => t.team_relegated += 1,
                }
            }

            // Snapshot DEPOIS
            let after = snapshot_drivers(&db_path);
            last_after = after.clone();
            let inj_after = snapshot_injuries(&db_path);

            // ── Lesões geradas nesta temporada ──
            let mut injured_pilots: HashSet<String> = HashSet::new();
            let mut leve = 0u64;
            let mut moderada = 0u64;
            let mut grave = 0u64;
            let mut critica = 0u64;
            for (id, (pilot, tipo)) in &inj_after {
                if !inj_before.contains(id) {
                    injured_pilots.insert(pilot.clone());
                    match tipo.as_str() {
                        "Leve" => leve += 1,
                        "Moderada" => moderada += 1,
                        "Grave" => grave += 1,
                        "Critica" => critica += 1,
                        _ => {}
                    }
                }
            }
            let injured_this_season = injured_pilots.len() as u64;
            t.injured_drivers += injured_this_season;
            t.inj_leve += leve;
            t.inj_moderada += moderada;
            t.inj_grave += grave;
            t.inj_critica += critica;
            t.inj_rate_samples
                .push(pct(injured_this_season, active_at_start));

            // ── Evolução: sobe / desce / estagna (sobreviventes) ──
            let mut s_sobe = 0u64;
            let mut s_desce = 0u64;
            let mut s_estagna = 0u64;
            for (id, snap_before) in &before {
                if let Some(snap_after) = after.get(id) {
                    let delta = snap_after.overall - snap_before.overall;
                    let bucket = age_bucket(snap_before.age);
                    let entry = t.by_age.entry(bucket).or_insert([0.0; 5]);
                    entry[3] += delta;
                    entry[4] += 1.0;
                    t.survivors += 1;
                    if delta > STAGNATION_THRESHOLD {
                        s_sobe += 1;
                        entry[0] += 1.0;
                    } else if delta < -STAGNATION_THRESHOLD {
                        s_desce += 1;
                        entry[1] += 1.0;
                    } else {
                        s_estagna += 1;
                        entry[2] += 1.0;
                    }
                }
            }
            t.sobe += s_sobe;
            t.desce += s_desce;
            t.estagna += s_estagna;
            let survivors_season = s_sobe + s_desce + s_estagna;
            t.sobe_rate_samples.push(pct(s_sobe, survivors_season));
            t.desce_rate_samples.push(pct(s_desce, survivors_season));
            t.estagna_rate_samples.push(pct(s_estagna, survivors_season));

            // ── Aposentadorias ──
            let retired_this_season = result.retirements.len() as u64;
            t.retirements += retired_this_season;
            for r in &result.retirements {
                t.retire_age_sum += r.age.max(0) as u64;
                *t.retire_reasons.entry(r.reason.clone()).or_insert(0) += 1;
                // Duração de carreira observada nesta simulação (temporadas vistas)
                if let Some(tr) = traj.get(&r.driver_id) {
                    t.retire_career_len_sum += tr.seasons_seen as u64;
                    t.retire_career_len_n += 1;
                }
            }
            t.retire_rate_samples
                .push(pct(retired_this_season, active_at_start));

            // ── Promoções / Rebaixamentos de pilotos ──
            let mut team_dir: HashMap<&str, &MovementType> = HashMap::new();
            for m in &result.promotion_result.movements {
                team_dir.insert(m.team_id.as_str(), &m.movement_type);
            }
            for e in &result.promotion_result.pilot_effects {
                match e.effect {
                    PilotEffectType::MovesWithTeam => match team_dir.get(e.team_id.as_str()) {
                        Some(MovementType::Promocao) => t.promoted += 1,
                        Some(MovementType::Rebaixamento) => t.relegated += 1,
                        None => {}
                    },
                    PilotEffectType::FreedNoLicense => t.freed_no_license += 1,
                    PilotEffectType::FreedPlayerStays => {}
                }
            }

            // Avança para a próxima Temporada (exceto na última iteração)
            if season + 1 < seasons {
                run_preseason_to_temporada(&base_dir);
            }
        }

        // Atualiza o último overall com o estado final pós-última temporada
        for (id, snap) in &last_after {
            if let Some(tr) = traj.get_mut(id) {
                tr.last_overall = snap.overall;
            }
        }

        // Consolida trajetórias de carreira (pilotos vistos em >= 2 temporadas)
        for tr in traj.values() {
            if tr.seasons_seen < 2 {
                continue;
            }
            let delta = tr.last_overall - tr.first_overall;
            t.traj_count += 1;
            t.traj_delta_sum += delta;
            let bucket = age_bucket(tr.first_age);
            let e = t.traj_by_age.entry(bucket).or_insert([0.0; 5]);
            e[3] += delta;
            e[4] += 1.0;
            if delta > CAREER_THRESHOLD {
                t.traj_sobe += 1;
                e[0] += 1.0;
            } else if delta < -CAREER_THRESHOLD {
                t.traj_desce += 1;
                e[1] += 1.0;
            } else {
                t.traj_estagna += 1;
                e[2] += 1.0;
            }
        }

        // Consolida trajetórias financeiras de equipe desta run
        for track in team_states.values() {
            if !track.ever_collapse {
                continue;
            }
            t.teams_ever_collapse += 1;
            t.collapse_seasons_sum += track.seasons_in_collapse as u64;
            if track.escaped {
                t.teams_escaped += 1;
            }
            if track.recovered {
                t.teams_recovered += 1;
                if let (Some(start), Some(end)) =
                    (track.first_collapse_season, track.recover_season)
                {
                    t.recover_time_sum += end.saturating_sub(start) as u64;
                    t.recover_time_n += 1;
                }
            }
            // "Preso": colapsou e terminou a simulação ainda em colapso
            if track.final_state_rank == state_rank("collapse") {
                t.teams_stuck += 1;
            }
        }

        // Desfecho dos episódios de colapso (contadores de produção)
        t.episodes_self_rescued += rescue_counter(&db_path, "self_rescued") as u64;
        t.episodes_sold += rescue_counter(&db_path, "sold") as u64;

        // Concentração de títulos de construtores acumulados ao fim da run
        {
            let titles = constructor_titles_by_team(&db_path);
            let total_titles: i64 = titles.iter().sum();
            if total_titles > 0 {
                let top: i64 = titles.iter().copied().max().unwrap_or(0);
                let with_any = titles.iter().filter(|&&n| n > 0).count();
                t.title_top_share_sum += top as f64 / total_titles as f64;
                t.title_teams_with_any_sum += with_any as f64;
                t.title_runs += 1;
            }
        }

        let _ = std::fs::remove_dir_all(&base_dir);
        println!(
            "  run {}/{} concluída ({:.1}s acumulado)",
            run + 1,
            runs,
            start.elapsed().as_secs_f64()
        );
    }

    // ── Relatório ─────────────────────────────────────────────────────────────
    println!("\n┌─────────────────────────────────────────────────────────────┐");
    println!("│ RESULTADO ({} pilotos-temporada observados)", t.driver_seasons);
    println!("└─────────────────────────────────────────────────────────────┘");

    println!("\n■ LESÕES");
    println!(
        "  % pilotos que se machucam / temporada: {:.1}%  (média/ciclo {:.1}% ± {:.1}, faixa {:.1}–{:.1})",
        pct(t.injured_drivers, t.driver_seasons),
        mean(&t.inj_rate_samples),
        stddev(&t.inj_rate_samples),
        minmax(&t.inj_rate_samples).0,
        minmax(&t.inj_rate_samples).1,
    );
    let inj_total = t.inj_leve + t.inj_moderada + t.inj_grave + t.inj_critica;
    println!("  Lesões geradas (total {inj_total}) por gravidade:");
    println!("    Leve     {:>5}  ({:.1}%)", t.inj_leve, pct(t.inj_leve, inj_total));
    println!("    Moderada {:>5}  ({:.1}%)", t.inj_moderada, pct(t.inj_moderada, inj_total));
    println!("    Grave    {:>5}  ({:.1}%)", t.inj_grave, pct(t.inj_grave, inj_total));
    println!("    Crítica  {:>5}  ({:.1}%)", t.inj_critica, pct(t.inj_critica, inj_total));

    println!("\n■ EVOLUÇÃO (sobreviventes: {} observações)", t.survivors);
    println!(
        "  SOBE    {:.1}%  (média/ciclo {:.1}% ± {:.1})",
        pct(t.sobe, t.survivors),
        mean(&t.sobe_rate_samples),
        stddev(&t.sobe_rate_samples)
    );
    println!(
        "  DESCE   {:.1}%  (média/ciclo {:.1}% ± {:.1})",
        pct(t.desce, t.survivors),
        mean(&t.desce_rate_samples),
        stddev(&t.desce_rate_samples)
    );
    println!(
        "  ESTAGNA {:.1}%  (média/ciclo {:.1}% ± {:.1})",
        pct(t.estagna, t.survivors),
        mean(&t.estagna_rate_samples),
        stddev(&t.estagna_rate_samples)
    );
    println!("\n  Por faixa etária:");
    println!("    idade   | sobe%  desce% estag% | Δ médio");
    println!("    --------+----------------------+--------");
    for (bucket, a) in &t.by_age {
        let n = a[4];
        if n == 0.0 {
            continue;
        }
        println!(
            "    {:<7} | {:>5.1} {:>5.1} {:>5.1} | {:+.2}",
            bucket,
            100.0 * a[0] / n,
            100.0 * a[1] / n,
            100.0 * a[2] / n,
            a[3] / n,
        );
    }

    println!("\n■ DESEMPENHO NA TEMPORADA (pilotos da IA que correram)");
    println!(
        "  Taxa de DNF (abandono): {:.1}%  ({} abandonos em {} largadas)",
        pct(t.total_dnfs as u64, t.total_starts.max(0) as u64),
        t.total_dnfs,
        t.total_starts
    );
    println!("  Distribuição de vitórias por piloto-temporada ({} obs):", t.drivers_raced);
    println!("    0 vitórias    {:>6}  ({:.1}%)", t.win_0, pct(t.win_0, t.drivers_raced));
    println!("    1–2 vitórias  {:>6}  ({:.1}%)", t.win_1_2, pct(t.win_1_2, t.drivers_raced));
    println!("    3–5 vitórias  {:>6}  ({:.1}%)", t.win_3_5, pct(t.win_3_5, t.drivers_raced));
    println!("    6+ vitórias   {:>6}  ({:.1}%)", t.win_6p, pct(t.win_6p, t.drivers_raced));
    println!("    com ≥1 pódio  {:>6}  ({:.1}%)", t.with_podium, pct(t.with_podium, t.drivers_raced));
    if t.motiv_n > 0 {
        println!(
            "  Motivação média: {:.1}/100   |   pilotos em risco (<20): {:.1}%",
            t.motiv_sum / t.motiv_n as f64,
            pct(t.motiv_lt20, t.motiv_n)
        );
    }

    println!(
        "\n■ TRAJETÓRIA DE CARREIRA ({} pilotos vistos em ≥2 temporadas; threshold ±{CAREER_THRESHOLD} pt)",
        t.traj_count
    );
    println!("  Comparando o 1º vs o último ano de cada piloto na simulação:");
    println!("  SOBE    {:.1}%", pct(t.traj_sobe, t.traj_count));
    println!("  DESCE   {:.1}%", pct(t.traj_desce, t.traj_count));
    println!("  ESTAGNA {:.1}%", pct(t.traj_estagna, t.traj_count));
    if t.traj_count > 0 {
        println!(
            "  Δ médio de carreira: {:+.2} pts",
            t.traj_delta_sum / t.traj_count as f64
        );
    }
    println!("\n  Por faixa etária (idade no 1º ano observado):");
    println!("    idade   | sobe%  desce% estag% | Δ médio");
    println!("    --------+----------------------+--------");
    for (bucket, a) in &t.traj_by_age {
        let n = a[4];
        if n == 0.0 {
            continue;
        }
        println!(
            "    {:<7} | {:>5.1} {:>5.1} {:>5.1} | {:+.2}",
            bucket,
            100.0 * a[0] / n,
            100.0 * a[1] / n,
            100.0 * a[2] / n,
            a[3] / n,
        );
    }

    println!("\n■ APOSENTADORIAS");
    println!(
        "  % aposenta / temporada: {:.1}%  (média/ciclo {:.1}% ± {:.1})",
        pct(t.retirements, t.driver_seasons),
        mean(&t.retire_rate_samples),
        stddev(&t.retire_rate_samples)
    );
    if t.retirements > 0 {
        println!(
            "  Idade média de aposentadoria: {:.1} anos",
            t.retire_age_sum as f64 / t.retirements as f64
        );
    }
    if t.retire_career_len_n > 0 {
        println!(
            "  Duração média de carreira observada: {:.1} temporadas",
            t.retire_career_len_sum as f64 / t.retire_career_len_n as f64
        );
    }
    println!("  Causas:");
    for (reason, count) in &t.retire_reasons {
        println!("    {:<28} {:>4}  ({:.1}%)", reason, count, pct(*count, t.retirements));
    }

    println!("\n■ PROMOÇÃO / REBAIXAMENTO (pilotos, % sobre pilotos-temporada)");
    println!(
        "  Promovidos (sobem c/ equipe):     {:>4}  ({:.1}%/temp)",
        t.promoted,
        pct(t.promoted, t.driver_seasons)
    );
    println!(
        "  Rebaixados (descem c/ equipe):    {:>4}  ({:.1}%/temp)",
        t.relegated,
        pct(t.relegated, t.driver_seasons)
    );
    println!(
        "  Liberados (sem licença p/ subir): {:>4}  ({:.1}%/temp)",
        t.freed_no_license,
        pct(t.freed_no_license, t.driver_seasons)
    );

    // ── EQUIPES ────────────────────────────────────────────────────────────────
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  EQUIPES ({} equipe-temporada observadas)", t.team_seasons);
    println!("╚══════════════════════════════════════════════════════════════╝");

    println!("\n■ SAÚDE FINANCEIRA");
    // Ordenar estados do mais saudável ao pior
    let state_order = ["elite", "healthy", "stable", "pressured", "crisis", "collapse"];
    for st in state_order {
        if let Some(c) = t.fin_state.get(st) {
            println!("    {:<10} {:>6}  ({:.1}%)", st, c, pct(*c, t.team_seasons));
        }
    }
    for (st, c) in &t.fin_state {
        if !state_order.contains(&st.as_str()) {
            println!("    {:<10} {:>6}  ({:.1}%)", st, c, pct(*c, t.team_seasons));
        }
    }
    println!(
        "  Equipes com caixa negativo ou dívida: {:.1}%",
        pct(t.team_insolvent, t.team_seasons)
    );
    if t.team_seasons > 0 {
        println!(
            "  Caixa médio: {:>12.0}   |   Dívida média: {:>12.0}",
            t.cash_sum / t.team_seasons as f64,
            t.debt_sum / t.team_seasons as f64
        );
    }

    println!(
        "\n■ RECUPERAÇÃO (trajetória individual: equipes que colapsaram ao menos 1x)"
    );
    println!(
        "  Equipes que entraram em colapso: {}",
        t.teams_ever_collapse
    );
    if t.teams_ever_collapse > 0 {
        println!(
            "    → RECUPERARAM (chegaram a 'stable'+ depois):  {:.1}%",
            pct(t.teams_recovered, t.teams_ever_collapse)
        );
        println!(
            "    → saíram do colapso (qualquer estado melhor):  {:.1}%",
            pct(t.teams_escaped, t.teams_ever_collapse)
        );
        println!(
            "    → PRESAS (terminaram a simulação em colapso):  {:.1}%",
            pct(t.teams_stuck, t.teams_ever_collapse)
        );
        println!(
            "  Temporadas médias em colapso por equipe: {:.1}",
            t.collapse_seasons_sum as f64 / t.teams_ever_collapse as f64
        );
        if t.recover_time_n > 0 {
            println!(
                "  Tempo médio para recuperar (das que recuperaram): {:.1} temporadas",
                t.recover_time_sum as f64 / t.recover_time_n as f64
            );
        }
    }

    let episodes_resolved = t.episodes_self_rescued + t.episodes_sold;
    println!("\n■ DESFECHO DOS EPISÓDIOS DE COLAPSO (resolvidos: {})", episodes_resolved);
    println!(
        "    Salvaram-se sozinhas no all-in (SEM venda): {}  ({:.1}%)",
        t.episodes_self_rescued,
        pct(t.episodes_self_rescued, episodes_resolved)
    );
    println!(
        "    Precisaram ser VENDIDAS (nova diretoria):   {}  ({:.1}%)",
        t.episodes_sold,
        pct(t.episodes_sold, episodes_resolved)
    );

    if t.team_seasons > 0 {
        let n = t.team_seasons as f64;
        println!("\n■ ATRIBUTOS MÉDIOS DE EQUIPE (0-100)");
        println!("    Instalações:    {:.1}", t.team_attr_sum[0] / n);
        println!("    Engenharia:     {:.1}", t.team_attr_sum[1] / n);
        println!("    Reputação:      {:.1}", t.team_attr_sum[2] / n);
        println!("    Confiabilidade: {:.1}", t.team_attr_sum[4] / n);
        println!("    Moral (mult.):  {:.2}", t.team_attr_sum[3] / n);
    }

    println!("\n■ PERFORMANCE DE CARRO por tier de categoria (média)");
    println!("    tier | car_performance | nº amostras");
    println!("    -----+-----------------+------------");
    for (tier, a) in &t.car_perf_by_tier {
        if a[1] == 0.0 {
            continue;
        }
        println!("    {:<4} | {:>15.1} | {:.0}", tier, a[0] / a[1], a[1]);
    }

    println!("\n■ MOVIMENTOS DE EQUIPE (promoção/rebaixamento entre categorias)");
    println!(
        "    Promovidas:  {}   |   Rebaixadas:  {}",
        t.team_promoted, t.team_relegated
    );

    if t.title_runs > 0 {
        println!("\n■ CONCENTRAÇÃO DE TÍTULOS DE CONSTRUTORES (média entre runs)");
        println!(
            "    Fatia da equipe mais vitoriosa: {:.1}% dos títulos",
            100.0 * t.title_top_share_sum / t.title_runs as f64
        );
        println!(
            "    Nº de equipes que ganharam ≥1 título (por run): {:.1}",
            t.title_teams_with_any_sum / t.title_runs as f64
        );
    }

    println!("\n■ SALÁRIOS por tier (anual, contratos ativos da IA)");
    println!("    tier | média        | mínimo      | máximo      | nº");
    println!("    -----+--------------+-------------+-------------+-----");
    for (tier, a) in &t.salary_by_tier {
        if a[1] == 0.0 {
            continue;
        }
        println!(
            "    {:<4} | {:>12.0} | {:>11.0} | {:>11.0} | {:.0}",
            tier,
            a[0] / a[1],
            a[2],
            a[3],
            a[1]
        );
    }

    println!(
        "\nTempo total: {:.1}s\n",
        start.elapsed().as_secs_f64()
    );
}
