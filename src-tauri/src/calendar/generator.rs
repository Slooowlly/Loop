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
    Lmp2Prototype,
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
        "lmp2" => Some(CalendarFamily::Lmp2Prototype),
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

/// Pool free regional por região geográfica. SOMENTE pistas gratuitas (ids
/// corretos do iRacing). A trava em `resolve_thematic_pool` reforça filtrando por
/// `gratuita`, então paga nunca entra numa categoria free mesmo por engano.
pub(crate) fn free_tracks_for_region(region: CalendarRegion) -> &'static [u32] {
    match region {
        // Cada venue lista TODOS os seus layouts; a rotação por temporada
        // (one_layout_per_venue) escolhe UM por ano, alternando a cada temporada.
        CalendarRegion::Usa => &[
            554, // Charlotte Motor Speedway – Roval
            353, 352, 354, // Lime Rock Park – Grand Prix / Classic / Chicanes
            9,   // Summit Point Raceway
            // Jefferson (8) é grátis mas pequeno demais p/ carro — fora da rotação.
            465, 467, 466, // Virginia International Raceway – Full / North / Grand
        ],
        CalendarRegion::Europa => &[
            181, 180, 183, 184, 185, // Oulton Park – Fosters / Intl / ... layouts
            297, 298, // Snetterton Circuit – 300 / 200
            489, // Circuit de Lédenon
            515, 516, // Circuito de Navarra – Speed Circuit / Medium
            449, 454, 455, // Motorsport Arena Oschersleben – Grand Prix / Alt / B
            451, // Rudskogen Motorsenter
        ],
        CalendarRegion::JapaoOceania => &[
            166, 167, // Okayama International Circuit – Full / Short
            324, // Tsukuba Circuit – 2000 Full
            202, 208, // Oran Park Raceway – Grand Prix / South
            440, 439, // Winton Motor Raceway – Club / National
        ],
    }
}

/// Pool Production Challenger: união explícita dos 3 regionais.
/// Mistura ampla de todas as regiões, cara de especial.
pub(crate) fn production_free_mix_pool() -> &'static [u32] {
    &[
        // USA (Jefferson 8 fica de fora — pequeno demais p/ carro)
        554, 353, 352, 354, 9, 465, 467, 466, // Europa
        181, 180, 183, 184, 185, 297, 298, 489, 515, 516, 449, 454, 455,
        451, // Japão/Oceania
        166, 167, 324, 202, 208, 440, 439,
    ]
}

/// Pool GT4: campeonato internacional plausível.
pub(crate) fn gt4_curated_pool() -> &'static [u32] {
    &[
        181, // Oulton Park – Fosters
        297, // Snetterton Circuit – 300
        465, // Virginia International Raceway – Full Course
        586, // WeatherTech Raceway at Laguna Seca
        145, // Brands Hatch – Grand Prix
        233, // Donington Park – Grand Prix
        341, // Silverstone Circuit – Grand Prix
        485, // Circuit Zandvoort
        501, // Misano World Circuit Marco Simoncelli
        266, // Autodromo Enzo e Dino Ferrari (Imola)
        127, // Road Atlanta – Full Course
        95,  // Sebring International Raceway
        433, // Watkins Glen International – Cup
        153, // Mid-Ohio Sports Car Course
        18,  // Road America
        144, // Canadian Tire Motorsport Park (Mosport)
        // ── Catálogo expandido (jul/2026): palcos regionais/nacionais (British GT, GT4 Euro, IMSA) ──
        229, // Circuit of the Americas – Grand Prix (top-tier, âncora)
        509, // Algarve International Circuit – Portimão (top-tier, âncora)
        413, // Hungaroring
        199, // Circuit Zolder
        475, // MotorLand Aragón – Grand Prix
        473, // Circuito de Jerez
        463, // Circuit de Nevers Magny-Cours
        585, // Barber Motorsports Park
        566, // Sonoma Raceway
        448, // Indianapolis Motor Speedway – Road Course
        218, // Circuit Gilles Villeneuve – Montreal
        521, // Sachsenring
    ]
}

