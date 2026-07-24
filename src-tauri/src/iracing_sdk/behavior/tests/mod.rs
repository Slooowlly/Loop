use crate::iracing_sdk::behavior::*;
use crate::simulation::pressure::{self, TitleContext};
use crate::simulation::track_knowledge::TrackKnowledge;

fn tc(in_contention: bool, is_leader: bool, decided: bool) -> TitleContext {
    TitleContext {
        in_contention,
        is_leader,
        title_decided: decided,
    }
}

fn neutral_inputs() -> BehaviorInputs {
    BehaviorInputs {
        base_aggression: 50.0,
        base_optimism: 50.0,
        base_smoothness: 50.0,
        base_skill: 60.0,
        mentality: 50.0,
        resilience: 0.5,
        title: tc(false, false, false), // fora de briga → sem pressão
        races_left: 8,
        event_stakes: 0.0, // sem casa cheia por padrão nos testes neutros
        recent_positions: Vec::new(),
        field_size: 20,
        season_length: 10,
        track: TrackKnowledge {
            starts: 5,
            best_finish: Some(6),
            last_season: Some(1),
        },
        is_wet: false,
        fator_chuva: 50.0,
        rain_intensity: 0.0,
        temp_c: 20.0,
        age: 27,
        global_rank_percentile: 0.5,
        grid_rank_percentile: 0.5,
        home_race: false,
        career_wins: 3,
        season_points: 50.0,
        contract_last_year: false,
        teammate_points: None,
        category_move: 0,
        team_morale: 1.0,
        all_points: vec![10.0, 20.0, 30.0],
        max_points: 26.0,
        injury_return: false,
        honeymoon: false,
        crashed_out_last_race: false,
        not_at_fault_dnfs: 0,
        track_crash: false,
        nemesis: false,
        switched_teams: false,
        reigning_champion: false,
        career_debut: false,
        mechanical_dnfs: 0,
        fame: 50.0,    // neutro → stardom sem empurrão
        bond_level: 3, // neutro → team_bond sem empurrão
        injury_active_penalty: 0.0,
        seed: 1,
    }
}

#[test]
fn mentalidade_e_o_ganho() {
    assert!(malleability(100.0) < malleability(0.0));
    assert!((malleability(50.0) - 1.0).abs() < 1e-9);
}

#[test]
fn pressao_choke_vs_clutch() {
    let title = tc(true, false, false);
    let choke = pressure_title(&title, 1, 0.15); // frágil
    let clutch = pressure_title(&title, 1, 0.9); // resiliente
    assert!(choke.nudge.aggression > 0.0 && choke.nudge.smoothness < 0.0 && choke.adverse);
    assert!(clutch.nudge.aggression < 0.0 && clutch.nudge.smoothness > 0.0 && !clutch.adverse);
}

#[test]
fn pace_export_respeita_headroom() {
    let title = tc(true, false, false);
    // Choke forte (frágil, última corrida): craque (skill alto, espaço p/ cair)
    // desaba MAIS que o azarão (perto do chão, quase não sente).
    let choke = pressure_title(&title, 1, 0.05).nudge;
    let drop_craque = 90.0 - compose(50.0, 50.0, 50.0, 90.0, 0.0, &[choke]).skill;
    let drop_azarao = 55.0 - compose(50.0, 50.0, 50.0, 55.0, 0.0, &[choke]).skill;
    assert!(
        drop_craque > drop_azarao,
        "craque desaba mais: {drop_craque} vs {drop_azarao}"
    );
    assert!(drop_craque <= 10.0, "não pode explodir: {drop_craque}");
    // Clutch (resiliente): azarão sobe MAIS que o craque (espaço até o teto).
    let clutch = pressure_title(&title, 1, 0.95).nudge;
    let up_craque = compose(50.0, 50.0, 50.0, 90.0, 0.0, &[clutch]).skill - 90.0;
    let up_azarao = compose(50.0, 50.0, 50.0, 55.0, 0.0, &[clutch]).skill - 55.0;
    assert!(
        up_azarao > up_craque,
        "azarão salta mais: {up_azarao} vs {up_craque}"
    );
}

