//! O RÁDIO DE RITMO: o engenheiro comenta a volta mais rápida sem ser perguntado.
//!
//! As outras famílias do engenheiro respondem — o push-to-talk pergunta, elas devolvem. Esta
//! e a de [`quebra`](super::quebra) são as duas que abrem o canal sozinhas, e por isso as duas
//! precisam responder à mesma pergunta antes de existir: **quantas vezes por corrida**.
//!
//! ## Três eventos, três frequências, três defesas diferentes
//!
//! | evento | quando | quantas vezes |
//! |---|---|---|
//! | a volta mais rápida CAIU | alguém baixou a marca da corrida | limitado por intervalo |
//! | NÓS tomamos a melhor | o jogador cravou a volta mais rápida | toda vez — é o prêmio |
//! | estamos encostando | nossa volta a menos de 1 s da melhor | limitado por intervalo |
//!
//! **O primeiro já foi "mudou de dono", e isso o deixava mudo.** Medido em três corridas
//! gravadas (`docs/radio-carga.md`): a marca melhorou 2, 3 e 5 vezes, e trocou de carro **zero**
//! vez nas duas provas longas — um carro crava a melhor volta cedo e depois só melhora a própria.
//!
//! A do meio não tem limite de propósito: ela é rara por natureza (o jogador não crava a
//! volta mais rápida toda volta) e é o momento que o rádio existe para marcar. Segurá-la seria
//! calar justamente a boa notícia.
//!
//! As outras duas têm. A volta mais rápida troca de dono muitas vezes nas primeiras voltas,
//! quando o grid inteiro ainda está melhorando a cada passagem — anunciar cada troca seria um
//! locutor de leilão. E a aproximação, sem trava, sairia em toda volta em que o piloto
//! estivesse num ritmo bom.
//!
//! ## Por que o intervalo é em VOLTAS e não em segundos
//!
//! Porque o evento é por volta. Um intervalo em segundos valeria coisas diferentes em Lime
//! Rock (40 s) e em Spa (2 min) — três voltas numa, uma volta na outra. Em voltas, "espera
//! três" quer dizer a mesma coisa nas duas.

use super::tempo_volta as tv;

/// Quantas voltas esperar antes de repetir um comentário do mesmo tipo.
///
/// Três. Numa corrida de 18 voltas isso dá um teto de seis anúncios por tipo — e na prática
/// bem menos, porque a volta mais rápida estabiliza depois do primeiro terço.
pub const INTERVALO_VOLTAS: i32 = 3;

/// Uma volta completada, do ponto de vista do rádio de ritmo.
#[derive(Debug, Clone, Copy)]
pub struct Passagem {
    pub volta: i32,
    /// Nossa última volta, em segundos. `<= 0` = sem volta válida.
    pub minha_volta_s: f64,
    /// A volta mais rápida da corrida, em segundos. `<= 0` = ninguém marcou.
    pub melhor_da_corrida_s: f64,
    /// `car_idx` de quem a detém. -1 se não há.
    pub dono_idx: i32,
    pub e_minha: bool,
}

/// O que o rádio decidiu dizer, se decidiu algo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fala {
    /// Nós cravamos a volta mais rápida. Uma chave, frase inteira.
    Tomamos(String),
    /// A volta mais rápida é de outro. `(lead, tempo)` — o SOBRENOME entra entre os dois, e
    /// quem o resolve é quem tem o banco: aqui não se sabe o nome de ninguém.
    DeOutro { lead: String, tempo: String, dono_idx: i32 },
    /// Estamos a menos de um segundo dela.
    Aproximando(String),
}

/// Estado por corrida. Zerado a cada largada.
#[derive(Debug, Default)]
pub struct Observador {
    ultimo_dono: i32,
    /// A melhor volta que já foi anunciada (ou ancorada). É o que separa "melhorou" de "trocou
    /// de dono" — ver [`Observador::observar`].
    ultima_melhor_s: f64,
    ultima_volta_anunciada: i32,
    ultima_volta_aproximacao: i32,
    /// Rodízio das três redações de "tomamos", para a mesma corrida não repetir a frase.
    tomadas: usize,
    iniciado: bool,
}

/// Abaixo disto, duas marcas são a mesma. Um milésimo — o tempo falado tem resolução de décimo,
/// então diferença menor que isso nem chega ao áudio.
const EPSILON_S: f64 = 0.001;

