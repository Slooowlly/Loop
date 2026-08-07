//! Leitura do archive de temporadas: histórico por categoria, fama arquivada e ano de estreia.

use super::*;

/// Uma linha de `driver_season_archive` com o `snapshot_json` JÁ desserializado.
pub(super) struct LinhaDoArquivo {
    pub(super) categoria: String,
    pub(super) pontos: f64,
    pub(super) snapshot: Value,
    pub(super) posicao_campeonato: Option<i32>,
    pub(super) season_number: i32,
    pub(super) ano: i32,
}

/// O archive de temporadas lido UMA vez.
///
/// Três leituras diferentes precisam das mesmas linhas do mesmo piloto — o
/// histórico por categoria, o ano de estreia e a fama arquivada. Cada uma tinha
/// a sua consulta e desserializava o snapshot de novo: no ranking mundial, com
/// 600 pilotos e 9 MB de snapshot, era o mesmo JSON atravessado três vezes por
/// montagem. Aqui ele é lido e desserializado uma vez só, e as três leituras
/// passam a ser trabalho de memória.
///
/// `existe` distingue "tabela ausente" (save anterior ao archive) de "tabela
/// vazia": o fallback do chamador não é o mesmo nos dois casos.
pub(super) struct Arquivo {
    por_piloto: HashMap<String, Vec<LinhaDoArquivo>>,
    existe: bool,
}

impl Arquivo {
    /// O archive inteiro, para quem vai percorrer o mundo todo.
    pub(super) fn carregar_tudo(conn: &Connection) -> Result<Self, String> {
        Self::carregar(conn, None)
    }

    /// Só as linhas de um piloto, para quem quer o índice de um só (mercado).
    pub(super) fn carregar_piloto(conn: &Connection, driver_id: &str) -> Result<Self, String> {
        Self::carregar(conn, Some(driver_id))
    }

    fn carregar(conn: &Connection, driver_id: Option<&str>) -> Result<Self, String> {
        if !table_exists(conn, "driver_season_archive")? {
            return Ok(Self {
                por_piloto: HashMap::new(),
                existe: false,
            });
        }

        let base = "SELECT piloto_id, categoria, pontos, snapshot_json,
                           posicao_campeonato, season_number, ano
                    FROM driver_season_archive";
        let sql = match driver_id {
            Some(_) => format!("{base} WHERE piloto_id = ?1"),
            None => base.to_string(),
        };
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("Falha ao preparar historico global do piloto: {e}"))?;
        let ler = |row: &rusqlite::Row| {
            Ok((
                row.get::<_, String>(0)?,
                LinhaDoArquivo {
                    categoria: row.get::<_, String>(1)?,
                    pontos: row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                    snapshot: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                    posicao_campeonato: row.get::<_, Option<i32>>(4)?,
                    season_number: row.get::<_, i32>(5)?,
                    ano: row.get::<_, i32>(6)?,
                },
            ))
        };
        let rows = match driver_id {
            Some(driver_id) => stmt.query_map(params![driver_id], ler),
            None => stmt.query_map([], ler),
        }
        .map_err(|e| format!("Falha ao consultar historico global do piloto: {e}"))?;

        let mut por_piloto: HashMap<String, Vec<LinhaDoArquivo>> = HashMap::new();
        for row in rows {
            let (piloto_id, linha) =
                row.map_err(|e| format!("Falha ao ler historico global do piloto: {e}"))?;
            por_piloto.entry(piloto_id).or_default().push(linha);
        }

