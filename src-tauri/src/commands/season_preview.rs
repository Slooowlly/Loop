//! Matéria de expectativas de PRÉ-TEMPORADA (abre a revista antes da 1ª corrida).
//!
//! Enquanto o jogador não disputou nenhuma etapa, a aba Notícias não tem edição de
//! corrida para mostrar. Em vez do "livro fechado", a revista abre com UMA matéria de
//! expectativas sobre o campeonato do ano — favoritos, o cenário do jogador, a pista
//! de abertura e o campeão em título.
//!
//! Espelha o boletim de corrida (`ai_news::enrich_race_news_ai`): monta fatos curados
//! determinísticos (i18n via rust-i18n), manda ao servidor (que chama o Gemini) e
//! cacheia o texto. Cache por TEMPORADA+categoria em `ai_race_story` (mesmo key-value
//! do boletim, com um `news_id` sintético). Em QUALQUER falha — inclusive o endpoint
//! `/season-preview` ainda não existir no servidor — devolve `story: None` e o front
//! cai no texto-placeholder (nunca quebra a tela).

use std::fmt::Write;

use tauri::Manager;

use crate::commands::ai_news::AiNewsResult;
use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::ai_story;
use crate::narrative::client::{self, StoryError};

/// Quantos favoritos listar na matéria (por potencial/skill).
const FAVORITES_COUNT: usize = 6;
/// Fama mínima (escala de exibição) para marcar um piloto como "nome de público" na
/// lista de favoritos — mesma régua de Estrela do rodapé "Do mundo do Grid".
const STAR_MIN_FAMA: f64 = 71.0;

/// Categoria atual do jogador: do contrato ativo; senão, do campo do piloto.
fn player_category(conn: &rusqlite::Connection, player: &crate::models::driver::Driver) -> String {
    use crate::db::queries::contracts;
    contracts::get_active_contract_for_pilot(conn, &player.id)
        .ok()
        .flatten()
        .map(|c| c.categoria)
        .or_else(|| player.categoria_atual.clone())
        .unwrap_or_default()
}

/// Bagagem de carreira de um piloto, destilada numa frase curta para os fatos: sem
/// corrida no currículo → estreante; com histórico → nº de corridas + o maior feito
/// (título > vitória > pódio > ainda sem pódio). É de propósito mais sobre HISTÓRICO
/// que sobre skill — dois pilotos de mesmo potencial contam histórias diferentes.
fn experience_clause(d: &crate::models::driver::Driver) -> String {
    let races = d.stats_carreira.corridas as i64;
    if races == 0 {
        return rust_i18n::t!("season_preview.facts.exp_debut").to_string();
    }
    if d.stats_carreira.titulos > 0 {
        rust_i18n::t!(
            "season_preview.facts.exp_titles",
            races = races,
            n = d.stats_carreira.titulos as i64
        )
        .to_string()
    } else if d.stats_carreira.vitorias > 0 {
        rust_i18n::t!(
            "season_preview.facts.exp_wins",
            races = races,
            n = d.stats_carreira.vitorias as i64
        )
        .to_string()
    } else if d.stats_carreira.podios > 0 {
        rust_i18n::t!(
            "season_preview.facts.exp_podiums",
            races = races,
            n = d.stats_carreira.podios as i64
        )
        .to_string()
    } else {
        rust_i18n::t!("season_preview.facts.exp_nopodium", races = races).to_string()
    }
}

/// Nome da equipe (nas contas do contrato ativo) + cor primária de um piloto, se houver.
fn driver_team(conn: &rusqlite::Connection, driver_id: &str) -> Option<(String, String)> {
    use crate::db::queries::{contracts, teams};
    let contract = contracts::get_active_contract_for_pilot(conn, driver_id)
        .ok()
        .flatten()?;
    let color = teams::get_team_by_id(conn, &contract.equipe_id)
        .ok()
        .flatten()
        .map(|t| t.cor_primaria)
        .unwrap_or_else(|| "#888".to_string());
    Some((contract.equipe_nome, color))
}

