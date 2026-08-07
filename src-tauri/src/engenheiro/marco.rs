//! Os MARCOS da carreira: as duas bandeiradas que não são só mais uma bandeirada.
//!
//! A ocasião da bandeirada ([`super::momento::Ocasiao::DepoisDaBandeirada`]) já existe e vale
//! para toda corrida. Este módulo pergunta se aquela bandeirada específica era **outra
//! coisa** — a primeira vitória de uma carreira, ou o título. As duas acontecem uma vez, e
//! uma fala genérica em cima delas é a definição de perder o momento.
//!
//! ## Por que aqui, e não no `momento`
//!
//! [`super::momento`] só conhece telemetria, e é por isso que ele é testável sem nada aberto.
//! Nada do que decide um marco está na telemetria: quantas vitórias você tem, quantas
//! corridas já disputou e quantas ainda faltam na temporada são todas do **save**. Misturar as
//! duas fontes lá dentro custaria a propriedade que faz aquele módulo valer.
//!
//! ## O instante em que isto é perguntado
//!
//! A bandeirada acabou de cair e o resultado **ainda não entrou no save** — no Loop o
//! resultado oficial volta por importação, depois, e não enquanto o piloto está no carro. É o
//! que torna a conta possível: `vitorias` e `corridas` são os números de ANTES desta corrida,
//! e a posição vem da telemetria, ao vivo. Se um dia o save passar a ser atualizado antes
//! disto, o efeito é conhecido e é o benigno: `vitorias` já valeria 1 e o marco simplesmente
//! não sairia. Nunca o contrário.
//!
//! ## O título sai de uma PROJEÇÃO, e isso é de propósito
//!
//! Campeão só é campeão depois de a temporada ser fechada — e fechar a temporada é uma coisa
//! que acontece minutos ou dias depois, longe do carro. Esperar por ela trocaria o instante
//! em que a fala vale por um instante em que ela não vale nada.
//!
//! Então a conta é a mesma da tabela no rádio: [`super::campeonato::projetar`], que responde
//! "onde você terminaria se a corrida acabasse agora" — e a corrida acabou mesmo agora. Ela é
//! **tudo ou nada**: um único carro do grid que não case com um piloto do save mata a
//! projeção inteira. Um título anunciado errado com a voz confiante do engenheiro é o pior
//! defeito possível desta feature, e o modo de falha herdado é o certo — na dúvida, ela some,
//! e o piloto ouve a fala normal de bandeirada.

use super::fala::cardinal;

/// Corridas de carreira **antes desta** para a vitória de estreia merecer fala própria.
///
/// Duas, ou seja: a vitória tem de ser na terceira corrida ou depois. Vencer de cara não é
/// uma história — é um piloto na categoria errada, e um engenheiro que faz discurso sobre
/// isso soa como quem nunca viu corrida.
const CORRIDAS_ANTES: u32 = 2;

/// O que o save sabe sobre a carreira no instante da bandeirada.
///
/// Tudo `Default` é "não sei", e "não sei" nunca produz marco nenhum: zero vitórias com zero
/// corridas é a estreia, e a estreia está abaixo do mínimo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contexto {
    /// Vitórias de carreira ANTES desta corrida.
    pub vitorias: u32,
    /// Corridas de carreira ANTES desta corrida.
    pub corridas: u32,
    /// Esta é a última corrida da temporada na categoria dele?
    pub ultima_da_temporada: bool,
    /// Onde ele terminaria a temporada se a corrida acabasse agora — e ela acabou. `None`
    /// quando a conta não fecha; ver o cabeçalho.
    pub projecao: Option<i32>,
}

/// A bandeirada que era outra coisa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marco {
    PrimeiraVitoria,
    Titulo,
}

impl Marco {
    /// O nome que vai ao servidor e escolhe o prompt.
    pub fn nome(self) -> &'static str {
        match self {
            Marco::PrimeiraVitoria => "primeira_vitoria",
            Marco::Titulo => "titulo",
        }
    }
}

/// **O marco desta bandeirada**, se houver um.
///
/// `posicao` é a da telemetria, ao vivo. Só é chamada quando a bandeira já é a da chegada —
/// quem decide isso é [`super::momento::ocasiao`].
///
/// O TÍTULO ganha da primeira vitória quando as duas coincidem, e as linhas de contexto
/// carregam a estreia junto: quem vence pela primeira vez na corrida que decide o campeonato
/// tem uma história só, e o campeonato é o assunto dela.
pub fn na_bandeirada(posicao: i32, c: &Contexto) -> Option<Marco> {
    if c.ultima_da_temporada && c.projecao == Some(1) {
        return Some(Marco::Titulo);
    }
    if posicao == 1 && c.vitorias == 0 && c.corridas >= CORRIDAS_ANTES {
        return Some(Marco::PrimeiraVitoria);
    }
    None
}

/// Os fatos do marco, para o modelo redigir em cima — nunca para calcular.
///
/// A contagem de corridas entra por extenso, como todo número que sobe ao servidor: o modelo
/// copia o que está escrito, e é assim que os erros de leitura de número sumiram.
pub fn linhas(m: Marco, c: &Contexto) -> Vec<String> {
    let estreia = posicao_de_estreia(c);
    match m {
        Marco::PrimeiraVitoria => {
            let mut v = vec!["Esta é a PRIMEIRA VITÓRIA da carreira dele.".to_string()];
            v.extend(estreia);
            v
        }
        Marco::Titulo => {
            let mut v = vec![
                "Ele acaba de ser CAMPEÃO da temporada — esta era a última corrida do ano."
                    .to_string(),
            ];
            // A estreia junto, quando as duas caem na mesma corrida. Sem esta linha o marco
            // maior apagaria o menor, e o piloto ouviria falar do título sem uma palavra
            // sobre a primeira vitória da vida dele.
            if c.vitorias == 0 && c.corridas >= CORRIDAS_ANTES {
                v.push("E é também a primeira vitória da carreira dele.".to_string());
                v.extend(estreia);
            }
            v
        }
    }
}

/// "Ele tinha disputado doze corridas até aqui." — o peso da espera, em palavras.
///
/// `None` quando o número não é dizível pelo acervo de cardinais. Vale a mesma regra do resto:
/// o que não se sabe dizer não entra, em vez de entrar como algarismo no meio da prosa.
fn posicao_de_estreia(c: &Contexto) -> Option<String> {
    cardinal(i32::try_from(c.corridas).ok()?)
        .map(|n| format!("Ele tinha disputado {n} corridas até esta."))
}