#[test]
fn casa_cheia_export_universal_e_adversa_no_fragil() {
    // Frágil sob casa cheia (sem briga de título) → choke: mais agressivo, adverso.
    let fragil = pressure_event(1.0, &[18, 20, 19], 24, 0.20);
    assert!(fragil.nudge.aggression > 0.0 && fragil.nudge.smoothness < 0.0 && fragil.adverse);
    // Resiliente no mesmo palco → clutch: mais suave, não adverso.
    let firme = pressure_event(1.0, &[18, 20, 19], 24, 0.85);
    assert!(firme.nudge.aggression < 0.0 && firme.nudge.smoothness > 0.0 && !firme.adverse);
    // Evento sem público → nada.
    assert!(pressure_event(0.0, &[10], 24, 0.5).nudge.aggression == 0.0);
}

#[test]
fn cruzeiro_relaxa() {
    let s = pressure_title(&tc(true, true, true), 1, 0.5);
    assert!(s.nudge.aggression < 0.0 && s.nudge.smoothness > 0.0 && !s.adverse);
}

#[test]
fn forma_alta_vs_seca() {
    let hot = form(&[1, 2, 1], 20, 0.5); // pódios
    let cold = form(&[18, 19, 20], 20, 0.15); // fundo + frágil
    assert!(hot.nudge.optimism > 0.0 && hot.nudge.aggression > 0.0 && !hot.adverse);
    assert!(cold.nudge.optimism < 0.0 && cold.nudge.smoothness < 0.0 && cold.adverse);
    assert!(cold.nudge.aggression > 0.0, "seca → desespero agressivo");
}

#[test]
fn pista_nova_cautelosa_dominio_ataca() {
    let nova = track_affinity(&TrackKnowledge {
        starts: 0,
        best_finish: None,
        last_season: None,
    });
    let dom = track_affinity(&TrackKnowledge {
        starts: 5,
        best_finish: Some(2),
        last_season: Some(3),
    });
    assert!(nova.nudge.aggression < 0.0 && nova.nudge.smoothness > 0.0 && nova.adverse);
    assert!(dom.nudge.aggression > 0.0 && dom.nudge.optimism > 0.0 && !dom.adverse);
}

#[test]
fn chuva_teme_vs_mestre() {
    let teme = weather(true, 10.0, 1.0);
    let mestre = weather(true, 95.0, 1.0);
    assert!(teme.nudge.aggression < 0.0 && teme.nudge.smoothness > 0.0 && teme.adverse);
    assert!(mestre.nudge.aggression > 0.0 && mestre.nudge.smoothness < 0.0 && !mestre.adverse);
    assert_eq!(weather(false, 10.0, 1.0).nudge.aggression, 0.0); // seco = nada
}

#[test]
fn jovem_vs_veterano() {
    let jovem = age_phase(18);
    let vet = age_phase(38);
    assert!(jovem.nudge.aggression > 0.0 && jovem.nudge.smoothness < 0.0);
    assert!(vet.nudge.aggression < 0.0 && vet.nudge.smoothness > 0.0);
    assert_eq!(age_phase(27).nudge.aggression, 0.0); // auge = neutro
}

#[test]
fn status_grid_pesa_mais_que_fama() {
    let alfa_local = status(0.5, 1.0);
    let craque_anonimo = status(1.0, 0.4);
    assert!(alfa_local.nudge.optimism > craque_anonimo.nudge.optimism);
    assert!(status(0.1, 0.1).nudge.optimism < 0.0 && status(0.1, 0.1).adverse);
}

#[test]
fn stack_cauteloso_vira_a_mao_do_agressivo() {
    let cautelosos = [
        track_affinity(&TrackKnowledge {
            starts: 0,
            best_finish: None,
            last_season: None,
        })
        .nudge,
        weather(true, 5.0, 1.0).nudge,
        age_phase(39).nudge,
        heat(38.0).nudge,
    ];
    let fraco = compose(92.0, 50.0, 50.0, 70.0, 10.0, &cautelosos);
    let forte = compose(92.0, 50.0, 50.0, 70.0, 95.0, &cautelosos);
    assert!(
        fraco.aggression < 60.0,
        "fraco devia cair muito: {}",
        fraco.aggression
    );
    assert!(forte.aggression > fraco.aggression, "forte resiste mais");
}

