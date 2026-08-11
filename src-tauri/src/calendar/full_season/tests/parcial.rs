//! Testes do gerador parcial (Etapa 10).

use rusqlite::Connection;

use crate::calendar::full_season::parcial::partial_compute_count;
use crate::calendar::full_season::{
    build_partial_special_divisions, generate_partial_special_divisions,
};
use crate::db::migrations;
use crate::db::queries::seasons::insert_season;
use crate::models::season::Season;

#[test]
fn partial_degrada_regra_descrita() {
    // (a) alvo + canônico cabe: span=42, target=10, min_spacing=3 → 3*9=27 ≤ 42
    assert_eq!(partial_compute_count(5, 47, 10, 3, 3, 2), (10, false));
    // (b) alvo falha, mínimo + canônico cabe: span=8, prod: 3*9=27>8 falha; 3*2=6≤8 ok
    assert_eq!(partial_compute_count(39, 47, 10, 3, 3, 2), (3, false));
    // (c) alvo+canônico falha, mínimo+canônico falha, mínimo+relaxado cabe: end min=2, relax=3
    //     span=3, end: 5*(2-1)=5>3 falha; 3*(2-1)=3≤3 ok
    assert_eq!(partial_compute_count(42, 45, 6, 2, 5, 3), (2, false));
    // (d) nem mínimo com relaxado cabe → o que couber, com aviso
    //     span=1, end: 3*(2-1)=3>1 falha; max=1+1/3=1; warn=true
    assert_eq!(partial_compute_count(44, 45, 6, 2, 5, 3), (1, true));
    // janela negativa → 0 rounds
    assert_eq!(partial_compute_count(50, 45, 6, 2, 5, 3), (0, true));
}

#[test]
fn partial_inicio_normal_gera_alvo_completo() {
    // from_sw=10: semana de abertura; janela completa → 10 prod + 6 end
    let entries = build_partial_special_divisions("S001", 2027, 10, 42).unwrap();
    let prod = entries
        .iter()
        .filter(|e| e.categoria == "production_challenger")
        .count();
    let end = entries
        .iter()
        .filter(|e| e.categoria == "endurance")
        .count();
    assert_eq!(prod, 10, "janela completa: production deve ter 10");
    assert_eq!(end, 6, "janela completa: endurance deve ter 6");
}

#[test]
fn partial_meio_do_ano_degrada() {
    // from_sw=40: janela reduzida. prod span=44-(40-4)=44-36=8 → 3*9=27>8 → tenta mín
    // prod mín 3: 3*2=6≤8 ok → 3 rounds.
    // end span=46-(40-4)=46-36=10 → 5*5=25>10 → tenta mín
    // end mín 2: 5*1=5≤10 ok → 2 rounds.
    let entries = build_partial_special_divisions("S001", 2027, 40, 99).unwrap();
    let prod = entries
        .iter()
        .filter(|e| e.categoria == "production_challenger")
        .count();
    let end = entries
        .iter()
        .filter(|e| e.categoria == "endurance")
        .count();
    assert_eq!(prod, 3, "from_sw=40: production deve ter 3 (mínimo)");
    assert_eq!(end, 2, "from_sw=40: endurance deve ter 2 (mínimo)");
}

#[test]
fn partial_fim_do_ano_gera_menos_que_minimo() {
    // from_sw=49: end span = 46-(49-4)=46-45=1 → nem o mínimo relaxado cabe → max=1+1/3=1.
    // prod span = 44-(49-4)=44-45<0 → janela vencida → 0 rounds.
    let entries = build_partial_special_divisions("S001", 2027, 49, 1).unwrap();
    let end = entries
        .iter()
        .filter(|e| e.categoria == "endurance")
        .count();
    assert_eq!(end, 1, "from_sw=49: endurance deve ter 1 rodada no máximo");
    // prod já passou do woy_end canônico (44 = sw 48)
    let prod = entries
        .iter()
        .filter(|e| e.categoria == "production_challenger")
        .count();
    assert_eq!(prod, 0, "from_sw=49: production já saiu da janela canônica");
}

