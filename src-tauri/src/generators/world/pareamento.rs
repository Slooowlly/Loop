//! O LAÇO de pareamento de um grid: quem é N1, quem é N2, em que equipe, e com que
//! contrato inicial.
//!
//! Existia duas vezes, palavra por palavra, em `genesis` e em `historico` — inclusive o
//! comentário do embaralhamento anti-roteiro do tier 0. As duas geram o mesmo grid; a
//! única diferença real é que o genesis reserva um assento para o JOGADOR, e ali o N2 da
//! equipe dele não é sorteado. Isso virou o parâmetro `player_slot`, e o resto passou a
//! ser um caminho só: mudar a regra de pareamento não pede mais tocar dois arquivos em
//! sincronia, o que é a forma mais fácil de os dois mundos passarem a nascer diferentes.

use std::cmp::Ordering;

use rand::seq::SliceRandom;
use rand::Rng;

use super::tipos::{contract_with_team_class, LocalIdAllocator};
use crate::models::contract::{generate_initial_contract, Contract};
use crate::models::driver::Driver;
use crate::models::enums::TeamRole;
use crate::models::team::Team;

/// O assento do jogador dentro de uma categoria: em que equipe ele senta e com que
/// skill (para decidir se ele nasce N1 ou N2 na hierarquia).
pub(crate) struct PlayerSlot<'a> {
    pub team_id: &'a str,
    pub driver_id: &'a str,
    pub driver_name: &'a str,
    pub skill: f64,
}

/// O que o pareamento produziu para uma categoria.
#[derive(Default)]
pub(crate) struct GridFill {
    pub drivers: Vec<Driver>,
    pub contracts: Vec<Contract>,
    /// A equipe do jogador, quando havia `player_slot` e ela foi encontrada.
    pub player_team_id: Option<String>,
    /// E o contrato dele.
    pub player_contract: Option<Contract>,
}

/// Pareia os pilotos de IA com as equipes de UMA categoria e emite os contratos iniciais.
///
/// `ai_drivers` entra na ordem em que a geração devolveu; o pareamento ordena por skill
/// (desempate por nome, para ser determinístico) e casa o melhor piloto com o melhor
/// carro. `teams` é mutada: hierarquia, assentos e `is_player_team` saem daqui.
///
/// **Consumo de RNG:** um único `shuffle`, e só no tier 0. Quem chama pode contar com
/// isso — os dois mundos dependem da sequência do `rng` para serem reproduzíveis por
/// semente.
pub(crate) fn fill_category_grid<R: Rng>(
    category_id: &str,
    tier: u8,
    teams: &mut [Team],
    ai_drivers: Vec<Driver>,
    player_slot: Option<PlayerSlot<'_>>,
    ids: &mut LocalIdAllocator,
    rng: &mut R,
) -> Result<GridFill, String> {
    let mut ai_drivers = ai_drivers;
    ai_drivers.sort_by(|left, right| {
        right
            .atributos
            .skill
            .total_cmp(&left.atributos.skill)
            .then_with(|| left.nome.cmp(&right.nome))
    });

    let team_count = teams.len();
    let mut n1_pool = ai_drivers.into_iter();
    let n1_drivers: Vec<Driver> = n1_pool.by_ref().take(team_count).collect();
    let mut n2_drivers = n1_pool;

    let mut team_order: Vec<usize> = (0..team_count).collect();
    team_order.sort_by(|left, right| {
        teams[*right]
            .car_performance
            .total_cmp(&teams[*left].car_performance)
            .then(Ordering::Equal)
    });

    // Anti-"sempre a mesma equipe": nas categorias SPEC (rookie), o carro não
    // afeta o resultado (todos idênticos na sim), então casar o melhor piloto com
    // o time de maior `car_performance` só serve pra entregar o ás pro mesmo time
    // (o de maior template) em TODO save novo — que então vence a 1ª rookie e sobe
    // a escada, roteirizando o começo de cada carreira. Embaralhando a ordem, o
    // melhor talento rookie vai pra um time aleatório e qual equipe desponta varia
    // por save. Fora da rookie o pareamento melhor-piloto↔melhor-carro é mantido.
    if tier == 0 {
        team_order.shuffle(rng);
    }

    let mut fill = GridFill::default();

    for (rank, team_index) in team_order.into_iter().enumerate() {
        let team = &mut teams[team_index];
        let n1_driver = n1_drivers
            .get(rank)
            .cloned()
            .ok_or_else(|| format!("Missing N1 driver for team {}", team.id))?;

        let assento_do_jogador = player_slot
            .as_ref()
            .filter(|slot| slot.team_id == team.id.as_str());

        team.piloto_1_id = Some(n1_driver.id.clone());
        team.hierarquia_n1_id = Some(n1_driver.id.clone());
        team.hierarquia_status = "estavel".to_string();
        team.hierarquia_tensao = 0.0;
        team.is_player_team = assento_do_jogador.is_some();

        fill.drivers.push(n1_driver.clone());
        fill.contracts.push(contract_with_team_class(
            generate_initial_contract(
                ids.next_contract_id(),
                &n1_driver.id,
                &n1_driver.nome,
                &team.id,
                &team.nome,
                TeamRole::Numero1,
                category_id,
                1,
            ),
            team,
        ));

        match assento_do_jogador {
            Some(slot) => {
                team.piloto_2_id = Some(slot.driver_id.to_string());
                // O jogador entra como Numero2 no contrato, mas a HIERARQUIA olha skill:
                // um rookie melhor que o N1 da equipe nasce por cima dele.
                if slot.skill > n1_driver.atributos.skill {
                    team.hierarquia_n1_id = Some(slot.driver_id.to_string());
                    team.hierarquia_n2_id = Some(n1_driver.id.clone());
                } else {
                    team.hierarquia_n2_id = Some(slot.driver_id.to_string());
                }
                fill.player_team_id = Some(team.id.clone());

                let contract = contract_with_team_class(
                    generate_initial_contract(
                        ids.next_contract_id(),
                        slot.driver_id,
                        slot.driver_name,
                        &team.id,
                        &team.nome,
                        TeamRole::Numero2,
                        category_id,
                        1,
                    ),
                    team,
                );
                fill.player_contract = Some(contract.clone());
                fill.contracts.push(contract);
            }
            None => {
                let n2_driver = n2_drivers
                    .next()
                    .ok_or_else(|| format!("Missing N2 driver for team {}", team.id))?;

                team.piloto_2_id = Some(n2_driver.id.clone());
                team.hierarquia_n2_id = Some(n2_driver.id.clone());

                fill.drivers.push(n2_driver.clone());
                fill.contracts.push(contract_with_team_class(
                    generate_initial_contract(
                        ids.next_contract_id(),
                        &n2_driver.id,
                        &n2_driver.nome,
                        &team.id,
                        &team.nome,
                        TeamRole::Numero2,
                        category_id,
                        1,
                    ),
                    team,
                ));
            }
        }
    }

    Ok(fill)
}
