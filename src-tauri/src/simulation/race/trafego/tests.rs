use super::*;
use rand::{rngs::StdRng, SeedableRng};

use crate::models::driver::Driver;
use crate::models::team::placeholder_team_from_db;

fn piloto(id: &str, racecraft: f64, defesa: f64, aggression: f64) -> SimDriver {
    let mut d = Driver::create_player(id.to_string(), format!("P{id}"), "BR".to_string(), 25);
    d.is_jogador = false;
    d.atributos.racecraft = racecraft;
    d.atributos.defesa = defesa;
    d.atributos.aggression = aggression;
    let team = placeholder_team_from_db(
        format!("T{id}"),
        format!("E{id}"),
        "gt4".to_string(),
        "2026-01-01T00:00:00".to_string(),
    );
    SimDriver::from_driver_and_team(&d, &team)
}

// ─────────────────────── Ar sujo ───────────────────────

#[test]
fn ar_sujo_so_vale_dentro_da_janela() {
    // Colado dói; na borda da janela, nada.
    assert!(perda_por_ar_sujo(0.0, 1.0) > 0.0);
    assert_eq!(perda_por_ar_sujo(JANELA_AR_SUJO_MS, 1.0), 0.0);
    assert_eq!(perda_por_ar_sujo(JANELA_AR_SUJO_MS * 3.0, 1.0), 0.0);
    // Gap negativo (carro à frente por arredondamento) não vira bônus.
    assert_eq!(perda_por_ar_sujo(-50.0, 1.0), 0.0);
}

#[test]
fn ar_sujo_cresce_conforme_cola() {
    let longe = perda_por_ar_sujo(900.0, 0.0);
    let perto = perda_por_ar_sujo(200.0, 0.0);
    assert!(perto > longe, "{perto} vs {longe}");
}

#[test]
fn carro_de_apoio_sofre_mais_que_carro_de_ponta() {
    // O eixo é o mesmo `vies_de_pico` do trim de classificação: apoio (handling) vive de
    // aerodinâmica e sofre atrás; ponta (power) dependia menos dela.
    let apoio = perda_por_ar_sujo(300.0, 1.0);
    let neutro = perda_por_ar_sujo(300.0, 0.0);
    let ponta = perda_por_ar_sujo(300.0, -1.0);
    assert!(apoio > neutro && neutro > ponta, "{apoio} {neutro} {ponta}");
    assert_eq!(
        ponta, 0.0,
        "carro de ponta pura não perde apoio que não usava"
    );
}

#[test]
fn ar_sujo_respeita_o_teto() {
    for gap in [0.0, 100.0, 500.0, 999.0] {
        assert!(perda_por_ar_sujo(gap, 1.0) <= PERDA_MAXIMA_AR_SUJO_PONTOS + 1e-9);
    }
}

// ─────────────────────── Ultrapassagem ───────────────────────

#[test]
fn sem_vantagem_de_ritmo_nao_se_passa() {
    assert_eq!(prob_de_ultrapassagem(0.0, 80.0, 20.0, 90.0, 1.0), 0.0);
    assert_eq!(prob_de_ultrapassagem(-2.0, 80.0, 20.0, 90.0, 1.0), 0.0);
}

#[test]
fn mais_rapido_passa_mais() {
    let pouco = prob_de_ultrapassagem(0.5, 60.0, 60.0, 50.0, 1.0);
    let muito = prob_de_ultrapassagem(5.0, 60.0, 60.0, 50.0, 1.0);
    assert!(muito > pouco, "{muito} vs {pouco}");
}

#[test]
fn racecraft_ataca_e_defesa_segura() {
    // `defesa` estava no `SimDriver` desde sempre e nunca tinha sido lida por ninguém.
    // Este é o pacote que a consome.
    let contra_defensor_fraco = prob_de_ultrapassagem(3.0, 80.0, 20.0, 50.0, 1.0);
    let contra_defensor_forte = prob_de_ultrapassagem(3.0, 80.0, 90.0, 50.0, 1.0);
    assert!(
        contra_defensor_fraco > contra_defensor_forte,
        "{contra_defensor_fraco} vs {contra_defensor_forte}"
    );

    let atacante_bom = prob_de_ultrapassagem(3.0, 90.0, 50.0, 50.0, 1.0);
    let atacante_ruim = prob_de_ultrapassagem(3.0, 20.0, 50.0, 50.0, 1.0);
    assert!(atacante_bom > atacante_ruim);
}

