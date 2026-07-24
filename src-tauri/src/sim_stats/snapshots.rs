//! Leituras do banco de uma carreira em andamento: fotos de pilotos, lesões,
//! desempenho da temporada, equipes, carros, títulos, vínculos e salários.

use super::*;

// ── Snapshot de pilotos ───────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub(super) struct DriverSnap {
    pub(super) age: i32,
    pub(super) overall: f64,
}

/// Snapshot de todos os pilotos da IA ativos (não aposentados).
pub(super) fn snapshot_drivers(db_path: &Path) -> HashMap<String, DriverSnap> {
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
pub(super) fn snapshot_injuries(db_path: &Path) -> HashMap<String, (String, String)> {
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
pub(super) struct SeasonPerf {
    pub(super) vitorias: i64,
    pub(super) podios: i64,
    pub(super) corridas: i64,
    pub(super) dnfs: i64,
    pub(super) motivacao: f64,
}

/// Lê as stats `temp_*` dos pilotos da IA (preenchidas pela simulação da temporada).
pub(super) fn snapshot_season_perf(db_path: &Path) -> Vec<SeasonPerf> {
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
pub(super) struct TeamSnap {
    pub(super) id: String,
    pub(super) categoria: String,
    pub(super) classe: String,
    pub(super) car_performance: f64,
    /// Nível do Carro (1–10) do Sistema de Nível do Carro (tabela team_car).
    pub(super) car_level: u8,
    /// Carro completo (11 peças) para diagnósticos de distribuição/foco.
    pub(super) car: Option<crate::car::Car>,
    pub(super) confiabilidade: f64,
    pub(super) reputacao: f64,
    pub(super) facilities: f64,
    pub(super) engineering: f64,
    pub(super) morale: f64,
    pub(super) cash_balance: f64,
    pub(super) debt_balance: f64,
    pub(super) financial_state: String,
    pub(super) foco: String,
}

/// Pares (car_performance do assento, skill do piloto que o ocupa) de cada equipe ativa
/// da IA, com a categoria — para medir a correlação carro↔skill por tier (KPI de deflação).
/// Junta cada assento (piloto_1_id / piloto_2_id) ao skill do piloto.
pub(super) fn snapshot_grid_pairs(db_path: &Path) -> Vec<(f64, f64, String)> {
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
pub(super) fn team_rivalry_snapshot(db_path: &Path) -> (u64, f64, f64, Vec<(String, u64)>) {
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

pub(super) fn snapshot_teams(db_path: &Path) -> Vec<TeamSnap> {
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
pub(super) fn classify_shape(car: &crate::car::Car) -> &'static str {
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
pub(super) fn rescue_counter(db_path: &Path, key: &str) -> i64 {
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
pub(super) fn constructor_titles_by_team(db_path: &Path) -> Vec<i64> {
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
pub(super) fn bond_tenure_snapshot(db_path: &Path) -> (f64, i64, i64, i64, i64) {
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

pub(super) fn premium_class_title_dist(db_path: &Path) -> Vec<Vec<i64>> {
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
pub(super) fn snapshot_salaries(db_path: &Path) -> Vec<(String, f64)> {
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

/// Categoria atual de cada piloto da IA ativo (id → categoria).
pub(super) fn snapshot_driver_categories(db_path: &Path) -> HashMap<String, String> {
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