/// Monta o "fact bundle" da pré-temporada + o mapa `nome da equipe → cor` (p/ o front
/// colorir os nomes citados). String de fatos vazia → sem material suficiente (o front
/// cai no placeholder). Só depende do banco + `base_dir/career_id` (para o campeão
/// anterior, que mora no arquivo de histórico).
fn build_season_preview_facts(
    conn: &rusqlite::Connection,
    base_dir: &std::path::Path,
    career_id: &str,
) -> (String, serde_json::Value) {
    use crate::db::queries::{calendar, contracts, drivers, player_nemesis, seasons, team_car, teams};

    let empty = (String::new(), serde_json::json!({}));

    let Ok(player) = drivers::get_player_driver(conn) else {
        return empty;
    };
    let categoria = player_category(conn, &player);
    if categoria.is_empty() {
        return empty;
    }
    let Some(season) = seasons::get_active_season(conn).ok().flatten() else {
        return empty;
    };

    // Campo da categoria (para favoritos e ranking do jogador).
    let field = drivers::get_drivers_by_active_category(conn, &categoria).unwrap_or_default();
    if field.is_empty() {
        return empty;
    }

    // Equipes da categoria: cor (p/ colorir TODOS os nomes citados) + nível de carro.
    let teams_in_cat = teams::get_teams_by_category(conn, &categoria).unwrap_or_default();
    let mut teams_map = serde_json::Map::new();
    let mut car_levels: Vec<(String, u8)> = Vec::new();
    for tm in &teams_in_cat {
        teams_map
            .entry(tm.nome.clone())
            .or_insert_with(|| serde_json::json!(tm.cor_primaria));
        if let Ok(Some(car)) = team_car::get_team_car(conn, &tm.id) {
            car_levels.push((tm.nome.clone(), car.display_level()));
        }
    }

    let mut f = String::new();

    // ---- CENÁRIO (cabeçalho) ----
    let cat_label = crate::constants::categories::get_category_config(&categoria)
        .map(|c| c.nome.to_string())
        .unwrap_or_else(|| categoria.clone());
    let season_num = season.numero.to_string();
    let year = season.ano.to_string();
    let _ = writeln!(
        f,
        "{}",
        rust_i18n::t!(
            "season_preview.facts.scenario",
            category = cat_label.as_str(),
            season = season_num.as_str(),
            year = year.as_str()
        )
    );

    // Pista de abertura + total de etapas (calendário ordenado por rodada ASC).
    let calendar_entries = calendar::get_calendar(conn, &season.id, &categoria).unwrap_or_default();
    if let Some(opening) = calendar_entries.first() {
        let rounds = calendar_entries.len().to_string();
        let _ = writeln!(
            f,
            "{}",
            rust_i18n::t!(
                "season_preview.facts.opening",
                track = opening.track_name.as_str(),
                rounds = rounds.as_str()
            )
        );
    }

    // ---- Campeão em título (temporada anterior) ----
    if let Some(champ_id) = crate::commands::career::get_previous_champions_in_base_dir(
        base_dir, career_id, &categoria,
    )
    .ok()
    .and_then(|c| c.driver_champion_id)
    {
        if let Ok(champ) = drivers::get_driver(conn, &champ_id) {
            let team = driver_team(conn, &champ_id);
            if let Some((name, color)) = &team {
                teams_map.insert(name.clone(), serde_json::json!(color));
            }
            let _ = writeln!(
                f,
                "{}",
                match &team {
                    Some((name, _)) => rust_i18n::t!(
                        "season_preview.facts.champion_team",
                        name = champ.nome.as_str(),
                        team = name.as_str()
                    ),
                    None => rust_i18n::t!(
                        "season_preview.facts.champion",
                        name = champ.nome.as_str()
                    ),
                }
            );
        }
    }

    // ---- CONDIÇÃO DOS CARROS (nível 1..=10 por equipe) ----
    // Se todos os carros estão no mesmo nível (típico no rookie), a diferença é 100%
    // do piloto; se há material desigual, o carro forte/fraco vira parte da história.
    if car_levels.len() >= 2 {
        let max = car_levels.iter().map(|(_, l)| *l).max().unwrap_or(1);
        let min = car_levels.iter().map(|(_, l)| *l).min().unwrap_or(1);
        if max == min {
            let _ = writeln!(
                f,
                "{}",
                rust_i18n::t!("season_preview.facts.car_uniform", level = max as i64)
            );
        } else {
            let best = car_levels.iter().max_by_key(|(_, l)| *l).unwrap();
            let worst = car_levels.iter().min_by_key(|(_, l)| *l).unwrap();
            let _ = writeln!(
                f,
                "{}",
                rust_i18n::t!(
                    "season_preview.facts.car_unequal",
                    best_team = best.0.as_str(),
                    best = best.1 as i64,
                    worst_team = worst.0.as_str(),
                    worst = worst.1 as i64
                )
            );
        }
    }

    // ---- FAVORITOS (por potencial/skill) ----
    let mut ranked: Vec<&crate::models::driver::Driver> = field.iter().collect();
    ranked.sort_by(|a, b| {
        b.atributos
            .skill
            .partial_cmp(&a.atributos.skill)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let _ = writeln!(f, "{}", rust_i18n::t!("season_preview.facts.favorites_head"));
    for d in ranked.iter().take(FAVORITES_COUNT) {
        let team = driver_team(conn, &d.id);
        if let Some((name, color)) = &team {
            teams_map
                .entry(name.clone())
                .or_insert_with(|| serde_json::json!(color));
        }
        let skill = d.atributos.skill.round() as i64;
        let star = if d.atributos.midia >= STAR_MIN_FAMA {
            rust_i18n::t!("season_preview.facts.favorite_star").to_string()
        } else {
            String::new()
        };
        let exp = experience_clause(d);
        let _ = writeln!(
            f,
            "{}",
            match &team {
                Some((name, _)) => rust_i18n::t!(
                    "season_preview.facts.favorite_line",
                    name = d.nome.as_str(),
                    team = name.as_str(),
                    skill = skill.to_string().as_str(),
                    exp = exp.as_str(),
                    star = star.as_str()
                ),
                None => rust_i18n::t!(
                    "season_preview.facts.favorite_line_noteam",
                    name = d.nome.as_str(),
                    skill = skill.to_string().as_str(),
                    exp = exp.as_str(),
                    star = star.as_str()
                ),
            }
        );
    }

    // ---- Maior salário do grid ----
    // O piloto mais bem pago carrega expectativa embutida (o mercado apostou nele). Só
    // o topo entra; salário 0/uniforme (típico no rookie) não vira fato.
    {
        let mut top: Option<(String, String, f64)> = None; // (nome, equipe, salário)
        for d in &field {
            if let Ok(Some(ct)) = contracts::get_active_contract_for_pilot(conn, &d.id) {
                if ct.salario_anual > top.as_ref().map_or(0.0, |(_, _, s)| *s) {
                    top = Some((d.nome.clone(), ct.equipe_nome.clone(), ct.salario_anual));
                }
            }
        }
        if let Some((name, team, sal)) = top {
            if sal > 0.0 {
                let value = format!("$ {}", sal.round() as i64);
                let _ = writeln!(
                    f,
                    "{}",
                    rust_i18n::t!(
                        "season_preview.facts.salary",
                        name = name.as_str(),
                        team = team.as_str(),
                        value = value.as_str()
                    )
                );
            }
        }
    }

    // ---- Cenário do jogador (ranking por potencial no grid) ----
    let field_size = ranked.len().to_string();
    let player_rank = ranked
        .iter()
        .position(|d| d.id == player.id)
        .map(|i| (i + 1).to_string());
    let player_team = driver_team(conn, &player.id);
    if let Some((name, color)) = &player_team {
        teams_map
            .entry(name.clone())
            .or_insert_with(|| serde_json::json!(color));
    }
    let _ = writeln!(
        f,
        "{}",
        match (&player_team, &player_rank) {
            (Some((team, _)), Some(rank)) => rust_i18n::t!(
                "season_preview.facts.player_line",
                name = player.nome.as_str(),
                team = team.as_str(),
                rank = rank.as_str(),
                field = field_size.as_str()
            ),
            (Some((team, _)), None) => rust_i18n::t!(
                "season_preview.facts.player_line_norank",
                name = player.nome.as_str(),
                team = team.as_str()
            ),
            (None, _) => rust_i18n::t!(
                "season_preview.facts.player_line_noteam",
                name = player.nome.as_str()
            ),
        }
    );
    // Bagagem do jogador (estreante x veterano com feitos) — mesma régua dos rivais.
    let _ = writeln!(
        f,
        "{}",
        rust_i18n::t!(
            "season_preview.facts.player_exp",
            exp = experience_clause(&player).as_str()
        )
    );

    // ---- Rivalidade carregada da temporada anterior (nêmesis) ----
    if let Ok(Some(nemesis_id)) = player_nemesis::get_current_nemesis(conn) {
        if let Ok(nemesis) = drivers::get_driver(conn, &nemesis_id) {
            let _ = writeln!(
                f,
                "{}",
                rust_i18n::t!(
                    "season_preview.facts.nemesis",
                    name = nemesis.nome.as_str()
                )
            );
        }
    }

    (f, serde_json::Value::Object(teams_map))
}

/// Comando: matéria de expectativas de pré-temporada por IA. O front chama ao abrir a
/// revista quando ainda não há edição de corrida. Cacheia por `season-preview:{season}:
/// {categoria}` no `ai_race_story` (mesmo key-value do boletim). Em cooldown/rede/cota
/// ou endpoint ausente → `story: None` e o front usa o placeholder.
#[tauri::command]
pub async fn enrich_season_preview_ai(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<AiNewsResult, String> {
    // async + spawn_blocking: o fetch de IA é bloqueante (até 45s) e travaria a THREAD
    // PRINCIPAL do Tauri se rodasse síncrono. Ver enrich_race_news_ai.
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

        let mut config = AppConfig::load_or_default(&base_dir);
        let install_id = config.get_or_create_install_id();
        let lang = config.language.clone();

        let db_path = config.saves_dir().join(&career_id).join("career.db");
        let db =
            Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

        // Chave de cache estável por temporada+categoria.
        let (facts, teams_value) =
            build_season_preview_facts(&db.conn, &base_dir, &career_id);
        if facts.trim().is_empty() {
            return Ok(AiNewsResult {
                story: None,
                status: "unavailable".to_string(),
                teams: None,
            });
        }

        let season_id = crate::db::queries::seasons::get_active_season(&db.conn)
            .ok()
            .flatten()
            .map(|s| s.id)
            .unwrap_or_default();
        let category = crate::db::queries::drivers::get_player_driver(&db.conn)
            .ok()
            .map(|p| player_category(&db.conn, &p))
            .unwrap_or_default();
        let cache_key = format!("season-preview:{season_id}:{category}");

        let teams = if teams_value.as_object().map_or(true, |m| m.is_empty()) {
            None
        } else {
            Some(teams_value.clone())
        };

        // Já gerado antes → devolve do cache (instantâneo, sem tocar no servidor).
        if let Ok(Some(row)) = ai_story::get_story(&db.conn, &cache_key) {
            if let Some(story) = row.story {
                return Ok(AiNewsResult {
                    story: Some(story),
                    status: "cached".to_string(),
                    teams,
                });
            }
        }

        // 1ª vez: gera no servidor e cacheia (fatos + texto).
        match client::fetch_season_preview(&facts, &lang, &install_id) {
            Ok(story) => {
                let teams_json = serde_json::to_string(&teams_value).unwrap_or_else(|_| "{}".into());
                if let Err(e) =
                    ai_story::store_race_facts(&db.conn, &cache_key, &facts, &teams_json)
                {
                    eprintln!("[season-preview] Falha ao guardar fatos: {e:?}");
                }
                if let Err(e) = ai_story::set_story(&db.conn, &cache_key, &story) {
                    eprintln!("[season-preview] Falha ao cachear matéria: {e:?}");
                }
                Ok(AiNewsResult {
                    story: Some(story),
                    status: "ok".to_string(),
                    teams,
                })
            }
            Err(StoryError::RateLimited) => Ok(AiNewsResult {
                story: None,
                status: "rate_limited".to_string(),
                teams,
            }),
            Err(_) => Ok(AiNewsResult {
                story: None,
                status: "error".to_string(),
                teams,
            }),
        }
    })
    .await
    .map_err(|e| format!("Falha ao executar matéria de pré-temporada: {e}"))?
}
