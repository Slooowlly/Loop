use rusqlite::{Connection, OptionalExtension};

use crate::db::connection::DbError;

mod baseline;
mod seed_incidentes;

/// Trava do schema-ouro (só em teste): descreve o schema de forma normalizada e
/// compara com um fixture versionado.
#[cfg(test)]
mod schema_ouro;

use baseline::BASELINE_DDL;
use seed_incidentes::seed_incident_catalog;

// ── Versão atual do schema ────────────────────────────────────────────────────

const CURRENT_VERSION: u32 = 58;

/// Versão da BASELINE — o piso a partir do qual um save ainda tem caminho de
/// atualização. Save carimbado abaixo disto veio das migrações incrementais que
/// foram colapsadas e não pode ser migrado; save em v53 ou acima sobe normalmente
/// pelas migrações incrementais registradas depois dela.
const BASELINE_VERSION: u32 = 53;

// ── API pública ───────────────────────────────────────────────────────────────

/// Registro declarativo das migrações: `(versão, função)`. ÚNICA fonte de verdade
/// da ordem e do conjunto — adicionar uma migração = UMA linha aqui (e bumpar
/// `CURRENT_VERSION`).
///
/// As 53 migrações incrementais originais foram colapsadas numa baseline única,
/// registrada na versão 53: um banco novo já nasce carimbado como 53 e
/// `run_pending` continua coerente. Saves antigos NÃO são migrados — a baseline
/// só sabe criar um banco do zero.
const MIGRATIONS: &[(u32, fn(&Connection) -> Result<(), DbError>)] = &[
    (53, migrate_baseline),
    (54, migrate_v54_forma_do_piloto),
    (55, migrate_v55_leitura_da_corrida),
    (56, migrate_v56_faixa_anunciada),
    (57, migrate_v57_leitura_do_fim_de_semana),
    (58, migrate_v58_semeia_carro_das_categorias_especiais),
];

/// Aplica todas as migrações num banco novo (versão 0 → CURRENT_VERSION).
pub fn run_all(conn: &Connection) -> Result<(), DbError> {
    for (version, migrate) in MIGRATIONS {
        migrate(conn)?;
        set_schema_version(conn, *version)?;
    }
    Ok(())
}

/// Aplica apenas as migrações pendentes num banco existente.
///
/// Um banco vazio (versão 0) nasce da baseline normalmente. Já um save carimbado
/// entre 1 e 52 veio das migrações incrementais que foram colapsadas: a baseline
/// é DDL `IF NOT EXISTS` e não faz backfill de dado, então aplicá-la ali só
/// carimbaria a versão 53 num banco que continua na forma velha. Recusamos em vez
/// de mentir sobre a versão — o arquivo do jogador fica intocado.
pub fn run_pending(conn: &Connection) -> Result<(), DbError> {
    let version = get_schema_version(conn)?;

    if version > 0 && version < BASELINE_VERSION {
        return Err(DbError::InvalidData(format!(
            "este save está no schema v{version}, anterior à baseline v{BASELINE_VERSION}. \
             As migrações incrementais foram colapsadas numa baseline única e não existe \
             caminho de atualização a partir daqui. O arquivo não foi alterado: use um \
             backup mais recente ou comece uma carreira nova."
        )));
    }

    for (target, migrate) in MIGRATIONS {
        if version < *target {
            migrate(conn)?;
            set_schema_version(conn, *target)?;
        }
    }
    Ok(())
}

// ── Baseline ──────────────────────────────────────────────────────────────────

/// Cria o schema final inteiro de uma vez e semeia o que o jogo precisa para
/// funcionar: a tabela `meta` (contadores e configuração) e o catálogo de
/// incidentes. Todo o DDL é `IF NOT EXISTS`, então reaplicar é inofensivo.
fn migrate_baseline(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(BASELINE_DDL)?;
    seed_meta(conn)?;
    seed_incident_catalog(conn)?;
    Ok(())
}

// ── Migrações incrementais sobre a baseline ───────────────────────────────────

/// v54 — estado da FORMA do momento do piloto (o AR(1) de `simulation::forma`).
///
/// É a única das três camadas de performance de fim de semana que precisa de
/// estado: afinidade de pista e acerto de fim de semana saem de hash
/// determinístico e não persistem nada. 0.0 = forma neutra, que é exatamente o
/// que um save antigo deve começar valendo.
fn migrate_v54_forma_do_piloto(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch("ALTER TABLE drivers ADD COLUMN forma REAL NOT NULL DEFAULT 0.0;")?;
    Ok(())
}

