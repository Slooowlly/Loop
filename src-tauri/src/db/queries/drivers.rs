#![allow(dead_code)]

use std::sync::OnceLock;

use rusqlite::{Connection, OptionalExtension};

use crate::db::connection::DbError;
use crate::models::driver::{Driver, DriverAttributes, DriverCareerStats, DriverSeasonStats};
use crate::models::enums::{DriverStatus, PrimaryPersonality, SecondaryPersonality};

/// As colunas de `drivers`, uma vez só.
///
/// Antes esta lista existia à mão em quatro lugares — a lista da `INSERT`, a lista de
/// binds dela, o `SET` da `UPDATE` e o `SELECT *` — e a paridade entre elas não era
/// verificada por nada além de rodar o jogo. Campo novo de piloto exigia acertar os
/// quatro, e errar um só produzia coluna gravada como `NULL` ou leitura em branco, sem
/// erro de compilação.
///
/// Agora a `INSERT`, a `UPDATE` e todo `SELECT` de piloto são GERADOS daqui, e o teste
/// `a_lista_de_colunas_bate_com_o_schema_real` compara a lista com o `PRAGMA table_info`
/// do schema das migrações nos dois sentidos. Sobra um único ponto manual: o
/// `named_params!` que dá valor a cada bind — e ali a divergência não é silenciosa, porque
/// nome que não existe na SQL é erro do rusqlite e coluna sem valor cai no `NOT NULL`.
///
/// A ORDEM não importa para o banco (tudo é por nome), mas é a mesma da tabela, o que
/// deixa a comparação com o `table_info` legível quando o teste acusa diferença.
pub(crate) const COLUNAS_DRIVER: &[&str] = &[
    "id",
    "nome",
    "is_jogador",
    "idade",
    "nacionalidade",
    "genero",
    "categoria_atual",
    "categoria_especial_ativa",
    "status",
    "personalidade_primaria",
    "personalidade_secundaria",
    "ano_inicio_carreira",
    "skill",
    "consistencia",
    "racecraft",
    "defesa",
    "ritmo_classificacao",
    "gestao_pneus",
    "habilidade_largada",
    "adaptabilidade",
    "fator_chuva",
    "fitness",
    "experiencia",
    "desenvolvimento",
    "aggression",
    "smoothness",
    "midia",
    "carisma",
    "mentalidade",
    "confianca",
    "potencial",
    "temp_pontos",
    "temp_vitorias",
    "temp_podios",
    "temp_poles",
    "temp_corridas",
    "temp_dnfs",
    "temp_posicao_media",
    "carreira_pontos_total",
    "carreira_vitorias",
    "carreira_podios",
    "carreira_poles",
    "carreira_corridas",
    "carreira_temporadas",
    "carreira_titulos",
    "carreira_dnfs",
    "motivacao",
    "forma",
    "historico_circuitos",
    "ultimos_resultados",
    "melhor_resultado_temp",
    "temporadas_na_categoria",
    "corridas_na_categoria",
    "temporadas_motivacao_baixa",
];

/// `id, nome, is_jogador, ...` — a projeção que substitui o `SELECT *`.
///
/// Com `*`, remover ou renomear uma coluna só estourava em runtime, no save de alguém,
/// dentro do `row.get("nome_da_coluna")` do `driver_from_row`. Nomeando as colunas, a
/// mesma quebra aparece na primeira consulta preparada, e o teste de schema a pega antes.
fn colunas_select() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| COLUNAS_DRIVER.join(", "))
}

