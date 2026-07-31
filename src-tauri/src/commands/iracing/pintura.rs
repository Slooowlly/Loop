//! Pintura do carro do jogador: exibição do esquema, aplicação automática e vínculo do custid.

use super::*;

/// Pintura que o JOGADOR deve aplicar na garagem do iRacing para ficar na cor do
/// time (igual à IA). A pintura embutida do jogador é account-side, então só dá
/// para MOSTRAR o esquema certo — o usuário aplica uma vez.
#[derive(serde::Serialize)]
pub struct PlayerPaint {
    pub team_name: String,
    pub pattern: String,
    pub color1: String,
    pub color2: String,
    pub color3: String,
    pub spec: String,
}

/// Lê o time do jogador na carreira e devolve o esquema de pintura a aplicar.
#[tauri::command]
pub fn iracing_player_paint(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<PlayerPaint, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
    use crate::iracing_sdk::roster_gen;
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let player =
        dq::get_player_driver(&db.conn).map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let team = cq::get_active_contract_for_pilot(&db.conn, &player.id)
        .ok()
        .flatten()
        .and_then(|contract| {
            tq::get_team_by_id(&db.conn, &contract.equipe_id)
                .ok()
                .flatten()
        })
        .ok_or("Você não tem contrato/time ativo nesta carreira.")?;

    let hex = roster_gen::normalize_hex(&team.cor_primaria);
    Ok(PlayerPaint {
        team_name: team.nome,
        pattern: roster_gen::DESIGN_PATTERN.to_string(),
        color1: format!("#{hex}"),
        color2: format!("#{}", roster_gen::DESIGN_COLOR2),
        color3: format!("#{}", roster_gen::DESIGN_COLOR3),
        spec: format!(
            "{},{hex},{},{}",
            roster_gen::DESIGN_PATTERN,
            roster_gen::DESIGN_COLOR2,
            roster_gen::DESIGN_COLOR3
        ),
    })
}

/// Resultado da pintura automática do carro do jogador.
#[derive(serde::Serialize)]
pub struct ApplyPaintResult {
    pub path: String,
    pub custid: i64,
    pub color: String,
}

/// Escreve a pintura (cor sólida do time) do carro do jogador como custom paint
/// do iRacing: `paint/<carro>/car_<custid>.tga`. Usa o custid já capturado.
#[tauri::command]
pub fn iracing_apply_player_paint(
    app: tauri::AppHandle,
    career_id: String,
    car_key: String,
) -> Result<ApplyPaintResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
    use crate::iracing_sdk::{paint_gen, paths, roster_gen};
    use tauri::Manager;

    let custid = iracing_sdk::cached_custid()
        .ok_or("Ainda não capturei seu custid — abra o iRacing e entre numa sessão uma vez.")?;
    let car =
        roster_gen::car_spec(&car_key).ok_or_else(|| format!("Carro desconhecido: {car_key}"))?;

    // Cor do time do jogador.
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;
    let player =
        dq::get_player_driver(&db.conn).map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let team = cq::get_active_contract_for_pilot(&db.conn, &player.id)
        .ok()
        .flatten()
        .and_then(|c| tq::get_team_by_id(&db.conn, &c.equipe_id).ok().flatten())
        .ok_or("Você não tem contrato/time ativo nesta carreira.")?;
    let hex = roster_gen::normalize_hex(&team.cor_primaria);

    // Escreve car_<custid>.tga na pasta de pintura do carro.
    let dir = paths::car_paint_dir(car.car_path)
        .ok_or("Não foi possível localizar a pasta de pintura do iRacing.")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    let path = dir.join(format!("car_{custid}.tga"));
    paint_gen::write_solid_tga(&path, &hex).map_err(|e| format!("Falha ao gravar pintura: {e}"))?;

    Ok(ApplyPaintResult {
        path: path.display().to_string(),
        custid,
        color: format!("#{hex}"),
    })
}

/// Chave da tabela `meta` (career.db) que guarda o custid do iRacing do jogador
/// VINCULADO a este save. Capturado uma vez (popup da 1ª corrida) e reutilizado
/// para repintar o carro automaticamente a cada troca de equipe no mercado.
const PLAYER_CUSTID_META_KEY: &str = "player_iracing_custid";

/// Mapeia a categoria da carreira no carro do iRacing (mesma regra do export).
fn car_key_for_category(categoria: &str) -> &'static str {
    let c = categoria.to_lowercase();
    if c.contains("gr86") || c.contains("toyota") {
        "gr86"
    } else if c.contains("bmw") || c.contains("m2") {
        "bmwm2"
    } else {
        "mx5" // mazda mx-5 e padrão
    }
}