/// v55 — o dado que EXPLICA o resultado da corrida passa a sobreviver ao save.
///
/// O motor já calculava tudo isto (posição por trecho, ar sujo, tentativas de
/// ultrapassagem, custo do box, safety car) e jogava fora em `build_race_results`:
/// só a linha "P8, 3 incidentes" chegava ao banco. Sem isto a reforma da simulação é
/// invisível — o jogador vê a posição e não a causa dela.
///
/// **Escalar vira coluna, vetor vira JSON.** A divisão não é estética:
///
/// - Os seis escalares (`segmentos_em_ar_sujo`, as três contagens de ultrapassagem,
///   `maior_sequencia_preso`, `estrategia_id`) são de aridade fixa e são exatamente os
///   campos que se quer AGREGAR: somar ultrapassagens da temporada, tirar a taxa de
///   conversão de um piloto, contar estratégias distintas no grid. Isso é `SUM`/`AVG`/
///   `COUNT(DISTINCT)` em SQL, e JSON dentro de coluna mataria essas consultas.
/// - Os vetores (posição e gap por trecho, e o trio das paradas) nunca são predicado:
///   são lidos inteiros, de uma corrida só, pra desenhar um gráfico. Além disso têm
///   aridade que NÃO é minha pra congelar — "5 trechos" é constante da simulação, e
///   `pos_trecho_1..5` em coluna transformaria uma constante de outro módulo em schema.
///   As paradas são de aridade genuinamente variável (0 a N) e os gaps têm elemento
///   nulo (o líder não tem carro à frente), duas coisas que coluna escalar não
///   representa sem inventar sentinela. Precedente do projeto: `drivers.historico_circuitos`.
///
/// São DOIS blobs, não um, e cada um espelha o dono do dado na simulação
/// (`RaceState::trafego` e `RaceState::paradas`) — misturá-los criaria um objeto sem
/// dono de quem nenhum dos dois pacotes responde.
///
/// O safety car é da CORRIDA, não do piloto, então vai em tabela própria — uma linha
/// por safety car, no molde de `race_breakdowns`. A ordem do pelotão no momento em que
/// ele entrou é uma lista ordenada de pilotos: JSON na linha, porque a única pergunta
/// que se faz dela é "como estava antes × como terminou", sempre lida inteira.
///
/// Save antigo não tem nada disso, e é por isso que todo default é o valor NEUTRO
/// (0 / '' / '{}' / nenhuma linha de safety car): a leitura precisa distinguir "não
/// aconteceu" de "não foi gravado", e ela faz isso pelo vetor VAZIO — corrida antiga
/// não desenha traçado nenhum em vez de desenhar um traçado errado.
fn migrate_v55_leitura_da_corrida(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "
        ALTER TABLE race_results ADD COLUMN segmentos_em_ar_sujo INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE race_results ADD COLUMN tentativas_ultrapassagem INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE race_results ADD COLUMN ultrapassagens_concluidas INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE race_results ADD COLUMN tentativas_sofridas INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE race_results ADD COLUMN maior_sequencia_preso INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE race_results ADD COLUMN estrategia_id TEXT NOT NULL DEFAULT '';
        ALTER TABLE race_results ADD COLUMN trafego_json TEXT NOT NULL DEFAULT '{}';
        ALTER TABLE race_results ADD COLUMN paradas_json TEXT NOT NULL DEFAULT '{}';

        CREATE TABLE IF NOT EXISTS race_safety_cars (
            race_id   TEXT NOT NULL,
            ordem     INTEGER NOT NULL,
            volta     INTEGER NOT NULL,
            grid_json TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (race_id, ordem)
        );
        ",
    )?;
    Ok(())
}

