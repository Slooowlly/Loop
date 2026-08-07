//! Spotter de BANDEIRA — o que o `session_flags` diz sobre a prova e sobre o seu carro.
//!
//! É o mais simples dos detectores da família, e por um motivo: aqui não há inferência
//! nenhuma. O `session_flags` já vem pronto do SDK e o [`crate::iracing_sdk::race_monitor`]
//! já sabia lê-lo desde sempre — o `EstadoAgora` traz o rótulo em português na tela. O que
//! faltava era a boca. Este módulo só decide QUANDO vale falar.
//!
//! ## O que foi medido, em 36 min de corrida real
//!
//! Oito transições de bandeira nas três capturas do acervo. Na faixa das famílias raras, e
//! duas ordens de grandeza abaixo do spotter lateral. Três coisas saíram da medição:
//!
//! **`FLAG_YELLOW` e `FLAG_YELLOW_WAVING` nunca dispararam.** A amarela chega como
//! `CAUTION` (`0x4000`) e `CAUTION_WAVING` (`0x8000`). Um detector que lesse o bit de nome
//! óbvio ficaria mudo a corrida inteira, e o defeito só apareceria numa amarela de verdade.
//! Daí a máscara [`AMARELA`] juntar os quatro.
//!
//! **Juntar os quatro bits transforma 3 transições cruas em 1 episódio.** Em Okayama a
//! amarela durou 349,9 s; lida bit a bit, ela produziria três falas para um único evento.
//!
//! **Branca e reparo nunca DESLIGAM.** São estados que travam até o fim da prova — zero
//! episódios fechados no acervo. Uma fala por episódio já entrega exatamente uma fala, sem
//! precisar de regra especial.
//!
//! ## O que este módulo NÃO fala, de propósito
//!
//! **Azul.** A porta da bandeira azul já vive dentro de
//! [`crate::iracing_sdk::spotter_tras`], fundida com a de ritmo porque a ação do piloto é a
//! mesma nas duas: deixar passar. Uma fala de bandeira azul aqui seria a segunda fala sobre
//! o mesmo evento no mesmo instante.
//!
//! **Quadriculada.** Quando ela sai, a prova acabou e não há o que fazer com o aviso.
//!
//! **Vermelha.** Fora por decisão de escopo: na prática o iRacing não a usa em corrida
//! normal, e ela nunca apareceu no acervo.
//!
//! **Preta.** Esta é a mais importante de entender, porque a fala chegou a existir e foi
//! retirada. No Loop a preta **não é punição do iRacing — é sinal nosso**: o
//! [`crate::car::breakdown`] dispara `!black #N <segundos>` para simular quebra de peça,
//! com o tempo de parada próprio de cada componente (`!dq` para DNF). Anunciar "bandeira
//! preta, vá aos boxes" diria ao piloto que ele foi punido por pilotagem quando o que
//! houve foi o motor quebrando. A informação certa ali é sobre a peça, e quem a tem é o
//! engenheiro, não o spotter. **Não recrie esta chave sem resolver essa ambiguidade.**
//!
//! O [`REPARO`] (*meatball*) sobrevive justamente por não ter esse problema: é dano de
//! verdade lido do sim, e o `breakdown` não o usa para nada.
//!
//! ## Sem lembrete
//!
//! A amarela de Okayama durou quase seis minutos. Uma permanência a cada poucos segundos
//! ali viraria alarme de carro — o oposto do que o lembrete lateral faz, onde a disputa
//! dura segundos e a dúvida ("ele ainda está aí?") se renova. Aqui o estado é estável e a
//! informação não muda: fala uma vez, e cala.

use std::sync::{Mutex, MutexGuard, OnceLock};

/// Amarela em qualquer das quatro formas. **A máscara é o detector** — ver o cabeçalho.
const AMARELA: u32 = 0x0000_0008 | 0x0000_0100 | 0x0000_4000 | 0x0000_8000;
/// Branca: última volta.
const BRANCA: u32 = 0x0000_0002;
/// Preta com disco laranja (*meatball*): dano que obriga a ir aos boxes. É bandeira DO
/// JOGADOR — o `session_flags` é o dele, não o da prova.
const REPARO: u32 = 0x0010_0000;
pub const CHAVE_AMARELA: &str = "amarela";
pub const CHAVE_ULTIMA_VOLTA: &str = "ultima_volta";
pub const CHAVE_REPARO: &str = "reparo";
/// A RELARGADA — a amarela apagando.
///
/// Medido no acervo: `pace_mode` **não** serve de sinal. Em Lime Rock ele fica constante em
/// 1 a corrida inteira, sem uma bandeira na prova; em Okayama passeia por 1, 2 e 3 sem
/// coincidir com o que importa. O que marca a relargada com precisão é a borda de descida
/// da máscara [`AMARELA`]: em Okayama, `t=532,3 s`, uma vez.
pub const CHAVE_VERDE: &str = "verde";

