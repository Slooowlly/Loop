//! O ENGENHEIRO NA CLASSIFICAÇÃO: a janela em que ele pode falar de verdade.
//!
//! Todas as outras famílias do rádio são curtas por obrigação — o piloto está correndo. Aqui não:
//! a volta de preparação é o único trecho de um fim de semana em que ele tem tempo de sobra e
//! nada para decidir. É onde o engenheiro deixa de ser um alarme e vira alguém.
//!
//! ## Três regras de tempo, e elas mandam mais que o conteúdo
//!
//! 1. **A linha é prazo duro.** Nada pode invadir a volta rápida. Cada peça só começa se couber
//!    antes da linha, e a conta é `lap_dist_pct` com a velocidade — não relógio.
//! 2. **A despedida é agendada pelo FIM.** Ela é a última coisa que ele diz, e tem que acabar
//!    pouco antes da linha, não começar num instante qualquer. Por isso ela declara a própria
//!    duração ([`DESPEDIDA_MAX_S`]) e um guard estrutural confere isso contra os `.wav`.
//! 3. **Na volta rápida, silêncio.** A exceção é a volta que já morreu — aí falar não atrapalha
//!    mais nada, e calar seria desperdiçar o único momento em que o piloto quer ouvir alguém.
//!
//! ## Por que a volta morta é a fala mais importante daqui
//!
//! É a única em que o engenheiro **repara** em você. As outras informam; esta reage. E ela vem
//! com a conta que o piloto realmente quer: não "faltam quatro minutos", que obriga a fazer
//! contas de cabeça a 200 km/h, mas quantas tentativas ainda cabem — porque é assim que ele
//! pensa. Ver [`tentativas_que_cabem`].

/// Prefixo das chaves desta família no catálogo do engenheiro.
pub const PREFIXO: &str = "cl_";

/// Teto de duração da despedida, em segundos.
///
/// Não é estimativa: um guard estrutural lê os `.wav` de `cl_despedida_*` e falha se algum
/// passar disto. É o número que decide QUANDO ela começa — se ela for mais longa que o
/// declarado, o engenheiro atravessa a linha falando, que é exatamente o que não pode.
pub const DESPEDIDA_MAX_S: f64 = 4.0;

/// Folga entre o fim da despedida e a linha. Meio segundo de silêncio antes da volta lançada é
/// deliberado: a última coisa que o piloto ouve não deve ser a voz dele, deve ser o motor.
pub const FOLGA_ANTES_DA_LINHA_S: f64 = 0.6;

/// Uma fala pronta: as peças na ordem de tocar, e o texto que elas somam para o card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fala {
    pub pecas: Vec<String>,
    pub texto: String,
}

// ─── As redações ─────────────────────────────────────────────────────────────

/// A DESPEDIDA, dita nos últimos segundos da volta de preparação.
///
/// É a única fala do acervo inteiro que não informa nada. Não tem número, não tem nome, não tem
/// dado — o engenheiro só **quer** alguma coisa. É de propósito: é o que separa uma voz de um
/// painel de instrumentos, e é o pedido que originou esta família.
///
/// Duas frases por peça, e aqui o respiro interno é o certo — o mesmo caso do conselho de
/// poupar. O gerador tem isenção para texto de mais de uma frase.
pub fn despedida(variante: usize) -> (&'static str, &'static str) {
    [
        (
            "cl_despedida_0",
            "Vamos lá. Sei que você consegue um bom tempo pra nós.",
        ),
        (
            "cl_despedida_1",
            "A equipe espera muito de você. Essa é a sua hora.",
        ),
        ("cl_despedida_2", "É agora. Capricha nessa."),
        ("cl_despedida_3", "Confio em você. Traz um tempo bom."),
        (
            "cl_despedida_4",
            "A equipe inteira está olhando. Manda ver.",
        ),
        ("cl_despedida_5", "Essa é a volta. Faz valer."),
    ][variante % 6]
}

/// O reconhecimento de que a volta morreu. Curto de propósito: o piloto já sabe, e a fala existe
/// para dizer que ELE não está sozinho nisso, não para explicar o que aconteceu.
pub fn perdeu(variante: usize) -> (&'static str, &'static str) {
    [
        ("cl_perdeu_0", "Perdeu essa."),
        ("cl_perdeu_1", "Essa foi embora."),
        ("cl_perdeu_2", "Deixa essa pra lá."),
    ][variante % 3]
}

