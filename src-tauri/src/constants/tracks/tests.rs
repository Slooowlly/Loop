use super::dados::TRACKS;
use super::*;
use crate::models::enums::TrackType;

#[test]
fn test_tracks_for_tier_0_only_free() {
    let tracks = get_tracks_for_tier(0);
    assert!(!tracks.is_empty());
    assert!(tracks.iter().all(|track| track.gratuita));
}

#[test]
fn brands_hatch_is_paid_content() {
    assert!(!get_track(145).expect("Brands GP").gratuita);
}

#[test]
fn free_or_substitute_swaps_paid_for_free_and_is_stable() {
    // Adelaide (580) é conteúdo pago no catálogo.
    assert!(!get_track(580).expect("Adelaide").gratuita);
    let sub = free_or_substitute(580).expect("deve devolver uma substituta");
    assert!(sub.gratuita, "a substituta precisa ser uma pista grátis");
    // Determinístico: a mesma pista paga sempre mapeia para a mesma free (o import
    // compara o resultado contra a pista exportada).
    assert_eq!(
        sub.track_id,
        free_or_substitute(580).expect("estável").track_id
    );
}

#[test]
fn free_or_substitute_keeps_free_tracks_untouched() {
    // 451 é uma pista grátis do catálogo → passa intacta.
    assert!(get_track(451).expect("free 451").gratuita);
    assert_eq!(free_or_substitute(451).expect("free").track_id, 451);
}

#[test]
fn free_or_substitute_prefers_same_track_type() {
    // Adelaide (580) é Road; a substituta também deve ser Road (todas as free são).
    let sub = free_or_substitute(580).expect("substituta");
    assert_eq!(sub.tipo, TrackType::Road);
}

/// A regra de substituição já opera sobre um conjunto de POSSE arbitrário — hoje ela
/// recebe as grátis, mas a lógica não sabe disso. É o que sobra do D-06 pronto para o
/// dia em que a lista de posse existir; ver o cabeçalho de `free_or_substitute`.
#[test]
fn substituta_para_posse_respeita_o_conjunto_recebido() {
    let possuidas: Vec<&'static TrackInfo> = [451, 202, 440]
        .iter()
        .map(|id| get_track(*id).expect("pista do catálogo"))
        .collect();

    // Pista de fora do conjunto → substituída por uma DE DENTRO dele.
    let sub = substituta_para_posse(580, &possuidas).expect("substituta");
    assert!(
        possuidas.iter().any(|t| t.track_id == sub.track_id),
        "a substituta {} não está no conjunto possuído",
        sub.track_id
    );
    // Determinística entre chamadas — o import depende disso.
    assert_eq!(
        sub.track_id,
        substituta_para_posse(580, &possuidas)
            .expect("estável")
            .track_id
    );

    // Pista de dentro do conjunto passa intacta, mesmo sendo conteúdo pago em geral.
    assert_eq!(
        substituta_para_posse(451, &possuidas)
            .expect("possuída")
            .track_id,
        451
    );

    // Sem dado de posse não se substitui nada: devolve a original.
    assert_eq!(
        substituta_para_posse(580, &[]).expect("original").track_id,
        580
    );
}

#[test]
fn current_free_road_tracks_are_in_catalog() {
    for track_id in [202, 440, 449, 451, 489, 515] {
        let track = get_track(track_id).unwrap_or_else(|| panic!("missing track {track_id}"));
        assert!(track.gratuita, "track {track_id} should be free");
    }
}

#[test]
fn catalog_has_no_duplicate_ids() {
    let mut ids: Vec<u32> = TRACKS.iter().map(|t| t.track_id).collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total, "há track_ids duplicados no catálogo");
}

#[test]
fn catalog_covers_all_owned_events() {
    assert_eq!(
        TRACKS.len(),
        107,
        "catálogo deve ter exatamente os eventos possuídos"
    );
}

#[test]
fn corrected_ids_match_iracing() {
    // Amostra das correções críticas contra o esquema legado.
    assert_eq!(get_track(239).expect("Monza").nome_curto, "Monza");
    assert_eq!(get_track(229).expect("COTA").nome_curto, "COTA");
    assert_eq!(get_track(212).expect("Interlagos").nome_curto, "Interlagos");
    assert_eq!(get_track(523).expect("Spa").nome_curto, "Spa");
    assert_eq!(get_track(168).expect("Suzuka").nome_curto, "Suzuka");
    assert_eq!(get_track(219).expect("Bathurst").nome_curto, "Bathurst");
    assert_eq!(
        get_track(586).expect("Laguna Seca").nome_curto,
        "Laguna Seca"
    );
}

#[test]
fn test_tracks_for_tier_3_includes_paid() {
    let tracks = get_tracks_for_tier(3);
    assert!(tracks.iter().any(|track| !track.gratuita));
}

#[test]
fn test_rain_chance_by_group() {
    assert_eq!(get_rain_chance(586), 0.02); // Laguna Seca (Dry)
    assert_eq!(get_rain_chance(9), 0.08); // Summit Point (Normal)
    assert_eq!(get_rain_chance(181), 0.18); // Oulton Park Fosters (Rainy)
}

#[test]
fn test_qualifying_duration_default() {
    assert_eq!(get_qualifying_duration(586), 15); // Laguna Seca (3.6km)
}

#[test]
fn test_qualifying_duration_long_tracks() {
    assert_eq!(get_qualifying_duration(249), 20); // Nordschleife
    assert_eq!(get_qualifying_duration(268), 20); // Le Mans
    assert_eq!(get_qualifying_duration(252), 20); // Nürburgring 24h
}
