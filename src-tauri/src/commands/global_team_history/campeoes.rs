//! Salão dos campeões de uma faixa: quem levou o título de construtores em cada
//! temporada, com que dupla, e o agregado de quem mais venceu.
//!
//! A fonte é a mesma que alimenta a estrela do ranking (`titulos_construtores = 1`
//! em `team_season_archive`), então a contagem daqui nunca diverge da de lá.

use super::*;

use crate::commands::career_types::{
    BandChampionDriver, BandChampionSeason, BandChampionsPayload, BandDynasty,
};

pub(crate) fn get_band_champions_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    band_key: &str,
) -> Result<BandChampionsPayload, String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    build_band_champions(&db.conn, band_key)
}

pub(crate) fn build_band_champions(
    conn: &Connection,
    band_key: &str,
) -> Result<BandChampionsPayload, String> {
    let band = resolve_band(band_key).ok_or_else(|| format!("Faixa desconhecida: {band_key}"))?;

    let seasons = load_champion_seasons(conn, band)?;
    let dynasties = build_dynasties(&seasons);

    Ok(BandChampionsPayload {
        band_key: band.key.to_string(),
        band_label: band.label.to_string(),
        dynasties,
        seasons,
    })
}

/// A chave da faixa é única no jogo inteiro, então a busca varre todas as famílias —
/// quem chama tem a chave, não precisa saber de que família ela é.
fn resolve_band(band_key: &str) -> Option<&'static TeamHistoryBandDef> {
    FAMILY_DEFS
        .iter()
        .flat_map(|family| family.bands.iter())
        .find(|band| band.key == band_key)
}

fn load_champion_seasons(
    conn: &Connection,
    band: &TeamHistoryBandDef,
) -> Result<Vec<BandChampionSeason>, String> {
    // O nome do piloto vem de `drivers` quando ele ainda existe e do arquivo da
    // temporada quando não existe mais — pilotos se aposentam, e um campeão de 2004
    // não pode virar um id cru na tela por causa disso.
    //
    // O campeão de pilotos precisa ser filtrado pela classe também: `production_challenger`
    // e `endurance` têm três classes disputando a mesma categoria, então há três linhas
    // com `posicao_campeonato = 1` por temporada. Como `driver_season_archive` não tem
    // coluna `classe`, a classe sai do `snapshot_json`. Sem esse filtro o `LIMIT 1` pegava
    // o campeão de uma classe irmã e a faixa inteira aparecia sem campeão marcado.
    let mut stmt = conn
        .prepare(
            "SELECT
                a.ano,
                a.team_id,
                COALESCE(t.nome, a.team_id) AS nome,
                COALESCE(NULLIF(TRIM(t.cor_primaria), ''), '#58a6ff') AS cor_primaria,
                COALESCE(a.vitorias, 0) AS vitorias,
                a.piloto_1_id,
                COALESCE(d1.nome, s1.nome, a.piloto_1_id) AS piloto_1_nome,
                a.piloto_2_id,
                COALESCE(d2.nome, s2.nome, a.piloto_2_id) AS piloto_2_nome,
                (SELECT c.piloto_id
                   FROM driver_season_archive c
                  WHERE c.season_number = a.season_number
                    AND c.categoria = a.categoria
                    AND c.posicao_campeonato = 1
                    AND (?2 = ''
                         OR COALESCE(
                              NULLIF(TRIM(json_extract(c.snapshot_json, '$.classe')), ''),
                              ''
                            ) = ?2)
                  LIMIT 1) AS campeao_pilotos
             FROM team_season_archive a
             LEFT JOIN teams t ON t.id = a.team_id
             LEFT JOIN drivers d1 ON d1.id = a.piloto_1_id
             LEFT JOIN drivers d2 ON d2.id = a.piloto_2_id
             LEFT JOIN driver_season_archive s1
                    ON s1.piloto_id = a.piloto_1_id AND s1.season_number = a.season_number
             LEFT JOIN driver_season_archive s2
                    ON s2.piloto_id = a.piloto_2_id AND s2.season_number = a.season_number
             WHERE a.titulos_construtores = 1
               AND a.categoria = ?1
               AND COALESCE(NULLIF(TRIM(a.classe), ''), '') = ?2
             ORDER BY a.ano DESC",
        )
        .map_err(|e| format!("Falha ao preparar campeoes da faixa: {e}"))?;

    let rows = stmt
        .query_map(
            rusqlite::params![band.category, band.class_name.unwrap_or("")],
            |row| {
                let campeao_pilotos: Option<String> = row.get(9)?;
                let mut drivers = Vec::new();
                for (id_index, nome_index) in [(5usize, 6usize), (7, 8)] {
                    let driver_id: Option<String> = row.get(id_index)?;
                    let Some(driver_id) = driver_id.filter(|value| !value.trim().is_empty()) else {
                        continue;
                    };
                    let nome: Option<String> = row.get(nome_index)?;
                    let is_season_champion = campeao_pilotos.as_deref() == Some(driver_id.as_str());
                    drivers.push(BandChampionDriver {
                        nome: nome.unwrap_or_else(|| driver_id.clone()),
                        driver_id,
                        is_season_champion,
                    });
                }
                Ok(BandChampionSeason {
                    year: row.get(0)?,
                    team_id: row.get(1)?,
                    nome: row.get(2)?,
                    cor_primaria: row.get(3)?,
                    wins: row.get(4)?,
                    drivers,
                })
            },
        )
        .map_err(|e| format!("Falha ao consultar campeoes da faixa: {e}"))?;

    let mut collected = Vec::new();
    for row in rows {
        collected.push(row.map_err(|e| format!("Falha ao ler campeoes da faixa: {e}"))?);
    }
    Ok(collected)
}

/// Agregado por equipe. Ordena por títulos e, no empate, por quem venceu mais
/// recentemente — entre duas equipes de 3 títulos, a que ainda ganha é a que o
/// jogador reconhece como forte hoje.
fn build_dynasties(seasons: &[BandChampionSeason]) -> Vec<BandDynasty> {
    let mut order: Vec<String> = Vec::new();
    let mut by_team: HashMap<String, BandDynasty> = HashMap::new();

    for season in seasons {
        let entry = by_team.entry(season.team_id.clone()).or_insert_with(|| {
            order.push(season.team_id.clone());
            BandDynasty {
                team_id: season.team_id.clone(),
                nome: season.nome.clone(),
                cor_primaria: season.cor_primaria.clone(),
                titles: 0,
                last_year: season.year,
            }
        });
        entry.titles += 1;
        entry.last_year = entry.last_year.max(season.year);
    }

    let mut dynasties: Vec<BandDynasty> = order
        .into_iter()
        .filter_map(|team_id| by_team.remove(&team_id))
        .collect();
    dynasties.sort_by(|left, right| {
        right
            .titles
            .cmp(&left.titles)
            .then(right.last_year.cmp(&left.last_year))
            .then(left.nome.cmp(&right.nome))
    });
    dynasties
}
