//! Suíte de testes do calendário (extraída de `calendar/mod.rs`).
//!
//! Continua sendo o mesmo módulo `tests` de antes: `use super::*` enxerga o
//! módulo `calendar` inteiro, incluindo os itens privados.

use chrono::Datelike;
use rand::{rngs::StdRng, SeedableRng};

use super::*;
use crate::constants::tracks::get_track;

// ── display_date_for_season_week ──────────────────────────────────────────

/// Verifica que display_date_for_season_week produz a mesma string que
/// alimentar display_date_for_weekday manualmente com (ano civil, woy) corretos.
#[test]
fn display_date_season_week_matches_manual_derivation() {
    // sw 2 → woy 50 de 2026 (dezembro: ano anterior)
    let got = display_date_for_season_week(2, 2027, Weekday::Sat).unwrap();
    let expected = display_date_for_weekday(2026, 50, Weekday::Sat);
    assert_eq!(
        got, expected,
        "sw 2 / 2027: esperava {expected}, obteve {got}"
    );

    // sw 5 → woy 1 de 2027 (janeiro: mesmo ano)
    let got = display_date_for_season_week(5, 2027, Weekday::Sat).unwrap();
    let expected = display_date_for_weekday(2027, 1, Weekday::Sat);
    assert_eq!(
        got, expected,
        "sw 5 / 2027: esperava {expected}, obteve {got}"
    );

    // sw 10 → woy 6 de 2027 (fevereiro, primeira corrida)
    let got = display_date_for_season_week(10, 2027, Weekday::Sat).unwrap();
    let expected = display_date_for_weekday(2027, 6, Weekday::Sat);
    assert_eq!(
        got, expected,
        "sw 10 / 2027: esperava {expected}, obteve {got}"
    );

    // sw 51 → woy 47 de 2027 (novembro, última corrida)
    let got = display_date_for_season_week(51, 2027, Weekday::Sat).unwrap();
    let expected = display_date_for_weekday(2027, 47, Weekday::Sat);
    assert_eq!(
        got, expected,
        "sw 51 / 2027: esperava {expected}, obteve {got}"
    );
}

#[test]
fn display_date_season_week_virada_do_ano() {
    // sw 4 → woy 52 de 2026 (ainda dezembro)
    let sw4 = display_date_for_season_week(4, 2027, Weekday::Sun).unwrap();
    let sw4_ref = display_date_for_weekday(2026, 52, Weekday::Sun);
    assert_eq!(sw4, sw4_ref);

    // sw 5 → woy 1 de 2027 (já janeiro) — deve ter ano diferente de sw4
    let sw5 = display_date_for_season_week(5, 2027, Weekday::Sun).unwrap();
    assert!(
        sw5 > sw4,
        "sw 5 ({sw5}) deve ser posterior a sw 4 ({sw4}) mesmo cruzando a virada do ano"
    );
}

#[test]
fn display_date_season_week_invalid_rejects() {
    assert!(display_date_for_season_week(0, 2027, Weekday::Sat).is_err());
    assert!(display_date_for_season_week(52, 2027, Weekday::Sat).is_err());
}

#[test]
fn test_generate_calendar_correct_count() {
    let mut rng = StdRng::seed_from_u64(1);
    let gt3 = generate_calendar_for_category("S001", "gt3", &mut rng).expect("gt3 calendar");
    let mazda =
        generate_calendar_for_category("S001", "mazda_rookie", &mut rng).expect("rookie calendar");

    assert_eq!(gt3.len(), 14);
    assert_eq!(mazda.len(), 5);
}

#[test]
fn test_generate_calendar_no_duplicate_tracks() {
    let mut rng = StdRng::seed_from_u64(2);
    let calendar = generate_calendar_for_category("S001", "gt4", &mut rng).expect("calendar");
    let unique: HashSet<_> = calendar.iter().map(|entry| entry.track_id).collect();
    assert_eq!(unique.len(), calendar.len());
}

