//! O vizinho com NOME — e, quando ele é alguém, com o vínculo junto.
//!
//! O acervo já respondia "o carro da frente está a um e dois". Verdadeiro, e sobre um
//! carro. O grid do Loop não é feito de carros: são duzentos pilotos com nome, equipe,
//! histórico e uma relação com o jogador que o mundo simulado construiu sozinho. Dizer
//! "Seu rival, Cooper, está a um e dois na sua frente" é a mesma informação sobre uma
//! PESSOA — e o piloto ataca ou administra de outro jeito quando sabe quem está ali.
//!
//! ## Reaproveita a gramática da quebra, não a reinventa
//!
//! A fala de quebra já monta `[abertura] [nome,] [trecho]` com as pausas calibradas. Aqui
//! a montagem é a mesma, com as MESMAS peças de abertura (`ab_nemesis`, `ab_rival`,
//! `ab_companheiro`) e os MESMOS 355 sobrenomes. O que é novo é só o fecho — a frase do
//! gap sem sujeito, porque o sujeito passou a ser o nome:
//!
//! ```text
//!   ab_rival        "Seu rival,"
//!   nm_cooper       "Cooper,"
//!   viz_frente_gap_1_2  "está a um e dois na sua frente."
//! ```
//!
//! ## Quando ele NÃO usa o nome
//!
//! Quando não sabe. O sobrenome só existe em áudio se saiu de um pool de nomes — um piloto
//! de save antigo, ou um humano que entrou no grid, não tem gravação. Aí a resposta volta
//! a ser a anônima, que continua verdadeira. É o mesmo tudo-ou-nada do resto do acervo:
//! meia fala gravada emendada com o nome faltando seria pior que a fala antiga inteira.

use crate::iracing_sdk::race_monitor::Vizinho;

use super::nomes;
use super::quebra::Vinculo;

/// Prefixo das peças desta família.
pub const PREFIXO: &str = "viz_";

/// O vínculo de cada vizinho com o jogador, lido do save.
///
/// `Nenhum` em ambos é o normal: a maioria do grid é gente com quem o jogador não tem
/// história. O vínculo muda a ABERTURA da fala, não o conteúdo — o gap é o mesmo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contexto {
    pub frente: Option<Vinculo>,
    pub atras: Option<Vinculo>,
}

impl Contexto {
    fn de(&self, frente: bool) -> Option<Vinculo> {
        if frente {
            self.frente
        } else {
            self.atras
        }
    }
}

/// A abertura que corresponde ao vínculo. Só três dos sete viram fala aqui.
///
/// `Lider`, `PontosAFrente` e `PontosAtras` ficam de fora de propósito: eles descrevem a
/// TABELA, e a tabela já tem quem a diga (ver [`super::campeonato`]). Repeti-los aqui faria
/// a mesma informação sair duas vezes na mesma resposta.
fn abertura(v: Vinculo) -> Option<&'static str> {
    match v {
        Vinculo::Nemesis => Some("ab_nemesis"),
        Vinculo::Rival => Some("ab_rival"),
        Vinculo::Companheiro => Some("ab_companheiro"),
        _ => None,
    }
}

/// **A resposta com nome.** `None` quando não dá para nomear — e aí quem chama volta para a
/// fala anônima do acervo, que continua correta.
///
/// Os mesmos três casos de recusa da fala anônima valem aqui, e por isso ela é consultada
/// primeiro pelo chamador: sem vizinho é liderança ou último lugar, e uma volta à parte é
/// tráfego e não disputa.
pub fn pecas(v: &Vizinho, frente: bool, ctx: &Contexto) -> Option<Vec<String>> {
    if v.volta_a_parte {
        return None;
    }
    let chave_nome = nomes::chave_sobrenome(nomes::sobrenome_de(&v.nome))?;

    // O fecho primeiro: se ele não existe, não há fala, e nem vale montar o resto.
    let fecho = if v.no_box {
        "viz_no_box".to_string()
    } else {
        let lado = if frente { "frente" } else { "atras" };
        match super::fala::sufixo_gap(v.gap_s) {
            Some(sufixo) => format!("{PREFIXO}{lado}_{sufixo}"),
            None if (0.0..0.1).contains(&v.gap_s) => format!("{PREFIXO}{lado}_colado"),
            None => return None,
        }
    };

    let mut pecas = Vec::with_capacity(3);
    if let Some(ab) = ctx.de(frente).and_then(abertura) {
        pecas.push(ab.to_string());
    }
    pecas.push(chave_nome);
    pecas.push(fecho);
    Some(pecas)
}

/// O nome do vizinho em prosa, para o dossiê do modelo.
///
/// Vai como LINHA à parte em vez de reescrever a linha do gap: o dossiê é lido por um
/// modelo que redige, e dois fatos separados dão a ele a liberdade de juntá-los como a
/// frase pedir.
pub fn linha(v: &Vizinho, frente: bool, ctx: &Contexto) -> Option<String> {
    let sobrenome = nomes::sobrenome_de(&v.nome);
    if sobrenome.is_empty() {
        return None;
    }
    let lado = if frente {
        "à sua frente"
    } else {
        "atrás de você"
    };
    let vinculo = match ctx.de(frente) {
        Some(Vinculo::Nemesis) => " — é o seu MAIOR RIVAL",
        Some(Vinculo::Rival) => " — é um RIVAL seu",
        Some(Vinculo::Companheiro) => " — é seu COMPANHEIRO de equipe",
        _ => "",
    };
    Some(format!("Quem está {lado}: {sobrenome}{vinculo}"))
}

/// Toda peça desta família, com o texto a gravar.
///
/// São os fechos sem sujeito. A grade de gaps é a MESMA da família anônima, e por
/// construção: ela é gerada pela mesma [`super::fala::gap_partes`], então uma resolução
/// nova entra nas duas de uma vez ou em nenhuma.
pub fn familia_vizinho() -> Vec<(String, String)> {
    let mut v = Vec::new();
    for (sufixo, texto) in super::fala::grade_de_gaps() {
        v.push((
            format!("{PREFIXO}frente_{sufixo}"),
            format!("está a {texto} na sua frente."),
        ));
        v.push((
            format!("{PREFIXO}atras_{sufixo}"),
            format!("está a {texto} atrás de você."),
        ));
    }
    v.push((
        format!("{PREFIXO}frente_colado"),
        "está colado em você.".to_string(),
    ));
    v.push((
        format!("{PREFIXO}atras_colado"),
        "está colado atrás de você.".to_string(),
    ));
    // O box não precisa de lado: quem entrou no box saiu da disputa dos dois jeitos, e o
    // nome já disse de quem se trata.
    v.push((format!("{PREFIXO}no_box"), "entrou no box.".to_string()));
    v
}
