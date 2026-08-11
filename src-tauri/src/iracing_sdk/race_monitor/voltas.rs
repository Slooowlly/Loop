//! O CICLO DE VIDA DE UMA VOLTA: quando ela abre, quando fecha e com que tempo é gravada.
//!
//! Este módulo existe por um motivo medido, não por gosto de arquitetura. O jeito óbvio de
//! registrar uma volta é ler `LapCompleted` e `LapLastLapTime` no mesmo tique: quando a contagem
//! sobe, o piloto acabou de cruzar a linha, então o "último tempo de volta" deve ser o dela. É
//! isso que o monitor fazia, e está errado.
//!
//! ## O que a medição mostrou
//!
//! Sobre a captura real `race_1785016497.jsonl` (53.551 quadros a 58 Hz, treino + corrida):
//!
//! - No tique em que `LapCompleted` vira N, `LapLastLapTime` **ainda carrega o tempo da volta
//!   N-1**. O valor novo aparece entre 0,067 s e 0,433 s depois. Toda a lista de voltas saía
//!   deslocada em uma posição: a volta 9 foi gravada com 97,877 s, que é a duração real da
//!   volta 8; a volta 10 com 97,313 s, que é a da volta 9.
//! - Na troca de sessão o campo não zera. Na virada do treino para a corrida o monitor gravou
//!   "volta 6 = 97,581 s" — uma volta que não existiu na corrida, com o tempo de uma volta do
//!   TREINO. Essa é a primeira volta bugada que aparecia no painel.
//! - `LapLastLapTime` volta `-1` com frequência (volta anulada, saída do box, carro fora do
//!   mundo). O gate `> 0.0` descartava a volta inteira e em silêncio: das cinco voltas do
//!   treino nessa captura, quatro sumiram e sobrou uma, com o tempo de outra.
//! - `LapCompleted` cai para `-1` quando o carro sai do mundo e SALTA na troca de sessão
//!   (0 → 6, no meio da pista). Nenhum dos dois é uma volta.
//!
//! ## A regra que este módulo impõe
//!
//! **Um tempo de volta só é aceito quando o SDK publica um valor NOVO depois da virada.**
//! Enquanto o campo repetir o que já exibia, ele é o tempo da volta anterior e não vale nada.
//! Isso mata o vazamento entre voltas e entre sessões por construção, em vez de por remendo.
//!
//! E quando o valor novo não chega — volta anulada pelo sim, out lap, quadro perdido —, a volta
//! não é jogada fora: ela é fechada com a duração que **nós** cronometramos entre as duas
//! viradas. Medido contra o valor oficial na mesma captura, o cronômetro erra 0,010 s. É melhor
//! ter a volta com dez milésimos de folga do que ter um buraco na lista.

/// Quanto esperamos o SDK publicar o tempo OFICIAL antes de fechar a volta pelo cronômetro.
///
/// Um segundo e meio. O maior atraso medido foi 0,433 s, e a folga de três vezes cobre um
/// engasgo de amostragem sem chegar perto da volta seguinte — a mais curta do iRacing (oval de
/// um quarto de milha) passa dos dez segundos.
pub(super) const ESPERA_TEMPO_OFICIAL_S: f64 = 1.5;

/// Fora desta faixa o valor não é a duração de uma volta, é sujeira de sentinela ou de âncora
/// velha. O piso baixo é de propósito: oval curto tem volta de poucos segundos.
const MIN_VOLTA_S: f64 = 1.0;
const MAX_VOLTA_S: f64 = 3600.0;

/// Abaixo disto dois tempos são o mesmo número. O canal é `f32` promovido a `f64`, então a
/// comparação é exata na prática; a tolerância existe só para não depender disso.
const IGUAIS_S: f64 = 1e-6;