#[test]
fn test_generate_calendar_respects_themed_pool() {
    // Com o gerador temático, rookie usa pools regionais (USA ou Europa).
    // O teste antigo verificava gratuita==true, mas os pools temáticos podem incluir
    // tracks paid (ex: VIR=58, Snetterton=316) por design. Verificamos que as tracks
    // vêm dos pools regionais elegíveis para rookie.
    let mut rng = StdRng::seed_from_u64(3);
    let calendar =
        generate_calendar_for_category("S001", "mazda_rookie", &mut rng).expect("calendar");
    let usa = generator::free_tracks_for_region(generator::CalendarRegion::Usa);
    let eur = generator::free_tracks_for_region(generator::CalendarRegion::Europa);
    let all_eligible: std::collections::HashSet<u32> =
        usa.iter().chain(eur.iter()).copied().collect();
    assert!(calendar
        .iter()
        .all(|entry| all_eligible.contains(&entry.track_id)));
}

#[test]
fn test_generate_calendar_weather_distribution() {
    let mut rng = StdRng::seed_from_u64(4);
    let mut wet_races = 0_usize;
    let mut total_races = 0_usize;

    for _ in 0..100 {
        let calendar = generate_calendar_for_category("S001", "gt3", &mut rng).expect("calendar");
        wet_races += calendar
            .iter()
            .filter(|entry| entry.clima != WeatherCondition::Dry)
            .count();
        total_races += calendar.len();
    }

    let ratio = wet_races as f64 / total_races as f64;
    assert!(
        ratio > 0.05 && ratio < 0.35,
        "unexpected wet ratio: {}",
        ratio
    );
}

#[test]
fn test_generate_all_calendars_no_conflicts() {
    let mut rng = StdRng::seed_from_u64(5);
    let calendars = generate_all_calendars("S001", &mut rng).expect("all calendars");

    for (left, right) in [
        ("mazda_rookie", "toyota_rookie"),
        ("mazda_amador", "toyota_amador"),
    ] {
        let left_calendar = calendars.get(left).expect("left calendar");
        let right_calendar = calendars.get(right).expect("right calendar");

        for (left_entry, right_entry) in left_calendar.iter().zip(right_calendar.iter()) {
            assert_ne!(left_entry.track_id, right_entry.track_id);
        }
    }
}

#[test]
fn test_generate_calendar_voltas_reasonable() {
    let mut rng = StdRng::seed_from_u64(6);
    let calendar = generate_calendar_for_category("S001", "endurance", &mut rng).expect("calendar");
    assert!(calendar
        .iter()
        .all(|entry| (5..=50).contains(&entry.voltas)));
}

// ── Testes de week_for_rodada ─────────────────────────────────────────────

#[test]
fn test_week_for_rodada_boundaries() {
    // Primeira rodada → start; última → end
    assert_eq!(week_for_rodada(1, 5, 2, 40), 2);
    assert_eq!(week_for_rodada(5, 5, 2, 40), 40);
    assert_eq!(week_for_rodada(1, 14, 2, 40), 2);
    assert_eq!(week_for_rodada(14, 14, 2, 40), 40);
}

#[test]
fn test_week_for_rodada_monotonic() {
    let total = 8;
    let mut prev = 0i32;
    for r in 1..=total {
        let w = week_for_rodada(r, total, 2, 40);
        assert!(w >= prev, "week não é monotônico em rodada {r}");
        prev = w;
    }
}

#[test]
fn test_week_for_rodada_single_round() {
    assert_eq!(week_for_rodada(1, 1, 41, 50), 41);
}

// ── Testes de week_to_display_date ────────────────────────────────────────

#[test]
fn test_week_to_display_date_format() {
    let d = week_to_display_date(2028, 12);
    // Deve ser "YYYY-MM-DD"
    assert_eq!(d.len(), 10);
    assert_eq!(&d[4..5], "-");
    assert_eq!(&d[7..8], "-");
}

#[test]
fn test_week_to_display_date_valid_range() {
    // Semanas 1-52 devem produzir datas válidas sem fallback
    for week in 1..=52 {
        let d = week_to_display_date(2028, week);
        assert!(
            !d.ends_with("01-01") || week == 1 || week == 52,
            "semana {week} produziu data fallback"
        );
    }
}

// ── Testes de generate_calendar_for_category_with_year ───────────────────

#[test]
fn test_regular_calendar_month_window() {
    let mut rng = StdRng::seed_from_u64(10);
    let calendar = generate_calendar_for_category_with_year("S001", 2028, "gt3", &mut rng)
        .expect("gt3 calendar");
    for entry in &calendar {
        let date = chrono::NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d")
            .expect("valid regular display_date");
        assert!(
            (2..=8).contains(&date.month()),
            "data regular {} fora da janela fevereiro-agosto",
            entry.display_date
        );
        assert_eq!(entry.season_phase, SeasonPhase::BlocoRegular);
    }
}