const ESTADO_CORRIDA: i32 = 4;

/// Por quanto tempo a bandeira precisa estar acesa antes de virar fala.
///
/// Meio segundo mata o piscar de um quadro sem atrasar nada que importe: a amarela dura
/// minutos e a mais curta do acervo (a azul, que não falamos) durou 2,1 s.
const CONFIRMA_S: f64 = 0.5;

/// Por quanto tempo a bandeira precisa ficar APAGADA antes de um episódio novo poder abrir.
///
/// A azul de Okayama acendeu, apagou por **1,9 s** e acendeu de novo — o mesmo evento
/// partido em dois pelo sim. Sem este piso, um pisca desses vira duas falas idênticas com
/// dois segundos de intervalo, que é como o rádio perde a credibilidade.
const REARME_S: f64 = 5.0;

/// Por quanto tempo a amarela precisa estar apagada antes de o verde sair.
///
/// É uma tensão real e o número é o meio-termo. Curto demais, um pisca de 1,9 s — como o da
/// azul de Okayama — vira "amarela… verde… amarela". Longo demais, o verde chega depois de
/// o campo já ter acelerado, e é a fala mais perecível do módulo: ela diz *agora vale
/// correr*. Dois segundos passam o maior pisca do acervo e ainda saem antes de a fila
/// esticar. Não há pisca de AMARELA medido — a de Okayama foi um episódio único de 349,9 s
/// —, então isto é margem contra um caso não observado, não contra um medido.
const LIBERA_S: f64 = 2.0;

/// Salto de `session_time` que significa replay, rebobinada ou sessão nova.
const SALTO_MAX_S: f64 = 5.0;

/// As famílias, **em ordem de gravidade** — a mesma ordem do `rotulo_bandeira` do
/// `EstadoAgora`, para a tela e o rádio nunca discordarem sobre o que é mais grave.
///
/// O reparo vem antes da amarela porque é sobre O SEU carro: a amarela é contexto, o
/// meatball é uma ordem. E a branca vem por último porque é informação, não perigo.
const FAMILIAS: [(u32, &str); 3] = [
    (REPARO, CHAVE_REPARO),
    (AMARELA, CHAVE_AMARELA),
    (BRANCA, CHAVE_ULTIMA_VOLTA),
];

/// A fala de SAÍDA de cada família, na mesma ordem de [`FAMILIAS`]. Só a amarela tem uma.
///
/// O reparo some quando o piloto conserta — ele já sabe, estava lá. A branca não apaga: a
/// prova acaba com ela acesa. **A amarela é a única cuja saída é notícia**, e é a mais
/// importante do módulo: é a relargada.
const LIBERACAO: [Option<&str>; 3] = [None, Some(CHAVE_VERDE), None];

/// Uma amostra do tique. Só quatro campos — é tudo o que a decisão precisa.
pub struct AmostraBandeira {
    pub tempo_s: f64,
    pub estado_sessao: i32,
    pub bandeiras: u32,
    /// O jogador está no carro. Bandeira anunciada para quem está no box ou no menu é
    /// ruído: ele não pode fazer nada com ela, e quando voltar a informação já mudou.
    pub jogador_no_carro: bool,
}

/// O estado de uma família de bandeira.
#[derive(Debug, Clone, Copy, Default)]
struct Estado {
    /// Desde quando está acesa, ou `None` se apagada.
    acesa_desde_s: Option<f64>,
    /// Desde quando está apagada. `None` antes da primeira vez.
    apagada_desde_s: Option<f64>,
    /// O episódio atual já virou fala.
    avisado: bool,
    /// A amarela apagou depois de ter sido anunciada: a RELARGADA está devendo uma fala.
    /// Só a família amarela usa isto — ver [`CHAVE_VERDE`].
    liberacao_devida: bool,
}

/// O que a última chamada devolveu, à espera de confirmação: ou a bandeira de uma família,
/// ou a liberação dela.
#[derive(Debug, Clone, Copy)]
enum Pendente {
    Bandeira(usize),
    Liberacao(usize),
}

