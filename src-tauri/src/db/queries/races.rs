use crate::db::connection::DbError;
use crate::simulation::race::RaceDriverResult;
use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Os vetores POR TRECHO de um carro, como vão pra coluna `race_results.trafego_json`.
///
/// Espelha o dono do dado na simulação (`RaceState::trafego`). Vive em JSON porque a
/// aridade é constante de OUTRO módulo ("5 trechos") e porque `gaps_ms` tem elemento
/// nulo — o líder não tem carro à frente. Ver a v55 em `db::migrations`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrafegoDaCorrida {
    /// Posição ao fim de cada trecho. Vazio = corrida anterior à v55 (nada a desenhar).
    #[serde(default)]
    pub posicoes: Vec<i32>,
    /// Gap pro carro da frente ao fim de cada trecho, em ms. `null` = era o líder.
    #[serde(default)]
    pub gaps_ms: Vec<Option<f64>>,
}

/// As paradas de um carro, como vão pra coluna `race_results.paradas_json`.
///
/// Espelha `RaceState::paradas`. Aridade genuinamente variável (0..N paradas), e os três
/// vetores são paralelos: o i-ésimo elemento de cada um descreve a i-ésima parada.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParadasDaCorrida {
    #[serde(default)]
    pub voltas: Vec<u32>,
    #[serde(default)]
    pub antes: Vec<i32>,
    #[serde(default)]
    pub depois: Vec<i32>,
}

/// Um safety car da corrida, já lido do banco.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyCarDaCorrida {
    /// Volta de entrada.
    pub volta: u32,
    /// A classificação no momento em que ele entrou, em ordem de posição.
    pub ordem_pre_safety_car: Vec<String>,
}

/// A LEITURA de um carro numa corrida: tudo o que o motor calculou pra explicar a
/// posição final e que antes da v55 morria em `build_race_results`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LeituraDoCarro {
    pub piloto_id: String,
    /// Resolvido por join com `drivers` — a tela não deveria precisar de uma 2ª consulta
    /// só pra saber de quem é a linha.
    pub piloto_nome: String,
    pub is_jogador: bool,
    pub posicao_largada: i32,
    pub posicao_final: i32,
    pub is_dnf: bool,
    pub segmentos_em_ar_sujo: i32,
    pub tentativas_ultrapassagem: i32,
    pub ultrapassagens_concluidas: i32,
    pub tentativas_sofridas: i32,
    pub maior_sequencia_preso: i32,
    pub estrategia_id: String,
    pub trafego: TrafegoDaCorrida,
    pub paradas: ParadasDaCorrida,
    /// A leitura do fim de semana ANUNCIADA antes desta corrida, como gravada — vinda da
    /// tabela própria da v57, por join.
    ///
    /// JSON cru de propósito: o formato é do DTO da fase 3, e esta camada só o carrega —
    /// interpretá-lo aqui acoplaria a query à apresentação. `'{}'` = não anunciada.
    pub leitura_fds_json: String,
}

