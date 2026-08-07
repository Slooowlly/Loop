//! Onde as fontes se encontram: a telemetria, o save e a memória da conversa.
//!
//! [`fala::renderizar`](super::fala::renderizar) e [`fatos::dossie`](super::fatos::dossie)
//! só conhecem `EstadoAgora` — telemetria pura, sem banco, testáveis sem nada aberto. É uma
//! propriedade que vale manter, então nem a tabela do campeonato nem os nomes dos vizinhos
//! entraram lá dentro: eles entram AQUI, por composição.
//!
//! Este módulo continua puro. Ele recebe os valores já lidos e decide o que sai; quem abre o
//! banco e quem guarda a memória é `commands::engenheiro`.
//!
//! ## A regra de composição
//!
//! - `Campeonato` — a resposta é a tabela, e só ela.
//! - `Frente` e `Atras` — o vizinho NOMEADO, com a anônima de reserva, mais a variação
//!   desde a última resposta sobre aquele mesmo carro.
//! - `Posicao` e `Geral` — a resposta de sempre, com o apêndice da tabela no fim.
//! - o resto — passa direto.
//!
//! O apêndice vai sempre no FIM, por um motivo de rádio: a informação que muda o que o
//! piloto faz agora vem primeiro. Ele perguntou onde está o carro da frente; que o
//! campeonato depende disso, ou que o gap encolheu desde a última vez, é o porquê — e o
//! porquê pode chegar meio segundo depois.

use crate::iracing_sdk::race_monitor::EstadoAgora;

use super::campeonato::{self, Contexto};
use super::memoria;
use super::tratamento::{self, Tratamento};
use super::vizinhanca;
use super::{fala, fatos, Intencao};

/// Tudo o que o engenheiro sabe **além da telemetria**.
///
/// Um struct em vez de parâmetros soltos porque eles crescem juntos: cada coisa nova que
/// ele passa a saber entra aqui, e as assinaturas não mudam de novo. Já são três fontes,
/// e nenhuma delas é o iRacing — a tabela e os vínculos vêm do save, e a memória vem da
/// própria conversa.
#[derive(Clone, Debug, Default)]
pub struct Extras {
    pub campeonato: Contexto,
    pub vizinhanca: vizinhanca::Contexto,
    /// Quanto o gap mudou desde a última resposta sobre o MESMO carro. Ver
    /// [`super::memoria`].
    pub memoria: Option<memoria::Delta>,
    /// Como ele te chama **nesta** resposta, ou `None` nas respostas em que ele não te chama
    /// de nada — que são a maioria, de propósito. Quem conta a cadência é
    /// `commands::engenheiro`; ver [`super::tratamento`].
    pub tratamento: Option<Tratamento>,
    /// Qual das redações do vocativo. Vive aqui e não lá dentro porque o rodízio é estado
    /// vivo — a mesma pergunta feita duas vezes tem que sair com palavras diferentes, e este
    /// módulo é puro.
    pub rodizio: usize,
}

/// **A resposta gravada**, telemetria mais tabela. `None` cai no modelo, como sempre.
pub fn renderizar(
    e: &EstadoAgora,
    camp: Option<&Contexto>,
    intencao: Intencao,
) -> Option<Vec<String>> {
    renderizar_com(
        e,
        &Extras {
            campeonato: camp.cloned().unwrap_or_default(),
            ..Extras::default()
        },
        intencao,
    )
}

/// A forma completa, com tudo o que ele sabe além da telemetria.
///
/// O VOCATIVO entra por fora, na frente do que quer que a resposta tenha virado. É a única
/// peça da fala que não depende do assunto — ele te chama pelo que você é, e não pelo que
/// você perguntou —, e por isso não tem lugar dentro da montagem de nenhuma intenção.
pub fn renderizar_com(e: &EstadoAgora, extras: &Extras, intencao: Intencao) -> Option<Vec<String>> {
    let mut pecas = resposta(e, extras, intencao)?;
    if let Some(voc) = extras
        .tratamento
        .as_ref()
        .and_then(|t| tratamento::peca(t, extras.rodizio))
    {
        pecas.insert(0, voc);
    }
    Some(pecas)
}

