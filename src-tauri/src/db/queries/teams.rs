#![allow(dead_code)]

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;
use crate::finance::cashflow::{RoundCashflowSummary, TeamRoundFinanceContext};
use crate::finance::planning::sync_legacy_budget_index;
use crate::models::team::{Team, TeamHierarchyClimate};

/// Fama (midia) dos pilotos do lineup REGULAR ativo de uma equipe. Base da presença
/// pública do time → patrocínio ([[fama → dinheiro]]). Vazio se o time não tem lineup.
pub fn get_team_lineup_medias(conn: &Connection, team_id: &str) -> Result<Vec<f64>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT d.midia FROM drivers d
         JOIN contracts c ON c.piloto_id = d.id
         WHERE c.equipe_id = ?1 AND c.status = 'Ativo' AND c.tipo = 'Regular'",
    )?;
    let medias = stmt
        .query_map(params![team_id], |row| row.get::<_, f64>(0))?
        .collect::<Result<Vec<f64>, _>>()?;
    Ok(medias)
}

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
            :is_player_team, :car_performance, :confiabilidade, :pit_strategy_risk,
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
    let mut team = stmt.query_row(params![id], team_from_row).optional()?;
    if let Some(team) = team.as_mut() {
        team.car = crate::db::queries::team_car::get_team_car(conn, &team.id)?;
    }
    Ok(team)
}

/// Anexa o carro (tabela `team_car`) a cada time carregado — o Sistema de Nível do Carro.
/// Times sem carro persistido (saves antigos, pré-seed) ficam com `car: None` (o sim cai
/// no fallback do `car_performance` escalar).
fn attach_cars(conn: &Connection, teams: &mut [Team]) -> Result<(), DbError> {
    for team in teams.iter_mut() {
        team.car = crate::db::queries::team_car::get_team_car(conn, &team.id)?;
    }
    Ok(())
}

pub fn get_all_teams(conn: &Connection) -> Result<Vec<Team>, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM teams ORDER BY nome")?;
    let mapped = stmt.query_map([], team_from_row)?;
    let mut teams = collect_teams(mapped)?;
    attach_cars(conn, &mut teams)?;
    Ok(teams)
}

/// Limpa `categoria_anterior` de todas as equipes. Chamado no início de cada
/// ciclo de promoção/rebaixamento para que o campo reflita apenas os movimentos
/// da temporada atual — caso contrário o badge de movimento na pré-temporada (e
/// os ajustes de car build / pit) acumulariam temporadas passadas.
pub fn clear_all_categoria_anterior(conn: &Connection) -> Result<(), DbError> {
    conn.execute("UPDATE teams SET categoria_anterior = NULL", [])?;
    Ok(())
}

/// Plano estratégico de longo prazo (3 temporadas) da equipe (Pilar C).
/// Retorna (tipo, anos_restantes); default ("sustainable", 0) se não houver
/// registro — o que faz a próxima pré-temporada escolher um plano novo.
pub fn get_strategic_plan(conn: &Connection, team_id: &str) -> Result<(String, i32), DbError> {
    let row = conn
        .query_row(
            "SELECT plan_type, remaining_years FROM team_strategic_plan WHERE team_id = ?1",
            params![team_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
        )
        .optional()?;
    Ok(row.unwrap_or_else(|| ("sustainable".to_string(), 0)))
}

/// Grava o plano estratégico da equipe (upsert).
pub fn set_strategic_plan(
    conn: &Connection,
    team_id: &str,
    plan_type: &str,
    remaining_years: i32,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO team_strategic_plan (team_id, plan_type, remaining_years)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(team_id) DO UPDATE SET
            plan_type = excluded.plan_type,
            remaining_years = excluded.remaining_years",
        params![team_id, plan_type, remaining_years],
    )?;
    Ok(())
}

/// Zera o plano da equipe (remove o registro). Usado quando a equipe muda de
/// categoria (promoção/rebaixamento): a próxima pré-temporada re-avalia o plano
/// para a nova realidade. (Pilar C)
pub fn reset_strategic_plan(conn: &Connection, team_id: &str) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM team_strategic_plan WHERE team_id = ?1",
        params![team_id],
    )?;
    Ok(())
}