pub fn insert_race_results_batch(
    tx: &Transaction<'_>,
    race_id: &str,
    results: &[RaceDriverResult],
) -> Result<(), DbError> {
    if results.is_empty() {
        return Err(DbError::InvalidData(format!(
            "batch de race_results vazio para corrida '{race_id}'"
        )));
    }

    let mut seen_pilots = HashSet::new();
    for result in results {
        if !seen_pilots.insert(result.pilot_id.clone()) {
            return Err(DbError::InvalidData(format!(
                "piloto duplicado no batch de race_results para corrida '{race_id}': {}",
                result.pilot_id
            )));
        }
    }

    let mut stmt = tx.prepare(
        "
        INSERT INTO race_results (
            race_id,
            piloto_id,
            equipe_id,
            posicao_largada,
            posicao_final,
            voltas_completadas,
            dnf,
            pontos,
            tempo_total,
            fastest_lap,
            dnf_reason,
            dnf_reason_key,
            dnf_segment,
            incidents_count,
            gap_to_winner_ms,
            final_tire_wear,
            dnf_catalog_id,
            damage_origin_segment,
            segmentos_em_ar_sujo,
            tentativas_ultrapassagem,
            ultrapassagens_concluidas,
            tentativas_sofridas,
            maior_sequencia_preso,
            estrategia_id,
            trafego_json,
            paradas_json
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
            ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26
        )
        ",
    )?;

    for result in results {
        let dnf_int = if result.is_dnf { 1 } else { 0 };
        let fastest_lap_int = if result.has_fastest_lap { 1 } else { 0 };

        // Os vetores por trecho e as paradas viram um blob cada. `unwrap_or` pro objeto
        // vazio: um erro de serialização aqui não pode derrubar a transação do fim de
        // semana inteiro — perder o traçado de uma corrida é infinitamente melhor que
        // perder o resultado dela.
        let trafego = serde_json::to_string(&TrafegoDaCorrida {
            posicoes: result.posicoes_por_segmento.clone(),
            gaps_ms: result.gaps_para_da_frente_ms.clone(),
        })
        .unwrap_or_else(|_| "{}".to_string());
        let paradas = serde_json::to_string(&ParadasDaCorrida {
            voltas: result.volta_da_parada.clone(),
            antes: result.posicao_antes_da_parada.clone(),
            depois: result.posicao_depois.clone(),
        })
        .unwrap_or_else(|_| "{}".to_string());

        stmt.execute(rusqlite::params![
            race_id,
            result.pilot_id,
            result.team_id,
            result.grid_position,
            result.finish_position,
            result.laps_completed,
            dnf_int,
            result.points_earned as f64,
            result.total_race_time_ms,
            fastest_lap_int,
            result.dnf_reason,
            result.dnf_reason_key,
            result.dnf_segment,
            result.incidents_count,
            result.gap_to_winner_ms,
            result.final_tire_wear,
            result.dnf_catalog_id,
            result.damage_origin_segment,
            result.segmentos_em_ar_sujo,
            result.tentativas_ultrapassagem,
            result.ultrapassagens_concluidas,
            result.tentativas_sofridas,
            result.maior_sequencia_preso,
            result.estrategia_id,
            trafego,
            paradas
        ])?;
    }

    Ok(())
}

/// Grava a leitura do fim de semana ANUNCIADA de cada piloto desta corrida (v57).
///
/// Recebe uma `Connection`, não uma `Transaction`, porque o ponto da v57 é justamente que
/// esta linha nasce no PREPARO DA ETAPA — antes da corrida, portanto fora da transação do
/// resultado, e antes de a linha de `race_results` sequer existir.
///
/// Recebe o JSON já serializado porque o formato é do DTO da fase 3
/// (`career_types::WeekendReading`): esta camada carrega o blob, não o interpreta.
///
/// `INSERT OR REPLACE` torna o preparo idempotente: reabrir a Sala de Estratégia reescreve
/// o mesmo par `(corrida, piloto)` em vez de duplicar. Lista vazia é no-op.
pub fn set_race_weekend_readings(
    conn: &Connection,
    race_id: &str,
    leituras: &[(String, String)],
) -> Result<(), DbError> {
    if leituras.is_empty() {
        return Ok(());
    }

    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO race_weekend_readings (race_id, driver_id, leitura_json)
         VALUES (?1, ?2, ?3)",
    )?;
    for (piloto_id, json) in leituras {
        stmt.execute(rusqlite::params![race_id, piloto_id, json])?;
    }
    Ok(())
}

