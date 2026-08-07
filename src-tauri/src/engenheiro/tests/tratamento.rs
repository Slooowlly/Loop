//! Como o engenheiro te chama.
//!
//! Os casos abaixo cercam duas coisas que erram de formas opostas. A **fase** erra devagar:
//! chamar de novato quem já venceu não quebra nada, só desmonta em silêncio a única fala do
//! rádio que era sobre você e não sobre a corrida. E o **vocativo na frente** erra de uma vez:
//! entrar no lugar errado da montagem produz "Seu rival, novato, Cooper está a um e dois",
//! que é uma frase que nenhum engenheiro diria.

use crate::engenheiro::quebra::Vinculo;
use crate::engenheiro::responder::{self, Extras};
use crate::engenheiro::tratamento::{
    self, Tratamento, A_CADA, CORRIDAS_DE_NOVATO, CORRIDAS_PARA_GRAVAR,
};
use crate::engenheiro::vizinhanca;
use crate::engenheiro::Intencao;

use super::{estado_base, vizinho};

const NOME: &str = "Magno Silva";

/// O tratamento pelo nome COM a gravação já no disco — o estado normal de uma carreira
/// rodada.
fn nomeado() -> Tratamento {
    Tratamento::Nome {
        falado: "Magno".into(),
        chave: Some("voc_magno".into()),
    }
}

// ─── A fase ──────────────────────────────────────────────────────────────────

#[test]
fn quem_acabou_de_comecar_e_novato() {
    assert_eq!(
        tratamento::decidir(0, 0, NOME, false),
        Some(Tratamento::Novato)
    );
    assert_eq!(
        tratamento::decidir(CORRIDAS_DE_NOVATO - 1, 0, NOME, true),
        Some(Tratamento::Novato),
        "a gravação existir não antecipa a fase — quem a vira é o resultado"
    );
}

#[test]
fn a_primeira_vitoria_encerra_o_novato_na_hora() {
    // A regra do produto inteira em uma linha: o apelido não expira por calendário, ele é
    // TROCADO por um resultado. Quem vence na segunda corrida deixa de ser novato na segunda.
    assert_eq!(tratamento::decidir(2, 1, NOME, true), Some(nomeado()));
}

#[test]
fn quem_nao_vence_deixa_de_ser_novato_pela_contagem() {
    // A rede de segurança. Sem ela, um piloto azarado seria chamado de novato por três
    // temporadas — e aí a palavra deixa de ser uma fase e vira um julgamento.
    assert_eq!(
        tratamento::decidir(CORRIDAS_DE_NOVATO, 0, NOME, true),
        Some(nomeado())
    );
}

#[test]
fn o_vocativo_e_o_PRIMEIRO_NOME() {
    // O engenheiro nomeia o grid pelo sobrenome ("Seu rival, Cooper,") e o JOGADOR pelo
    // primeiro nome. A assimetria é o ponto: os outros são adversários, e ele é a pessoa com
    // quem você fala a corrida inteira.
    let Some(Tratamento::Nome { falado, .. }) = tratamento::decidir(20, 5, NOME, false) else {
        panic!("devia ser nome");
    };
    assert_eq!(falado, "Magno");
    // Nome de três partes ainda dá o primeiro, e o espaço à toa não vira nome vazio.
    for (digitado, esperado) in [
        ("Carlos Magno Silva", "Carlos"),
        ("  Ayrton  ", "Ayrton"),
        ("Ayrton", "Ayrton"),
    ] {
        let Some(Tratamento::Nome { falado, .. }) = tratamento::decidir(20, 5, digitado, false)
        else {
            panic!("devia ser nome: {digitado:?}");
        };
        assert_eq!(falado, esperado, "{digitado:?}");
    }
}

#[test]
fn sem_nome_ele_nao_te_chama_de_nada() {
    // E em particular NÃO volta a "novato": rebaixar quem já venceu por causa de um campo em
    // branco seria trocar um silêncio por um erro.
    assert_eq!(tratamento::decidir(20, 5, "", true), None);
    assert_eq!(tratamento::decidir(20, 5, "   ", true), None);
}

