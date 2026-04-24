#![allow(dead_code)]

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;
use crate::finance::planning::sync_legacy_budget_index;
use crate::models::team::{Team, TeamHierarchyClimate};
use crate::simulation::car_build::CarBuildProfile;

pub fn insert_team(conn: &Connection, team: &Team) -> Result<(), DbError> {
    let mut persisted_team = team.clone();
    sync_legacy_budget_index(&mut persisted_team);
    let team = &persisted_team;

    conn.execute(
        "INSERT INTO teams (
            id, nome, nome_curto, cor_primaria, cor_secundaria, pais_sede,
            ano_fundacao, categoria, ativa, marca, classe, piloto_1_id, piloto_2_id,
            is_player_team, car_performance, car_build_profile, confiabilidade, pit_strategy_risk,
            pit_crew_quality, budget, cash_balance, debt_balance, financial_state,
            season_strategy, last_round_income, last_round_expenses, last_round_net,
            parachute_payment_remaining, facilities,
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
            :is_player_team, :car_performance, :car_build_profile, :confiabilidade, :pit_strategy_risk,
            :pit_crew_quality, :budget, :cash_balance, :debt_balance, :financial_state,
            :season_strategy, :last_round_income, :last_round_expenses, :last_round_net,
            :parachute_payment_remaining, :facilities,
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
            ":car_build_profile": team.car_build_profile.as_str(),
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

pub fn get_team_by_id(conn: &Connection, id: &str) -> Result<Option<Team>, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM teams WHERE id = ?1")?;
    let team = stmt.query_row(params![id], team_from_row).optional()?;
    Ok(team)
}

pub fn get_all_teams(conn: &Connection) -> Result<Vec<Team>, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM teams ORDER BY nome")?;
    let mapped = stmt.query_map([], team_from_row)?;
    let teams = collect_teams(mapped)?;
    Ok(teams)
}

pub fn get_teams_by_category(conn: &Connection, category_id: &str) -> Result<Vec<Team>, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM teams WHERE categoria = ?1 ORDER BY nome")?;
    let mapped = stmt.query_map(params![category_id], team_from_row)?;
    let teams = collect_teams(mapped)?;
    Ok(teams)
}

/// Equipes de uma categoria filtradas por classe, ordenadas por desempenho desc.
/// Usado na convocação especial para montar o grid classe a classe.
pub fn get_teams_by_category_and_class(
    conn: &Connection,
    categoria: &str,
    classe: &str,
) -> Result<Vec<crate::models::team::Team>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM teams WHERE categoria = ?1 AND classe = ?2 ORDER BY car_performance DESC",
    )?;
    let mapped = stmt.query_map(params![categoria, classe], team_from_row)?;
    collect_teams(mapped)
}

/// Limpa `piloto_1_id` e `piloto_2_id` de todas as equipes especiais.
/// Afeta production_challenger (mazda/toyota/bmw) e endurance (gt4/gt3/lmp2).
/// Equipes LMP2 nunca recebem lineup neste redesign inicial, portanto a operação
/// sobre elas é inócua — o WHERE não as exclui explicitamente para manter a
/// semântica de "limpar tudo das categorias especiais".
pub fn clear_special_team_lineups(conn: &Connection) -> Result<usize, DbError> {
    let n = conn.execute(
        "UPDATE teams SET piloto_1_id = NULL, piloto_2_id = NULL
         WHERE categoria IN ('production_challenger', 'endurance')",
        [],
    )?;
    Ok(n)
}

/// Reseta todos os campos de hierarquia das equipes especiais.
/// Mesma nota de LMP2: afeta toda a categoria endurance, mas LMP2 está sempre
/// sem lineup, então o reset é inócuo para essas equipes.
pub fn reset_special_team_hierarchies(conn: &Connection) -> Result<(), DbError> {
    conn.execute(
        "UPDATE teams SET
            hierarquia_n1_id = NULL, hierarquia_n2_id = NULL,
            hierarquia_status = 'estavel', hierarquia_tensao = 0.0,
            hierarquia_duelos_total = 0, hierarquia_duelos_n2_vencidos = 0,
            hierarquia_sequencia_n2 = 0, hierarquia_sequencia_n1 = 0,
            hierarquia_inversoes_temporada = 0
         WHERE categoria IN ('production_challenger', 'endurance')",
        [],
    )?;
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
            car_build_profile = :car_build_profile,
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
            ":car_build_profile": team.car_build_profile.as_str(),
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

pub fn update_team_pilots(
    conn: &Connection,
    team_id: &str,
    piloto_1_id: Option<&str>,
    piloto_2_id: Option<&str>,
) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE teams SET piloto_1_id = ?1, piloto_2_id = ?2 WHERE id = ?3",
        params![piloto_1_id, piloto_2_id, team_id],
    )?;
    ensure_team_rows_affected(affected, team_id, "atualizar pilotos da equipe")?;
    Ok(())
}