/// A mesma projeção qualificada por um alias de tabela (`d.id, d.nome, ...`), para as
/// consultas que fazem junção e antes usavam `SELECT d.*`.
fn colunas_select_com_alias(alias: &str) -> String {
    COLUNAS_DRIVER
        .iter()
        .map(|coluna| format!("{alias}.{coluna}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn sql_insert_driver() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        let binds = COLUNAS_DRIVER
            .iter()
            .map(|coluna| format!(":{coluna}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "INSERT INTO drivers ({}) VALUES ({binds})",
            COLUNAS_DRIVER.join(", ")
        )
    })
}

/// `UPDATE` de piloto inteiro. O `id` sai do `SET` (é a chave) e entra só no `WHERE`.
fn sql_update_driver() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| {
        let sets = COLUNAS_DRIVER
            .iter()
            .filter(|coluna| **coluna != "id")
            .map(|coluna| format!("{coluna} = :{coluna}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("UPDATE drivers SET {sets} WHERE id = :id")
    })
}

pub fn insert_driver(conn: &Connection, driver: &Driver) -> Result<(), DbError> {
    let historico = serialize_json_field(&driver.historico_circuitos, "historico_circuitos")?;
    let ultimos = serialize_json_field(&driver.ultimos_resultados, "ultimos_resultados")?;

    conn.execute(
        sql_insert_driver(),
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
            ":carisma": driver.atributos.carisma,
            ":mentalidade": driver.atributos.mentalidade,
            ":confianca": driver.atributos.confianca,
            ":potencial": driver.atributos.potencial,
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
            ":forma": driver.forma,
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
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM drivers WHERE id = ?1",
        colunas_select()
    ))?;
    stmt.query_row(rusqlite::params![id], driver_from_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("Piloto '{}' nao encontrado", id))
            }
            other => map_driver_query_error(other),
        })
}

pub fn get_driver_by_name(conn: &Connection, nome: &str) -> Result<Driver, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM drivers WHERE nome = ?1 LIMIT 1",
        colunas_select()
    ))?;
    stmt.query_row(rusqlite::params![nome], driver_from_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DbError::NotFound(format!("Piloto '{}' nao encontrado", nome))
            }
            other => map_driver_query_error(other),
        })
}

pub fn get_all_drivers(conn: &Connection) -> Result<Vec<Driver>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM drivers ORDER BY nome",
        colunas_select()
    ))?;
    let rows = stmt.query_map([], driver_from_row)?;
    collect_drivers(rows)
}

pub fn get_drivers_by_category(conn: &Connection, categoria: &str) -> Result<Vec<Driver>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM drivers WHERE categoria_atual = ?1 ORDER BY nome",
        colunas_select()
    ))?;
    let rows = stmt.query_map(rusqlite::params![categoria], driver_from_row)?;
    collect_drivers(rows)
}

pub fn get_drivers_by_active_category(
    conn: &Connection,
    categoria: &str,
) -> Result<Vec<Driver>, DbError> {
    let filtro = if matches!(categoria, "production_challenger" | "endurance") {
        "categoria_especial_ativa = ?1"
    } else {
        "categoria_atual = ?1 AND categoria_especial_ativa IS NULL"
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM drivers WHERE {filtro} ORDER BY nome",
        colunas_select()
    ))?;
    let rows = stmt.query_map(rusqlite::params![categoria], driver_from_row)?;
    collect_drivers(rows)
}

pub fn get_drivers_by_status(conn: &Connection, status: &str) -> Result<Vec<Driver>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM drivers WHERE status = ?1 ORDER BY nome",
        colunas_select()
    ))?;
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
            let mut stmt = conn.prepare(&format!(
                "SELECT {} FROM drivers WHERE is_jogador = 1",
                colunas_select()
            ))?;
            stmt.query_row([], driver_from_row)
                .map_err(map_driver_query_error)
        }
        count => Err(DbError::InvalidData(format!(
            "Esperado exatamente 1 piloto do jogador, encontrado {count}"
        ))),
    }
}

pub fn get_free_drivers(conn: &Connection) -> Result<Vec<Driver>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM drivers WHERE categoria_atual IS NULL AND status = 'Ativo' ORDER BY nome",
        colunas_select()
    ))?;
    let rows = stmt.query_map([], driver_from_row)?;
    collect_drivers(rows)
}

pub fn get_drivers_without_active_contract(conn: &Connection) -> Result<Vec<Driver>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM drivers d
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
        colunas_select_com_alias("d")
    ))?;
    let rows = stmt.query_map([], driver_from_row)?;
    collect_drivers(rows)
}

