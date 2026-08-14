#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::calendar::CalendarEntry;
use crate::models::enums::SeasonPhase;

// ── Conversores season_week ↔ week_of_year ────────────────────────────────────
//
// Régua interna season_week (1–51):
//   sw  1– 4  →  woy 49–52  (dezembro do ano civil ANTERIOR ao da temporada)
//   sw  5– 9  →  woy  1– 5  (janeiro–fevereiro do ano da temporada)
//   sw 10–51  →  woy  6–47  (corridas fevereiro–novembro)
//
// A semana 48 (fim de novembro) é terra de ninguém no modelo 9D:
//   fica entre a última corrida (woy 47) e a abertura do mercado (woy 49).
//   Ela não tem season_week correspondente e é erro explícito neste módulo.

/// Converte season_week (1–51) em week_of_year (1–52, exceto 48).
/// Retorna Err para faixas inválidas.
pub fn season_week_to_week_of_year(season_week: u8) -> Result<u8, String> {
    match season_week {
        1..=4 => Ok(48 + season_week), // 49, 50, 51, 52
        5..=51 => Ok(season_week - 4), // 1..=47
        0 => Err("season_week 0 é inválida (faixa: 1–51)".to_string()),
        _ => Err(format!("season_week {season_week} fora do intervalo 1–51")),
    }
}

/// Retorna o ano civil correspondente a uma season_week dentro de uma temporada.
///
/// REGRA CRÍTICA: season_week 1–4 correspondem a dezembro do ano civil ANTERIOR
/// ao ano da temporada (ex.: temporada 2027, sw 2 = woy 50 de 2026).
/// season_week 5–51 pertencem ao ano da temporada.
pub fn season_week_to_calendar_year(season_week: u8, season_year: i32) -> Result<i32, String> {
    match season_week {
        1..=4 => Ok(season_year - 1),
        5..=51 => Ok(season_year),
        0 => Err("season_week 0 é inválida (faixa: 1–51)".to_string()),
        _ => Err(format!("season_week {season_week} fora do intervalo 1–51")),
    }
}

/// A MESMA régua de `week_of_year_to_season_week`, escrita em SQL.
///
/// Existe porque o calendário guarda `season_week` NULL em linhas antigas e as consultas
/// precisam derivar a semana da temporada dentro do próprio SELECT. A versão anterior
/// embutia `COALESCE(season_week, week_of_year + 4)` direto na query: a aritmética só vale
/// na faixa 1–47 e, para dezembro (woy 49–52), produzia 53–56, que nunca casa com a semana
/// consultada — a linha simplesmente não era encontrada, sem erro nenhum.
///
/// A terra de ninguém da semana 48 vira `NULL` aqui, que é o equivalente SQL do `Err` do
/// conversor: não casa com nada. O teste `a_regua_em_sql_bate_com_a_regua_em_rust` compara
/// as duas implementações em toda a faixa 1–52 e é o que impede elas de se separarem.
pub const SQL_SEASON_WEEK_DERIVADA: &str = "COALESCE(season_week, CASE
        WHEN week_of_year BETWEEN 49 AND 52 THEN week_of_year - 48
        WHEN week_of_year BETWEEN 1 AND 47 THEN week_of_year + 4
        ELSE NULL
    END)";

/// Converte week_of_year (1–52, exceto 48) em season_week (1–51).
///
/// O parâmetro `phase` existe para futura validação com dados legados — ele NÃO
/// altera o mapeamento aritmético, que é determinístico pelo valor de week_of_year.
pub fn week_of_year_to_season_week(week_of_year: u8, _phase: &SeasonPhase) -> Result<u8, String> {
    match week_of_year {
        49..=52 => Ok(week_of_year - 48), // → 1, 2, 3, 4
        1..=47 => Ok(week_of_year + 4),   // → 5..=51
        48 => Err(
            "week_of_year 48 é terra de ninguém no modelo 9D (entre woy 47 e woy 49)".to_string(),
        ),
        0 => Err("week_of_year 0 é inválida (faixa: 1–52)".to_string()),
        _ => Err(format!(
            "week_of_year {week_of_year} fora do intervalo 1–52"
        )),
    }
}