/// v56 — a FAIXA ANUNCIADA do fim de semana vira fato histórico da corrida.
///
/// A leitura do fim de semana (fase 3) é anunciada ao jogador ANTES da largada, e o
/// debrief depois fecha o loop dizendo se o que foi anunciado se cumpriu. Isso só
/// funciona se a faixa anunciada for imutável — e ela não é, se for recomputada na hora
/// de exibir:
///
/// - A tela pós-corrida é REVISITÁVEL (`race_screens/<id>.json` persiste, e
///   `get_saved_race_screen` a reabre). Um patch que mude o σ de uma camada faria a
///   corrida antiga passar a mostrar faixa diferente da anunciada no sábado — pré e pós
///   discordando por atualização, sem ninguém ter errado nada.
/// - E há um segundo motivo, mais imediato: `update_driver_forma` é chamado DENTRO da
///   simulação, então `drivers.forma` antes da corrida guarda o estado da etapa
///   ANTERIOR. A faixa anunciada é derivada de `proxima_forma` com semente determinística
///   — reproduzível, mas função também de motivação e confiança, que podem mudar no meio
///   do fim de semana. Recomputar depois não devolve necessariamente o que foi anunciado.
///
/// A faixa anunciada é, portanto, da mesma natureza dos campos da v55: **fato do fim de
/// semana, não função do estado atual do jogo**. Vai no mesmo caminho e por isso é um
/// blob — é lida inteira, de uma corrida só, nunca é predicado, e sua aridade ("3 camadas
/// × 2 canais") é constante de outro módulo, que não é minha para congelar em coluna.
///
/// Default `'{}'` = corrida sem faixa anunciada (toda corrida anterior a esta, e toda
/// corrida enquanto o motor não fornecer a leitura). A exibição trata isso como "não
/// anunciado" e não mostra nada — melhor calar do que mostrar coisa diferente da que
/// aconteceu.
///
/// Migração NOVA em vez de emenda na v55: a v55 não foi publicada, mas está na árvore de
/// trabalho de uma sessão com outros dois trabalhos ativos, e pode já ter sido aplicada a
/// algum save local. Custo de errar isso é save corrompido; custo de uma linha extra aqui
/// é uma linha extra. Na dúvida, trate como lançada.
fn migrate_v56_faixa_anunciada(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "ALTER TABLE race_results ADD COLUMN leitura_fds_json TEXT NOT NULL DEFAULT '{}';",
    )?;
    Ok(())
}

/// v57 — a leitura do fim de semana ganha tabela PRÓPRIA, e com isso o anúncio passa a ser
/// seguro **por construção** em vez de por ordem de chamadas.
///
/// Supersede a coluna `race_results.leitura_fds_json` da v56. O motivo é que a v56 estava no
/// lugar errado, por duas razões independentes:
///
/// 1. **Cronologia.** A leitura é anunciada ANTES da largada; a linha de `race_results` só
///    nasce DEPOIS da corrida. Guardar ali obrigava o anúncio a ser recomputado ao vivo no
///    caminho pré-corrida — dois cálculos em dois instantes, cuja concordância dependia de
///    as entradas serem idênticas nos dois.
/// 2. **A forma não é como o clima.** O precedente do projeto
///    (`resolve_and_persist_race_weather`) recomputa com segurança porque
///    `event_seed(career_id, race_id)` não depende de nada mutável. Já a forma sai de
///    `proxima_forma(estado_anterior, semente, motivacao, confianca)`: `drivers.forma` vale
///    a etapa N−1 até a simulação avançá-la (o `update_driver_forma` roda DENTRO dela), e
///    motivação/confiança são estado mutável. Recomputar depois podia divergir por um passo
///    de AR — divergência de uma faixa, ou de nenhuma, que é a mais difícil de notar.
///
/// Com tabela própria a linha pode nascer no preparo da etapa, antes de existir resultado, e
/// as duas telas leem a MESMA linha. Some o segundo cálculo, e com ele a classe inteira de
/// bug.
///
/// PK `(race_id, driver_id)` no molde de `race_breakdowns` e `race_safety_cars` — reescrever
/// o mesmo par sobrescreve em vez de duplicar, que é o que um preparo de etapa idempotente
/// precisa.
///
/// A coluna da v56 fica onde está, sem leitor nem escritor. Removê-la exigiria um
/// `DROP COLUMN` que, se falhar em alguma build de SQLite, impede o save de abrir — custo
/// desproporcional para uma coluna provadamente vazia (nada nunca a populou em produção,
/// porque o produtor da leitura ainda não existe).
fn migrate_v57_leitura_do_fim_de_semana(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS race_weekend_readings (
            race_id      TEXT NOT NULL,
            driver_id    TEXT NOT NULL,
            leitura_json TEXT NOT NULL DEFAULT '{}',
            PRIMARY KEY (race_id, driver_id)
        );
        ",
    )?;
    Ok(())
}

