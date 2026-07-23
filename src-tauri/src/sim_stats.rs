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
        33..=36 => "33-36",
        37..=39 => "37-39",
        40..=42 => "40-42",
        _ => "43+",
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
    classe: String,
    car_performance: f64,
    /// Nível do Carro (1–10) do Sistema de Nível do Carro (tabela team_car).
    car_level: u8,
    /// Carro completo (11 peças) para diagnósticos de distribuição/foco.
    car: Option<crate::car::Car>,
    confiabilidade: f64,
    reputacao: f64,
    facilities: f64,
    engineering: f64,
    morale: f64,
    cash_balance: f64,
    debt_balance: f64,
    financial_state: String,
    foco: String,
}

/// Pares (car_performance do assento, skill do piloto que o ocupa) de cada equipe ativa
/// da IA, com a categoria — para medir a correlação carro↔skill por tier (KPI de deflação).
/// Junta cada assento (piloto_1_id / piloto_2_id) ao skill do piloto.
fn snapshot_grid_pairs(db_path: &Path) -> Vec<(f64, f64, String)> {
    let db = Database::open_existing(db_path).expect("db");
    let mut stmt = db
        .conn
        .prepare(
            "SELECT t.car_performance, d.skill, t.categoria \
             FROM teams t \
             JOIN drivers d ON d.id IN (t.piloto_1_id, t.piloto_2_id) \
             WHERE t.ativa = 1 AND t.is_player_team = 0 \
               AND d.is_jogador = 0 AND d.status != 'Aposentado'",
        )
        .expect("prepare grid pairs");
    let rows = stmt
        .query_map([], |row| {
            let car: f64 = row.get(0)?;
            let skill: f64 = row.get(1)?;
            let categoria: String = row.get(2)?;
            Ok((car, skill, categoria))
        })
        .expect("query grid pairs");
    rows.filter_map(Result::ok).collect()
}

/// Fim de run: rivalidades de EQUIPE vivas (percebida ≥ 20) — contagem, soma/máx da
/// percebida e distribuição por fonte. Confirma "poucas e quentes", não ruído.
fn team_rivalry_snapshot(db_path: &Path) -> (u64, f64, f64, Vec<(String, u64)>) {
    let db = Database::open_existing(db_path).expect("db");
    let mut stmt = db
        .conn
        .prepare(
            "SELECT tipo, historical_intensity * 0.4 + recent_activity * 0.6 AS perceived \
             FROM team_rivalries",
        )
        .expect("prepare team_rivalries");
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))
        .expect("query team_rivalries");
    let mut count = 0u64;
    let mut sum = 0.0;
    let mut max = 0.0f64;
    let mut by_source: BTreeMap<String, u64> = BTreeMap::new();
    for (tipo, perceived) in rows.filter_map(Result::ok) {
        if perceived < 20.0 {
            continue; // só as vivas/relevantes
        }
        count += 1;
        sum += perceived;
        max = max.max(perceived);
        *by_source.entry(tipo).or_insert(0) += 1;
    }
    (count, sum, max, by_source.into_iter().collect())
}

fn snapshot_teams(db_path: &Path) -> Vec<TeamSnap> {
    let db = Database::open_existing(db_path).expect("db");
    let mut snaps: Vec<TeamSnap> = {
        let mut stmt = db
            .conn
            .prepare(
                "SELECT teams.id, categoria, COALESCE(classe,''), car_performance, confiabilidade, \
                 reputacao, facilities, engineering, morale, cash_balance, debt_balance, \
                 COALESCE(financial_state,'?'), COALESCE(tf.foco,'meio_de_grid') \
                 FROM teams LEFT JOIN team_focus tf ON tf.team_id = teams.id \
                 WHERE ativa = 1 AND is_player_team = 0",
            )
            .expect("prepare teams");
        let rows = stmt
            .query_map([], |row| {
                Ok(TeamSnap {
                    id: row.get(0)?,
                    categoria: row.get(1)?,
                    classe: row.get(2)?,
                    car_performance: row.get(3)?,
                    car_level: 1,
                    car: None,
                    confiabilidade: row.get(4)?,
                    reputacao: row.get(5)?,
                    facilities: row.get(6)?,
                    engineering: row.get(7)?,
                    morale: row.get(8)?,
                    cash_balance: row.get(9)?,
                    debt_balance: row.get(10)?,
                    financial_state: row.get(11)?,
                    foco: row.get(12)?,
                })
            })
            .expect("query teams");
        rows.filter_map(Result::ok).collect()
    };
    // 2ª passada: carro completo (tabela team_car) → Nível + peças + shape.
    for snap in snaps.iter_mut() {
        let car = crate::db::queries::team_car::get_team_car(&db.conn, &snap.id)
            .ok()
            .flatten();
        snap.car_level = car.as_ref().map(|c| c.display_level()).unwrap_or(1);
        snap.car = car;
    }
    snaps
}

/// Classifica o FOCO do carro pelo vetor PHA: o atributo dominante (se passar de ~37% do
/// total) ou "balanceado".
fn classify_shape(car: &crate::car::Car) -> &'static str {
    let (p, h, a) = car.pha();
    let total = p + h + a;
    if total <= 0.0 {
        return "balanceado";
    }
    let (pf, hf, af) = (p / total, h / total, a / total);
    let max = pf.max(hf).max(af);
    if max < 0.37 {
        "balanceado"
    } else if pf >= hf && pf >= af {
        "potência"
    } else if hf >= af {
        "handling"
    } else {
        "aceleração"
    }
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

/// Distribuição de títulos de construtores POR CLASSE premium (Pilar D). Retorna,
/// para cada (categoria, classe) premium, a lista de títulos por equipe que ganhou
/// ≥1 — permite medir vencedores únicos e fatia da top NA GRANULARIDADE da dinastia
/// (a métrica mundial dilui isso com os grids de rookie/amador).
/// Estatística de tenure dos vínculos ao fim de uma run:
/// (soma_temporadas, nº_pares, max_temporadas, nº≥3, nº≥4).
fn bond_tenure_snapshot(db_path: &Path) -> (f64, i64, i64, i64, i64) {
    let db = Database::open_existing(db_path).expect("db");
    db.conn
        .query_row(
            "SELECT COALESCE(SUM(temporadas),0), COUNT(*), COALESCE(MAX(temporadas),0),
                    COALESCE(SUM(CASE WHEN temporadas>=3 THEN 1 ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN temporadas>=4 THEN 1 ELSE 0 END),0)
             FROM driver_team_bond",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)? as f64,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        )
        .unwrap_or((0.0, 0, 0, 0, 0))
}

fn premium_class_title_dist(db_path: &Path) -> Vec<Vec<i64>> {
    let db = Database::open_existing(db_path).expect("db");
    let mut stmt = db
        .conn
        .prepare(
            "SELECT categoria, IFNULL(classe,''), team_id, SUM(titulos_construtores) \
             FROM team_season_archive \
             WHERE categoria IN ('production_challenger','gt3','gt4','lmp2','endurance') \
             GROUP BY categoria, classe, team_id HAVING SUM(titulos_construtores) > 0",
        )
        .expect("prepare premium titles");
    let rows = stmt
        .query_map([], |row| {
            let cat: String = row.get(0)?;
            let cls: String = row.get(1)?;
            let n: i64 = row.get(3)?;
            Ok((format!("{cat}|{cls}"), n))
        })
        .expect("query premium titles");
    let mut by_class: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for (key, n) in rows.filter_map(Result::ok) {
        by_class.entry(key).or_default().push(n);
    }
    by_class.into_values().collect()
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

/// Rótulo do tier (nível) para o relatório de funil de carreira.
fn tier_label(tier: u8) -> &'static str {
    match tier {
        0 => "Rookie",
        1 => "Amador",
        2 => "Pro",
        3 => "Super Pro",
        4 => "Master",
        5 => "Elite (LMP2)",
        6 => "Endurance",
        _ => "?",
    }
}

/// Banda pelo PICO de habilidade na carreira (o quão bom o piloto chegou a ser).
fn skill_band_of(overall: f64) -> &'static str {
    if overall >= 65.0 {
        "Elite (65+)"
    } else if overall >= 57.0 {
        "Bom (57-65)"
    } else {
        "Comum (<57)"
    }
}

