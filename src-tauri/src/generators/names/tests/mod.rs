use std::collections::HashSet;

use rand::{rngs::StdRng, SeedableRng};

use super::*;
use crate::generators::nationality::get_all_nationalities;

#[test]
fn test_generate_name_returns_nonempty() {
    let mut rng = StdRng::seed_from_u64(11);
    let (first_name, last_name) = generate_name("br", "M", &mut rng);
    assert!(!first_name.is_empty());
    assert!(!last_name.is_empty());
}

#[test]
fn test_generate_unique_name_no_collision() {
    let mut rng = StdRng::seed_from_u64(22);
    let mut existing = HashSet::new();

    for _ in 0..50 {
        let (first_name, last_name) = generate_unique_name("gb", "M", &existing, &mut rng);
        let full_name = format!("{} {}", first_name, last_name);
        assert!(existing.insert(full_name));
    }
}

#[test]
fn test_random_gender_distribution() {
    let mut rng = StdRng::seed_from_u64(33);
    let mut female_count = 0;
    for _ in 0..1000 {
        if random_gender(&mut rng) == "F" {
            female_count += 1;
        }
    }

    assert!((20..=100).contains(&female_count));
}

#[test]
fn test_generate_pilot_identity_complete() {
    let mut rng = StdRng::seed_from_u64(44);
    let existing = HashSet::new();
    let identity = generate_pilot_identity(&existing, &mut rng);

    assert!(!identity.nome_completo.is_empty());
    assert!(!identity.primeiro_nome.is_empty());
    assert!(!identity.sobrenome.is_empty());
    assert!(!identity.nacionalidade_id.is_empty());
    assert!(!identity.nacionalidade_label.is_empty());
    assert!(identity.genero == "M" || identity.genero == "F");
}

#[test]
fn test_all_nationalities_have_name_pools() {
    for nationality in get_all_nationalities() {
        assert!(
            get_name_pool(nationality.id).is_some(),
            "missing pool for {}",
            nationality.id
        );
    }
}

#[test]
fn test_name_pools_minimum_sizes() {
    for pool in get_all_name_pools() {
        assert!(pool.nomes_masculinos.len() >= 15, "{}", pool.nationality_id);
        assert!(pool.nomes_femininos.len() >= 4, "{}", pool.nationality_id);
        assert!(pool.sobrenomes.len() >= 15, "{}", pool.nationality_id);
    }
}

#[test]
fn test_generate_200_unique_pilots() {
    let mut rng = StdRng::seed_from_u64(55);
    let mut existing = HashSet::new();

    for _ in 0..200 {
        let identity = generate_pilot_identity(&existing, &mut rng);
        assert!(existing.insert(identity.nome_completo));
    }
}
