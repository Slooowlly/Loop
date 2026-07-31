//! Moral viva das equipes (ideia 3 do "sistema de equipes vivo").
//!
//! A `morale` (0.5–1.5, neutro 1.0) já era LIDA em dois lugares — a eficiência de
//! desenvolvimento do carro no offseason ([`crate::finance::cashflow`]) e o sinal
//! de comportamento da IA no export do iRacing ([`crate::iracing_sdk::behavior`]) —
//! mas `update_team_morale` nunca era chamada, então a moral ficava congelada em
//! ~1.0 e era invisível. Este módulo:
//!
//! 1. **Faz a moral SE MOVER** ao fim de cada temporada (mais volátil que a
//!    reputação — moral é humor): sobe com bom resultado, desce com decepção, e
//!    APANHA da treta interna (tensão N1/N2 alta). Reverte ao neutro.
//! 2. **Faz a moral ser SENTIDA na pista**: [`morale_pace_delta`] /
//!    [`morale_reliability_delta`] entram na simulação ([`crate::simulation::context`])
//!    para TODO carro do grid — corridas simuladas do jogador E times de IA por
//!    igual (requisito do usuário: a moral afeta tanto o jogador quanto as IAs).
//!
//! Sensação travada com o usuário: **Volátil** (`ADJUST_RATE = 0.35`) e efeito de
//! pista **Sutil** (`PACE_SPAN = 1.0` → ±0.5 de car_performance nos extremos;
//! `RELIABILITY_SPAN = 6.0` → ±3 de confiabilidade).

use rusqlite::Connection;

use crate::db::queries::teams as team_queries;
use crate::models::season::Season;

/// Amplitude do alvo por resultado: campeão de grid cheio mira `1.0 + RESULT_SPAN`.
const RESULT_SPAN: f64 = 0.4;
/// Penalidade máxima da treta interna (garagem em crise) sobre o alvo.
const STRIFE_SPAN: f64 = 0.35;
/// Fração da distância até o alvo percorrida por temporada (volátil — humor).
const ADJUST_RATE: f64 = 0.35;
const FLOOR: f64 = 0.5;
const CEIL: f64 = 1.5;

/// Efeito de pista (sutil): amplitude do bônus/pênalti de ritmo por moral.
/// `(moral−1.0) × PACE_SPAN` → nos extremos (0.5 / 1.5) dá ∓0.5 / ±0.5.
const PACE_SPAN: f64 = 1.0;
/// Efeito de pista: amplitude do bônus/pênalti de confiabilidade por moral
/// → nos extremos ∓3 / ±3 pontos de confiabilidade.
const RELIABILITY_SPAN: f64 = 6.0;

/// Força no grid a partir da posição no campeonato: 1º = +1, último = −1, meio = 0.
/// Grids degenerados (`<= 1`) não têm ordenação → 0.
fn standing_strength(position: i32, grid_size: i32) -> f64 {
    if grid_size <= 1 {
        return 0.0;
    }
    let clamped = position.clamp(1, grid_size);
    1.0 - 2.0 * ((clamped - 1) as f64) / ((grid_size - 1) as f64)
}

/// Avança a moral uma temporada. Puro e determinístico.
///
/// - `standing_strength` em `[-1, 1]` (resultado vs grid).
/// - `tension` = `hierarquia_tensao` (0–100). Só a treta ACIMA de 50 pune (garagem
///   calma é neutra); em crise (100) o alvo cai `STRIFE_SPAN` inteiro.
pub fn advance_team_morale(current: f64, standing_strength: f64, tension: f64) -> f64 {
    let strife = ((tension.clamp(0.0, 100.0) - 50.0) / 50.0).max(0.0);
    let target = 1.0 + standing_strength.clamp(-1.0, 1.0) * RESULT_SPAN - strife * STRIFE_SPAN;
    let next = current + (target - current) * ADJUST_RATE;
    next.clamp(FLOOR, CEIL)
}

/// Bônus/pênalti de `car_performance` na simulação por moral (efeito sutil).
pub fn morale_pace_delta(morale: f64) -> f64 {
    (morale.clamp(FLOOR, CEIL) - 1.0) * PACE_SPAN
}

/// Bônus/pênalti de confiabilidade na simulação por moral (efeito sutil).
pub fn morale_reliability_delta(morale: f64) -> f64 {
    (morale.clamp(FLOOR, CEIL) - 1.0) * RELIABILITY_SPAN
}

