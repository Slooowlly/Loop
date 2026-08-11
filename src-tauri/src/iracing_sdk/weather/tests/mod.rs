use super::horario::sun_times;
use super::*;

#[test]
fn seco_sem_penalidade() {
    assert_eq!(rain_skill_penalty(0.0, RainIntensity::None), 0);
    assert_eq!(rain_skill_penalty(100.0, RainIntensity::None), 0);
}

#[test]
fn chuva_decente_bate_os_exemplos() {
    // PIOR caso segue no número original do user (30). O fundo subiu de 8 para 22 quando a
    // curva passou a ancorar por baixo.
    assert_eq!(rain_skill_penalty(0.0, RainIntensity::Decent), 30);
    assert_eq!(rain_skill_penalty(100.0, RainIntensity::Decent), 22);
}

#[test]
fn chuva_muito_forte_bate_os_exemplos() {
    // Pior caso no número original do user (40); o joelho em 90 e o fundo subiram.
    assert_eq!(rain_skill_penalty(0.0, RainIntensity::VeryHeavy), 40);
    assert_eq!(rain_skill_penalty(90.0, RainIntensity::VeryHeavy), 32);
    assert_eq!(rain_skill_penalty(100.0, RainIntensity::VeryHeavy), 30);
}

#[test]
fn o_debuff_geral_domina_a_diferenciacao() {
    // DOUTRINA: na chuva o grosso da punição é GERAL. A IA não erra e repete o mesmo tempo
    // toda volta; o debuff do pelotão inteiro é o que a faz andar com cuidado e o que
    // justifica ela não errar. Ser bom de chuva sobe um pouco a partir desse fundo, sem
    // escapar dele. Então o melhor de chuva do mundo leva a maior parte do que o pior leva.
    for intensity in [
        RainIntensity::Light,
        RainIntensity::Decent,
        RainIntensity::Heavy,
        RainIntensity::VeryHeavy,
    ] {
        let pior = rain_skill_penalty(0.0, intensity) as f64;
        let melhor = rain_skill_penalty(100.0, intensity) as f64;
        assert!(
            melhor >= pior * 0.65,
            "{intensity:?}: o ás da chuva escapa demais ({melhor} de {pior})"
        );
        assert!(
            melhor < pior,
            "{intensity:?}: ser bom de chuva tem que valer alguma coisa"
        );
    }
}

#[test]
fn intensidade_sobe_a_reta_toda() {
    // Pro MESMO piloto, mais chuva = mais penalidade (até pros bons).
    let bom = 95.0;
    let leve = rain_skill_penalty(bom, RainIntensity::Light);
    let dec = rain_skill_penalty(bom, RainIntensity::Decent);
    let forte = rain_skill_penalty(bom, RainIntensity::Heavy);
    let mf = rain_skill_penalty(bom, RainIntensity::VeryHeavy);
    assert!(
        leve < dec && dec < forte && forte < mf,
        "{leve} {dec} {forte} {mf}"
    );
}

#[test]
fn fator_chuva_alto_perde_menos() {
    // Na mesma chuva, quem é melhor na chuva perde menos.
    let i = RainIntensity::Heavy;
    assert!(rain_skill_penalty(100.0, i) < rain_skill_penalty(50.0, i));
    assert!(rain_skill_penalty(50.0, i) < rain_skill_penalty(0.0, i));
}

#[test]
fn fator_clampa() {
    assert_eq!(
        rain_skill_penalty(150.0, RainIntensity::Decent),
        rain_skill_penalty(100.0, RainIntensity::Decent)
    );
}

#[test]
fn estacao_por_hemisferio() {
    // Janeiro: inverno no norte, verão no sul.
    assert_eq!(season_for(1, Hemisphere::North), Season::Winter);
    assert_eq!(season_for(1, Hemisphere::South), Season::Summer);
    // Julho: verão no norte, inverno no sul.
    assert_eq!(season_for(7, Hemisphere::North), Season::Summer);
    assert_eq!(season_for(7, Hemisphere::South), Season::Winter);
}

