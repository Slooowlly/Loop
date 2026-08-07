//! Entrada do ranking global: abre o banco da carreira e monta o payload completo.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use super::*;

pub(crate) fn get_global_driver_rankings_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    selected_driver_id: Option<&str>,
) -> Result<GlobalDriverRankingPayload, String> {
    let db_path = caminho_do_banco(base_dir, career_id)?;
    ranking_do_banco(&db_path, selected_driver_id)
}

fn caminho_do_banco(base_dir: &Path, career_id: &str) -> Result<PathBuf, String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
    }
    Ok(db_path)
}

// ── Memória do último ranking montado ─────────────────────────────────────────
//
// A posição de um piloto no mundo só existe em relação aos outros 600+, então
// não há atalho: cada consulta monta o ranking inteiro. Enquanto o jogador
// folheia fichas — abrir um piloto, seguir para o companheiro de equipe, voltar
// — nada é escrito no banco e o resultado é bit a bit o mesmo; recomputar ali é
// puro desperdício, e é exatamente o intervalo em que a marca do ranking demora
// a aparecer na ficha.
//
// A chave é a ASSINATURA do arquivo (tamanho + mtime do `.db` e do `-wal`, quando
// existe). Cada comando abre e fecha a sua conexão, e o fechamento faz checkpoint
// do WAL — então qualquer escrita mexe no arquivo e derruba a entrada. Assinatura
// ilegível (arquivo sumiu, permissão) desliga o cache em vez de arriscar servir
// número velho.

type Assinatura = ((u64, Option<SystemTime>), Option<(u64, Option<SystemTime>)>);

struct RankingMemorizado {
    db_path: PathBuf,
    assinatura: Assinatura,
    payload: GlobalDriverRankingPayload,
}

static MEMORIA: Mutex<Option<RankingMemorizado>> = Mutex::new(None);

fn ranking_do_banco(
    db_path: &Path,
    selected_driver_id: Option<&str>,
) -> Result<GlobalDriverRankingPayload, String> {
    let assinatura = assinatura_do_banco(db_path);

    if let Some(assinatura) = assinatura.as_ref() {
        if let Ok(memoria) = MEMORIA.lock() {
            if let Some(entrada) = memoria.as_ref() {
                if entrada.db_path == db_path && &entrada.assinatura == assinatura {
                    let mut payload = entrada.payload.clone();
                    payload.selected_driver_id = selected_driver_id.map(str::to_string);
                    return Ok(payload);
                }
            }
        }
    }

    let db = Database::open_existing(db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let payload = build_global_driver_rankings(&db.conn, selected_driver_id)?;

    // A assinatura vale a de ANTES da montagem: `open_existing` roda migrações
    // pendentes e pode escrever no arquivo. Guardar a de depois carimbaria o
    // estado novo num payload lido do estado velho.
    if let Some(assinatura) = assinatura {
        if let Ok(mut memoria) = MEMORIA.lock() {
            *memoria = Some(RankingMemorizado {
                db_path: db_path.to_path_buf(),
                assinatura,
                payload: payload.clone(),
            });
        }
    }

    Ok(payload)
}

fn assinatura_do_banco(db_path: &Path) -> Option<Assinatura> {
    let principal = assinatura_do_arquivo(db_path)?;
    let wal = db_path.with_extension("db-wal");
    Some((principal, assinatura_do_arquivo(&wal)))
}

fn assinatura_do_arquivo(path: &Path) -> Option<(u64, Option<SystemTime>)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()))
}

/// Índice e posição de UM piloto no ranking mundial.
///
/// Roda o ranking inteiro e devolve só a linha pedida: a posição só existe em
/// relação aos outros, então não há atalho barato — o que este ponto de entrada
/// evita é atravessar a ponte com as 200+ linhas do painel para exibir quatro
/// números. `Ok(None)` quando o piloto não tem visibilidade no ranking (sem
/// história competitiva e fora do grid regular), e a ficha simplesmente não
/// desenha a marca.
///
/// Divide a memória de `ranking_do_banco` com o painel completo: abrir o painel
/// e depois folhear fichas paga a montagem UMA vez, e cada ficha seguinte
/// responde na hora enquanto ninguém escrever no banco.
pub(crate) fn get_driver_world_rank_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    driver_id: &str,
) -> Result<Option<DriverWorldRank>, String> {
    let db_path = caminho_do_banco(base_dir, career_id)?;
    let payload = ranking_do_banco(&db_path, Some(driver_id))?;
    let total = payload.rows.len() as i32;

    Ok(payload
        .rows
        .into_iter()
        .find(|row| row.id == driver_id)
        .map(|row| DriverWorldRank {
            indice: row.historical_index,
            posicao: row.historical_rank,
            total,
            delta: row.historical_rank_delta,
        }))
}