/// v58 — semeia `team_car` para as equipes que nunca tiveram carro.
///
/// As categorias do bloco especial (`endurance`, `production_challenger`) rodavam fora do
/// Sistema de Nível do Carro: o cérebro de manutenção mora no bloco de finanças da corrida,
/// de onde essas categorias saíam antes por um `return`. Nunca criavam linha aqui, e o
/// `effective_car_performance` delas caía na coluna legada `car_performance` — sem teto e
/// sem atualização pelo sistema de peças.
///
/// Esta migração fecha o buraco nos saves em campo. Sem ela, o primeiro pit dessas equipes
/// semearia um carro NEUTRO (qualidade 0,5) para todas — achatando de uma vez a hierarquia
/// que o save levou temporadas construindo. Aqui o nível sai do `car_performance` que a
/// equipe tem HOJE, medido dentro da própria `(categoria, classe)`, então a ordem relativa
/// do grid é preservada ao entrar na escala nova.
///
/// Os números de teto e o piso do seed estão COPIADOS de `car::cost`/`car::seed` de
/// propósito: migração é um retrato congelado do que valia quando ela foi escrita, e não
/// pode mudar de comportamento porque a regra viva evoluiu depois.
fn migrate_v58_semeia_carro_das_categorias_especiais(conn: &Connection) -> Result<(), DbError> {
    /// Teto de nível por `(categoria, classe)`, congelado nesta versão.
    fn ceiling_for(categoria: &str, classe: Option<&str>) -> i64 {
        match (categoria, classe) {
            ("endurance", Some("gt4")) => 6,
            ("endurance", Some("gt3")) => 7,
            ("endurance", Some("lmp2")) => 8,
            ("endurance", _) => 8,
            ("mazda_rookie" | "toyota_rookie", _) => 1,
            ("mazda_amador" | "toyota_amador", _) => 2,
            ("bmw_m2", _) => 3,
            ("production_challenger", _) => 4,
            ("gt4", _) => 6,
            ("gt3", _) => 7,
            ("lmp2", _) => 8,
            _ => 4,
        }
    }

    const PART_TYPES: [&str; 11] = [
        "chassis",
        "engine",
        "front_wing",
        "rear_wing",
        "underbody",
        "sidepods",
        "cooling",
        "gearbox",
        "brakes",
        "suspension",
        "electronics",
    ];

    // (id, categoria, classe, car_performance) das equipes sem carro.
    let rows: Vec<(String, String, Option<String>, f64)> = {
        let mut stmt = conn.prepare(
            "SELECT t.id, t.categoria, t.classe, COALESCE(t.car_performance, 0.0)
               FROM teams t
              WHERE NOT EXISTS (SELECT 1 FROM team_car c WHERE c.team_id = t.id)",
        )?;
        let mapped = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })?;
        let mut collected = Vec::new();
        for row in mapped {
            collected.push(row?);
        }
        collected
    };

    if rows.is_empty() {
        return Ok(());
    }

    // Min/max de car_performance por (categoria, classe) — a escala é da CLASSE, não da
    // categoria: no Endurance, GT4/GT3/LMP2 dividem a categoria com carros diferentes.
    let mut bounds: std::collections::HashMap<(String, String), (f64, f64)> =
        std::collections::HashMap::new();
    for (_, categoria, classe, perf) in &rows {
        let key = (categoria.clone(), classe.clone().unwrap_or_default());
        let entry = bounds
            .entry(key)
            .or_insert((f64::INFINITY, f64::NEG_INFINITY));
        entry.0 = entry.0.min(*perf);
        entry.1 = entry.1.max(*perf);
    }

    let mut insert = conn.prepare(
        "INSERT OR IGNORE INTO team_car (team_id, part_type, level, wear)
         VALUES (?1, ?2, ?3, 0.0)",
    )?;

    for (id, categoria, classe, perf) in &rows {
        let ceiling = ceiling_for(categoria, classe.as_deref());
        let level = if ceiling <= 1 {
            1
        } else {
            let key = (categoria.clone(), classe.clone().unwrap_or_default());
            let (min, max) = bounds.get(&key).copied().unwrap_or((0.0, 0.0));
            let spread = max - min;
            let quality = if spread.abs() < f64::EPSILON {
                0.5
            } else {
                ((perf - min) / spread).clamp(0.0, 1.0)
            };
            // Mesma interpolação do `car::seed::seed_car`: piso em 40% do teto, para que a
            // equipe mais fraca da classe não caia no nível 1 numa categoria alta.
            let ceiling_f = ceiling as f64;
            let floor = (ceiling_f * 0.4).round().max(1.0);
            (floor + quality * (ceiling_f - floor))
                .round()
                .clamp(1.0, ceiling_f) as i64
        };

        for part_type in PART_TYPES {
            insert.execute(rusqlite::params![id, part_type, level])?;
        }
    }

    Ok(())
}