#[test]
fn wobble_deterministico() {
    assert_eq!(wobble(42).nudge.aggression, wobble(42).nudge.aggression);
    assert!(wobble(1).nudge.aggression != wobble(2).nudge.aggression);
}

#[test]
fn tier2_sinais() {
    // Streak: 3 vitórias seguidas → swagger; 1 só → nada.
    assert!(win_streak(&[1, 1, 1]).nudge.optimism > 0.0);
    assert_eq!(win_streak(&[1, 4, 1]).nudge.optimism, 0.0);
    // Quase lá: pódios sem vitória → coceira de risco; com vitória → nada.
    assert!(near_miss(&[2, 3, 2]).nudge.aggression > 0.0);
    assert_eq!(near_miss(&[1, 3, 2]).nudge.aggression, 0.0);
    // Desgaste: fim de temporada longa = adverso; meio de temporada = nada.
    let f = end_season_fatigue(1, 14);
    assert!(f.nudge.optimism < 0.0 && f.adverse);
    assert_eq!(end_season_fatigue(8, 14).nudge.optimism, 0.0);
    // Prodígio: jovem indo bem → confiança; jovem indo mal ou veterano → nada.
    assert!(rising_prodigy(18, &[1, 2, 1], 20).nudge.optimism > 0.0);
    assert_eq!(rising_prodigy(18, &[18, 19], 20).nudge.optimism, 0.0);
    assert_eq!(rising_prodigy(30, &[1, 1], 20).nudge.optimism, 0.0);
}

#[test]
fn tier2b_sinais() {
    // Caça a marco: 99→100ª = fogo; 12→13 = nada.
    assert!(milestone_chase(99).nudge.aggression > 0.0);
    assert_eq!(milestone_chase(12).nudge.aggression, 0.0);
    // Contrato último ano: showboat + adverso; frágil fica mais bruto que resiliente.
    let frag = contract_year(true, 0.1);
    let res = contract_year(true, 0.9);
    assert!(frag.nudge.aggression > 0.0 && frag.adverse);
    assert!(frag.nudge.smoothness < res.nudge.smoothness);
    assert_eq!(contract_year(false, 0.5).nudge.aggression, 0.0);
    // Duelo interno: apanhando do companheiro → agressivo; à frente → nada.
    assert!(teammate_duel(30.0, Some(80.0)).nudge.aggression > 0.0);
    assert_eq!(teammate_duel(80.0, Some(30.0)).nudge.aggression, 0.0);
    assert_eq!(teammate_duel(50.0, None).nudge.aggression, 0.0);
    // Categoria: promovido = cauteloso/adverso; rebaixado = swagger.
    assert!(category_move(1).nudge.smoothness > 0.0 && category_move(1).adverse);
    assert!(category_move(-1).nudge.aggression > 0.0 && !category_move(-1).adverse);
    assert_eq!(category_move(0).nudge.aggression, 0.0);
    // Moral: feliz → otimismo/suave; infeliz → frustrado/adverso.
    assert!(team_morale(1.4).nudge.optimism > 0.0 && !team_morale(1.4).adverse);
    let unhappy = team_morale(0.6);
    assert!(unhappy.nudge.aggression > 0.0 && unhappy.nudge.smoothness < 0.0 && unhappy.adverse);
}

#[test]
fn tier3_sinais() {
    // Lua de mel: chegou agora → afoito (agressivo, menos suave).
    let hm = honeymoon(true);
    assert!(hm.nudge.aggression > 0.0 && hm.nudge.smoothness < 0.0 && !hm.adverse);
    assert_eq!(honeymoon(false).nudge.aggression, 0.0);
    // Vingança: tirado na última → furioso, adverso.
    assert!(revenge(true).nudge.aggression > 0.0 && revenge(true).adverse);
    assert_eq!(revenge(false).nudge.aggression, 0.0);
    // Azar: 2+ DNFs sem culpa → frustrado/adverso; 1 só = nada.
    assert!(bad_luck(3).nudge.aggression > 0.0 && bad_luck(3).nudge.smoothness < 0.0);
    assert!(bad_luck(3).adverse && bad_luck(3).nudge.aggression > bad_luck(2).nudge.aggression);
    assert_eq!(bad_luck(1).nudge.aggression, 0.0);
    // Trauma de pista: bateu aqui antes → cauteloso/adverso.
    let tr = track_trauma(true);
    assert!(tr.nudge.aggression < 0.0 && tr.nudge.smoothness > 0.0 && tr.adverse);
    assert_eq!(track_trauma(false).nudge.smoothness, 0.0);
}

