//! Utilitarios compartilhados do fim de semana: leitura da categoria e da proxima corrida do jogador, carimbo de ultimo acesso e aviso de efeito colateral que falhou.

use super::*;

pub(super) fn update_last_played(meta_path: &Path) -> Result<(), String> {
    let content =
        std::fs::read_to_string(meta_path).map_err(|e| format!("Falha ao ler meta.json: {e}"))?;
    let mut meta: SaveMeta =
        serde_json::from_str(&content).map_err(|e| format!("Falha ao parsear meta.json: {e}"))?;
    meta.last_played = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let json = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Falha ao serializar meta.json: {e}"))?;
    std::fs::write(meta_path, json).map_err(|e| format!("Falha ao gravar meta.json: {e}"))
}

pub(super) fn warn_if_side_effect_fails<T>(result: Result<T, String>, context: &str) {
    if let Err(error) = result {
        eprintln!("Aviso: {context}: {error}");
    }
}

pub(super) fn get_player_active_category(
    conn: &rusqlite::Connection,
    active_season: &Season,
) -> Result<Option<String>, String> {
    let player = driver_queries::get_player_driver(conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;

    if active_season.fase.is_racing() {
        if let Some(contract) =
            contract_queries::get_active_especial_contract_for_pilot(conn, &player.id)
                .map_err(|e| format!("Falha ao buscar contrato especial ativo: {e}"))?
        {
            return Ok(Some(contract.categoria));
        }
    }

    if let Some(contract) =
        contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
            .map_err(|e| format!("Falha ao buscar contrato regular ativo: {e}"))?
    {
        return Ok(Some(contract.categoria));
    }

    if active_season.fase.is_racing() {
        if let Some(category) = player.categoria_especial_ativa {
            return Ok(Some(category));
        }
    }

    Ok(player.categoria_atual)
}

pub(crate) fn get_next_player_race(
    conn: &rusqlite::Connection,
    active_season: &Season,
) -> Result<Option<CalendarEntry>, String> {
    let Some(category_id) = get_player_active_category(conn, active_season)? else {
        return Ok(None);
    };

    calendar_queries::get_next_race(conn, &active_season.id, &category_id)
        .map_err(|e| format!("Falha ao buscar proxima corrida do jogador: {e}"))
}
