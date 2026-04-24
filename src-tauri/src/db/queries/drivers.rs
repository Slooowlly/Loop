#![allow(dead_code)]

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::models::driver::{Driver, DriverAttributes, DriverCareerStats, DriverSeasonStats};
use crate::models::enums::{DriverStatus, PrimaryPersonality, SecondaryPersonality};

pub fn insert_driver(conn: &Connection, driver: &Driver) -> Result<(), DbError> {
    let historico = serialize_json_field(&driver.historico_circuitos, "historico_circuitos")?;
    let ultimos = serialize_json_field(&driver.ultimos_resultados, "ultimos_resultados")?;

    conn.execute(
        "INSERT INTO drivers (
            id, nome, is_jogador, idade, nacionalidade, genero, categoria_atual,
            categoria_especial_ativa, status, personalidade_primaria, personalidade_secundaria,
            ano_inicio_carreira, skill, consistencia, racecraft, defesa, ritmo_classificacao,
            gestao_pneus, habilidade_largada, adaptabilidade, fator_chuva, fitness, experiencia,
            desenvolvimento, aggression, smoothness, midia, mentalidade, confianca,
            temp_pontos, temp_vitorias, temp_podios, temp_poles, temp_corridas, temp_dnfs,
            temp_posicao_media, carreira_pontos_total, carreira_vitorias, carreira_podios,
            carreira_poles, carreira_corridas, carreira_temporadas, carreira_titulos,
            carreira_dnfs, motivacao, historico_circuitos, ultimos_resultados,
            melhor_resultado_temp, temporadas_na_categoria, corridas_na_categoria,
            temporadas_motivacao_baixa
        ) VALUES (
            :id, :nome, :is_jogador, :idade, :nacionalidade, :genero, :categoria_atual,
            :categoria_especial_ativa, :status, :personalidade_primaria, :personalidade_secundaria,
            :ano_inicio_carreira, :skill, :consistencia, :racecraft, :defesa, :ritmo_classificacao,
            :gestao_pneus, :habilidade_largada, :adaptabilidade, :fator_chuva, :fitness, :experiencia,
            :desenvolvimento, :aggression, :smoothness, :midia, :mentalidade, :confianca,
            :temp_pontos, :temp_vitorias, :temp_podios, :temp_poles, :temp_corridas, :temp_dnfs,
            :temp_posicao_media, :carreira_pontos_total, :carreira_vitorias, :carreira_podios,
            :carreira_poles, :carreira_corridas, :carreira_temporadas, :carreira_titulos,
            :carreira_dnfs, :motivacao, :historico_circuitos, :ultimos_resultados,
            :melhor_resultado_temp, :temporadas_na_categoria, :corridas_na_categoria,
            :temporadas_motivacao_baixa
        )",
        rusqlite::named_params! {
            ":id": &driver.id,
            ":nome": &driver.nome,
            ":is_jogador": driver.is_jogador as i64,
            ":idade": driver.idade as i64,
            ":nacionalidade": &driver.nacionalidade,
            ":genero": &driver.genero,
            ":categoria_atual": &driver.categoria_atual,
            ":categoria_especial_ativa": &driver.categoria_especial_ativa,
            ":status": driver.status.as_str(),
            ":personalidade_primaria": driver.personalidade_primaria.as_ref().map(|p| p.as_str()),
            ":personalidade_secundaria": driver.personalidade_secundaria.as_ref().map(|p| p.as_str()),
            ":ano_inicio_carreira": driver.ano_inicio_carreira as i64,
            ":skill": driver.atributos.skill,
            ":consistencia": driver.atributos.consistencia,
            ":racecraft": driver.atributos.racecraft,
            ":defesa": driver.atributos.defesa,
            ":ritmo_classificacao": driver.atributos.ritmo_classificacao,
            ":gestao_pneus": driver.atributos.gestao_pneus,
            ":habilidade_largada": driver.atributos.habilidade_largada,
            ":adaptabilidade": driver.atributos.adaptabilidade,
            ":fator_chuva": driver.atributos.fator_chuva,
            ":fitness": driver.atributos.fitness,
            ":experiencia": driver.atributos.experiencia,
            ":desenvolvimento": driver.atributos.desenvolvimento,
            ":aggression": driver.atributos.aggression,
            ":smoothness": driver.atributos.smoothness,
            ":midia": driver.atributos.midia,
            ":mentalidade": driver.atributos.mentalidade,
            ":confianca": driver.atributos.confianca,
            ":temp_pontos": driver.stats_temporada.pontos,
            ":temp_vitorias": driver.stats_temporada.vitorias as i64,
            ":temp_podios": driver.stats_temporada.podios as i64,
            ":temp_poles": driver.stats_temporada.poles as i64,
            ":temp_corridas": driver.stats_temporada.corridas as i64,
            ":temp_dnfs": driver.stats_temporada.dnfs as i64,
            ":temp_posicao_media": driver.stats_temporada.posicao_media,
            ":carreira_pontos_total": driver.stats_carreira.pontos_total,
            ":carreira_vitorias": driver.stats_carreira.vitorias as i64,
            ":carreira_podios": driver.stats_carreira.podios as i64,
            ":carreira_poles": driver.stats_carreira.poles as i64,
            ":carreira_corridas": driver.stats_carreira.corridas as i64,
            ":carreira_temporadas": driver.stats_carreira.temporadas as i64,
            ":carreira_titulos": driver.stats_carreira.titulos as i64,
            ":carreira_dnfs": driver.stats_carreira.dnfs as i64,
            ":motivacao": driver.motivacao,
            ":historico_circuitos": &historico,
            ":ultimos_resultados": &ultimos,
            ":melhor_resultado_temp": driver.melhor_resultado_temp.map(|v| v as i64),
            ":temporadas_na_categoria": driver.temporadas_na_categoria as i64,
            ":corridas_na_categoria": driver.corridas_na_categoria as i64,
            ":temporadas_motivacao_baixa": driver.temporadas_motivacao_baixa as i64,
        },
    )?;
    Ok(())
}