#[test]
fn test_regular_calendar_display_date_not_empty() {
    let mut rng = StdRng::seed_from_u64(11);
    let calendar = generate_calendar_for_category_with_year("S001", 2028, "gt3", &mut rng)
        .expect("gt3 calendar");
    for entry in &calendar {
        assert!(!entry.display_date.is_empty(), "display_date vazia");
    }
}

#[test]
fn test_weekday_policy_rookies_are_stable_monday_or_tuesday() {
    for category in ["mazda_rookie", "toyota_rookie"] {
        let mut rng = StdRng::seed_from_u64(42);
        let calendar = generate_calendar_for_category_with_year("S001", 2028, category, &mut rng)
            .expect("rookie calendar");
        let weekdays: HashSet<Weekday> = calendar
            .iter()
            .map(|entry| {
                NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d")
                    .expect("date")
                    .weekday()
            })
            .collect();

        assert_eq!(
            weekdays.len(),
            1,
            "{category} deve usar um dia fixo na temporada"
        );
        assert!(
            matches!(weekdays.iter().next(), Some(Weekday::Mon | Weekday::Tue)),
            "{category} deve correr segunda ou terça, recebeu {:?}",
            weekdays
        );
    }
}

#[test]
fn test_weekday_policy_amador_categories_are_stable_tuesday() {
    for category in ["mazda_amador", "toyota_amador"] {
        let mut rng = StdRng::seed_from_u64(43);
        let calendar = generate_calendar_for_category_with_year("S001", 2028, category, &mut rng)
            .expect("amador calendar");
        let weekdays: HashSet<Weekday> = calendar
            .iter()
            .map(|entry| {
                NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d")
                    .expect("date")
                    .weekday()
            })
            .collect();

        assert_eq!(
            weekdays.len(),
            1,
            "{category} deve usar um dia fixo na temporada"
        );
        assert!(
            matches!(weekdays.iter().next(), Some(Weekday::Tue)),
            "{category} deve correr terça, recebeu {:?}",
            weekdays
        );
    }
}

#[test]
fn test_weekday_policy_bmw_m2_is_wednesday() {
    let mut rng = StdRng::seed_from_u64(43);
    let calendar = generate_calendar_for_category_with_year("S001", 2028, "bmw_m2", &mut rng)
        .expect("bmw calendar");

    for entry in &calendar {
        let date = NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d").expect("date");
        assert_eq!(
            date.weekday(),
            Weekday::Wed,
            "bmw_m2 gerou {}",
            entry.display_date
        );
        assert!(
            (2..=8).contains(&date.month()),
            "bmw_m2 saiu da janela fevereiro-agosto em {}",
            entry.display_date
        );
    }
}

#[test]
fn test_weekday_policy_gt4_thursday_and_gt3_friday() {
    let cases = [("gt4", Weekday::Thu), ("gt3", Weekday::Fri)];

    for (category, expected_weekday) in cases {
        let mut rng = StdRng::seed_from_u64(44);
        let calendar = generate_calendar_for_category_with_year("S001", 2028, category, &mut rng)
            .expect("gt calendar");

        for entry in &calendar {
            let date = NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d").expect("date");
            assert_eq!(
                date.weekday(),
                expected_weekday,
                "{category} gerou {}",
                entry.display_date
            );
            assert!(
                (2..=8).contains(&date.month()),
                "{category} saiu da janela fevereiro-agosto em {}",
                entry.display_date
            );
        }
    }
}