#[test]
fn tendencia_pista_e_estacao() {
    // Chance de molhar: Rainy no inverno é o teto (~0.27) >> Dry no verão mínima (~0.01).
    let molhada = rain_tendency(ClimateTendency::Rainy, Season::Winter);
    let seca = rain_tendency(ClimateTendency::Dry, Season::Summer);
    assert!((0.22..0.35).contains(&molhada), "{molhada}");
    assert!(seca < 0.05, "{seca}");
    // Normal agora TAMBÉM molha (não é mais zero): ~0.08 na primavera.
    let normal = rain_tendency(ClimateTendency::Normal, Season::Spring);
    assert!((0.05..0.15).contains(&normal), "{normal}");
}

#[test]
fn primeira_corrida_e_roteirizada() {
    let w = generate_weather(1, Hemisphere::North, ClimateTendency::Rainy, 123, true);
    assert_eq!(w.scenario, WeatherScenario::FirstRaceScript);
    assert!(!w.is_wet_race);
    assert_eq!(w.race_intensity, RainIntensity::None);
}

#[test]
fn dry_no_verao_quase_nunca_molha() {
    // Pista Dry no verão = chance mínima (~2%): rara, mas NÃO mais zero.
    let mut wet = 0;
    for seed in 0..1000u64 {
        let w = generate_weather(7, Hemisphere::North, ClimateTendency::Dry, seed, false);
        if w.is_wet_race {
            wet += 1;
        }
    }
    assert!(wet > 0, "Dry no verão deveria molhar RARAMENTE, não nunca");
    assert!(wet < 80, "Dry no verão molhou demais ({wet}/1000)");
}

#[test]
fn normal_agora_molha_as_vezes() {
    // MUDANÇA de design: pista Normal (a maioria do catálogo) agora molha às vezes.
    let mut wet = 0;
    for seed in 0..500u64 {
        let w = generate_weather(1, Hemisphere::North, ClimateTendency::Normal, seed, false);
        if w.is_wet_race {
            wet += 1;
        }
    }
    // Normal no inverno ~12% → nem nunca nem maioria.
    assert!(
        (25..110).contains(&wet),
        "Normal inverno molhou {wet}/500 (esperado ~12%)"
    );
}

#[test]
fn temporal_so_no_tier_alto() {
    // VeryHeavy só aparece em Rainy inverno/outono; nunca em Normal/Dry.
    let mut vh_rainy = 0;
    for seed in 0..2000u64 {
        let w = generate_weather(1, Hemisphere::North, ClimateTendency::Rainy, seed, false);
        if w.race_intensity == RainIntensity::VeryHeavy {
            vh_rainy += 1;
        }
        let n = generate_weather(1, Hemisphere::North, ClimateTendency::Normal, seed, false);
        assert_ne!(
            n.race_intensity,
            RainIntensity::VeryHeavy,
            "Normal deu temporal (seed {seed})"
        );
    }
    assert!(vh_rainy > 0, "Rainy inverno nunca deu temporal");
}

#[test]
fn bias_geral_ainda_seco_na_maioria() {
    // Mesmo com a chuva mais frequente, a MAIORIA das corridas segue seca (Normal
    // ao longo do ano < 30% molhado).
    let mut wet = 0;
    let total = 12 * 200;
    for month in 1..=12u32 {
        for seed in 0..200u64 {
            let w = generate_weather(
                month,
                Hemisphere::North,
                ClimateTendency::Normal,
                seed,
                false,
            );
            if w.is_wet_race {
                wet += 1;
            }
        }
    }
    assert!(
        (wet as f64) / (total as f64) < 0.30,
        "molhou demais: {wet}/{total}"
    );
}

#[test]
fn deterministico_mesmo_seed() {
    let a = generate_weather(1, Hemisphere::North, ClimateTendency::Rainy, 999, false);
    let b = generate_weather(1, Hemisphere::North, ClimateTendency::Rainy, 999, false);
    assert_eq!(a.scenario, b.scenario);
    assert_eq!(a.is_wet_race, b.is_wet_race);
    assert_eq!(a.race_intensity, b.race_intensity);
}

const SEASONS: [Season; 4] = [
    Season::Winter,
    Season::Spring,
    Season::Summer,
    Season::Autumn,
];

#[test]
fn dia_nunca_no_meio_dia() {
    for season in SEASONS {
        for seed in 0..300u64 {
            // Rookie = sempre de dia; nenhum horário cai em 11–14h.
            let h = generate_race_start_hour(season, 0, false, seed);
            assert!(!(11.0..14.0).contains(&h), "meio-dia: {h} ({season:?})");
        }
    }
}

