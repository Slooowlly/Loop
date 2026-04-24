// Gerador de calendário temático — fundação conceitual
//
// Este módulo declara a linguagem do gerador temático:
// famílias de campeonato, regiões geográficas e pools curados de pistas.
//
// Os pools são declarativos: expressam a intenção temática completa,
// incluindo tracks ainda ausentes do DB (marcadas com // TODO).
// Ao wiring no gerador real, filtrar com get_track(id).is_some()
// para operar apenas com tracks existentes no catálogo atual.
//
// O algoritmo principal de geração permanece em calendar/mod.rs.
// Este módulo prepara a matéria-prima para o próximo passo.

// ── Família de campeonato ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CalendarFamily {
    FreeRegional,
    FreeSpecialMix,
    GtInternational,
    EnduranceCurated,
}

/// Retorna None para categoria desconhecida — sem fallback silencioso.
pub(crate) fn calendar_family_for_category(category_id: &str) -> Option<CalendarFamily> {
    match category_id {
        "mazda_rookie" | "toyota_rookie" | "mazda_amador" | "toyota_amador" | "bmw_m2" => {
            Some(CalendarFamily::FreeRegional)
        }
        "production_challenger" => Some(CalendarFamily::FreeSpecialMix),
        "gt4" | "gt3" => Some(CalendarFamily::GtInternational),
        "endurance" => Some(CalendarFamily::EnduranceCurated),
        _ => None,
    }
}

// ── Região geográfica ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CalendarRegion {
    Usa,
    Europa,
    JapaoOceania,
}

/// Retorna Some apenas para categorias FreeRegional.
/// GT4, GT3, Endurance e Production não usam regiões — retornam None.
/// Rookie nunca recebe JapaoOceania.
pub(crate) fn eligible_regions_for_category(
    category_id: &str,
) -> Option<&'static [CalendarRegion]> {
    match category_id {
        "mazda_rookie" | "toyota_rookie" => Some(&[CalendarRegion::Usa, CalendarRegion::Europa]),
        "mazda_amador" | "toyota_amador" | "bmw_m2" => Some(&[
            CalendarRegion::Usa,
            CalendarRegion::Europa,
            CalendarRegion::JapaoOceania,
        ]),
        _ => None,
    }
}

// ── Pools de pistas por região e família ──────────────────────────────────────

/// Pool free regional por região geográfica.
/// VIR (58) e Snetterton (316) são incluídos intencionalmente no regional free
/// do modo carreira, mesmo quando a origem externa muda a classificação comercial.
pub(crate) fn free_tracks_for_region(region: CalendarRegion) -> &'static [u32] {
    match region {
        CalendarRegion::Usa => &[
            554, // Charlotte Motor Speedway – Roval
            14,  // Lime Rock Park – Full Course
            9,   // Summit Point Raceway
            58,  // Virginia International Raceway – Full Course
            47,  // WeatherTech Raceway at Laguna Seca
        ],
        CalendarRegion::Europa => &[
            261, // Oulton Park – Fosters
            316, // Snetterton Circuit – 300
            489, // Circuit de Lédenon
            515, // Circuito de Navarra – Speed Circuit
            449, // Motorsport Arena Oschersleben – Grand Prix
            451, // Rudskogen Motorsenter
        ],
        CalendarRegion::JapaoOceania => &[
            166, // Okayama International Circuit
            325, // Tsukuba Circuit – 2000 Full Course
            202, // Oran Park Raceway – Grand Prix
            440, // Winton Motor Raceway – Club Circuit
        ],
    }
}

/// Pool Production Challenger: união explícita dos 3 regionais.
/// Mistura ampla de todas as regiões, cara de especial.
pub(crate) fn production_free_mix_pool() -> &'static [u32] {
    &[
        // USA
        554, 14, 9, 58, 47, // Europa
        261, 316, 489, 515, 449, 451, // Japão/Oceania
        166, 325, 202, 440,
    ]
}

/// Pool GT4: campeonato internacional plausível.
pub(crate) fn gt4_curated_pool() -> &'static [u32] {
    &[
        261, // Oulton Park – Fosters
        316, // Snetterton Circuit – 300
        58,  // Virginia International Raceway – Full Course
        47,  // WeatherTech Raceway at Laguna Seca
        300, // Brands Hatch – Grand Prix
        399, // Donington Park – Grand Prix
        106, // Silverstone Circuit – Grand Prix
        389, // Circuit Zandvoort
        363, // Misano World Circuit Marco Simoncelli
        455, // Autodromo Enzo e Dino Ferrari (Imola)
        249, // Road Atlanta – Full Course
        238, // Sebring International Raceway
        67,  // Watkins Glen International – Boot
        51,  // Mid-Ohio Sports Car Course
        52,  // Road America
        125, // Canadian Tire Motorsport Park (Mosport)
             // TODO: Circuito de Navarra (not yet in DB)
             // TODO: Motorsport Arena Oschersleben (not yet in DB)
    ]
}

