use super::*;
use std::collections::HashSet;

#[test]
fn o_tempo_e_dito_como_o_radio_diz() {
    assert_eq!(tempo_falado(453).unwrap(), "quarenta e cinco e três");
    assert_eq!(tempo_falado(924).unwrap(), "um trinta e dois e quatro");
    // Segundo de um dígito leva "zero" na frente — a convenção do rádio. Sem ele, "um nove e
    // zero" fica a um passo de soar como 1,9 s.
    assert_eq!(tempo_falado(690).unwrap(), "um zero nove e zero");
    assert_eq!(tempo_falado(600).unwrap(), "um zero zero e zero");
    assert_eq!(tempo_falado(1253).unwrap(), "dois zero cinco e três");
    assert_eq!(tempo_falado(2400).unwrap(), "quatro zero zero e zero");
    assert_eq!(tempo_falado(300).unwrap(), "trinta e zero");
}

#[test]
fn fora_da_faixa_devolve_none_em_vez_de_inventar() {
    // O `None` é o mecanismo, não um detalhe: ele sobe até o renderizador e manda a pergunta
    // para o modelo. Um `Some` aqui pediria um `.wav` que ninguém gravou, e o sintoma na pista
    // seria silêncio — indistinguível de o jogador não ter perguntado.
    assert!(tempo_falado(MIN_DECIMOS - 1).is_none());
    assert!(tempo_falado(MAX_DECIMOS + 1).is_none());
    assert!(chave_tempo(299).is_none());
    assert!(chave_tempo(2401).is_none());
    // Nordschleife: 11:43. Fora da faixa de propósito — 2,6% do calendário.
    assert!(chave_tempo(7030).is_none());
    assert!(chave_tempo(MIN_DECIMOS).is_some());
    assert!(chave_tempo(MAX_DECIMOS).is_some());
}

#[test]
fn toda_a_faixa_tem_redacao_e_nenhuma_se_repete() {
    // Um buraco no meio da faixa é o defeito silencioso desta família: o tempo cai numa chave
    // que não existe e o engenheiro emudece só naquele valor.
    let mut textos = HashSet::new();
    for d in MIN_DECIMOS..=MAX_DECIMOS {
        let t = tempo_falado(d).unwrap_or_else(|| panic!("sem redação para {d}"));
        assert!(!t.is_empty());
        assert!(textos.insert(t.clone()), "texto repetido em {d}: {t:?}");
    }
    assert_eq!(textos.len(), (MAX_DECIMOS - MIN_DECIMOS + 1) as usize);
}

#[test]
fn nenhum_tempo_tem_pontuacao_no_meio() {
    // MEDIDO no acervo da quebra: texto com vírgula ou travessão sai do modelo com ~0,35 s de
    // silêncio DENTRO da gravação, contra 0,01 s da oração corrida. Um tempo de volta partido
    // no meio soa como rádio falhando.
    for d in MIN_DECIMOS..=MAX_DECIMOS {
        let t = tempo_falado(d).unwrap();
        assert!(
            !t.contains(',') && !t.contains('—') && !t.contains(';'),
            "{d}: {t:?}"
        );
    }
}

#[test]
fn o_arredondamento_e_para_o_decimo_mais_proximo() {
    // Como o cronômetro mostraria. Truncar faria o rádio dizer sempre um décimo a menos que a
    // tela, e o jogador confia na tela.
    assert_eq!(decimos_de(92.37), 924);
    assert_eq!(decimos_de(92.34), 923);
    assert_eq!(decimos_de(45.0), 450);
}

#[test]
fn a_aproximacao_concorda_em_numero() {
    // "Faltam um décimo" é o erro que ninguém nota escrevendo e todo mundo ouve.
    let (_, um) = aproximacao_frase(1).unwrap();
    assert_eq!(um, "Falta um décimo para a melhor volta.");
    let (_, tres) = aproximacao_frase(3).unwrap();
    assert_eq!(tres, "Faltam três décimos para a melhor volta.");
}

#[test]
fn a_aproximacao_para_abaixo_de_um_segundo() {
    // Acima de um segundo, "está chegando perto" é otimismo e não informação: um segundo numa
    // volta de um e trinta é mais de 1% do tempo.
    assert!(aproximacao_frase(0).is_none());
    assert!(aproximacao_frase(DECIMOS_DE_APROXIMACAO).is_some());
    assert!(aproximacao_frase(DECIMOS_DE_APROXIMACAO + 1).is_none());
}

#[test]
fn o_catalogo_tem_o_tamanho_medido_e_nenhuma_chave_repetida() {
    let v = familia_tempo_volta();
    // 2.101 tempos + 3 aberturas + 9 de aproximação + 3 de "tomamos".
    assert_eq!(v.len(), 2101 + 2 + 9 + 3);
    let chaves: HashSet<&String> = v.iter().map(|(k, _)| k).collect();
    assert_eq!(chaves.len(), v.len(), "chave repetida");
    let textos: HashSet<&String> = v.iter().map(|(_, t)| t).collect();
    assert_eq!(
        textos.len(),
        v.len(),
        "texto repetido — duas tomadas da mesma frase"
    );
}

#[test]
fn o_catalogo_e_estavel_entre_chamadas() {
    assert_eq!(familia_tempo_volta(), familia_tempo_volta());
}

#[test]
fn a_faixa_gravada_cobre_o_calendario_medido() {
    // Os números vêm de `simulation/profile/lap_times.rs`, 618 entradas: mínimo 30,0 s, máximo
    // 703,0 s. O piso tem que alcançar o mínimo; o teto NÃO alcança o máximo de propósito, e é
    // por isso que ele está escrito aqui — para a decisão ser revisável em vez de esquecida.
    assert!(
        MIN_DECIMOS <= 300,
        "o piso deixou de cobrir a volta mais rápida do jogo"
    );
    assert_eq!(
        MAX_DECIMOS, 2400,
        "o teto mudou — reconfira quantos % do calendário ficam fora"
    );
}