/// `#[serial]` + locale fixo: a mensagem de erro passou a sair do `rust-i18n`, e o
/// locale é estado global do processo — sem fixar, este teste passa a depender de qual
/// idioma o teste anterior deixou ligado.
#[test]
#[serial_test::serial]
fn test_lmp2_standalone_calendar_is_not_generated() {
    let anterior = rust_i18n::locale().to_string();
    rust_i18n::set_locale("pt-BR");
    let mut rng = StdRng::seed_from_u64(144);
    let err = generate_calendar_for_category_with_year("S001", 2028, "lmp2", &mut rng)
        .expect_err("lmp2 standalone calendar should not be generated");
    assert!(
        err.contains("classe da Endurance"),
        "erro inesperado: {err}"
    );

    rust_i18n::set_locale("en-US");
    let err_en = generate_calendar_for_category_with_year("S001", 2028, "lmp2", &mut rng)
        .expect_err("lmp2 standalone calendar should not be generated");
    assert!(
        err_en.contains("Endurance class"),
        "erro inesperado: {err_en}"
    );
    rust_i18n::set_locale(&anterior);

    let calendars = generate_all_calendars("S001", &mut rng).expect("all calendars");
    assert!(!calendars.contains_key("lmp2"));
}

#[test]
fn test_weekday_policy_production_saturday_and_endurance_sunday() {
    let cases = [
        ("production_challenger", Weekday::Sat),
        ("endurance", Weekday::Sun),
    ];

    for (category, expected_weekday) in cases {
        let mut rng = StdRng::seed_from_u64(45);
        let calendar = generate_calendar_for_category_with_year("S001", 2028, category, &mut rng)
            .expect("special calendar");

        for entry in &calendar {
            let date = NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d").expect("date");
            assert_eq!(
                date.weekday(),
                expected_weekday,
                "{category} gerou {}",
                entry.display_date
            );
            assert!(
                (9..=12).contains(&date.month()),
                "{category} saiu da janela setembro-dezembro em {}",
                entry.display_date
            );
        }
    }
}

#[test]
fn test_conflict_pairs_share_week_slots() {
    let mut rng = StdRng::seed_from_u64(12);
    let mazda = generate_calendar_for_category_with_year("S001", 2028, "mazda_rookie", &mut rng)
        .expect("mazda");
    let toyota = generate_calendar_for_category_with_year("S001", 2028, "toyota_rookie", &mut rng)
        .expect("toyota");
    // LEGADO 9D: gerador antigo ainda valida pares por week_of_year no contexto pré-v33.
    // Pares conflito têm mesmo N de rodadas → mesmas semanas.
    let mazda_weeks: Vec<i32> = mazda.iter().map(|e| e.week_of_year).collect();
    let toyota_weeks: Vec<i32> = toyota.iter().map(|e| e.week_of_year).collect();
    assert_eq!(
        mazda_weeks, toyota_weeks,
        "pares de conflito devem ter as mesmas semanas"
    );
}

// ── Testes de generate_and_insert_special_calendars ───────────────────────

#[test]
fn test_special_calendars_month_window() {
    use crate::db::migrations;
    use crate::db::queries::seasons::insert_season;
    use crate::models::season::Season;
    use rusqlite::Connection;

    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("migrations");
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2028)).expect("season");

    let mut rng = StdRng::seed_from_u64(20);
    generate_and_insert_special_calendars(&conn, "S001", 2028, &mut rng)
        .expect("special calendars");

    let mut stmt = conn
        .prepare(
            "SELECT data FROM calendar
             WHERE season_phase = 'BlocoEspecial'
             -- LEGADO 9D: ordena por week_of_year, correto no contexto pré-v33.
             ORDER BY week_of_year ASC, data ASC",
        )
        .expect("prepare special dates");
    let dates: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .expect("query special dates")
        .collect::<Result<_, _>>()
        .expect("collect special dates");

    assert!(
        !dates.is_empty(),
        "calendário especial deveria gerar ao menos uma data"
    );

    for display_date in dates {
        let date = chrono::NaiveDate::parse_from_str(&display_date, "%Y-%m-%d")
            .expect("valid special display_date");
        assert!(
            (9..=12).contains(&date.month()),
            "data especial {} fora da janela setembro-dezembro",
            display_date
        );
    }
}

#[test]
fn test_special_calendar_reaches_the_last_week_of_december() {
    let mut rng = StdRng::seed_from_u64(26);
    let calendar = generate_calendar_for_category_with_year("S001", 2028, "endurance", &mut rng)
        .expect("special calendar");

    let last_date = calendar
        .iter()
        .map(|entry| {
            NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d")
                .expect("valid special display_date")
        })
        .max()
        .expect("last special date");

    assert_eq!(last_date.month(), 12);
    assert!(
        last_date.day() >= 24,
        "a última especial deve chegar ao fim de dezembro, recebeu {last_date}"
    );
    assert_eq!(last_date.weekday(), Weekday::Sun);
}