// ─── A gravação própria ──────────────────────────────────────────────────────

#[test]
fn sem_o_arquivo_o_caminho_gravado_fica_sem_vocativo() {
    // A espera, e não um buraco: até o MP3 chegar ao save, a fala sai sem o nome na frente.
    // O modelo continua chamando pelo nome, porque lá o texto é sintetizado na hora.
    let t = tratamento::decidir(20, 5, NOME, false).expect("nome");
    assert_eq!(tratamento::peca(&t, 0), None);
    assert_eq!(tratamento::como_chamar(&t), "Magno");
}

#[test]
fn com_o_arquivo_a_peca_propria_entra_na_fala() {
    assert_eq!(tratamento::peca(&nomeado(), 0).as_deref(), Some("voc_magno"));
    // E o rodízio não a multiplica: a peça própria é uma só.
    for rodizio in 0..5 {
        assert_eq!(
            tratamento::peca(&nomeado(), rodizio).as_deref(),
            Some("voc_magno")
        );
    }
}

#[test]
fn a_chave_passa_pela_MESMA_normalizacao_do_acervo() {
    // O acento é o caso que importa: sem a dobra, "Gonçalo" viraria `voc_gon_alo` — chave
    // válida para um arquivo que nunca seria escrito com esse nome.
    assert_eq!(
        tratamento::chave_do_nome("Gonçalo Ribeiro").as_deref(),
        Some("voc_goncalo")
    );
    assert_eq!(
        tratamento::chave_do_nome("Magno Silva").as_deref(),
        Some("voc_magno")
    );
    // Nome que não sobra nada depois da normalização não tem arquivo a pedir.
    assert_eq!(tratamento::chave_do_nome(""), None);
    assert_eq!(tratamento::chave_do_nome("!!! ???"), None);
}

#[test]
fn o_texto_sintetizado_abre_em_MAIUSCULA_e_sem_virgula() {
    // A vírgula é posta pelo servidor, e de propósito: a prosódia de vocativo é propriedade
    // da peça, e deixá-la ao cliente seria o começo de duas peças com a mesma chave.
    assert_eq!(
        tratamento::texto_da_peca("magno silva").as_deref(),
        Some("Magno")
    );
    assert!(!tratamento::texto_da_peca(NOME).unwrap().contains(','));
    assert_eq!(tratamento::texto_da_peca("  "), None);
}

#[test]
fn a_gravacao_so_e_pedida_depois_de_tres_corridas() {
    // Uma carreira criada para ser olhada e abandonada não paga uma síntese nem um arquivo no
    // disco de quem joga.
    for corridas in 0..CORRIDAS_PARA_GRAVAR {
        assert!(!tratamento::pode_gravar(corridas, 0), "{corridas}");
    }
    assert!(tratamento::pode_gravar(CORRIDAS_PARA_GRAVAR, 0));
    // A vitória antecipa: ela vira a fase, e esperar a terceira corrida deixaria o piloto uma
    // corrida sem vocativo por causa de um limiar pensado para outra coisa.
    assert!(tratamento::pode_gravar(1, 1));
}

#[test]
fn o_rodizio_percorre_as_tres_redacoes_e_volta() {
    let ditas: Vec<String> = (0..4)
        .map(|i| tratamento::peca(&Tratamento::Novato, i).expect("novato tem peça"))
        .collect();
    assert_eq!(ditas[0], "voc_novato");
    assert_eq!(ditas[3], ditas[0], "o rodízio volta ao começo");
    assert_eq!(
        ditas[..3].iter().collect::<std::collections::HashSet<_>>().len(),
        3,
        "as três primeiras são diferentes: {ditas:?}"
    );
}

// ─── O lugar dele na fala ────────────────────────────────────────────────────