#[test]
fn partial_alem_da_janela_end_zero_entries() {
    // from_sw=51: end span = 46-(51-4)=46-47<0 → 0 rounds de endurance
    let entries = build_partial_special_divisions("S001", 2027, 51, 7).unwrap();
    let end = entries
        .iter()
        .filter(|e| e.categoria == "endurance")
        .count();
    assert_eq!(end, 0, "from_sw>50: endurance deve ter 0 entradas");
}

#[test]
fn partial_season_week_em_range_valido() {
    for from_sw in [10u32, 20, 30, 40, 48] {
        let entries = build_partial_special_divisions("S001", 2027, from_sw, from_sw as u64)
            .unwrap_or_else(|e| panic!("from_sw={from_sw} falhou: {e}"));
        for entry in &entries {
            let sw = entry.season_week.unwrap();
            let max_sw = if entry.categoria == "endurance" {
                50
            } else {
                48
            };
            assert!(
                sw >= from_sw && sw <= max_sw,
                "from_sw={from_sw} {}: sw={sw} fora de [{from_sw},{max_sw}]",
                entry.categoria
            );
        }
    }
}

#[test]
fn partial_season_phase_temporada() {
    let entries = build_partial_special_divisions("S001", 2027, 10, 42).unwrap();
    for entry in &entries {
        assert_eq!(
            entry.season_phase,
            crate::models::enums::SeasonPhase::Temporada,
            "entry {} tem phase {:?}",
            entry.id,
            entry.season_phase
        );
    }
}

#[test]
fn partial_slots_somente_regulares() {
    let special = [
        crate::models::enums::ThematicSlot::AberturaEspecial,
        crate::models::enums::ThematicSlot::RodadaEspecial,
        crate::models::enums::ThematicSlot::FinalEspecial,
    ];
    let entries = build_partial_special_divisions("S001", 2027, 10, 42).unwrap();
    for entry in &entries {
        assert!(
            !special.contains(&entry.thematic_slot),
            "entry {} tem slot especial {:?}",
            entry.id,
            entry.thematic_slot
        );
    }
}

#[test]
fn partial_sem_lmp2() {
    let entries = build_partial_special_divisions("S001", 2027, 10, 42).unwrap();
    assert!(
        entries.iter().all(|e| e.categoria != "lmp2"),
        "partial não deve gerar lmp2"
    );
}

#[test]
fn partial_determinismo() {
    let a = build_partial_special_divisions("S001", 2027, 15, 77).unwrap();
    let b = build_partial_special_divisions("S001", 2027, 15, 77).unwrap();
    assert_eq!(a.len(), b.len(), "determinismo: tamanhos diferentes");
    for (ea, eb) in a.iter().zip(b.iter()) {
        assert_eq!(ea.track_id, eb.track_id, "determinismo: track_id diferente");
        assert_eq!(
            ea.season_week, eb.season_week,
            "determinismo: season_week diferente"
        );
        assert_eq!(
            ea.thematic_slot, eb.thematic_slot,
            "determinismo: slot diferente"
        );
    }
}

#[test]
fn partial_seeds_diferentes_calendarios_diferentes() {
    let a = build_partial_special_divisions("S001", 2027, 10, 1).unwrap();
    let b = build_partial_special_divisions("S001", 2027, 10, 2).unwrap();
    let a_tracks: Vec<u32> = a.iter().map(|e| e.track_id).collect();
    let b_tracks: Vec<u32> = b.iter().map(|e| e.track_id).collect();
    assert_ne!(
        a_tracks, b_tracks,
        "seeds distintos devem gerar calendários distintos"
    );
}

#[test]
fn partial_db_insere_e_ids_unicos() {
    let conn = Connection::open_in_memory().unwrap();
    migrations::run_all(&conn).unwrap();
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();

    let inserted = generate_partial_special_divisions(&conn, "S001", 2027, 10, 42).unwrap();
    assert_eq!(
        inserted, 16,
        "from_sw=10 deve inserir 16 entradas (10 prod + 6 end)"
    );

    let distinct: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT id) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(distinct, 16, "todos os IDs devem ser únicos");
}

#[test]
fn partial_db_season_week_nao_nulo() {
    let conn = Connection::open_in_memory().unwrap();
    migrations::run_all(&conn).unwrap();
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();
    generate_partial_special_divisions(&conn, "S001", 2027, 10, 42).unwrap();

    let nulos: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001' AND season_week IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(nulos, 0, "nenhuma entrada deve ter season_week NULL");
}