/// Temporadas consecutivas em colapso financeiro de uma equipe (0 se não houver
/// registro). Suporta o evento de venda/nova diretoria.
pub fn get_collapse_streak(conn: &Connection, team_id: &str) -> Result<i32, DbError> {
    let streak = conn
        .query_row(
            "SELECT streak FROM team_collapse_streak WHERE team_id = ?1",
            params![team_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(streak.unwrap_or(0))
}

/// Grava o contador de temporadas consecutivas em colapso de uma equipe.
pub fn set_collapse_streak(conn: &Connection, team_id: &str, streak: i32) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO team_collapse_streak (team_id, streak) VALUES (?1, ?2)
         ON CONFLICT(team_id) DO UPDATE SET streak = excluded.streak",
        params![team_id, streak],
    )?;
    Ok(())
}

/// Histórico de promoção de uma equipe: `(última temporada em que subiu, nº de promoções
/// na janela móvel)`. `(0, 0)` se não houver registro — nunca subiu (ou save antigo). Base
/// do retorno decrescente anti-snowball do chain-promotion (ver `promotion::effects`).
pub fn get_promotion_history(conn: &Connection, team_id: &str) -> Result<(i32, i32), DbError> {
    let row = conn
        .query_row(
            "SELECT last_promotion_season, recent_promotions
             FROM team_promotion_history WHERE team_id = ?1",
            params![team_id],
            |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)),
        )
        .optional()?;
    Ok(row.unwrap_or((0, 0)))
}

/// Grava o histórico de promoção de uma equipe (upsert). `last_promotion_season = 0` zera
/// a contagem (ex.: rebaixamento quebra a cadeia de promoções).
pub fn set_promotion_history(
    conn: &Connection,
    team_id: &str,
    last_promotion_season: i32,
    recent_promotions: i32,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO team_promotion_history (team_id, last_promotion_season, recent_promotions)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(team_id) DO UPDATE SET
            last_promotion_season = excluded.last_promotion_season,
            recent_promotions = excluded.recent_promotions",
        params![team_id, last_promotion_season, recent_promotions],
    )?;
    Ok(())
}

/// Incrementa um contador agregado de eventos de resgate (ex.: "sold",
/// "self_rescued"). Usado para estatística e telemetria.
pub fn incr_rescue_counter(conn: &Connection, key: &str) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO team_rescue_counters (key, value) VALUES (?1, 1)
         ON CONFLICT(key) DO UPDATE SET value = value + 1",
        params![key],
    )?;
    Ok(())
}

/// Lê um contador agregado de eventos de resgate (0 se ausente).
pub fn get_rescue_counter(conn: &Connection, key: &str) -> Result<i64, DbError> {
    let value = conn
        .query_row(
            "SELECT value FROM team_rescue_counters WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value.unwrap_or(0))
}

/// Registra um evento de propriedade/diretoria da equipe (ex.: venda por colapso
/// crônico). Exibido na ficha da equipe.
#[allow(clippy::too_many_arguments)]
pub fn insert_team_ownership_event(
    conn: &Connection,
    team_id: &str,
    season_number: i32,
    ano: i32,
    event_type: &str,
    debt_cleared: f64,
    cash_injected: f64,
    detail: &str,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO team_ownership_events
            (team_id, season_number, ano, event_type, debt_cleared, cash_injected, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            team_id,
            season_number,
            ano,
            event_type,
            debt_cleared,
            cash_injected,
            detail
        ],
    )?;
    Ok(())
}

