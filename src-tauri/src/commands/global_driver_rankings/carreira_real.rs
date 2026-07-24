//! O que cada piloto correu de verdade em `race_results` — lastro de histórico e split de pódios.

use super::*;

/// O que cada piloto REALMENTE fez na pista, direto de `race_results` — a única
/// fonte que não carrega passado carimbado. `stats_carreira` não serve como
/// histórico: todo piloto gerado fora das categorias rookie nasce com um bloco de
/// corridas/temporadas inventado por `seed_initial_career_history` (sem uma única
/// corrida por trás), e o acumulado ainda arrasta a contagem dobrada de saves
/// antigos. Usado como lastro quando o piloto não tem archive de temporada.
///
/// `titles` fica 0 de propósito: título é evento de campeonato, vem do archive.
/// `driver_id = None` agrega a tabela inteira; `Some(id)` agrega só aquele piloto.
pub(super) fn real_career_by_driver_filtered(
    conn: &Connection,
    driver_id: Option<&str>,
) -> Result<HashMap<String, CategoryStats>, String> {
    if !table_exists(conn, "race_results")? {
        return Ok(HashMap::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT piloto_id,
                COUNT(*) AS corridas,
                COALESCE(SUM(pontos), 0) AS pontos,
                COALESCE(SUM(CASE WHEN posicao_final = 1 AND dnf = 0 THEN 1 ELSE 0 END), 0) AS vitorias,
                COALESCE(SUM(CASE WHEN posicao_final BETWEEN 1 AND 3 AND dnf = 0 THEN 1 ELSE 0 END), 0) AS podios,
                COALESCE(SUM(CASE WHEN posicao_largada = 1 THEN 1 ELSE 0 END), 0) AS poles,
                COALESCE(SUM(CASE WHEN dnf = 1 THEN 1 ELSE 0 END), 0) AS dnfs
             FROM race_results
             WHERE ?1 IS NULL OR piloto_id = ?1
             GROUP BY piloto_id",
        )
        .map_err(|e| format!("Falha ao preparar carreira real do piloto: {e}"))?;
    let rows = stmt
        .query_map(params![driver_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                CategoryStats {
                    category: String::new(),
                    class_name: None,
                    races: row.get::<_, i32>(1)?,
                    points: row.get::<_, f64>(2)?,
                    wins: row.get::<_, i32>(3)?,
                    podiums: row.get::<_, i32>(4)?,
                    poles: row.get::<_, i32>(5)?,
                    dnfs: row.get::<_, i32>(6)?,
                    titles: 0,
                    title_years: Vec::new(),
                },
            ))
        })
        .map_err(|e| format!("Falha ao consultar carreira real do piloto: {e}"))?;
    let mut by_driver = HashMap::new();
    for row in rows {
        let (id, stats) = row.map_err(|e| format!("Falha ao ler carreira real do piloto: {e}"))?;
        by_driver.insert(id, stats);
    }
    Ok(by_driver)
}

/// Índice do que cada piloto realmente correu. Quando o save tem resultado gravado,
/// ele é a única fonte de histórico aceita no lugar do archive de temporada. Num save
/// sem nenhum resultado (fixture de teste, base recém-criada) não há verdade de campo
/// pra consultar, e o agregado de carreira volta a ser o último recurso.
pub(super) struct RealCareerIndex {
    pub(super) by_driver: HashMap<String, CategoryStats>,
    pub(super) has_results: bool,
}

impl RealCareerIndex {
    pub(super) fn load(conn: &Connection) -> Result<Self, String> {
        let by_driver = real_career_by_driver_filtered(conn, None)?;
        Ok(Self {
            has_results: !by_driver.is_empty(),
            by_driver,
        })
    }

    pub(super) fn for_driver(conn: &Connection, driver_id: &str) -> Result<Self, String> {
        let by_driver = real_career_by_driver_filtered(conn, Some(driver_id))?;
        Ok(Self {
            has_results: !by_driver.is_empty(),
            by_driver,
        })
    }

    /// Histórico de lastro do piloto, herdando o rótulo de categoria de `from_career`.
    /// Com resultado gravado no save, quem nunca largou fica zerado — é assim que o
    /// bloco carimbado deixa de contar como carreira.
    pub(super) fn history_for(&self, driver_id: &str, from_career: CategoryStats) -> CategoryStats {
        if !self.has_results {
            return from_career;
        }
        let mut stats = self.by_driver.get(driver_id).cloned().unwrap_or_default();
        stats.category = from_career.category;
        stats.class_name = from_career.class_name;
        stats
    }
}

/// Conta, por piloto e pela carreira inteira, quantas vezes terminou em 2º e em 3º
/// — o detalhe que quebra "pódios que não foram vitória". Lê os resultados reais
/// (`race_results`, nunca podados entre temporadas), então cobre tudo o que foi
/// corrido no jogo. Pilotos históricos pré-gerados não têm linhas aqui e simplesmente
/// não aparecem no mapa (o chamador os deixa em 0). Uma varredura indexada, barata.
pub(super) fn career_podium_splits(conn: &Connection) -> Result<HashMap<String, (i32, i32)>, String> {
    if !table_exists(conn, "race_results")? {
        return Ok(HashMap::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT piloto_id,
                COALESCE(SUM(CASE WHEN posicao_final = 2 THEN 1 ELSE 0 END), 0) AS segundos,
                COALESCE(SUM(CASE WHEN posicao_final = 3 THEN 1 ELSE 0 END), 0) AS terceiros
             FROM race_results
             GROUP BY piloto_id",
        )
        .map_err(|e| format!("Falha ao preparar split de podios: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (row.get::<_, i32>(1)?, row.get::<_, i32>(2)?),
            ))
        })
        .map_err(|e| format!("Falha ao consultar split de podios: {e}"))?;
    let mut splits = HashMap::new();
    for row in rows {
        let (id, split) = row.map_err(|e| format!("Falha ao ler split de podios: {e}"))?;
        splits.insert(id, split);
    }
    Ok(splits)
}