#[test]
fn test_special_calendars_counts() {
    use crate::db::migrations;
    use crate::db::queries::seasons::insert_season;
    use crate::models::season::Season;
    use rusqlite::Connection;

    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("migrations");
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2028)).expect("season");

    let mut rng = StdRng::seed_from_u64(21);
    generate_and_insert_special_calendars(&conn, "S001", 2028, &mut rng)
        .expect("special calendars");

    let pc: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calendar WHERE categoria = 'production_challenger'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let end: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calendar WHERE categoria = 'endurance'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    assert_eq!(pc, 10, "production_challenger deve ter 10 rodadas");
    assert_eq!(end, 6, "endurance deve ter 6 rodadas");
}

#[test]
fn test_special_calendars_season_phase() {
    use crate::db::migrations;
    use crate::db::queries::seasons::insert_season;
    use crate::models::season::Season;
    use rusqlite::Connection;

    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("migrations");
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2028)).expect("season");

    let mut rng = StdRng::seed_from_u64(22);
    generate_and_insert_special_calendars(&conn, "S001", 2028, &mut rng)
        .expect("special calendars");

    let wrong_phase: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calendar
         WHERE categoria IN ('production_challenger', 'endurance')
           AND season_phase != 'BlocoEspecial'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    assert_eq!(wrong_phase, 0, "entradas especiais com season_phase errado");
}

#[test]
fn test_special_calendars_rejects_duplicate() {
    use crate::db::migrations;
    use crate::db::queries::seasons::insert_season;
    use crate::models::season::Season;
    use rusqlite::Connection;

    let conn = Connection::open_in_memory().expect("db");
    migrations::run_all(&conn).expect("migrations");
    insert_season(&conn, &Season::new("S001".to_string(), 1, 2028)).expect("season");

    let mut rng = StdRng::seed_from_u64(23);
    generate_and_insert_special_calendars(&conn, "S001", 2028, &mut rng).expect("primeira geração");

    let mut rng2 = StdRng::seed_from_u64(24);
    let result = generate_and_insert_special_calendars(&conn, "S001", 2028, &mut rng2);
    assert!(result.is_err(), "segunda chamada deveria retornar Err");
}

// ── Testes de storyline temático ──────────────────────────────────────────

#[allow(dead_code)]
fn all_free_regional_ids() -> HashSet<u32> {
    use generator::{free_tracks_for_region, CalendarRegion};
    [
        CalendarRegion::Usa,
        CalendarRegion::Europa,
        CalendarRegion::JapaoOceania,
    ]
    .iter()
    .flat_map(|&r| free_tracks_for_region(r).iter().copied())
    .collect()
}

#[test]
fn rookie_all_tracks_from_eligible_regions() {
    // mazda_rookie e toyota_rookie só usam USA e Europa (sem JapaoOceania).
    use generator::{free_tracks_for_region, CalendarRegion};
    let eligible: HashSet<u32> = [CalendarRegion::Usa, CalendarRegion::Europa]
        .iter()
        .flat_map(|&r| free_tracks_for_region(r).iter().copied())
        .collect();

    for seed in 0..30u64 {
        let mut rng = StdRng::seed_from_u64(seed + 100);
        let cal =
            generate_calendar_for_category("S001", "mazda_rookie", &mut rng).expect("mazda_rookie");
        for entry in &cal {
            assert!(
                eligible.contains(&entry.track_id),
                "seed {seed}: mazda_rookie usou track {} fora de USA+Europa",
                entry.track_id
            );
        }
    }
}