/// Evento de mudança de dono/diretoria mais recente de uma equipe (ex.: venda após
/// colapso). Devolve `(event_type, season_number)` do último evento, se houver. Usado
/// no rodapé de notícias para a notinha "nova diretoria assumiu a equipe X".
pub fn get_latest_ownership_event(
    conn: &Connection,
    team_id: &str,
) -> Result<Option<(String, i32)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT event_type, season_number
         FROM team_ownership_events
         WHERE team_id = ?1
         ORDER BY season_number DESC, id DESC
         LIMIT 1",
    )?;
    let row = stmt
        .query_row(params![team_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    Ok(row)
}

/// Total de títulos de CONSTRUTORES de uma equipe na categoria (soma do arquivo).
pub fn get_team_category_constructor_titles(
    conn: &Connection,
    team_id: &str,
    categoria: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COALESCE(SUM(titulos_construtores), 0) FROM team_season_archive
         WHERE team_id = ?1 AND categoria = ?2",
        params![team_id, categoria],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Maior número de títulos de construtores na categoria entre TODAS as equipes EXCETO
/// uma (para saber se a campeã da temporada virou dona isolada do recorde). 0 se nenhuma.
pub fn get_category_constructor_titles_leader_excluding(
    conn: &Connection,
    categoria: &str,
    exclude_team: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COALESCE(MAX(cnt), 0) FROM (
            SELECT team_id, SUM(titulos_construtores) AS cnt FROM team_season_archive
            WHERE categoria = ?1 AND team_id <> ?2
            GROUP BY team_id
         )",
        params![categoria, exclude_team],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Vitórias de uma equipe na categoria (todas as temporadas), contadas do resultado.
pub fn get_team_category_wins(
    conn: &Connection,
    team_id: &str,
    categoria: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COUNT(*) FROM race_results r JOIN calendar c ON r.race_id = c.id
         WHERE r.equipe_id = ?1 AND c.categoria = ?2 AND r.posicao_final = 1",
        params![team_id, categoria],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Maior número de vitórias na categoria entre TODAS as equipes EXCETO uma. 0 se nenhuma.
pub fn get_category_team_win_leader_excluding(
    conn: &Connection,
    categoria: &str,
    exclude_team: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COALESCE(MAX(cnt), 0) FROM (
            SELECT r.equipe_id, COUNT(*) AS cnt FROM race_results r
            JOIN calendar c ON r.race_id = c.id
            WHERE c.categoria = ?1 AND r.posicao_final = 1 AND r.equipe_id <> ?2
            GROUP BY r.equipe_id
         )",
        params![categoria, exclude_team],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Dobradinhas (1-2 na mesma corrida) de uma equipe na categoria: corridas em que a
/// equipe terminou com um carro em 1º E outro em 2º.
pub fn get_team_category_one_two(
    conn: &Connection,
    team_id: &str,
    categoria: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COUNT(*) FROM (
            SELECT r.race_id FROM race_results r JOIN calendar c ON r.race_id = c.id
            WHERE r.equipe_id = ?1 AND c.categoria = ?2 AND r.posicao_final IN (1, 2)
            GROUP BY r.race_id HAVING COUNT(*) = 2
         )",
        params![team_id, categoria],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Maior número de dobradinhas na categoria entre TODAS as equipes EXCETO uma. 0 se nenhuma.
pub fn get_category_one_two_leader_excluding(
    conn: &Connection,
    categoria: &str,
    exclude_team: &str,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COALESCE(MAX(cnt), 0) FROM (
            SELECT equipe_id, COUNT(*) AS cnt FROM (
                SELECT r.equipe_id, r.race_id FROM race_results r
                JOIN calendar c ON r.race_id = c.id
                WHERE c.categoria = ?1 AND r.posicao_final IN (1, 2) AND r.equipe_id <> ?2
                GROUP BY r.equipe_id, r.race_id HAVING COUNT(*) = 2
            )
            GROUP BY equipe_id
         )",
        params![categoria, exclude_team],
        |r| r.get(0),
    )?;
    Ok(n)
}

pub fn get_teams_by_category(conn: &Connection, category_id: &str) -> Result<Vec<Team>, DbError> {
    let mut stmt = conn.prepare("SELECT * FROM teams WHERE categoria = ?1 ORDER BY nome")?;
    let mapped = stmt.query_map(params![category_id], team_from_row)?;
    let mut teams = collect_teams(mapped)?;
    attach_cars(conn, &mut teams)?;
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
    let mut teams = collect_teams(mapped)?;
    attach_cars(conn, &mut teams)?;
    Ok(teams)
}

/// Limpa `piloto_1_id` e `piloto_2_id` de todas as equipes especiais.
/// Afeta production_challenger (mazda/toyota/bmw) e endurance (gt4/gt3/lmp2).
pub fn clear_special_team_lineups(conn: &Connection) -> Result<usize, DbError> {
    let n = conn.execute(
        "UPDATE teams SET piloto_1_id = NULL, piloto_2_id = NULL
         WHERE categoria IN ('production_challenger', 'endurance')",
        [],
    )?;
    Ok(n)
}

/// Reseta todos os campos de hierarquia das equipes especiais.
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

/// Ajusta o caixa de um time por um delta (positivo credita, negativo debita). Base
/// da 1ª transferência de dinheiro time→time (multa de rescisão do poaching, Fase 2b).
pub fn adjust_team_cash(conn: &Connection, team_id: &str, delta: f64) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE teams SET cash_balance = cash_balance + ?1, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?2",
        params![delta, team_id],
    )?;
    ensure_team_rows_affected(affected, team_id, "ajustar caixa da equipe")?;
    Ok(())
}

/// Uma linha do histórico financeiro por rodada (tabela `team_finance_history`).
/// Carrega a divisão REAL de receita/despesa da rodada + caixa/dívida resultantes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamFinanceHistoryEntry {
    pub season_number: i32,
    pub round: i32,
    pub category: String,
    pub sponsorship_income: f64,
    /// Bilheteria/portão da rodada (Fase 3 do Estrelato). 0 nas linhas legadas.
    pub gate_income: f64,
    pub result_bonus: f64,
    pub partial_prize_income: f64,
    pub aid_income: f64,
    pub salary_expense: f64,
    pub event_operations_cost: f64,
    pub structural_maintenance_cost: f64,
    pub technical_investment_cost: f64,
    pub debt_service_cost: f64,
    /// Prêmio de construtores creditado no encerramento da temporada (0 em rodadas de
    /// corrida; > 0 só na linha de fechamento gravada por `insert_team_finance_season_close`).
    pub constructor_prize_income: f64,
    pub income_total: f64,
    pub expenses_total: f64,
    pub net: f64,
    pub cash_balance: f64,
    pub debt_balance: f64,
}

/// Grava a divisão financeira REAL de uma rodada para uma equipe. `team` deve estar no
/// estado PÓS-rodada (já com `apply_round_cashflow` aplicado), para `cash_balance`/
/// `debt_balance` refletirem o resultado. `INSERT OR REPLACE` na chave
/// (team_id, season_number, round) torna a gravação idempotente contra re-simulação.
pub fn insert_team_finance_history(
    conn: &Connection,
    team: &Team,
    context: &TeamRoundFinanceContext,
    summary: &RoundCashflowSummary,
    season_number: i32,
    round: i32,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT OR REPLACE INTO team_finance_history (
            team_id, season_number, round, category,
            sponsorship_income, gate_income, result_bonus, partial_prize_income, aid_income,
            salary_expense, event_operations_cost, structural_maintenance_cost,
            technical_investment_cost, debt_service_cost,
            income_total, expenses_total, net, cash_balance, debt_balance
        ) VALUES (
            :team_id, :season_number, :round, :category,
            :sponsorship_income, :gate_income, :result_bonus, :partial_prize_income, :aid_income,
            :salary_expense, :event_operations_cost, :structural_maintenance_cost,
            :technical_investment_cost, :debt_service_cost,
            :income_total, :expenses_total, :net, :cash_balance, :debt_balance
        )",
        rusqlite::named_params! {
            ":team_id": &team.id,
            ":season_number": season_number,
            ":round": round,
            ":category": &team.categoria,
            ":sponsorship_income": context.sponsorship_income,
            ":gate_income": context.gate_income,
            ":result_bonus": context.result_bonus,
            ":partial_prize_income": context.partial_prize_income,
            ":aid_income": context.aid_income,
            ":salary_expense": context.salary_expense,
            ":event_operations_cost": context.event_operations_cost,
            ":structural_maintenance_cost": context.structural_maintenance_cost,
            ":technical_investment_cost": context.technical_investment_cost,
            ":debt_service_cost": context.debt_service_cost,
            ":income_total": summary.income,
            ":expenses_total": summary.expenses,
            ":net": summary.net,
            ":cash_balance": team.cash_balance,
            ":debt_balance": team.debt_balance,
        },
    )?;
    Ok(())
}

/// Últimas `limit` rodadas do histórico financeiro de uma equipe, em ordem cronológica
/// (season_number, round) ASC. Fonte do dossiê financeiro real da aba My Team.
pub fn get_team_finance_history_recent(
    conn: &Connection,
    team_id: &str,
    limit: i64,
) -> Result<Vec<TeamFinanceHistoryEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT season_number, round, category,
                sponsorship_income, result_bonus, partial_prize_income, aid_income,
                salary_expense, event_operations_cost, structural_maintenance_cost,
                technical_investment_cost, debt_service_cost,
                income_total, expenses_total, net, cash_balance, debt_balance,
                constructor_prize_income, gate_income
         FROM team_finance_history
         WHERE team_id = ?1
         ORDER BY season_number DESC, round DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![team_id, limit], |row| {
        Ok(TeamFinanceHistoryEntry {
            season_number: row.get(0)?,
            round: row.get(1)?,
            category: row.get(2)?,
            sponsorship_income: row.get(3)?,
            result_bonus: row.get(4)?,
            partial_prize_income: row.get(5)?,
            aid_income: row.get(6)?,
            salary_expense: row.get(7)?,
            event_operations_cost: row.get(8)?,
            structural_maintenance_cost: row.get(9)?,
            technical_investment_cost: row.get(10)?,
            debt_service_cost: row.get(11)?,
            income_total: row.get(12)?,
            expenses_total: row.get(13)?,
            net: row.get(14)?,
            cash_balance: row.get(15)?,
            debt_balance: row.get(16)?,
            constructor_prize_income: row.get(17)?,
            gate_income: row.get(18)?,
        })
    })?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    // Consultado DESC (para o LIMIT pegar as mais recentes); devolve ASC (cronológico).
    entries.reverse();
    Ok(entries)
}