/// Pool GT3: campeonato internacional de prestígio.
pub(crate) fn gt3_curated_pool() -> &'static [u32] {
    &[
        523, // Circuit de Spa-Francorchamps
        239, // Autodromo Nazionale Monza
        341, // Silverstone Circuit – Grand Prix
        168, // Suzuka International Racing Course
        212, // Autódromo José Carlos Pace (Interlagos)
        192, // Daytona International Speedway – Road Course
        95,  // Sebring International Raceway
        433, // Watkins Glen International – Cup
        127, // Road Atlanta – Full Course
        266, // Autodromo Enzo e Dino Ferrari (Imola)
        403, // Red Bull Ring
        263, // Nürburgring – Combined (Short)
        252, // Nürburgring – Combined (24H)
        345, // Circuit de Barcelona-Catalunya
        498, // Autodromo Internazionale del Mugello
        390, // Hockenheimring Baden-Württemberg – GP
        444, // Fuji International Speedway
        219, // Mount Panorama Motor Racing Circuit (Bathurst)
        // ── Catálogo expandido (jul/2026): palcos reais de GTWC / Intercontinental GT / IMSA GTD ──
        229, // Circuit of the Americas – Grand Prix (top-tier, âncora)
        509, // Algarve International Circuit – Portimão (top-tier, âncora)
        413, // Hungaroring
        199, // Circuit Zolder (GTWC sprint)
        475, // MotorLand Aragón – Grand Prix (GTWC)
        473, // Circuito de Jerez (GTWC)
        485, // Circuit Zandvoort – Grand Prix (GTWC)
        145, // Brands Hatch – Grand Prix (GTWC)
        501, // Misano World Circuit (GTWC)
        540, // The Bend Motorsport Park – GT Circuit (Intercontinental GT)
        152, // Phillip Island (endurance GT3)
        566, // Sonoma Raceway (IMSA GTD)
        218, // Circuit Gilles Villeneuve – Montreal
        448, // Indianapolis Motor Speedway – Road Course
        521, // Sachsenring (ADAC GT Masters)
        195, // Mobility Resort Motegi (Super GT)
        585, // Barber Motorsports Park
    ]
}

/// Pool LMP2: mistura palcos fortes de GT3 com provas clássicas de endurance.
pub(crate) fn lmp2_curated_pool() -> &'static [u32] {
    &[
        192, // Daytona International Speedway – Road Course
        95,  // Sebring International Raceway
        523, // Circuit de Spa-Francorchamps
        268, // Circuit des 24 Heures du Mans
        127, // Road Atlanta – Full Course
        433, // Watkins Glen International – Cup
        341, // Silverstone Circuit – Grand Prix
        239, // Autodromo Nazionale Monza
        168, // Suzuka International Racing Course
        444, // Fuji International Speedway
        212, // Autódromo José Carlos Pace (Interlagos)
        266, // Autodromo Enzo e Dino Ferrari (Imola)
        // ── Catálogo expandido (jul/2026): protótipo IMSA/WEC + rounds urbanos ──
        229, // Circuit of the Americas (WEC/IMSA, top-tier)
        448, // Indianapolis Motor Speedway – Road Course (IMSA)
        218, // Circuit Gilles Villeneuve – Montreal (IMSA)
        536, // Portland International Raceway (IMSA)
        566, // Sonoma Raceway (IMSA)
        509, // Algarve International Circuit – Portimão (WEC)
        179, // Long Beach Street Circuit (IMSA street)
        584, // St. Petersburg Grand Prix (IMSA street)
        319, // Detroit Grand Prix at Belle Isle (IMSA street)
        405, // Chicago Street Course (IMSA street)
    ]
}

/// Pool Endurance: pequeno e rotativo, apenas grandes palcos.
pub(crate) fn endurance_curated_pool() -> &'static [u32] {
    &[
        192, // Daytona International Speedway – Road Course
        95,  // Sebring International Raceway
        523, // Circuit de Spa-Francorchamps
        268, // Circuit des 24 Heures du Mans
        127, // Road Atlanta – Full Course
        433, // Watkins Glen International – Cup
        168, // Suzuka International Racing Course
        444, // Fuji International Speedway
        252, // Nürburgring – Combined (24H)
        212, // Autódromo José Carlos Pace (Interlagos)
        // ── Catálogo expandido (jul/2026): grandes provas de resistência ──
        219, // Mount Panorama (Bathurst 12h)
        152, // Phillip Island (12h)
        541, // The Bend Motorsport Park – International (endurance)
        229, // Circuit of the Americas
        509, // Algarve International Circuit – Portimão (WEC)
    ]
}

// ── Pistas fortes por família ─────────────────────────────────────────────────

/// Subconjunto forte de uma região free — elegível para slots narrativos (final).
pub(crate) fn strong_free_tracks_for_region(region: CalendarRegion) -> &'static [u32] {
    match region {
        CalendarRegion::Usa => &[465, 554],    // VIR, Charlotte Roval
        CalendarRegion::Europa => &[181, 297], // Oulton Park Fosters, Snetterton
        CalendarRegion::JapaoOceania => &[166, 324], // Okayama, Tsukuba
    }
}

