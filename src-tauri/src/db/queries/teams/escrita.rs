//! Escrita da linha de equipe: insert, update completo, stats e remoção.

use rusqlite::{params, Connection};

use crate::db::connection::DbError;
use crate::finance::planning::sync_legacy_budget_index;
use crate::models::team::Team;

use super::mapeamento::ensure_team_rows_affected;

pub fn insert_team(conn: &Connection, team: &Team) -> Result<(), DbError> {
    let mut persisted_team = team.clone();
    sync_legacy_budget_index(&mut persisted_team);
    let team = &persisted_team;

    conn.execute(
        "INSERT INTO teams (
            id, nome, nome_curto, cor_primaria, cor_secundaria, pais_sede,
            ano_fundacao, categoria, ativa, marca, classe, piloto_1_id, piloto_2_id,
            is_player_team, car_performance, confiabilidade, pit_strategy_risk,
            pit_crew_quality, budget, cash_balance, debt_balance, financial_state,
            season_strategy, last_round_income, last_round_expenses, last_round_net,
            parachute_payment_remaining, socorros_na_temporada, socorros_temporada_ref, facilities,
            engineering, reputacao, morale, aerodinamica, motor, chassi,
            hierarquia_n1_id, hierarquia_n2_id, hierarquia_status, hierarquia_tensao,
            hierarquia_duelos_total, hierarquia_duelos_n2_vencidos, hierarquia_sequencia_n2,
            hierarquia_sequencia_n1, hierarquia_inversoes_temporada,
            parent_team_id, aceita_rookies, meta_posicao, stats_vitorias, stats_podios,
            stats_poles, stats_pontos, stats_melhor_resultado,
            temp_posicao, historico_vitorias, historico_podios,
            historico_poles, historico_pontos, historico_titulos_pilotos,
            carreira_titulos, temporada_atual, created_at, updated_at,
            categoria_anterior
        ) VALUES (
            :id, :nome, :nome_curto, :cor_primaria, :cor_secundaria, :pais_sede,
            :ano_fundacao, :categoria, :ativa, :marca, :classe, :piloto_1_id, :piloto_2_id,
            :is_player_team, :car_performance, :confiabilidade, :pit_strategy_risk,
            :pit_crew_quality, :budget, :cash_balance, :debt_balance, :financial_state,
            :season_strategy, :last_round_income, :last_round_expenses, :last_round_net,
            :parachute_payment_remaining, :socorros_na_temporada, :socorros_temporada_ref, :facilities,
            :engineering, :reputacao, :morale, :aerodinamica, :motor, :chassi,
            :hierarquia_n1_id, :hierarquia_n2_id, :hierarquia_status, :hierarquia_tensao,
            :hierarquia_duelos_total, :hierarquia_duelos_n2_vencidos, :hierarquia_sequencia_n2,
            :hierarquia_sequencia_n1, :hierarquia_inversoes_temporada,
            :parent_team_id, :aceita_rookies, :meta_posicao, :stats_vitorias, :stats_podios,
            :stats_poles, :stats_pontos, :stats_melhor_resultado,
            :temp_posicao, :historico_vitorias, :historico_podios,
            :historico_poles, :historico_pontos, :historico_titulos_pilotos,
            :carreira_titulos, :temporada_atual, :created_at, :updated_at,
            :categoria_anterior
        )",
        rusqlite::named_params! {
            ":id": &team.id,
            ":nome": &team.nome,
            ":nome_curto": &team.nome_curto,
            ":cor_primaria": &team.cor_primaria,
            ":cor_secundaria": &team.cor_secundaria,
            ":pais_sede": &team.pais_sede,
            ":ano_fundacao": team.ano_fundacao,
            ":categoria": &team.categoria,
            ":ativa": team.ativa as i64,
            ":marca": &team.marca,
            ":classe": &team.classe,
            ":piloto_1_id": &team.piloto_1_id,
            ":piloto_2_id": &team.piloto_2_id,
            ":is_player_team": team.is_player_team as i64,
            ":car_performance": team.car_performance,
            ":confiabilidade": team.confiabilidade,
            ":pit_strategy_risk": team.pit_strategy_risk,
            ":pit_crew_quality": team.pit_crew_quality,
            ":budget": team.budget,
            ":cash_balance": team.cash_balance,
            ":debt_balance": team.debt_balance,
            ":financial_state": &team.financial_state,
            ":season_strategy": &team.season_strategy,
            ":last_round_income": team.last_round_income,
            ":last_round_expenses": team.last_round_expenses,
            ":last_round_net": team.last_round_net,
            ":parachute_payment_remaining": team.parachute_payment_remaining,
            ":socorros_na_temporada": team.socorros_na_temporada,
            ":socorros_temporada_ref": team.socorros_temporada_ref,
            ":facilities": team.facilities,
            ":engineering": team.engineering,
            ":reputacao": team.reputacao,
            ":morale": team.morale,
            ":aerodinamica": team.aerodinamica,
            ":motor": team.motor,
            ":chassi": team.chassi,
            ":hierarquia_n1_id": &team.hierarquia_n1_id,
            ":hierarquia_n2_id": &team.hierarquia_n2_id,
            ":hierarquia_status": &team.hierarquia_status,
            ":hierarquia_tensao": team.hierarquia_tensao,
            ":hierarquia_duelos_total": team.hierarquia_duelos_total,
            ":hierarquia_duelos_n2_vencidos": team.hierarquia_duelos_n2_vencidos,
            ":hierarquia_sequencia_n2": team.hierarquia_sequencia_n2,
            ":hierarquia_sequencia_n1": team.hierarquia_sequencia_n1,
            ":hierarquia_inversoes_temporada": team.hierarquia_inversoes_temporada,
            ":parent_team_id": &team.parent_team_id,
            ":aceita_rookies": team.aceita_rookies as i64,
            ":meta_posicao": team.meta_posicao,
            ":stats_vitorias": team.stats_vitorias,
            ":stats_podios": team.stats_podios,
            ":stats_poles": team.stats_poles,
            ":stats_pontos": team.stats_pontos,
            ":stats_melhor_resultado": team.stats_melhor_resultado,
            ":temp_posicao": team.temp_posicao,
            ":historico_vitorias": team.historico_vitorias,
            ":historico_podios": team.historico_podios,
            ":historico_poles": team.historico_poles,
            ":historico_pontos": team.historico_pontos,
            ":historico_titulos_pilotos": team.historico_titulos_pilotos,
            ":carreira_titulos": team.historico_titulos_construtores,
            ":temporada_atual": team.temporada_atual,
            ":created_at": &team.created_at,
            ":updated_at": &team.updated_at,
            ":categoria_anterior": &team.categoria_anterior,
        },
    )?;
    Ok(())
}