        Ok(Self {
            por_piloto,
            existe: true,
        })
    }

    pub(super) fn existe(&self) -> bool {
        self.existe
    }

    pub(super) fn linhas(&self, driver_id: &str) -> &[LinhaDoArquivo] {
        self.por_piloto
            .get(driver_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// Lê o histórico por categoria do archive (uma `CategoryStats` por temporada-
/// categoria) e os eventos de título já contados. Núcleo compartilhado entre o
/// caminho de ativo (`load_driver_category_stats`) e o de aposentado por id
/// (`load_archive_category_stats`).
pub(super) fn read_archive_category_stats(
    conn: &Connection,
    driver_id: &str,
    linhas: &[LinhaDoArquivo],
) -> Result<(Vec<CategoryStats>, HashSet<TitleEventKey>), String> {
    let mut stats = Vec::new();
    let mut counted_title_events = HashSet::<TitleEventKey>::new();
    for linha in linhas {
        let snapshot = &linha.snapshot;
        let category = normalized_archive_category(snapshot, linha.categoria.clone());
        let class_name =
            archived_title_class(conn, driver_id, &category, linha.season_number, snapshot)?;
        let points = json_f64(snapshot, "pontos").unwrap_or(linha.pontos);
        let wins = json_i32(snapshot, "vitorias");
        let podiums = json_i32(snapshot, "podios");
        let poles = json_i32(snapshot, "poles");
        let races = json_i32(snapshot, "corridas");
        let titles = valid_archived_title_count(
            json_i32_option(snapshot, "titulos"),
            linha.posicao_campeonato,
            points,
            wins,
            podiums,
            poles,
            races,
        );
        if titles > 0 {
            counted_title_events.insert(title_event_key(
                linha.season_number,
                &category,
                class_name.as_deref(),
            ));
        }
        let title_team_id =
            json_string(snapshot, "team_id").filter(|value| !value.trim().is_empty());
        stats.push(CategoryStats {
            category,
            class_name,
            points,
            wins,
            podiums,
            poles,
            races,
            titles,
            title_years: title_years_for_event(titles, linha.ano, title_team_id),
            dnfs: json_i32(snapshot, "dnfs"),
        });
    }

    Ok((stats, counted_title_events))
}

/// Fama (`atributos.midia`) registrada no snapshot MAIS RECENTE do archive de
/// temporadas — a base pra medir "quanto a fama subiu" nesta temporada. `None`
/// quando não há archive/tabela/snapshot com o campo (ex.: 1ª temporada).
pub(super) fn latest_archived_media(linhas: &[LinhaDoArquivo]) -> Option<f64> {
    linhas
        .iter()
        .max_by_key(|linha| (linha.season_number, linha.ano))?
        .snapshot
        .get("atributos")
        .and_then(|atributos| atributos.get("midia"))
        .and_then(Value::as_f64)
}

/// Histórico por categoria de um piloto (por id), incluindo títulos como campeão
/// de equipe. Vazio se não houver archive — o chamador decide o fallback.
pub(super) fn load_archive_category_stats(
    conn: &Connection,
    driver_id: &str,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
    arquivo: &Arquivo,
) -> Result<Vec<CategoryStats>, String> {
    if !arquivo.existe() {
        return Ok(Vec::new());
    }
    let (mut stats, counted_title_events) =
        read_archive_category_stats(conn, driver_id, arquivo.linhas(driver_id))?;
    let team_title_stats = team_champion_title_stats_for_driver(
        driver_id,
        &counted_title_events,
        team_title_stats_by_driver,
    );
    stats.extend(team_title_stats);
    Ok(stats)
}

pub(super) fn load_driver_category_stats(
    conn: &Connection,
    driver: &Driver,
    fallback_category: Option<&str>,
    team_title_stats_by_driver: &TeamTitleStatsByDriver,
    real_career: &RealCareerIndex,
    arquivo: &Arquivo,
) -> Result<Vec<CategoryStats>, String> {
    let fallback =
        || real_career.history_for(&driver.id, stats_from_driver(driver, fallback_category));
    if !arquivo.existe() {
        return Ok(vec![fallback()]);
    }

    let (mut stats, counted_title_events) =
        read_archive_category_stats(conn, &driver.id, arquivo.linhas(&driver.id))?;
    let team_title_stats = team_champion_title_stats_for_driver(
        &driver.id,
        &counted_title_events,
        team_title_stats_by_driver,
    );
    if stats.is_empty() {
        stats.push(fallback());
    }
    stats.extend(team_title_stats);

    Ok(stats)
}

pub(super) fn active_driver_debut_year(
    driver: &Driver,
    current_year: i32,
    linhas: &[LinhaDoArquivo],
) -> i32 {
    let fallback_year = driver.ano_inicio_carreira as i32;
    let mut archive_year: Option<i32> = None;
    for linha in linhas {
        let category = normalized_archive_category(&linha.snapshot, linha.categoria.clone());
        if category == "unknown"
            || is_especial(&category)
            || !has_competitive_archive_participation(&linha.snapshot)
        {
            continue;
        }
        archive_year = Some(archive_year.map_or(linha.ano, |current| current.min(linha.ano)));
    }

    match archive_year {
        Some(year) => year,
        None => inferred_active_driver_debut_year(driver, current_year, fallback_year),
    }
}

pub(super) fn inferred_active_driver_debut_year(
    driver: &Driver,
    current_year: i32,
    fallback_year: i32,
) -> i32 {
    if current_year > 0 {
        let career_seasons = driver.stats_carreira.temporadas as i32;
        if career_seasons > 0 {
            return (current_year - career_seasons + 1).max(1);
        }
        // Sem temporada fechada, toda largada que ele tem é da temporada em
        // curso: a estreia é este ano. `ano_inicio_carreira` NÃO serve aqui —
        // nasce como pano de fundo (o ano em que o piloto pegou num kart, aos
        // 16), não como estreia na carreira, e fazia o piloto do jogador saltar
        // de 0 pra 5 anos assim que largava pela primeira vez.
        return current_year;
    }

    fallback_year.max(0)
}

pub(super) fn has_competitive_archive_participation(snapshot: &Value) -> bool {
    json_i32(snapshot, "corridas") > 0
        || json_f64(snapshot, "pontos").unwrap_or(0.0) > 0.0
        || json_i32(snapshot, "vitorias") > 0
        || json_i32(snapshot, "podios") > 0
        || json_i32(snapshot, "poles") > 0
        || json_i32(snapshot, "titulos") > 0
}