pub fn update_team_hierarchy(
    conn: &Connection,
    team_id: &str,
    n1_id: Option<&str>,
    n2_id: Option<&str>,
    status: &str,
    tensao: f64,
) -> Result<(), DbError> {
    let normalized = TeamHierarchyClimate::from_str_strict(status)
        .map_err(DbError::InvalidData)?
        .as_str()
        .to_string();
    let affected = conn.execute(
        "UPDATE teams
         SET hierarquia_n1_id = ?1,
             hierarquia_n2_id = ?2,
             hierarquia_status = ?3,
             hierarquia_tensao = ?4
         WHERE id = ?5",
        params![n1_id, n2_id, normalized, tensao, team_id],
    )?;
    ensure_team_rows_affected(affected, team_id, "atualizar hierarquia da equipe")?;
    Ok(())
}

/// Persiste todos os 9 campos da hierarquia interna de uma equipe de uma vez.
/// Use este após processar o sistema de hierarquia pós-corrida.
pub fn update_team_hierarchy_full(conn: &Connection, team: &Team) -> Result<(), DbError> {
    TeamHierarchyClimate::from_str_strict(&team.hierarquia_status).map_err(DbError::InvalidData)?;
    let affected = conn.execute(
        "UPDATE teams
         SET hierarquia_n1_id = ?1,
             hierarquia_n2_id = ?2,
             hierarquia_status = ?3,
             hierarquia_tensao = ?4,
             hierarquia_duelos_total = ?5,
             hierarquia_duelos_n2_vencidos = ?6,
             hierarquia_sequencia_n2 = ?7,
             hierarquia_sequencia_n1 = ?8,
             hierarquia_inversoes_temporada = ?9
         WHERE id = ?10",
        rusqlite::params![
            &team.hierarquia_n1_id,
            &team.hierarquia_n2_id,
            &team.hierarquia_status,
            team.hierarquia_tensao,
            team.hierarquia_duelos_total,
            team.hierarquia_duelos_n2_vencidos,
            team.hierarquia_sequencia_n2,
            team.hierarquia_sequencia_n1,
            team.hierarquia_inversoes_temporada,
            &team.id,
        ],
    )?;
    ensure_team_rows_affected(
        affected,
        &team.id,
        "atualizar hierarquia completa da equipe",
    )?;
    Ok(())
}

pub fn update_team_duel_counters(
    conn: &Connection,
    team_id: &str,
    duelos_total: i32,
    duelos_n2_vencidos: i32,
    sequencia_n2: i32,
    sequencia_n1: i32,
    inversoes_temporada: i32,
) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE teams
         SET hierarquia_duelos_total = ?1,
             hierarquia_duelos_n2_vencidos = ?2,
             hierarquia_sequencia_n2 = ?3,
             hierarquia_sequencia_n1 = ?4,
             hierarquia_inversoes_temporada = ?5
         WHERE id = ?6",
        params![
            duelos_total,
            duelos_n2_vencidos,
            sequencia_n2,
            sequencia_n1,
            inversoes_temporada,
            team_id
        ],
    )?;
    ensure_team_rows_affected(affected, team_id, "atualizar contadores de duelo da equipe")?;
    Ok(())
}

