//! A tabela no rádio.
//!
//! Dois modos de falha guiam estes casos, e nenhum dos dois aparece como erro:
//!
//! 1. **Falar sem saber.** Uma temporada sem pontos, um jogador fora da tabela ou um save
//!    que não abriu produzem, se ninguém cuidar, "você está em zero no campeonato" — dito
//!    com a mesma convicção de um fato.
//! 2. **Falar demais.** O apêndice entra numa resposta que era sobre outra coisa. Se ele
//!    recitar a classificação a cada pergunta de posição, o engenheiro vira locutor.

use crate::engenheiro::campeonato::{apendice, catalogo, linhas, pecas, Contexto};
use crate::engenheiro::responder;
use crate::engenheiro::Intencao;

use super::estado_base;

/// Um contexto com posição e as duas margens iguais — cobre líder e perseguidor.
fn ctx(posicao: i32, margem: Option<f64>) -> Contexto {
    Contexto {
        posicao,
        para_o_proximo: margem,
        folga: margem,
        projecao: None,
    }
}

#[test]
fn sem_tabela_o_radio_nao_menciona_campeonato() {
    // Temporada recém-começada: ninguém pontuou, e o carregador devolve o vazio. Um
    // engenheiro não anuncia que a classificação está em branco.
    let vazio = Contexto::default();
    assert!(!vazio.conhecido());
    assert_eq!(pecas(&vazio), None);
    assert!(apendice(&vazio).is_empty());
    assert!(linhas(&vazio).is_empty());
}

#[test]
fn o_lider_fala_de_folga_e_o_resto_de_diferenca() {
    // A assimetria é a do próprio esporte: quem lidera defende, quem persegue caça. Trocar
    // as duas produziria "você está a doze pontos do próximo" para o líder do campeonato.
    assert_eq!(
        pecas(&ctx(1, Some(12.0))),
        Some(vec!["camp_lidera".into(), "camp_folga_12".into()])
    );
    assert_eq!(
        pecas(&ctx(4, Some(12.0))),
        Some(vec!["camp_pos_4".into(), "camp_para_12".into()])
    );
}

#[test]
fn a_posicao_sai_mesmo_sem_margem_para_dizer() {
    // Único pontuador da temporada: lidera, e não há ninguém atrás de quem ter folga. A
    // posição continua sendo verdade e continua valendo a pena.
    assert_eq!(
        pecas(&Contexto {
            posicao: 1,
            para_o_proximo: None,
            folga: None,
            projecao: None,
        }),
        Some(vec!["camp_lidera".into()])
    );
}

#[test]
fn diferenca_grande_demais_some_em_vez_de_virar_peca_inexistente() {
    // Sessenta é o teto do acervo. Acima dele a diferença deixou de ser uma diferença — e,
    // o que importa mais, não existe `.wav` para ela. Dizer só a posição é a resposta
    // certa e a única possível.
    assert_eq!(pecas(&ctx(3, Some(61.0))), Some(vec!["camp_pos_3".into()]));
    assert_eq!(pecas(&ctx(3, Some(60.0))).unwrap().len(), 2);
}

#[test]
fn posicao_acima_do_teto_cai_no_modelo() {
    // Não há `camp_pos_41`. O `None` é o que manda a pergunta ao modelo, que tem as mesmas
    // informações em prosa — em vez de pedir um arquivo que nunca foi gerado.
    assert_eq!(pecas(&ctx(41, Some(3.0))), None);
    assert!(
        !linhas(&ctx(41, Some(3.0))).is_empty(),
        "o modelo fica sem nada"
    );
}

#[test]
fn o_apendice_so_cita_a_margem_quando_ela_cabe_numa_corrida() {
    // Vitória vale 25 pontos. Abaixo disso a diferença é assunto de hoje e entra sem ser
    // pedida; acima, ela é assunto da temporada e só sai se a pergunta for do campeonato.
    assert_eq!(
        apendice(&ctx(3, Some(25.0))),
        vec!["camp_pos_3".to_string(), "camp_para_25".to_string()]
    );
    assert_eq!(
        apendice(&ctx(3, Some(26.0))),
        vec!["camp_pos_3".to_string()]
    );
    // Mas a pergunta DIRETA continua dando o número, porque foi ele que se pediu.
    assert_eq!(pecas(&ctx(3, Some(26.0))).unwrap().len(), 2);
}