pub fn get_driver(conn: &Connection, id: &str) -> Result<Driver, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM drivers WHERE id = ?1")?;
    stmt.query_row(rusqlite::params![id], driver_from_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("Piloto '{}' nao encontrado", id))
            }
            other => map_driver_query_error(other),
        })
}

pub fn get_driver_by_name(conn: &Connection, nome: &str) -> Result<Driver, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM drivers WHERE nome = ?1 LIMIT 1")?;
    stmt.query_row(rusqlite::params![nome], driver_from_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("Piloto '{}' nao encontrado", nome))
            }
            other => map_driver_query_error(other),
        })
}

pub fn get_all_drivers(conn: &Connection) -> Result<Vec<Driver>, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM drivers ORDER BY nome")?;
    let rows = stmt.query_map([], driver_from_row)?;
    collect_drivers(rows)
}

pub fn get_drivers_by_category(conn: &Connection, categoria: &str) -> Result<Vec<Driver>, DbError> {
    let mut stmt =
        conn.prepare("SELECT * FROM drivers WHERE categoria_atual = ?1 ORDER BY nome")?;
    let rows = stmt.query_map(rusqlite::params![categoria], driver_from_row)?;
    collect_drivers(rows)
}

pub fn get_drivers_by_active_category(
    conn: &Connection,
    categoria: &str,
) -> Result<Vec<Driver>, DbError> {
    let sql = if matches!(categoria, "production_challenger" | "endurance") {
        "SELECT * FROM drivers WHERE categoria_especial_ativa = ?1 ORDER BY nome"
    } else {
        "SELECT * FROM drivers WHERE categoria_atual = ?1 AND categoria_especial_ativa IS NULL ORDER BY nome"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(rusqlite::params![categoria], driver_from_row)?;
    collect_drivers(rows)
}

pub fn get_drivers_by_status(conn: &Connection, status: &str) -> Result<Vec<Driver>, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM drivers WHERE status = ?1 ORDER BY nome")?;
    let rows = stmt.query_map(rusqlite::params![status], driver_from_row)?;
    collect_drivers(rows)
}

pub fn get_player_driver(conn: &Connection) -> Result<Driver, DbError> {
    let player_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM drivers WHERE is_jogador = 1",
        [],
        |row| row.get(0),
    )?;
    match player_count {
        0 => Err(DbError::NotFound(
            "Piloto do jogador nao encontrado".to_string(),
        )),
        1 => {
            let mut stmt = conn.prepare("SELECT * FROM drivers WHERE is_jogador = 1")?;
            stmt.query_row([], driver_from_row)
                .map_err(map_driver_query_error)
        }
        count => Err(DbError::InvalidData(format!(
            "Esperado exatamente 1 piloto do jogador, encontrado {count}"
        ))),
    }
}

pub fn get_free_drivers(conn: &Connection) -> Result<Vec<Driver>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM drivers WHERE categoria_atual IS NULL AND status = 'Ativo' ORDER BY nome",
    )?;
    let rows = stmt.query_map([], driver_from_row)?;
    collect_drivers(rows)
}

