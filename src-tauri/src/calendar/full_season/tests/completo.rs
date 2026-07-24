//! Testes do gerador 9D completo.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::calendar::full_season::{build_full_season_calendar, generate_full_season_calendar};
use crate::constants::categories::CALENDAR_CONFLICTS;
use crate::db::migrations;
use crate::db::queries::seasons::insert_season;
use crate::models::enums::{SeasonPhase, ThematicSlot};
use crate::models::season::Season;

// ── Testes básicos (seed fixo) ────────────────────────────────────────────

#[test]
fn total_de_74_entradas() {
    let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
    assert_eq!(
        entries.len(),
        74,
        "full season deve ter exatamente 74 entradas (5+5+8+8+8+10+10+14+6)"
    );
}

#[test]
fn contagem_por_categoria_correta() {
    let expected: &[(&str, usize)] = &[
        ("mazda_rookie", 5),
        ("toyota_rookie", 5),
        ("mazda_amador", 8),
        ("toyota_amador", 8),
        ("bmw_m2", 8),
        ("production_challenger", 10),
        ("gt4", 10),
        ("gt3", 14),
        ("endurance", 6),
    ];
    let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
    for &(cat, expected_count) in expected {
        let actual = entries.iter().filter(|e| e.categoria == cat).count();
        assert_eq!(
            actual, expected_count,
            "categoria {cat}: esperado {expected_count}, encontrado {actual}"
        );
    }
}

#[test]
fn todas_as_entradas_com_season_phase_temporada() {
    let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
    for entry in &entries {
        assert_eq!(
            entry.season_phase,
            SeasonPhase::Temporada,
            "entry {} ({}) tem phase {:?}",
            entry.id,
            entry.categoria,
            entry.season_phase
        );
    }
}

#[test]
fn season_week_definida_e_no_intervalo_10_a_51() {
    let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
    for entry in &entries {
        let sw = entry
            .season_week
            .unwrap_or_else(|| panic!("entry {} sem season_week", entry.id));
        assert!(
            (10..=51).contains(&sw),
            "entry {} ({}) tem season_week={sw} (fora de 10–51)",
            entry.id,
            entry.categoria
        );
    }
}

#[test]
fn thematic_slot_somente_grupo_regular() {
    let special_slots = [
        ThematicSlot::AberturaEspecial,
        ThematicSlot::RodadaEspecial,
        ThematicSlot::FinalEspecial,
    ];
    let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
    for entry in &entries {
        assert!(
            !special_slots.contains(&entry.thematic_slot),
            "entry {} ({}) tem slot especial {:?}",
            entry.id,
            entry.categoria,
            entry.thematic_slot
        );
    }
}

#[test]
fn abertura_em_sw_10_a_13() {
    let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
    let mut by_cat: HashMap<&str, Vec<_>> = HashMap::new();
    for entry in &entries {
        by_cat
            .entry(entry.categoria.as_str())
            .or_default()
            .push(entry);
    }
    for (cat, mut rounds) in by_cat {
        rounds.sort_by_key(|e| e.rodada);
        let first_sw = rounds[0].season_week.unwrap();
        assert!(
            (10..=13).contains(&first_sw),
            "categoria {cat}: abertura sw={first_sw} fora de 10–13"
        );
    }
}

#[test]
fn final_escalonado_por_prestigio() {
    // Prestígio fecha o fim de novembro (sw 48–51); as demais terminam antes.
    let prestige = ["production_challenger", "gt4", "endurance", "gt3"];
    let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
    let mut by_cat: HashMap<&str, Vec<_>> = HashMap::new();
    for entry in &entries {
        by_cat
            .entry(entry.categoria.as_str())
            .or_default()
            .push(entry);
    }
    for (cat, mut rounds) in by_cat {
        rounds.sort_by_key(|e| e.rodada);
        let last_sw = rounds.last().unwrap().season_week.unwrap();
        if prestige.contains(&cat) {
            assert!(
                (48..=51).contains(&last_sw),
                "prestígio {cat}: final sw={last_sw} fora de 48–51"
            );
        } else {
            assert!(
                (40..=47).contains(&last_sw),
                "{cat}: final sw={last_sw} deveria terminar antes do prestígio (40–47)"
            );
        }
    }
}

#[test]
fn conflict_pairs_sem_season_week_compartilhada() {
    let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
    for &(cat_a, cat_b) in &CALENDAR_CONFLICTS {
        let weeks_a: std::collections::HashSet<u32> = entries
            .iter()
            .filter(|e| e.categoria == cat_a)
            .map(|e| e.season_week.unwrap())
            .collect();
        let weeks_b: std::collections::HashSet<u32> = entries
            .iter()
            .filter(|e| e.categoria == cat_b)
            .map(|e| e.season_week.unwrap())
            .collect();
        let shared: Vec<u32> = weeks_a.intersection(&weeks_b).copied().collect();
        assert!(
            shared.is_empty(),
            "par ({cat_a}, {cat_b}) compartilha season_weeks: {shared:?}"
        );
    }
}