impl Observador {
    pub const fn novo() -> Observador {
        Observador {
            ultimo_dono: -1,
            ultima_melhor_s: 0.0,
            ultima_volta_anunciada: i32::MIN,
            ultima_volta_aproximacao: i32::MIN,
            tomadas: 0,
            iniciado: false,
        }
    }

    /// Corrida nova. Sem isto, a volta mais rápida da corrida anterior seguiria valendo como
    /// "dono anterior" e a primeira troca da corrida nova não seria anunciada.
    pub fn reiniciar(&mut self) {
        *self = Observador::novo();
    }

    /// Uma volta foi completada. Devolve o que dizer, ou `None`.
    pub fn observar(&mut self, p: Passagem) -> Option<Fala> {
        if p.melhor_da_corrida_s <= 0.0 || p.dono_idx < 0 {
            return None;
        }

        // A PRIMEIRA leitura só ancora. Sem isto, entrar na sessão com a corrida em andamento
        // anunciaria como "novidade" uma volta mais rápida marcada dez voltas atrás — e o
        // jogador ouviria um anúncio sobre algo que ele não viu acontecer.
        if !self.iniciado {
            self.iniciado = true;
            self.ultimo_dono = p.dono_idx;
            self.ultima_melhor_s = p.melhor_da_corrida_s;
            return None;
        }

        let trocou = p.dono_idx != self.ultimo_dono;
        // MELHOROU é o gatilho, e trocar de dono é só um caso dele.
        //
        // O gatilho original era a troca de dono, e ele estava morto. Medido em três corridas
        // gravadas (`docs/radio-carga.md`): a volta mais rápida melhorou 2, 3 e 5 vezes, e trocou
        // de carro ZERO vez nas duas provas longas. Um carro crava a melhor volta nas primeiras
        // passagens e depois só melhora a PRÓPRIA marca — como "melhorou" não era "trocou", a
        // família inteira ficava calada, com 14 peças gravadas e nenhuma chance de tocar.
        let melhorou = self.ultima_melhor_s <= 0.0
            || p.melhor_da_corrida_s < self.ultima_melhor_s - EPSILON_S;
        self.ultimo_dono = p.dono_idx;
        if melhorou {
            self.ultima_melhor_s = p.melhor_da_corrida_s;
        }

        if trocou && p.e_minha {
            // Sem trava: é raro por natureza e é o momento que o rádio existe para marcar.
            let (chave, _) = tv::tomamos_frase(self.tomadas);
            self.tomadas += 1;
            self.ultima_volta_anunciada = p.volta;
            return Some(Fala::Tomamos(chave.to_string()));
        }

        // A melhor volta MELHOROU e não é nossa. A trava de intervalo é o que segura o locutor de
        // leilão das primeiras voltas, quando o grid inteiro ainda melhora a cada passagem.
        if melhorou && !p.e_minha && self.passou_intervalo(self.ultima_volta_anunciada, p.volta) {
            if let Some(tempo) = tv::chave_tempo(tv::decimos_de(p.melhor_da_corrida_s)) {
                self.ultima_volta_anunciada = p.volta;
                return Some(Fala::DeOutro {
                    lead: tv::LEAD_MELHOR.0.to_string(),
                    tempo,
                    dono_idx: p.dono_idx,
                });
            }
            // Tempo fora da faixa gravada (pista de mais de quatro minutos): não anuncia, e
            // NÃO marca o intervalo — assim a próxima troca ainda tem chance.
        }

        // Encostando. Só faz sentido quando a melhor é de outro: perseguir a própria não é
        // perseguir nada, e "faltam zero décimos" seria pior que o silêncio.
        if !p.e_minha
            && p.minha_volta_s > 0.0
            && self.passou_intervalo(self.ultima_volta_aproximacao, p.volta)
        {
            let falta = tv::decimos_de(p.minha_volta_s - p.melhor_da_corrida_s);
            if let Some((chave, _)) = tv::aproximacao_frase(falta) {
                self.ultima_volta_aproximacao = p.volta;
                return Some(Fala::Aproximando(chave));
            }
        }

        None
    }

    fn passou_intervalo(&self, ultima: i32, agora: i32) -> bool {
        ultima == i32::MIN || agora - ultima >= INTERVALO_VOLTAS
    }
}

#[cfg(test)]
mod tests;
