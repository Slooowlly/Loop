use super::*;
use rand::{rngs::StdRng, SeedableRng};

/// Os parâmetros de PRODUÇÃO. Estes testes descrevem o comportamento que o jogo roda, então
/// eles medem o padrão — a variação existe para o harness, não para a asserção de propriedade.
const PAR: &ParametrosDeTrafego = &ParametrosDeTrafego::PADRAO;

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
    assert!(perda_por_ar_sujo(0.0, 1.0, PAR) > 0.0);
    assert_eq!(perda_por_ar_sujo(JANELA_AR_SUJO_MS, 1.0, PAR), 0.0);
    assert_eq!(perda_por_ar_sujo(JANELA_AR_SUJO_MS * 3.0, 1.0, PAR), 0.0);
    // Gap negativo (carro à frente por arredondamento) não vira bônus.
    assert_eq!(perda_por_ar_sujo(-50.0, 1.0, PAR), 0.0);
}

#[test]
fn ar_sujo_cresce_conforme_cola() {
    let longe = perda_por_ar_sujo(900.0, 0.0, PAR);
    let perto = perda_por_ar_sujo(200.0, 0.0, PAR);
    assert!(perto > longe, "{perto} vs {longe}");
}

#[test]
fn carro_de_apoio_sofre_mais_que_carro_de_ponta() {
    // O eixo é o mesmo `vies_de_pico` do trim de classificação: apoio (handling) vive de
    // aerodinâmica e sofre atrás; ponta (power) dependia menos dela.
    let apoio = perda_por_ar_sujo(300.0, 1.0, PAR);
    let neutro = perda_por_ar_sujo(300.0, 0.0, PAR);
    let ponta = perda_por_ar_sujo(300.0, -1.0, PAR);
    assert!(apoio > neutro && neutro > ponta, "{apoio} {neutro} {ponta}");
    assert_eq!(
        ponta, 0.0,
        "carro de ponta pura não perde apoio que não usava"
    );
}

#[test]
fn ar_sujo_respeita_o_teto() {
    for gap in [0.0, 100.0, 500.0, 999.0] {
        assert!(perda_por_ar_sujo(gap, 1.0, PAR) <= PERDA_MAXIMA_AR_SUJO_PONTOS + 1e-9);
    }
}

// ─────────────────────── Ultrapassagem ───────────────────────

#[test]
fn sem_vantagem_de_ritmo_nao_se_passa() {
    assert_eq!(
        prob_de_ultrapassagem(0.0, 80.0, 20.0, 90.0, 1.0, 0.0, PAR),
        0.0
    );
    assert_eq!(
        prob_de_ultrapassagem(-2.0, 80.0, 20.0, 90.0, 1.0, 0.0, PAR),
        0.0
    );
}

#[test]
fn mais_rapido_passa_mais() {
    let pouco = prob_de_ultrapassagem(0.5, 60.0, 60.0, 50.0, 1.0, 0.0, PAR);
    let muito = prob_de_ultrapassagem(5.0, 60.0, 60.0, 50.0, 1.0, 0.0, PAR);
    assert!(muito > pouco, "{muito} vs {pouco}");
}

#[test]
fn racecraft_ataca_e_defesa_segura() {
    // `defesa` estava no `SimDriver` desde sempre e nunca tinha sido lida por ninguém.
    // Este é o pacote que a consome.
    let contra_defensor_fraco = prob_de_ultrapassagem(3.0, 80.0, 20.0, 50.0, 1.0, 0.0, PAR);
    let contra_defensor_forte = prob_de_ultrapassagem(3.0, 80.0, 90.0, 50.0, 1.0, 0.0, PAR);
    assert!(
        contra_defensor_fraco > contra_defensor_forte,
        "{contra_defensor_fraco} vs {contra_defensor_forte}"
    );

    let atacante_bom = prob_de_ultrapassagem(3.0, 90.0, 50.0, 50.0, 1.0, 0.0, PAR);
    let atacante_ruim = prob_de_ultrapassagem(3.0, 20.0, 50.0, 50.0, 1.0, 0.0, PAR);
    assert!(atacante_bom > atacante_ruim);
}