#[test]
fn gt3_e_endurance_final_em_semanas_distintas() {
    let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
    let gt3_final = entries
        .iter()
        .filter(|e| e.categoria == "gt3")
        .max_by_key(|e| e.rodada)
        .map(|e| e.season_week.unwrap())
        .expect("gt3 entries");
    let end_final = entries
        .iter()
        .filter(|e| e.categoria == "endurance")
        .max_by_key(|e| e.rodada)
        .map(|e| e.season_week.unwrap())
        .expect("endurance entries");
    assert_ne!(
        gt3_final, end_final,
        "gt3 e endurance não podem ter o final na mesma semana"
    );
}

#[test]
fn spacing_minimo_por_categoria() {
    let min_spacing = |cat: &str| -> u32 {
        match cat {
            "endurance" => 5,
            "gt3" => 2,
            "gt4" | "production_challenger" => 3,
            "mazda_amador" | "toyota_amador" | "bmw_m2" => 4,
            "mazda_rookie" | "toyota_rookie" => 6,
            _ => 1,
        }
    };
    let entries = build_full_season_calendar("S001", 2027, 42).unwrap();
    let mut by_cat: HashMap<&str, Vec<u32>> = HashMap::new();
    for entry in &entries {
        by_cat
            .entry(entry.categoria.as_str())
            .or_default()
            .push(entry.season_week.unwrap());
    }
    for (cat, mut weeks) in by_cat {
        weeks.sort_unstable();
        let min_sp = min_spacing(cat);
        for w in weeks.windows(2) {
            let sp = w[1] - w[0];
            assert!(
                sp >= min_sp,
                "categoria {cat}: spacing={sp} < min={min_sp} entre sw={} e sw={}",
                w[0],
                w[1]
            );
        }
    }
}

#[test]
fn determinismo_mesmo_seed() {
    let a = build_full_season_calendar("S001", 2027, 77).unwrap();
    let b = build_full_season_calendar("S001", 2027, 77).unwrap();
    assert_eq!(a.len(), b.len());
    for (ea, eb) in a.iter().zip(b.iter()) {
        assert_eq!(
            ea.track_id, eb.track_id,
            "não-determinístico: rodada {} / {}",
            ea.rodada, ea.categoria
        );
        assert_eq!(ea.categoria, eb.categoria);
        assert_eq!(ea.season_week, eb.season_week);
        assert_eq!(ea.thematic_slot, eb.thematic_slot);
    }
}

#[test]
fn seeds_diferentes_produzem_calendarios_diferentes() {
    let a = build_full_season_calendar("S001", 2027, 1).unwrap();
    let b = build_full_season_calendar("S001", 2027, 2).unwrap();
    let a_tracks: Vec<u32> = a.iter().map(|e| e.track_id).collect();
    let b_tracks: Vec<u32> = b.iter().map(|e| e.track_id).collect();
    assert_ne!(
        a_tracks, b_tracks,
        "seeds distintos devem produzir calendários distintos"
    );
}

// ── Validação de 100 seeds ────────────────────────────────────────────────