pub fn update_driver(conn: &Connection, driver: &Driver) -> Result<(), DbError> {
    let historico = serialize_json_field(&driver.historico_circuitos, "historico_circuitos")?;
    let ultimos = serialize_json_field(&driver.ultimos_resultados, "ultimos_resultados")?;

    conn.execute(
        sql_update_driver(),
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
            ":carisma": driver.atributos.carisma,
            ":mentalidade": driver.atributos.mentalidade, ":confianca": driver.atributos.confianca,
            ":potencial": driver.atributos.potencial,
            ":temp_pontos": driver.stats_temporada.pontos, ":temp_vitorias": driver.stats_temporada.vitorias as i64,
            ":temp_podios": driver.stats_temporada.podios as i64, ":temp_poles": driver.stats_temporada.poles as i64,
            ":temp_corridas": driver.stats_temporada.corridas as i64, ":temp_dnfs": driver.stats_temporada.dnfs as i64,
            ":temp_posicao_media": driver.stats_temporada.posicao_media,
            ":carreira_pontos_total": driver.stats_carreira.pontos_total, ":carreira_vitorias": driver.stats_carreira.vitorias as i64,
            ":carreira_podios": driver.stats_carreira.podios as i64, ":carreira_poles": driver.stats_carreira.poles as i64,
            ":carreira_corridas": driver.stats_carreira.corridas as i64, ":carreira_temporadas": driver.stats_carreira.temporadas as i64,
            ":carreira_titulos": driver.stats_carreira.titulos as i64, ":carreira_dnfs": driver.stats_carreira.dnfs as i64,
            ":motivacao": driver.motivacao, ":forma": driver.forma,
            ":historico_circuitos": &historico, ":ultimos_resultados": &ultimos,
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
            midia = :midia, carisma = :carisma, mentalidade = :mentalidade, confianca = :confianca, potencial = :potencial
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
            ":carisma": attrs.carisma,
            ":mentalidade": attrs.mentalidade,
            ":confianca": attrs.confianca,
            ":potencial": attrs.potencial,
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
    // Legacy cleanup for temporary special-call marks. Production/Endurance are
    // real divisions now, so PosEspecial must not clear their regular driver state.
    let n = conn.execute(
        "UPDATE drivers
         SET categoria_especial_ativa = NULL
         WHERE categoria_especial_ativa IS NOT NULL
           AND categoria_especial_ativa NOT IN ('production_challenger', 'endurance')",
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

/// Grava o estado da FORMA do momento (AR(1) de `simulation::forma`, adimensional
/// e normalizado em σ = 1, cortado no teto de sigmas do próprio módulo). Escrito
/// uma vez por fim de semana disputado, no setup da corrida.
pub fn update_driver_forma(conn: &Connection, id: &str, forma: f64) -> Result<(), DbError> {
    let teto = crate::simulation::forma::TETO_SIGMAS;
    conn.execute(
        "UPDATE drivers SET forma = ?1 WHERE id = ?2",
        rusqlite::params![forma.clamp(-teto, teto), id],
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

/// Decaimento passivo da fama de um piloto rumo ao piso, escalado pelo carisma
/// (carismático decai mais devagar). Espelha `fame::decay_fame_toward`: o fator
/// `(1.5 − carisma/100)` é o `fame_decay_mult`. Não mexe em quem já está no piso.
pub fn decay_driver_fame(
    conn: &Connection,
    id: &str,
    floor: f64,
    base_rate: f64,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE drivers
         SET midia = midia - (midia - ?1) * MIN(1.0, MAX(0.0, ?2 * (1.5 - carisma / 100.0)))
         WHERE id = ?3 AND midia > ?1",
        rusqlite::params![floor, base_rate, id],
    )?;
    Ok(())
}

/// Pedigree de carreira que sustenta o PISO PESSOAL da fama
/// ([`crate::fame::personal_fame_floor`]). Só os três números que entram na conta —
/// leitura barata, feita uma vez por corrida para o grid inteiro.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FamePedigree {
    pub titulos: u32,
    pub vitorias: u32,
    /// Temporadas disputadas em categoria de tier ≥ [`crate::fame::FAME_ELITE_TIER_MIN`].
    pub temporadas_elite: u32,
}

/// Pedigree de fama de vários pilotos de uma vez. Duas leituras: os totais de carreira
/// em `drivers` e a contagem de temporadas na elite em `standings` (uma linha por
/// piloto/temporada/categoria — o tier vem do catálogo, não do SQL).
///
/// Quem não existir no banco simplesmente não aparece no mapa; o call site cai no piso
/// base. Lista vazia devolve mapa vazio sem tocar no banco.
pub fn get_fame_pedigrees(
    conn: &Connection,
    ids: &[&str],
) -> Result<std::collections::HashMap<String, FamePedigree>, DbError> {
    use std::collections::HashMap;

    let mut unicos: Vec<&str> = ids.iter().copied().filter(|id| !id.is_empty()).collect();
    unicos.sort_unstable();
    unicos.dedup();
    if unicos.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = vec!["?"; unicos.len()].join(",");
    let params: Vec<&dyn rusqlite::ToSql> =
        unicos.iter().map(|id| id as &dyn rusqlite::ToSql).collect();

    let mut mapa: HashMap<String, FamePedigree> = HashMap::new();
    let sql = format!(
        "SELECT id, carreira_titulos, carreira_vitorias FROM drivers WHERE id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params.as_slice(), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (id, titulos, vitorias) = row?;
        mapa.insert(
            id,
            FamePedigree {
                titulos: titulos.max(0) as u32,
                vitorias: vitorias.max(0) as u32,
                temporadas_elite: 0,
            },
        );
    }

    // Temporadas na elite: pares (piloto, temporada) distintos em categoria de tier alto.
    // A tabela pode não existir em bancos de teste enxutos — nesse caso o termo é 0.
    let sql = format!(
        "SELECT piloto_id, categoria, COUNT(DISTINCT temporada_id)
         FROM standings WHERE piloto_id IN ({placeholders}) GROUP BY piloto_id, categoria"
    );
    if let Ok(mut stmt) = conn.prepare(&sql) {
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        });
        if let Ok(rows) = rows {
            for row in rows.flatten() {
                let (piloto_id, categoria, temporadas) = row;
                let tier = crate::constants::categories::get_category_config(&categoria)
                    .map(|c| c.tier)
                    .unwrap_or(0);
                if tier < crate::fame::FAME_ELITE_TIER_MIN {
                    continue;
                }
                mapa.entry(piloto_id).or_default().temporadas_elite += temporadas.max(0) as u32;
            }
        }
    }

    Ok(mapa)
}

/// Ajusta o carisma de um piloto por um delta (deriva de carreira), preso a 0–100.
pub fn bump_driver_carisma(conn: &Connection, id: &str, delta: f64) -> Result<(), DbError> {
    conn.execute(
        "UPDATE drivers SET carisma = MAX(0.0, MIN(100.0, carisma + ?1)) WHERE id = ?2",
        rusqlite::params![delta, id],
    )?;
    Ok(())
}

/// Carisma (0–100) de um piloto. `None` se o id não existir. Leve — só a coluna.
pub fn get_driver_carisma(conn: &Connection, id: &str) -> Result<Option<f64>, DbError> {
    let v = conn
        .query_row(
            "SELECT carisma FROM drivers WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get::<_, f64>(0),
        )
        .optional()?;
    Ok(v)
}

/// Soma um delta de fama, passando pelo retorno decrescente do topo
/// ([`crate::fame::apply_fame_gain`]).
///
/// Era um `UPDATE ... midia + ?1` puro em SQL. Deixou de ser possível quando o ganho
/// passou a depender da fama ATUAL: a alternativa seria reescrever a curva em SQL, que é
/// a mesma classe de defeito que o resto desta rodada removeu — duas cópias da mesma
/// regra em linguagens diferentes, discordando na primeira vez que uma delas mudar.
pub fn update_driver_midia_delta(conn: &Connection, id: &str, delta: f64) -> Result<(), DbError> {
    // Piloto inexistente é NO-OP, como era no `UPDATE ... WHERE id = ?` puro. Transformar
    // isso em erro mudaria o contrato de uma função chamada de dentro do laço de fama.
    let Some(atual) = conn
        .query_row(
            "SELECT midia FROM drivers WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get::<_, f64>(0),
        )
        .optional()?
    else {
        return Ok(());
    };
    conn.execute(
        "UPDATE drivers SET midia = ?1 WHERE id = ?2",
        rusqlite::params![crate::fame::apply_fame_gain(atual, delta), id],
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
            carisma: row.get("carisma")?,
            mentalidade: row.get("mentalidade")?,
            confianca: row.get("confianca")?,
            potencial: row.get("potencial")?,
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
        forma: row.get("forma")?,
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

    /// Prova a afirmação que vivia como `TODO(migration)` em `models/driver.rs`: todo campo
    /// de `DriverAttributes` tem coluna no schema e sobrevive à ida e volta pelo banco.
    ///
    /// Roda contra o schema REAL (`migrations::run_all`), não contra o DDL de bancada do
    /// `setup_test_db` — o DDL de bancada é uma cópia à mão e pode divergir da baseline sem
    /// que ninguém perceba, que é justamente o buraco que este teste fecha.
    ///
    /// Cada atributo recebe um valor DISTINTO. Com todos iguais, uma troca de duas colunas
    /// na `INSERT` ou na `driver_from_row` passaria despercebida.
    #[test]
    fn todo_atributo_do_piloto_tem_coluna_e_volta_igual_do_banco() {
        let conn = Connection::open_in_memory().expect("banco em memória");
        crate::db::migrations::run_all(&conn).expect("schema real");

        let mut driver = sample_driver("P_ATRIBUTOS");
        let esperado = DriverAttributes {
            skill: 1.5,
            consistencia: 2.5,
            racecraft: 3.5,
            defesa: 4.5,
            ritmo_classificacao: 5.5,
            gestao_pneus: 6.5,
            habilidade_largada: 7.5,
            adaptabilidade: 8.5,
            fator_chuva: 9.5,
            fitness: 10.5,
            experiencia: 11.5,
            desenvolvimento: 12.5,
            aggression: 13.5,
            smoothness: 14.5,
            midia: 15.5,
            carisma: 16.5,
            mentalidade: 17.5,
            confianca: 18.5,
            potencial: 19.5,
        };
        driver.atributos = esperado.clone();

        insert_driver(&conn, &driver).expect("gravar piloto");
        let lido = get_driver(&conn, &driver.id).expect("ler piloto");

        assert_eq!(lido.atributos.skill, esperado.skill, "skill");
        assert_eq!(
            lido.atributos.consistencia, esperado.consistencia,
            "consistencia"
        );
        assert_eq!(lido.atributos.racecraft, esperado.racecraft, "racecraft");
        assert_eq!(lido.atributos.defesa, esperado.defesa, "defesa");
        assert_eq!(
            lido.atributos.ritmo_classificacao, esperado.ritmo_classificacao,
            "ritmo_classificacao"
        );
        assert_eq!(
            lido.atributos.gestao_pneus, esperado.gestao_pneus,
            "gestao_pneus"
        );
        assert_eq!(
            lido.atributos.habilidade_largada, esperado.habilidade_largada,
            "habilidade_largada"
        );
        assert_eq!(
            lido.atributos.adaptabilidade, esperado.adaptabilidade,
            "adaptabilidade"
        );
        assert_eq!(
            lido.atributos.fator_chuva, esperado.fator_chuva,
            "fator_chuva"
        );
        assert_eq!(lido.atributos.fitness, esperado.fitness, "fitness");
        assert_eq!(
            lido.atributos.experiencia, esperado.experiencia,
            "experiencia"
        );
        assert_eq!(
            lido.atributos.desenvolvimento, esperado.desenvolvimento,
            "desenvolvimento"
        );
        assert_eq!(lido.atributos.aggression, esperado.aggression, "aggression");
        assert_eq!(lido.atributos.smoothness, esperado.smoothness, "smoothness");
        assert_eq!(lido.atributos.midia, esperado.midia, "midia");
        assert_eq!(lido.atributos.carisma, esperado.carisma, "carisma");
        assert_eq!(
            lido.atributos.mentalidade, esperado.mentalidade,
            "mentalidade"
        );
        assert_eq!(lido.atributos.confianca, esperado.confianca, "confianca");
        assert_eq!(lido.atributos.potencial, esperado.potencial, "potencial");
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

    /// Banco de teste com o schema REAL das migrações.
    ///
    /// Aqui existia uma quinta cópia à mão das 54 colunas — um `CREATE TABLE drivers`
    /// de bancada. Ela podia divergir da baseline sem que nada acusasse, e um teste que
    /// roda contra um schema que não é o do jogo prova menos do que aparenta.
    fn setup_test_db() -> Result<Connection, DbError> {
        let conn = Connection::open_in_memory()?;
        crate::db::migrations::run_all(&conn)?;
        Ok(conn)
    }

    /// O guard que fecha o A6.4: a lista central e a tabela real não podem divergir.
    ///
    /// Compara nos DOIS sentidos, porque as duas falhas são diferentes e as duas doem:
    /// coluna no schema e fora da lista nunca é lida nem gravada (campo que some em
    /// silêncio); coluna na lista e fora do schema derruba toda consulta de piloto no
    /// primeiro `prepare`.
    #[test]
    fn a_lista_de_colunas_bate_com_o_schema_real() {
        let conn = setup_test_db().expect("schema real");

        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_info('drivers')")
            .expect("table_info");
        let no_schema: std::collections::BTreeSet<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("nomes");

        let na_lista: std::collections::BTreeSet<String> =
            COLUNAS_DRIVER.iter().map(|c| c.to_string()).collect();

        let so_no_schema: Vec<&String> = no_schema.difference(&na_lista).collect();
        let so_na_lista: Vec<&String> = na_lista.difference(&no_schema).collect();
        assert!(
            so_no_schema.is_empty(),
            "colunas no schema e fora de COLUNAS_DRIVER (não são gravadas nem lidas): {so_no_schema:?}"
        );
        assert!(
            so_na_lista.is_empty(),
            "colunas em COLUNAS_DRIVER e fora do schema (derrubam todo SELECT de piloto): {so_na_lista:?}"
        );
        assert_eq!(
            COLUNAS_DRIVER.len(),
            na_lista.len(),
            "COLUNAS_DRIVER tem nome repetido"
        );
    }

    /// A ida e volta do piloto INTEIRO, campo a campo, com valor distinto em cada um.
    ///
    /// O teste dos atributos ao lado cobre os 19 do bloco `DriverAttributes`. Este cobre o
    /// resto — identidade, estado de carreira, estatísticas de temporada e de carreira —
    /// que é justamente onde a `INSERT` gerada, o `named_params!` à mão e o
    /// `driver_from_row` precisam concordar. Valores distintos porque, com todos iguais,
    /// uma troca de duas colunas passaria batida.
    #[test]
    fn o_piloto_inteiro_sobrevive_a_ida_e_volta_pelo_banco() {
        let conn = setup_test_db().expect("schema real");

        let mut driver = sample_driver("P_INTEIRO");
        driver.nome = "Piloto Completo".to_string();
        driver.is_jogador = true;
        driver.idade = 31;
        driver.nacionalidade = "pt".to_string();
        driver.genero = "F".to_string();
        driver.categoria_atual = Some("gt3".to_string());
        driver.categoria_especial_ativa = Some("endurance".to_string());
        driver.status = DriverStatus::Ativo;
        driver.personalidade_primaria = Some(PrimaryPersonality::Mercenario);
        driver.personalidade_secundaria = Some(SecondaryPersonality::CabecaQuente);
        driver.ano_inicio_carreira = 2019;
        driver.stats_temporada = DriverSeasonStats {
            pontos: 101.0,
            vitorias: 2,
            podios: 3,
            poles: 4,
            corridas: 5,
            dnfs: 6,
            posicao_media: 7.5,
        };
        driver.stats_carreira = DriverCareerStats {
            pontos_total: 202.0,
            vitorias: 8,
            podios: 9,
            poles: 10,
            corridas: 11,
            temporadas: 12,
            titulos: 13,
            dnfs: 14,
        };
        driver.motivacao = 63.5;
        driver.forma = -1.25;
        driver.historico_circuitos = serde_json::json!({ "interlagos": 3 });
        driver.ultimos_resultados = serde_json::json!([1, 4, 9]);
        driver.melhor_resultado_temp = Some(2);
        driver.temporadas_na_categoria = 15;
        driver.corridas_na_categoria = 16;
        driver.temporadas_motivacao_baixa = 17;

        insert_driver(&conn, &driver).expect("gravar");
        let lido = get_driver(&conn, &driver.id).expect("ler");

        assert_eq!(lido.nome, driver.nome);
        assert_eq!(lido.is_jogador, driver.is_jogador);
        assert_eq!(lido.idade, driver.idade);
        assert_eq!(lido.nacionalidade, driver.nacionalidade);
        assert_eq!(lido.genero, driver.genero);
        assert_eq!(lido.categoria_atual, driver.categoria_atual);
        assert_eq!(
            lido.categoria_especial_ativa,
            driver.categoria_especial_ativa
        );
        assert_eq!(lido.status.as_str(), driver.status.as_str());
        assert_eq!(
            lido.personalidade_primaria.as_ref().map(|p| p.as_str()),
            driver.personalidade_primaria.as_ref().map(|p| p.as_str())
        );
        assert_eq!(
            lido.personalidade_secundaria.as_ref().map(|p| p.as_str()),
            driver.personalidade_secundaria.as_ref().map(|p| p.as_str())
        );
        assert_eq!(lido.ano_inicio_carreira, driver.ano_inicio_carreira);
        assert_eq!(lido.stats_temporada.pontos, 101.0);
        assert_eq!(lido.stats_temporada.vitorias, 2);
        assert_eq!(lido.stats_temporada.podios, 3);
        assert_eq!(lido.stats_temporada.poles, 4);
        assert_eq!(lido.stats_temporada.corridas, 5);
        assert_eq!(lido.stats_temporada.dnfs, 6);
        assert_eq!(lido.stats_temporada.posicao_media, 7.5);
        assert_eq!(lido.stats_carreira.pontos_total, 202.0);
        assert_eq!(lido.stats_carreira.vitorias, 8);
        assert_eq!(lido.stats_carreira.podios, 9);
        assert_eq!(lido.stats_carreira.poles, 10);
        assert_eq!(lido.stats_carreira.corridas, 11);
        assert_eq!(lido.stats_carreira.temporadas, 12);
        assert_eq!(lido.stats_carreira.titulos, 13);
        assert_eq!(lido.stats_carreira.dnfs, 14);
        assert_eq!(lido.motivacao, 63.5);
        assert_eq!(lido.forma, -1.25);
        assert_eq!(lido.historico_circuitos, driver.historico_circuitos);
        assert_eq!(lido.ultimos_resultados, driver.ultimos_resultados);
        assert_eq!(lido.melhor_resultado_temp, Some(2));
        assert_eq!(lido.temporadas_na_categoria, 15);
        assert_eq!(lido.corridas_na_categoria, 16);
        assert_eq!(lido.temporadas_motivacao_baixa, 17);
    }

    /// A `UPDATE` gerada escreve TODAS as colunas mutáveis, não só as que alguém lembrou.
    #[test]
    fn o_update_de_piloto_inteiro_reescreve_todos_os_campos() {
        let conn = setup_test_db().expect("schema real");
        let driver = sample_driver("P_UPDATE");
        insert_driver(&conn, &driver).expect("gravar");

        let mut alterado = driver.clone();
        alterado.nome = "Outro Nome".to_string();
        alterado.idade = 44;
        alterado.motivacao = 12.5;
        alterado.forma = 0.75;
        alterado.atributos.skill = 88.5;
        alterado.stats_carreira.titulos = 3;
        alterado.temporadas_motivacao_baixa = 9;
        update_driver(&conn, &alterado).expect("atualizar");

        let lido = get_driver(&conn, &driver.id).expect("ler");
        assert_eq!(lido.nome, "Outro Nome");
        assert_eq!(lido.idade, 44);
        assert_eq!(lido.motivacao, 12.5);
        assert_eq!(lido.forma, 0.75);
        assert_eq!(lido.atributos.skill, 88.5);
        assert_eq!(lido.stats_carreira.titulos, 3);
        assert_eq!(lido.temporadas_motivacao_baixa, 9);
    }
}