/// Pistas fortes de Production: final deve sair deste conjunto.
pub(crate) fn strong_production_tracks() -> &'static [u32] {
    &[465, 554, 181, 166] // VIR, Charlotte Roval, Oulton Park, Okayama
}

/// Pistas fortes GT4: abertura e final devem sair deste conjunto.
pub(crate) fn strong_gt4_tracks() -> &'static [u32] {
    &[145, 341, 266, 95, 433, 127, 181, 229, 509]
    // Brands Hatch GP, Silverstone, Imola, Sebring, Watkins Glen, Road Atlanta,
    // Oulton Park, COTA, Portimão
}

/// Pistas fortes GT3: abertura, penúltima (opcional) e final devem sair deste conjunto.
pub(crate) fn strong_gt3_tracks() -> &'static [u32] {
    &[523, 239, 168, 212, 192, 95, 433, 219, 341, 229, 509]
    // Spa, Monza, Suzuka, Interlagos, Daytona, Sebring, Watkins Glen, Bathurst,
    // Silverstone, COTA, Portimão
}

/// Pistas fortes LMP2: abertura e final devem parecer evento principal.
pub(crate) fn strong_lmp2_tracks() -> &'static [u32] {
    &[192, 95, 523, 268, 127, 433, 341]
    // Daytona, Sebring, Spa, Le Mans, Road Atlanta, Watkins Glen, Silverstone
}

/// Âncoras Endurance: final + âncora de miolo devem sair deste conjunto.
pub(crate) fn strong_endurance_tracks() -> &'static [u32] {
    &[192, 95, 523, 268, 127]
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

/// Venue de uma pista = nome antes de " - " (ex.: "Oulton Park - Fosters" →
/// "Oulton Park"). Agrupa layouts do mesmo circuito.
fn track_venue(id: u32) -> Option<&'static str> {
    crate::constants::tracks::get_track(id)
        .filter(|t| t.gratuita)
        .map(|t| t.nome.split(" - ").next().unwrap_or(t.nome))
}

