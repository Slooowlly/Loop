//! A TESE JORNALÍSTICA do boletim: os sinais destilados do resultado e a eleição
//! do ângulo dominante da matéria.

use super::beats::BeatKind;
use super::consulta::find;
use crate::simulation::race::RaceResult;

/// A TESE JORNALÍSTICA do boletim — o ângulo dominante da matéria (voz de revista,
/// grid inteiro). Diferente da prévia/debrief (que giram no piloto do leitor), aqui o
/// eixo é a HISTÓRIA DA CORRIDA: caos, vitória improvável, pole que afundou, remontada,
/// domínio, ou dia de administração. O piloto do leitor segue citado, nunca protagonista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaceThesis {
    Caos,
    VitoriaImprovavel,
    PoleFrustrada,
    Remontada,
    Dominio,
    CorridaLimpa,
}

/// Sinais destilados do resultado que alimentam a tese (puros/testáveis, sem depender
/// da estrutura inteira do `RaceResult`).
pub struct RaceThesisSignals {
    pub total_dnfs: i32,
    pub field_size: i32,
    pub winner_name: String,
    pub winner_team: String,
    pub winner_grid: i32,
    /// (nome, chegada) da POLE quando ela NÃO venceu e afundou (>= P5).
    pub pole_flopped: Option<(String, i32)>,
    /// (nome, largada, chegada, ganho) da maior recuperação, se NÃO foi o vencedor.
    pub biggest_recovery: Option<(String, i32, i32, i32)>,
}

/// Extrai os sinais da tese do resultado bruto.
pub fn race_thesis_signals(result: &RaceResult) -> RaceThesisSignals {
    let rows = &result.race_results;
    let winner = find(rows, &result.winner_id);
    let pole_flopped = find(rows, &result.pole_sitter_id).and_then(|p| {
        if p.pilot_id != result.winner_id && !p.is_dnf && p.finish_position >= 5 {
            Some((p.pilot_name.clone(), p.finish_position))
        } else {
            None
        }
    });
    let biggest_recovery = result
        .most_positions_gained_id
        .as_ref()
        .and_then(|id| find(rows, id))
        .and_then(|d| {
            if d.pilot_id != result.winner_id && d.positions_gained >= 1 {
                Some((
                    d.pilot_name.clone(),
                    d.grid_position,
                    d.finish_position,
                    d.positions_gained,
                ))
            } else {
                None
            }
        });
    RaceThesisSignals {
        total_dnfs: result.total_dnfs,
        field_size: rows.len() as i32,
        winner_name: winner
            .map(|w| w.pilot_name.clone())
            .unwrap_or_else(|| rust_i18n::t!("narrative.beat.winner_fallback").to_string()),
        winner_team: winner.map(|w| w.team_name.clone()).unwrap_or_default(),
        winner_grid: winner.map(|w| w.grid_position).unwrap_or(0),
        pole_flopped,
        biggest_recovery,
    }
}

/// Elege a tese dominante (1ª que casar vence). Devolve o statement do EIXO + os
/// `BeatKind` que devem subir para a camada de DESTAQUES; o resto vira pano de fundo.
pub fn select_race_thesis(s: &RaceThesisSignals) -> (RaceThesis, String, Vec<BeatKind>) {
    use BeatKind::*;
    // Caos: muitos abandonos redesenharam o grid.
    let caos_gate = 4.max(s.field_size / 4);
    if s.total_dnfs >= caos_gate {
        return (
            RaceThesis::Caos,
            rust_i18n::t!("narrative.thesis.caos", dnfs = s.total_dnfs).to_string(),
            vec![Abandono, Acidente, Vitoria],
        );
    }
    // Vitória improvável: o vencedor veio lá de trás.
    if s.winner_grid >= 6 {
        return (
            RaceThesis::VitoriaImprovavel,
            rust_i18n::t!(
                "narrative.thesis.improbable_win",
                name = s.winner_name.as_str(),
                team = s.winner_team.as_str(),
                grid = s.winner_grid
            )
            .to_string(),
            vec![Vitoria, Recuperacao],
        );
    }
    // Pole frustrada: o favorito da pole afundou.
    if let Some((pole_name, finish)) = &s.pole_flopped {
        return (
            RaceThesis::PoleFrustrada,
            rust_i18n::t!(
                "narrative.thesis.pole_flop",
                pole = pole_name.as_str(),
                finish = finish,
                winner = s.winner_name.as_str()
            )
            .to_string(),
            vec![Decepcao, Vitoria],
        );
    }
    // Remontada épica de um não-vencedor.
    if let Some((name, grid, finish, gained)) = &s.biggest_recovery {
        if *gained >= 8 && *finish <= 6 {
            return (
                RaceThesis::Remontada,
                rust_i18n::t!(
                    "narrative.thesis.comeback",
                    name = name.as_str(),
                    grid = grid,
                    finish = finish,
                    gained = gained
                )
                .to_string(),
                vec![Recuperacao, Vitoria],
            );
        }
    }
    // Domínio de quem largou na frente.
    if s.winner_grid >= 1 && s.winner_grid <= 2 {
        return (
            RaceThesis::Dominio,
            rust_i18n::t!(
                "narrative.thesis.dominance",
                name = s.winner_name.as_str(),
                team = s.winner_team.as_str(),
                grid = s.winner_grid
            )
            .to_string(),
            vec![Vitoria, VoltaRapida],
        );
    }
    // Baseline: dia de administração.
    (
        RaceThesis::CorridaLimpa,
        rust_i18n::t!(
            "narrative.thesis.clean_race",
            name = s.winner_name.as_str(),
            team = s.winner_team.as_str()
        )
        .to_string(),
        vec![Vitoria, Podio],
    )
}