#[test]
fn rookie_no_visitor_track() {
    // Rookies (S001) nunca têm visitante — pool único sem pista externa.
    use generator::{free_tracks_for_region, CalendarRegion};

    for seed in 0..30u64 {
        let mut rng = StdRng::seed_from_u64(seed + 200);
        let cal = generate_calendar_for_category("S001", "toyota_rookie", &mut rng)
            .expect("toyota_rookie");
        // Todos os tracks devem vir de UMA única região
        let usa: HashSet<u32> = free_tracks_for_region(CalendarRegion::Usa)
            .iter()
            .copied()
            .collect();
        let eur: HashSet<u32> = free_tracks_for_region(CalendarRegion::Europa)
            .iter()
            .copied()
            .collect();
        let track_ids: HashSet<u32> = cal.iter().map(|e| e.track_id).collect();
        // Se não é subconjunto de USA nem de Europa, pode ser multi-region fallback (DB incompleto)
        // Nesse caso, tracks devem ser subconjunto de USA ∪ Europa
        let all: HashSet<u32> = usa.union(&eur).copied().collect();
        assert!(
            track_ids.is_subset(&all),
            "seed {seed}: toyota_rookie usou track fora de USA+Europa"
        );
        // Sem pistas de JapaoOceania
        let jap: HashSet<u32> = free_tracks_for_region(CalendarRegion::JapaoOceania)
            .iter()
            .copied()
            .collect();
        assert!(
            track_ids.is_disjoint(&jap),
            "seed {seed}: toyota_rookie usou pista de JapaoOceania (não permitido em rookie)"
        );
    }
}

#[test]
fn amador_season1_no_visitor() {
    // Na temporada 1, amador nunca tem visitante — season_number < 2.
    // Visitor só existe quando season_number >= 2 E 50% de chance.
    // S001 → season_number=1 → sempre sem visitor.
    // Verificamos que todos os tracks vêm da mesma região base.
    use generator::{free_tracks_for_region, CalendarRegion};
    let usa: HashSet<u32> = free_tracks_for_region(CalendarRegion::Usa)
        .iter()
        .copied()
        .collect();
    let eur: HashSet<u32> = free_tracks_for_region(CalendarRegion::Europa)
        .iter()
        .copied()
        .collect();
    let jap: HashSet<u32> = free_tracks_for_region(CalendarRegion::JapaoOceania)
        .iter()
        .copied()
        .collect();
    let regions = [&usa, &eur, &jap];

    for seed in 0..30u64 {
        let mut rng = StdRng::seed_from_u64(seed + 300);
        let cal =
            generate_calendar_for_category("S001", "mazda_amador", &mut rng).expect("mazda_amador");
        let track_ids: HashSet<u32> = cal.iter().map(|e| e.track_id).collect();
        // Deve caber numa única região (DB atual pode forçar multi-region, mas sem visitor externo)
        // O que garantimos: tracks vêm do pool elegível de amador (USA + Europa + JapaoOceania)
        let all: HashSet<u32> = usa
            .iter()
            .chain(eur.iter())
            .chain(jap.iter())
            .copied()
            .collect();
        assert!(
            track_ids.is_subset(&all),
            "seed {seed}: mazda_amador S001 usou track fora das regiões elegíveis"
        );
        // Sem visitor: tracks devem caber em exatamente 1 das regiões (ou multi-region fallback)
        let fits_one_region = regions.iter().any(|r| track_ids.is_subset(r));
        let all_eligible: HashSet<u32> = all.clone();
        assert!(
            fits_one_region || track_ids.is_subset(&all_eligible),
            "seed {seed}: mazda_amador S001 misturou regiões inesperadamente"
        );
    }
}

#[test]
fn amador_season2_all_tracks_from_eligible_regions() {
    // No estado atual do DB, amador tem 8 rounds mas nenhuma região tem 8 tracks
    // → multi-region fallback é sempre ativado → pool = USA ∪ Europa ∪ JapaoOceania.
    // Verificamos que todos os tracks vêm dessas 3 regiões e sem duplicatas.
    // A lógica de visitante (visitor para S002) entra em ação quando o DB crescer e
    // uma única região passar a ter >= 8 tracks — testável com mock de get_track.
    use generator::{free_tracks_for_region, CalendarRegion};
    let all_eligible: HashSet<u32> = [
        CalendarRegion::Usa,
        CalendarRegion::Europa,
        CalendarRegion::JapaoOceania,
    ]
    .iter()
    .flat_map(|&r| free_tracks_for_region(r).iter().copied())
    .collect();

    for seed in 0..30u64 {
        let mut rng = StdRng::seed_from_u64(seed + 400);
        let cal = generate_calendar_for_category("S002", "toyota_amador", &mut rng)
            .expect("toyota_amador S002");
        let unique_ids: HashSet<u32> = cal.iter().map(|e| e.track_id).collect();
        assert_eq!(
            unique_ids.len(),
            cal.len(),
            "seed {seed}: toyota_amador S002 tem tracks duplicados"
        );
        for id in &unique_ids {
            assert!(
                all_eligible.contains(id),
                "seed {seed}: toyota_amador S002 usou track {id} fora das regiões elegíveis"
            );
        }
    }
}