/// O que o monitor entrega ao coletor a cada tique.
#[derive(Debug, Clone, Copy)]
pub(super) struct Tique {
    /// `SessionTime` deste tique.
    pub agora: f64,
    /// `LapCompleted` (jogador) ou `CarIdxLapCompleted` (um carro). Negativo = fora do mundo.
    pub contagem: i32,
    /// `LapLastLapTime` / `CarIdxLastLapTime`. Atrasado — ver o cabeçalho do módulo.
    pub ultimo_tempo_sdk: f64,
    /// `LapCurrentTime` do tique. Só o jogador tem esse canal; `None` para os demais carros.
    /// No tique da virada ele ainda marca a volta que fechou, e serve de rede quando não
    /// vimos a abertura dela.
    pub cronometro_do_sim: Option<f64>,
    /// Combustível restante agora, carimbado na volta que fechar. Negativo = desconhecido.
    pub combustivel_l: f64,
    /// O carro está no pit road neste tique? Acumulado ao longo da volta.
    pub no_box: bool,
}

/// Uma volta que fechou, pronta para ser armazenada.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct VoltaFechada {
    /// O número da volta, na contagem do sim. Nunca 0 e nunca negativo.
    pub volta: i32,
    pub tempo_s: f64,
    /// Combustível no instante em que a volta fechou.
    pub combustivel_l: f64,
    /// A volta passou pelo pit road.
    pub passou_no_box: bool,
    /// O tempo veio do canal oficial do SDK. `false` = cronometrado por nós.
    pub oficial: bool,
}

/// A volta que já cruzou a linha e espera o tempo oficial do SDK.
#[derive(Debug, Clone, Copy)]
struct Pendente {
    volta: i32,
    /// Duração que nós medimos. `None` quando não vimos a abertura desta volta.
    cronometrado_s: Option<f64>,
    /// O valor obsoleto que o SDK ainda exibia na virada. É contra ele que se decide se o
    /// tempo novo chegou.
    obsoleto_s: f64,
    /// `session_time` da virada — o começo da espera.
    desde: f64,
    combustivel_l: f64,
    passou_no_box: bool,
}

/// O ciclo de vida das voltas de UM carro (ou do jogador).
#[derive(Debug, Clone, Copy)]
pub(super) struct ColetorDeVoltas {
    /// Última contagem vista. `None` = ainda não observamos nenhum tique válido, e é o que
    /// impede a "volta 0" de nascer no primeiro tique da sessão.
    ultima_contagem: Option<i32>,
    /// `session_time` da última virada — a âncora do cronômetro. `None` = sem âncora, ou
    /// porque ainda não houve virada, ou porque um salto a invalidou.
    virada_em: Option<f64>,
    pendente: Option<Pendente>,
    /// O carro passou pelo pit road na volta EM ANDAMENTO. Zera na virada, e é justamente
    /// esse reset que faz o box de uma volta não sujar a seguinte.
    viu_box: bool,
}

impl ColetorDeVoltas {
    pub(super) const DEFAULT: ColetorDeVoltas = ColetorDeVoltas {
        ultima_contagem: None,
        virada_em: None,
        pendente: None,
        viu_box: false,
    };

    /// Sessão nova, tentativa nova. Tudo que sobrasse aqui viraria volta da corrida errada.
    pub(super) fn reiniciar(&mut self) {
        *self = ColetorDeVoltas::DEFAULT;
    }

    /// Nenhuma volta em observação. É o estado de quem nunca viu um tique válido — e o que
    /// um reset precisa deixar para trás.
    pub(super) fn ocioso(&self) -> bool {
        self.ultima_contagem.is_none() && self.pendente.is_none() && self.virada_em.is_none()
    }

