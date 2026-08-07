//! A MEMÓRIA da conversa: o engenheiro lembrando do que já te disse.
//!
//! Até aqui ele respondia cada pergunta como se fosse a primeira. Perguntar o gap duas
//! vezes na mesma corrida dava dois números soltos, e quem fazia a subtração era o piloto —
//! a 200 por hora, de cabeça, enquanto dirige. É exatamente a conta que um engenheiro de
//! verdade já faz para você:
//!
//! ```text
//!   "Cooper, está a um e dois na sua frente. São quatro décimos a menos que da última vez."
//! ```
//!
//! ## O que ela guarda, e por que tão pouco
//!
//! O gap de cada lado, o CARRO a que ele se refere, e em que volta foi dito. Nada mais.
//!
//! O `idx` do carro é o campo que impede a mentira. Sem ele, uma ultrapassagem entre duas
//! perguntas faria a conta comparar dois carros diferentes — "são três segundos a menos que
//! da última vez" seria dito com toda a convicção sobre alguém que acabou de aparecer ali.
//! A memória só fala quando o carro é o MESMO.
//!
//! ## Ela mede o que foi DITO, não o que aconteceu
//!
//! "Desde a última vez" é desde a última vez que ele **te respondeu**, e não desde a volta
//! passada. Por isso o registro acontece no fim da resposta, e só nas perguntas que
//! realmente disseram um gap. Registrar em toda pergunta faria a frase se referir a um
//! instante sobre o qual o engenheiro nunca abriu a boca.

use crate::iracing_sdk::race_monitor::EstadoAgora;

use super::fala::grade_de_gaps;
use super::Intencao;

/// Prefixo das peças desta família.
pub const PREFIXO: &str = "mem_";

/// Menor variação que vale dizer, em DÉCIMOS.
///
/// Dois. Abaixo disso é ruído de medição e de tráfego — o gap oscila sozinho entre uma
/// curva e outra, e anunciar um décimo de variação faria o engenheiro comentar o próprio
/// erro de leitura.
///
/// Em décimos inteiros, e não em segundos, porque a comparação em `f64` mente na borda: um
/// gap de 1,2 menos um de 1,0 dá 0,19999999999999996, e um limiar de `>= 0.2` recusava
/// exatamente a variação que deveria aceitar. Arredondar primeiro também alinha o corte com
/// o que se FALA — a peça é escolhida por décimo, então o limiar tem de ser por décimo.
const MINIMO_DECIMOS: i64 = 2;

/// Por quantas voltas "a última vez" ainda é a última vez.
///
/// Cinco. É um julgamento, não uma medida: passado isso a comparação continua verdadeira e
/// deixa de ser útil — a corrida entre os dois instantes teve paradas, tráfego e talvez
/// uma amarela, e a diferença deixou de ser sobre o duelo.
const VALIDADE_VOLTAS: i32 = 5;

/// O que ficou dito sobre um lado.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Lembranca {
    /// `CarIdx` do carro sobre o qual se falou. É o que impede comparar dois carros.
    idx: i32,
    gap_s: f64,
    volta: i32,
}

/// A memória de uma corrida.
///
/// Zerada por SUBSESSÃO: cada corrida é uma conversa, e carregar o gap da anterior para a
/// seguinte compararia dois eventos diferentes.
#[derive(Clone, Debug, Default)]
pub struct Memoria {
    subsessao: i64,
    frente: Option<Lembranca>,
    atras: Option<Lembranca>,
}

/// Quanto o gap mudou desde a última vez que o engenheiro o disse.
///
/// Positivo = **diminuiu** (o carro está mais perto agora). É o sinal que a fala usa, e ele
/// é o mesmo dos dois lados de propósito: quem diz de quem se trata é a frase anterior.
pub type Delta = f64;

impl Memoria {
    /// Nova corrida apaga a conversa.
    fn sincronizar(&mut self, subsessao: i64) {
        if self.subsessao != subsessao {
            *self = Memoria {
                subsessao,
                ..Memoria::default()
            };
        }
    }