#[test]
fn agressivo_passa_mais_e_bate_mais() {
    // O trade-off que faltava à `aggression`.
    let agressivo = prob_de_ultrapassagem(3.0, 60.0, 60.0, 95.0, 1.0);
    let cauteloso = prob_de_ultrapassagem(3.0, 60.0, 60.0, 15.0, 1.0);
    assert!(agressivo > cauteloso, "{agressivo} vs {cauteloso}");

    assert!(prob_de_contato(95.0, 1.0) > prob_de_contato(15.0, 1.0));
}

#[test]
fn pista_dificil_segura_a_ultrapassagem() {
    // O `overtaking_difficulty_multiplier` é calculado em `profile/`, carregado no contexto
    // e — até este pacote — nunca lido por ninguém. Agora ele decide.
    let facil = prob_de_ultrapassagem(3.0, 60.0, 60.0, 50.0, 0.7);
    let neutra = prob_de_ultrapassagem(3.0, 60.0, 60.0, 50.0, 1.0);
    let dificil = prob_de_ultrapassagem(3.0, 60.0, 60.0, 50.0, 1.6);
    assert!(
        facil > neutra && neutra > dificil,
        "{facil} {neutra} {dificil}"
    );
}

#[test]
fn probabilidade_fica_no_intervalo() {
    for delta in [0.1, 1.0, 5.0, 50.0] {
        for dif in [0.1, 0.5, 1.0, 2.0, 5.0] {
            let p = prob_de_ultrapassagem(delta, 99.0, 0.0, 99.0, dif);
            assert!((0.0..=0.95).contains(&p), "p={p}");
        }
    }
}

#[test]
fn tentativa_devolve_os_tres_desfechos() {
    let atacante = piloto("A", 70.0, 50.0, 80.0);
    let defensor = piloto("D", 50.0, 70.0, 50.0);
    let mut rng = StdRng::seed_from_u64(7);
    let mut concluidas = 0;
    let mut falhas = 0;
    let mut contatos = 0;
    for _ in 0..2000 {
        match tentar_ultrapassagem(&atacante, &defensor, 3.0, 1.0, true, &mut rng) {
            DesfechoDaTentativa::Concluida => concluidas += 1,
            DesfechoDaTentativa::Falhou => falhas += 1,
            DesfechoDaTentativa::FalhouComContato => contatos += 1,
        }
    }
    assert!(
        concluidas > 0 && falhas > 0 && contatos > 0,
        "{concluidas}/{falhas}/{contatos}"
    );
    // A taxa de sucesso deixou de ser implicitamente 100%.
    assert!(concluidas < 2000);
}

#[test]
fn sem_incidentes_ligados_nao_ha_contato() {
    let atacante = piloto("A", 70.0, 50.0, 99.0);
    let defensor = piloto("D", 50.0, 70.0, 50.0);
    let mut rng = StdRng::seed_from_u64(9);
    for _ in 0..500 {
        assert_ne!(
            tentar_ultrapassagem(&atacante, &defensor, 3.0, 1.0, false, &mut rng),
            DesfechoDaTentativa::FalhouComContato
        );
    }
}

// ─────────────────────── Observáveis ───────────────────────

#[test]
fn sequencia_de_preso_conta_o_maior_trecho_consecutivo() {
    let mut t = TrafegoDoCarro::default();
    for preso in [true, true, false, true, true, true] {
        t.marcar_preso(preso);
    }
    assert_eq!(t.maior_sequencia_preso, 3);
    assert_eq!(t.sequencia_preso_atual, 3);

    let mut t2 = TrafegoDoCarro::default();
    t2.marcar_preso(false);
    assert_eq!(t2.maior_sequencia_preso, 0);
}
