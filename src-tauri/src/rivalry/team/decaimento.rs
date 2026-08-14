//! Decaimento anual das rivalidades de equipe (fim de temporada).

use std::collections::HashSet;

use rusqlite::Connection;

use crate::common::time::current_timestamp;
use crate::db::connection::DbError;
use crate::db::queries::team_rivalries::{
    delete_team_rivalry, get_all_team_rivalries, update_team_rivalry_axes,
};
use crate::models::rivalry::{normalize_team_pair, rivalry_lifecycle, RivalryLifecycle};

use super::campeonato::pares_da_briga_de_construtores;

/// Aplica o decaimento anual a todas as rivalidades de equipe (mesma regra do piloto):
/// - Ativa nesta temporada (`temporada_update == atual`): `recent *= 0.5`, histórico intacto.
/// - Inativa: `recent *= 0.2`, `historical *= 0.85`.
/// - Ciclo de vida `Extinta` → removida do banco.
///
/// Deve ser chamada uma vez no pipeline de fim de temporada, **antes** do veredito de
/// campeonato (`process_constructor_battle_rivalry`): o veredito é o resumo da temporada
/// que fecha e passá-lo pelo decaimento do mesmo ano esfria duas vezes — um par não
/// decisor nascia em 4/10, saía do decaimento em 4/5 e era apagado no mesmo tique.
///
/// Como o veredito vem depois, o decaimento pergunta ao `campeonato` quem ainda vai ser
/// reforçado nesta virada e trata esse par de duas maneiras:
/// - **ativo**, porque brigar pelo campeonato é atividade da temporada, mesmo que o
///   carimbo de `temporada_update` ainda seja o do ano passado;
/// - **nunca apagado**, porque a linha guarda o eixo histórico que o clássico levou
///   temporadas acumulando, e ele some junto se o decaimento a remover meio segundo
///   antes do reforço chegar.
pub fn apply_season_end_team_rivalry_decay(
    conn: &Connection,
    temporada_atual: i32,
) -> Result<(), DbError> {
    let a_reforcar: HashSet<(String, String)> =
        pares_da_briga_de_construtores(conn, temporada_atual)?
            .iter()
            .filter_map(|brigante| normalize_team_pair(&brigante.team_a, &brigante.team_b))
            .map(|par| (par.team1_id, par.team2_id))
            .collect();
    let all = get_all_team_rivalries(conn)?;
    let now = current_timestamp();

    for r in all {
        let sera_reforcada = a_reforcar.contains(&(r.team1_id.clone(), r.team2_id.clone()));
        let (new_historical, new_recent) =
            if r.temporada_update == temporada_atual || sera_reforcada {
                (r.historical_intensity, r.recent_activity * 0.5)
            } else {
                (r.historical_intensity * 0.85, r.recent_activity * 0.2)
            };

        if !sera_reforcada
            && matches!(
                rivalry_lifecycle(new_historical, new_recent),
                RivalryLifecycle::Extinta
            )
        {
            delete_team_rivalry(conn, &r.id)?;
        } else {
            update_team_rivalry_axes(
                conn,
                &r.id,
                new_historical,
                new_recent,
                &now,
                r.temporada_update,
            )?;
        }
    }

    Ok(())
}