#[test]
fn agressivo_passa_mais_e_bate_mais() {
    // O trade-off que faltava à `aggression`.
    let agressivo = prob_de_ultrapassagem(3.0, 60.0, 60.0, 95.0, 1.0, 0.0, PAR);
    let cauteloso = prob_de_ultrapassagem(3.0, 60.0, 60.0, 15.0, 1.0, 0.0, PAR);
    assert!(agressivo > cauteloso, "{agressivo} vs {cauteloso}");

    assert!(prob_de_contato(95.0, 1.0) > prob_de_contato(15.0, 1.0));
}

#[test]
fn pista_dificil_segura_a_ultrapassagem() {
    // O `overtaking_difficulty_multiplier` é calculado em `profile/`, carregado no contexto
    // e — até este pacote — nunca lido por ninguém. Agora ele decide.
    let facil = prob_de_ultrapassagem(3.0, 60.0, 60.0, 50.0, 0.7, 0.0, PAR);
    let neutra = prob_de_ultrapassagem(3.0, 60.0, 60.0, 50.0, 1.0, 0.0, PAR);
    let dificil = prob_de_ultrapassagem(3.0, 60.0, 60.0, 50.0, 1.6, 0.0, PAR);
    assert!(
        facil > neutra && neutra > dificil,
        "{facil} {neutra} {dificil}"
    );
}

