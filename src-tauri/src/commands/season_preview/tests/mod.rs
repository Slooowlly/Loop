use super::*;

#[test]
fn percentil_ordena_do_pior_ao_melhor() {
    let all = vec![10.0, 20.0, 30.0, 40.0];
    assert!(percentile(40.0, &all) > percentile(10.0, &all));
    assert_eq!(percentile(40.0, &all), 1.0);
}

#[test]
fn jitter_e_deterministico_e_limitado() {
    let a = jitter("P123");
    assert_eq!(a, jitter("P123"), "mesmo id → mesmo ruído");
    assert!(a.abs() <= JITTER_RANGE / 2.0 + 1e-9);
    assert_ne!(jitter("P123"), jitter("P124"));
}

/// O CURRÍCULO tem que pesar mais que o skill oculto: um piloto com pódio precisa
/// ficar à frente de outro sem resultado, mesmo com skill bem menor. É a assimetria
/// de informação do design (§5.2) — a imprensa lê resultado, não ritmo.
#[test]
fn podio_pesa_mais_que_skill_na_percepcao() {
    let com_podio = W_PODIUM * 1.0 + W_SKILL_HINT * 40.0;
    let so_skill = W_SKILL_HINT * 50.0;
    assert!(
        com_podio > so_skill + JITTER_RANGE,
        "um pódio deve superar a vantagem de skill mesmo no pior caso de ruído"
    );
}

#[test]
fn tese_de_estreantes_vence_quando_grid_e_novato() {
    assert!(matches!(
        select_thesis(0.8, &Material::Uniform, false),
        Thesis::RookieSeason
    ));
    assert!(matches!(
        select_thesis(0.1, &Material::Uniform, true),
        Thesis::VacantThrone
    ));
    assert!(matches!(
        select_thesis(0.1, &Material::Uniform, false),
        Thesis::OpenOnTalent
    ));
}