#[cfg(test)]
mod temporal_tests {
    use super::*;
    use crate::models::enums::SeasonPhase;

    // ── season_week_to_week_of_year ───────────────────────────────────────────

    #[test]
    fn sw_to_woy_boundaries() {
        assert_eq!(season_week_to_week_of_year(1).unwrap(), 49);
        assert_eq!(season_week_to_week_of_year(4).unwrap(), 52);
        assert_eq!(season_week_to_week_of_year(5).unwrap(), 1);
        assert_eq!(season_week_to_week_of_year(9).unwrap(), 5);
        assert_eq!(season_week_to_week_of_year(10).unwrap(), 6);
        assert_eq!(season_week_to_week_of_year(51).unwrap(), 47);
    }

    #[test]
    fn sw_to_woy_invalid() {
        assert!(season_week_to_week_of_year(0).is_err());
        assert!(season_week_to_week_of_year(52).is_err());
        assert!(season_week_to_week_of_year(100).is_err());
    }

    // ── season_week_to_calendar_year ──────────────────────────────────────────

    #[test]
    fn sw_to_year_december_side() {
        // sw 1-4 = dezembro do ano anterior
        assert_eq!(season_week_to_calendar_year(1, 2027).unwrap(), 2026);
        assert_eq!(season_week_to_calendar_year(4, 2027).unwrap(), 2026);
    }

    #[test]
    fn sw_to_year_january_side() {
        // sw 5 = semana 1 do ano da temporada
        assert_eq!(season_week_to_calendar_year(5, 2027).unwrap(), 2027);
        assert_eq!(season_week_to_calendar_year(51, 2027).unwrap(), 2027);
    }

    #[test]
    fn sw_to_year_invalid() {
        assert!(season_week_to_calendar_year(0, 2027).is_err());
        assert!(season_week_to_calendar_year(52, 2027).is_err());
    }

    // ── week_of_year_to_season_week ───────────────────────────────────────────

    #[test]
    fn woy_to_sw_boundaries() {
        assert_eq!(
            week_of_year_to_season_week(49, &SeasonPhase::BlocoRegular).unwrap(),
            1
        );
        assert_eq!(
            week_of_year_to_season_week(52, &SeasonPhase::BlocoRegular).unwrap(),
            4
        );
        assert_eq!(
            week_of_year_to_season_week(1, &SeasonPhase::BlocoRegular).unwrap(),
            5
        );
        assert_eq!(
            week_of_year_to_season_week(5, &SeasonPhase::BlocoRegular).unwrap(),
            9
        );
        assert_eq!(
            week_of_year_to_season_week(6, &SeasonPhase::BlocoRegular).unwrap(),
            10
        );
        assert_eq!(
            week_of_year_to_season_week(47, &SeasonPhase::BlocoRegular).unwrap(),
            51
        );
    }

    #[test]
    fn woy_to_sw_week_48_is_error() {
        assert!(week_of_year_to_season_week(48, &SeasonPhase::BlocoRegular).is_err());
        // Terra de ninguém — deve ser explícito, não silencioso
        let err = week_of_year_to_season_week(48, &SeasonPhase::BlocoRegular).unwrap_err();
        assert!(
            err.contains("terra de ninguém"),
            "mensagem de erro inesperada: {err}"
        );
    }

    #[test]
    fn woy_to_sw_invalid() {
        assert!(week_of_year_to_season_week(0, &SeasonPhase::Temporada).is_err());
        assert!(week_of_year_to_season_week(53, &SeasonPhase::Temporada).is_err());
        assert!(week_of_year_to_season_week(100, &SeasonPhase::Temporada).is_err());
    }

    // ── paridade entre a régua em Rust e a régua em SQL ───────────────────────