/// Pool GT3: campeonato internacional de prestígio.
pub(crate) fn gt3_curated_pool() -> &'static [u32] {
    &[
        188, // Circuit de Spa-Francorchamps
        93,  // Autodromo Nazionale Monza
        106, // Silverstone Circuit – Grand Prix
        164, // Suzuka International Racing Course
        199, // Autódromo José Carlos Pace (Interlagos)
        45,  // Daytona International Speedway – Road Course
        238, // Sebring International Raceway
        67,  // Watkins Glen International – Boot
        249, // Road Atlanta – Full Course
        455, // Autodromo Enzo e Dino Ferrari (Imola)
        397, // Red Bull Ring
        192, // Nürburgring – Grand Prix Strecke
        528, // Nürburgring – Combined (24H)
        281, // Circuit de Barcelona-Catalunya
        452, // Autodromo Internazionale del Mugello
        193, // Hockenheimring Baden-Württemberg – GP
        373, // Fuji International Speedway
        119, // Mount Panorama Motor Racing Circuit (Bathurst)
    ]
}

/// Pool Endurance: pequeno e rotativo, apenas grandes palcos.
pub(crate) fn endurance_curated_pool() -> &'static [u32] {
    &[
        45,  // Daytona International Speedway – Road Course
        238, // Sebring International Raceway
        188, // Circuit de Spa-Francorchamps
        287, // Circuit de la Sarthe – Le Mans 24H
        249, // Road Atlanta – Full Course
        67,  // Watkins Glen International – Boot
        164, // Suzuka International Racing Course
        373, // Fuji International Speedway
        528, // Nürburgring – Combined (24H)
        199, // Autódromo José Carlos Pace (Interlagos)
    ]
}

// ── Pistas fortes por família ─────────────────────────────────────────────────

/// Subconjunto forte de uma região free — elegível para slots narrativos (final).
pub(crate) fn strong_free_tracks_for_region(region: CalendarRegion) -> &'static [u32] {
    match region {
        CalendarRegion::Usa => &[58, 47],            // VIR, Laguna Seca
        CalendarRegion::Europa => &[261, 316],       // Oulton Park Fosters, Snetterton
        CalendarRegion::JapaoOceania => &[166, 325], // Okayama, Tsukuba
    }
}

/// Pistas fortes de Production: final deve sair deste conjunto.
pub(crate) fn strong_production_tracks() -> &'static [u32] {
    &[58, 47, 261, 166] // VIR, Laguna Seca, Oulton Park, Okayama
}

/// Pistas fortes GT4: abertura e final devem sair deste conjunto.
pub(crate) fn strong_gt4_tracks() -> &'static [u32] {
    &[300, 106, 455, 238, 67, 249, 261]
    // Brands Hatch GP, Silverstone, Imola, Sebring, Watkins Glen, Road Atlanta, Oulton Park
}

/// Pistas fortes GT3: abertura, penúltima (opcional) e final devem sair deste conjunto.
pub(crate) fn strong_gt3_tracks() -> &'static [u32] {
    &[188, 93, 164, 199, 45, 238, 67, 119, 106]
    // Spa, Monza, Suzuka, Interlagos, Daytona, Sebring, Watkins Glen, Bathurst, Silverstone
}

/// Âncoras Endurance: final + âncora de miolo devem sair deste conjunto.
pub(crate) fn strong_endurance_tracks() -> &'static [u32] {
    &[45, 238, 188, 287, 249]
    // Daytona, Sebring, Spa, Le Mans, Road Atlanta
}

// ── Pool temático resolvido ───────────────────────────────────────────────────

/// Flags de slots narrativos para o algoritmo de seleção.
pub(crate) struct NarrativeRounds {
    pub strong_first: bool,  // round 1 deve sair de strong_ids
    pub strong_last: bool,   // round N deve sair de strong_ids
    pub strong_penult: bool, // round N-1 deve sair de strong_ids (GT3)
}

/// Pool resolvido para uma categoria/temporada específica.
pub(crate) struct ThematicPool {
    /// Track IDs candidatos (filtrados: existem no catálogo atual).
    pub candidate_ids: Vec<u32>,
    /// Subconjunto forte para slots narrativos.
    pub strong_ids: Vec<u32>,
    /// Track visitante opcional (Amador/BMW season >= 2).
    /// Tratado como slot de miolo dedicado — nunca vai para rounds narrativos.
    pub visitor_id: Option<u32>,
    /// Configuração de slots narrativos para o algoritmo de seleção.
    pub narrative_rounds: NarrativeRounds,
}

