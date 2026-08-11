//! Testes da camada de comandos do iRacing.
//!
//! A pasta inteira estava sem nenhum `#[cfg(test)]`, e é por aqui que passa o caminho
//! principal do jogo: exportar o grid, exportar o calendário, importar o resultado
//! oficial. O que se cobre aqui é a parte PURA — números fixos por piloto, hemisfério e
//! clima determinístico, escada de dificuldade, validade dos bilhetes entre as etapas do
//! export. O resto exige o sim aberto e continua sendo validado correndo.

use super::*;

fn pasta_de_teste(rotulo: &str) -> std::path::PathBuf {
    let unico = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("loop_iracing_{rotulo}_{unico}"));
    std::fs::create_dir_all(&dir).expect("criar pasta temporária de teste");
    dir
}

// ─── Números fixos por piloto ────────────────────────────────────────────────

#[test]
fn numero_atribuido_e_o_menor_livre_e_a_ordem_nao_importa() {
    let dir = pasta_de_teste("numeros");
    let ids: Vec<String> = ["p_zeta", "p_alfa", "p_beta"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mapa = ensure_driver_numbers(&dir, "c1", &ids).expect("atribuir números");

    // A atribuição ordena por id antes de distribuir, então é determinística.
    assert_eq!(mapa.get("p_alfa"), Some(&1));
    assert_eq!(mapa.get("p_beta"), Some(&2));
    assert_eq!(mapa.get("p_zeta"), Some(&3));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn o_numero_de_um_piloto_nao_muda_quando_o_grid_muda() {
    // É o contrato inteiro deste mapa: o número é a ponte entre o carro no iRacing e o
    // `driver_id` da carreira. Se ele trocar entre duas rodadas, o resultado da corrida
    // volta casado com o piloto errado.
    let dir = pasta_de_teste("numeros_estaveis");
    let primeira = ensure_driver_numbers(&dir, "c1", &["b".to_string(), "c".to_string()])
        .expect("primeira rodada");

    // Chega um piloto que ordena ANTES dos que já tinham número.
    let segunda = ensure_driver_numbers(
        &dir,
        "c1",
        &["a".to_string(), "b".to_string(), "c".to_string()],
    )
    .expect("segunda rodada");

    assert_eq!(segunda.get("b"), primeira.get("b"));
    assert_eq!(segunda.get("c"), primeira.get("c"));
    assert_eq!(
        segunda.get("a"),
        Some(&3),
        "o novo pega o menor número LIVRE"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cada_carreira_tem_o_proprio_mapa_de_numeros() {
    let dir = pasta_de_teste("numeros_por_carreira");
    let um = ensure_driver_numbers(&dir, "c1", &["x".to_string()]).expect("carreira 1");
    let dois = ensure_driver_numbers(&dir, "c2", &["y".to_string()]).expect("carreira 2");
    assert_eq!(um.get("x"), Some(&1));
    assert_eq!(dois.get("y"), Some(&1));
    assert!(
        !dois.contains_key("x"),
        "o mapa de uma carreira não pode vazar para a outra"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nenhum_numero_se_repete_num_grid_grande() {
    let dir = pasta_de_teste("numeros_unicos");
    let ids: Vec<String> = (0..40).map(|i| format!("piloto_{i:02}")).collect();
    let mapa = ensure_driver_numbers(&dir, "c1", &ids).expect("grid grande");
    let distintos: std::collections::HashSet<i64> = mapa.values().copied().collect();
    assert_eq!(distintos.len(), ids.len(), "número repetido no grid");
    assert!(mapa.values().all(|n| *n >= 1), "número tem de começar em 1");
    let _ = std::fs::remove_dir_all(&dir);
}

// ─── Atitude da IA: o que os abandonos recentes dizem ────────────────────────

/// `(piloto, fonte, abandonou)` como o banco entrega.
fn dnf(pid: &str, fonte: &str) -> (String, Option<String>, bool) {
    (pid.to_string(), Some(fonte.to_string()), true)
}

#[test]
fn tirado_de_corrida_na_ultima_rodada_vira_vinganca() {
    let sinais = classificar_abandonos(&[
        vec![dnf("a", "PostCollision")],
        vec![dnf("b", "PostCollision")],
    ]);
    assert!(
        sinais.tirado_na_ultima.contains("a"),
        "a vingança é da ÚLTIMA corrida"
    );
    assert!(
        !sinais.tirado_na_ultima.contains("b"),
        "duas rodadas atrás já passou"
    );
}

#[test]
fn culpa_propria_nao_vira_azar_nem_desconfianca() {
    // O piloto que roda sozinho não pode sair correndo com raiva de terceiros nem
    // desconfiando do carro: as três fontes são disjuntas de propósito.
    let sinais = classificar_abandonos(&[vec![dnf("a", "DriverError")]]);
    assert!(sinais.azar.is_empty());
    assert!(sinais.mecanico.is_empty());
    assert!(sinais.tirado_na_ultima.is_empty());
}

#[test]
fn falha_de_carro_conta_como_desconfianca_e_soma_entre_rodadas() {
    let sinais = classificar_abandonos(&[
        vec![dnf("a", "Mechanical")],
        vec![dnf("a", "Operational")],
        vec![dnf("a", "Mechanical")],
    ]);
    assert_eq!(sinais.mecanico.get("a"), Some(&3));
    assert!(sinais.azar.is_empty());
}

#[test]
fn quem_terminou_nao_entra_em_nenhuma_conta() {
    let sinais =
        classificar_abandonos(&[vec![("a".to_string(), Some("PostCollision".into()), false)]]);
    assert_eq!(sinais, SinaisDeAbandono::default());
}

#[test]
fn fonte_desconhecida_conta_como_azar() {
    // Fonte nova no banco não pode sumir da conta: o padrão é "não foi culpa dele".
    let sinais = classificar_abandonos(&[vec![
        dnf("a", "AlgumaFonteNova"),
        ("b".to_string(), None, true),
    ]]);
    assert_eq!(sinais.azar.get("a"), Some(&1));
    assert_eq!(sinais.azar.get("b"), Some(&1));
}

#[test]
fn nemesis_exige_repetir_o_mesmo_rival() {
    // Terminar ao lado de gente diferente é o normal de uma corrida; nêmesis é quando
    // é sempre o MESMO carro.
    let uma_vez_cada = nemesis_por_vizinhanca(&[
        vec![("a".into(), 1), ("b".into(), 2)],
        vec![("a".into(), 1), ("c".into(), 2)],
    ]);
    assert!(uma_vez_cada.is_empty(), "{uma_vez_cada:?}");

    let sempre_o_mesmo = nemesis_por_vizinhanca(&[
        vec![("a".into(), 1), ("b".into(), 2)],
        vec![("a".into(), 3), ("b".into(), 4)],
    ]);
    assert!(sempre_o_mesmo.contains("a"));
    assert!(sempre_o_mesmo.contains("b"));
}

#[test]
fn so_conta_quem_esta_ao_lado_na_chegada() {
    // Primeiro e terceiro não são vizinhos: entre eles passou o segundo.
    let n = nemesis_por_vizinhanca(&[
        vec![("a".into(), 1), ("m".into(), 2), ("z".into(), 3)],
        vec![("a".into(), 1), ("m".into(), 2), ("z".into(), 3)],
    ]);
    assert!(n.contains("m"), "o do meio é vizinho dos dois");
    assert!(n.contains("a"));
    assert!(n.contains("z"));

    let distantes = nemesis_por_vizinhanca(&[
        vec![("a".into(), 1), ("m".into(), 2), ("z".into(), 3)],
        vec![("a".into(), 1), ("x".into(), 2), ("z".into(), 3)],
    ]);
    assert!(
        !distantes.contains("z") || !distantes.contains("a"),
        "primeiro e terceiro não podem virar nêmesis um do outro"
    );
}

#[test]
fn posicao_invalida_nao_entra_na_vizinhanca() {
    let n = nemesis_por_vizinhanca(&[
        vec![("a".into(), 0), ("b".into(), 1)],
        vec![("a".into(), 0), ("b".into(), 1)],
    ]);
    assert!(n.is_empty(), "posição 0 é 'sem classificação'");
}

// ─── Nome de pasta do roster ─────────────────────────────────────────────────

#[test]
fn o_nome_do_roster_perde_o_que_o_windows_recusa() {
    assert_eq!(
        nome_seguro_de_roster("GT3: Brasil/2026").as_deref(),
        Some("GT3 Brasil2026")
    );
    assert_eq!(
        nome_seguro_de_roster("  Rookie  ").as_deref(),
        Some("Rookie")
    );
}

#[test]
fn nome_que_some_inteiro_nao_vira_pasta_sem_nome() {
    assert_eq!(nome_seguro_de_roster("///"), None);
    assert_eq!(nome_seguro_de_roster("   "), None);
    assert_eq!(nome_seguro_de_roster(""), None);
}

// ─── Clima determinístico da etapa ───────────────────────────────────────────

#[test]
fn a_semente_da_etapa_e_estavel_e_separa_carreiras() {
    // Ela é a base de TODO o clima e de todo o sorteio de quebra da etapa: se variar
    // entre duas chamadas, o forecast e o disparo ao vivo divergem na mesma corrida.
    assert_eq!(event_seed("save-1", "R003"), event_seed("save-1", "R003"));
    assert_ne!(event_seed("save-1", "R003"), event_seed("save-2", "R003"));
    assert_ne!(event_seed("save-1", "R003"), event_seed("save-1", "R004"));
}

#[test]
fn a_semana_do_ano_vira_mes_dentro_do_calendario() {
    assert_eq!(month_from_week(1), 1);
    assert_eq!(month_from_week(52), 12);
    // Fora da faixa não pode produzir mês inválido: o gerador de clima indexa por mês.
    assert_eq!(month_from_week(0), 1);
    assert_eq!(month_from_week(-5), 1);
    assert_eq!(month_from_week(999), 12);
    for semana in 1..=52 {
        let m = month_from_week(semana);
        assert!((1..=12).contains(&m), "semana {semana} deu mês {m}");
    }
}

#[test]
fn brasil_e_australia_correm_no_hemisferio_sul() {
    use crate::iracing_sdk::weather::Hemisphere;
    assert_eq!(track_hemisphere("🇧🇷 Brasil"), Hemisphere::South);
    assert_eq!(track_hemisphere("🇦🇺 Austrália"), Hemisphere::South);
    assert_eq!(track_hemisphere("🇺🇸 EUA"), Hemisphere::North);
    assert_eq!(track_hemisphere("🇩🇪 Alemanha"), Hemisphere::North);
}

#[test]
fn todo_pais_do_catalogo_tem_hemisferio_declarado() {
    // O guard que a vistoria pediu. A dedução do hemisfério é por bandeira, e país fora
    // das listas cai no NORTE em silêncio, invertendo a estação da etapa inteira. Aqui
    // uma pista de país novo quebra a suíte em vez de sortear verão em julho no Brasil.
    let mut desconhecidos: Vec<&str> = Vec::new();
    for track in crate::constants::tracks::get_all_tracks() {
        let conhecido = PAISES_DO_SUL.iter().any(|f| track.pais.contains(f))
            || PAISES_DO_NORTE.iter().any(|f| track.pais.contains(f));
        if !conhecido && !desconhecidos.contains(&track.pais) {
            desconhecidos.push(track.pais);
        }
    }
    assert!(
        desconhecidos.is_empty(),
        "país sem hemisfério declarado em clima.rs: {desconhecidos:?} — \
         acrescente à lista certa (PAISES_DO_SUL ou PAISES_DO_NORTE)"
    );
}

#[test]
fn a_historia_seca_nunca_vira_pista_molhada() {
    use crate::iracing_sdk::weather::{RainIntensity, Season, WeatherScenario, WeatherStory};
    use crate::models::enums::WeatherCondition as W;

    let mut story = WeatherStory {
        scenario: WeatherScenario::SteadyRain,
        is_wet_race: false,
        race_intensity: RainIntensity::Heavy,
        qualy_intensity: RainIntensity::None,
        season: Season::Summer,
        tendency: 0.5,
    };
    assert_eq!(
        story_to_weather_condition(&story),
        W::Dry,
        "sem corrida molhada a intensidade não conta"
    );

    story.is_wet_race = true;
    for (intensidade, esperado) in [
        (RainIntensity::Light, W::Damp),
        (RainIntensity::Decent, W::Wet),
        (RainIntensity::Heavy, W::HeavyRain),
        (RainIntensity::VeryHeavy, W::HeavyRain),
    ] {
        story.race_intensity = intensidade;
        assert_eq!(
            story_to_weather_condition(&story),
            esperado,
            "{intensidade:?}"
        );
    }
}

#[test]
fn pista_desconhecida_devolve_clima_neutro_em_vez_de_chutar() {
    let neutro = crate::car::breakdown::Weather::NEUTRAL;
    let w = race_breakdown_weather(u32::MAX, 20, 12345, false, 1.0);
    assert_eq!(w.wetness, neutro.wetness);
    assert_eq!(w.temperature, neutro.temperature);
}

#[test]
fn o_clima_da_etapa_e_o_mesmo_nas_duas_chamadas() {
    // O forecast (Sala de Estratégia) e o disparo ao vivo chamam esta função por caminhos
    // diferentes com a mesma semente. Divergir aqui é mostrar ao jogador um risco que a
    // corrida dele não vai correr.
    let Some(track) = crate::constants::tracks::get_all_tracks().first() else {
        return;
    };
    let a = race_breakdown_weather(track.track_id, 30, 987_654, false, 1.0);
    let b = race_breakdown_weather(track.track_id, 30, 987_654, false, 1.0);
    assert_eq!(a.wetness, b.wetness);
    assert_eq!(a.temperature, b.temperature);
    assert_eq!(a.humidity, b.humidity);
    assert_eq!(a.wind_kmh, b.wind_kmh);
}

#[test]
fn forcar_chuva_molha_a_pista() {
    let Some(track) = crate::constants::tracks::get_all_tracks().first() else {
        return;
    };
    let forcado = race_breakdown_weather(track.track_id, 30, 1, true, 1.0);
    assert!(
        forcado.wetness > 0.0,
        "o modo de teste de chuva tem de molhar a pista"
    );
}

#[test]
fn o_ano_do_sim_cai_sempre_na_janela_de_quatro_anos() {
    // A carreira pode chegar a 2040; o arquivo do iRacing só aceita a janela que o sim
    // conhece. A dobra é por resto de 4 a partir de 2024 — e precisa valer também para
    // ano ANTERIOR a 2024, que é onde um `%` cru devolveria negativo.
    for ano in [1999, 2020, 2024, 2025, 2026, 2031, 2040, 2100] {
        let seguro = sim_safe_year(ano);
        assert!(
            (2024..=2027).contains(&seguro),
            "ano {ano} virou {seguro}, fora da janela do sim"
        );
    }
    assert_eq!(sim_safe_year(2024), 2024);
    assert_eq!(sim_safe_year(2028), 2024);
    assert_eq!(
        sim_safe_year(2023),
        2027,
        "ano anterior não pode dar negativo"
    );
}

// ─── Escada de dificuldade ───────────────────────────────────────────────────

#[test]
fn o_sweet_spot_nunca_sai_da_escala_do_iracing() {
    // O `driverSkill` do iRacing vai até 125. Estourar aqui produzia arquivo inválido.
    let dir = pasta_de_teste("sweet_spot");
    for tier in 0u8..=8 {
        for pista in [None, Some(451i64), Some(465), Some(489)] {
            let s = ai_sweet_spot(tier, pista, &dir, 0);
            assert!(
                (0..=125).contains(&s),
                "tier {tier} pista {pista:?} deu {s}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_escada_de_dificuldade_sobe_com_o_tier() {
    // Sem perfil adaptativo e sem offset de pista, subir de divisão tem de ser mais
    // difícil, nunca mais fácil. O rookie fica de fora: ele leva o desconto fixo.
    let dir = pasta_de_teste("escada");
    let mut anterior = ai_sweet_spot(1, None, &dir, 0);
    for tier in 2u8..=8 {
        let atual = ai_sweet_spot(tier, None, &dir, 0);
        assert!(
            atual >= anterior,
            "tier {tier} ({atual}) ficou mais fácil que o anterior ({anterior})"
        );
        anterior = atual;
    }
    assert!(
        ai_sweet_spot(0, None, &dir, 0) < ai_sweet_spot(1, None, &dir, 0),
        "o rookie tem de ser o degrau mais fácil"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn o_offset_da_pista_desloca_o_sweet_spot() {
    let dir = pasta_de_teste("offset");
    let base = ai_sweet_spot(4, Some(451), &dir, 0); // Rudskogen = pista baseline, offset 0
    let vir = ai_sweet_spot(4, Some(465), &dir, 0); // VIR Full Course, offset alto
    assert!(
        vir > base,
        "a pista com offset positivo tem de subir o sweet spot ({vir} vs {base})"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ─── Bilhetes entre as etapas do export ──────────────────────────────────────

#[test]
fn o_bilhete_do_export_volta_como_foi_gravado() {
    let dir = pasta_de_teste("postit_ida_volta");
    let band = ExportSkillBand {
        categoria: "gt3".to_string(),
        track_id: 451,
        min: 40.0,
        max: 88.0,
        gravado_em_unix: agora_unix(),
    };
    save_export_skill_band(&dir, 777, &band).expect("gravar a faixa");
    let lido = load_export_skill_band(&dir, 777).expect("ler a faixa");
    assert_eq!(lido.categoria, "gt3");
    assert_eq!(lido.track_id, 451);
    assert_eq!(lido.min, 40.0);
    assert_eq!(lido.max, 88.0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bilhete_velho_e_ignorado_em_vez_de_produzir_banda_errada() {
    // O defeito que isto fecha: um export INTERROMPIDO dias atrás, na mesma pista e
    // categoria, era lido como se fosse de agora — e a temporada saía com a banda dele.
    let dir = pasta_de_teste("postit_velho");
    let velho = ExportSkillBand {
        categoria: "gt3".to_string(),
        track_id: 451,
        min: 40.0,
        max: 88.0,
        gravado_em_unix: agora_unix() - (48 * 60 * 60),
    };
    save_export_skill_band(&dir, 778, &velho).expect("gravar a faixa velha");
    assert!(
        load_export_skill_band(&dir, 778).is_none(),
        "bilhete de dois dias atrás não é deste fluxo"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bilhete_sem_carimbo_continua_valendo() {
    // Save de quem já jogava, gravado antes de o campo existir: recusar um bilhete
    // legítimo é pior do que aceitar um velho.
    assert!(postit_esta_fresco(0, "teste"));
    assert!(postit_esta_fresco(-1, "teste"));
}

#[test]
fn bilhete_recem_gravado_esta_fresco() {
    assert!(postit_esta_fresco(agora_unix(), "teste"));
    assert!(postit_esta_fresco(agora_unix() - 3600, "teste"));
}

#[test]
fn contexto_de_carro_ausente_nao_e_erro() {
    let dir = pasta_de_teste("contexto_ausente");
    assert!(load_car_difficulty_context(&dir, 999).is_none());
    assert!(load_export_skill_band(&dir, 999).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn o_contexto_de_carro_volta_com_as_vantagens_por_numero() {
    let dir = pasta_de_teste("contexto_carro");
    let mut by_number = std::collections::HashMap::new();
    by_number.insert("7".to_string(), 0.35);
    by_number.insert("12".to_string(), -0.10);
    save_car_difficulty_context(
        &dir,
        555,
        &CarDifficultyContext {
            track_id: 353,
            player_advantage: 0.2,
            by_number,
            gravado_em_unix: agora_unix(),
        },
    )
    .expect("gravar o contexto");

    let lido = load_car_difficulty_context(&dir, 555).expect("ler o contexto");
    assert_eq!(lido.track_id, 353);
    assert_eq!(lido.player_advantage, 0.2);
    assert_eq!(lido.by_number.get("7"), Some(&0.35));
    assert_eq!(lido.by_number.get("12"), Some(&-0.10));
    let _ = std::fs::remove_dir_all(&dir);
}

// ─── Contexto do piloto no export do roster ──────────────────────────────────
// As três decisões que o export toma sobre CADA piloto antes de escrever o roster, e que
// saíram de dentro do comando de 613 linhas justamente para poderem ser conferidas sem
// banco e sem o sim aberto.

#[test]
fn o_duelo_interno_olha_o_melhor_companheiro_e_nao_o_proprio_piloto() {
    let time = vec![
        ("p1".to_string(), 120.0),
        ("p2".to_string(), 240.0),
        ("p3".to_string(), 80.0),
    ];
    assert_eq!(pontos_do_melhor_companheiro(&time, "p1"), Some(240.0));
    // O melhor do time não se mede contra si mesmo: para ele o duelo é com o segundo.
    assert_eq!(pontos_do_melhor_companheiro(&time, "p2"), Some(120.0));
}

#[test]
fn carro_unico_no_time_nao_tem_duelo_interno() {
    // `None` e "companheiro com zero ponto" são coisas diferentes: com um carro só não há
    // com quem duelar, e inventar o zero daria ao piloto a sensação de estar ganhando.
    let time = vec![("p1".to_string(), 0.0)];
    assert_eq!(pontos_do_melhor_companheiro(&time, "p1"), None);
    let zerados = vec![("p1".to_string(), 0.0), ("p2".to_string(), 0.0)];
    assert_eq!(pontos_do_melhor_companheiro(&zerados, "p1"), Some(0.0));
}

#[test]
fn o_percentil_mundial_vai_de_um_no_topo_a_zero_na_lanterna() {
    assert!((percentil_mundial(1, 100) - 1.0).abs() < 1e-9);
    assert!(percentil_mundial(100, 100).abs() < 1e-9);
    assert!((percentil_mundial(50, 99) - 0.5).abs() < 1e-9);
}

#[test]
fn ranking_de_um_piloto_so_devolve_o_neutro() {
    // Sem ninguém para comparar, 1.0 daria ao único piloto do mundo o bônus de quem venceu
    // todos os outros. O ranking vazio cai no mesmo caso.
    assert!((percentil_mundial(1, 1) - 0.5).abs() < 1e-9);
    assert!((percentil_mundial(1, 0) - 0.5).abs() < 1e-9);
}

#[test]
fn trocar_de_equipe_exige_time_novo_nesta_temporada_e_outro_time_antes() {
    let historico = vec![(3, "t_velho".to_string()), (5, "t_novo".to_string())];
    assert!(trocou_de_equipe(5, "t_novo", 5, &historico));
}

#[test]
fn rookie_no_primeiro_time_nao_trocou_de_equipe() {
    let historico = vec![(5, "t_novo".to_string())];
    assert!(!trocou_de_equipe(5, "t_novo", 5, &historico));
}

#[test]
fn renovar_com_o_mesmo_time_nao_e_troca() {
    // Contrato novo nesta temporada, mas o passado é todo do MESMO time: ele ficou.
    let historico = vec![(2, "t_mesmo".to_string()), (5, "t_mesmo".to_string())];
    assert!(!trocou_de_equipe(5, "t_mesmo", 5, &historico));
}

#[test]
fn contrato_assinado_em_temporada_passada_nao_conta_como_troca() {
    // A troca é o assunto DESTA temporada; quem mudou há dois anos já não corre com raiva.
    let historico = vec![(1, "t_velho".to_string()), (3, "t_novo".to_string())];
    assert!(!trocou_de_equipe(3, "t_novo", 5, &historico));
}

// ─── Banda de skill: o contrato com o `normalize_to_roster` ──────────────────
// O roster normaliza os alvos em 0–100 e devolve a banda ABSOLUTA; a temporada escreve
// essa banda como `minSkill`/`maxSkill`, e o iRacing estica o roster para preencher a
// faixa. Os casos degenerados são convenção do lado que gera (`roster_gen.rs`) e são
// conferidos aqui, no lado que escreve, para que os dois envelheçam juntos.

#[test]
fn a_banda_vira_os_dois_inteiros_que_o_iracing_aceita() {
    assert_eq!(limites_da_banda(62.4, 88.6), (62, 89));
}

#[test]
fn grid_de_um_piloto_nunca_escreve_faixa_de_largura_zero() {
    // `normalize_to_roster` devolve `min + 1.0` quando não há faixa para esticar (um
    // piloto só, ou empate exato de skill). É essa convenção que impede o `minSkill` de
    // sair igual ao `maxSkill` — o valor pelo qual o esticão do iRacing divide.
    let (skills, banda) = crate::iracing_sdk::roster_gen::normalize_to_roster(&[74.0]);
    assert_eq!(skills, vec![50]);
    let (min, max) = limites_da_banda(banda.min, banda.max);
    assert!(
        max > min,
        "faixa degenerada chegou ao arquivo: {min}..{max}"
    );

    let (empatados, banda_empate) =
        crate::iracing_sdk::roster_gen::normalize_to_roster(&[74.0, 74.0, 74.0]);
    assert_eq!(empatados, vec![50, 50, 50]);
    let (min, max) = limites_da_banda(banda_empate.min, banda_empate.max);
    assert!(
        max > min,
        "empate exato chegou como faixa nula: {min}..{max}"
    );
}

#[test]
fn grid_vazio_nao_escreve_faixa_invalida() {
    // Lista vazia devolve 0..1 por convenção. O comando nem chega aqui (falha antes, com
    // "Nenhum piloto de IA"), mas se chegar o arquivo continua válido.
    let (skills, banda) = crate::iracing_sdk::roster_gen::normalize_to_roster(&[]);
    assert!(skills.is_empty());
    assert_eq!(limites_da_banda(banda.min, banda.max), (0, 1));
}

#[test]
fn o_arredondamento_nunca_poe_o_minimo_acima_do_maximo() {
    // Faixa estreitíssima: os dois arredondam para o mesmo inteiro. O `min` é limitado
    // pelo `max` já arredondado, então o arquivo nunca sai com minSkill > maxSkill.
    let (min, max) = limites_da_banda(88.6, 88.7);
    assert!(min <= max, "{min} > {max}");
}

#[test]
fn a_banda_respeita_o_teto_de_125_do_iracing() {
    assert_eq!(limites_da_banda(-4.0, 900.0), (0, 125));
}