#[derive(Debug, Default)]
pub struct ObservadorBandeira {
    estados: [Estado; FAMILIAS.len()],
    ultimo_tempo_s: Option<f64>,
    pendente: Option<Pendente>,
}

impl ObservadorBandeira {
    pub fn novo() -> Self {
        Self::default()
    }

    fn zerar(&mut self) {
        self.estados = Default::default();
        self.pendente = None;
    }

    /// Alimenta o observador. Devolve no máximo uma chave por chamada — a mais grave que
    /// esteja pendente.
    pub fn observar(&mut self, a: AmostraBandeira) -> Option<&'static str> {
        // Replay, rebobinada ou sessão nova: o que estava aceso não descreve mais nada.
        if let Some(ant) = self.ultimo_tempo_s {
            if a.tempo_s < ant || a.tempo_s - ant > SALTO_MAX_S {
                self.zerar();
            }
        }
        self.ultimo_tempo_s = Some(a.tempo_s);
        self.pendente = None;

        // Fora de corrida o bitfield ainda existe (treino tem amarela, quali tem branca),
        // mas nada disso é notícia de rádio. Zerar aqui é o que faz a bandeira que já
        // estava acesa na largada NÃO virar uma fala no primeiro tique da prova.
        if a.estado_sessao != ESTADO_CORRIDA || !a.jogador_no_carro {
            self.zerar();
            return None;
        }

        for (i, (mascara, _)) in FAMILIAS.iter().enumerate() {
            let acesa = a.bandeiras & mascara != 0;
            let e = &mut self.estados[i];
            if acesa {
                if e.acesa_desde_s.is_none() {
                    // BORDA DE SUBIDA. O vão é medido AQUI, uma vez, contra o instante em
                    // que apagou — e não continuamente contra o relógio. A primeira versão
                    // comparava com o tempo corrente, e o efeito era o oposto do desejado:
                    // uma bandeira que voltava a acender e FICAVA acesa rearmava sozinha
                    // cinco segundos depois e falava de novo, sem ter apagado uma segunda
                    // vez. O teste do pisca de 1,9 s foi quem mostrou.
                    let vao_longo = e
                        .apagada_desde_s
                        .map(|t| a.tempo_s - t >= REARME_S)
                        .unwrap_or(true);
                    e.acesa_desde_s = Some(a.tempo_s);
                    // Vão curto é o mesmo evento partido pelo sim: o episódio continua o
                    // de antes, e um episódio já avisado não avisa duas vezes.
                    if vao_longo {
                        e.avisado = false;
                    }
                }
            } else {
                if e.acesa_desde_s.is_some() {
                    e.apagada_desde_s = Some(a.tempo_s);
                    // BORDA DE DESCIDA. Se a bandeira chegou a ser anunciada, a saída dela
                    // é notícia por si — a relargada. Como no `livre` do lateral: a
                    // liberação só existe se houve ocupação, e uma amarela que nunca foi
                    // dita não tem verde a dizer.
                    if e.avisado && LIBERACAO[i].is_some() {
                        e.liberacao_devida = true;
                    }
                }
                e.acesa_desde_s = None;
            }
        }

        // A LIBERAÇÃO vem antes de qualquer entrada. O verde é a informação mais perecível
        // do módulo: ele diz "agora vale correr", e chegar tarde é chegar depois de o campo
        // já ter acelerado.
        for i in 0..FAMILIAS.len() {
            let e = &self.estados[i];
            if !e.liberacao_devida {
                continue;
            }
            let Some(chave) = LIBERACAO[i] else { continue };
            // Precisa estar apagada há [`LIBERA_S`] — ver a constante para a tensão entre
            // dizer cedo e não dizer por um pisca.
            let Some(desde) = e.apagada_desde_s else { continue };
            if e.acesa_desde_s.is_some() || a.tempo_s - desde < LIBERA_S {
                continue;
            }
            self.pendente = Some(Pendente::Liberacao(i));
            return Some(chave);
        }