/// Resolve o pool temático de uma categoria para uma temporada.
///
/// Retorna None apenas para categorias desconhecidas.
///
/// `needed`: número de corridas na temporada (corridas_por_temporada).
/// Usado para selecionar automaticamente a região com tracks suficientes.
///
/// `season_number` é derivado do season_id pelo chamador (proxy v1).
pub(crate) fn resolve_thematic_pool<R: rand::Rng>(
    category_id: &str,
    season_number: i32,
    needed: usize,
    rng: &mut R,
) -> Option<ThematicPool> {
    use crate::constants::tracks::get_track;
    let family = calendar_family_for_category(category_id)?;

    match family {
        CalendarFamily::FreeRegional => {
            let regions = eligible_regions_for_category(category_id)?;
            let is_rookie = matches!(category_id, "mazda_rookie" | "toyota_rookie");

            // Shuffles para não ter preferência estática de ordem
            let mut shuffled: Vec<CalendarRegion> = regions.to_vec();
            // shuffle manual sem SliceRandom para manter independência de imports
            for i in (1..shuffled.len()).rev() {
                let j = rng.gen_range(0..=i);
                shuffled.swap(i, j);
            }

            // Tentar encontrar região única com tracks suficientes
            let single_region = shuffled.iter().copied().find(|&r| {
                free_tracks_for_region(r)
                    .iter()
                    .filter(|&&id| get_track(id).is_some())
                    .count()
                    >= needed
            });

            let (base_region, candidate_ids, used_multi_region) = if let Some(r) = single_region {
                let ids: Vec<u32> = free_tracks_for_region(r)
                    .iter()
                    .copied()
                    .filter(|&id| get_track(id).is_some())
                    .collect();
                (r, ids, false)
            } else {
                // Fallback: pool de todas as regiões elegíveis (DB ainda incompleto)
                let primary = shuffled[0];
                let mut seen = std::collections::HashSet::new();
                let ids: Vec<u32> = regions
                    .iter()
                    .flat_map(|&r| free_tracks_for_region(r).iter().copied())
                    .filter(|&id| get_track(id).is_some() && seen.insert(id))
                    .collect();
                (primary, ids, true)
            };

            // Visitante: Amador/BMW, season >= 2, 50% chance.
            // Disponível apenas quando usou região única (sem multi-region fallback).
            // season_number derivado de season_id via parse_season_number() — proxy v1.
            let visitor_id =
                if !is_rookie && !used_multi_region && season_number >= 2 && rng.gen_bool(0.5) {
                    let visitor_candidates: Vec<u32> = regions
                        .iter()
                        .filter(|&&r| r != base_region)
                        .flat_map(|&r| free_tracks_for_region(r).iter().copied())
                        .filter(|&id| get_track(id).is_some() && !candidate_ids.contains(&id))
                        .collect();
                    if visitor_candidates.is_empty() {
                        None
                    } else {
                        Some(visitor_candidates[rng.gen_range(0..visitor_candidates.len())])
                    }
                } else {
                    None
                };

            let strong_ids: Vec<u32> = strong_free_tracks_for_region(base_region)
                .iter()
                .copied()
                .filter(|id| candidate_ids.contains(id))
                .collect();

            Some(ThematicPool {
                candidate_ids,
                strong_ids,
                visitor_id,
                narrative_rounds: NarrativeRounds {
                    strong_first: false,
                    strong_last: true,
                    strong_penult: false,
                },
            })
        }

        CalendarFamily::FreeSpecialMix => {
            let candidate_ids: Vec<u32> = production_free_mix_pool()
                .iter()
                .copied()
                .filter(|&id| {
                    use crate::constants::tracks::get_track;
                    get_track(id).is_some()
                })
                .collect();
            let strong_ids: Vec<u32> = strong_production_tracks()
                .iter()
                .copied()
                .filter(|id| candidate_ids.contains(id))
                .collect();
            Some(ThematicPool {
                candidate_ids,
                strong_ids,
                visitor_id: None,
                narrative_rounds: NarrativeRounds {
                    strong_first: false,
                    strong_last: true,
                    strong_penult: false,
                },
            })
        }

        CalendarFamily::GtInternational => {
            let raw_pool = if category_id == "gt3" {
                gt3_curated_pool()
            } else {
                gt4_curated_pool()
            };
            let candidate_ids: Vec<u32> = raw_pool
                .iter()
                .copied()
                .filter(|&id| {
                    use crate::constants::tracks::get_track;
                    get_track(id).is_some()
                })
                .collect();
            let strong_raw = if category_id == "gt3" {
                strong_gt3_tracks()
            } else {
                strong_gt4_tracks()
            };
            let strong_ids: Vec<u32> = strong_raw
                .iter()
                .copied()
                .filter(|id| candidate_ids.contains(id))
                .collect();
            Some(ThematicPool {
                candidate_ids,
                strong_ids,
                visitor_id: None,
                narrative_rounds: NarrativeRounds {
                    strong_first: true,
                    strong_last: true,
                    strong_penult: category_id == "gt3",
                },
            })
        }

        CalendarFamily::EnduranceCurated => {
            let candidate_ids: Vec<u32> = endurance_curated_pool()
                .iter()
                .copied()
                .filter(|&id| {
                    use crate::constants::tracks::get_track;
                    get_track(id).is_some()
                })
                .collect();
            let strong_ids: Vec<u32> = strong_endurance_tracks()
                .iter()
                .copied()
                .filter(|id| candidate_ids.contains(id))
                .collect();
            Some(ThematicPool {
                candidate_ids,
                strong_ids,
                visitor_id: None,
                narrative_rounds: NarrativeRounds {
                    strong_first: false,
                    strong_last: true,
                    strong_penult: false,
                },
            })
        }
    }
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rookie_never_gets_japao_oceania() {
        let regions = eligible_regions_for_category("mazda_rookie").expect("mazda_rookie");
        assert!(!regions.contains(&CalendarRegion::JapaoOceania));
        let regions = eligible_regions_for_category("toyota_rookie").expect("toyota_rookie");
        assert!(!regions.contains(&CalendarRegion::JapaoOceania));
    }

    #[test]
    fn amador_gets_three_regions() {
        assert_eq!(
            eligible_regions_for_category("mazda_amador")
                .expect("mazda_amador")
                .len(),
            3
        );
        assert_eq!(
            eligible_regions_for_category("bmw_m2")
                .expect("bmw_m2")
                .len(),
            3
        );
    }

    #[test]
    fn category_families_correct() {
        assert_eq!(
            calendar_family_for_category("gt3"),
            Some(CalendarFamily::GtInternational)
        );
        assert_eq!(
            calendar_family_for_category("endurance"),
            Some(CalendarFamily::EnduranceCurated)
        );
        assert_eq!(
            calendar_family_for_category("production_challenger"),
            Some(CalendarFamily::FreeSpecialMix)
        );
        assert_eq!(
            calendar_family_for_category("mazda_rookie"),
            Some(CalendarFamily::FreeRegional)
        );
        assert_eq!(calendar_family_for_category("unknown_cat"), None);
    }

    #[test]
    fn gt_categories_have_no_regions() {
        assert!(eligible_regions_for_category("gt3").is_none());
        assert!(eligible_regions_for_category("gt4").is_none());
        assert!(eligible_regions_for_category("endurance").is_none());
        assert!(eligible_regions_for_category("production_challenger").is_none());
    }

    #[test]
    fn gt3_pool_has_spa_and_monza() {
        let pool = gt3_curated_pool();
        assert!(pool.contains(&188)); // Spa
        assert!(pool.contains(&93)); // Monza
    }

    #[test]
    fn endurance_pool_has_le_mans() {
        assert!(endurance_curated_pool().contains(&287));
    }

    #[test]
    fn production_pool_is_superset_of_regional_pools() {
        let prod = production_free_mix_pool();
        for region in [
            CalendarRegion::Usa,
            CalendarRegion::Europa,
            CalendarRegion::JapaoOceania,
        ] {
            for id in free_tracks_for_region(region) {
                assert!(prod.contains(id), "prod pool missing track {id}");
            }
        }
    }

    #[test]
    fn regional_free_pools_include_current_free_tracks_and_exclude_brands() {
        let europa = free_tracks_for_region(CalendarRegion::Europa);
        for track_id in [449, 451, 489, 515] {
            assert!(
                europa.contains(&track_id),
                "Europa pool missing free track {track_id}"
            );
        }
        assert!(!europa.contains(&300), "Brands GP should not be free");
        assert!(!europa.contains(&301), "Brands Indy should not be free");

        let japao_oceania = free_tracks_for_region(CalendarRegion::JapaoOceania);
        for track_id in [202, 440] {
            assert!(
                japao_oceania.contains(&track_id),
                "JapaoOceania pool missing free track {track_id}"
            );
        }

        let production = production_free_mix_pool();
        for track_id in [202, 440, 449, 451, 489, 515] {
            assert!(
                production.contains(&track_id),
                "Production pool missing free track {track_id}"
            );
        }
        assert!(
            !production.contains(&300) && !production.contains(&301),
            "Production free pool should not include Brands Hatch"
        );
    }

    #[test]
    fn gt3_pool_large_enough_for_season() {
        // GT3 tem 14 rodadas — pool deve ter >= 14 tracks
        assert!(gt3_curated_pool().len() >= 14);
    }

    #[test]
    fn gt4_pool_large_enough_for_season() {
        // GT4 tem 10 rodadas — pool deve ter >= 10 tracks
        assert!(gt4_curated_pool().len() >= 10);
    }
}