    /// As consultas do calendário derivam `season_week` dentro do SELECT quando a coluna é
    /// NULL (linha antiga). Essa derivação é uma SEGUNDA implementação da régua, e este
    /// teste é o que impede as duas de divergirem: para toda week_of_year de 1 a 52, o
    /// SQLite tem de devolver exatamente o que o conversor devolve, e NULL exatamente onde
    /// o conversor devolve erro.
    #[test]
    fn a_regua_em_sql_bate_com_a_regua_em_rust() {
        let conn = rusqlite::Connection::open_in_memory().expect("banco em memória");
        conn.execute_batch("CREATE TABLE calendar (season_week INTEGER, week_of_year INTEGER);")
            .expect("tabela");

        let sql = format!("SELECT {SQL_SEASON_WEEK_DERIVADA} FROM calendar");

        for woy in 1u8..=52 {
            conn.execute("DELETE FROM calendar", []).expect("limpar");
            conn.execute(
                "INSERT INTO calendar (season_week, week_of_year) VALUES (NULL, ?1)",
                rusqlite::params![woy as i64],
            )
            .expect("linha");

            let derivada: Option<i64> = conn
                .query_row(&sql, [], |row| row.get(0))
                .expect("derivação em SQL");

            match week_of_year_to_season_week(woy, &SeasonPhase::BlocoRegular) {
                Ok(esperado) => assert_eq!(
                    derivada,
                    Some(esperado as i64),
                    "a régua em SQL divergiu da régua em Rust na week_of_year {woy}"
                ),
                Err(_) => assert_eq!(
                    derivada, None,
                    "week_of_year {woy} é erro em Rust e precisa ser NULL em SQL"
                ),
            }
        }
    }

    /// Quando a coluna `season_week` está preenchida, ela manda — a derivação é só o
    /// fallback da linha antiga.
    #[test]
    fn a_coluna_preenchida_tem_precedencia_sobre_a_derivacao() {
        let conn = rusqlite::Connection::open_in_memory().expect("banco em memória");
        conn.execute_batch(
            "CREATE TABLE calendar (season_week INTEGER, week_of_year INTEGER);
             INSERT INTO calendar (season_week, week_of_year) VALUES (7, 40);",
        )
        .expect("tabela");

        let derivada: Option<i64> = conn
            .query_row(
                &format!("SELECT {SQL_SEASON_WEEK_DERIVADA} FROM calendar"),
                [],
                |row| row.get(0),
            )
            .expect("derivação");
        assert_eq!(derivada, Some(7));
    }

    // ── round-trip para todas as 51 semanas ───────────────────────────────────

    #[test]
    fn roundtrip_all_51_weeks() {
        for sw in 1u8..=51 {
            let woy = season_week_to_week_of_year(sw)
                .unwrap_or_else(|e| panic!("sw {sw} → woy falhou: {e}"));
            let back = week_of_year_to_season_week(woy, &SeasonPhase::Temporada)
                .unwrap_or_else(|e| panic!("woy {woy} → sw falhou (originado de sw {sw}): {e}"));
            assert_eq!(
                back, sw,
                "round-trip falhou para sw {sw}: woy {woy} → sw {back}"
            );
        }
    }
}

/// Estado temporal derivado da temporada ativa.
/// Combina macroestado (season.fase) com estado factual (calendar).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonTemporalSummary {
    pub fase: SeasonPhase,
    /// MAX(season_week) das corridas concluídas — a semana efetivamente alcançada.
    /// None se nenhuma corrida foi concluída ainda.
    pub effective_week: Option<i32>,
    /// Data atual visível da carreira para a UI.
    pub current_display_date: String,
    /// Próxima corrida pendente da categoria do jogador.
    /// NOTA: acoplamento temporário com CalendarEntry. Em iteração futura,
    /// pode virar um DTO temporal mais enxuto sem todos os campos de corrida.
    pub next_player_event: Option<CalendarEntry>,
    /// Data da próxima corrida do jogador, pronta para a UI.
    pub next_event_display_date: Option<String>,
    /// Distância em dias até o próximo evento.
    pub days_until_next_event: Option<i32>,
    /// Corridas pendentes na fase atual da temporada.
    pub pending_in_phase: i32,
}