/// Categoria atual de cada piloto da IA ativo (id → categoria).
fn snapshot_driver_categories(db_path: &Path) -> HashMap<String, String> {
    let db = Database::open_existing(db_path).expect("db");
    let mut stmt = db
        .conn
        .prepare(
            "SELECT id, COALESCE(categoria_atual, '') FROM drivers \
             WHERE is_jogador = 0 AND status != 'Aposentado'",
        )
        .expect("prepare categories");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("query categories");
    rows.filter_map(Result::ok).collect()
}

/// Trajetória de carreira por tier de um piloto dentro de uma run.
struct CareerTrack {
    first_season: usize,
    started_rookie: bool,
    peak_tier: u8,
    /// Maior habilidade (overall) observada na carreira — proxy de "quão bom ficou".
    peak_skill: f64,
    /// Primeira temporada (índice) em que alcançou cada tier.
    reached_at: [Option<usize>; 7],
}

// ── Helpers de ciclo de temporada (não-assertivos) ────────────────────────────

fn career_db_path(base_dir: &Path) -> PathBuf {
    let config = AppConfig::load_or_default(base_dir);
    config.saves_dir().join("career_001").join("career.db")
}

fn career_dir(base_dir: &Path) -> PathBuf {
    let config = AppConfig::load_or_default(base_dir);
    config.saves_dir().join("career_001")
}