pub fn get_drivers_without_active_contract(conn: &Connection) -> Result<Vec<Driver>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT d.* FROM drivers d
         WHERE d.status = 'Ativo'
           AND NOT EXISTS (
               SELECT 1 FROM contracts c
               WHERE c.piloto_id = d.id AND c.status = 'Ativo' AND c.tipo = 'Regular'
           )
           AND NOT EXISTS (
               SELECT 1 FROM contracts c
               WHERE c.piloto_id = d.id AND c.status = 'Ativo' AND c.tipo = 'Especial'
           )
         ORDER BY d.nome",
    )?;
    let rows = stmt.query_map([], driver_from_row)?;
    collect_drivers(rows)
}

pub fn update_driver(conn: &Connection, driver: &Driver) -> Result<(), DbError> {
    let historico = serialize_json_field(&driver.historico_circuitos, "historico_circuitos")?;
    let ultimos = serialize_json_field(&driver.ultimos_resultados, "ultimos_resultados")?;

    conn.execute(
        "UPDATE drivers SET
            nome = :nome, is_jogador = :is_jogador, idade = :idade,
            nacionalidade = :nacionalidade, genero = :genero, categoria_atual = :categoria_atual,
            categoria_especial_ativa = :categoria_especial_ativa, status = :status,
            personalidade_primaria = :personalidade_primaria, personalidade_secundaria = :personalidade_secundaria,
            ano_inicio_carreira = :ano_inicio_carreira, skill = :skill, consistencia = :consistencia,
            racecraft = :racecraft, defesa = :defesa, ritmo_classificacao = :ritmo_classificacao,
            gestao_pneus = :gestao_pneus, habilidade_largada = :habilidade_largada, adaptabilidade = :adaptabilidade,
            fator_chuva = :fator_chuva, fitness = :fitness, experiencia = :experiencia, desenvolvimento = :desenvolvimento,
            aggression = :aggression, smoothness = :smoothness, midia = :midia, mentalidade = :mentalidade,
            confianca = :confianca, temp_pontos = :temp_pontos, temp_vitorias = :temp_vitorias,
            temp_podios = :temp_podios, temp_poles = :temp_poles, temp_corridas = :temp_corridas,
            temp_dnfs = :temp_dnfs, temp_posicao_media = :temp_posicao_media,
            carreira_pontos_total = :carreira_pontos_total, carreira_vitorias = :carreira_vitorias,
            carreira_podios = :carreira_podios, carreira_poles = :carreira_poles, carreira_corridas = :carreira_corridas,
            carreira_temporadas = :carreira_temporadas, carreira_titulos = :carreira_titulos, carreira_dnfs = :carreira_dnfs,
            motivacao = :motivacao, historico_circuitos = :historico_circuitos, ultimos_resultados = :ultimos_resultados,
            melhor_resultado_temp = :melhor_resultado_temp, temporadas_na_categoria = :temporadas_na_categoria,
            corridas_na_categoria = :corridas_na_categoria, temporadas_motivacao_baixa = :temporadas_motivacao_baixa
        WHERE id = :id",
        rusqlite::named_params! {
            ":id": &driver.id, ":nome": &driver.nome, ":is_jogador": driver.is_jogador as i64,
            ":idade": driver.idade as i64, ":nacionalidade": &driver.nacionalidade, ":genero": &driver.genero,
            ":categoria_atual": &driver.categoria_atual, ":categoria_especial_ativa": &driver.categoria_especial_ativa,
            ":status": driver.status.as_str(),
            ":personalidade_primaria": driver.personalidade_primaria.as_ref().map(|p| p.as_str()),
            ":personalidade_secundaria": driver.personalidade_secundaria.as_ref().map(|p| p.as_str()),
            ":ano_inicio_carreira": driver.ano_inicio_carreira as i64, ":skill": driver.atributos.skill,
            ":consistencia": driver.atributos.consistencia, ":racecraft": driver.atributos.racecraft,
            ":defesa": driver.atributos.defesa, ":ritmo_classificacao": driver.atributos.ritmo_classificacao,
            ":gestao_pneus": driver.atributos.gestao_pneus, ":habilidade_largada": driver.atributos.habilidade_largada,
            ":adaptabilidade": driver.atributos.adaptabilidade, ":fator_chuva": driver.atributos.fator_chuva,
            ":fitness": driver.atributos.fitness, ":experiencia": driver.atributos.experiencia,
            ":desenvolvimento": driver.atributos.desenvolvimento, ":aggression": driver.atributos.aggression,
            ":smoothness": driver.atributos.smoothness, ":midia": driver.atributos.midia,
            ":mentalidade": driver.atributos.mentalidade, ":confianca": driver.atributos.confianca,
            ":temp_pontos": driver.stats_temporada.pontos, ":temp_vitorias": driver.stats_temporada.vitorias as i64,
            ":temp_podios": driver.stats_temporada.podios as i64, ":temp_poles": driver.stats_temporada.poles as i64,
            ":temp_corridas": driver.stats_temporada.corridas as i64, ":temp_dnfs": driver.stats_temporada.dnfs as i64,
            ":temp_posicao_media": driver.stats_temporada.posicao_media,
            ":carreira_pontos_total": driver.stats_carreira.pontos_total, ":carreira_vitorias": driver.stats_carreira.vitorias as i64,
            ":carreira_podios": driver.stats_carreira.podios as i64, ":carreira_poles": driver.stats_carreira.poles as i64,
            ":carreira_corridas": driver.stats_carreira.corridas as i64, ":carreira_temporadas": driver.stats_carreira.temporadas as i64,
            ":carreira_titulos": driver.stats_carreira.titulos as i64, ":carreira_dnfs": driver.stats_carreira.dnfs as i64,
            ":motivacao": driver.motivacao, ":historico_circuitos": &historico, ":ultimos_resultados": &ultimos,
            ":melhor_resultado_temp": driver.melhor_resultado_temp.map(|v| v as i64),
            ":temporadas_na_categoria": driver.temporadas_na_categoria as i64,
            ":corridas_na_categoria": driver.corridas_na_categoria as i64,
            ":temporadas_motivacao_baixa": driver.temporadas_motivacao_baixa as i64,
        },
    )?;
    Ok(())
}

