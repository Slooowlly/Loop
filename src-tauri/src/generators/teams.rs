//! Geração das equipes persistentes de uma categoria a partir dos templates.
//!
//! Isto morava em `models/team.rs`, e era geração de MUNDO dentro da camada de modelo — a
//! outra metade da mesma exceção que tirou o SQL de licenças de `models/license.rs`. Model
//! declara a FORMA de uma equipe (`Team`, `Team::from_template`); quem POVOA o mundo é o
//! gerador, que é onde vivem os três call sites (`generators::world::genesis`,
//! `generators::world::historico` e a auditoria de economia).
//!
//! Nada aqui toca no banco: a persistência das equipes geradas é do chamador.

use rand::Rng;

use crate::constants::categories::{get_category_config, is_especial};
use crate::constants::teams::get_team_templates;
use crate::models::team::Team;

/// Gera o conjunto de equipes persistentes de uma categoria a partir dos templates.
pub fn generate_teams_for_category<F>(
    category_id: &str,
    temporada: i32,
    id_generator: &mut F,
) -> Vec<Team>
where
    F: FnMut() -> String,
{
    let mut rng = rand::thread_rng();
    generate_teams_for_category_with_rng(category_id, temporada, id_generator, &mut rng)
}

fn generate_teams_for_category_with_rng<F, R>(
    category_id: &str,
    temporada: i32,
    id_generator: &mut F,
    rng: &mut R,
) -> Vec<Team>
where
    F: FnMut() -> String,
    R: Rng,
{
    let templates = get_team_templates(category_id);
    let teams: Vec<Team> = templates
        .into_iter()
        .map(|template| {
            Team::from_template_with_rng(template, category_id, id_generator(), temporada, rng)
        })
        .collect();

    if let Some(config) = get_category_config(category_id) {
        // Legacy category capacity can differ from own templates: Endurance has
        // 18 competitive slots, but six come from the regular LMP2 category.
        let expected_team_count = if is_especial(category_id) {
            teams.len()
        } else {
            config.num_equipes as usize
        };

        assert_eq!(
            teams.len(),
            expected_team_count,
            "Quantidade de equipes persistentes gerada para '{}' difere da configuracao da categoria",
            category_id
        );
    }

    teams
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_generate_teams_for_category_correct_count() {
        let mut rng = StdRng::seed_from_u64(44);
        let mut seq = 1_u32;
        let mut next_id = || {
            let id = format!("T{:03}", seq);
            seq += 1;
            id
        };

        let teams = generate_teams_for_category_with_rng("gt3", 2026, &mut next_id, &mut rng);

        assert_eq!(teams.len(), 14);
        assert_eq!(teams.first().map(|team| team.id.as_str()), Some("T001"));
        assert_eq!(teams.last().map(|team| team.id.as_str()), Some("T014"));
    }
}