        // A mais grave que já confirmou e ainda não falou. `FAMILIAS` está em ordem, então
        // o primeiro que servir é o certo.
        for (i, (_, chave)) in FAMILIAS.iter().enumerate() {
            let e = &self.estados[i];
            if e.avisado {
                continue;
            }
            let Some(desde) = e.acesa_desde_s else { continue };
            if a.tempo_s - desde < CONFIRMA_S {
                continue;
            }
            // NÃO marca como avisado aqui — quem sabe se a fala saiu é o chamador. Ver
            // `ObservadorBandeira::confirmar_aviso` e a doutrina em docs/spotter-frentes.
            self.pendente = Some(Pendente::Bandeira(i));
            return Some(chave);
        }
        None
    }

    /// A chave devolvida pela última [`ObservadorBandeira::observar`] virou fala de fato.
    pub fn confirmar_aviso(&mut self) {
        match self.pendente.take() {
            Some(Pendente::Bandeira(i)) => self.estados[i].avisado = true,
            Some(Pendente::Liberacao(i)) => {
                // Só isto. NÃO mexer em `avisado`: quem decide se a próxima acesa é notícia
                // é o rearme da borda de subida, que mede o vão real. Zerar aqui faria um
                // pisca curto virar TRÊS falas — amarela, verde, amarela —, que é
                // exatamente o defeito que o rearme existe para impedir.
                self.estados[i].liberacao_devida = false;
            }
            None => {}
        }
    }
}

// ─────────────────────────── fachada ───────────────────────────

fn observador() -> &'static Mutex<ObservadorBandeira> {
    static OBS: OnceLock<Mutex<ObservadorBandeira>> = OnceLock::new();
    OBS.get_or_init(|| Mutex::new(ObservadorBandeira::novo()))
}

fn lock() -> MutexGuard<'static, ObservadorBandeira> {
    observador().lock().unwrap_or_else(|e| e.into_inner())
}

/// Alimenta o observador global. Devolve a chave de fala, se houver.
pub fn observar(t: &crate::iracing_sdk::IracingTelemetry) -> Option<&'static str> {
    lock().observar(AmostraBandeira {
        tempo_s: t.session_time,
        estado_sessao: t.session_state,
        bandeiras: t.session_flags as u32,
        jogador_no_carro: t.on_track && !t.is_replay_playing,
    })
}