// ── Helpers de versão ─────────────────────────────────────────────────────────

pub fn get_schema_version(conn: &Connection) -> Result<u32, DbError> {
    // A tabela meta pode não existir ainda num banco vazio.
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='meta'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false);

    if !exists {
        return Ok(0);
    }

    conn.query_row(
        "SELECT value FROM meta WHERE key = 'schema_version'",
        [],
        |row| row.get::<_, String>(0),
    )
    .map_err(DbError::Sqlite)
    .and_then(|v| {
        v.parse::<u32>()
            .map_err(|_| DbError::InvalidData(format!("schema_version invalida em meta: '{v}'")))
    })
}

fn set_schema_version(conn: &Connection, version: u32) -> Result<(), DbError> {
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![version.to_string()],
    )?;
    Ok(())
}

// ── Seed inicial da tabela meta ───────────────────────────────────────────────

fn seed_meta(conn: &Connection) -> Result<(), DbError> {
    let seeds = [
        ("next_driver_id", "1"),
        ("next_team_id", "1"),
        ("next_season_id", "1"),
        ("next_race_id", "1"),
        ("next_contract_id", "1"),
        ("next_news_id", "1"),
        ("next_rivalry_id", "1"),
        ("next_team_rivalry_id", "1"),
        ("current_season", "1"),
        ("current_year", "2024"),
        ("career_start_year", "2024"),
        ("difficulty", "Normal"),
    ];

    for (key, value) in &seeds {
        conn.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guarda-corpo do registro declarativo: as versões precisam ser
    /// estritamente crescentes e a última tem de bater com `CURRENT_VERSION`.
    /// Se alguém adicionar uma migração e esquecer de bumpar a versão (ou
    /// registrar fora de ordem), este teste quebra na hora.
    #[test]
    fn migrations_table_is_sorted_and_matches_current_version() {
        for par in MIGRATIONS.windows(2) {
            assert!(
                par[0].0 < par[1].0,
                "MIGRATIONS fora de ordem: {} vem antes de {}",
                par[0].0,
                par[1].0
            );
        }
        assert_eq!(
            MIGRATIONS.last().map(|(v, _)| *v),
            Some(CURRENT_VERSION),
            "última migração registrada deve ser CURRENT_VERSION ({CURRENT_VERSION})"
        );
    }

    #[test]
    fn test_run_pending_rejects_invalid_schema_version_without_replaying_migrations() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', 'quebrada');

            CREATE TABLE race_results (
                id TEXT PRIMARY KEY,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL
            );

            INSERT INTO race_results (id, race_id, piloto_id, equipe_id)
            VALUES ('legacy', 'R001', 'P001', 'T001');
            ",
        )
        .expect("legacy schema");

        let err = run_pending(&conn).expect_err("invalid schema version should fail");
        assert!(
            matches!(err, DbError::InvalidData(_)),
            "expected invalid-data error, got {err:?}"
        );

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM race_results", [], |row| row.get(0))
            .expect("existing race_results should remain untouched");
        assert_eq!(count, 1);
    }

    /// Um banco novo já nasce carimbado na versão atual, e reabrir (run_pending)
    /// não reaplica nada nem quebra.
    #[test]
    fn banco_novo_nasce_na_versao_atual_e_reabrir_e_noop() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        run_all(&conn).expect("schema");
        assert_eq!(get_schema_version(&conn).expect("versão"), CURRENT_VERSION);

        run_pending(&conn).expect("reabrir não deve migrar nada");
        assert_eq!(get_schema_version(&conn).expect("versão"), CURRENT_VERSION);
    }

    /// As seeds da baseline precisam sobreviver ao colapso: sem elas o jogo
    /// abre sem contadores e sem catálogo de incidentes.
    #[test]
    fn baseline_semeia_meta_e_catalogo_de_incidentes() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        run_all(&conn).expect("schema");

        let incidentes: i64 = conn
            .query_row("SELECT COUNT(*) FROM incident_catalog", [], |row| {
                row.get(0)
            })
            .expect("contagem do catálogo");
        assert_eq!(incidentes, 54);

        for chave in ["next_driver_id", "current_year", "difficulty"] {
            let existe: Option<String> = conn
                .query_row("SELECT value FROM meta WHERE key = ?1", [chave], |row| {
                    row.get(0)
                })
                .optional()
                .expect("consulta meta");
            assert!(existe.is_some(), "meta.{chave} não foi semeada");
        }
    }

    #[test]
    fn test_team_season_archive_schema() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        run_all(&conn).expect("schema");

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'team_season_archive'",
                [],
                |row| row.get(0),
            )
            .expect("table count");
        assert_eq!(table_count, 1);

        for column in [
            "team_id",
            "season_number",
            "ano",
            "categoria",
            "classe",
            "posicao_campeonato",
            "pontos",
            "vitorias",
            "podios",
            "poles",
            "corridas",
            "titulos_construtores",
            "piloto_1_id",
            "piloto_2_id",
            "snapshot_json",
        ] {
            assert!(
                column_exists(&conn, "team_season_archive", column),
                "missing team_season_archive.{column}"
            );
        }
    }

    /// v55 — as colunas da leitura da corrida e a tabela do safety car existem num banco
    /// novo, e o default de cada uma é o valor NEUTRO (o que um save antigo vale).
    #[test]
    fn v55_cria_as_colunas_da_leitura_da_corrida() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        run_all(&conn).expect("schema");

        for column in [
            "segmentos_em_ar_sujo",
            "tentativas_ultrapassagem",
            "ultrapassagens_concluidas",
            "tentativas_sofridas",
            "maior_sequencia_preso",
            "estrategia_id",
            "trafego_json",
            "paradas_json",
        ] {
            assert!(
                column_exists(&conn, "race_results", column),
                "missing race_results.{column}"
            );
        }

        for column in ["race_id", "ordem", "volta", "grid_json"] {
            assert!(
                column_exists(&conn, "race_safety_cars", column),
                "missing race_safety_cars.{column}"
            );
        }
    }

    /// v56 — a coluna da faixa ANUNCIADA existe e nasce vazia. O default `'{}'` é o que
    /// faz a leitura distinguir "não anunciado" de "anunciado como neutro": ele NÃO
    /// desserializa numa `WeekendReading` válida, então a exibição cai em `None` e cala,
    /// em vez de inventar um fim de semana morno.
    #[test]
    fn v56_cria_a_coluna_da_faixa_anunciada_vazia() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        run_all(&conn).expect("schema");
        assert!(
            column_exists(&conn, "race_results", "leitura_fds_json"),
            "missing race_results.leitura_fds_json"
        );

        conn.execute_batch(
            "
            PRAGMA foreign_keys = OFF;
            INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final)
            VALUES ('C001', 'P001', 'T001', 3);
            PRAGMA foreign_keys = ON;
            ",
        )
        .expect("linha nova");
        let json: String = conn
            .query_row(
                "SELECT leitura_fds_json FROM race_results WHERE piloto_id = 'P001'",
                [],
                |row| row.get(0),
            )
            .expect("default");
        assert_eq!(json, "{}");
    }

    /// v57 — a tabela própria da leitura do fim de semana existe, e sua chave permite a
    /// linha nascer ANTES do resultado (que é a razão inteira de ela existir).
    #[test]
    fn v57_cria_a_tabela_da_leitura_do_fim_de_semana() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        run_all(&conn).expect("schema");

        for column in ["race_id", "driver_id", "leitura_json"] {
            assert!(
                column_exists(&conn, "race_weekend_readings", column),
                "missing race_weekend_readings.{column}"
            );
        }

        // Sem FK para `race_results`: a leitura precede o resultado, então amarrá-la a ele
        // recriaria exatamente o problema que a v57 resolve.
        conn.execute(
            "INSERT INTO race_weekend_readings (race_id, driver_id, leitura_json)
             VALUES ('C001', 'P001', '{\"available\":true}')",
            [],
        )
        .expect("leitura sem resultado nenhum gravado");
    }

    /// O caminho de upgrade v56 → v57: um save que já tinha a coluna da v56 ganha a tabela
    /// nova sem perder nada. A coluna velha continua lá, sem leitor nem escritor — removê-la
    /// custaria um `DROP COLUMN` capaz de impedir o save de abrir, por nada.
    #[test]
    fn save_em_v56_sobe_para_v57_ganhando_a_tabela_nova() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        migrate_baseline(&conn).expect("baseline");
        migrate_v54_forma_do_piloto(&conn).expect("v54");
        migrate_v55_leitura_da_corrida(&conn).expect("v55");
        migrate_v56_faixa_anunciada(&conn).expect("v56");
        set_schema_version(&conn, 56).expect("carimbo v56");
        conn.execute_batch(
            "
            PRAGMA foreign_keys = OFF;
            INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final)
            VALUES ('C001', 'P001', 'T001', 5);
            PRAGMA foreign_keys = ON;
            ",
        )
        .expect("resultado da v56");

        run_pending(&conn).expect("upgrade v56 → v57");
        assert_eq!(get_schema_version(&conn).expect("versão"), CURRENT_VERSION);

        let posicao: i64 = conn
            .query_row(
                "SELECT posicao_final FROM race_results WHERE piloto_id = 'P001'",
                [],
                |row| row.get(0),
            )
            .expect("linha preservada");
        assert_eq!(posicao, 5);

        let tabela: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'race_weekend_readings'",
                [],
                |row| row.get(0),
            )
            .expect("tabela nova");
        assert_eq!(tabela, 1);
    }

    /// O caminho de upgrade v55 → v56: um save que já tem a leitura da corrida ganha a
    /// coluna da faixa anunciada sem perder nada do que a v55 gravou.
    #[test]
    fn save_em_v55_sobe_para_v56_preservando_a_leitura_da_corrida() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        migrate_baseline(&conn).expect("baseline");
        migrate_v54_forma_do_piloto(&conn).expect("v54");
        migrate_v55_leitura_da_corrida(&conn).expect("v55");
        set_schema_version(&conn, 55).expect("carimbo v55");
        conn.execute_batch(
            "
            PRAGMA foreign_keys = OFF;
            INSERT INTO race_results
                (race_id, piloto_id, equipe_id, posicao_final, trafego_json, estrategia_id)
            VALUES ('C001', 'P001', 'T001', 6, '{\"posicoes\":[4,5,9,7,6]}', '1-parada-cedo');
            PRAGMA foreign_keys = ON;
            ",
        )
        .expect("leitura da v55");

        run_pending(&conn).expect("upgrade v55 → v56");
        assert_eq!(get_schema_version(&conn).expect("versão"), CURRENT_VERSION);

        let (trafego, estrategia, faixa): (String, String, String) = conn
            .query_row(
                "SELECT trafego_json, estrategia_id, leitura_fds_json
                 FROM race_results WHERE piloto_id = 'P001'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("linha da v55 preservada");
        assert_eq!(trafego, "{\"posicoes\":[4,5,9,7,6]}");
        assert_eq!(estrategia, "1-parada-cedo");
        assert_eq!(faixa, "{}");
    }

    /// O caminho de UPGRADE, que é o cenário que de fato queima o jogador: um save
    /// carimbado na v54 sobe pra v55 sem perder a linha que já estava lá, e a coluna nova
    /// vale o default neutro na linha antiga.
    #[test]
    fn save_em_v54_sobe_para_v55_preservando_o_resultado_antigo() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        // Simula um save parado na v54: baseline + v54, sem a v55.
        migrate_baseline(&conn).expect("baseline");
        migrate_v54_forma_do_piloto(&conn).expect("v54");
        set_schema_version(&conn, 54).expect("carimbo v54");
        // O que está sob teste é a MIGRAÇÃO, não a integridade referencial: a linha
        // legada existe só pra provar que o `ALTER TABLE` não a apaga nem a reescreve.
        // Sem isto seria preciso semear calendário, piloto e equipe inteiros — três
        // fixtures que não têm nada a ver com o que este teste afirma.
        conn.execute_batch(
            "
            PRAGMA foreign_keys = OFF;
            INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final)
            VALUES ('C001', 'P001', 'T001', 7);
            PRAGMA foreign_keys = ON;
            ",
        )
        .expect("resultado antigo");

        run_pending(&conn).expect("upgrade v54 → v55");
        assert_eq!(get_schema_version(&conn).expect("versão"), CURRENT_VERSION);

        // A linha antiga sobreviveu e a coluna nova nasceu neutra — é isso que permite à
        // leitura tratar corrida pré-v55 como "sem traçado" em vez de traçado errado.
        let (posicao, ar_sujo, trafego): (i64, i64, String) = conn
            .query_row(
                "SELECT posicao_final, segmentos_em_ar_sujo, trafego_json
                 FROM race_results WHERE piloto_id = 'P001'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("linha antiga preservada");
        assert_eq!(posicao, 7);
        assert_eq!(ar_sujo, 0);
        assert_eq!(trafego, "{}");
    }

    #[test]
    fn v58_semeia_carro_preservando_a_ordem_do_grid_dentro_da_classe() {
        let conn = Connection::open_in_memory().expect("db");
        // Save parado na v57: tem `team_car`, mas as equipes do bloco especial nunca
        // criaram linha nela.
        migrate_baseline(&conn).expect("baseline");
        migrate_v54_forma_do_piloto(&conn).expect("v54");
        migrate_v55_leitura_da_corrida(&conn).expect("v55");
        migrate_v56_faixa_anunciada(&conn).expect("v56");
        migrate_v57_leitura_do_fim_de_semana(&conn).expect("v57");
        set_schema_version(&conn, 57).expect("carimbo v57");

        conn.execute_batch(
            "
            PRAGMA foreign_keys = OFF;
            INSERT INTO teams (id, nome, nome_curto, categoria, classe, ativa, car_performance,
                               created_at, updated_at)
            VALUES
                -- Endurance/GT3: a ponta larga do runaway da coluna legada.
                ('T001', 'Forte',  'FRT', 'endurance', 'gt3',  1, 79.6, '', ''),
                ('T002', 'Media',  'MED', 'endurance', 'gt3',  1, 74.0, '', ''),
                ('T003', 'Fraca',  'FRC', 'endurance', 'gt3',  1, 73.7, '', ''),
                -- Endurance/GT4: classe irmã, escala própria.
                ('T004', 'G4Alta', 'G4A', 'endurance', 'gt4',  1, 75.3, '', ''),
                ('T005', 'G4Baixa','G4B', 'endurance', 'gt4',  1, 70.1, '', ''),
                -- Já tem carro: a migração não pode tocar nela.
                ('T006', 'ComCarro','CC', 'gt3',       NULL,   1, 16.0, '', '');

            INSERT INTO team_car (team_id, part_type, level, wear)
            VALUES ('T006', 'engine', 4, 0.5);
            PRAGMA foreign_keys = ON;
            ",
        )
        .expect("save v57 sem carro no bloco especial");

        run_pending(&conn).expect("upgrade v57 → v58");
        assert_eq!(get_schema_version(&conn).expect("versão"), CURRENT_VERSION);

        let level = |team_id: &str| -> i64 {
            conn.query_row(
                "SELECT MAX(level) FROM team_car WHERE team_id = ?1",
                [team_id],
                |row| row.get(0),
            )
            .expect("nivel do carro")
        };
        let parts = |team_id: &str| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM team_car WHERE team_id = ?1",
                [team_id],
                |row| row.get(0),
            )
            .expect("contagem de pecas")
        };

        // Todas ganharam as 11 peças.
        for team_id in ["T001", "T002", "T003", "T004", "T005"] {
            assert_eq!(parts(team_id), 11, "{team_id} deveria ter 11 pecas");
        }

        // A ordem do grid dentro da classe sobrevive à troca de escala.
        assert!(
            level("T001") > level("T003"),
            "a mais forte do GT3 ({}) deveria manter nivel acima da mais fraca ({})",
            level("T001"),
            level("T003")
        );
        assert!(level("T004") > level("T005"));

        // O teto é o da CLASSE: GT3 do Endurance vai até 7, GT4 até 6.
        assert!(level("T001") <= 7, "GT3 do Endurance passou do teto 7");
        assert!(level("T004") <= 6, "GT4 do Endurance passou do teto 6");

        // Quem já tinha carro fica intacto — a migração não reescreve nível nem desgaste.
        assert_eq!(parts("T006"), 1);
        assert_eq!(level("T006"), 4);
    }

    fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table_info");
        let mut rows = stmt.query([]).expect("rows");
        while let Some(row) = rows.next().expect("row") {
            let name: String = row.get("name").expect("name");
            if name == column {
                return true;
            }
        }
        false
    }
}