/// A leitura ANUNCIADA de um piloto numa corrida, como JSON cru. `None` = não anunciada.
///
/// É o que a Sala de Estratégia lê para NÃO recalcular: o anúncio nasce uma vez e as duas
/// telas leem a mesma linha. Se voltar `None`, quem chama deve calcular e gravar via
/// [`set_race_weekend_readings`] — e a partir daí a leitura está congelada para aquele fim
/// de semana.
pub fn get_race_weekend_reading(
    conn: &Connection,
    race_id: &str,
    driver_id: &str,
) -> Result<Option<String>, DbError> {
    conn.query_row(
        "SELECT leitura_json FROM race_weekend_readings WHERE race_id = ?1 AND driver_id = ?2",
        rusqlite::params![race_id, driver_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(DbError::Sqlite)
}

/// Grava os safety cars da corrida — uma linha por entrada, na ordem em que saíram.
///
/// Chamado dentro da MESMA transação do resultado. Corrida sem safety car não grava
/// linha nenhuma, e é assim que a leitura sabe que não houve (em vez de não ter sido
/// gravado — ver o default neutro da v55). `INSERT OR REPLACE` porque a chave é
/// (corrida, ordem): reimportar a mesma corrida sobrescreve em vez de duplicar.
pub fn insert_race_safety_cars(
    tx: &Transaction<'_>,
    race_id: &str,
    voltas: &[u32],
    ordens: &[Vec<String>],
) -> Result<(), DbError> {
    if voltas.is_empty() {
        return Ok(());
    }

    let mut stmt = tx.prepare(
        "INSERT OR REPLACE INTO race_safety_cars (race_id, ordem, volta, grid_json)
         VALUES (?1, ?2, ?3, ?4)",
    )?;

    for (idx, volta) in voltas.iter().enumerate() {
        // A ordem do pelotão é opcional na prática: o motor empurra as duas listas em
        // paralelo, mas um safety car sem ordem registrada ainda vale como fato ("saiu na
        // volta 12"), então o `get` que falha vira lista vazia em vez de erro.
        let grid = ordens
            .get(idx)
            .map(|o| serde_json::to_string(o).unwrap_or_else(|_| "[]".to_string()))
            .unwrap_or_else(|| "[]".to_string());
        stmt.execute(rusqlite::params![race_id, idx as i64, *volta, grid])?;
    }

    Ok(())
}

/// Lê a leitura de todos os carros de uma corrida, em ordem de chegada (DNF por último).
///
/// Corrida gravada antes da v55 volta com os escalares em 0 e os vetores vazios — é o
/// default da migração, e é o que a tela usa pra decidir não desenhar nada.
pub fn get_race_reading(conn: &Connection, race_id: &str) -> Result<Vec<LeituraDoCarro>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.piloto_id, r.posicao_largada, r.posicao_final, r.dnf,
                r.segmentos_em_ar_sujo, r.tentativas_ultrapassagem, r.ultrapassagens_concluidas,
                r.tentativas_sofridas, r.maior_sequencia_preso, r.estrategia_id,
                r.trafego_json, r.paradas_json,
                COALESCE(d.nome, ''), COALESCE(d.is_jogador, 0),
                COALESCE(w.leitura_json, '{}')
         FROM race_results r
         LEFT JOIN drivers d ON d.id = r.piloto_id
         LEFT JOIN race_weekend_readings w
                ON w.race_id = r.race_id AND w.driver_id = r.piloto_id
         WHERE r.race_id = ?1
         ORDER BY r.dnf ASC, r.posicao_final ASC",
    )?;

    let rows = stmt.query_map([race_id], |row| {
        let trafego_json: String = row.get(10)?;
        let paradas_json: String = row.get(11)?;
        Ok(LeituraDoCarro {
            piloto_id: row.get(0)?,
            piloto_nome: row.get(12)?,
            is_jogador: row.get::<_, i64>(13)? != 0,
            posicao_largada: row.get(1)?,
            posicao_final: row.get(2)?,
            is_dnf: row.get::<_, i64>(3)? != 0,
            segmentos_em_ar_sujo: row.get(4)?,
            tentativas_ultrapassagem: row.get(5)?,
            ultrapassagens_concluidas: row.get(6)?,
            tentativas_sofridas: row.get(7)?,
            maior_sequencia_preso: row.get(8)?,
            estrategia_id: row.get(9)?,
            // JSON corrompido não derruba a leitura da corrida: vira vetor vazio, que a
            // tela já sabe tratar como "sem traçado".
            trafego: serde_json::from_str(&trafego_json).unwrap_or_default(),
            paradas: serde_json::from_str(&paradas_json).unwrap_or_default(),
            leitura_fds_json: row.get(14)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::Sqlite)
}

/// Os safety cars de uma corrida, na ordem em que saíram. Vazio = não houve (ou corrida
/// anterior à v55).
pub fn get_race_safety_cars(
    conn: &Connection,
    race_id: &str,
) -> Result<Vec<SafetyCarDaCorrida>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT volta, grid_json FROM race_safety_cars WHERE race_id = ?1 ORDER BY ordem ASC",
    )?;

    let rows = stmt.query_map([race_id], |row| {
        let grid_json: String = row.get(1)?;
        Ok(SafetyCarDaCorrida {
            volta: row.get(0)?,
            ordem_pre_safety_car: serde_json::from_str(&grid_json).unwrap_or_default(),
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::Sqlite)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::teams::get_team_templates;
    use crate::db::migrations::run_all;
    use crate::db::queries::{
        drivers as driver_queries, seasons as season_queries, teams as team_queries,
    };
    use crate::models::driver::Driver;
    use crate::models::season::Season;
    use crate::models::team::Team;
    use crate::simulation::race::{ClassificationStatus, RaceDriverResult};
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_all(&conn).unwrap();

        let season = Season::new("S001".to_string(), 1, 2026);
        season_queries::insert_season(&conn, &season).unwrap();
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao, data)
             VALUES ('C001', 'S001', 1, 'Interlagos', 'gt3', 'Seco', 60, '2026-01-01')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO races (id, temporada_id, calendar_id, rodada, pista, data, clima, status)
             VALUES ('R001', 'S001', 'C001', 1, 'Interlagos', '2026-01-01', 'Seco', 'Pendente')",
            [],
        )
        .unwrap();

        let mut driver = Driver::new(
            "P001".to_string(),
            "Driver 1".to_string(),
            "BR".to_string(),
            "M".to_string(),
            25,
            2020,
        );
        driver.categoria_atual = Some("gt3".to_string());
        driver_queries::insert_driver(&conn, &driver).unwrap();

        let template = get_team_templates("gt3")[0];
        let mut rng = StdRng::seed_from_u64(42);
        let team =
            Team::from_template_with_rng(template, "gt3", "T001".to_string(), 2026, &mut rng);
        team_queries::insert_team(&conn, &team).unwrap();

        conn
    }

    fn sample_result() -> RaceDriverResult {
        RaceDriverResult {
            pilot_id: "P001".to_string(),
            pilot_name: "Driver 1".to_string(),
            team_id: "T001".to_string(),
            team_name: "Team 1".to_string(),
            grid_position: 1,
            finish_position: 1,
            positions_gained: 0,
            best_lap_time_ms: 90000.0,
            total_race_time_ms: 3_600_000.0,
            gap_to_winner_ms: 0.0,
            is_dnf: false,
            dnf_reason: None,
            dnf_reason_key: None,
            dnf_segment: None,
            incidents_count: 0,
            incidents: Vec::new(),
            has_fastest_lap: true,
            points_earned: 25,
            is_jogador: false,
            laps_completed: 20,
            final_tire_wear: 0.4,
            final_physical: 0.8,
            classification_status: ClassificationStatus::Finished,
            notable_incident: None,
            dnf_catalog_id: None,
            damage_origin_segment: None,
            posicoes_por_segmento: Vec::new(),
            gaps_para_da_frente_ms: Vec::new(),
            segmentos_em_ar_sujo: 0,
            tentativas_ultrapassagem: 0,
            ultrapassagens_concluidas: 0,
            tentativas_sofridas: 0,
            maior_sequencia_preso: 0,
            volta_da_parada: Vec::new(),
            posicao_antes_da_parada: Vec::new(),
            posicao_depois: Vec::new(),
            estrategia_id: String::new(),
        }
    }

    #[test]
    fn test_insert_race_results_batch_rejects_empty_batch() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();

        let err = insert_race_results_batch(&tx, "R001", &[]).expect_err("empty batch should fail");
        assert!(err.to_string().contains("batch de race_results vazio"));
    }

    #[test]
    fn test_insert_race_results_batch_rejects_duplicate_pilot_in_same_race() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();

        let result = sample_result();
        let err = insert_race_results_batch(&tx, "R001", &[result.clone(), result])
            .expect_err("duplicate pilot should fail");
        assert!(err.to_string().contains("piloto duplicado"));
    }

    /// O dado que EXPLICA o resultado sobrevive ao save (v55): escalares nas colunas,
    /// vetores nos dois blobs, e volta idêntico na leitura. É o teste que impede a
    /// regressão silenciosa — o motor calculando e o banco engolindo.
    #[test]
    fn leitura_da_corrida_faz_ida_e_volta_completa() {
        let mut conn = setup_test_db();

        let mut result = sample_result();
        result.finish_position = 6;
        result.grid_position = 4;
        result.posicoes_por_segmento = vec![4, 5, 9, 7, 6];
        // O líder não tem carro à frente: o `None` no meio é justamente o caso que
        // coluna escalar não representaria sem inventar sentinela.
        result.gaps_para_da_frente_ms = vec![Some(820.0), None, Some(1400.5), Some(310.0), None];
        result.segmentos_em_ar_sujo = 3;
        result.tentativas_ultrapassagem = 2;
        result.ultrapassagens_concluidas = 0;
        result.tentativas_sofridas = 5;
        result.maior_sequencia_preso = 3;
        result.estrategia_id = "1-parada-cedo".to_string();
        result.volta_da_parada = vec![12];
        result.posicao_antes_da_parada = vec![4];
        result.posicao_depois = vec![9];

        let tx = conn.transaction().unwrap();
        insert_race_results_batch(&tx, "C001", &[result]).expect("inserção");
        insert_race_safety_cars(
            &tx,
            "C001",
            &[18],
            &[vec!["P001".to_string(), "P002".to_string()]],
        )
        .expect("safety cars");
        tx.commit().unwrap();

        let leitura = get_race_reading(&conn, "C001").expect("leitura");
        assert_eq!(leitura.len(), 1);
        let carro = &leitura[0];
        assert_eq!(carro.piloto_id, "P001");
        // O join com `drivers` resolve o nome: a tela não precisa de 2ª consulta.
        assert_eq!(carro.piloto_nome, "Driver 1");
        assert_eq!(carro.posicao_largada, 4);
        assert_eq!(carro.posicao_final, 6);
        assert_eq!(carro.trafego.posicoes, vec![4, 5, 9, 7, 6]);
        assert_eq!(
            carro.trafego.gaps_ms,
            vec![Some(820.0), None, Some(1400.5), Some(310.0), None]
        );
        assert_eq!(carro.segmentos_em_ar_sujo, 3);
        assert_eq!(carro.tentativas_ultrapassagem, 2);
        assert_eq!(carro.ultrapassagens_concluidas, 0);
        assert_eq!(carro.tentativas_sofridas, 5);
        assert_eq!(carro.maior_sequencia_preso, 3);
        assert_eq!(carro.estrategia_id, "1-parada-cedo");
        // Os três vetores de parada são paralelos: largou P4, parou na volta 12, voltou P9.
        assert_eq!(carro.paradas.voltas, vec![12]);
        assert_eq!(carro.paradas.antes, vec![4]);
        assert_eq!(carro.paradas.depois, vec![9]);

        let scs = get_race_safety_cars(&conn, "C001").expect("safety cars");
        assert_eq!(scs.len(), 1);
        assert_eq!(scs[0].volta, 18);
        assert_eq!(scs[0].ordem_pre_safety_car, vec!["P001", "P002"]);
    }

    /// Corrida SEM o dado novo (import do iRacing, ou save anterior à v55) precisa voltar
    /// neutra em vez de erro — e com os vetores VAZIOS, que é como a tela distingue
    /// "não aconteceu" de "não foi gravado" e decide não desenhar traçado nenhum.
    #[test]
    fn corrida_sem_dado_de_trecho_volta_neutra_e_nao_quebra() {
        let mut conn = setup_test_db();

        let tx = conn.transaction().unwrap();
        insert_race_results_batch(&tx, "C001", &[sample_result()]).expect("inserção");
        tx.commit().unwrap();

        let leitura = get_race_reading(&conn, "C001").expect("leitura");
        let carro = &leitura[0];
        assert!(carro.trafego.posicoes.is_empty());
        assert!(carro.trafego.gaps_ms.is_empty());
        assert!(carro.paradas.voltas.is_empty());
        assert_eq!(carro.segmentos_em_ar_sujo, 0);
        assert_eq!(carro.estrategia_id, "");
        // Sem safety car gravado não existe linha — e não é um erro.
        assert!(get_race_safety_cars(&conn, "C001")
            .expect("safety cars")
            .is_empty());
    }

    /// A faixa ANUNCIADA sobrevive por piloto e não é recomputada na leitura (v56). É o
    /// que impede uma recalibração do σ de mudar, retroativamente, o que foi anunciado no
    /// sábado de uma corrida antiga — a tela pós-corrida é revisitável.
    fn blob_de_leitura() -> &'static str {
        r#"{"race_id":"C001","available":true,"track_affinity":{"band":1,"qualifying_band":2,"trend":null,"support":null},"driver_form":{"band":-1,"qualifying_band":-1,"trend":-1,"support":null},"car_setup":{"band":0,"qualifying_band":0,"trend":null,"support":null}}"#
    }

    /// **O teste que justifica a v57.** A leitura é gravada e lida ANTES de existir qualquer
    /// linha de `race_results` — que é exatamente o que a coluna da v56 tornava impossível,
    /// porque a linha dela só nasce depois da corrida.
    ///
    /// É o que permite o anúncio nascer uma vez, no preparo da etapa, e as duas telas lerem
    /// a mesma linha em vez de recalcular cada uma no seu instante.
    #[test]
    fn leitura_do_fim_de_semana_nasce_antes_de_existir_resultado() {
        let conn = setup_test_db();

        set_race_weekend_readings(
            &conn,
            "C001",
            &[("P001".to_string(), blob_de_leitura().to_string())],
        )
        .expect("anúncio no preparo da etapa");

        // Nenhuma corrida foi rodada ainda, e a leitura já está lá.
        assert_eq!(
            get_race_weekend_reading(&conn, "C001", "P001").expect("leitura"),
            Some(blob_de_leitura().to_string())
        );
        assert!(get_race_reading(&conn, "C001").expect("leitura").is_empty());
    }

    /// O preparo da etapa é IDEMPOTENTE: reabrir a Sala de Estratégia reescreve o mesmo par
    /// (corrida, piloto) em vez de duplicar, e o anúncio congelado não muda.
    #[test]
    fn regravar_a_leitura_do_mesmo_piloto_sobrescreve_sem_duplicar() {
        let conn = setup_test_db();
        let par = [("P001".to_string(), blob_de_leitura().to_string())];

        set_race_weekend_readings(&conn, "C001", &par).expect("1ª gravação");
        set_race_weekend_readings(&conn, "C001", &par).expect("2ª gravação");

        let linhas: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM race_weekend_readings WHERE race_id = 'C001'",
                [],
                |row| row.get(0),
            )
            .expect("contagem");
        assert_eq!(linhas, 1);
    }

    /// Depois da corrida, a leitura da corrida traz a faixa anunciada por JOIN — a mesma
    /// linha que foi gravada antes da largada.
    #[test]
    fn faixa_anunciada_chega_na_leitura_da_corrida_por_join() {
        let mut conn = setup_test_db();
        set_race_weekend_readings(
            &conn,
            "C001",
            &[("P001".to_string(), blob_de_leitura().to_string())],
        )
        .expect("anúncio antes da corrida");

        let tx = conn.transaction().unwrap();
        insert_race_results_batch(&tx, "C001", &[sample_result()]).expect("inserção");
        tx.commit().unwrap();

        let leitura = get_race_reading(&conn, "C001").expect("leitura");
        assert_eq!(leitura[0].leitura_fds_json, blob_de_leitura());
    }

    /// Sem anúncio o JOIN não acha linha e o `COALESCE` devolve `'{}'` — que NÃO é uma
    /// leitura válida, e por isso a exibição trata como "não anunciado" em vez de "morno".
    #[test]
    fn corrida_sem_faixa_anunciada_volta_vazia() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();
        insert_race_results_batch(&tx, "C001", &[sample_result()]).expect("inserção");
        tx.commit().unwrap();

        assert_eq!(
            get_race_weekend_reading(&conn, "C001", "P001").expect("leitura"),
            None
        );
        let leitura = get_race_reading(&conn, "C001").expect("leitura");
        assert_eq!(leitura[0].leitura_fds_json, "{}");
    }

    /// Safety car é opcional: uma corrida sem nenhum não grava linha, e a chamada com
    /// vetor vazio é um no-op silencioso (não um erro, ao contrário do batch vazio de
    /// resultados — lá o vazio significa perda de dado, aqui significa corrida limpa).
    #[test]
    fn corrida_sem_safety_car_nao_grava_linha() {
        let mut conn = setup_test_db();
        let tx = conn.transaction().unwrap();
        insert_race_safety_cars(&tx, "C001", &[], &[]).expect("no-op");
        tx.commit().unwrap();

        assert!(get_race_safety_cars(&conn, "C001")
            .expect("safety cars")
            .is_empty());
    }

    // ───────── v68: o motivo do abandono POR QUEBRA não congela no idioma da corrida ─────────

    /// Grava um abandono de quebra num locale e lê o motivo pelo `get_event_results` em outro.
    fn motivo_gravado_em_lido_em(locale_gravacao: &str, locale_leitura: &str) -> Option<String> {
        use crate::car::breakdown::{problem_key, Severity};
        use crate::car::PartType;
        use crate::db::queries::race_history::get_event_results;

        let anterior = rust_i18n::locale().to_string();
        let mut conn = setup_test_db();

        rust_i18n::set_locale(locale_gravacao);
        let chave = problem_key(PartType::Gearbox, 0, Severity::Dnf);
        let mut result = sample_result();
        result.is_dnf = true;
        result.classification_status = ClassificationStatus::Dnf;
        // Exatamente o que os dois caminhos de quebra fazem: frase de agora + chave.
        result.marcar_dnf_de_quebra(
            crate::car::breakdown::problem_text(PartType::Gearbox, 0, Severity::Dnf),
            Some(chave),
        );

        let tx = conn.transaction().unwrap();
        insert_race_results_batch(&tx, "C001", &[result]).expect("inserção");
        tx.commit().unwrap();

        rust_i18n::set_locale(locale_leitura);
        let motivo = get_event_results(&conn, "C001").expect("resultados")[0]
            .dnf_reason
            .clone();
        rust_i18n::set_locale(&anterior);
        motivo
    }

    /// **Carreira jogada em pt-BR, lida em en-US.** É o bug que a v68 fecha: o painel de quebra
    /// já se re-renderizava pela trinca de `race_breakdowns` e o motivo do resultado, ao lado,
    /// continuava em português para sempre.
    ///
    /// `#[serial]` porque o locale é global do processo.
    #[test]
    #[serial_test::serial]
    fn dnf_de_quebra_gravado_em_pt_br_e_lido_em_ingles() {
        assert_eq!(
            motivo_gravado_em_lido_em("pt-BR", "en-US").as_deref(),
            Some("gearbox broke")
        );
    }

    /// O caminho inverso, e ele não é simetria de fachada: quem joga em inglês e volta para o
    /// português tem o mesmo direito, e é o sentido em que a heurística de "traduzir a prosa"
    /// falharia mais feio.
    #[test]
    #[serial_test::serial]
    fn dnf_de_quebra_gravado_em_ingles_e_lido_em_pt_br() {
        assert_eq!(
            motivo_gravado_em_lido_em("en-US", "pt-BR").as_deref(),
            Some("câmbio quebrou")
        );
    }

    /// **Save legado.** Linha gravada antes da v68: só prosa, `dnf_reason_key` NULL. A leitura
    /// devolve o texto de então, sem falhar e sem tentar adivinhar chave a partir da frase.
    #[test]
    #[serial_test::serial]
    fn save_legado_sem_chave_continua_mostrando_o_texto_gravado() {
        use crate::db::queries::race_history::get_event_results;

        let anterior = rust_i18n::locale().to_string();
        let mut conn = setup_test_db();

        rust_i18n::set_locale("pt-BR");
        let mut result = sample_result();
        result.is_dnf = true;
        result.classification_status = ClassificationStatus::Dnf;
        result.dnf_reason = Some("motor fundiu por superaquecimento".to_string());
        result.dnf_reason_key = None; // o que toda linha anterior à v68 tem
        let tx = conn.transaction().unwrap();
        insert_race_results_batch(&tx, "C001", &[result]).expect("inserção");
        tx.commit().unwrap();

        rust_i18n::set_locale("en-US");
        let motivo = get_event_results(&conn, "C001").expect("resultados")[0]
            .dnf_reason
            .clone();
        rust_i18n::set_locale(&anterior);

        assert_eq!(motivo.as_deref(), Some("motor fundiu por superaquecimento"));
    }

    /// **Abandono que NÃO é quebra** (batida, erro de pilotagem, pane do catálogo) continua
    /// exatamente como era: o texto do incidente, sem chave e sem re-render. A v68 não converte
    /// motivo genérico em chave, porque não existe estrutura segura para isso.
    #[test]
    #[serial_test::serial]
    fn dnf_que_nao_e_quebra_atravessa_intacto() {
        use crate::db::queries::race_history::get_event_results;

        let anterior = rust_i18n::locale().to_string();
        let mut conn = setup_test_db();

        rust_i18n::set_locale("pt-BR");
        let mut result = sample_result();
        result.is_dnf = true;
        result.classification_status = ClassificationStatus::Dnf;
        result.dnf_reason = Some("rodou sozinho na saída da última curva".to_string());
        let tx = conn.transaction().unwrap();
        insert_race_results_batch(&tx, "C001", &[result]).expect("inserção");
        tx.commit().unwrap();

        let chave: Option<String> = conn
            .query_row(
                "SELECT dnf_reason_key FROM race_results WHERE race_id = 'C001'",
                [],
                |r| r.get(0),
            )
            .expect("coluna");
        assert_eq!(chave, None, "abandono sem quebra não pode ganhar chave");

        rust_i18n::set_locale("en-US");
        let motivo = get_event_results(&conn, "C001").expect("resultados")[0]
            .dnf_reason
            .clone();
        rust_i18n::set_locale(&anterior);

        assert_eq!(
            motivo.as_deref(),
            Some("rodou sozinho na saída da última curva")
        );
    }
}