pub fn insert_teams(conn: &Connection, teams: &[Team]) -> Result<(), DbError> {
    for team in teams {
        insert_team(conn, team)?;
    }
    Ok(())
}

/// Limpa `categoria_anterior` de todas as equipes. Chamado no início de cada
/// ciclo de promoção/rebaixamento para que o campo reflita apenas os movimentos
/// da temporada atual — caso contrário o badge de movimento na pré-temporada (e
/// os ajustes de car build / pit) acumulariam temporadas passadas.
pub fn clear_all_categoria_anterior(conn: &Connection) -> Result<(), DbError> {
    conn.execute("UPDATE teams SET categoria_anterior = NULL", [])?;
    Ok(())
}

pub fn update_team(conn: &Connection, team: &Team) -> Result<(), DbError> {
    let mut persisted_team = team.clone();
    sync_legacy_budget_index(&mut persisted_team);
    let team = &persisted_team;

    let affected = conn.execute(
        "UPDATE teams SET
            nome = :nome,
            nome_curto = :nome_curto,
            cor_primaria = :cor_primaria,
            cor_secundaria = :cor_secundaria,
            pais_sede = :pais_sede,
            ano_fundacao = :ano_fundacao,
            categoria = :categoria,
            ativa = :ativa,
            marca = :marca,
            classe = :classe,
            piloto_1_id = :piloto_1_id,
            piloto_2_id = :piloto_2_id,
            is_player_team = :is_player_team,
            car_performance = :car_performance,
            confiabilidade = :confiabilidade,
            pit_strategy_risk = :pit_strategy_risk,
            pit_crew_quality = :pit_crew_quality,
            budget = :budget,
            cash_balance = :cash_balance,
            debt_balance = :debt_balance,
            financial_state = :financial_state,
            season_strategy = :season_strategy,
            last_round_income = :last_round_income,
            last_round_expenses = :last_round_expenses,
            last_round_net = :last_round_net,
            parachute_payment_remaining = :parachute_payment_remaining,
            socorros_na_temporada = :socorros_na_temporada,
            socorros_temporada_ref = :socorros_temporada_ref,
            facilities = :facilities,
            engineering = :engineering,
            reputacao = :reputacao,
            morale = :morale,
            aerodinamica = :aerodinamica,
            motor = :motor,
            chassi = :chassi,
            hierarquia_n1_id = :hierarquia_n1_id,
            hierarquia_n2_id = :hierarquia_n2_id,
            hierarquia_status = :hierarquia_status,
            hierarquia_tensao = :hierarquia_tensao,
            hierarquia_duelos_total = :hierarquia_duelos_total,
            hierarquia_duelos_n2_vencidos = :hierarquia_duelos_n2_vencidos,
            hierarquia_sequencia_n2 = :hierarquia_sequencia_n2,
            hierarquia_sequencia_n1 = :hierarquia_sequencia_n1,
            hierarquia_inversoes_temporada = :hierarquia_inversoes_temporada,
            parent_team_id = :parent_team_id,
            aceita_rookies = :aceita_rookies,
            meta_posicao = :meta_posicao,
            stats_vitorias = :stats_vitorias,
            stats_podios = :stats_podios,
            stats_poles = :stats_poles,
            stats_pontos = :stats_pontos,
            stats_melhor_resultado = :stats_melhor_resultado,
            temp_posicao = :temp_posicao,
            historico_vitorias = :historico_vitorias,
            historico_podios = :historico_podios,
            historico_poles = :historico_poles,
            historico_pontos = :historico_pontos,
            historico_titulos_pilotos = :historico_titulos_pilotos,
            carreira_titulos = :carreira_titulos,
            temporada_atual = :temporada_atual,
            updated_at = :updated_at,
            categoria_anterior = :categoria_anterior
        WHERE id = :id",
        rusqlite::named_params! {
            ":id": &team.id,
            ":nome": &team.nome,
            ":nome_curto": &team.nome_curto,
            ":cor_primaria": &team.cor_primaria,
            ":cor_secundaria": &team.cor_secundaria,
            ":pais_sede": &team.pais_sede,
            ":ano_fundacao": team.ano_fundacao,
            ":categoria": &team.categoria,
            ":ativa": team.ativa as i64,
            ":marca": &team.marca,
            ":classe": &team.classe,
            ":piloto_1_id": &team.piloto_1_id,
            ":piloto_2_id": &team.piloto_2_id,
            ":is_player_team": team.is_player_team as i64,
            ":car_performance": team.car_performance,
            ":confiabilidade": team.confiabilidade,
            ":pit_strategy_risk": team.pit_strategy_risk,
            ":pit_crew_quality": team.pit_crew_quality,
            ":budget": team.budget,
            ":cash_balance": team.cash_balance,
            ":debt_balance": team.debt_balance,
            ":financial_state": &team.financial_state,
            ":season_strategy": &team.season_strategy,
            ":last_round_income": team.last_round_income,
            ":last_round_expenses": team.last_round_expenses,
            ":last_round_net": team.last_round_net,
            ":parachute_payment_remaining": team.parachute_payment_remaining,
            ":socorros_na_temporada": team.socorros_na_temporada,
            ":socorros_temporada_ref": team.socorros_temporada_ref,
            ":facilities": team.facilities,
            ":engineering": team.engineering,
            ":reputacao": team.reputacao,
            ":morale": team.morale,
            ":aerodinamica": team.aerodinamica,
            ":motor": team.motor,
            ":chassi": team.chassi,
            ":hierarquia_n1_id": &team.hierarquia_n1_id,
            ":hierarquia_n2_id": &team.hierarquia_n2_id,
            ":hierarquia_status": &team.hierarquia_status,
            ":hierarquia_tensao": team.hierarquia_tensao,
            ":hierarquia_duelos_total": team.hierarquia_duelos_total,
            ":hierarquia_duelos_n2_vencidos": team.hierarquia_duelos_n2_vencidos,
            ":hierarquia_sequencia_n2": team.hierarquia_sequencia_n2,
            ":hierarquia_sequencia_n1": team.hierarquia_sequencia_n1,
            ":hierarquia_inversoes_temporada": team.hierarquia_inversoes_temporada,
            ":parent_team_id": &team.parent_team_id,
            ":aceita_rookies": team.aceita_rookies as i64,
            ":meta_posicao": team.meta_posicao,
            ":stats_vitorias": team.stats_vitorias,
            ":stats_podios": team.stats_podios,
            ":stats_poles": team.stats_poles,
            ":stats_pontos": team.stats_pontos,
            ":stats_melhor_resultado": team.stats_melhor_resultado,
            ":temp_posicao": team.temp_posicao,
            ":historico_vitorias": team.historico_vitorias,
            ":historico_podios": team.historico_podios,
            ":historico_poles": team.historico_poles,
            ":historico_pontos": team.historico_pontos,
            ":historico_titulos_pilotos": team.historico_titulos_pilotos,
            ":carreira_titulos": team.historico_titulos_construtores,
            ":temporada_atual": team.temporada_atual,
            ":updated_at": &team.updated_at,
            ":categoria_anterior": &team.categoria_anterior,
        },
    )?;
    ensure_team_rows_affected(affected, &team.id, "atualizar equipe")?;
    Ok(())
}