#[test]
fn rookie_nunca_de_noite() {
    for season in SEASONS {
        let (_, ss) = sun_times(season);
        for seed in 0..300u64 {
            // Mesmo no Charlotte (lit), rookie nunca de noite.
            let h = generate_race_start_hour(season, 0, true, seed);
            assert!(h < ss, "rookie de noite: {h} ({season:?})");
        }
    }
}

#[test]
fn charlotte_corre_muito_de_noite() {
    let (_, ss) = sun_times(Season::Spring);
    let night = (0..1000u64)
        .filter(|&s| generate_race_start_hour(Season::Spring, 1, true, s) > ss + 0.5)
        .count();
    let frac = night as f64 / 1000.0;
    assert!((0.72..0.88).contains(&frac), "Charlotte noite {frac}");
}

#[test]
fn pista_sem_luz_raramente_de_noite() {
    let (_, ss) = sun_times(Season::Spring);
    let night = (0..1000u64)
        .filter(|&s| generate_race_start_hour(Season::Spring, 1, false, s) > ss + 0.5)
        .count();
    let frac = night as f64 / 1000.0;
    assert!((0.06..0.17).contains(&frac), "noite sem luz {frac}");
}

#[test]
fn golden_segue_a_estacao() {
    // Pôr do sol mais cedo no inverno → golden hour da tarde mais cedo.
    assert!(sun_times(Season::Winter).1 < sun_times(Season::Summer).1);
}

#[test]
fn horario_deterministico() {
    let a = generate_race_start_hour(Season::Summer, 1, false, 42);
    let b = generate_race_start_hour(Season::Summer, 1, false, 42);
    assert_eq!(a.to_bits(), b.to_bits());
}

fn story(scenario: WeatherScenario, wet: bool, intensity: RainIntensity) -> WeatherStory {
    WeatherStory {
        scenario,
        is_wet_race: wet,
        race_intensity: intensity,
        qualy_intensity: RainIntensity::None,
        season: Season::Spring,
        tendency: 0.5,
    }
}

#[test]
fn perfil_molhado_tem_chuva() {
    let p = story_to_profile(
        &story(WeatherScenario::SteadyRain, true, RainIntensity::VeryHeavy),
        20,
    );
    assert!(p.track_water > 0);
    assert!(p.keyframes.iter().any(|(et, _)| *et >= 6), "sem chuva");
}

#[test]
fn perfil_seco_sem_chuva_no_grosso() {
    let p = story_to_profile(
        &story(WeatherScenario::ClearDry, false, RainIntensity::None),
        20,
    );
    assert_eq!(p.track_water, 0);
    assert!(p.keyframes.iter().all(|(et, _)| *et < 6));
}

const WET_SCENARIOS: [WeatherScenario; 5] = [
    WeatherScenario::SteadyRain,
    WeatherScenario::Improving,
    WeatherScenario::StormArrives,
    WeatherScenario::PulsingStorm,
    WeatherScenario::LightQualyWorseRace,
];