#[test]
fn lote_novo_sinais() {
    // Nêmesis: rivalidade → agressivo e menos suave, adverso; sem rival = nada.
    let nem = nemesis(true);
    assert!(nem.nudge.aggression > 0.0 && nem.nudge.smoothness < 0.0 && nem.adverse);
    assert_eq!(nemesis(false).nudge.aggression, 0.0);
    // Ex-equipe: algo a provar → agressivo/otimista, adverso; sem troca = nada.
    let ex = former_team_grudge(true);
    assert!(ex.nudge.aggression > 0.0 && ex.nudge.optimism > 0.0 && ex.adverse);
    assert_eq!(former_team_grudge(false).nudge.aggression, 0.0);
    // Campeão reinante: resiliente = autoridade serena (favorável); frágil = peso
    // tenso (adverso). Não-campeão = nada.
    let autoridade = reigning_champion(true, 0.95);
    let peso = reigning_champion(true, 0.05);
    assert!(
        autoridade.nudge.optimism > 0.0
            && autoridade.nudge.smoothness > 0.0
            && !autoridade.adverse
    );
    assert!(peso.nudge.aggression > 0.0 && peso.nudge.smoothness < 0.0 && peso.adverse);
    assert_eq!(reigning_champion(false, 0.5).nudge.optimism, 0.0);
    // Estreia: nervo → recolhido (agg↓ opt↓ suav↑), adverso; veterano = nada.
    let deb = career_debut(true);
    assert!(
        deb.nudge.aggression < 0.0
            && deb.nudge.optimism < 0.0
            && deb.nudge.smoothness > 0.0
            && deb.adverse
    );
    assert_eq!(career_debut(false).nudge.smoothness, 0.0);
    // Desconfiança mecânica: poupa o carro (agg↓ suav↑), adverso, escala com DNFs;
    // 0 = nada. Contraste com bad_luck (agg↑).
    let md = mechanical_distrust(3);
    assert!(md.nudge.aggression < 0.0 && md.nudge.smoothness > 0.0 && md.adverse);
    assert!(mechanical_distrust(3).nudge.smoothness > mechanical_distrust(1).nudge.smoothness);
    assert_eq!(mechanical_distrust(0).nudge.aggression, 0.0);
    assert!(mechanical_distrust(2).nudge.aggression < 0.0 && bad_luck(2).nudge.aggression > 0.0);
    // Pista-fantasma: experiente mas ruim aqui (best P15 em 20) → resignação, adverso.
    let bogey = bogey_track(
        &TrackKnowledge {
            starts: 5,
            best_finish: Some(15),
            last_season: Some(2),
        },
        20,
    );
    assert!(
        bogey.nudge.aggression < 0.0
            && bogey.nudge.optimism < 0.0
            && bogey.nudge.smoothness > 0.0
            && bogey.adverse
    );
    // Não é bogey: pouca experiência, ou pódio aqui, ou resultado decente.
    assert_eq!(
        bogey_track(
            &TrackKnowledge {
                starts: 2,
                best_finish: Some(18),
                last_season: None
            },
            20
        )
        .nudge
        .aggression,
        0.0
    );
    assert_eq!(
        bogey_track(
            &TrackKnowledge {
                starts: 6,
                best_finish: Some(2),
                last_season: None
            },
            20
        )
        .nudge
        .aggression,
        0.0
    );
    assert_eq!(
        bogey_track(
            &TrackKnowledge {
                starts: 6,
                best_finish: Some(6),
                last_season: None
            },
            20
        )
        .nudge
        .aggression,
        0.0
    );
    // E o experiente-mas-ruim NÃO é mais tratado como domínio pelo track_affinity.
    let aff = track_affinity(&TrackKnowledge {
        starts: 6,
        best_finish: Some(15),
        last_season: Some(2),
    });
    assert_eq!(aff.nudge.aggression, 0.0, "experiência sem resultado ≠ domínio");
}