pub fn update_driver_stats(
    conn: &Connection,
    id: &str,
    stats: &DriverSeasonStats,
    stats_carreira: &DriverCareerStats,
    motivacao: f64,
    melhor_resultado_temp: Option<u32>,
    temporadas_na_categoria: u32,
    corridas_na_categoria: u32,
    temporadas_motivacao_baixa: u32,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE drivers SET
            temp_pontos = :temp_pontos, temp_vitorias = :temp_vitorias,
            temp_podios = :temp_podios, temp_poles = :temp_poles, temp_corridas = :temp_corridas,
            temp_dnfs = :temp_dnfs, temp_posicao_media = :temp_posicao_media,
            carreira_pontos_total = :carreira_pontos_total, carreira_vitorias = :carreira_vitorias,
            carreira_podios = :carreira_podios, carreira_poles = :carreira_poles,
            carreira_corridas = :carreira_corridas, carreira_temporadas = :carreira_temporadas,
            carreira_titulos = :carreira_titulos, carreira_dnfs = :carreira_dnfs,
            motivacao = :motivacao, melhor_resultado_temp = :melhor_resultado_temp,
            temporadas_na_categoria = :temporadas_na_categoria, corridas_na_categoria = :corridas_na_categoria,
            temporadas_motivacao_baixa = :temporadas_motivacao_baixa
        WHERE id = :id",
        rusqlite::named_params! {
            ":id": id,
            ":temp_pontos": stats.pontos,
            ":temp_vitorias": stats.vitorias as i64,
            ":temp_podios": stats.podios as i64,
            ":temp_poles": stats.poles as i64,
            ":temp_corridas": stats.corridas as i64,
            ":temp_dnfs": stats.dnfs as i64,
            ":temp_posicao_media": stats.posicao_media,
            ":carreira_pontos_total": stats_carreira.pontos_total,
            ":carreira_vitorias": stats_carreira.vitorias as i64,
            ":carreira_podios": stats_carreira.podios as i64,
            ":carreira_poles": stats_carreira.poles as i64,
            ":carreira_corridas": stats_carreira.corridas as i64,
            ":carreira_temporadas": stats_carreira.temporadas as i64,
            ":carreira_titulos": stats_carreira.titulos as i64,
            ":carreira_dnfs": stats_carreira.dnfs as i64,
            ":motivacao": motivacao,
            ":melhor_resultado_temp": melhor_resultado_temp.map(|v| v as i64),
            ":temporadas_na_categoria": temporadas_na_categoria as i64,
            ":corridas_na_categoria": corridas_na_categoria as i64,
            ":temporadas_motivacao_baixa": temporadas_motivacao_baixa as i64,
        },
    )?;
    Ok(())
}