pub fn update_team_finance_snapshot(conn: &Connection, team: &Team) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE teams
         SET cash_balance = ?1,
             debt_balance = ?2,
             financial_state = ?3,
             season_strategy = ?4,
             last_round_income = ?5,
             last_round_expenses = ?6,
             last_round_net = ?7,
             parachute_payment_remaining = ?8
         WHERE id = ?9",
        params![
            team.cash_balance,
            team.debt_balance,
            &team.financial_state,
            &team.season_strategy,
            team.last_round_income,
            team.last_round_expenses,
            team.last_round_net,
            team.parachute_payment_remaining,
            &team.id,
        ],
    )?;
    ensure_team_rows_affected(
        affected,
        &team.id,
        "atualizar snapshot financeiro da equipe",
    )?;
    Ok(())
}

pub fn remove_pilot_from_team(
    conn: &Connection,
    driver_id: &str,
    team_id: &str,
) -> Result<(), DbError> {
    let team = get_team_by_id(conn, team_id)?
        .ok_or_else(|| DbError::NotFound(format!("Equipe '{team_id}' nao encontrada")))?;
    let piloto_1 = if team.piloto_1_id.as_deref() == Some(driver_id) {
        None
    } else {
        team.piloto_1_id.as_deref()
    };
    let piloto_2 = if team.piloto_2_id.as_deref() == Some(driver_id) {
        None
    } else {
        team.piloto_2_id.as_deref()
    };
    let removed_from_hierarchy = team.hierarquia_n1_id.as_deref() == Some(driver_id)
        || team.hierarquia_n2_id.as_deref() == Some(driver_id);
    update_team_pilots(conn, team_id, piloto_1, piloto_2)?;
    if removed_from_hierarchy {
        update_team_hierarchy(
            conn,
            team_id,
            None,
            None,
            TeamHierarchyClimate::Estavel.as_str(),
            0.0,
        )?;
        update_team_duel_counters(conn, team_id, 0, 0, 0, 0, 0)?;
    }
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

pub fn count_teams_by_category(conn: &Connection, category_id: &str) -> Result<i32, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM teams WHERE categoria = ?1",
        params![category_id],
        |row| row.get(0),
    )?;
    Ok(count as i32)
}

fn collect_teams(
    mapped: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Team>>,
) -> Result<Vec<Team>, DbError> {
    let mut result = Vec::new();
    for row in mapped {
        result.push(row?);
    }
    Ok(result)
}

fn team_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Team> {
    let car_build_profile_value: String = row.get("car_build_profile")?;
    let car_build_profile =
        CarBuildProfile::from_str_strict(&car_build_profile_value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
            )
        })?;

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
        car_build_profile,
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