#[test]
fn fama_vinculo_e_lesao_ativa() {
    // Estrela (fama alta) → confiante + swagger, favorável; anônimo → menos otimismo, sem
    // punir agressividade; fama neutra (50) → nada.
    let astro = stardom(90.0);
    assert!(astro.nudge.optimism > 0.0 && astro.nudge.aggression > 0.0 && !astro.adverse);
    let anonimo = stardom(10.0);
    assert!(anonimo.nudge.optimism < 0.0 && anonimo.nudge.aggression == 0.0);
    assert_eq!(stardom(50.0).nudge.optimism, 0.0);
    // Vínculo alto (casa) → sereno/confiante; nível 3 = neutro; baixo → leve insegurança.
    let casa = team_bond(6);
    assert!(casa.nudge.optimism > 0.0 && casa.nudge.smoothness > 0.0 && !casa.adverse);
    assert_eq!(team_bond(3).nudge.optimism, 0.0);
    assert!(team_bond(1).nudge.optimism < 0.0 && team_bond(1).nudge.smoothness == 0.0);
    // Lesão ATIVA reduz o pace base pela rampa (skill × fração); 0 = sem efeito. É físico:
    // entra direto no pace, não some com a compostura de um mental forte.
    let mut i = neutral_inputs();
    i.base_skill = 80.0;
    let sadio = compute(&i).skill;
    i.injury_active_penalty = 0.20; // 20% do pace perdido
    let ferido = compute(&i).skill;
    assert!(ferido < sadio - 10.0, "pace ferido {ferido} << são {sadio}");
    // Mental forte não blinda o handicap físico (some do que a compostura protege).
    i.mentality = 100.0;
    assert!(compute(&i).skill < sadio - 10.0, "compostura não cura lesão");
}

#[test]
fn prize_e_lesao() {
    // Briga por grana: fim de temporada, fora do título, rival colado → tensão.
    let pf = prize_fight(40.0, &[100.0, 42.0, 40.0, 10.0], 2, 26.0, false);
    assert!(pf.nudge.aggression > 0.0 && pf.adverse);
    // Em briga de título não conta (a pressão de título cobre).
    assert_eq!(
        prize_fight(40.0, &[42.0, 40.0], 2, 26.0, true)
            .nudge
            .aggression,
        0.0
    );
    // Longe do fim não conta.
    assert_eq!(
        prize_fight(40.0, &[42.0, 40.0], 8, 26.0, false)
            .nudge
            .aggression,
        0.0
    );
    // Retorno de lesão: cauteloso + adverso.
    let inj = injury_return(true);
    assert!(inj.nudge.aggression < 0.0 && inj.nudge.smoothness > 0.0 && inj.adverse);
    assert_eq!(injury_return(false).nudge.smoothness, 0.0);
}

#[test]
fn impacto_adverso_granular() {
    // Fraco SEMPRE leva o adverso cheio (1.0). Quanto mais forte, MENOR o impacto
    // adverso médio (granular, sem extremos binários).
    for s in 0..80 {
        assert_eq!(
            adverse_multiplier(0.0, s),
            1.0,
            "fraco leva cheio (seed {s})"
        );
    }
    let avg = |m: f64| (0..500u64).map(|s| adverse_multiplier(m, s)).sum::<f64>() / 500.0;
    assert!((avg(0.0) - 1.0).abs() < 1e-9);
    assert!(avg(100.0) < avg(70.0));
    assert!(avg(70.0) < avg(40.0));
    assert!(avg(40.0) < avg(0.0));
}