fn com_tratamento(t: Option<Tratamento>) -> Extras {
    Extras {
        tratamento: t,
        ..Extras::default()
    }
}

#[test]
fn o_vocativo_entra_na_FRENTE_de_tudo() {
    let mut e = estado_base();
    e.posicao = 5;
    let pecas = responder::renderizar_com(&e, &com_tratamento(Some(Tratamento::Novato)), Intencao::Posicao)
        .expect("a posição sempre renderiza");
    assert_eq!(pecas[0], "voc_novato");
    assert!(pecas.len() > 1, "o vocativo não é a resposta: {pecas:?}");
}

#[test]
fn o_vocativo_vem_antes_da_abertura_do_vizinho() {
    // O caso que erra alto. A resposta do vizinho nomeado já é montada — "Seu rival," +
    // "Cooper," + o fecho — e um vocativo enfiado no meio produziria uma frase que ninguém
    // diria. Ele abre a fala, ponto.
    let mut e = estado_base();
    e.frente = Some(vizinho("James Cooper", 1.2));
    let extras = Extras {
        tratamento: Some(Tratamento::Novato),
        vizinhanca: vizinhanca::Contexto {
            frente: Some(Vinculo::Rival),
            atras: None,
        },
        ..Extras::default()
    };
    let pecas =
        responder::renderizar_com(&e, &extras, Intencao::Frente).expect("vizinho nomeado renderiza");
    assert_eq!(pecas[0], "voc_novato");
    assert_eq!(pecas[1], "ab_rival");
}

#[test]
fn sem_tratamento_a_resposta_e_exatamente_a_de_sempre() {
    // A garantia de que o vocativo é aditivo: nas três respostas em cada quatro em que ele
    // não sai, nada mais muda.
    let mut e = estado_base();
    e.posicao = 5;
    for intencao in [Intencao::Posicao, Intencao::Frente, Intencao::Geral] {
        e.frente = Some(vizinho("James Cooper", 1.2));
        let com = responder::renderizar_com(&e, &com_tratamento(None), intencao);
        let sem = responder::renderizar_com(&e, &Extras::default(), intencao);
        assert_eq!(com, sem, "{intencao:?}");
    }
}

#[test]
fn o_NOME_SEM_gravacao_nao_deixa_rastro_no_caminho_gravado() {
    // Um `Tratamento::Nome` sem peça não pode virar uma fala com um buraco na frente — tem
    // que sair a resposta limpa, idêntica à de quem não tem tratamento nenhum.
    let mut e = estado_base();
    e.posicao = 5;
    let sem_arquivo = Tratamento::Nome {
        falado: "Magno".into(),
        chave: None,
    };
    assert_eq!(
        responder::renderizar_com(&e, &com_tratamento(Some(sem_arquivo)), Intencao::Posicao),
        responder::renderizar_com(&e, &Extras::default(), Intencao::Posicao)
    );
}

#[test]
fn a_peca_propria_abre_a_fala_como_o_novato_abre() {
    // O par do teste do vocativo de novato, agora com a peça que veio do save. A chave é
    // arbitrária — ela sai do nome do jogador —, e é justamente por isso que o renderizador
    // não pode ter nenhuma lista dela.
    let mut e = estado_base();
    e.posicao = 5;
    let pecas = responder::renderizar_com(&e, &com_tratamento(Some(nomeado())), Intencao::Posicao)
        .expect("a posição sempre renderiza");
    assert_eq!(pecas[0], "voc_magno");
    assert!(pecas.len() > 1, "{pecas:?}");
}

#[test]
fn a_cadencia_e_maior_que_um() {
    // Guard de sanidade sobre a constante, não sobre o código: com `A_CADA = 1` ele chamaria
    // seu nome em toda resposta, que é o defeito exato que a cadência existe para evitar — e
    // a mudança seria uma linha inocente num `const`.
    assert!(A_CADA > 1, "vocativo em toda resposta vira cacoete");
}