/// Escreve `car_<custid>.tga` (cor sólida `hex`) na pasta de pintura do carro.
/// Núcleo compartilhado pela pintura da 1ª vez e pela repintura no mercado.
/// Recebe o `hex` já normalizado pelo chamador.
fn write_player_car_tga(car_key: &str, hex: &str, custid: i64) -> Result<(String, String), String> {
    use crate::iracing_sdk::{paint_gen, paths, roster_gen};
    let car =
        roster_gen::car_spec(car_key).ok_or_else(|| format!("Carro desconhecido: {car_key}"))?;
    let hex = roster_gen::normalize_hex(hex);
    let dir = paths::car_paint_dir(car.car_path)
        .ok_or("Não foi possível localizar a pasta de pintura do iRacing.")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    let path = dir.join(format!("car_{custid}.tga"));
    paint_gen::write_solid_tga(&path, &hex).map_err(|e| format!("Falha ao gravar pintura: {e}"))?;
    Ok((path.display().to_string(), format!("#{hex}")))
}

/// Lê o custid já capturado (sampler) ou tenta ler a sessão atual agora.
fn capture_player_custid() -> Option<i64> {
    if let Some(id) = iracing_sdk::cached_custid() {
        return Some(id);
    }
    if let Ok(session) = iracing_sdk::read_session() {
        iracing_sdk::note_session_custid(&session.session_yaml);
    }
    iracing_sdk::cached_custid()
}

/// `true` se já capturamos o custid do jogador — ou seja, ele já conectou ao iRacing
/// (SDK) ao menos uma vez. O front usa para mostrar a opção "pegar a cor do carro"
/// na Sala de Estratégia só quando faz sentido: já temos o ID para poder pintar.
#[tauri::command]
pub fn iracing_has_player_id() -> bool {
    iracing_sdk::cached_custid().is_some()
}

/// Custid do iRacing VINCULADO a este save (`None` se ainda não vinculado). O front
/// usa isto para decidir se mostra o popup de "pegar a cor do carro" na 1ª corrida.
#[tauri::command]
pub fn iracing_linked_custid(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Option<i64>, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;
    let val = crate::db::queries::meta::get_meta_value(&db.conn, PLAYER_CUSTID_META_KEY)
        .map_err(|e| format!("Falha ao ler meta: {e}"))?;
    Ok(val.and_then(|s| s.trim().parse::<i64>().ok()))
}

/// 1ª vez (popup da Sala de Estratégia): captura o custid do iRacing, VINCULA ao
/// save (tabela meta) e pinta o carro na cor do time atual do jogador. Erro claro
/// se o custid ainda não foi capturado (precisa abrir o iRacing uma vez).
#[tauri::command]
pub fn iracing_link_player_paint(
    app: tauri::AppHandle,
    career_id: String,
    car_key: String,
) -> Result<ApplyPaintResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
    use crate::iracing_sdk::roster_gen;
    use tauri::Manager;

    let custid = capture_player_custid().ok_or(
        "Ainda não capturei seu ID do iRacing — abra o iRacing e entre numa sessão ou pista uma vez para vincularmos.",
    )?;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;
    let player =
        dq::get_player_driver(&db.conn).map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let team = cq::get_active_contract_for_pilot(&db.conn, &player.id)
        .ok()
        .flatten()
        .and_then(|c| tq::get_team_by_id(&db.conn, &c.equipe_id).ok().flatten())
        .ok_or("Você não tem contrato/time ativo nesta carreira.")?;

    let (path, color) = write_player_car_tga(
        &car_key,
        &roster_gen::normalize_hex(&team.cor_primaria),
        custid,
    )?;

    crate::db::queries::meta::put_meta_value(&db.conn, PLAYER_CUSTID_META_KEY, &custid.to_string())
        .map_err(|e| format!("Falha ao vincular o ID ao save: {e}"))?;

    Ok(ApplyPaintResult {
        path,
        custid,
        color,
    })
}

/// Mercado: repinta o carro do jogador na cor da NOVA equipe ao aceitar um contrato.
/// Usa o custid vinculado ao save (ou o capturado na sessão, persistindo-o). Devolve
/// `None` silenciosamente se ainda não há custid (jamais abriu o iRacing) — o front
/// simplesmente não mostra o toast nesse caso.
#[tauri::command]
pub fn iracing_apply_market_paint(
    app: tauri::AppHandle,
    career_id: String,
    team_color: String,
    category: String,
) -> Result<Option<ApplyPaintResult>, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let linked = crate::db::queries::meta::get_meta_value(&db.conn, PLAYER_CUSTID_META_KEY)
        .map_err(|e| format!("Falha ao ler meta: {e}"))?
        .and_then(|s| s.trim().parse::<i64>().ok());
    let custid = match linked.or_else(capture_player_custid) {
        Some(id) => id,
        None => return Ok(None), // sem ID ainda → nada a fazer (silencioso)
    };
    if linked.is_none() {
        let _ = crate::db::queries::meta::put_meta_value(
            &db.conn,
            PLAYER_CUSTID_META_KEY,
            &custid.to_string(),
        );
    }

    let car_key = car_key_for_category(&category);
    let (path, color) = write_player_car_tga(car_key, &team_color, custid)?;
    Ok(Some(ApplyPaintResult {
        path,
        custid,
        color,
    }))
}