#[test]
fn fraco_sempre_afetado_forte_resiste_mais() {
    // Input adverso (pista nova + medo de chuva → agressividade despenca).
    let aggression_for = |mentality: f64, seed: u64| {
        let mut i = neutral_inputs();
        i.mentality = mentality;
        i.seed = seed;
        i.track = TrackKnowledge {
            starts: 0,
            best_finish: None,
            last_season: None,
        };
        i.is_wet = true;
        i.fator_chuva = 5.0;
        i.rain_intensity = 1.0;
        compute(&i).aggression
    };
    let max_fraco = (0..300u64)
        .map(|s| aggression_for(0.0, s))
        .fold(f64::MIN, f64::max);
    let max_forte = (0..300u64)
        .map(|s| aggression_for(100.0, s))
        .fold(f64::MIN, f64::max);
    // Fraco nunca blinda → mesmo no melhor dia fica bem abaixo da base (50).
    assert!(max_fraco < 42.0, "fraco max {max_fraco}");
    // Forte, num bom dia, reduz muito o adverso → chega perto da base.
    assert!(max_forte > 45.0, "forte max {max_forte}");
}

// ─── HARNESS DE CALIBRAÇÃO ───────────────────────────────────────────────────
// Não valida nada: IMPRIME tabelas pra calibrar as magnitudes vendo números.
//   cargo test --lib behavior::tests::calibracao -- --nocapture --test-threads=1
// (rode com CARGO_TARGET_DIR fora do OneDrive)

fn row(label: &str, s: Signal) {
    let n = s.nudge;
    println!(
        "{:<26} agg {:>6.1}  opt {:>6.1}  suav {:>6.1}  pace {:>5.1}   {}",
        label,
        n.aggression,
        n.optimism,
        n.smoothness,
        n.skill,
        if s.adverse { "ADVERSO" } else { "favor." }
    );
}

/// TABELA A: peso CRU de cada sinal (antes de ganho×mentalidade e da blindagem do
/// adverso). É o "quanto cada lever vale" — a referência principal pra calibrar.
#[test]
fn calibracao_magnitudes() {
    println!("\n=== TABELA A · magnitude CRUA por sinal (gain=1, sem blindagem) ===");
    let title = tc(true, false, false);
    row("pressao_titulo choke", pressure_title(&title, 1, 0.15));
    row("pressao_titulo clutch", pressure_title(&title, 1, 0.90));
    row(
        "cruzeiro (titulo ganho)",
        pressure_title(&tc(true, true, true), 1, 0.5),
    );
    row("casa_cheia choke", pressure_event(1.0, &[18, 20, 19], 24, 0.15));
    row("casa_cheia clutch", pressure_event(1.0, &[18, 20, 19], 24, 0.90));
    row("forma_alta", form(&[1, 2, 1], 20, 0.5));
    row("forma_seca (fragil)", form(&[18, 19, 20], 20, 0.15));
    row(
        "pista_nova",
        track_affinity(&TrackKnowledge {
            starts: 0,
            best_finish: None,
            last_season: None,
        }),
    );
    row(
        "pista_dominio",
        track_affinity(&TrackKnowledge {
            starts: 5,
            best_finish: Some(2),
            last_season: Some(1),
        }),
    );
    row("chuva_teme", weather(true, 10.0, 1.0));
    row("chuva_mestre", weather(true, 95.0, 1.0));
    row("calor_extremo (38C)", heat(38.0));
    row("jovem (18)", age_phase(18));
    row("veterano (38)", age_phase(38));
    row("status_alto (alfa grid)", status(0.5, 1.0));
    row("status_baixo (fundo)", status(0.1, 0.1));
    row("casa (pais natal)", home_race(true));
    row("win_streak (3)", win_streak(&[1, 1, 1]));
    row("near_miss (podios)", near_miss(&[2, 3, 2]));
    row("fim_temporada_fadiga", end_season_fatigue(1, 14));
    row("prodigio_ascensao", rising_prodigy(18, &[1, 2, 1], 20));
    row("caca_marco (99->100)", milestone_chase(99));
    row("contrato_ult_ano (frag)", contract_year(true, 0.15));
    row("duelo_companheiro", teammate_duel(30.0, Some(80.0)));
    row("promovido", category_move(1));
    row("rebaixado", category_move(-1));
    row("moral_feliz", team_morale(1.4));
    row("moral_infeliz", team_morale(0.6));
    row(
        "briga_grana (prize)",
        prize_fight(40.0, &[100.0, 42.0, 40.0, 10.0], 2, 26.0, false),
    );
    row("retorno_lesao", injury_return(true));
    row("lua_de_mel", honeymoon(true));
    row("vinganca", revenge(true));
    row("azar (3 dnf)", bad_luck(3));
    row("trauma_pista", track_trauma(true));
    row("NEMESIS", nemesis(true));
    row("EX-EQUIPE", former_team_grudge(true));
    row("CAMPEAO autoridade", reigning_champion(true, 0.90));
    row("CAMPEAO peso (fragil)", reigning_champion(true, 0.15));
    row("ESTREIA", career_debut(true));
    row("DESCONFIANCA_MEC (3)", mechanical_distrust(3));
    row(
        "PISTA-FANTASMA (P15/20)",
        bogey_track(
            &TrackKnowledge {
                starts: 5,
                best_finish: Some(15),
                last_season: Some(2),
            },
            20,
        ),
    );
    row("wobble (humor, seed 7)", wobble(7));
    println!("(crus; no jogo cada um leva ×gain[0.6..1.4] e o ADVERSO ainda é blindado pela compostura)\n");
}