pub fn update_driver_attributes(
    conn: &Connection,
    id: &str,
    attrs: &DriverAttributes,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE drivers SET
            skill = :skill, consistencia = :consistencia, racecraft = :racecraft, defesa = :defesa,
            ritmo_classificacao = :ritmo_classificacao, gestao_pneus = :gestao_pneus,
            habilidade_largada = :habilidade_largada, adaptabilidade = :adaptabilidade,
            fator_chuva = :fator_chuva, fitness = :fitness, experiencia = :experiencia,
            desenvolvimento = :desenvolvimento, aggression = :aggression, smoothness = :smoothness,
            midia = :midia, mentalidade = :mentalidade, confianca = :confianca
        WHERE id = :id",
        rusqlite::named_params! {
            ":id": id,
            ":skill": attrs.skill,
            ":consistencia": attrs.consistencia,
            ":racecraft": attrs.racecraft,
            ":defesa": attrs.defesa,
            ":ritmo_classificacao": attrs.ritmo_classificacao,
            ":gestao_pneus": attrs.gestao_pneus,
            ":habilidade_largada": attrs.habilidade_largada,
            ":adaptabilidade": attrs.adaptabilidade,
            ":fator_chuva": attrs.fator_chuva,
            ":fitness": attrs.fitness,
            ":experiencia": attrs.experiencia,
            ":desenvolvimento": attrs.desenvolvimento,
            ":aggression": attrs.aggression,
            ":smoothness": attrs.smoothness,
            ":midia": attrs.midia,
            ":mentalidade": attrs.mentalidade,
            ":confianca": attrs.confianca,
        },
    )?;
    Ok(())
}

pub fn update_driver_especial_category(
    conn: &Connection,
    driver_id: &str,
    categoria_especial: Option<&str>,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE drivers SET categoria_especial_ativa = ?1 WHERE id = ?2",
        rusqlite::params![categoria_especial, driver_id],
    )?;
    Ok(())
}

pub fn clear_all_categoria_especial_ativa(conn: &Connection) -> Result<usize, DbError> {
    let n = conn.execute(
        "UPDATE drivers SET categoria_especial_ativa = NULL WHERE categoria_especial_ativa IS NOT NULL",
        [],
    )?;
    Ok(n)
}

pub fn update_driver_status(
    conn: &Connection,
    id: &str,
    status: &DriverStatus,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE drivers SET status = ?1 WHERE id = ?2",
        rusqlite::params![status.as_str(), id],
    )?;
    Ok(())
}

pub fn update_driver_motivation(
    conn: &Connection,
    id: &str,
    motivacao: f64,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE drivers SET motivacao = ?1 WHERE id = ?2",
        rusqlite::params![motivacao.clamp(0.0, 100.0), id],
    )?;
    Ok(())
}

pub fn update_driver_midia(conn: &Connection, id: &str, midia: f64) -> Result<(), DbError> {
    conn.execute(
        "UPDATE drivers SET midia = ?1 WHERE id = ?2",
        rusqlite::params![midia.clamp(0.0, 100.0), id],
    )?;
    Ok(())
}

pub fn update_driver_midia_delta(conn: &Connection, id: &str, delta: f64) -> Result<(), DbError> {
    conn.execute(
        "UPDATE drivers SET midia = MAX(0.0, MIN(100.0, midia + ?1)) WHERE id = ?2",
        rusqlite::params![delta, id],
    )?;
    Ok(())
}

pub fn delete_driver(conn: &Connection, id: &str) -> Result<(), DbError> {
    conn.execute("DELETE FROM drivers WHERE id = ?1", rusqlite::params![id])?;
    Ok(())
}

pub fn count_drivers(conn: &Connection) -> Result<u32, DbError> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM drivers", [], |row| row.get(0))?;
    u32::try_from(n).map_err(|_| DbError::InvalidData(format!("Contagem de pilotos invalida: {n}")))
}

pub fn count_drivers_by_category(conn: &Connection, categoria: &str) -> Result<u32, DbError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM drivers WHERE categoria_atual = ?1",
        rusqlite::params![categoria],
        |row| row.get(0),
    )?;
    u32::try_from(n).map_err(|_| {
        DbError::InvalidData(format!(
            "Contagem de pilotos invalida para categoria '{categoria}': {n}"
        ))
    })
}