/// Pré-temporada → Temporada, sem asserts (para rodar muitas temporadas seguidas).
fn run_preseason_to_temporada(base_dir: &Path) {
    let db_path = career_db_path(base_dir);
    let career_dir = career_dir(base_dir);

    let _ = advance_market_week_in_base_dir(base_dir, "career_001", None);

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
    // #2: perfil de quem larga por falta de motivacao (talento desperdicado?)
    motiv_retire_n: u64,
    motiv_retire_overall_sum: f64,
    motiv_retire_good: u64,

    // ── Equipes ──
    team_seasons: u64,
    fin_state: BTreeMap<String, u64>,
    // Foco da equipe (ideia 4): distribuição das fases (deve espalhar entre os 6,
    // não travar num só).
    focus_dist: BTreeMap<String, u64>,
    team_insolvent: u64, // cash<0 ou debt>0
    cash_sum: f64,
    debt_sum: f64,
    car_perf_by_tier: BTreeMap<u8, [f64; 2]>, // [soma, n]
    // KPI anti-deflação: correlação carro↔skill por tier. Para cada assento ocupado
    // (car_performance do time, skill do piloto), acumula os 6 momentos de Pearson
    // [n, Σcar, Σskill, Σcar², Σskill², Σ(car·skill)]. r alto = o melhor carro fica com o
    // melhor piloto (o que o Item 1/2 deve reforçar); r baixo/zero = grade deflacionada.
    grid_corr_by_tier: BTreeMap<u8, [f64; 6]>,
    // Nível do Carro (1–10) por CATEGORIA: [soma, n, min, max] — mede se o spread emerge
    // e converge perto dos tetos (calibração do Sistema de Nível do Carro).
    car_level_by_category: BTreeMap<String, [f64; 4]>,
    // Nível médio POR PEÇA (só times não-rookie): [soma, n] por nome de peça — mostra em
    // quais peças o cérebro investe.
    part_level_by_type: BTreeMap<String, [f64; 2]>,
    // Distribuição de FOCO do carro (potência/handling/aceleração/balanceado): contagem.
    shape_focus: BTreeMap<String, u64>,
    // Nível médio POR PEÇA quebrado por ARQUÉTIPO de foco: chave "foco|peça" → [soma, n].
    // Mostra COMO cada arquétipo distribui as peças (onde o de-investimento aparece).
    part_level_by_focus: BTreeMap<String, [f64; 2]>,
    // Reputação viva: dispersão por tier [soma, soma², min, max, n] — mede se a
    // reputação deixou de ser plana (semente ~±3) e passou a separar topo/fundo.
    rep_by_tier: BTreeMap<u8, [f64; 5]>,
    // Moral viva: dispersão global [soma, soma², min, max, n] — mede se a moral
    // deixou de ficar travada em 1.0 e passou a variar por forma/treta interna.
    morale_dist: [f64; 5],
    // Rivalidade entre EQUIPES (Fase 2): confirma que existem e são POUCAS (clássicos,
    // não ruído) e que as 4 fontes contribuem. Medido ao fim de cada run (percebida ≥ 20).
    tr_runs: u64,
    tr_count_sum: u64, // rivalidades vivas somadas entre runs
    tr_perceived_sum: f64,
    tr_perceived_max: f64,
    tr_by_source: BTreeMap<String, u64>,
    team_attr_sum: [f64; 5], // facilities, engineering, reputacao, morale, confiabilidade
    team_promoted: u64,
    team_relegated: u64,
    // ── Ideia 1 (soft landing): sobrevivência do promovido ──
    // Onde o carro do promovido aterrissou no campo de destino (pós-promoção):
    promo_landing_gap_sum: f64, // soma (car_promovido − pior_do_campo); <0 = isolado abaixo do lanterna
    promo_landing_n: u64,
    promo_landing_rank_worst: u64, // aterrissou como PIOR do campo (rank 0 vindo de baixo)
    promo_landing_rank_near: u64,  // logo acima do pior (rank 1–2)
    promo_landing_rank_mid: u64,   // meio de tabela+ (rank 3+)
    // Bounce-down: promovida rebaixada logo em seguida (janelas observáveis).
    promo_events_obs1: u64, // promoções com S+1 dentro do horizonte
    promo_bounce_1: u64,    // dessas, rebaixadas já em S+1
    promo_events_obs2: u64, // promoções com S+2 dentro do horizonte
    promo_bounce_2: u64,    // dessas, rebaixadas em S+1 ou S+2
    // Ricochete do rebaixado: caiu e voltou a subir em ≤2 temporadas.
    releg_events_obs2: u64,
    releg_bounce_back_2: u64,
    // Concentração de títulos de construtores (consolidado por run)
    title_top_share_sum: f64,
    title_runs: u64,
    title_teams_with_any_sum: f64,
    // Dinastia por classe premium (Pilar D): vencedores únicos + fatia da top, por classe
    premium_unique_sum: f64,
    premium_top_share_sum: f64,
    premium_class_count: u64,
    // Vínculo piloto-equipe (ideia 4): tenure das duplas ao fim de cada run — mede se
    // times seguram pilotos (duplas de era) SEM congelar o mercado.
    bond_tenure_sum: f64,
    bond_pairs: u64,
    bond_max_tenure: i64,
    bond_ge3: u64,
    bond_ge4: u64,
    // DIAGNÓSTICO SNOWBALL: distribuição do maior "sobe-e-vence" por equipe (comprimento
    // da cadeia de promoções consecutivas temporada+1/tier+1) e o máximo global.
    ladder_chain_hist: BTreeMap<usize, u64>,
    max_ladder_chain: usize,
    // Identidade: nomes das equipes que fazem cadeia >=2 e nomes de todo campeão da
    // rookie (promoção do tier 0). Se poucos nomes dominam → é sempre a mesma equipe.
    climber_names: BTreeMap<String, u64>,
    rookie_champ_names: BTreeMap<String, u64>,

    // ── Salários por tier ── [soma, n, min, max]
    salary_by_tier: BTreeMap<u8, [f64; 4]>,

    // ── Recuperação de equipes em colapso (trajetória por equipe) ──
    teams_ever_collapse: u64,
    teams_recovered: u64,  // chegaram a stable+ depois do colapso
    teams_escaped: u64,    // saíram do colapso (qualquer estado melhor) depois
    teams_stuck: u64,      // colapsaram e TERMINARAM em colapso
    recover_time_sum: u64, // temporadas do colapso até recuperação (stable+)
    recover_time_n: u64,
    collapse_seasons_sum: u64, // total de temporada-em-colapso (para média)
    // Desfecho dos episódios de colapso (contadores de produção)
    episodes_self_rescued: u64,     // salvaram-se no all-in, sem venda
    episodes_sold: u64,             // precisaram ser vendidas
    ownership_events_recorded: u64, // linhas em team_ownership_events (verificação)

    // ── Funil de carreira (cohort que começou no rookie, tier 0) ──
    rookie_cohort: u64,
    // Por tier de destino (1..=6): quantos chegaram (peak >= t) e soma de temporadas até chegar
    reached_tier: [u64; 7],
    time_to_tier_sum: [u64; 7],
    time_to_tier_n: [u64; 7],
    // Impacto do talento: cohort por faixa de habilidade inicial → [n, soma_peak_tier, chegaram_elite(t>=5)]
    skill_band: BTreeMap<&'static str, [u64; 3]>,
    // Aposentadorias por tier (de onde abrem vagas) — contexto p/ emergência
    retire_by_tier: [u64; 7],

    // ── Textura de nomes do Rookie (grid ~24 assentos; mede "mercado pequeno,
    //    nomes reconhecíveis"). Acumulado a partir da 2ª temporada (a 1ª é o mundo
    //    recém-criado, em que todo nome é inédito por definição). ──
    rookie_season_count: u64, // nº de temporadas-rookie observadas (>= temporada 1)
    rookie_obs: u64,          // observações de ocupante de assento rookie
    rookie_fresh: u64,        // estreias NOVAS (id nunca visto antes nesta run)
    rookie_retained: u64,     // mesmo piloto que já estava no rookie na temporada anterior
    rookie_returning: u64,    // conhecido (já visto) mas não estava no rookie ano passado
    rookie_age_sum: u64,
    rookie_age_n: u64,
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
    println!(
        "Runs (carreiras): {runs}   Temporadas por run: {seasons}   Total ciclos: {}",
        runs * seasons
    );
    println!(
        "Threshold de estagnação: ±{STAGNATION_THRESHOLD} pts (média de {} atributos)\n",
        CORE_ATTRS.len()
    );

    let mut t = Totals::default();
    crate::market::pipeline::EMERGENCY_PROMOTIONS.store(0, std::sync::atomic::Ordering::Relaxed);
    crate::market::pipeline::EMERGENCY_ROOKIES.store(0, std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut paths) = crate::market::pipeline::EMERGENCY_PROMO_PATHS.lock() {
        paths.clear();
    }
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
        // Trajetória de tier (carreira) por piloto nesta run
        let mut careers: HashMap<String, CareerTrack> = HashMap::new();
        // Textura de nomes do Rookie: ids já vistos nesta run e o grid rookie anterior.
        let mut seen_ever: HashSet<String> = HashSet::new();
        let mut prev_rookie: HashSet<String> = HashSet::new();
        // DIAGNÓSTICO SNOWBALL: promoções (campeonatos) por equipe nesta run, como
        // (temporada, tier_de_origem). Uma equipe que sobe a escada ganhando ano
        // após ano vira uma cadeia de (s, t), (s+1, t+1), (s+2, t+2)...
        let mut promo_by_team: HashMap<String, Vec<(usize, u8)>> = HashMap::new();
        let mut team_name_of: HashMap<String, String> = HashMap::new();
        // Ideia 1: eventos de promoção/rebaixamento por equipe nesta run, para medir
        // bounce-down (promovida cai logo) e ricochete (rebaixada volta logo).
        let mut promoted_at: Vec<(String, usize)> = Vec::new();
        let mut relegated_seasons: HashMap<String, Vec<usize>> = HashMap::new();

        for season in 0..seasons {
            // Snapshot ANTES da temporada
            let before = snapshot_drivers(&db_path);

            // ── Textura de nomes do Rookie: estreia nova vs. retido vs. conhecido
            //    retornando. Medido no início da temporada (o grid já foi montado pela
            //    pré-temporada anterior). A 1ª temporada (mundo novo) é ignorada. ──
            let cats_now = snapshot_driver_categories(&db_path);
            let mut rookie_now: HashSet<String> = HashSet::new();
            for (id, cat) in &cats_now {
                if tier_of(cat) != 0 {
                    continue;
                }
                rookie_now.insert(id.clone());
                if season > 0 {
                    t.rookie_obs += 1;
                    if !seen_ever.contains(id) {
                        t.rookie_fresh += 1;
                    } else if prev_rookie.contains(id) {
                        t.rookie_retained += 1;
                    } else {
                        t.rookie_returning += 1;
                    }
                    if let Some(s) = before.get(id) {
                        t.rookie_age_sum += s.age.max(0) as u64;
                        t.rookie_age_n += 1;
                    }
                }
            }
            if season > 0 {
                t.rookie_season_count += 1;
            }
            for id in cats_now.keys() {
                seen_ever.insert(id.clone());
            }
            prev_rookie = rookie_now;

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
            let inj_before: HashSet<String> = snapshot_injuries(&db_path).into_keys().collect();
            let active_at_start = before.len() as u64;
            t.driver_seasons += active_at_start;

            // Salários ativos no início da temporada (por tier)
            for (cat, sal) in snapshot_salaries(&db_path) {
                let tier = tier_of(&cat);
                let e = t
                    .salary_by_tier
                    .entry(tier)
                    .or_insert([0.0, 0.0, f64::INFINITY, 0.0]);
                e[0] += sal;
                e[1] += 1.0;
                e[2] = e[2].min(sal);
                e[3] = e[3].max(sal);
            }

            // Roda todas as corridas (gera resultados, lesões, finanças)
            skip_all_pending_races_in_base_dir(&base_dir, "career_001").expect("skip all pending");

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
            let result =
                advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

            // ── KPI anti-deflação: correlação carro↔skill por tier ──
            for (car, skill, categoria) in snapshot_grid_pairs(&db_path) {
                let m = t
                    .grid_corr_by_tier
                    .entry(tier_of(&categoria))
                    .or_insert([0.0; 6]);
                m[0] += 1.0;
                m[1] += car;
                m[2] += skill;
                m[3] += car * car;
                m[4] += skill * skill;
                m[5] += car * skill;
            }

            // ── Equipes: snapshot pós-temporada ──
            let team_snaps = snapshot_teams(&db_path);
            // Mapa id → (categoria, classe, car) para medir onde o promovido aterrissou.
            let car_by_id: HashMap<String, (String, String, f64)> = team_snaps
                .iter()
                .map(|tm| {
                    (
                        tm.id.clone(),
                        (tm.categoria.clone(), tm.classe.clone(), tm.car_performance),
                    )
                })
                .collect();
            for tm in &team_snaps {
                t.team_seasons += 1;
                *t.fin_state.entry(tm.financial_state.clone()).or_insert(0) += 1;
                *t.focus_dist.entry(tm.foco.clone()).or_insert(0) += 1;
                if tm.cash_balance < 0.0 || tm.debt_balance > 0.0 {
                    t.team_insolvent += 1;
                }
                t.cash_sum += tm.cash_balance;
                t.debt_sum += tm.debt_balance;
                let tier = tier_of(&tm.categoria);
                let e = t.car_perf_by_tier.entry(tier).or_insert([0.0, 0.0]);
                e[0] += tm.car_performance;
                e[1] += 1.0;
                // Nível do Carro (1–10) por categoria.
                let cl = t
                    .car_level_by_category
                    .entry(tm.categoria.clone())
                    .or_insert([0.0, 0.0, f64::MAX, f64::MIN]);
                cl[0] += tm.car_level as f64;
                cl[1] += 1.0;
                cl[2] = cl[2].min(tm.car_level as f64);
                cl[3] = cl[3].max(tm.car_level as f64);
                // Distribuição por peça + foco do carro (só categorias não-spec: teto > 1).
                if let Some(car) = &tm.car {
                    if crate::car::cost::category_ceiling(&tm.categoria) > 1 {
                        let focus = classify_shape(car);
                        for part in &car.parts {
                            let pe = t
                                .part_level_by_type
                                .entry(part.part_type.as_str().to_string())
                                .or_insert([0.0, 0.0]);
                            pe[0] += part.level as f64;
                            pe[1] += 1.0;
                            let fe = t
                                .part_level_by_focus
                                .entry(format!("{focus}|{}", part.part_type.as_str()))
                                .or_insert([0.0, 0.0]);
                            fe[0] += part.level as f64;
                            fe[1] += 1.0;
                        }
                        *t.shape_focus.entry(focus.to_string()).or_insert(0) += 1;
                    }
                }
                let r = t
                    .rep_by_tier
                    .entry(tier)
                    .or_insert([0.0, 0.0, f64::MAX, f64::MIN, 0.0]);
                r[0] += tm.reputacao;
                r[1] += tm.reputacao * tm.reputacao;
                r[2] = r[2].min(tm.reputacao);
                r[3] = r[3].max(tm.reputacao);
                r[4] += 1.0;
                t.team_attr_sum[0] += tm.facilities;
                t.team_attr_sum[1] += tm.engineering;
                t.team_attr_sum[2] += tm.reputacao;
                t.team_attr_sum[3] += tm.morale;
                t.team_attr_sum[4] += tm.confiabilidade;
                let md = &mut t.morale_dist;
                if md[4] == 0.0 {
                    md[2] = tm.morale;
                    md[3] = tm.morale;
                }
                md[0] += tm.morale;
                md[1] += tm.morale * tm.morale;
                md[2] = md[2].min(tm.morale);
                md[3] = md[3].max(tm.morale);
                md[4] += 1.0;

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
                    MovementType::Promocao => {
                        t.team_promoted += 1;
                        promo_by_team
                            .entry(m.team_id.clone())
                            .or_default()
                            .push((season, tier_of(&m.from_category)));
                        team_name_of.insert(m.team_id.clone(), m.team_name.clone());
                        if tier_of(&m.from_category) == 0 {
                            *t.rookie_champ_names.entry(m.team_name.clone()).or_insert(0) += 1;
                        }
                        promoted_at.push((m.team_id.clone(), season));
                        // Onde o carro do promovido aterrissou no campo de destino
                        // (mesma categoria+classe, excluindo ele próprio; a rebaixada
                        // já saiu na troca). Mede se entra isolado em último (gap<0)
                        // ou logo acima do lanterna (Ideia 1: gap≈margem).
                        if let Some((cat, cls, car)) = car_by_id.get(&m.team_id) {
                            let others: Vec<f64> = car_by_id
                                .iter()
                                .filter(|(id, (c, cl, _))| {
                                    id.as_str() != m.team_id && c == cat && cl == cls
                                })
                                .map(|(_, (_, _, cp))| *cp)
                                .collect();
                            if !others.is_empty() {
                                let worst =
                                    others.iter().copied().fold(f64::INFINITY, f64::min);
                                t.promo_landing_gap_sum += car - worst;
                                t.promo_landing_n += 1;
                                let rank_from_bottom =
                                    others.iter().filter(|&&c| c < *car).count();
                                match rank_from_bottom {
                                    0 => t.promo_landing_rank_worst += 1,
                                    1..=2 => t.promo_landing_rank_near += 1,
                                    _ => t.promo_landing_rank_mid += 1,
                                }
                            }
                        }
                    }
                    MovementType::Rebaixamento => {
                        t.team_relegated += 1;
                        relegated_seasons
                            .entry(m.team_id.clone())
                            .or_default()
                            .push(season);
                    }
                }
            }

            // Snapshot DEPOIS
            let after = snapshot_drivers(&db_path);
            last_after = after.clone();
            let inj_after = snapshot_injuries(&db_path);

            // ── Funil de carreira: tier de cada piloto pós-promoção ──
            for (id, categoria) in snapshot_driver_categories(&db_path) {
                let tier = tier_of(&categoria);
                if tier > 6 {
                    continue; // categoria desconhecida
                }
                let overall = after.get(&id).map(|s| s.overall).unwrap_or(0.0);
                let track = careers.entry(id).or_insert_with(|| CareerTrack {
                    first_season: season,
                    started_rookie: tier == 0,
                    peak_tier: tier,
                    peak_skill: overall,
                    reached_at: [None; 7],
                });
                track.peak_tier = track.peak_tier.max(tier);
                track.peak_skill = track.peak_skill.max(overall);
                if track.reached_at[tier as usize].is_none() {
                    track.reached_at[tier as usize] = Some(season);
                }
            }

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
            t.estagna_rate_samples
                .push(pct(s_estagna, survivors_season));

            // ── Aposentadorias ──
            let retired_this_season = result.retirements.len() as u64;
            t.retirements += retired_this_season;
            for r in &result.retirements {
                t.retire_age_sum += r.age.max(0) as u64;
                *t.retire_reasons.entry(r.reason.clone()).or_insert(0) += 1;
                let rt = r.categoria.as_deref().map(tier_of).unwrap_or(99);
                if rt <= 6 {
                    t.retire_by_tier[rt as usize] += 1;
                }
                // Duração de carreira observada nesta simulação (temporadas vistas)
                if let Some(tr) = traj.get(&r.driver_id) {
                    t.retire_career_len_sum += tr.seasons_seen as u64;
                    t.retire_career_len_n += 1;
                    if r.reason.contains("falta de motivacao") {
                        t.motiv_retire_n += 1;
                        t.motiv_retire_overall_sum += tr.last_overall;
                        if tr.last_overall >= 60.0 {
                            t.motiv_retire_good += 1;
                        }
                    }
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

        // DIAGNÓSTICO SNOWBALL: maior cadeia de promoções consecutivas (temporada+1,
        // tier+1) por equipe = "sobe a escada ganhando todo ano". Cadeia de comprimento
        // 1 = campeã uma vez. 3+ = exatamente o bug relatado (rookie→cup→production...).
        for (team_id, promos) in &promo_by_team {
            let mut seq = promos.clone();
            seq.sort_by_key(|&(s, tier)| (s, tier));
            let mut best = 1usize;
            let mut cur = 1usize;
            for w in seq.windows(2) {
                let (s0, t0) = w[0];
                let (s1, t1) = w[1];
                if s1 == s0 + 1 && t1 == t0 + 1 {
                    cur += 1;
                    best = best.max(cur);
                } else {
                    cur = 1;
                }
            }
            *t.ladder_chain_hist.entry(best).or_insert(0) += 1;
            t.max_ladder_chain = t.max_ladder_chain.max(best);
            // Identidade dos "climbers" (cadeia >= 2): mostra se é sempre a mesma equipe.
            if best >= 2 {
                if let Some(name) = team_name_of.get(team_id) {
                    *t.climber_names.entry(name.clone()).or_insert(0) += 1;
                }
            }
        }

        // Ideia 1: bounce-down do promovido — promovida em S rebaixada em S+1 (ou S+2).
        // Só conta promoções cuja janela é observável dentro do horizonte da run.
        for (team_id, s) in &promoted_at {
            let rels = relegated_seasons.get(team_id);
            if s + 1 < seasons {
                t.promo_events_obs1 += 1;
                if rels.is_some_and(|v| v.contains(&(s + 1))) {
                    t.promo_bounce_1 += 1;
                }
            }
            if s + 2 < seasons {
                t.promo_events_obs2 += 1;
                if rels.is_some_and(|v| v.iter().any(|&r| r == s + 1 || r == s + 2)) {
                    t.promo_bounce_2 += 1;
                }
            }
        }
        // Ricochete do rebaixado — rebaixada em S volta a ser promovida em ≤2 temporadas.
        for (team_id, seasons_down) in &relegated_seasons {
            let ups: Vec<usize> = promoted_at
                .iter()
                .filter(|(id, _)| id == team_id)
                .map(|(_, s)| *s)
                .collect();
            for &s in seasons_down {
                if s + 2 < seasons {
                    t.releg_events_obs2 += 1;
                    if ups.iter().any(|&u| u == s + 1 || u == s + 2) {
                        t.releg_bounce_back_2 += 1;
                    }
                }
            }
        }

        // Consolida o funil de carreira do cohort que começou no rookie.
        for track in careers.values() {
            if !track.started_rookie {
                continue;
            }
            t.rookie_cohort += 1;
            let band = skill_band_of(track.peak_skill);
            let entry = t.skill_band.entry(band).or_insert([0; 3]);
            entry[0] += 1;
            entry[1] += track.peak_tier as u64;
            if track.peak_tier >= 5 {
                entry[2] += 1;
            }
            for tier in 1..=6usize {
                if track.peak_tier as usize >= tier {
                    t.reached_tier[tier] += 1;
                }
                if let Some(reached) = track.reached_at[tier] {
                    t.time_to_tier_sum[tier] += reached.saturating_sub(track.first_season) as u64;
                    t.time_to_tier_n[tier] += 1;
                }
            }
        }

        // Desfecho dos episódios de colapso (contadores de produção)
        t.episodes_self_rescued += rescue_counter(&db_path, "self_rescued") as u64;
        t.episodes_sold += rescue_counter(&db_path, "sold") as u64;
        {
            let db = Database::open_existing(&db_path).expect("db");
            let n: i64 = db
                .conn
                .query_row("SELECT COUNT(*) FROM team_ownership_events", [], |r| {
                    r.get(0)
                })
                .unwrap_or(0);
            t.ownership_events_recorded += n as u64;
        }

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
            // Dinastia por classe premium (granularidade correta do Pilar D).
            for class_titles in premium_class_title_dist(&db_path) {
                let total: i64 = class_titles.iter().sum();
                if total > 0 {
                    let top = class_titles.iter().copied().max().unwrap_or(0);
                    t.premium_top_share_sum += top as f64 / total as f64;
                    t.premium_unique_sum += class_titles.len() as f64;
                    t.premium_class_count += 1;
                }
            }
        }

        // Tenure de vínculo ao fim da run (ideia 4): duplas de era vs mercado congelado.
        {
            let (sum, count, max_t, ge3, ge4) = bond_tenure_snapshot(&db_path);
            if count > 0 {
                t.bond_tenure_sum += sum;
                t.bond_pairs += count as u64;
                t.bond_max_tenure = t.bond_max_tenure.max(max_t);
                t.bond_ge3 += ge3 as u64;
                t.bond_ge4 += ge4 as u64;
            }
        }

        // Rivalidade entre EQUIPES ao fim da run (Fase 2): magnitude e fontes.
        {
            let (count, sum, max, by_source) = team_rivalry_snapshot(&db_path);
            t.tr_runs += 1;
            t.tr_count_sum += count;
            t.tr_perceived_sum += sum;
            t.tr_perceived_max = t.tr_perceived_max.max(max);
            for (src, n) in by_source {
                *t.tr_by_source.entry(src).or_insert(0) += n;
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
    println!(
        "│ RESULTADO ({} pilotos-temporada observados)",
        t.driver_seasons
    );
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
    println!(
        "    Leve     {:>5}  ({:.1}%)",
        t.inj_leve,
        pct(t.inj_leve, inj_total)
    );
    println!(
        "    Moderada {:>5}  ({:.1}%)",
        t.inj_moderada,
        pct(t.inj_moderada, inj_total)
    );
    println!(
        "    Grave    {:>5}  ({:.1}%)",
        t.inj_grave,
        pct(t.inj_grave, inj_total)
    );
    println!(
        "    Crítica  {:>5}  ({:.1}%)",
        t.inj_critica,
        pct(t.inj_critica, inj_total)
    );

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
    println!(
        "  Distribuição de vitórias por piloto-temporada ({} obs):",
        t.drivers_raced
    );
    println!(
        "    0 vitórias    {:>6}  ({:.1}%)",
        t.win_0,
        pct(t.win_0, t.drivers_raced)
    );
    println!(
        "    1–2 vitórias  {:>6}  ({:.1}%)",
        t.win_1_2,
        pct(t.win_1_2, t.drivers_raced)
    );
    println!(
        "    3–5 vitórias  {:>6}  ({:.1}%)",
        t.win_3_5,
        pct(t.win_3_5, t.drivers_raced)
    );
    println!(
        "    6+ vitórias   {:>6}  ({:.1}%)",
        t.win_6p,
        pct(t.win_6p, t.drivers_raced)
    );
    println!(
        "    com ≥1 pódio  {:>6}  ({:.1}%)",
        t.with_podium,
        pct(t.with_podium, t.drivers_raced)
    );
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
        println!(
            "    {:<28} {:>4}  ({:.1}%)",
            reason,
            count,
            pct(*count, t.retirements)
        );
    }
    if t.motiv_retire_n > 0 {
        println!(
            "  Perfil de quem largou por MOTIVACAO: overall medio {:.1} | 'bons' (>=60): {} de {} ({:.1}%)",
            t.motiv_retire_overall_sum / t.motiv_retire_n as f64,
            t.motiv_retire_good,
            t.motiv_retire_n,
            pct(t.motiv_retire_good, t.motiv_retire_n)
        );
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
    println!(
        "║  EQUIPES ({} equipe-temporada observadas)",
        t.team_seasons
    );
    println!("╚══════════════════════════════════════════════════════════════╝");

    println!("\n■ SAÚDE FINANCEIRA");
    // Ordenar estados do mais saudável ao pior
    let state_order = [
        "elite",
        "healthy",
        "stable",
        "pressured",
        "crisis",
        "collapse",
    ];
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

    println!("\n■ RECUPERAÇÃO (trajetória individual: equipes que colapsaram ao menos 1x)");
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
    println!(
        "\n■ DESFECHO DOS EPISÓDIOS DE COLAPSO (resolvidos: {})",
        episodes_resolved
    );
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
    println!(
        "    Eventos de venda gravados na ficha:         {}  (deve bater com vendidas)",
        t.ownership_events_recorded
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

    println!("\n■ NÍVEL DO CARRO (1–10) por categoria — Sistema de Nível do Carro");
    println!("    Alvo: a média deveria convergir perto do TETO da categoria, com spread (min<max).");
    println!("    categoria             | teto | média | min | max | nº");
    println!("    ----------------------+------+-------+-----+-----+-----");
    for (cat, a) in &t.car_level_by_category {
        if a[1] == 0.0 {
            continue;
        }
        let ceiling = crate::car::cost::category_ceiling(cat);
        println!(
            "    {:<21} | {:>4} | {:>5.1} | {:>3.0} | {:>3.0} | {:.0}",
            cat, ceiling, a[0] / a[1], a[2], a[3], a[1]
        );
    }

    // ── (1) Distribuição por peça ──
    println!("\n■ NÍVEL MÉDIO POR PEÇA (times não-spec) — onde o cérebro investe");
    println!("    peça          | nível médio | nº");
    println!("    --------------+-------------+------");
    let mut parts: Vec<(&String, &[f64; 2])> = t.part_level_by_type.iter().collect();
    parts.sort_by(|a, b| {
        let av = if a.1[1] > 0.0 { a.1[0] / a.1[1] } else { 0.0 };
        let bv = if b.1[1] > 0.0 { b.1[0] / b.1[1] } else { 0.0 };
        bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
    });
    for (name, a) in parts {
        if a[1] == 0.0 {
            continue;
        }
        println!("    {:<13} | {:>11.2} | {:.0}", name, a[0] / a[1], a[1]);
    }

    // ── (2) Foco do carro ──
    println!("\n■ FOCO DO CARRO (times não-spec) — tendência de shape");
    let focus_total: u64 = t.shape_focus.values().sum();
    if focus_total > 0 {
        for (focus, n) in &t.shape_focus {
            println!(
                "    {:<12} {:>6} ({:.1}%)",
                focus,
                n,
                *n as f64 / focus_total as f64 * 100.0
            );
        }
    }

    // ── (2b) Distribuição por peça QUEBRADA por arquétipo — onde o foco/de-investimento aparece ──
    println!("\n■ DISTRIBUIÇÃO POR PEÇA × ARQUÉTIPO (nível médio; onde cada foco investe/larga)");
    {
        use crate::car::PartType;
        let focuses = ["balanceado", "potência", "handling", "aceleração"];
        let avg = |focus: &str, part: &str| -> Option<f64> {
            t.part_level_by_focus
                .get(&format!("{focus}|{part}"))
                .filter(|a| a[1] > 0.0)
                .map(|a| a[0] / a[1])
        };
        println!("    peça          | balanc. | potênc. | handl. | aceler.");
        println!("    --------------+---------+---------+--------+--------");
        // Ordena as peças pela dispersão entre arquétipos (as que mais separam no topo).
        let mut rows: Vec<(&'static str, [Option<f64>; 4], f64)> = PartType::ALL
            .iter()
            .map(|pt| {
                let name = pt.as_str();
                let vals = [
                    avg(focuses[0], name),
                    avg(focuses[1], name),
                    avg(focuses[2], name),
                    avg(focuses[3], name),
                ];
                let present: Vec<f64> = vals.iter().flatten().copied().collect();
                let spread = match (
                    present.iter().cloned().fold(f64::MIN, f64::max),
                    present.iter().cloned().fold(f64::MAX, f64::min),
                ) {
                    (mx, mn) if present.len() > 1 => mx - mn,
                    _ => 0.0,
                };
                (name, vals, spread)
            })
            .collect();
        rows.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        let cell = |v: Option<f64>| match v {
            Some(x) => format!("{x:>5.1}  "),
            None => "   -   ".to_string(),
        };
        for (name, vals, _) in rows {
            println!(
                "    {:<13} | {} | {} | {} | {}",
                name,
                cell(vals[0]),
                cell(vals[1]),
                cell(vals[2]),
                cell(vals[3])
            );
        }
    }

    // ── (3) Foco vs pista: o ganho de um carro focado na pista do seu atributo ──
    println!("\n■ FOCO vs PISTA — bônus de car_performance por (foco do carro × tipo de pista)");
    println!("    Carros FORTEMENTE focados (4 peças no talo, resto no piso). Diagonal = casado.");
    println!("    ~1,6 pts de car_performance ≈ 1 nível de carro NAQUELA pista; ainda ×car_weight na corrida.");
    {
        use crate::car::sim_bridge::car_shape_weights;
        use crate::car::PartType::*;
        use crate::simulation::car_build::track_delta_from_shape;
        use crate::simulation::track_profile::get_track_simulation_data;
        let mk = |focus: &[crate::car::PartType]| -> crate::car::Car {
            let mut c = crate::car::Car::uniform(2);
            for &p in focus {
                c.set_level(p, 10);
            }
            c
        };
        let cars = [
            ("potência", mk(&[Engine, Gearbox, Cooling, Electronics])),
            ("handling", mk(&[FrontWing, RearWing, Brakes, Suspension])),
            ("aceleração", mk(&[Gearbox, Chassis, Suspension, Electronics])),
            ("balanceado", crate::car::Car::uniform(6)),
        ];
        let tracks = [
            ("power(Monza)", 93u32),
            ("handl(Ledenon)", 489u32),
            ("accel(Tsukuba)", 325u32),
        ];
        print!("    {:<12}", "carro\\pista");
        for (tname, _) in &tracks {
            print!(" | {:>14}", tname);
        }
        println!();
        for (cname, car) in &cars {
            let shape = car_shape_weights(car);
            print!("    {:<12}", cname);
            for (_, tid) in &tracks {
                let d = get_track_simulation_data(*tid);
                let tw = (d.acceleration_weight, d.power_weight, d.handling_weight);
                print!(" | {:>+14.2}", track_delta_from_shape(shape, tw));
            }
            println!();
        }
    }

    println!("\n■ MOVIMENTOS DE EQUIPE (promoção/rebaixamento entre categorias)");
    println!(
        "    Promovidas:  {}   |   Rebaixadas:  {}",
        t.team_promoted, t.team_relegated
    );

    println!(
        "\n■ SOFT LANDING — sobrevivência do promovido (Ideia 1; flag IRACER_PROMO_SOFT_LANDING)"
    );
    println!("  Onde o carro do promovido aterrissa no campo de destino (pós-promoção):");
    if t.promo_landing_n > 0 {
        println!(
            "    Gap médio p/ o lanterna do campo: {:+.2} pts de carro   (>0 acima do pior; <0 ISOLADO abaixo)",
            t.promo_landing_gap_sum / t.promo_landing_n as f64
        );
        let n = t.promo_landing_n;
        println!(
            "    Aterrissou como PIOR do campo (isolado):  {:.1}%   ({} de {})",
            pct(t.promo_landing_rank_worst, n),
            t.promo_landing_rank_worst,
            n
        );
        println!(
            "    Logo acima do lanterna (2º/3º pior):      {:.1}%   ({} de {})",
            pct(t.promo_landing_rank_near, n),
            t.promo_landing_rank_near,
            n
        );
        println!(
            "    Meio de tabela ou melhor:                 {:.1}%   ({} de {})",
            pct(t.promo_landing_rank_mid, n),
            t.promo_landing_rank_mid,
            n
        );
    } else {
        println!("    (sem promoções observadas)");
    }
    println!("  Bounce-down (subiu e caiu logo — o 'vai não vai'):");
    if t.promo_events_obs1 > 0 {
        println!(
            "    Rebaixada já na temporada seguinte (S+1): {:.1}%   ({} de {})",
            pct(t.promo_bounce_1, t.promo_events_obs1),
            t.promo_bounce_1,
            t.promo_events_obs1
        );
    }
    if t.promo_events_obs2 > 0 {
        println!(
            "    Rebaixada em ≤2 temporadas (S+1 ou S+2):  {:.1}%   ({} de {})",
            pct(t.promo_bounce_2, t.promo_events_obs2),
            t.promo_bounce_2,
            t.promo_events_obs2
        );
    }
    if t.releg_events_obs2 > 0 {
        println!(
            "  Ricochete do rebaixado (caiu e voltou a subir em ≤2): {:.1}%   ({} de {})",
            pct(t.releg_bounce_back_2, t.releg_events_obs2),
            t.releg_bounce_back_2,
            t.releg_events_obs2
        );
    }

    println!("\n■ SNOWBALL — cadeia de 'sobe-e-vence' por equipe (promoções em temporadas");
    println!("  e tiers consecutivos; 1 = campeã 1x, 3+ = sobe a escada ganhando todo ano)");
    println!("    comprimento | nº de equipes");
    println!("    ------------+--------------");
    for (len, n) in &t.ladder_chain_hist {
        println!("    {:<11} | {}", len, n);
    }
    println!("    Maior cadeia observada (qualquer run): {}", t.max_ladder_chain);
    {
        let total: u64 = t.rookie_champ_names.values().sum();
        let mut ranked: Vec<(&String, &u64)> = t.rookie_champ_names.iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(a.1));
        println!("\n  IDENTIDADE — campeões da ROOKIE (nome, nº de títulos, % do total={total}):");
        for (name, n) in ranked.iter().take(10) {
            let pct = if total > 0 { 100.0 * **n as f64 / total as f64 } else { 0.0 };
            println!("    {:<28} | {:>3} | {:>4.1}%", name, n, pct);
        }
        let mut climbers: Vec<(&String, &u64)> = t.climber_names.iter().collect();
        climbers.sort_by(|a, b| b.1.cmp(a.1));
        if !climbers.is_empty() {
            println!("  IDENTIDADE — equipes que sobem 2+ tiers seguidos (nome, nº):");
            for (name, n) in climbers.iter().take(10) {
                println!("    {:<28} | {:>3}", name, n);
            }
        }
    }

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

    if t.premium_class_count > 0 {
        println!("\n■ DINASTIAS por CLASSE premium (Production/GT3/GT4/LMP2/Endurance)");
        println!(
            "    Vencedores únicos por classe (média):  {:.2}   (sem dinastia ~6; alvo ~3)",
            t.premium_unique_sum / t.premium_class_count as f64
        );
        println!(
            "    Fatia da equipe TOP da classe (média): {:.1}%   (alvo alto mas < ~55%)",
            100.0 * t.premium_top_share_sum / t.premium_class_count as f64
        );
    }

    if !t.rep_by_tier.is_empty() {
        println!("\n■ REPUTAÇÃO VIVA por tier (semente plana ~±3 → deve separar topo/fundo)");
        println!("    tier | média | desv.pad | mínimo | máximo | nº");
        println!("    -----+-------+----------+--------+--------+------");
        for (tier, r) in &t.rep_by_tier {
            let n = r[4];
            if n <= 0.0 {
                continue;
            }
            let mean = r[0] / n;
            let var = (r[1] / n - mean * mean).max(0.0);
            println!(
                "     {:>3} | {:>5.1} | {:>8.1} | {:>6.1} | {:>6.1} | {:>5.0}",
                tier,
                mean,
                var.sqrt(),
                r[2],
                r[3],
                n
            );
        }
    }

    if t.morale_dist[4] > 0.0 {
        let md = t.morale_dist;
        let mean = md[0] / md[4];
        let var = (md[1] / md[4] - mean * mean).max(0.0);
        if !t.focus_dist.is_empty() {
            let total: u64 = t.focus_dist.values().sum();
            println!("\n■ FOCO DA EQUIPE (deve espalhar entre as 6 fases, não travar)");
            let mut rows: Vec<(&String, &u64)> = t.focus_dist.iter().collect();
            rows.sort_by(|a, b| b.1.cmp(a.1));
            for (foco, n) in rows {
                let pct = if total > 0 {
                    100.0 * *n as f64 / total as f64
                } else {
                    0.0
                };
                println!("    {:<20} | {:>5} | {:>4.1}%", foco, n, pct);
            }
        }

        if t.bond_pairs > 0 {
            let avg = t.bond_tenure_sum / t.bond_pairs as f64;
            let pct_ge3 = 100.0 * t.bond_ge3 as f64 / t.bond_pairs as f64;
            let pct_ge4 = 100.0 * t.bond_ge4 as f64 / t.bond_pairs as f64;
            println!("\n■ VÍNCULO piloto-equipe (duplas de era SEM congelar o mercado)");
            println!(
                "    tenure médio {:.2} temporadas | ≥3 juntos {:.1}% | ≥4 (era) {:.1}% | máx {} | pares {}",
                avg, pct_ge3, pct_ge4, t.bond_max_tenure, t.bond_pairs
            );
        }

        println!("\n■ MORAL VIVA (travada em 1.0 = morta → deve variar por forma/treta)");
        println!(
            "    média {:.3} | desv.pad {:.3} | mínimo {:.2} | máximo {:.2} | nº {:.0}",
            mean,
            var.sqrt(),
            md[2],
            md[3],
            md[4]
        );
    }

    // ── RIVALIDADE ENTRE EQUIPES (Fase 2: poucas e quentes, 4 fontes) ───────
    if t.tr_runs > 0 {
        println!("\n■ RIVALIDADE ENTRE EQUIPES (vivas ≥20; deve existir, ser POUCA e quente)");
        let avg_count = t.tr_count_sum as f64 / t.tr_runs as f64;
        let avg_perceived = if t.tr_count_sum > 0 {
            t.tr_perceived_sum / t.tr_count_sum as f64
        } else {
            0.0
        };
        println!(
            "    vivas/run {:.1} | percebida média {:.1} | máx {:.1}",
            avg_count, avg_perceived, t.tr_perceived_max
        );
        if t.tr_by_source.is_empty() {
            println!("    (nenhuma rivalidade viva — nenhuma fonte disparou)");
        } else {
            print!("    por fonte:");
            for (src, n) in &t.tr_by_source {
                print!("  {src}={n}");
            }
            println!();
        }
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

    // ── TEXTURA DE NOMES DO ROOKIE ──────────────────────────────────────────
    if t.rookie_season_count > 0 {
        let sc = t.rookie_season_count as f64;
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║  TEXTURA DE NOMES DO ROOKIE (por temporada, exclui a 1ª)      ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!("  Temporadas-rookie observadas:  {}", t.rookie_season_count);
        println!(
            "  Grid médio do Rookie:          {:.1} assentos",
            t.rookie_obs as f64 / sc
        );
        println!("\n  composição média do grid por temporada:");
        println!(
            "    Estreias NOVAS (nome inédito):       {:>5.1}  ({:.1}%)",
            t.rookie_fresh as f64 / sc,
            pct(t.rookie_fresh, t.rookie_obs)
        );
        println!(
            "    Retidos (mesmo piloto do ano ant.):  {:>5.1}  ({:.1}%)",
            t.rookie_retained as f64 / sc,
            pct(t.rookie_retained, t.rookie_obs)
        );
        println!(
            "    Conhecidos retornando (ag. livre):   {:>5.1}  ({:.1}%)",
            t.rookie_returning as f64 / sc,
            pct(t.rookie_returning, t.rookie_obs)
        );
        if t.rookie_age_n > 0 {
            println!(
                "  Idade média no Rookie:         {:.1} anos",
                t.rookie_age_sum as f64 / t.rookie_age_n as f64
            );
        }
    }

    // ── FUNIL DE CARREIRA ───────────────────────────────────────────────────
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  FUNIL DE CARREIRA — pilotos que começaram no Rookie          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!(
        "  Cohort (estrearam no tier 0 / Rookie): {}",
        t.rookie_cohort
    );
    println!("\n  Tier alcançado          | % do cohort | tempo médio p/ chegar");
    println!("  ------------------------+-------------+----------------------");
    for tier in 1..=6usize {
        let reached = t.reached_tier[tier];
        let avg_time = if t.time_to_tier_n[tier] > 0 {
            format!(
                "{:.1} temporadas",
                t.time_to_tier_sum[tier] as f64 / t.time_to_tier_n[tier] as f64
            )
        } else {
            "—".to_string()
        };
        println!(
            "  {:<2} {:<20} | {:>9.1}% | {}",
            tier,
            tier_label(tier as u8),
            pct(reached, t.rookie_cohort),
            avg_time
        );
    }

    println!("\n  ■ Impacto do talento (PICO de habilidade na carreira → topo atingido)");
    println!("    pico de skill    |   n  | tier médio | % que chega ao topo (5+)");
    println!("    -----------------+------+------------+-------------------------");
    for band in ["Elite (65+)", "Bom (57-65)", "Comum (<57)"] {
        if let Some(a) = t.skill_band.get(band) {
            if a[0] == 0 {
                continue;
            }
            println!(
                "    {:<16} | {:>4} | {:>10.2} | {:.1}%",
                band,
                a[0],
                a[1] as f64 / a[0] as f64,
                pct(a[2], a[0])
            );
        }
    }
    println!(
        "\n  (Nota: corridas longas dão mais tempo de carreira — rode com IRACER_MC_SEASONS alto)"
    );

    // ── KPI ANTI-DEFLAÇÃO — o melhor carro fica com o melhor piloto? ──────────
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  DEFLAÇÃO DA GRADE — correlação carro↔skill por tier          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("  (r→+1 = melhor carro com melhor piloto; r→0 = grade deflacionada/mal alocada)");
    println!("\n  tier                  |    n   | skill médio | corr r(carro,skill)");
    println!("  ----------------------+--------+-------------+--------------------");
    for (tier, m) in &t.grid_corr_by_tier {
        let n = m[0];
        if n < 2.0 {
            continue;
        }
        let mean_skill = m[2] / n;
        let cov = m[5] - m[1] * m[2] / n;
        let var_car = m[3] - m[1] * m[1] / n;
        let var_skill = m[4] - m[2] * m[2] / n;
        let denom = (var_car * var_skill).max(0.0).sqrt();
        let r = if denom > 1e-9 { cov / denom } else { 0.0 };
        println!(
            "  {:<2} {:<18} | {:>6} | {:>11.2} | {:>+.3}",
            tier,
            tier_label(*tier),
            n as u64,
            mean_skill,
            r
        );
    }

    let em_promo =
        crate::market::pipeline::EMERGENCY_PROMOTIONS.load(std::sync::atomic::Ordering::Relaxed);
    let em_rookie =
        crate::market::pipeline::EMERGENCY_ROOKIES.load(std::sync::atomic::Ordering::Relaxed);
    println!("\n[ PREENCHIMENTO DE EMERGENCIA (escassez na escada fechada) ]");
    println!("  Promocoes de emergencia (ignora merito): {em_promo}");
    println!("  Rookies de emergencia (sem feeder):      {em_rookie}");
    println!(
        "  Total de disparos em {} ciclos: {}",
        runs * seasons,
        em_promo + em_rookie
    );

    // PRA ONDE: tier da vaga que a emergência preencheu (onde a escassez bate);
    // DE ONDE: tier do feeder de onde o piloto foi promovido.
    let paths: Vec<(u8, u8)> = crate::market::pipeline::EMERGENCY_PROMO_PATHS
        .lock()
        .map(|p| p.clone())
        .unwrap_or_default();
    if !paths.is_empty() {
        let mut by_to: BTreeMap<u8, u64> = BTreeMap::new();
        let mut gaps: BTreeMap<i32, u64> = BTreeMap::new(); // (to_tier - from_tier)
        for (from, to) in &paths {
            *by_to.entry(*to).or_insert(0) += 1;
            *gaps.entry(*to as i32 - *from as i32).or_insert(0) += 1;
        }
        println!("\n  PRA ONDE vai a promoção de emergência (tier da vaga):");
        for (to, n) in &by_to {
            println!(
                "    → {:<14} {:>4}  ({:.1}%)",
                tier_label(*to),
                n,
                pct(*n, paths.len() as u64)
            );
        }
        println!("  DE ONDE vem o piloto (salto de tiers feeder→vaga):");
        for (gap, n) in &gaps {
            println!(
                "    {} tier(s) abaixo: {:>4}  ({:.1}%)",
                gap,
                n,
                pct(*n, paths.len() as u64)
            );
        }
    }

    let retire_total: u64 = t.retire_by_tier.iter().sum();
    if retire_total > 0 {
        println!("\n  DE ONDE abrem as vagas — aposentadorias por tier:");
        for tier in 0..=6usize {
            let n = t.retire_by_tier[tier];
            if n == 0 {
                continue;
            }
            println!(
                "    {:<14} {:>5}  ({:.1}%)",
                tier_label(tier as u8),
                n,
                pct(n, retire_total)
            );
        }
    }

    println!("\nTempo total: {:.1}s\n", start.elapsed().as_secs_f64());
}