/// Roda 200 "dias" (seeds) e imprime a média final vs base — o adverso não depende
/// mais do sorteio do dia.
fn profile(label: &str, mut base: BehaviorInputs) {
    let n = 200u64;
    let (ba, bo, bs, bk) = (
        base.base_aggression,
        base.base_optimism,
        base.base_smoothness,
        base.base_skill,
    );
    let (mut a, mut o, mut s, mut k) = (0.0, 0.0, 0.0, 0.0);
    for seed in 0..n {
        base.seed = seed;
        let out = compute(&base);
        a += out.aggression;
        o += out.optimism;
        s += out.smoothness;
        k += out.skill;
    }
    let f = n as f64;
    println!(
        "{:<34} agg {:>5.1} (b{:>3.0})  opt {:>5.1} (b{:>3.0})  suav {:>5.1} (b{:>3.0})  pace {:>5.1} (b{:>3.0})",
        label, a / f, ba, o / f, bo, s / f, bs, k / f, bk,
    );
}

/// TABELA B: perfis realistas end-to-end. Mostra onde os stacks aterrissam e como a
/// mentalidade blinda o adverso.
#[test]
fn calibracao_perfis() {
    println!("\n=== TABELA B · perfis realistas (média de 200 dias) ===");

    // 1. Rookie estreante, pista nova, chuva forte, mental fraco.
    let mut p = neutral_inputs();
    p.base_aggression = 55.0;
    p.base_optimism = 50.0;
    p.base_smoothness = 48.0;
    p.base_skill = 45.0;
    p.mentality = 30.0;
    p.resilience = pressure::pressure_resilience(30.0, 10.0);
    p.age = 18;
    p.track = TrackKnowledge {
        starts: 0,
        best_finish: None,
        last_season: None,
    };
    p.is_wet = true;
    p.fator_chuva = 20.0;
    p.rain_intensity = 1.0;
    p.career_debut = true;
    profile("1. rookie estreante·pista nova·chuva", p);

    // 2. Campeão reinante frágil, defendendo, última corrida, título em jogo.
    let mut p = neutral_inputs();
    p.base_aggression = 52.0;
    p.base_optimism = 60.0;
    p.base_smoothness = 55.0;
    p.base_skill = 88.0;
    p.mentality = 35.0;
    p.resilience = pressure::pressure_resilience(35.0, 60.0);
    p.title = tc(true, false, false);
    p.races_left = 1;
    p.reigning_champion = true;
    profile("2. campeao reinante fragil·defesa", p);

    // 3. Veterano, fim de temporada, azar mecânico, moral baixa.
    let mut p = neutral_inputs();
    p.base_aggression = 48.0;
    p.base_optimism = 50.0;
    p.base_smoothness = 62.0;
    p.base_skill = 72.0;
    p.mentality = 55.0;
    p.resilience = pressure::pressure_resilience(55.0, 90.0);
    p.age = 38;
    p.races_left = 1;
    p.season_length = 14;
    p.mechanical_dnfs = 2;
    p.team_morale = 0.7;
    profile("3. veterano·fim season·azar mec", p);

    // 4. Jovem prodígio em alta, casa, win streak, mental forte.
    let mut p = neutral_inputs();
    p.base_aggression = 60.0;
    p.base_optimism = 58.0;
    p.base_smoothness = 50.0;
    p.base_skill = 78.0;
    p.mentality = 85.0;
    p.resilience = pressure::pressure_resilience(85.0, 40.0);
    p.age = 19;
    p.recent_positions = vec![1, 1, 1];
    p.home_race = true;
    profile("4. jovem prodigio·alta·casa", p);

    // 5. Rivalidade carregada: nêmesis + ex-equipe + trauma de pista.
    let mut p = neutral_inputs();
    p.base_aggression = 58.0;
    p.base_optimism = 52.0;
    p.base_smoothness = 54.0;
    p.base_skill = 74.0;
    p.mentality = 50.0;
    p.resilience = pressure::pressure_resilience(50.0, 50.0);
    p.nemesis = true;
    p.switched_teams = true;
    p.track_crash = true;
    profile("5. nemesis+ex-equipe+trauma", p);

    println!("\n--- combos EXTREMOS (tudo alinhado) ---");
    // 6. COMBO FAVORÁVEL MÁXIMO (mental fraco = gain 1.4 amplifica o favorável):
    // jovem prodígio, tri-vitória, casa, status topo, domínio de pista, mestre de
    // chuva, rebaixado (swagger), lua de mel, caça-marco, moral alta. Empurra
    // agg/opt pro TETO 100.
    let mut p = neutral_inputs();
    p.base_aggression = 55.0;
    p.base_optimism = 55.0;
    p.base_smoothness = 50.0;
    p.base_skill = 78.0;
    p.mentality = 20.0; // fraco → gain alto → favorável amplificado
    p.resilience = pressure::pressure_resilience(20.0, 20.0);
    p.age = 18;
    p.recent_positions = vec![1, 1, 1];
    p.field_size = 20;
    p.home_race = true;
    p.global_rank_percentile = 1.0;
    p.grid_rank_percentile = 1.0;
    p.track = TrackKnowledge {
        starts: 6,
        best_finish: Some(1),
        last_season: Some(1),
    };
    p.is_wet = true;
    p.fator_chuva = 95.0;
    p.rain_intensity = 1.0;
    p.category_move = -1; // rebaixado → swagger
    p.honeymoon = true;
    p.career_wins = 99; // próxima = marco 100
    p.team_morale = 1.5;
    profile("6. COMBO favoravel max (fraco)", p);

    // 7. COMBO ADVERSO MÁXIMO (mental fraco, tudo contra, sinais que RECOLHEM):
    // pista nova, teme a chuva, veterano, calor, retorno de lesão, promovido,
    // desconfiança mecânica, trauma de pista. Empurra agg/opt pro CHÃO 0 e suav pro
    // teto. (Sem choke/vingança/nêmesis, que SOBEM a agressividade.)
    let mut p = neutral_inputs();
    p.base_aggression = 45.0;
    p.base_optimism = 45.0;
    p.base_smoothness = 55.0;
    p.base_skill = 60.0;
    p.mentality = 5.0; // fraco → não blinda nada, gain máximo
    p.resilience = pressure::pressure_resilience(5.0, 5.0);
    p.age = 38; // veterano
    p.field_size = 20;
    p.track = TrackKnowledge {
        starts: 0,
        best_finish: None,
        last_season: None,
    }; // nova
    p.is_wet = true;
    p.fator_chuva = 5.0; // teme a chuva
    p.rain_intensity = 1.0;
    p.temp_c = 39.0; // calor extremo
    p.injury_return = true;
    p.category_move = 1; // promovido → cautela
    p.mechanical_dnfs = 3;
    p.track_crash = true; // trauma
    profile("7. COMBO adverso max (fraco)", p);
    println!();
}