/// Reduz uma lista de ids a UM layout por VENUE, escolhido pela temporada — o
/// mesmo circuito não repete no ano, e temporadas diferentes pegam versões
/// diferentes (rotação determinística). Só pistas grátis.
fn one_layout_per_venue(ids: &[u32], season_number: i32) -> Vec<u32> {
    use std::collections::BTreeMap;
    let mut by_venue: BTreeMap<&'static str, Vec<u32>> = BTreeMap::new();
    for &id in ids {
        if let Some(venue) = track_venue(id) {
            by_venue.entry(venue).or_default().push(id);
        }
    }
    by_venue
        .into_values()
        .map(|mut layouts| {
            layouts.sort_unstable();
            let idx = ((season_number.max(1) - 1) as usize) % layouts.len();
            layouts[idx]
        })
        .collect()
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

            // Pool de uma região = 1 layout por venue, rotacionado pela temporada.
            // No rookie, VIR Grand Course (466) é excluído: pista longa/técnica demais p/ a
            // categoria de entrada (a rotação da VIR no rookie só pega Full 465 / North 467).
            let region_pool = |r: CalendarRegion| {
                let ids: Vec<u32> = free_tracks_for_region(r)
                    .iter()
                    .copied()
                    .filter(|&id| !(is_rookie && id == 466))
                    .collect();
                one_layout_per_venue(&ids, season_number)
            };

            // Tentar encontrar região única com venues suficientes.
            let single_region = shuffled
                .iter()
                .copied()
                .find(|&r| region_pool(r).len() >= needed);

            let (base_region, candidate_ids, used_multi_region) = if let Some(r) = single_region {
                (r, region_pool(r), false)
            } else {
                // Fallback: pool de todas as regiões elegíveis (DB ainda incompleto)
                let primary = shuffled[0];
                let mut seen = std::collections::HashSet::new();
                let ids: Vec<u32> = regions
                    .iter()
                    .flat_map(|&r| region_pool(r))
                    .filter(|&id| seen.insert(id))
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
                        .flat_map(|&r| region_pool(r))
                        .filter(|id| !candidate_ids.contains(id))
                        .collect();
                    if visitor_candidates.is_empty() {
                        None
                    } else {
                        Some(visitor_candidates[rng.gen_range(0..visitor_candidates.len())])
                    }
                } else {
                    None
                };

            // Strong = layouts (já rotacionados) cujo VENUE é forte na região.
            let strong_venues: std::collections::HashSet<&'static str> =
                strong_free_tracks_for_region(base_region)
                    .iter()
                    .filter_map(|&id| track_venue(id))
                    .collect();
            let strong_ids: Vec<u32> = candidate_ids
                .iter()
                .copied()
                .filter(|&id| track_venue(id).is_some_and(|v| strong_venues.contains(v)))
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
            // 1 layout por venue, rotacionado pela temporada (não repete circuito).
            let candidate_ids = one_layout_per_venue(production_free_mix_pool(), season_number);
            let strong_venues: std::collections::HashSet<&'static str> = strong_production_tracks()
                .iter()
                .filter_map(|&id| track_venue(id))
                .collect();
            let strong_ids: Vec<u32> = candidate_ids
                .iter()
                .copied()
                .filter(|&id| track_venue(id).is_some_and(|v| strong_venues.contains(v)))
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

        CalendarFamily::Lmp2Prototype => {
            let candidate_ids: Vec<u32> = lmp2_curated_pool()
                .iter()
                .copied()
                .filter(|&id| {
                    use crate::constants::tracks::get_track;
                    get_track(id).is_some()
                })
                .collect();
            let strong_ids: Vec<u32> = strong_lmp2_tracks()
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
                    strong_penult: false,
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
        assert!(pool.contains(&523)); // Spa
        assert!(pool.contains(&239)); // Monza
    }

    #[test]
    fn endurance_pool_has_le_mans() {
        assert!(endurance_curated_pool().contains(&268));
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
    fn lmp2_uses_own_gt3_endurance_mixed_family() {
        assert_eq!(
            calendar_family_for_category("lmp2"),
            Some(CalendarFamily::Lmp2Prototype)
        );
        assert!(lmp2_curated_pool().contains(&523)); // Spa
        assert!(lmp2_curated_pool().contains(&268)); // Le Mans
        assert!(lmp2_curated_pool().len() >= 10);
    }

    #[test]
    fn gt4_pool_large_enough_for_season() {
        // GT4 tem 10 rodadas — pool deve ter >= 10 tracks
        assert!(gt4_curated_pool().len() >= 10);
    }

    #[test]
    fn expanded_catalog_tracks_wired_into_gt_pools() {
        // As âncoras top-tier do catálogo expandido entram nos pools GT e nas
        // listas fortes (elegíveis a abertura/final).
        for id in [229, 509] {
            assert!(gt3_curated_pool().contains(&id), "GT3 pool sem {id}");
            assert!(gt4_curated_pool().contains(&id), "GT4 pool sem {id}");
            assert!(strong_gt3_tracks().contains(&id), "strong GT3 sem {id}");
            assert!(strong_gt4_tracks().contains(&id), "strong GT4 sem {id}");
        }
        // Amostra das "menos conhecidas" que ficam só na rotação (pool, sem ser forte).
        assert!(gt3_curated_pool().contains(&413)); // Hungaroring
        assert!(gt3_curated_pool().contains(&199)); // Zolder
        assert!(lmp2_curated_pool().contains(&179)); // Long Beach (protótipo urbano)
        assert!(endurance_curated_pool().contains(&219)); // Bathurst 12h
    }

    #[test]
    fn all_pool_ids_exist_in_catalog() {
        // Nenhum id de pool pode ser fantasma — todos precisam existir no catálogo.
        use crate::constants::tracks::get_track;
        let pools: [&[u32]; 4] = [
            gt3_curated_pool(),
            gt4_curated_pool(),
            lmp2_curated_pool(),
            endurance_curated_pool(),
        ];
        for pool in pools {
            for &id in pool {
                assert!(get_track(id).is_some(), "pool referencia track inexistente {id}");
            }
        }
    }

    #[test]
    fn venue_rotation_um_por_venue_e_varia_por_temporada() {
        // Oulton tem vários layouts free — cada temporada pega UM, e muda.
        let oulton = [181, 180, 183, 184, 185];
        let s1 = one_layout_per_venue(&oulton, 1);
        let s2 = one_layout_per_venue(&oulton, 2);
        assert_eq!(s1.len(), 1, "1 layout por venue");
        assert_eq!(s2.len(), 1);
        assert_ne!(s1[0], s2[0], "temporadas diferentes → layouts diferentes");

        // Mistura de venues (Oulton x2, Snetterton x2, VIR x2) → 3 ids distintos.
        let mixed = [181, 180, 297, 298, 465, 467];
        assert_eq!(one_layout_per_venue(&mixed, 1).len(), 3);

        // Determinístico: mesma temporada → mesmo resultado.
        assert_eq!(
            one_layout_per_venue(&oulton, 5),
            one_layout_per_venue(&oulton, 5)
        );
    }
}
