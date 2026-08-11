//! Sorteio de nome, genero e identidade completa do piloto.

use std::collections::HashSet;

use rand::Rng;

use super::pool::{get_name_pool, NAME_POOLS};
use crate::generators::nationality::{format_nationality, random_nationality};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PilotIdentity {
    pub nome_completo: String,
    pub primeiro_nome: String,
    pub sobrenome: String,
    pub nacionalidade_id: String,
    pub nacionalidade_label: String,
    pub genero: String,
}

pub fn generate_name(nationality_id: &str, genero: &str, rng: &mut impl Rng) -> (String, String) {
    let pool = get_name_pool(nationality_id).unwrap_or(&NAME_POOLS[0]);
    let first_names = if genero.eq_ignore_ascii_case("F") && !pool.nomes_femininos.is_empty() {
        pool.nomes_femininos
    } else {
        pool.nomes_masculinos
    };

    let first_name = first_names[rng.gen_range(0..first_names.len())].to_string();
    let last_name = pool.sobrenomes[rng.gen_range(0..pool.sobrenomes.len())].to_string();
    (first_name, last_name)
}

pub fn generate_unique_name(
    nationality_id: &str,
    genero: &str,
    existing_names: &HashSet<String>,
    rng: &mut impl Rng,
) -> (String, String) {
    for _ in 0..50 {
        let (first_name, last_name) = generate_name(nationality_id, genero, rng);
        let full_name = format!("{} {}", first_name, last_name);
        if !existing_names.contains(&full_name) {
            return (first_name, last_name);
        }
    }

    let pool = get_name_pool(nationality_id).unwrap_or(&NAME_POOLS[0]);
    let first_names = if genero.eq_ignore_ascii_case("F") && !pool.nomes_femininos.is_empty() {
        pool.nomes_femininos
    } else {
        pool.nomes_masculinos
    };

    for first_name in first_names {
        for last_name in pool.sobrenomes {
            let full_name = format!("{} {}", first_name, last_name);
            if !existing_names.contains(&full_name) {
                return ((*first_name).to_string(), (*last_name).to_string());
            }
        }
    }

    let base_first = first_names[0].to_string();
    let base_last = pool.sobrenomes[0];
    let mut suffix = 2_u32;
    loop {
        let forced_last = format!("{} {}", base_last, suffix);
        let full_name = format!("{} {}", base_first, forced_last);
        if !existing_names.contains(&full_name) {
            return (base_first.clone(), forced_last);
        }
        suffix += 1;
    }
}

pub fn random_gender(rng: &mut impl Rng) -> &'static str {
    if rng.gen_ratio(1, 20) {
        "F"
    } else {
        "M"
    }
}

/// Identidade completa de um piloto novo.
///
/// `nacionalidade_label` nasce SEMPRE em pt-BR, e isso é decisão, não esquecimento: ele é
/// o TOKEN que vai para `drivers.nacionalidade` no banco, no mesmo desenho do `pais` cru
/// de `constants/tracks`. Quem escolhe o idioma é a tela, por
/// [`crate::generators::nationality::nationality_display_label`], que resolve o token no
/// locale ativo. Gravar no locale da geração congelaria o rótulo: um piloto criado com o
/// jogo em inglês ficaria em inglês para sempre, mesmo depois de o jogador voltar ao
/// português.
pub fn generate_pilot_identity(
    existing_names: &HashSet<String>,
    rng: &mut impl Rng,
) -> PilotIdentity {
    let nationality = random_nationality(rng);
    let genero = random_gender(rng);
    let (primeiro_nome, sobrenome) =
        generate_unique_name(nationality.id, genero, existing_names, rng);
    let nome_completo = format!("{} {}", primeiro_nome, sobrenome);

    PilotIdentity {
        nome_completo,
        primeiro_nome,
        sobrenome,
        nacionalidade_id: nationality.id.to_string(),
        nacionalidade_label: format_nationality(nationality.id, genero, "pt-BR"),
        genero: genero.to_string(),
    }
}