/// A resposta em si, sem o vocativo.
fn resposta(e: &EstadoAgora, extras: &Extras, intencao: Intencao) -> Option<Vec<String>> {
    let camp = Some(&extras.campeonato);
    if matches!(intencao, Intencao::Frente | Intencao::Atras) {
        let frente = intencao == Intencao::Frente;
        let viz = if frente { &e.frente } else { &e.atras };
        // O VIZINHO NOMEADO tenta primeiro e cede o lugar quando não sabe o nome. A ordem é
        // a regra do produto: "Seu rival, Cooper, está a um e dois na sua frente" é a mesma
        // informação de "o carro da frente está a um e dois", sobre uma pessoa em vez de um
        // carro. Só quando o sobrenome não existe em áudio — piloto de save antigo, humano
        // no grid — é que a fala anônima volta, ainda correta.
        let base = viz
            .as_ref()
            .and_then(|v| vizinhanca::pecas(v, frente, &extras.vizinhanca))
            .or_else(|| fala::renderizar(e, intencao));
        if let Some(mut pecas) = base {
            // A MEMÓRIA no fim, e só quando há resposta a que ela se referir. "São quatro
            // décimos a menos que da última vez" sozinha não é resposta: ela qualifica o
            // número que acabou de sair.
            if let Some(mem) = extras.memoria.and_then(memoria::peca) {
                pecas.push(mem);
            }
            return Some(pecas);
        }
        return None;
    }
    match intencao {
        // Sem save não há o que responder aqui, e o modelo também não teria fato nenhum —
        // mas quem decide isso é o dossiê, que devolve as linhas vazias e faz o
        // roteamento desistir. Aqui basta o `None`.
        Intencao::Campeonato => campeonato::pecas(camp?),
        Intencao::Posicao | Intencao::Geral => {
            let mut v = fala::renderizar(e, intencao)?;
            if let Some(c) = camp {
                v.extend(campeonato::apendice(c));
            }
            Some(v)
        }
        _ => fala::renderizar(e, intencao),
    }
}

/// **O dossiê** que sobe ao modelo, pelas mesmas regras.
///
/// A pergunta ABERTA leva tudo — inclusive a tabela. Foi a lição do combustível: um estado
/// com tanque para 6 voltas e 8 por correr produzia, para "como estamos?", um dossiê que
/// não mencionava combustível. Quem não delimitou o assunto está pedindo o retrato todo, e
/// a posição no campeonato faz parte dele.
pub fn dossie(e: &EstadoAgora, camp: Option<&Contexto>, intencao: Intencao) -> Vec<String> {
    dossie_com(
        e,
        &Extras {
            campeonato: camp.cloned().unwrap_or_default(),
            ..Extras::default()
        },
        intencao,
    )
}

/// A forma completa, com tudo o que o save sabe.
pub fn dossie_com(e: &EstadoAgora, extras: &Extras, intencao: Intencao) -> Vec<String> {
    let mut linhas = match intencao {
        Intencao::Campeonato => Vec::new(),
        Intencao::Geral => fatos::dossie_completo(e),
        _ => fatos::dossie(e, intencao),
    };
    if matches!(
        intencao,
        Intencao::Campeonato | Intencao::Posicao | Intencao::Geral
    ) {
        linhas.extend(campeonato::linhas(&extras.campeonato));
    }
    // QUEM é o vizinho entra sempre que a pergunta o envolve — inclusive na aberta, onde o
    // retrato da corrida sem nome nenhum é um retrato de trânsito.
    for (frente, viz) in [(true, &e.frente), (false, &e.atras)] {
        let cabe = match intencao {
            Intencao::Frente => frente,
            Intencao::Atras => !frente,
            Intencao::Geral => true,
            _ => false,
        };
        if !cabe {
            continue;
        }
        if let Some(l) = viz
            .as_ref()
            .and_then(|v| vizinhanca::linha(v, frente, &extras.vizinhanca))
        {
            linhas.push(l);
        }
    }
    // A variação desde a última resposta. Vai como fato FECHADO, e não como os dois gaps
    // para o modelo subtrair — pedir aritmética a quem redige é como se ganha uma conta
    // errada em prosa perfeita.
    if matches!(intencao, Intencao::Frente | Intencao::Atras) {
        if let Some(d) = extras.memoria {
            linhas.push(memoria::linha(d));
        }
    }
    linhas
}
