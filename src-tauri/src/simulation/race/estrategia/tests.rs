use super::*;

// ─────────────────────── Bandeira amarela ───────────────────────

fn incidente(tipo: IncidentType, sev: IncidentSeverity, dnf: bool) -> IncidentResult {
    IncidentResult {
        pilot_id: "P1".to_string(),
        incident_type: tipo,
        severity: sev,
        segment: "MID".to_string(),
        positions_lost: 0,
        is_dnf: dnf,
        description: "x".to_string(),
        linked_pilot_id: None,
        is_two_car_incident: false,
        injury_risk_multiplier: 1.0,
        narrative_importance_hint: 1,
        catalog_id: None,
        damage_origin_segment: None,
    }
}

#[test]
fn batida_forte_traz_amarela_e_pane_nao() {
    assert!(traz_bandeira_amarela(&incidente(
        IncidentType::Collision,
        IncidentSeverity::Critical,
        false
    )));
    // Batida média só neutraliza se tirou o carro.
    assert!(traz_bandeira_amarela(&incidente(
        IncidentType::Collision,
        IncidentSeverity::Major,
        true
    )));
    assert!(!traz_bandeira_amarela(&incidente(
        IncidentType::Collision,
        IncidentSeverity::Major,
        false
    )));
    // O carro que recolhe pro box não neutraliza.
    assert!(!traz_bandeira_amarela(&incidente(
        IncidentType::Mechanical,
        IncidentSeverity::Critical,
        true
    )));
}

// ─────────────────────── Compressão do pelotão ───────────────────────

#[test]
fn safety_car_zera_os_gaps() {
    // 30 s de vantagem viram uma fração — é o que transforma liderança em briga.
    let antes = 30_000.0;
    let depois = atraso_sob_safety_car(antes, 5);
    assert!(depois < antes * 0.5, "{depois} contra {antes}");
}

#[test]
fn safety_car_nao_reordena_o_pelotao() {
    // As duas parcelas do `max` crescem com a posição, então quem estava na frente continua
    // na frente. Sem isto, o SC viraria sorteio puro em vez de embaralhador.
    let gaps: Vec<f64> = (0..12).map(|i| (i as f64).powf(1.6) * 900.0).collect();
    let atrasos: Vec<f64> = gaps
        .iter()
        .enumerate()
        .map(|(i, g)| atraso_sob_safety_car(*g, i as i32 + 1))
        .collect();
    assert!(
        atrasos.windows(2).all(|p| p[1] >= p[0]),
        "a ordem mudou: {atrasos:?}"
    );
    assert_eq!(atrasos[0], 0.0, "o líder é a âncora");
}

#[test]
fn safety_car_deixa_o_pelotao_em_fila_e_nao_empilhado() {
    // Mesmo com gap zero de entrada, cada posição guarda um espaçamento mínimo.
    let a = atraso_sob_safety_car(0.0, 1);
    let b = atraso_sob_safety_car(0.0, 2);
    let c = atraso_sob_safety_car(0.0, 3);
    assert_eq!(a, 0.0);
    assert!(b > a && c > b, "{a} {b} {c}");
    assert_eq!(b, GAP_MINIMO_SOB_SAFETY_CAR_MS);
}

// ─────────────────────── A chamada da equipe ───────────────────────

#[test]
fn prova_curta_nao_tem_parada() {
    // O caso da monomarca de entrada: 20 minutos não têm janela de parada, e o alvo do
    // harness diz "rookie sem parada, 1–2 estratégias distintas no grid".
    let p = planejar_paradas("D1", "T1", 523, 13, 20, 70.0);
    assert!(p.voltas_planejadas.is_empty());
    assert_eq!(p.estrategia_id, "sem-parada");
}

#[test]
fn prova_longa_tem_uma_parada_na_janela() {
    let total = 30;
    let p = planejar_paradas("D1", "T1", 523, total, 45, 70.0);
    assert_eq!(p.voltas_planejadas.len(), 1);
    let volta = p.voltas_planejadas[0];
    let (inicio, fim) = JANELA_DE_PARADA;
    assert!(
        volta as f64 >= total as f64 * inicio - 1.0 && volta as f64 <= total as f64 * fim + 1.0,
        "volta {volta} fora da janela de {total}"
    );
}