pub fn update_team_season_stats(
    conn: &Connection,
    team_id: &str,
    vitorias: i32,
    podios: i32,
    poles: i32,
    pontos: i32,
    melhor_resultado: i32,
) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE teams
         SET stats_vitorias = ?1,
             stats_podios = ?2,
             stats_poles = ?3,
             stats_pontos = ?4,
             stats_melhor_resultado = ?5
         WHERE id = ?6",
        params![vitorias, podios, poles, pontos, melhor_resultado, team_id],
    )?;
    ensure_team_rows_affected(affected, team_id, "atualizar estatisticas da equipe")?;
    Ok(())
}

pub fn reset_team_season_stats(conn: &Connection, team_id: &str) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE teams
         SET stats_vitorias = 0,
             stats_podios = 0,
             stats_poles = 0,
             stats_pontos = 0,
             stats_melhor_resultado = 99,
             temp_posicao = 0
         WHERE id = ?1",
        params![team_id],
    )?;
    ensure_team_rows_affected(affected, team_id, "resetar estatisticas sazonais da equipe")?;
    Ok(())
}

pub fn update_team_morale(conn: &Connection, team_id: &str, morale: f64) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE teams SET morale = ?1 WHERE id = ?2",
        params![morale, team_id],
    )?;
    ensure_team_rows_affected(affected, team_id, "atualizar moral da equipe")?;
    Ok(())
}

pub fn delete_team(conn: &Connection, id: &str) -> Result<(), DbError> {
    let affected = conn.execute("DELETE FROM teams WHERE id = ?1", params![id])?;
    ensure_team_rows_affected(affected, id, "remover equipe")?;
    Ok(())
}