#[test]
fn o_apendice_entra_na_posicao_e_na_pergunta_aberta_e_em_mais_nada() {
    let mut e = estado_base();
    e.posicao = 5;
    e.total_carros = 24;
    let c = ctx(3, Some(10.0));

    let posicao = responder::renderizar(&e, Some(&c), Intencao::Posicao).unwrap();
    assert!(
        posicao.len() > 1 && posicao.last().unwrap().starts_with("camp_"),
        "a posição saiu sem o campeonato: {posicao:?}"
    );
    // E ele vem DEPOIS. A informação que muda o que o piloto faz agora é a da pista.
    assert!(!posicao[0].starts_with("camp_"));

    // Uma pergunta de pneu não é lugar de classificação.
    if let Some(pneu) = responder::renderizar(&e, Some(&c), Intencao::Pneu) {
        assert!(!pneu.iter().any(|p| p.starts_with("camp_")), "{pneu:?}");
    }
}

#[test]
fn sem_save_a_resposta_de_posicao_continua_inteira() {
    // A regra que sustenta o resto: a tabela é tempero. Um save que não abriu tem de custar
    // uma frase a menos, nunca a resposta toda.
    let mut e = estado_base();
    e.posicao = 5;
    e.total_carros = 24;
    let com = responder::renderizar(&e, Some(&Contexto::default()), Intencao::Posicao);
    let sem = responder::renderizar(&e, None, Intencao::Posicao);
    assert_eq!(com, sem);
    assert!(com.is_some());
}

#[test]
fn a_pergunta_do_campeonato_leva_a_tabela_ao_modelo_quando_nao_renderiza() {
    // O caminho de escape: posição fora do teto do acervo. O dossiê tem de conter a
    // informação, senão o modelo recebe uma pergunta sobre campeonato e nenhum fato de
    // campeonato — e responde sobre outra coisa, com a voz confiante de sempre.
    let e = estado_base();
    let d = responder::dossie(&e, Some(&ctx(41, Some(8.0))), Intencao::Campeonato);
    assert!(d.iter().any(|l| l.contains("41")), "{d:?}");
    assert!(d.iter().any(|l| l.contains("8")), "{d:?}");
}

#[test]
fn a_pergunta_aberta_leva_o_campeonato_junto_com_o_resto() {
    // Mesma lição do combustível: quem não delimitou o assunto está pedindo o retrato todo.
    let e = estado_base();
    let d = responder::dossie(&e, Some(&ctx(2, Some(4.0))), Intencao::Geral);
    assert!(d.iter().any(|l| l.starts_with("Campeonato:")), "{d:?}");
    assert!(d.len() > 3, "o retrato veio só com o campeonato: {d:?}");
}

#[test]
fn o_catalogo_concorda_com_o_que_as_pecas_pedem() {
    // O acoplamento que emudece o rádio sem erro: a chave montada aqui e a chave gravada
    // pelo gerador são a mesma string, e nada além deste teste garante isso.
    let chaves: std::collections::HashSet<String> =
        catalogo().into_iter().map(|(c, _)| c).collect();
    for posicao in 1..=40 {
        for margem in [0.4, 1.0, 1.4, 25.0, 59.6, 60.0] {
            for p in pecas(&ctx(posicao, Some(margem))).unwrap_or_default() {
                assert!(chaves.contains(&p), "peça '{p}' fora do catálogo");
            }
        }
    }
}

#[test]
fn um_ponto_e_singular() {
    // Num texto seria deselegante; numa GRAVAÇÃO é permanente. "Você está a um pontos do
    // próximo" ficaria no pacote até alguém regerar as 160 peças.
    let c = catalogo();
    let um = |chave: &str| {
        c.iter()
            .find(|(k, _)| k == chave)
            .map(|(_, t)| t.clone())
            .unwrap()
    };
    assert!(
        um("camp_para_1").contains("um ponto d"),
        "{}",
        um("camp_para_1")
    );
    assert!(
        um("camp_para_2").contains("dois pontos"),
        "{}",
        um("camp_para_2")
    );
    assert!(
        um("camp_folga_1").contains("um ponto de"),
        "{}",
        um("camp_folga_1")
    );
}

// ─── A conta ─────────────────────────────────────────────────────────────────
//
// Aqui mora o modo de falha mais caro do módulo. Uma projeção errada não parece errada: sai
// com a mesma voz, na mesma frase, e o piloto muda a corrida por causa dela. Os casos abaixo
// são quase todos sobre a projeção se RECUSAR a sair.

use crate::engenheiro::campeonato::projetar;
use std::collections::HashMap;

/// Tabela da temporada: `(piloto, pontos)`.
fn tabela(pares: &[(&str, f64)]) -> Vec<(String, f64)> {
    pares
        .iter()
        .map(|(id, p)| ((*id).to_string(), *p))
        .collect()
}