fn collect_drivers(
    mapped: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Driver>>,
) -> Result<Vec<Driver>, DbError> {
    let mut result = Vec::new();
    for row in mapped {
        result.push(row.map_err(map_driver_query_error)?);
    }
    Ok(result)
}

fn driver_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Driver> {
    let historico_str: String = row.get("historico_circuitos")?;
    let ultimos_str: String = row.get("ultimos_resultados")?;

    Ok(Driver {
        id: row.get("id")?,
        nome: row.get("nome")?,
        is_jogador: row.get::<_, i64>("is_jogador")? != 0,
        idade: parse_non_negative_u32(row, "idade")?,
        nacionalidade: row.get("nacionalidade")?,
        genero: row.get("genero")?,
        categoria_atual: row.get("categoria_atual")?,
        categoria_especial_ativa: row.get("categoria_especial_ativa")?,
        status: DriverStatus::from_str_strict(&row.get::<_, String>("status")?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        personalidade_primaria: row
            .get::<_, Option<String>>("personalidade_primaria")?
            .map(|s| PrimaryPersonality::from_str_strict(&s))
            .transpose()
            .map_err(rusqlite::Error::InvalidParameterName)?,
        personalidade_secundaria: row
            .get::<_, Option<String>>("personalidade_secundaria")?
            .map(|s| SecondaryPersonality::from_str_strict(&s))
            .transpose()
            .map_err(rusqlite::Error::InvalidParameterName)?,
        ano_inicio_carreira: parse_non_negative_u32(row, "ano_inicio_carreira")?,
        atributos: DriverAttributes {
            skill: row.get("skill")?,
            consistencia: row.get("consistencia")?,
            racecraft: row.get("racecraft")?,
            defesa: row.get("defesa")?,
            ritmo_classificacao: row.get("ritmo_classificacao")?,
            gestao_pneus: row.get("gestao_pneus")?,
            habilidade_largada: row.get("habilidade_largada")?,
            adaptabilidade: row.get("adaptabilidade")?,
            fator_chuva: row.get("fator_chuva")?,
            fitness: row.get("fitness")?,
            experiencia: row.get("experiencia")?,
            desenvolvimento: row.get("desenvolvimento")?,
            aggression: row.get("aggression")?,
            smoothness: row.get("smoothness")?,
            midia: row.get("midia")?,
            mentalidade: row.get("mentalidade")?,
            confianca: row.get("confianca")?,
        },
        stats_temporada: DriverSeasonStats {
            pontos: row.get("temp_pontos")?,
            vitorias: parse_non_negative_u32(row, "temp_vitorias")?,
            podios: parse_non_negative_u32(row, "temp_podios")?,
            poles: parse_non_negative_u32(row, "temp_poles")?,
            corridas: parse_non_negative_u32(row, "temp_corridas")?,
            dnfs: parse_non_negative_u32(row, "temp_dnfs")?,
            posicao_media: row.get("temp_posicao_media")?,
        },
        stats_carreira: DriverCareerStats {
            pontos_total: row.get("carreira_pontos_total")?,
            vitorias: parse_non_negative_u32(row, "carreira_vitorias")?,
            podios: parse_non_negative_u32(row, "carreira_podios")?,
            poles: parse_non_negative_u32(row, "carreira_poles")?,
            corridas: parse_non_negative_u32(row, "carreira_corridas")?,
            temporadas: parse_non_negative_u32(row, "carreira_temporadas")?,
            titulos: parse_non_negative_u32(row, "carreira_titulos")?,
            dnfs: parse_non_negative_u32(row, "carreira_dnfs")?,
        },
        motivacao: row.get("motivacao")?,
        historico_circuitos: parse_json_object_field(&historico_str, "historico_circuitos")?,
        ultimos_resultados: parse_json_array_field(&ultimos_str, "ultimos_resultados")?,
        melhor_resultado_temp: parse_optional_non_negative_u32(row, "melhor_resultado_temp")?,
        temporadas_na_categoria: parse_non_negative_u32(row, "temporadas_na_categoria")?,
        corridas_na_categoria: parse_non_negative_u32(row, "corridas_na_categoria")?,
        temporadas_motivacao_baixa: parse_non_negative_u32(row, "temporadas_motivacao_baixa")?,
    })
}

fn serialize_json_field(value: &serde_json::Value, field: &str) -> Result<String, DbError> {
    serde_json::to_string(value)
        .map_err(|e| DbError::InvalidData(format!("Falha ao serializar '{field}': {e}")))
}

fn map_driver_query_error(error: rusqlite::Error) -> DbError {
    match error {
        rusqlite::Error::InvalidParameterName(message) => DbError::InvalidData(message),
        other => DbError::Sqlite(other),
    }
}

fn invalid_driver_data_error(message: impl Into<String>) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.into())
}