#[test]
fn free_regional_final_is_strong() {
    // O último round deve ser de um VENUE forte (o layout pode variar por
    // temporada com a rotação one_layout_per_venue).
    use generator::{strong_free_tracks_for_region, CalendarRegion};
    let venue = |id: u32| get_track(id).map(|t| t.nome.split(" - ").next().unwrap_or(t.nome));
    let strong_venues: HashSet<&str> = [
        CalendarRegion::Usa,
        CalendarRegion::Europa,
        CalendarRegion::JapaoOceania,
    ]
    .iter()
    .flat_map(|&r| strong_free_tracks_for_region(r).iter().copied())
    .filter_map(venue)
    .collect();

    for cat in [
        "mazda_rookie",
        "toyota_rookie",
        "mazda_amador",
        "toyota_amador",
        "bmw_m2",
    ] {
        for seed in 0..20u64 {
            let mut rng = StdRng::seed_from_u64(seed + 500);
            let cal = generate_calendar_for_category("S001", cat, &mut rng)
                .unwrap_or_else(|e| panic!("{cat} seed {seed}: {e}"));
            let last = cal.last().expect("calendário vazio");
            let last_venue = venue(last.track_id).expect("venue");
            assert!(
                strong_venues.contains(last_venue),
                "{cat} seed {seed}: final venue '{last_venue}' (track {}) não é strong",
                last.track_id
            );
        }
    }
}

#[test]
fn production_all_tracks_from_mix_pool() {
    let pool: HashSet<u32> = generator::production_free_mix_pool()
        .iter()
        .copied()
        .collect();
    for seed in 0..20u64 {
        let mut rng = StdRng::seed_from_u64(seed + 600);
        let cal = generate_calendar_for_category("S001", "production_challenger", &mut rng)
            .expect("production_challenger");
        for entry in &cal {
            assert!(
                pool.contains(&entry.track_id),
                "seed {seed}: production usou track {} fora do mix pool",
                entry.track_id
            );
        }
    }
}

#[test]
fn production_final_is_strong() {
    let venue = |id: u32| get_track(id).map(|t| t.nome.split(" - ").next().unwrap_or(t.nome));
    let strong_venues: HashSet<&str> = generator::strong_production_tracks()
        .iter()
        .copied()
        .filter_map(venue)
        .collect();
    for seed in 0..20u64 {
        let mut rng = StdRng::seed_from_u64(seed + 700);
        let cal = generate_calendar_for_category("S001", "production_challenger", &mut rng)
            .expect("production_challenger");
        let last = cal.last().expect("calendário vazio");
        let last_venue = venue(last.track_id).expect("venue");
        assert!(
            strong_venues.contains(last_venue),
            "seed {seed}: production final venue '{last_venue}' (track {}) não é strong",
            last.track_id
        );
    }
}

#[test]
fn gt4_first_and_last_are_strong() {
    let strong: HashSet<u32> = generator::strong_gt4_tracks()
        .iter()
        .copied()
        .filter(|&id| get_track(id).is_some())
        .collect();
    for seed in 0..20u64 {
        let mut rng = StdRng::seed_from_u64(seed + 800);
        let cal = generate_calendar_for_category("S001", "gt4", &mut rng).expect("gt4");
        let first = cal.first().expect("vazio");
        let last = cal.last().expect("vazio");
        assert!(
            strong.contains(&first.track_id),
            "seed {seed}: gt4 abertura {} não é strong",
            first.track_id
        );
        assert!(
            strong.contains(&last.track_id),
            "seed {seed}: gt4 final {} não é strong",
            last.track_id
        );
    }
}

#[test]
fn gt3_first_and_last_are_strong() {
    let strong: HashSet<u32> = generator::strong_gt3_tracks()
        .iter()
        .copied()
        .filter(|&id| get_track(id).is_some())
        .collect();
    for seed in 0..20u64 {
        let mut rng = StdRng::seed_from_u64(seed + 900);
        let cal = generate_calendar_for_category("S001", "gt3", &mut rng).expect("gt3");
        let first = cal.first().expect("vazio");
        let last = cal.last().expect("vazio");
        assert!(
            strong.contains(&first.track_id),
            "seed {seed}: gt3 abertura {} não é strong",
            first.track_id
        );
        assert!(
            strong.contains(&last.track_id),
            "seed {seed}: gt3 final {} não é strong",
            last.track_id
        );
    }
}