/// Quantas tentativas ainda cabem. Vem depois do reconhecimento, e é a metade útil da fala.
///
/// `None` acima do teto gravado: com muitas tentativas pela frente o número não muda decisão
/// nenhuma, e a peça genérica diz o que importa.
pub fn restam(tentativas: u32) -> (&'static str, &'static str) {
    match tentativas {
        1 => ("cl_restam_1", "Ainda dá pra mais uma."),
        2 => ("cl_restam_2", "Ainda dá pra mais duas."),
        3 => ("cl_restam_3", "Ainda dá pra mais três."),
        4 => ("cl_restam_4", "Ainda dá pra mais quatro."),
        _ => ("cl_restam_muitas", "Ainda temos bastante tempo."),
    }
}

/// Não cabe mais nenhuma. A fala que não consola — e é por não consolar que ela vale.
///
/// Dizer "ainda dá tempo" quando não dá é a única coisa que faria o piloto desconfiar de tudo o
/// mais que este rádio diz.
pub fn acabou(variante: usize) -> (&'static str, &'static str) {
    [
        ("cl_acabou_0", "Era essa. Volta pro box."),
        (
            "cl_acabou_1",
            "Acabou o nosso tempo aqui. Traz ele de volta.",
        ),
        ("cl_acabou_2", "Não dá pra outra. Pode encerrar."),
    ][variante % 3]
}

// ─── A aritmética das tentativas ─────────────────────────────────────────────

/// Quantas tentativas inteiras ainda cabem no tempo que resta.
///
/// Cada tentativa custa **duas** voltas: a de preparação e a lançada. Contar só a lançada seria
/// prometer uma tentativa que não existe — e a fala perderia a única coisa que a torna útil.
///
/// Sem referência de tempo de volta (`volta_s <= 0`), devolve `None`: sem saber quanto custa uma
/// tentativa não há como dizer quantas cabem, e chutar aqui é chutar no ouvido do piloto.
pub fn tentativas_que_cabem(restante_s: f64, volta_s: f64) -> Option<u32> {
    if volta_s <= 0.0 || restante_s < 0.0 {
        return None;
    }
    Some((restante_s / (volta_s * 2.0)).floor().max(0.0) as u32)
}

// ─── O observador ────────────────────────────────────────────────────────────

/// Onde estamos na classificação, do ponto de vista do rádio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Fase {
    /// No box, ou fora da sessão.
    #[default]
    Parado,
    /// Volta de preparação: o engenheiro pode falar à vontade.
    Preparacao,
    /// Volta lançada: silêncio, salvo se ela morreu.
    Voando,
}

/// Um instante da classificação. O que é bookkeeping do SDK (onde estamos na volta, se saímos do
/// box) fica com o monitor; o que é decisão de rádio fica aqui.
#[derive(Debug, Clone, Copy)]
pub struct Momento {
    /// Segundos até cruzar a linha, no ritmo atual. Negativo/infinito = desconhecido.
    pub ate_a_linha_s: f64,
    /// Tempo de sessão que resta.
    pub restante_s: f64,
    /// A melhor volta da sessão, para a conta das tentativas. 0 = nenhuma ainda.
    pub volta_referencia_s: f64,
    /// A volta em curso já morreu (saiu da pista, ou delta acima do limiar).
    pub volta_morta: bool,
    /// O carro está numa volta de preparação (acabou de sair do box ou abortou a anterior).
    pub em_preparacao: bool,
    /// O carro está numa volta lançada.
    pub voando: bool,
}

/// Estado por sessão de classificação. Zerado a cada nova.
#[derive(Debug, Default)]
pub struct Observador {
    fase: Fase,
    /// Rodízio das redações — a mesma frase duas vezes na mesma sessão soa mecânica justo na
    /// família que existe para soar humana.
    despedidas: usize,
    perdidas: usize,
    acabadas: usize,
    /// A despedida desta preparação já saiu.
    despediu: bool,
    /// A morte desta volta lançada já foi comentada.
    comentou_morte: bool,
}

impl Observador {
    pub const fn novo() -> Observador {
        Observador {
            fase: Fase::Parado,
            despedidas: 0,
            perdidas: 0,
            acabadas: 0,
            despediu: false,
            comentou_morte: false,
        }
    }