fn parse_non_negative_u32(row: &rusqlite::Row<'_>, field: &str) -> rusqlite::Result<u32> {
    let value: i64 = row.get(field)?;
    u32::try_from(value).map_err(|_| {
        invalid_driver_data_error(format!(
            "Campo '{field}' invalido: esperado inteiro nao negativo, recebido {value}"
        ))
    })
}

fn parse_optional_non_negative_u32(
    row: &rusqlite::Row<'_>,
    field: &str,
) -> rusqlite::Result<Option<u32>> {
    row.get::<_, Option<i64>>(field)?
        .map(|value| {
            u32::try_from(value).map_err(|_| {
                invalid_driver_data_error(format!(
                    "Campo '{field}' invalido: esperado inteiro nao negativo, recebido {value}"
                ))
            })
        })
        .transpose()
}

fn parse_json_object_field(raw: &str, field: &str) -> rusqlite::Result<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| invalid_driver_data_error(format!("JSON invalido em '{field}': {e}")))?;
    if !value.is_object() {
        return Err(invalid_driver_data_error(format!(
            "Campo '{field}' invalido: esperado objeto JSON"
        )));
    }
    Ok(value)
}

fn parse_json_array_field(raw: &str, field: &str) -> rusqlite::Result<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| invalid_driver_data_error(format!("JSON invalido em '{field}': {e}")))?;
    if !value.is_array() {
        return Err(invalid_driver_data_error(format!(
            "Campo '{field}' invalido: esperado array JSON"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    #[test]
    fn test_invalid_driver_status_from_db_returns_error() {
        let conn = setup_test_db().expect("test db");
        let driver = sample_driver("P001");
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "UPDATE drivers SET status = 'status_quebrado' WHERE id = ?1",
            rusqlite::params![&driver.id],
        )
        .expect("corrupt status");

        let err = get_driver(&conn, &driver.id).expect_err("invalid status should fail");
        assert!(err.to_string().contains("DriverStatus inv"));
    }

    #[test]
    fn test_invalid_primary_personality_from_db_returns_error() {
        let conn = setup_test_db().expect("test db");
        let driver = sample_driver("P001");
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "UPDATE drivers SET personalidade_primaria = 'perfil_quebrado' WHERE id = ?1",
            rusqlite::params![&driver.id],
        )
        .expect("corrupt primary personality");

        let err =
            get_driver(&conn, &driver.id).expect_err("invalid primary personality should fail");
        assert!(err.to_string().contains("PrimaryPersonality inv"));
    }

    #[test]
    fn test_invalid_secondary_personality_from_db_returns_error() {
        let conn = setup_test_db().expect("test db");
        let driver = sample_driver("P001");
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "UPDATE drivers SET personalidade_secundaria = 'perfil_quebrado' WHERE id = ?1",
            rusqlite::params![&driver.id],
        )
        .expect("corrupt secondary personality");

        let err =
            get_driver(&conn, &driver.id).expect_err("invalid secondary personality should fail");
        assert!(err.to_string().contains("SecondaryPersonality inv"));
    }

    #[test]
    fn test_invalid_historico_json_from_db_returns_error() {
        let conn = setup_test_db().expect("test db");
        let driver = sample_driver("P001");
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "UPDATE drivers SET historico_circuitos = '[]' WHERE id = ?1",
            rusqlite::params![&driver.id],
        )
        .expect("corrupt track history json shape");

        let err = get_driver(&conn, &driver.id).expect_err("invalid history json should fail");
        assert!(err.to_string().contains("historico_circuitos"));
    }

    #[test]
    fn test_invalid_recent_results_json_from_db_returns_error() {
        let conn = setup_test_db().expect("test db");
        let driver = sample_driver("P001");
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "UPDATE drivers SET ultimos_resultados = '{}' WHERE id = ?1",
            rusqlite::params![&driver.id],
        )
        .expect("corrupt recent results json shape");

        let err = get_driver(&conn, &driver.id).expect_err("invalid recent results should fail");
        assert!(err.to_string().contains("ultimos_resultados"));
    }

    #[test]
    fn test_negative_driver_counter_from_db_returns_error() {
        let conn = setup_test_db().expect("test db");
        let driver = sample_driver("P001");
        insert_driver(&conn, &driver).expect("insert driver");
        conn.execute(
            "UPDATE drivers SET temp_corridas = -1 WHERE id = ?1",
            rusqlite::params![&driver.id],
        )
        .expect("corrupt season counter");

        let err = get_driver(&conn, &driver.id).expect_err("negative counter should fail");
        assert!(err.to_string().contains("temp_corridas"));
    }

    #[test]
    fn test_get_player_driver_rejects_multiple_players() {
        let conn = setup_test_db().expect("test db");
        let mut player_a = sample_driver("P001");
        player_a.is_jogador = true;
        insert_driver(&conn, &player_a).expect("insert player a");

        let mut player_b = sample_driver("P002");
        player_b.is_jogador = true;
        insert_driver(&conn, &player_b).expect("insert player b");

        let err = get_player_driver(&conn).expect_err("duplicate players should fail");
        assert!(err.to_string().contains("exatamente 1 piloto do jogador"));
    }

    fn sample_driver(id: &str) -> Driver {
        let mut driver = Driver::new(
            id.to_string(),
            "Piloto Teste".to_string(),
            "br".to_string(),
            "M".to_string(),
            20,
            2024,
        );
        driver.categoria_atual = Some("gt4".to_string());
        driver
    }

    fn setup_test_db() -> Result<Connection, DbError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE drivers (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                is_jogador INTEGER NOT NULL DEFAULT 0,
                idade INTEGER NOT NULL,
                nacionalidade TEXT NOT NULL,
                genero TEXT NOT NULL,
                categoria_atual TEXT,
                categoria_especial_ativa TEXT,
                status TEXT NOT NULL DEFAULT 'Ativo',
                personalidade_primaria TEXT,
                personalidade_secundaria TEXT,
                ano_inicio_carreira INTEGER NOT NULL,
                skill REAL NOT NULL DEFAULT 50.0,
                consistencia REAL NOT NULL DEFAULT 50.0,
                racecraft REAL NOT NULL DEFAULT 50.0,
                defesa REAL NOT NULL DEFAULT 50.0,
                ritmo_classificacao REAL NOT NULL DEFAULT 50.0,
                gestao_pneus REAL NOT NULL DEFAULT 50.0,
                habilidade_largada REAL NOT NULL DEFAULT 50.0,
                adaptabilidade REAL NOT NULL DEFAULT 50.0,
                fator_chuva REAL NOT NULL DEFAULT 50.0,
                fitness REAL NOT NULL DEFAULT 50.0,
                experiencia REAL NOT NULL DEFAULT 50.0,
                desenvolvimento REAL NOT NULL DEFAULT 50.0,
                aggression REAL NOT NULL DEFAULT 50.0,
                smoothness REAL NOT NULL DEFAULT 50.0,
                midia REAL NOT NULL DEFAULT 50.0,
                mentalidade REAL NOT NULL DEFAULT 50.0,
                confianca REAL NOT NULL DEFAULT 50.0,
                temp_pontos REAL NOT NULL DEFAULT 0.0,
                temp_vitorias INTEGER NOT NULL DEFAULT 0,
                temp_podios INTEGER NOT NULL DEFAULT 0,
                temp_poles INTEGER NOT NULL DEFAULT 0,
                temp_corridas INTEGER NOT NULL DEFAULT 0,
                temp_dnfs INTEGER NOT NULL DEFAULT 0,
                temp_posicao_media REAL NOT NULL DEFAULT 0.0,
                carreira_pontos_total REAL NOT NULL DEFAULT 0.0,
                carreira_vitorias INTEGER NOT NULL DEFAULT 0,
                carreira_podios INTEGER NOT NULL DEFAULT 0,
                carreira_poles INTEGER NOT NULL DEFAULT 0,
                carreira_corridas INTEGER NOT NULL DEFAULT 0,
                carreira_temporadas INTEGER NOT NULL DEFAULT 0,
                carreira_titulos INTEGER NOT NULL DEFAULT 0,
                carreira_dnfs INTEGER NOT NULL DEFAULT 0,
                motivacao REAL NOT NULL DEFAULT 70.0,
                historico_circuitos TEXT NOT NULL DEFAULT '{}',
                ultimos_resultados TEXT NOT NULL DEFAULT '[]',
                melhor_resultado_temp INTEGER,
                temporadas_na_categoria INTEGER NOT NULL DEFAULT 0,
                corridas_na_categoria INTEGER NOT NULL DEFAULT 0,
                temporadas_motivacao_baixa INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        Ok(conn)
    }
}