/// Mapa número do carro → piloto, como o `iracing_numbers/<carreira>.json`.
fn numeros(pares: &[(i64, &str)]) -> HashMap<i64, String> {
    pares
        .iter()
        .map(|(n, id)| (*n, (*id).to_string()))
        .collect()
}

#[test]
fn a_conta_soma_os_pontos_da_corrida_e_reordena() {
    // Bianchi tem 40 e lidera; o jogador tem 30. Hoje o jogador vence (25) e Bianchi é
    // terceiro (15): 55 contra 55 — empate, decidido pelo id, e o jogador perde o desempate
    // por "bianchi" < "jogador". A conta tem de refletir isso em vez de arredondar a favor.
    let t = tabela(&[("bianchi", 40.0), ("jogador", 30.0), ("cooper", 10.0)]);
    let ordem = [(1, 7), (2, 9), (3, 3)];
    let n = numeros(&[(9, "cooper"), (3, "bianchi")]);
    assert_eq!(projetar(&t, &ordem, &n, "jogador", 1, false), Some(2));

    // Agora Bianchi cai para fora dos pontos: o jogador assume de verdade.
    let ordem = [(1, 7), (2, 9), (11, 3)];
    assert_eq!(projetar(&t, &ordem, &n, "jogador", 1, false), Some(1));
}

#[test]
fn um_carro_nao_mapeado_na_zona_de_pontos_MATA_a_projecao() {
    // O caso que justifica o tudo-ou-nada. O carro 3 pontua e não casa com piloto nenhum:
    // pode ser um rival do campeonato levando 15 pontos que a conta não viu. Projetar
    // assim mesmo daria uma posição otimista dita com a convicção de um fato.
    let t = tabela(&[("bianchi", 40.0), ("jogador", 30.0)]);
    let ordem = [(1, 7), (2, 9), (3, 3)];
    let n = numeros(&[(9, "bianchi")]); // o 3 ficou de fora
    assert_eq!(projetar(&t, &ordem, &n, "jogador", 1, false), None);
}

#[test]
fn carro_nao_mapeado_FORA_da_zona_de_pontos_nao_atrapalha() {
    // Décimo primeiro para baixo não move a tabela, então não há o que resolver. Exigir o
    // mapa do grid inteiro faria a projeção sumir por causa de um retardatário.
    let t = tabela(&[("bianchi", 40.0), ("jogador", 30.0)]);
    let ordem = [(1, 7), (2, 9), (11, 3), (24, 5)];
    let n = numeros(&[(9, "bianchi")]);
    // Jogador 30+25 = 55; Bianchi 40+18 = 58. O que importa é a conta ter SAÍDO: os dois
    // carros fora da zona de pontos não tinham por que resolver, e exigir o mapa do grid
    // inteiro faria a projeção sumir por causa de um retardatário.
    assert_eq!(projetar(&t, &ordem, &n, "jogador", 1, false), Some(2));
}

#[test]
fn o_jogador_resolve_pela_POSICAO_e_nao_pelo_mapa() {
    // Ele não está no roster de IA exportado — o mapa não tem o número dele. Quem diz onde
    // ele está é a telemetria, e é assim que a conta o encontra.
    let t = tabela(&[("jogador", 10.0), ("bianchi", 12.0)]);
    let ordem = [(1, 99), (2, 9)];
    let n = numeros(&[(9, "bianchi")]);
    assert_eq!(projetar(&t, &ordem, &n, "jogador", 1, false), Some(1));
}

#[test]
fn sem_ordem_nao_ha_projecao() {
    // Fora de corrida a ordem chega vazia. É o que impede "terminando assim você cai para
    // nono" a partir de uma ordem de treino livre.
    let t = tabela(&[("jogador", 10.0)]);
    assert_eq!(projetar(&t, &[], &numeros(&[]), "jogador", 1, false), None);
    let ordem = [(1, 99)];
    assert_eq!(
        projetar(&t, &ordem, &numeros(&[]), "jogador", 0, false),
        None
    );
}

#[test]
fn quem_ainda_nao_pontuou_entra_na_conta() {
    // O estreante que sobe ao pódio não está na tabela da temporada. Sem entrar com zero,
    // ele sumiria da projeção — e a posição do jogador sairia melhor do que vai ser.
    let t = tabela(&[("jogador", 20.0)]);
    let ordem = [(1, 9), (2, 99)];
    let n = numeros(&[(9, "estreante")]);
    // Estreante: 0 + 25 = 25. Jogador: 20 + 18 = 38. O jogador ainda lidera.
    assert_eq!(projetar(&t, &ordem, &n, "jogador", 2, false), Some(1));
    // Mas com o jogador partindo de 5 pontos, o estreante passa por cima.
    let t = tabela(&[("jogador", 5.0)]);
    assert_eq!(projetar(&t, &ordem, &n, "jogador", 2, false), Some(2));
}