fn ensure_team_rows_affected(
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

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::constants::teams::get_team_templates;
    use crate::models::team::Team;

    #[test]
    fn test_insert_and_get_team() {
        let conn = setup_test_db().expect("test db");
        let team = sample_team("gt3", "T001");

        insert_team(&conn, &team).expect("insert team");
        let loaded = get_team_by_id(&conn, "T001")
            .expect("get team")
            .expect("team should exist");

        assert_eq!(loaded.id, "T001");
        assert_eq!(loaded.nome, team.nome);
        assert_eq!(loaded.categoria, "gt3");
        assert_eq!(loaded.stats_vitorias, 0);
    }

    #[test]
    fn test_insert_and_get_team_persists_extended_fields() {
        let conn = setup_test_db().expect("test db");
        let mut team = sample_team("gt3", "T010");
        team.piloto_1_id = Some("P001".to_string());
        team.piloto_2_id = Some("P002".to_string());
        team.cash_balance = 2_450_000.0;
        team.debt_balance = 325_000.0;
        team.financial_state = "healthy".to_string();
        team.season_strategy = "balanced".to_string();
        team.last_round_income = 180_000.0;
        team.last_round_expenses = 152_500.0;
        team.last_round_net = 27_500.0;
        team.parachute_payment_remaining = 500_000.0;
        team.hierarquia_n1_id = Some("P001".to_string());
        team.hierarquia_n2_id = Some("P002".to_string());
        team.hierarquia_tensao = 33.0;
        team.stats_podios = 4;
        team.stats_poles = 2;
        team.stats_pontos = 87;
        team.stats_melhor_resultado = 1;
        team.historico_podios = 12;
        team.historico_poles = 5;
        team.historico_pontos = 230;
        team.historico_titulos_pilotos = 1;

        insert_team(&conn, &team).expect("insert team");
        update_team_pilots(&conn, "T010", Some("P001"), Some("P002")).expect("update pilots");
        update_team_hierarchy(
            &conn,
            "T010",
            Some("P001"),
            Some("P002"),
            "competitivo",
            33.0,
        )
        .expect("update hierarchy");
        update_team_season_stats(&conn, "T010", 3, 4, 2, 87, 1).expect("update season stats");

        let loaded = get_team_by_id(&conn, "T010")
            .expect("get team")
            .expect("team should exist");

        assert_eq!(loaded.nome_curto, team.nome_curto);
        assert_eq!(loaded.cor_primaria, team.cor_primaria);
        assert_eq!(loaded.cor_secundaria, team.cor_secundaria);
        assert_eq!(loaded.pais_sede, team.pais_sede);
        assert_eq!(loaded.piloto_1_id.as_deref(), Some("P001"));
        assert_eq!(loaded.piloto_2_id.as_deref(), Some("P002"));
        assert_eq!(loaded.car_build_profile, team.car_build_profile);
        assert_eq!(loaded.pit_strategy_risk, team.pit_strategy_risk);
        assert_eq!(loaded.pit_crew_quality, team.pit_crew_quality);
        assert_eq!(loaded.cash_balance, team.cash_balance);
        assert_eq!(loaded.debt_balance, team.debt_balance);
        assert_eq!(loaded.financial_state, team.financial_state);
        assert_eq!(loaded.season_strategy, team.season_strategy);
        assert_eq!(loaded.last_round_income, team.last_round_income);
        assert_eq!(loaded.last_round_expenses, team.last_round_expenses);
        assert_eq!(loaded.last_round_net, team.last_round_net);
        assert_eq!(
            loaded.parachute_payment_remaining,
            team.parachute_payment_remaining
        );
        assert_eq!(loaded.hierarquia_n1_id.as_deref(), Some("P001"));
        assert_eq!(loaded.hierarquia_n2_id.as_deref(), Some("P002"));
        assert_eq!(loaded.hierarquia_status, "competitivo");
        assert_eq!(loaded.hierarquia_tensao, 33.0);
        assert_eq!(loaded.stats_podios, 4);
        assert_eq!(loaded.stats_poles, 2);
        assert_eq!(loaded.stats_pontos, 87);
        assert_eq!(loaded.stats_melhor_resultado, 1);
    }

    #[test]
    fn test_insert_team_syncs_legacy_budget_from_money() {
        let conn = setup_test_db().expect("test db");
        let mut team = sample_team("gt4", "T020");
        team.cash_balance = 6_000_000.0;
        team.debt_balance = 0.0;
        team.financial_state = "healthy".to_string();
        team.budget = 1.0;

        insert_team(&conn, &team).expect("insert team");
        let loaded = get_team_by_id(&conn, "T020")
            .expect("get team")
            .expect("team should exist");

        let expected_budget = crate::finance::planning::derive_budget_index_from_money(&loaded);
        assert!((loaded.budget - expected_budget).abs() < 0.0001);
        assert!(loaded.budget > 1.0);
    }

    #[test]
    fn test_insert_and_get_team_uses_current_team_schema_without_legacy_columns() {
        let conn = setup_test_db().expect("test db");
        assert!(test_column_exists(&conn, "teams", "confiabilidade"));
        assert!(test_column_exists(&conn, "teams", "reputacao"));
        assert!(!test_column_exists(&conn, "teams", "reliability"));
        assert!(!test_column_exists(&conn, "teams", "prestige"));
        assert!(!test_column_exists(&conn, "teams", "temp_pontos"));
        assert!(!test_column_exists(&conn, "teams", "temp_vitorias"));
        assert!(!test_column_exists(&conn, "teams", "carreira_vitorias"));

        let mut team = sample_team("gt4", "T_SCHEMA");
        team.confiabilidade = 71.0;
        team.reputacao = 63.0;
        team.stats_vitorias = 4;
        team.stats_pontos = 98;
        team.historico_vitorias = 12;

        insert_team(&conn, &team).expect("insert team with current schema");
        let loaded = get_team_by_id(&conn, "T_SCHEMA")
            .expect("get team")
            .expect("team should exist");

        assert_eq!(loaded.confiabilidade, 71.0);
        assert_eq!(loaded.reputacao, 63.0);
        assert_eq!(loaded.stats_vitorias, 4);
        assert_eq!(loaded.stats_pontos, 98);
        assert_eq!(loaded.historico_vitorias, 12);
    }

    #[test]
    fn test_get_teams_by_category() {
        let conn = setup_test_db().expect("test db");
        insert_team(&conn, &sample_team("gt3", "T001")).expect("insert team 1");
        insert_team(&conn, &sample_team("gt3", "T002")).expect("insert team 2");
        insert_team(&conn, &sample_team("gt4", "T003")).expect("insert team 3");

        let gt3_teams = get_teams_by_category(&conn, "gt3").expect("query teams");

        assert_eq!(gt3_teams.len(), 2);
        assert!(gt3_teams.iter().all(|team| team.categoria == "gt3"));
    }

    #[test]
    fn test_update_team_pilots() {
        let conn = setup_test_db().expect("test db");
        insert_team(&conn, &sample_team("gt3", "T001")).expect("insert team");

        update_team_pilots(&conn, "T001", Some("P001"), Some("P002")).expect("update pilots");
        let loaded = get_team_by_id(&conn, "T001")
            .expect("get team")
            .expect("team should exist");

        assert_eq!(loaded.piloto_1_id.as_deref(), Some("P001"));
        assert_eq!(loaded.piloto_2_id.as_deref(), Some("P002"));
    }

    #[test]
    fn test_count_teams_by_category() {
        let conn = setup_test_db().expect("test db");
        insert_team(&conn, &sample_team("gt3", "T001")).expect("insert team 1");
        insert_team(&conn, &sample_team("gt3", "T002")).expect("insert team 2");
        insert_team(&conn, &sample_team("endurance", "T003")).expect("insert team 3");

        let count = count_teams_by_category(&conn, "gt3").expect("count teams");

        assert_eq!(count, 2);
    }

    #[test]
    fn test_remove_pilot_from_team_clears_matching_slot() {
        let conn = setup_test_db().expect("test db");
        let mut team = sample_team("gt3", "T001");
        team.piloto_1_id = Some("P001".to_string());
        team.piloto_2_id = Some("P002".to_string());
        insert_team(&conn, &team).expect("insert team");

        remove_pilot_from_team(&conn, "P002", "T001").expect("remove pilot");

        let refreshed = get_team_by_id(&conn, "T001")
            .expect("team query")
            .expect("team");
        assert_eq!(refreshed.piloto_1_id.as_deref(), Some("P001"));
        assert!(refreshed.piloto_2_id.is_none());
    }

    #[test]
    fn test_blob_in_required_text_field_returns_error() {
        let conn = setup_test_db().expect("test db");
        conn.execute(
            "INSERT INTO teams (id, nome, nome_curto, cor_primaria, categoria, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                "T_BLOB_TEXT",
                "Blob Team",
                "Blob",
                rusqlite::types::Value::Blob(vec![0xDE, 0xAD, 0xBE, 0xEF]),
                "gt3",
                "2026-01-01",
                "2026-01-01",
            ],
        )
        .expect("insert blob team");

        let result = get_team_by_id(&conn, "T_BLOB_TEXT");
        assert!(
            result.is_err(),
            "BLOB em campo obrigatorio TEXT deve retornar erro"
        );
    }

    #[test]
    fn test_blob_in_required_real_field_returns_error() {
        let conn = setup_test_db().expect("test db");
        conn.execute(
            "INSERT INTO teams (
                id, nome, nome_curto, categoria, hierarquia_tensao, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                "T_BLOB_REAL",
                "Blob Team",
                "Blob",
                "gt3",
                rusqlite::types::Value::Blob(vec![0xBA, 0xAD, 0xF0, 0x0D]),
                "2026-01-01",
                "2026-01-01",
            ],
        )
        .expect("insert blob hierarchy");

        let result = get_team_by_id(&conn, "T_BLOB_REAL");
        assert!(
            result.is_err(),
            "BLOB em campo obrigatorio REAL deve retornar erro"
        );
    }

    #[test]
    fn test_update_team_pilots_returns_not_found_for_missing_team() {
        let conn = setup_test_db().expect("test db");

        let error = update_team_pilots(&conn, "T404", Some("P001"), Some("P002"))
            .expect_err("missing team should fail");

        assert!(matches!(error, DbError::NotFound(_)));
    }

    #[test]
    fn test_remove_pilot_from_team_resets_hierarchy_when_removed_pilot_was_ranked() {
        let conn = setup_test_db().expect("test db");
        let mut team = sample_team("gt3", "T777");
        team.piloto_1_id = Some("P001".to_string());
        team.piloto_2_id = Some("P002".to_string());
        team.hierarquia_n1_id = Some("P001".to_string());
        team.hierarquia_n2_id = Some("P002".to_string());
        team.hierarquia_status = "competitivo".to_string();
        team.hierarquia_tensao = 55.0;
        team.hierarquia_duelos_total = 4;
        team.hierarquia_duelos_n2_vencidos = 2;
        team.hierarquia_sequencia_n2 = 1;
        team.hierarquia_sequencia_n1 = 2;
        team.hierarquia_inversoes_temporada = 1;
        insert_team(&conn, &team).expect("insert team");

        remove_pilot_from_team(&conn, "P001", "T777").expect("remove pilot");

        let refreshed = get_team_by_id(&conn, "T777")
            .expect("team query")
            .expect("team exists");
        assert!(refreshed.piloto_1_id.is_none());
        assert_eq!(refreshed.piloto_2_id.as_deref(), Some("P002"));
        assert!(refreshed.hierarquia_n1_id.is_none());
        assert!(refreshed.hierarquia_n2_id.is_none());
        assert_eq!(refreshed.hierarquia_status, "estavel");
        assert_eq!(refreshed.hierarquia_tensao, 0.0);
        assert_eq!(refreshed.hierarquia_duelos_total, 0);
        assert_eq!(refreshed.hierarquia_duelos_n2_vencidos, 0);
        assert_eq!(refreshed.hierarquia_sequencia_n2, 0);
        assert_eq!(refreshed.hierarquia_sequencia_n1, 0);
        assert_eq!(refreshed.hierarquia_inversoes_temporada, 0);
    }

    #[test]
    fn test_invalid_hierarchy_status_from_db_returns_error() {
        let conn = setup_test_db().expect("test db");
        conn.execute(
            "INSERT INTO teams (id, nome, nome_curto, categoria, hierarquia_status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                "T_BAD_HIER",
                "Bad Team",
                "BAD",
                "gt3",
                "alienigena",
                "2026-01-01",
                "2026-01-01",
            ],
        )
        .expect("insert invalid hierarchy team");

        let result = get_team_by_id(&conn, "T_BAD_HIER");
        assert!(
            result.is_err(),
            "hierarquia_status invalido deve retornar erro, nao cair em estavel"
        );
    }

    #[test]
    fn test_invalid_meta_posicao_from_db_returns_error() {
        let conn = setup_test_db().expect("test db");
        conn.execute(
            "INSERT INTO teams (id, nome, nome_curto, categoria, meta_posicao, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                "T_BAD_META",
                "Bad Meta Team",
                "BMT",
                "gt3",
                "abc",
                "2026-01-01",
                "2026-01-01",
            ],
        )
        .expect("insert invalid meta_posicao team");

        let result = get_team_by_id(&conn, "T_BAD_META");
        assert!(
            result.is_err(),
            "meta_posicao invalida deve retornar erro, nao cair em default silencioso"
        );
    }

    #[test]
    fn test_raw_legacy_team_row_without_current_schema_returns_error() {
        let conn = Connection::open_in_memory().expect("legacy db");
        conn.execute_batch(
            "CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT ''
            );
            INSERT INTO teams (id, nome, categoria, created_at)
            VALUES ('T_OLD', 'Equipe Legada', 'gt3', '2026-01-01');",
        )
        .expect("legacy schema");

        let result = get_team_by_id(&conn, "T_OLD");
        assert!(
            result.is_err(),
            "raw legacy teams schema must be migrated before query mapping"
        );
    }

    fn sample_team(category_id: &str, team_id: &str) -> Team {
        let template = get_team_templates(category_id)[0];
        let mut rng = StdRng::seed_from_u64(55);
        Team::from_template_with_rng(template, category_id, team_id.to_string(), 2026, &mut rng)
    }

    fn setup_test_db() -> Result<Connection, DbError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                nome_curto TEXT NOT NULL,
                cor_primaria TEXT NOT NULL DEFAULT '#FFFFFF',
                cor_secundaria TEXT NOT NULL DEFAULT '#000000',
                pais_sede TEXT NOT NULL DEFAULT 'Unknown',
                ano_fundacao INTEGER NOT NULL DEFAULT 2024,
                categoria TEXT NOT NULL,
                ativa INTEGER NOT NULL DEFAULT 1,
                marca TEXT,
                classe TEXT,
                piloto_1_id TEXT,
                piloto_2_id TEXT,
                is_player_team INTEGER NOT NULL DEFAULT 0,
                car_performance REAL NOT NULL DEFAULT 0.0,
                car_build_profile TEXT NOT NULL DEFAULT 'balanced',
                confiabilidade REAL NOT NULL DEFAULT 60.0,
                pit_strategy_risk REAL NOT NULL DEFAULT 50.0,
                pit_crew_quality REAL NOT NULL DEFAULT 50.0,
                budget REAL NOT NULL DEFAULT 50.0,
                cash_balance REAL NOT NULL DEFAULT 0.0,
                debt_balance REAL NOT NULL DEFAULT 0.0,
                financial_state TEXT NOT NULL DEFAULT 'stable',
                season_strategy TEXT NOT NULL DEFAULT 'balanced',
                last_round_income REAL NOT NULL DEFAULT 0.0,
                last_round_expenses REAL NOT NULL DEFAULT 0.0,
                last_round_net REAL NOT NULL DEFAULT 0.0,
                parachute_payment_remaining REAL NOT NULL DEFAULT 0.0,
                facilities REAL NOT NULL DEFAULT 50.0,
                engineering REAL NOT NULL DEFAULT 50.0,
                reputacao REAL NOT NULL DEFAULT 50.0,
                morale REAL NOT NULL DEFAULT 1.0,
                aerodinamica REAL NOT NULL DEFAULT 50.0,
                motor REAL NOT NULL DEFAULT 50.0,
                chassi REAL NOT NULL DEFAULT 50.0,
                hierarquia_n1_id TEXT,
                hierarquia_n2_id TEXT,
                hierarquia_status TEXT NOT NULL DEFAULT 'estavel',
                hierarquia_tensao REAL NOT NULL DEFAULT 0.0,
                hierarquia_duelos_total INTEGER NOT NULL DEFAULT 0,
                hierarquia_duelos_n2_vencidos INTEGER NOT NULL DEFAULT 0,
                hierarquia_sequencia_n2 INTEGER NOT NULL DEFAULT 0,
                hierarquia_sequencia_n1 INTEGER NOT NULL DEFAULT 0,
                hierarquia_inversoes_temporada INTEGER NOT NULL DEFAULT 0,
                parent_team_id TEXT,
                aceita_rookies INTEGER NOT NULL DEFAULT 1,
                meta_posicao INTEGER NOT NULL DEFAULT 10,
                stats_vitorias INTEGER NOT NULL DEFAULT 0,
                stats_podios INTEGER NOT NULL DEFAULT 0,
                stats_poles INTEGER NOT NULL DEFAULT 0,
                stats_pontos INTEGER NOT NULL DEFAULT 0,
                stats_melhor_resultado INTEGER NOT NULL DEFAULT 99,
                temp_posicao INTEGER NOT NULL DEFAULT 0,
                historico_vitorias INTEGER NOT NULL DEFAULT 0,
                historico_podios INTEGER NOT NULL DEFAULT 0,
                historico_poles INTEGER NOT NULL DEFAULT 0,
                historico_pontos INTEGER NOT NULL DEFAULT 0,
                historico_titulos_pilotos INTEGER NOT NULL DEFAULT 0,
                carreira_titulos INTEGER NOT NULL DEFAULT 0,
                temporada_atual INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT '',
                categoria_anterior TEXT
            );",
        )?;
        Ok(conn)
    }

    fn test_column_exists(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({})", table))
            .expect("pragma table_info");
        let mut rows = stmt.query([]).expect("query pragma");

        while let Some(row) = rows.next().expect("next row") {
            let name: String = row.get("name").expect("column name");
            if name == column {
                return true;
            }
        }

        false
    }
}