#[test]
fn a_chamada_e_deterministica() {
    // Personagem da equipe, não sorte do dia: mesma equipe, mesma pista, mesma chamada.
    let a = planejar_paradas("D1", "T1", 523, 30, 45, 70.0);
    let b = planejar_paradas("D1", "T1", 523, 30, 45, 70.0);
    assert_eq!(a.voltas_planejadas, b.voltas_planejadas);
    assert_eq!(a.estrategia_id, b.estrategia_id);
}

#[test]
fn os_dois_lados_da_garagem_costumam_dividir_a_estrategia() {
    // O `driver_id` entra na semente, então os dois carros da mesma equipe recebem chamadas
    // independentes. Cair na MESMA volta é legítimo e acontece (a volta é inteira, e duas
    // chamadas próximas arredondam igual) — o que importa é que dividir seja comum, porque é
    // isso que permite ao grid ter mais de uma estratégia viva. O alvo pede 2–4 distintas.
    let mut divididas = 0;
    let equipes = 40;
    for t in 0..equipes {
        let team = format!("T{t}");
        let a = planejar_paradas("D1", &team, 523, 30, 45, 40.0);
        let b = planejar_paradas("D2", &team, 523, 30, 45, 40.0);
        if a.voltas_planejadas != b.voltas_planejadas {
            divididas += 1;
        }
    }
    assert!(
        divididas * 2 > equipes,
        "só {divididas}/{equipes} equipes dividiram a estratégia entre os dois carros"
    );
}

#[test]
fn equipe_boa_acerta_o_meio_da_janela_e_equipe_ruim_erra() {
    // A chamada ruim é de propósito: é o personagem da equipe dentro da corrida.
    let total = 30;
    let (inicio, fim) = JANELA_DE_PARADA;
    let meio = total as f64 * (inicio + fim) / 2.0;

    let erro_medio = |qualidade: f64| {
        let mut soma = 0.0;
        let n = 60;
        for i in 0..n {
            let p = planejar_paradas(&format!("D{i}"), "T1", 523, total, 45, qualidade);
            soma += (p.voltas_planejadas[0] as f64 - meio).abs();
        }
        soma / n as f64
    };

    let boa = erro_medio(95.0);
    let ruim = erro_medio(15.0);
    assert!(
        ruim > boa * 1.5,
        "equipe ruim deveria errar mais a chamada: ruim={ruim:.2} boa={boa:.2}"
    );
}

#[test]
fn a_chamada_perfeita_cai_no_meio_da_janela() {
    let total = 30;
    let (inicio, fim) = JANELA_DE_PARADA;
    let meio = (total as f64 * (inicio + fim) / 2.0).round() as u32;
    let p = planejar_paradas("D1", "T1", 523, total, 45, 100.0);
    assert_eq!(p.voltas_planejadas[0], meio);
    assert_eq!(p.estrategia_id, "1-parada-ideal");
}

#[test]
fn os_rotulos_cobrem_os_tres_tercos_da_janela() {
    // O harness conta estratégias distintas pelo rótulo, então os três têm que aparecer.
    let mut vistos = std::collections::HashSet::new();
    for i in 0..200 {
        let p = planejar_paradas(&format!("D{i}"), &format!("T{}", i % 7), 523, 30, 45, 10.0);
        vistos.insert(p.estrategia_id);
    }
    assert!(
        vistos.len() >= 3,
        "esperava os três terços da janela, vi {vistos:?}"
    );
}

#[test]
fn parada_sob_safety_car_e_muito_mais_barata() {
    // O coração do "regala quem ia parar mesmo".
    let cheia = CUSTO_DE_PARADA_MS;
    let sob_sc = CUSTO_DE_PARADA_MS * FRACAO_DO_CUSTO_SOB_SAFETY_CAR;
    assert!(sob_sc < cheia * 0.6, "{sob_sc} contra {cheia}");
    assert!(sob_sc > 0.0, "parar sob SC não pode ser de graça");
}