#[test]
fn endurance_usa_a_outra_tabela_de_pontos() {
    // 35 pela vitória em vez de 25. Usar a tabela errada não derruba nada — só dá um
    // número errado, que é pior.
    let t = tabela(&[("jogador", 0.0), ("bianchi", 30.0)]);
    let ordem = [(1, 99), (2, 9)];
    let n = numeros(&[(9, "bianchi")]);
    // Padrão: jogador 25, Bianchi 30 + 18 = 48. Endurance: jogador 35, Bianchi 30 + 28 = 58.
    assert_eq!(projetar(&t, &ordem, &n, "jogador", 1, false), Some(2));
    assert_eq!(projetar(&t, &ordem, &n, "jogador", 1, true), Some(2));
    // Com Bianchi fora dos pontos a diferença entre as tabelas aparece no jogador.
    let ordem = [(1, 99), (12, 9)];
    assert_eq!(projetar(&t, &ordem, &n, "jogador", 1, true), Some(1));
}

#[test]
fn a_projecao_que_MUDA_a_tabela_troca_de_lugar_com_a_margem() {
    // Duas frases, sempre. Quando o resultado de hoje move a tabela, é ele que vale a
    // segunda — a diferença de pontos virou detalhe de uma situação em movimento.
    let mut c = ctx(4, Some(12.0));
    c.projecao = Some(2);
    assert_eq!(
        pecas(&c),
        Some(vec!["camp_pos_4".into(), "camp_proj_sobe_2".into()])
    );
    // Já a projeção que não muda nada cede o lugar de volta para a margem.
    c.projecao = Some(4);
    assert_eq!(
        pecas(&c),
        Some(vec!["camp_pos_4".into(), "camp_para_12".into()])
    );
}

#[test]
fn assumir_a_lideranca_tem_nome_proprio() {
    // "Sobe para primeiro" é a forma burocrática da única troca que o automobilismo nomeia.
    let mut c = ctx(3, Some(8.0));
    c.projecao = Some(1);
    assert_eq!(pecas(&c).unwrap()[1], "camp_proj_lidera");
    let mut c = ctx(1, Some(8.0));
    c.projecao = Some(1);
    // Líder que segue líder não é notícia: volta a falar da folga.
    assert_eq!(pecas(&c).unwrap()[1], "camp_folga_8");
}

#[test]
fn o_apendice_so_carrega_a_projecao_que_e_noticia() {
    // Numa resposta sobre outra coisa, "terminando assim você segue em terceiro" é
    // informação de quem perguntou — e aqui ninguém perguntou.
    let mut c = ctx(3, Some(40.0)); // margem fora do alcance: não entraria de todo jeito
    c.projecao = Some(3);
    assert_eq!(apendice(&c), vec!["camp_pos_3".to_string()]);
    c.projecao = Some(5);
    assert_eq!(
        apendice(&c),
        vec!["camp_pos_3".to_string(), "camp_proj_cai_5".to_string()]
    );
}

#[test]
fn a_projecao_vai_ao_modelo_como_conta_FECHADA() {
    // Mandar a tabela e a ordem do grid para o modelo somar seria pedir aritmética a quem
    // redige — e ele erraria em prosa perfeita.
    let mut c = ctx(3, Some(8.0));
    c.projecao = Some(2);
    let l = linhas(&c);
    assert!(
        l.iter().any(|x| x.contains("terminando esta corrida")),
        "{l:?}"
    );
    assert!(l.iter().any(|x| x.contains("2º")), "{l:?}");
}

#[test]
fn meio_ponto_arredonda_em_vez_de_sumir() {
    // O banco guarda `f64` e há categoria com meio ponto. Truncar para `as i32` faria
    // 0,6 virar zero — e zero não tem peça, então a margem sumiria da fala inteira.
    assert_eq!(
        pecas(&ctx(3, Some(0.6))),
        Some(vec!["camp_pos_3".into(), "camp_para_1".into()])
    );
    // Já meio ponto para baixo arredonda para zero, que não é margem nenhuma: some.
    assert_eq!(pecas(&ctx(3, Some(0.4))), Some(vec!["camp_pos_3".into()]));
}