/// Rodada "sintética" da linha de encerramento (prêmio de construtores). Alta o bastante
/// para nenhuma temporada real alcançar, então ordena SEMPRE depois da última corrida da
/// temporada no gráfico de caixa.
pub const SEASON_CLOSE_ROUND: i32 = 1000;

/// Grava a linha de ENCERRAMENTO da temporada com o prêmio de construtores como receita
/// REAL. `team` deve estar no estado PÓS-prêmio (caixa já creditado). `INSERT OR REPLACE`
/// na chave (team_id, season_number, round = SEASON_CLOSE_ROUND) — idempotente contra
/// reprocessamento do encerramento. As 9 linhas de corrida ficam 0; só o prêmio entra.
pub fn insert_team_finance_season_close(
    conn: &Connection,
    team: &Team,
    season_number: i32,
    prize: f64,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT OR REPLACE INTO team_finance_history (
            team_id, season_number, round, category,
            constructor_prize_income, income_total, expenses_total, net,
            cash_balance, debt_balance
        ) VALUES (
            :team_id, :season_number, :round, :category,
            :prize, :prize, 0.0, :prize, :cash_balance, :debt_balance
        )",
        rusqlite::named_params! {
            ":team_id": &team.id,
            ":season_number": season_number,
            ":round": SEASON_CLOSE_ROUND,
            ":category": &team.categoria,
            ":prize": prize,
            ":cash_balance": team.cash_balance,
            ":debt_balance": team.debt_balance,
        },
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

pub fn count_teams_by_category_and_class(
    conn: &Connection,
    category_id: &str,
    class_name: &str,
) -> Result<i32, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM teams WHERE categoria = ?1 AND classe = ?2",
        params![category_id, class_name],
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

    fn create_finance_history_table(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE team_finance_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                team_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                round INTEGER NOT NULL,
                category TEXT NOT NULL DEFAULT '',
                sponsorship_income REAL NOT NULL DEFAULT 0.0,
                gate_income REAL NOT NULL DEFAULT 0.0,
                result_bonus REAL NOT NULL DEFAULT 0.0,
                partial_prize_income REAL NOT NULL DEFAULT 0.0,
                aid_income REAL NOT NULL DEFAULT 0.0,
                salary_expense REAL NOT NULL DEFAULT 0.0,
                event_operations_cost REAL NOT NULL DEFAULT 0.0,
                structural_maintenance_cost REAL NOT NULL DEFAULT 0.0,
                technical_investment_cost REAL NOT NULL DEFAULT 0.0,
                debt_service_cost REAL NOT NULL DEFAULT 0.0,
                income_total REAL NOT NULL DEFAULT 0.0,
                expenses_total REAL NOT NULL DEFAULT 0.0,
                net REAL NOT NULL DEFAULT 0.0,
                cash_balance REAL NOT NULL DEFAULT 0.0,
                debt_balance REAL NOT NULL DEFAULT 0.0,
                constructor_prize_income REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(team_id, season_number, round)
            );",
        )
        .expect("create team_finance_history table");
    }

    fn sample_finance_context() -> TeamRoundFinanceContext {
        TeamRoundFinanceContext {
            sponsorship_income: 100_000.0,
            gate_income: 15_000.0,
            result_bonus: 20_000.0,
            partial_prize_income: 5_000.0,
            aid_income: 0.0,
            salary_expense: 60_000.0,
            event_operations_cost: 25_000.0,
            structural_maintenance_cost: 12_000.0,
            technical_investment_cost: 8_000.0,
            debt_service_cost: 3_000.0,
        }
    }

    #[test]
    fn test_insert_and_read_team_finance_history_roundtrip() {
        let conn = setup_test_db().expect("test db");
        create_finance_history_table(&conn);
        let mut team = sample_team("gt3", "T001");
        insert_team(&conn, &team).expect("insert team");

        let context = sample_finance_context();
        let summary_r3 = RoundCashflowSummary {
            income: 125_000.0,
            expenses: 108_000.0,
            net: 17_000.0,
        };
        team.cash_balance = 517_000.0;
        team.debt_balance = 0.0;
        insert_team_finance_history(&conn, &team, &context, &summary_r3, 1, 3)
            .expect("insert history r3");

        let summary_r4 = RoundCashflowSummary {
            income: 130_000.0,
            expenses: 110_000.0,
            net: 20_000.0,
        };
        team.cash_balance = 537_000.0;
        insert_team_finance_history(&conn, &team, &context, &summary_r4, 1, 4)
            .expect("insert history r4");

        let entries = get_team_finance_history_recent(&conn, "T001", 10).expect("read history");
        assert_eq!(entries.len(), 2);
        // Ordem cronológica ASC (season, round).
        assert_eq!(entries[0].round, 3);
        assert_eq!(entries[1].round, 4);
        assert_eq!(entries[0].sponsorship_income, 100_000.0);
        assert_eq!(entries[0].income_total, 125_000.0);
        assert_eq!(entries[0].net, 17_000.0);
        assert_eq!(entries[0].cash_balance, 517_000.0);
        assert_eq!(entries[1].cash_balance, 537_000.0);
    }

    #[test]
    fn test_season_close_row_records_constructor_prize_after_races() {
        let conn = setup_test_db().expect("test db");
        create_finance_history_table(&conn);
        let mut team = sample_team("gt3", "T001");
        insert_team(&conn, &team).expect("insert team");

        // Uma rodada de corrida normal (sem prêmio).
        let context = sample_finance_context();
        let summary = RoundCashflowSummary {
            income: 125_000.0,
            expenses: 108_000.0,
            net: 17_000.0,
        };
        team.cash_balance = 517_000.0;
        insert_team_finance_history(&conn, &team, &context, &summary, 1, 14)
            .expect("insert last race round");

        // Encerramento: prêmio de construtores creditado como linha de receita real.
        team.cash_balance = 517_000.0 + 5_200_000.0;
        insert_team_finance_season_close(&conn, &team, 1, 5_200_000.0).expect("insert prize row");

        let entries = get_team_finance_history_recent(&conn, "T001", 10).expect("read history");
        assert_eq!(entries.len(), 2);
        // A rodada de corrida não tem prêmio.
        assert_eq!(entries[0].round, 14);
        assert_eq!(entries[0].constructor_prize_income, 0.0);
        // A linha de encerramento ordena DEPOIS da última corrida e carrega o prêmio.
        assert_eq!(entries[1].round, SEASON_CLOSE_ROUND);
        assert_eq!(entries[1].constructor_prize_income, 5_200_000.0);
        assert_eq!(entries[1].income_total, 5_200_000.0);
        assert_eq!(entries[1].net, 5_200_000.0);
        assert_eq!(entries[1].expenses_total, 0.0);
        assert_eq!(entries[1].cash_balance, 5_717_000.0);
    }

    #[test]
    fn test_team_finance_history_reround_is_idempotent() {
        let conn = setup_test_db().expect("test db");
        create_finance_history_table(&conn);
        let mut team = sample_team("gt3", "T001");
        insert_team(&conn, &team).expect("insert team");
        let context = sample_finance_context();

        let first = RoundCashflowSummary {
            income: 125_000.0,
            expenses: 108_000.0,
            net: 17_000.0,
        };
        team.cash_balance = 517_000.0;
        insert_team_finance_history(&conn, &team, &context, &first, 1, 4).expect("insert r4");

        // Re-simular a MESMA (season, round) substitui a linha, não duplica.
        let second = RoundCashflowSummary {
            income: 200_000.0,
            expenses: 100_000.0,
            net: 100_000.0,
        };
        team.cash_balance = 999_000.0;
        insert_team_finance_history(&conn, &team, &context, &second, 1, 4).expect("re-insert r4");

        let entries = get_team_finance_history_recent(&conn, "T001", 10).expect("read history");
        assert_eq!(entries.len(), 1, "re-gravar a mesma rodada não deve duplicar");
        assert_eq!(entries[0].cash_balance, 999_000.0, "deve refletir o novo valor");
        assert_eq!(entries[0].income_total, 200_000.0);
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
        insert_team(&conn, &sample_team("gt4", "T003")).expect("insert team 3");

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

    #[test]
    fn test_constructor_title_queries() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE team_season_archive (
                team_id TEXT, season_number INTEGER, categoria TEXT, titulos_construtores INTEGER
             );
             INSERT INTO team_season_archive (team_id, season_number, categoria, titulos_construtores) VALUES
             ('TA', 1, 'gt3', 1), ('TA', 2, 'gt3', 1), ('TA', 3, 'gt3', 1),
             ('TB', 1, 'gt3', 1), ('TB', 4, 'gt3', 0);",
        )
        .unwrap();
        assert_eq!(
            get_team_category_constructor_titles(&conn, "TA", "gt3").unwrap(),
            3
        );
        assert_eq!(
            get_team_category_constructor_titles(&conn, "TB", "gt3").unwrap(),
            1
        );
        // Maior entre as outras exceto TA → TB, com 1.
        assert_eq!(
            get_category_constructor_titles_leader_excluding(&conn, "gt3", "TA").unwrap(),
            1
        );
    }

    #[test]
    fn test_team_win_queries() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE calendar (id TEXT PRIMARY KEY, categoria TEXT);
             CREATE TABLE race_results (race_id TEXT, equipe_id TEXT, posicao_final INTEGER);
             INSERT INTO calendar (id, categoria) VALUES ('R1', 'gt3'), ('R2', 'gt3'), ('R3', 'gt3');
             INSERT INTO race_results (race_id, equipe_id, posicao_final) VALUES
             ('R1', 'TA', 1), ('R2', 'TA', 1), ('R3', 'TB', 1), ('R1', 'TB', 2);",
        )
        .unwrap();
        assert_eq!(get_team_category_wins(&conn, "TA", "gt3").unwrap(), 2);
        assert_eq!(get_team_category_wins(&conn, "TB", "gt3").unwrap(), 1);
        // Maior entre as outras exceto TA → TB, com 1.
        assert_eq!(
            get_category_team_win_leader_excluding(&conn, "gt3", "TA").unwrap(),
            1
        );
    }

    #[test]
    fn test_one_two_queries() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE calendar (id TEXT PRIMARY KEY, categoria TEXT);
             CREATE TABLE race_results (race_id TEXT, equipe_id TEXT, posicao_final INTEGER);
             INSERT INTO calendar (id, categoria) VALUES ('R1', 'gt3'), ('R2', 'gt3'), ('R3', 'gt3');
             -- R1: TA faz dobradinha (1 e 2). R2: TA vence mas 2º é da TB (não é dobradinha).
             -- R3: TA dobradinha de novo (1 e 2).
             INSERT INTO race_results (race_id, equipe_id, posicao_final) VALUES
             ('R1', 'TA', 1), ('R1', 'TA', 2),
             ('R2', 'TA', 1), ('R2', 'TB', 2),
             ('R3', 'TA', 1), ('R3', 'TA', 2);",
        )
        .unwrap();
        // TA tem 2 dobradinhas (R1 e R3); a R2 não conta (2º foi da TB).
        assert_eq!(get_team_category_one_two(&conn, "TA", "gt3").unwrap(), 2);
        assert_eq!(get_team_category_one_two(&conn, "TB", "gt3").unwrap(), 0);
        // Ninguém além de TA fez dobradinha.
        assert_eq!(
            get_category_one_two_leader_excluding(&conn, "gt3", "TA").unwrap(),
            0
        );
    }
}