/// Ao fim da temporada, move a moral de toda equipe que competiu, a partir do
/// resultado (posição arquivada) e da tensão interna atual do time.
///
/// Roda no offseason ([`crate::evolution::pipeline`]) com o archive já gravado.
/// A promoção/rebaixamento aplica os próprios multiplicadores de moral depois.
pub(crate) fn update_team_morale_from_season(
    conn: &Connection,
    season: &Season,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT team_id, posicao_campeonato,
                    COUNT(*) OVER (PARTITION BY categoria, COALESCE(classe, '')) AS grid_size
             FROM team_season_archive
             WHERE season_number = ?1 AND posicao_campeonato IS NOT NULL",
        )
        .map_err(|e| format!("Falha ao preparar consulta de moral: {e}"))?;
    let rows = stmt
        .query_map([season.numero], |row| {
            let team_id: String = row.get(0)?;
            let position: i32 = row.get(1)?;
            let grid_size: i32 = row.get(2)?;
            Ok((team_id, position, grid_size))
        })
        .map_err(|e| format!("Falha ao consultar moral da temporada: {e}"))?;

    let mut updates: Vec<(String, i32, i32)> = Vec::new();
    for row in rows {
        updates.push(row.map_err(|e| format!("Falha ao mapear moral: {e}"))?);
    }
    drop(stmt);

    for (team_id, position, grid_size) in updates {
        let mut team = match team_queries::get_team_by_id(conn, &team_id) {
            Ok(Some(team)) => team,
            Ok(None) => continue,
            Err(e) => return Err(format!("Falha ao carregar equipe {team_id}: {e}")),
        };
        let strength = standing_strength(position, grid_size);
        team.morale = advance_team_morale(team.morale, strength, team.hierarquia_tensao);
        team_queries::update_team(conn, &team)
            .map_err(|e| format!("Falha ao atualizar moral da equipe {team_id}: {e}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_GRID: i32 = 6;

    #[test]
    fn winning_lifts_morale_losing_lowers_it() {
        let up = advance_team_morale(1.0, standing_strength(1, FULL_GRID), 0.0);
        let down = advance_team_morale(1.0, standing_strength(FULL_GRID, FULL_GRID), 0.0);
        assert!(up > 1.0, "campeão anima, foi {up}");
        assert!(down < 1.0, "lanterna desanima, foi {down}");
    }

    #[test]
    fn internal_strife_drags_morale_down_even_for_a_mid_team() {
        let calm = advance_team_morale(1.0, 0.0, 10.0);
        let crisis = advance_team_morale(1.0, 0.0, 100.0);
        assert!(
            (calm - 1.0).abs() < 0.001,
            "garagem calma no meio = neutra, foi {calm}"
        );
        assert!(crisis < 0.95, "crise interna derruba a moral, foi {crisis}");
    }

    #[test]
    fn winning_can_mask_some_strife() {
        // Campeão numa garagem em guerra: sobe pouco, mas não desaba.
        let m = advance_team_morale(1.0, standing_strength(1, FULL_GRID), 100.0);
        assert!(m > 1.0, "vencer mascara parte da treta, foi {m}");
    }

    #[test]
    fn morale_is_volatile_reaching_target_faster_than_reputation() {
        // Um passo já cobre 35% da distância até o alvo (~1.4 p/ campeão calmo).
        let after_one = advance_team_morale(1.0, 1.0, 0.0);
        assert!(
            after_one > 1.12,
            "volátil: sobe rápido em 1 temporada, foi {after_one}"
        );
    }

    #[test]
    fn morale_reverts_toward_neutral_when_results_normalize() {
        // Time eufórico (1.4) que passa a terminar no meio decai rumo a 1.0.
        let after = advance_team_morale(1.4, 0.0, 10.0);
        assert!(after < 1.4 && after > 1.0, "reverte ao neutro, foi {after}");
    }

    #[test]
    fn morale_is_clamped_to_band() {
        assert!(advance_team_morale(1.5, 1.0, 0.0) <= CEIL);
        assert!(advance_team_morale(0.5, -1.0, 100.0) >= FLOOR);
    }

    #[test]
    fn sim_deltas_are_subtle_and_symmetric_around_neutral() {
        assert_eq!(morale_pace_delta(1.0), 0.0);
        assert_eq!(morale_reliability_delta(1.0), 0.0);
        assert!((morale_pace_delta(1.5) - 0.5).abs() < 1e-9);
        assert!((morale_pace_delta(0.5) + 0.5).abs() < 1e-9);
        assert!((morale_reliability_delta(1.5) - 3.0).abs() < 1e-9);
        assert!((morale_reliability_delta(0.5) + 3.0).abs() < 1e-9);
    }

    #[test]
    fn sim_deltas_clamp_out_of_band_morale() {
        // Moral fora da banda não amplifica o efeito além dos extremos.
        assert!((morale_pace_delta(9.0) - 0.5).abs() < 1e-9);
    }
}
