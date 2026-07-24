//! Mapeamento de linha → `Team` e helpers compartilhados pelos submódulos.

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::models::team::{Team, TeamHierarchyClimate};

/// Anexa o carro (tabela `team_car`) a cada time carregado — o Sistema de Nível do Carro.
/// Times sem carro persistido (saves antigos, pré-seed) ficam com `car: None` (o sim cai
/// no fallback do `car_performance` escalar).
pub(super) fn attach_cars(conn: &Connection, teams: &mut [Team]) -> Result<(), DbError> {
    for team in teams.iter_mut() {
        team.car = crate::db::queries::team_car::get_team_car(conn, &team.id)?;
    }
    Ok(())
}

pub(super) fn collect_teams(
    mapped: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Team>>,
) -> Result<Vec<Team>, DbError> {
    let mut result = Vec::new();
    for row in mapped {
        result.push(row?);
    }
    Ok(result)
}

pub(super) fn team_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Team> {
    let hierarquia_status_value: String = row.get("hierarquia_status")?;
    let hierarquia_status = TeamHierarchyClimate::from_str_strict(&hierarquia_status_value)
        .map(|status| status.as_str().to_string())
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
            )
        })?;

    Ok(Team {
        id: row.get("id")?,
        nome: row.get("nome")?,
        nome_curto: row.get("nome_curto")?,
        cor_primaria: row.get("cor_primaria")?,
        cor_secundaria: row.get("cor_secundaria")?,
        pais_sede: row.get("pais_sede")?,
        ano_fundacao: required_i32_column(row, "ano_fundacao")?,
        categoria: row.get("categoria")?,
        ativa: row.get::<_, i64>("ativa")? != 0,
        marca: row.get("marca")?,
        classe: row.get("classe")?,
        piloto_1_id: row.get("piloto_1_id")?,
        piloto_2_id: row.get("piloto_2_id")?,
        car_performance: row.get("car_performance")?,
        // Anexado pelos loaders (get_teams_*) a partir da tabela `team_car`.
        car: None,
        confiabilidade: row.get("confiabilidade")?,
        pit_strategy_risk: row.get("pit_strategy_risk")?,
        pit_crew_quality: row.get("pit_crew_quality")?,
        budget: row.get("budget")?,
        cash_balance: row.get("cash_balance")?,
        debt_balance: row.get("debt_balance")?,
        financial_state: row.get("financial_state")?,
        season_strategy: row.get("season_strategy")?,
        last_round_income: row.get("last_round_income")?,
        last_round_expenses: row.get("last_round_expenses")?,
        last_round_net: row.get("last_round_net")?,
        parachute_payment_remaining: row.get("parachute_payment_remaining")?,
        facilities: row.get("facilities")?,
        engineering: row.get("engineering")?,
        reputacao: row.get("reputacao")?,
        morale: row.get("morale")?,
        aerodinamica: row.get("aerodinamica")?,
        motor: row.get("motor")?,
        chassi: row.get("chassi")?,
        hierarquia_n1_id: row.get("hierarquia_n1_id")?,
        hierarquia_n2_id: row.get("hierarquia_n2_id")?,
        hierarquia_status,
        hierarquia_tensao: row.get("hierarquia_tensao")?,
        hierarquia_duelos_total: required_i32_column(row, "hierarquia_duelos_total")?,
        hierarquia_duelos_n2_vencidos: required_i32_column(row, "hierarquia_duelos_n2_vencidos")?,
        hierarquia_sequencia_n2: required_i32_column(row, "hierarquia_sequencia_n2")?,
        hierarquia_sequencia_n1: required_i32_column(row, "hierarquia_sequencia_n1")?,
        hierarquia_inversoes_temporada: required_i32_column(row, "hierarquia_inversoes_temporada")?,
        stats_vitorias: required_i32_column(row, "stats_vitorias")?,
        stats_podios: required_i32_column(row, "stats_podios")?,
        stats_poles: required_i32_column(row, "stats_poles")?,
        stats_pontos: required_i32_column(row, "stats_pontos")?,
        stats_melhor_resultado: required_i32_column(row, "stats_melhor_resultado")?,
        historico_vitorias: required_i32_column(row, "historico_vitorias")?,
        historico_podios: required_i32_column(row, "historico_podios")?,
        historico_poles: required_i32_column(row, "historico_poles")?,
        historico_pontos: required_i32_column(row, "historico_pontos")?,
        historico_titulos_pilotos: required_i32_column(row, "historico_titulos_pilotos")?,
        historico_titulos_construtores: required_i32_column(row, "carreira_titulos")?,
        temporada_atual: required_i32_column(row, "temporada_atual")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        is_player_team: row.get::<_, i64>("is_player_team")? != 0,
        parent_team_id: row.get("parent_team_id")?,
        aceita_rookies: required_i32_column(row, "aceita_rookies")? != 0,
        meta_posicao: required_i32_column(row, "meta_posicao")?,
        temp_posicao: required_i32_column(row, "temp_posicao")?,
        categoria_anterior: row.get("categoria_anterior")?,
    })
}

pub(super) fn ensure_team_rows_affected(
    affected: usize,
    team_id: &str,
    operation: &str,
) -> Result<(), DbError> {
    if affected == 0 {
        return Err(DbError::NotFound(format!(
            "Equipe '{team_id}' nao encontrada ao {operation}"
        )));
    }
    Ok(())
}

fn required_i32_column(row: &rusqlite::Row<'_>, column_name: &str) -> rusqlite::Result<i32> {
    let value = row.get::<_, i64>(column_name)?;
    i32::try_from(value).map_err(|_| invalid_integer_conversion_error(column_name, value))
}

fn invalid_integer_conversion_error(column_name: &str, value: i64) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Integer,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("coluna '{column_name}' fora do range i32: {value}"),
        )),
    )
}