    /// Um tique. Devolve a volta que fechou, quando alguma fechar.
    pub(super) fn tique(&mut self, e: Tique) -> Option<VoltaFechada> {
        // Fora do mundo (garagem, guincho, carro ainda não carregado) o SDK devolve -1 na
        // contagem e sentinela no resto. Não é volta nem reinício: é ausência de dado, e
        // tratá-la como qualquer um dos dois inventaria evento. Congela e sai.
        if e.contagem < 0 {
            return None;
        }
        self.viu_box |= e.no_box;

        let mut fechada = self.tentar_confirmar(&e);
        let virou = self
            .ultima_contagem
            .is_some_and(|anterior| e.contagem == anterior + 1)
            && e.contagem >= 1;

        if virou {
            // A virada é o limite absoluto da espera: se o tempo oficial não chegou até aqui,
            // ele não vem mais — o campo já pertence à volta que acabou de fechar.
            if let Some(p) = self.pendente.take() {
                fechada = fechada.or_else(|| Self::pelo_cronometro(&p));
            }
            self.pendente = Some(Pendente {
                volta: e.contagem,
                cronometrado_s: self.duracao_medida(&e),
                obsoleto_s: e.ultimo_tempo_sdk,
                desde: e.agora,
                combustivel_l: e.combustivel_l,
                passou_no_box: self.viu_box,
            });
            self.virada_em = Some(e.agora);
            // A volta nova começa limpa. Sem este reset o box de uma volta reapareceria na
            // seguinte, e uma parada marcaria duas voltas. O tique da virada ainda conta para
            // ela: quem cruza a linha dentro do pit road está na out lap.
            self.viu_box = e.no_box;
        } else if self.ultima_contagem != Some(e.contagem) {
            // Salto: troca de sessão (0 → 6 no meio da pista), reidratação do SDK, reinício.
            // Nada disso é uma volta observada. Reposiciona o contador e derruba a âncora —
            // a próxima volta só será gravada se for vista do começo ao fim.
            self.pendente = None;
            self.virada_em = None;
            self.viu_box = e.no_box;
        }
        self.ultima_contagem = Some(e.contagem);
        fechada
    }

    /// O tempo oficial chegou? O sinal é ele MUDAR: enquanto repetir o que exibia na virada,
    /// é o tempo da volta anterior. Se a janela estourar, fecha pelo cronômetro.
    fn tentar_confirmar(&mut self, e: &Tique) -> Option<VoltaFechada> {
        let p = self.pendente?;
        let chegou =
            e.ultimo_tempo_sdk > 0.0 && (e.ultimo_tempo_sdk - p.obsoleto_s).abs() > IGUAIS_S;
        if chegou {
            self.pendente = None;
            return Some(VoltaFechada {
                volta: p.volta,
                tempo_s: e.ultimo_tempo_sdk,
                combustivel_l: p.combustivel_l,
                passou_no_box: p.passou_no_box,
                oficial: true,
            });
        }
        if e.agora - p.desde >= ESPERA_TEMPO_OFICIAL_S {
            self.pendente = None;
            return Self::pelo_cronometro(&p);
        }
        None
    }

    /// A volta que o sim não quis cronometrar (anulada, out lap, quadro perdido) fecha com a
    /// nossa medição. Sem medição, ela não existiu para nós e não entra na lista.
    fn pelo_cronometro(p: &Pendente) -> Option<VoltaFechada> {
        p.cronometrado_s.map(|s| VoltaFechada {
            volta: p.volta,
            tempo_s: s,
            combustivel_l: p.combustivel_l,
            passou_no_box: p.passou_no_box,
            oficial: false,
        })
    }

    /// Quanto durou a volta que acabou de fechar, pela nossa conta.
    ///
    /// A âncora entre as duas viradas é a fonte melhor (erra 0,010 s contra o oficial). O
    /// `LapCurrentTime` do tique entra como segunda opção porque cobre o caso que a âncora não
    /// cobre: a PRIMEIRA virada observada, quando não vimos a abertura da volta.
    fn duracao_medida(&self, e: &Tique) -> Option<f64> {
        let plausivel = |s: f64| (MIN_VOLTA_S..=MAX_VOLTA_S).contains(&s);
        self.virada_em
            .map(|a| e.agora - a)
            .filter(|s| plausivel(*s))
            .or_else(|| e.cronometro_do_sim.filter(|s| plausivel(*s)))
    }
}

#[cfg(test)]
mod tests;