    fn lado(&self, frente: bool) -> Option<Lembranca> {
        if frente {
            self.frente
        } else {
            self.atras
        }
    }

    /// O que dizer sobre a variação, se houver o que dizer.
    ///
    /// `None` em todos os casos em que a comparação seria falsa ou inútil: outro carro,
    /// memória velha, variação dentro do ruído, ou nenhuma resposta anterior.
    pub fn consultar(&mut self, e: &EstadoAgora, subsessao: i64, intencao: Intencao) -> Option<Delta> {
        self.sincronizar(subsessao);
        let frente = match intencao {
            Intencao::Frente => true,
            Intencao::Atras => false,
            _ => return None,
        };
        let atual = if frente { e.frente.as_ref() } else { e.atras.as_ref() }?;
        let antes = self.lado(frente)?;
        // O MESMO carro. Sem esta linha, uma ultrapassagem entre duas perguntas faria a
        // conta comparar dois pilotos diferentes.
        if antes.idx != atual.idx {
            return None;
        }
        if e.volta - antes.volta > VALIDADE_VOLTAS {
            return None;
        }
        // Arredondado para o décimo AQUI, e não na hora de escolher a peça: assim o valor
        // que passa pelo limiar é o mesmo que vai ser dito.
        let decimos = ((antes.gap_s - atual.gap_s) * 10.0).round() as i64;
        (decimos.abs() >= MINIMO_DECIMOS).then(|| decimos as f64 / 10.0)
    }

    /// Registra o que acabou de ser dito. Só as perguntas que disseram um gap.
    pub fn registrar(&mut self, e: &EstadoAgora, subsessao: i64, intencao: Intencao) {
        self.sincronizar(subsessao);
        let frente = match intencao {
            Intencao::Frente => true,
            Intencao::Atras => false,
            _ => return,
        };
        let Some(v) = (if frente { &e.frente } else { &e.atras }) else {
            return;
        };
        // Um vizinho no box ou uma volta à parte não teve gap dito — a fala foi outra. Guardar
        // o número aqui faria a próxima comparação partir de um instante que ninguém anunciou.
        if v.no_box || v.volta_a_parte || !v.gap_s.is_finite() || v.gap_s < 0.0 {
            return;
        }
        let nova = Some(Lembranca {
            idx: v.idx,
            gap_s: v.gap_s,
            volta: e.volta,
        });
        if frente {
            self.frente = nova;
        } else {
            self.atras = nova;
        }
    }
}

/// A peça que diz a variação. `None` quando o valor está fora da grade gravada.
pub fn peca(delta: Delta) -> Option<String> {
    let sufixo = super::fala::sufixo_gap(delta.abs())?;
    let sentido = if delta > 0.0 { "menos" } else { "mais" };
    Some(format!("{PREFIXO}{sentido}_{sufixo}"))
}

/// A variação em prosa, para o dossiê do modelo.
pub fn linha(delta: Delta) -> String {
    format!(
        "Desde a última resposta sobre este carro, o gap {} {:.1} segundo(s)",
        if delta > 0.0 { "DIMINUIU" } else { "AUMENTOU" },
        delta.abs()
    )
}

/// Toda peça desta família.
///
/// A grade é a MESMA dos gaps, e por construção — uma variação de oito décimos se diz com
/// as mesmas palavras que uma distância de oito décimos, e duas listas divergiriam na
/// primeira resolução nova.
pub fn familia_memoria() -> Vec<(String, String)> {
    let mut v = Vec::new();
    for (sufixo, texto) in grade_de_gaps() {
        v.push((
            format!("{PREFIXO}menos_{sufixo}"),
            format!("São {texto} a menos que da última vez."),
        ));
        v.push((
            format!("{PREFIXO}mais_{sufixo}"),
            format!("São {texto} a mais que da última vez."),
        ));
    }
    v
}