    pub fn reiniciar(&mut self) {
        *self = Observador::novo();
    }

    /// Um tique. Devolve o que dizer, ou `None` — que é a resposta na esmagadora maioria deles.
    pub fn observar(&mut self, m: Momento) -> Option<Fala> {
        let fase = if m.voando {
            Fase::Voando
        } else if m.em_preparacao {
            Fase::Preparacao
        } else {
            Fase::Parado
        };
        if fase != self.fase {
            // Mudou de volta: rearma o que é por volta. O rodízio NÃO se rearma — ele é por
            // sessão, senão a mesma despedida sairia em toda tentativa.
            self.fase = fase;
            self.despediu = false;
            self.comentou_morte = false;
        }

        match fase {
            Fase::Preparacao => self.na_preparacao(m),
            Fase::Voando => self.voando(m),
            Fase::Parado => None,
        }
    }

    /// A despedida, agendada para TERMINAR pouco antes da linha.
    fn na_preparacao(&mut self, m: Momento) -> Option<Fala> {
        if self.despediu {
            return None;
        }
        // A janela tem PISO e teto, e o piso é o que importa. Teto sozinho ("faltam menos de
        // 4,6 s") deixaria a fala começar faltando meio segundo — e aí ela termina dentro da
        // volta lançada, com o piloto entrando na primeira curva ouvindo o engenheiro. O piso é
        // a duração declarada da própria fala: se não cabe inteira, não começa.
        //
        // Perder a janela (a telemetria pular de 5,0 s para 3,5 s) custa a despedida daquela
        // tentativa. É a falha certa: silêncio em vez de atropelo.
        let teto = DESPEDIDA_MAX_S + FOLGA_ANTES_DA_LINHA_S;
        if !(m.ate_a_linha_s.is_finite()
            && m.ate_a_linha_s >= DESPEDIDA_MAX_S
            && m.ate_a_linha_s <= teto)
        {
            return None;
        }
        self.despediu = true;
        let (chave, texto) = despedida(self.despedidas);
        self.despedidas += 1;
        Some(Fala {
            pecas: vec![chave.to_string()],
            texto: texto.to_string(),
        })
    }

    /// Na volta lançada ele cala — a menos que ela tenha morrido.
    fn voando(&mut self, m: Momento) -> Option<Fala> {
        if self.comentou_morte || !m.volta_morta {
            return None;
        }
        self.comentou_morte = true;

        let (k_perdeu, t_perdeu) = perdeu(self.perdidas);
        self.perdidas += 1;

        // O que sobra de sessão a partir de AGORA já desconta a volta em curso: ela morreu, o
        // piloto vai abortar, e a próxima tentativa começa da linha.
        let sobra = m.restante_s - m.ate_a_linha_s.max(0.0);
        let cabem = tentativas_que_cabem(sobra, m.volta_referencia_s);
        let (k2, t2) = match cabem {
            Some(0) => {
                let (k, t) = acabou(self.acabadas);
                self.acabadas += 1;
                (k, t)
            }
            Some(n) => restam(n),
            // Sem referência de volta não dá para contar tentativas. Ele reconhece e para por
            // aí — meia fala honesta em vez de um número inventado.
            None => {
                return Some(Fala {
                    pecas: vec![k_perdeu.to_string()],
                    texto: t_perdeu.to_string(),
                })
            }
        };
        Some(Fala {
            pecas: vec![k_perdeu.to_string(), k2.to_string()],
            texto: format!("{t_perdeu} {t2}"),
        })
    }
}

/// TODA peça que esta família pode emitir, com o texto exato a gravar.
pub fn familia_classificacao() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for i in 0..6 {
        let (k, t) = despedida(i);
        v.push((k.to_string(), t.to_string()));
    }
    for i in 0..3 {
        let (k, t) = perdeu(i);
        v.push((k.to_string(), t.to_string()));
    }
    for n in [1u32, 2, 3, 4, 99] {
        let (k, t) = restam(n);
        v.push((k.to_string(), t.to_string()));
    }
    for i in 0..3 {
        let (k, t) = acabou(i);
        v.push((k.to_string(), t.to_string()));
    }
    v
}

#[cfg(test)]
mod tests;