#[test]
fn probabilidade_fica_no_intervalo() {
    for delta in [0.1, 1.0, 5.0, 50.0] {
        for dif in [0.1, 0.5, 1.0, 2.0, 5.0] {
            let p = prob_de_ultrapassagem(delta, 99.0, 0.0, 99.0, dif, 0.0, PAR);
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
        match tentar_ultrapassagem(&atacante, &defensor, 3.0, 1.0, true, 0.0, PAR, &mut rng) {
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
            tentar_ultrapassagem(&atacante, &defensor, 3.0, 1.0, false, 0.0, PAR, &mut rng),
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

// ─────────────────────── Rivalidade em pista ───────────────────────

/// A escada de rivalidade só passa a mexer em pista de `clara` para cima, e satura
/// em `intensa`. Abaixo disso ela é memória e manchete, não comportamento.
#[test]
fn fator_de_rivalidade_entra_na_clara_e_satura_na_intensa() {
    assert_eq!(fator_de_rivalidade(0.0), 0.0);
    assert_eq!(fator_de_rivalidade(39.9), 0.0);
    assert_eq!(fator_de_rivalidade(40.0), 0.0);
    assert!((fator_de_rivalidade(60.0) - 0.5).abs() < 1e-9);
    assert_eq!(fator_de_rivalidade(80.0), 1.0);
    assert_eq!(fator_de_rivalidade(100.0), 1.0);
}

/// SEM RIVALIDADE, NADA MUDA. O fator 0 tem que reproduzir exatamente o
/// comportamento anterior — a calibração da corrida depende disso.
#[test]
fn rivalidade_zero_nao_altera_a_ultrapassagem() {
    let sem = prob_de_ultrapassagem(3.0, 70.0, 50.0, 60.0, 1.0, 0.0, PAR);
    assert!(sem > 0.0);
    // Sem vantagem de ritmo e sem rivalidade continua sendo zero.
    assert_eq!(
        prob_de_ultrapassagem(0.0, 70.0, 50.0, 60.0, 1.0, 0.0, PAR),
        0.0
    );
    assert_eq!(
        prob_de_ultrapassagem(-2.0, 70.0, 50.0, 60.0, 1.0, 0.0, PAR),
        0.0
    );
}

/// Contra o rival o piloto VAI mesmo sem carro para a manobra — mas tentar não é
/// passar: a chance sai do piso emprestado, bem abaixo da de quem tem ritmo.
#[test]
fn contra_rival_ataca_sem_ter_ritmo_mas_com_chance_baixa() {
    let sem_ritmo_contra_rival = prob_de_ultrapassagem(0.0, 70.0, 50.0, 60.0, 1.0, 1.0, PAR);
    let com_ritmo_de_sobra = prob_de_ultrapassagem(6.0, 70.0, 50.0, 60.0, 1.0, 0.0, PAR);

    assert!(
        sem_ritmo_contra_rival > 0.0,
        "contra rival, o piloto tenta assim mesmo"
    );
    assert!(
        sem_ritmo_contra_rival < com_ritmo_de_sobra,
        "tentar nao pode valer o mesmo que ter carro: {sem_ritmo_contra_rival} vs {com_ritmo_de_sobra}"
    );
}

/// E CEDE MENOS: com o mesmo ritmo e os mesmos atributos, passar o rival é mais
/// difícil do que passar um estranho. É o que produz duelo longo em vez de
/// ultrapassagem limpa — e duelo longo é chegada colada.
#[test]
fn contra_rival_a_defesa_endurece() {
    // O delta tem que ficar ABAIXO de `delta_de_ritmo_que_satura` (2,0 desde a calibração de
    // B9): saturado, os dois lados batem no teto de 0,95 e a comparação não mede a defesa.
    let delta = PAR.delta_de_ritmo_que_satura / 2.0;
    let estranho = prob_de_ultrapassagem(delta, 70.0, 50.0, 60.0, 1.0, 0.0, PAR);
    let rival = prob_de_ultrapassagem(delta, 70.0, 50.0, 60.0, 1.0, 1.0, PAR);
    assert!(
        rival < estranho,
        "passar o rival tem que ser mais dificil: {rival} vs {estranho}"
    );
}

/// O duelo é gravado dos dois lados, então basta um apontar para o outro.
#[test]
fn fator_do_par_so_vale_entre_os_dois_do_duelo() {
    let mut jogador = piloto("1", 70.0, 50.0, 60.0);
    let mut rival = piloto("2", 70.0, 50.0, 60.0);
    let estranho = piloto("3", 70.0, 50.0, 60.0);

    jogador.duelo_de_pista = Some((rival.id.clone(), 80.0));
    rival.duelo_de_pista = Some((jogador.id.clone(), 80.0));

    assert_eq!(fator_de_rivalidade_do_par(&jogador, &rival), 1.0);
    assert_eq!(fator_de_rivalidade_do_par(&rival, &jogador), 1.0);
    assert_eq!(fator_de_rivalidade_do_par(&jogador, &estranho), 0.0);
}

/// DOIS RIVAIS DO JOGADOR NÃO SÃO RIVAIS ENTRE SI. Os dois têm duelo gravado, mas
/// apontando para o jogador — sem exigir o apontamento, eles se enfrentariam como
/// se tivessem uma história que não existe.
#[test]
fn dois_rivais_do_jogador_nao_duelam_entre_si() {
    let mut um = piloto("1", 70.0, 50.0, 60.0);
    let mut outro = piloto("2", 70.0, 50.0, 60.0);
    um.duelo_de_pista = Some(("JOGADOR".to_string(), 90.0));
    outro.duelo_de_pista = Some(("JOGADOR".to_string(), 85.0));

    assert_eq!(fator_de_rivalidade_do_par(&um, &outro), 0.0);
}

/// A margem só existe contra rival, e cresce com a intensidade.
#[test]
fn margem_de_ataque_so_existe_contra_rival() {
    assert_eq!(margem_de_ataque_por_rivalidade(0.0), 0.0);
    assert!(margem_de_ataque_por_rivalidade(1.0) < 0.0);
    assert!(margem_de_ataque_por_rivalidade(1.0) < margem_de_ataque_por_rivalidade(0.5));
}