#[test]
fn corrida_molhada_nunca_e_so_garoa() {
    // O gerador NUNCA deve entregar `Light` como caráter de uma corrida molhada
    // (garoa = pista no limiar → grid larga dividido de pneu). Varre tendências e
    // seeds e confere o piso.
    for month in 1..=12u32 {
        for hemi in [Hemisphere::North, Hemisphere::South] {
            for group in [
                ClimateTendency::Dry,
                ClimateTendency::Normal,
                ClimateTendency::Rainy,
            ] {
                for seed in 0..300u64 {
                    let w = generate_weather(month, hemi, group, seed, false);
                    if w.is_wet_race {
                        assert_ne!(
                            w.race_intensity,
                            RainIntensity::Light,
                            "corrida molhada saiu como garoa (seed {seed})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn corrida_molhada_larga_inequivocamente_molhada() {
    // Para TODO cenário molhado e TODA intensidade de corrida molhada, a largada
    // (keyframe logo após a âncora QUALI) tem chuva de verdade (≥7) e a pista já
    // está encharcada (track_water ≥4) → o iRacing põe TODOS de wet, sem dúvida.
    for scenario in WET_SCENARIOS {
        for intensity in [
            RainIntensity::Decent,
            RainIntensity::Heavy,
            RainIntensity::VeryHeavy,
        ] {
            let p = story_to_profile(&story(scenario, true, intensity), 30);
            assert!(
                p.track_water >= 4,
                "{scenario:?}/{intensity:?}: pista seca demais na largada (water {})",
                p.track_water
            );
            // kf[0] é a âncora de QUALI (offset -90); kf[1] é a LARGADA da corrida.
            let start = p.keyframes[1];
            assert!(
                start.0 >= 7,
                "{scenario:?}/{intensity:?}: largada de garoa (event {}) — grid divide o pneu",
                start.0
            );
        }
    }
}

#[test]
fn timeline_comeca_na_largada_e_termina_na_bandeira() {
    // Qualquer cenário: frações ordenadas, começa ~0 e termina ~1, sem a QUALI.
    let p = story_to_timeline(&story(
        WeatherScenario::PulsingStorm,
        true,
        RainIntensity::Heavy,
    ));
    assert!(!p.is_empty());
    assert!(p[0].frac <= 0.05, "não começa na largada: {}", p[0].frac);
    assert!(p.last().unwrap().frac >= 0.95, "não termina na bandeira");
    assert!(
        p.windows(2).all(|w| w[0].frac <= w[1].frac + 1e-9),
        "frações fora de ordem"
    );
    assert!(
        p.iter().all(|pt| pt.frac >= 0.0),
        "tem ponto antes da largada (QUALI?)"
    );
}

#[test]
fn timeline_molhado_tem_chuva_seco_nao() {
    let wet = story_to_timeline(&story(
        WeatherScenario::SteadyRain,
        true,
        RainIntensity::VeryHeavy,
    ));
    assert!(
        wet.iter().any(|p| p.event_type >= 6),
        "corrida molhada sem chuva"
    );
    let dry = story_to_timeline(&story(
        WeatherScenario::ClearDry,
        false,
        RainIntensity::None,
    ));
    assert!(
        dry.iter().all(|p| p.event_type < 6),
        "corrida seca com chuva"
    );
}

#[test]
fn temperatura_fica_na_faixa_do_iracing() {
    // Toda combinação estação × intensidade × seed cai em [18, 32].
    for season in SEASONS {
        for intensity in [
            RainIntensity::None,
            RainIntensity::Light,
            RainIntensity::Decent,
            RainIntensity::Heavy,
            RainIntensity::VeryHeavy,
        ] {
            for seed in 0..300u64 {
                let mut s = story(
                    WeatherScenario::SteadyRain,
                    intensity != RainIntensity::None,
                    intensity,
                );
                s.season = season;
                let t = story_temperature(&s, seed);
                assert!(
                    (18..=32).contains(&t),
                    "temp fora da faixa: {t} ({season:?}/{intensity:?})"
                );
            }
        }
    }
}

#[test]
fn temperatura_chuva_esfria_e_verao_esquenta() {
    // Média: verão seco > inverno com temporal (a chuva alinha e esfria).
    let media = |season, intensity, wet| {
        let mut acc = 0i64;
        for seed in 0..500u64 {
            let mut s = story(WeatherScenario::SteadyRain, wet, intensity);
            s.season = season;
            acc += story_temperature(&s, seed);
        }
        acc as f64 / 500.0
    };
    let verao_seco = media(Season::Summer, RainIntensity::None, false);
    let inverno_temporal = media(Season::Winter, RainIntensity::VeryHeavy, true);
    assert!(
        verao_seco > inverno_temporal,
        "{verao_seco} vs {inverno_temporal}"
    );
}

#[test]
fn temperatura_deterministica() {
    let s = story(WeatherScenario::ClearDry, false, RainIntensity::None);
    assert_eq!(story_temperature(&s, 77), story_temperature(&s, 77));
}

#[test]
fn vento_usa_a_escada_e_varia() {
    // Todo vento é um degrau da WIND_LADDER (2–48) e a direção fica em [0,359].
    // Seco deve exercitar vários degraus (varia entre corridas).
    let ladder: std::collections::HashSet<i64> = WIND_LADDER.iter().map(|(k, _)| *k).collect();
    let mut speeds = std::collections::HashSet::new();
    for seed in 0..500u64 {
        let s = story(WeatherScenario::ClearDry, false, RainIntensity::None);
        let w = generate_wind(&s, seed);
        assert!(
            ladder.contains(&w.speed_kmh),
            "vento fora da escada: {}",
            w.speed_kmh
        );
        assert!(
            (0..=359).contains(&w.dir_deg),
            "direção fora da faixa: {}",
            w.dir_deg
        );
        speeds.insert(w.speed_kmh);
    }
    assert!(
        speeds.len() >= 4,
        "vento seco não varia o suficiente ({} degraus)",
        speeds.len()
    );
}

#[test]
fn temporal_venta_mais() {
    // Média de vento no temporal > média no seco.
    let media = |intensity, wet| {
        let mut acc = 0i64;
        for seed in 0..500u64 {
            let s = story(WeatherScenario::SteadyRain, wet, intensity);
            acc += generate_wind(&s, seed).speed_kmh;
        }
        acc as f64 / 500.0
    };
    assert!(
        media(RainIntensity::VeryHeavy, true) > media(RainIntensity::None, false),
        "temporal deveria ventar mais"
    );
}

#[test]
fn vento_deterministico() {
    let s = story(WeatherScenario::ClearDry, false, RainIntensity::None);
    assert_eq!(generate_wind(&s, 42), generate_wind(&s, 42));
}

const DRY_SCENARIOS: [WeatherScenario; 7] = [
    WeatherScenario::ClearDry,
    WeatherScenario::Scare,
    WeatherScenario::LastDrops,
    WeatherScenario::PassingDrizzle,
    WeatherScenario::ClearingUp,
    WeatherScenario::WetQualyDryRace,
    WeatherScenario::FirstRaceScript,
];

#[test]
fn corrida_seca_so_chove_no_trecho_final() {
    // DOUTRINA: a corrida é 100% seca ou 100% molhada. Numa prova SECA (IA sem
    // penalidade nenhuma) a chuva só pode aparecer no fim — nunca na largada nem no
    // meio. Confere no MESMO arco que vai pro iRacing, em frações da prova.
    for scenario in DRY_SCENARIOS {
        let pontos = story_to_timeline(&story(scenario, false, RainIntensity::None));
        for p in pontos {
            if p.event_type >= 6 {
                assert!(
                    p.frac >= DRY_RAIN_ONSET - 0.02,
                    "{scenario:?}: chuva (event {}) em {:.2} da prova seca",
                    p.event_type,
                    p.frac
                );
            }
        }
    }
}

#[test]
fn corrida_seca_larga_com_pista_seca() {
    // Sem água residual: numa prova sem penalidade o grid não pode ter dúvida de pneu.
    for scenario in DRY_SCENARIOS {
        let p = story_to_profile(&story(scenario, false, RainIntensity::None), 30);
        assert_eq!(
            p.track_water, 0,
            "{scenario:?}: pista molhada na largada de uma corrida seca"
        );
    }
}

#[test]
fn corrida_molhada_nunca_afrouxa_nem_passa_do_teto() {
    // A penalidade é FIXA no fim de semana e sai da `race_intensity`. Então o arco da
    // prova molhada tem que ficar entre "chuva de verdade" (7) e o event_type dessa
    // mesma intensidade: nem afrouxa pra garoa (pista volta ao limiar do slick) nem
    // roda como temporal quando foi cobrada como chuva decente.
    for scenario in WET_SCENARIOS {
        for (intensity, teto) in [
            (RainIntensity::Decent, 7),
            (RainIntensity::Heavy, 7),
            (RainIntensity::VeryHeavy, 8),
        ] {
            let p = story_to_profile(&story(scenario, true, intensity), 30);
            // kf[0] é a âncora de QUALI; o resto é a corrida.
            for (event, offset) in p.keyframes.iter().skip(1) {
                assert!(
                    (7..=teto).contains(event),
                    "{scenario:?}/{intensity:?}: event {event} no offset {offset} (esperado 7..={teto})"
                );
            }
        }
    }
}

#[test]
fn primeira_corrida_termina_com_pingos() {
    let p = story_to_profile(
        &story(WeatherScenario::FirstRaceScript, false, RainIntensity::None),
        20,
    );
    let last = p.keyframes.last().unwrap();
    assert_eq!(last.0, 6, "não terminou com pingos");
    // Estrutura (robusta ao C do modelo afim): largada LIMPA (keyframe após a âncora
    // QUALI), termina com CHUVA, e os offsets são crescentes (ordenados no tempo).
    assert_eq!(p.keyframes[1].0, 0, "largada não está limpa");
    assert!(
        p.keyframes.windows(2).all(|w| w[0].1 <= w[1].1),
        "offsets fora de ordem"
    );
}