#[test]
fn validacao_100_seeds() {
    let special_slots = [
        ThematicSlot::AberturaEspecial,
        ThematicSlot::RodadaEspecial,
        ThematicSlot::FinalEspecial,
    ];
    let expected_counts: &[(&str, usize)] = &[
        ("mazda_rookie", 5),
        ("toyota_rookie", 5),
        ("mazda_amador", 8),
        ("toyota_amador", 8),
        ("bmw_m2", 8),
        ("production_challenger", 10),
        ("gt4", 10),
        ("gt3", 14),
        ("endurance", 6),
    ];

    for seed in 0u64..100 {
        let entries = build_full_season_calendar("S001", 2027, seed)
            .unwrap_or_else(|e| panic!("seed={seed} falhou: {e}"));

        // Contagem total
        assert_eq!(entries.len(), 74, "seed={seed}: total deve ser 74");

        // Contagem por categoria
        for &(cat, count) in expected_counts {
            let actual = entries.iter().filter(|e| e.categoria == cat).count();
            assert_eq!(actual, count, "seed={seed} cat={cat}");
        }

        // Todas Temporada
        for e in &entries {
            assert_eq!(e.season_phase, SeasonPhase::Temporada, "seed={seed} phase");
        }

        // Sem slots especiais
        for e in &entries {
            assert!(
                !special_slots.contains(&e.thematic_slot),
                "seed={seed}: slot especial em {}",
                e.categoria
            );
        }

        // season_week em 10–51
        for e in &entries {
            let sw = e.season_week.unwrap();
            assert!((10..=51).contains(&sw), "seed={seed} sw={sw}");
        }

        // Conflict pairs disjuntos
        for &(cat_a, cat_b) in &CALENDAR_CONFLICTS {
            let wa: std::collections::HashSet<u32> = entries
                .iter()
                .filter(|e| e.categoria == cat_a)
                .map(|e| e.season_week.unwrap())
                .collect();
            let wb: std::collections::HashSet<u32> = entries
                .iter()
                .filter(|e| e.categoria == cat_b)
                .map(|e| e.season_week.unwrap())
                .collect();
            assert!(
                wa.is_disjoint(&wb),
                "seed={seed} par ({cat_a},{cat_b}) compartilha weeks"
            );
        }

        // Abertura (10–13) e final escalonado por prestígio (topo 48–51, resto antes)
        let prestige = ["production_challenger", "gt4", "endurance", "gt3"];
        let mut by_cat: HashMap<&str, Vec<_>> = HashMap::new();
        for e in &entries {
            by_cat.entry(e.categoria.as_str()).or_default().push(e);
        }
        for (cat, mut rounds) in by_cat {
            rounds.sort_by_key(|e| e.rodada);
            let first_sw = rounds[0].season_week.unwrap();
            let last_sw = rounds.last().unwrap().season_week.unwrap();
            assert!(
                (10..=13).contains(&first_sw),
                "seed={seed} {cat}: abertura sw={first_sw}"
            );
            let final_ok = if prestige.contains(&cat) {
                (48..=51).contains(&last_sw)
            } else {
                (40..=47).contains(&last_sw)
            };
            assert!(final_ok, "seed={seed} {cat}: final sw={last_sw}");
        }

        // gt3 final ≠ endurance final
        let gt3_final = entries
            .iter()
            .filter(|e| e.categoria == "gt3")
            .max_by_key(|e| e.rodada)
            .map(|e| e.season_week.unwrap())
            .unwrap();
        let end_final = entries
            .iter()
            .filter(|e| e.categoria == "endurance")
            .max_by_key(|e| e.rodada)
            .map(|e| e.season_week.unwrap())
            .unwrap();
        assert_ne!(
            gt3_final, end_final,
            "seed={seed}: gt3 e endurance final iguais"
        );
    }
}

// ── Testes de DB ──────────────────────────────────────────────────────────

#[test]
fn db_round_trip_conta_74() {
    let conn = Connection::open_in_memory().unwrap();
    migrations::run_all(&conn).unwrap();
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();

    let inserted = generate_full_season_calendar(&conn, "S001", 2027, 42).unwrap();
    assert_eq!(inserted, 74);

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calendar WHERE COALESCE(season_id,temporada_id)='S001'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 74);
}

#[test]
fn db_round_trip_season_week_nao_nulo() {
    let conn = Connection::open_in_memory().unwrap();
    migrations::run_all(&conn).unwrap();
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();
    generate_full_season_calendar(&conn, "S001", 2027, 42).unwrap();

    let non_null: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001' AND season_week IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        non_null, 74,
        "todas as 74 entradas devem ter season_week definido"
    );
}

#[test]
fn db_round_trip_season_phase_temporada() {
    let conn = Connection::open_in_memory().unwrap();
    migrations::run_all(&conn).unwrap();
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();
    generate_full_season_calendar(&conn, "S001", 2027, 42).unwrap();

    let non_temporada: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001' AND season_phase != 'Temporada'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        non_temporada, 0,
        "todas as entradas devem ter season_phase='Temporada'"
    );
}

#[test]
fn db_round_trip_ids_unicos() {
    let conn = Connection::open_in_memory().unwrap();
    migrations::run_all(&conn).unwrap();
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();
    generate_full_season_calendar(&conn, "S001", 2027, 42).unwrap();

    let distinct: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT id) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(distinct, 74, "todos os 74 IDs devem ser únicos");
}

#[test]
fn db_season_week_consistente_com_week_of_year() {
    let conn = Connection::open_in_memory().unwrap();
    migrations::run_all(&conn).unwrap();
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2027)).unwrap();
    generate_full_season_calendar(&conn, "S001", 2027, 42).unwrap();

    // Para a janela de corridas: season_week = week_of_year + 4
    let inconsistentes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calendar \
                 WHERE COALESCE(season_id,temporada_id)='S001' \
                   AND season_week IS NOT NULL \
                   AND season_week != week_of_year + 4",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        inconsistentes, 0,
        "season_week deve ser exatamente week_of_year+4 para todas as entradas"
    );
}