pub(super) fn build_global_driver_rankings(
    conn: &Connection,
    selected_driver_id: Option<&str>,
) -> Result<GlobalDriverRankingPayload, String> {
    let current_year = season_queries::get_active_season(conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa do ranking global: {e}"))?
        .map(|season| season.ano)
        .unwrap_or(2024);
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos globais: {e}"))?;
    let team_title_stats_by_driver = load_all_team_champion_title_stats(conn)?;
    let team_lookup = load_team_lookup(conn)?;
    let real_career = RealCareerIndex::load(conn)?;
    let arquivo = Arquivo::carregar_tudo(conn)?;
    let mut entries = Vec::new();
    let mut seen_driver_ids = HashSet::new();
    let mut retired_by_id: HashMap<String, RetiredDriverSnapshot> =
        load_retired_snapshots(conn, &team_title_stats_by_driver, &team_lookup)?
            .into_iter()
            .map(|retired| (retired.id.clone(), retired))
            .collect();

    for driver in drivers {
        seen_driver_ids.insert(driver.id.clone());
        if driver.status == DriverStatus::Aposentado {
            if let Some(retired) = retired_by_id.remove(&driver.id) {
                // Pontuação consistente com ativos: histórico POR CATEGORIA do archive
                // (em vez do agregado de carreira × multiplicador da categoria final, que
                // inflava a carreira toda no peso da endurance). Vazio sem archive → o
                // construtor cai no que ele correu de verdade.
                let archive_stats = load_archive_category_stats(
                    conn,
                    &driver.id,
                    &team_title_stats_by_driver,
                    &arquivo,
                )?;
                entries.push(build_retired_driver_entry_from_driver(
                    retired,
                    &driver,
                    current_year,
                    &team_lookup,
                    archive_stats,
                    &real_career,
                ));
                continue;
            }
        }
        entries.push(build_current_driver_entry(
            conn,
            &driver,
            current_year,
            &team_title_stats_by_driver,
            &team_lookup,
            &real_career,
            &arquivo,
        )?);
    }

    for retired in retired_by_id.into_values() {
        if seen_driver_ids.contains(&retired.id) {
            continue;
        }
        // Aposentado sem registro na tabela `drivers` (purgado): histórico por
        // categoria do archive; se não houver, cai no que ele correu de verdade.
        let archive_stats =
            load_archive_category_stats(conn, &retired.id, &team_title_stats_by_driver, &arquivo)?;
        entries.push(build_retired_driver_entry(
            retired,
            current_year,
            &team_lookup,
            archive_stats,
            &real_career,
        ));
    }

    let unranked_player_driver = entries
        .iter()
        .find(|entry| entry.row.is_jogador)
        .map(|entry| entry.row.clone());
    entries.retain(|entry| has_ranking_visibility(&entry.row));
    let stats_by_driver = entries
        .iter()
        .map(|entry| (entry.row.id.clone(), entry.stats_by_category.clone()))
        .collect::<HashMap<_, _>>();
    let mut rows = entries
        .into_iter()
        .map(|entry| entry.row)
        .collect::<Vec<_>>();
    rows.retain(has_ranking_visibility);
    // Marca os favoritados (watchlist) — alimenta a estrela inline + o filtro "Favoritos".
    let favorites = crate::db::queries::favorites::get_favorite_ids(conn).unwrap_or_default();
    // Split dos pódios por posição (2º/3º) direto dos resultados reais — alimenta o
    // tooltip "quantos pódios não foram vitória". Pilotos sem `race_results` ficam 0.
    let podium_splits = career_podium_splits(conn)?;
    for row in &mut rows {
        row.is_favorito = favorites.contains(&row.id);
        if let Some(&(segundos, terceiros)) = podium_splits.get(&row.id) {
            row.segundos = segundos;
            row.terceiros = terceiros;
        }
    }
    assign_ranks(&mut rows);
    assign_rank_deltas(conn, &mut rows, &stats_by_driver)?;
    let leaders = build_leaders(&rows);
    let player_driver = rows
        .iter()
        .find(|row| row.is_jogador)
        .cloned()
        .or(unranked_player_driver);

    Ok(GlobalDriverRankingPayload {
        selected_driver_id: selected_driver_id.map(str::to_string),
        player_driver,
        rows,
        leaders,
    })
}