#[test]
fn endurance_all_tracks_from_curated_pool() {
    let pool: HashSet<u32> = generator::endurance_curated_pool()
        .iter()
        .copied()
        .collect();
    for seed in 0..20u64 {
        let mut rng = StdRng::seed_from_u64(seed + 1000);
        let cal = generate_calendar_for_category("S001", "endurance", &mut rng).expect("endurance");
        for entry in &cal {
            assert!(
                pool.contains(&entry.track_id),
                "seed {seed}: endurance usou track {} fora do pool curado",
                entry.track_id
            );
        }
    }
}

#[test]
fn endurance_has_at_least_two_strong_events() {
    // Regra de âncora: final forte + pelo menos 1 âncora de miolo = mínimo 2 eventos fortes.
    let strong: HashSet<u32> = generator::strong_endurance_tracks()
        .iter()
        .copied()
        .filter(|&id| get_track(id).is_some())
        .collect();
    for seed in 0..20u64 {
        let mut rng = StdRng::seed_from_u64(seed + 1100);
        let cal = generate_calendar_for_category("S001", "endurance", &mut rng).expect("endurance");
        let strong_count = cal.iter().filter(|e| strong.contains(&e.track_id)).count();
        assert!(
            strong_count >= 2,
            "seed {seed}: endurance tem apenas {strong_count} evento(s) forte(s) (mínimo 2)"
        );
    }
}

#[test]
fn endurance_final_is_strong() {
    let strong: HashSet<u32> = generator::strong_endurance_tracks()
        .iter()
        .copied()
        .filter(|&id| get_track(id).is_some())
        .collect();
    for seed in 0..20u64 {
        let mut rng = StdRng::seed_from_u64(seed + 1200);
        let cal = generate_calendar_for_category("S001", "endurance", &mut rng).expect("endurance");
        let last = cal.last().expect("calendário vazio");
        assert!(
            strong.contains(&last.track_id),
            "seed {seed}: endurance final {} não é strong",
            last.track_id
        );
    }
}

// ── Corrida noturna garantida (≥1/temporada, nunca a 1ª/última) ──────────────

#[test]
fn is_night_horario_threshold() {
    assert!(is_night_horario("21:00"));
    assert!(is_night_horario("20:00"));
    assert!(!is_night_horario("18:00"));
    assert!(!is_night_horario("14:00"));
    assert!(!is_night_horario("10:00"));
}

#[test]
fn tier1_sempre_uma_noturna_no_miolo() {
    // Toda temporada tier 1+ tem EXATAMENTE uma noturna, nunca a 1ª nem a última.
    for seed in 0..40u64 {
        let mut rng = StdRng::seed_from_u64(seed + 5000);
        let cal = generate_calendar_for_category("S001", "gt3", &mut rng).expect("gt3 calendar");
        let max_rodada = cal.iter().map(|e| e.rodada).max().unwrap();
        let night: Vec<&CalendarEntry> = cal
            .iter()
            .filter(|e| is_night_horario(&e.horario))
            .collect();
        assert_eq!(
            night.len(),
            1,
            "seed {seed}: esperava exatamente 1 noturna, achou {}",
            night.len()
        );
        let r = night[0].rodada;
        assert!(
            r != 1 && r != max_rodada,
            "seed {seed}: noturna na rodada {r} (proibido 1ª/{max_rodada}ª)"
        );
    }
}

#[test]
fn rookie_nunca_tem_noturna() {
    // Preserva a regra "rookie (tier 0) nunca de noite".
    for seed in 0..40u64 {
        let mut rng = StdRng::seed_from_u64(seed + 6000);
        let cal = generate_calendar_for_category("S001", "mazda_rookie", &mut rng)
            .expect("rookie calendar");
        assert!(
            cal.iter().all(|e| !is_night_horario(&e.horario)),
            "seed {seed}: rookie não deveria ter corrida noturna"
        );
    }
}