pub fn confirmar_aviso() {
    lock().confirmar_aviso();
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    /// Um mundo que roda a 60 Hz com as bandeiras que o teste mandar.
    struct Cena {
        obs: ObservadorBandeira,
        t: f64,
        bandeiras: u32,
        estado: i32,
        no_carro: bool,
        confirma: bool,
        chaves: Vec<&'static str>,
    }

    impl Cena {
        fn nova() -> Self {
            Cena {
                obs: ObservadorBandeira::novo(),
                t: 0.0,
                bandeiras: 0,
                estado: ESTADO_CORRIDA,
                no_carro: true,
                confirma: true,
                chaves: Vec::new(),
            }
        }

        fn rodar(&mut self, segundos: f64) {
            let mut restante = segundos;
            while restante > 0.0 {
                let a = AmostraBandeira {
                    tempo_s: self.t,
                    estado_sessao: self.estado,
                    bandeiras: self.bandeiras,
                    jogador_no_carro: self.no_carro,
                };
                if let Some(c) = self.obs.observar(a) {
                    if self.confirma {
                        self.obs.confirmar_aviso();
                    }
                    self.chaves.push(c);
                }
                self.t += DT;
                restante -= DT;
            }
        }

        fn conta(&self, chave: &str) -> usize {
            self.chaves.iter().filter(|c| **c == chave).count()
        }
    }

    #[test]
    fn a_amarela_fala_uma_vez_e_cala_pelos_seis_minutos() {
        // A de Okayama durou 349,9 s. Uma fala; permanência aqui seria alarme de carro.
        let mut c = Cena::nova();
        c.bandeiras = 0x4000;
        c.rodar(350.0);
        assert_eq!(c.conta(CHAVE_AMARELA), 1, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn as_quatro_formas_da_amarela_sao_um_evento_so() {
        // Medido: `CAUTION` e `CAUTION_WAVING` disparam separados para a MESMA amarela, e
        // os bits de nome óbvio (`YELLOW`) não disparam nunca. Lidos bit a bit, seriam três
        // falas para um evento.
        let mut c = Cena::nova();
        c.bandeiras = 0x4000;
        c.rodar(2.0);
        c.bandeiras = 0x4000 | 0x8000;
        c.rodar(2.0);
        c.bandeiras = 0x8000;
        c.rodar(2.0);
        assert_eq!(c.conta(CHAVE_AMARELA), 1, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn o_bit_de_nome_obvio_tambem_serve() {
        // A máscara existe porque `CAUTION` é quem dispara — mas se um dia o sim mandar
        // `YELLOW`, tem de falar igual.
        let mut c = Cena::nova();
        c.bandeiras = 0x8;
        c.rodar(2.0);
        assert_eq!(c.conta(CHAVE_AMARELA), 1);
    }

    #[test]
    fn um_pisca_curto_nao_vira_fala() {
        let mut c = Cena::nova();
        c.bandeiras = 0x4000;
        c.rodar(CONFIRMA_S - 0.1);
        c.bandeiras = 0;
        c.rodar(1.0);
        assert!(c.chaves.is_empty(), "chaves: {:?}", c.chaves);
    }

    #[test]
    fn o_pisca_de_dois_segundos_nao_vira_duas_falas() {
        // O caso real da azul em Okayama: acesa 2,1 s, apagada 1,9 s, acesa 7,5 s. É o
        // mesmo evento partido em dois pelo sim, e sem o rearme sairiam duas falas iguais
        // com dois segundos de intervalo.
        let mut c = Cena::nova();
        c.bandeiras = 0x4000;
        c.rodar(2.1);
        c.bandeiras = 0;
        c.rodar(1.9);
        c.bandeiras = 0x4000;
        c.rodar(7.5);
        assert_eq!(c.conta(CHAVE_AMARELA), 1, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn uma_amarela_de_verdade_depois_do_rearme_fala_de_novo() {
        // O contrapeso do teste acima: duas amarelas separadas por minutos são dois
        // eventos, e emudecer a segunda seria pior que repetir a primeira.
        let mut c = Cena::nova();
        c.bandeiras = 0x4000;
        c.rodar(10.0);
        c.bandeiras = 0;
        c.rodar(REARME_S + 1.0);
        c.bandeiras = 0x4000;
        c.rodar(2.0);
        assert_eq!(c.conta(CHAVE_AMARELA), 2, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn o_reparo_ganha_da_amarela() {
        let mut c = Cena::nova();
        c.bandeiras = 0x4000 | REPARO;
        c.rodar(2.0);
        assert_eq!(c.chaves.first(), Some(&CHAVE_REPARO), "chaves: {:?}", c.chaves);
        // E a amarela sai em seguida: as duas são verdade, e a ordem é que importa.
        assert_eq!(c.conta(CHAVE_AMARELA), 1);
    }

    #[test]
    fn a_amarela_apagando_vira_relargada() {
        // O caso real de Okayama: amarela em t=182,4 s, apaga em t=532,3 s. Uma vez.
        let mut c = Cena::nova();
        c.bandeiras = 0x4000;
        c.rodar(20.0);
        assert_eq!(c.conta(CHAVE_AMARELA), 1);
        c.bandeiras = 0;
        c.rodar(LIBERA_S + 1.0);
        assert_eq!(c.conta(CHAVE_VERDE), 1, "chaves: {:?}", c.chaves);
        // E não repete enquanto a pista segue verde.
        c.rodar(30.0);
        assert_eq!(c.conta(CHAVE_VERDE), 1, "verde repetiu: {:?}", c.chaves);
    }

    #[test]
    fn amarela_que_nunca_falou_nao_tem_verde_a_dizer() {
        // A doutrina do `livre`: a liberação só existe se houve ocupação. Uma amarela que
        // acendeu e apagou dentro do tempo de confirmação nunca foi dita, e anunciar o
        // verde dela seria responder a uma pergunta que ninguém ouviu.
        let mut c = Cena::nova();
        c.bandeiras = 0x4000;
        c.rodar(CONFIRMA_S - 0.1);
        c.bandeiras = 0;
        c.rodar(LIBERA_S + 1.0);
        assert!(c.chaves.is_empty(), "chaves: {:?}", c.chaves);
    }

    #[test]
    fn um_pisca_nao_vira_amarela_verde_amarela() {
        // O defeito que quase entrou: se a liberação zerasse o `avisado`, o pisca curto
        // produziria TRÊS falas. Quem decide se a próxima acesa é notícia é o rearme da
        // borda de subida, e a liberação não pode mexer nele.
        let mut c = Cena::nova();
        c.bandeiras = 0x4000;
        c.rodar(10.0);
        c.bandeiras = 0;
        c.rodar(1.9);
        c.bandeiras = 0x4000;
        c.rodar(10.0);
        assert_eq!(c.conta(CHAVE_AMARELA), 1, "amarela repetiu: {:?}", c.chaves);
        assert_eq!(c.conta(CHAVE_VERDE), 0, "verde saiu num pisca: {:?}", c.chaves);
    }

    #[test]
    fn duas_amarelas_de_verdade_dao_duas_relargadas() {
        // Okayama teve duas amarelas separadas por minutos. Cada uma é um evento.
        let mut c = Cena::nova();
        c.bandeiras = 0x4000;
        c.rodar(10.0);
        c.bandeiras = 0;
        c.rodar(REARME_S + 2.0);
        c.bandeiras = 0x4000;
        c.rodar(10.0);
        c.bandeiras = 0;
        c.rodar(LIBERA_S + 1.0);
        assert_eq!(c.conta(CHAVE_AMARELA), 2, "chaves: {:?}", c.chaves);
        assert_eq!(c.conta(CHAVE_VERDE), 2, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn a_ultima_volta_e_o_reparo_nao_tem_liberacao() {
        // Medido: nenhum dos dois apaga até o fim da prova. E se apagassem, não seria
        // notícia — o piloto que consertou estava lá.
        let mut c = Cena::nova();
        c.bandeiras = REPARO | BRANCA;
        c.rodar(10.0);
        c.bandeiras = 0;
        c.rodar(LIBERA_S + 2.0);
        assert_eq!(c.conta(CHAVE_VERDE), 0, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn a_bandeira_preta_nao_fala() {
        // A preta é o mecanismo de dano do PRÓPRIO Loop — `!black #N <segundos>` do
        // `car::breakdown` simulando quebra de peça. Anunciá-la diria "você foi punido"
        // quando o motor quebrou. Ver o cabeçalho: só volta a existir se alguém resolver
        // essa ambiguidade, e a informação certa é da peça, não da bandeira.
        let mut c = Cena::nova();
        c.bandeiras = 0x0001_0000;
        c.rodar(5.0);
        assert!(c.chaves.is_empty(), "a preta falou: {:?}", c.chaves);
    }

    #[test]
    fn o_reparo_e_a_ultima_volta_sao_estados_que_travam() {
        // Medido: nenhum dos dois DESLIGA até o fim da prova. Uma fala por episódio já dá
        // exatamente uma fala, sem regra especial.
        let mut c = Cena::nova();
        c.bandeiras = REPARO | BRANCA;
        c.rodar(120.0);
        assert_eq!(c.conta(CHAVE_REPARO), 1);
        assert_eq!(c.conta(CHAVE_ULTIMA_VOLTA), 1);
    }

    #[test]
    fn a_bandeira_ja_acesa_na_largada_nao_vira_fala() {
        // Antes da corrida o bitfield já carrega o que a sessão anterior deixou. Anunciar
        // no primeiro tique da prova seria falar de uma bandeira que ninguém viu.
        let mut c = Cena::nova();
        c.estado = 3;
        c.bandeiras = 0x4000;
        c.rodar(10.0);
        assert!(c.chaves.is_empty(), "falou fora de corrida: {:?}", c.chaves);
    }

    #[test]
    fn no_box_o_detector_cala() {
        let mut c = Cena::nova();
        c.no_carro = false;
        c.bandeiras = 0x4000;
        c.rodar(10.0);
        assert!(c.chaves.is_empty(), "chaves: {:?}", c.chaves);
    }

    #[test]
    fn um_aviso_que_nao_virou_fala_volta_no_tick_seguinte() {
        // A doutrina da casa: nunca descartar para preservar cadência, no máximo adiar.
        let mut c = Cena::nova();
        c.confirma = false;
        c.bandeiras = 0x4000;
        c.rodar(2.0);
        assert!(c.conta(CHAVE_AMARELA) > 1, "sem confirmação deveria insistir");
        c.confirma = true;
        c.rodar(0.1);
        let depois = c.conta(CHAVE_AMARELA);
        c.rodar(2.0);
        assert_eq!(c.conta(CHAVE_AMARELA), depois, "confirmou e continuou falando");
    }

    #[test]
    fn um_salto_de_tempo_reinicia_sem_inventar_fala() {
        let mut c = Cena::nova();
        c.bandeiras = 0x4000;
        c.rodar(5.0);
        assert_eq!(c.conta(CHAVE_AMARELA), 1);
        c.t += SALTO_MAX_S + 1.0;
        c.rodar(2.0);
        // Zerou e a bandeira segue acesa: é uma sessão nova, e ela é notícia de novo.
        assert_eq!(c.conta(CHAVE_AMARELA), 2, "chaves: {:?}", c.chaves);
    }
}
