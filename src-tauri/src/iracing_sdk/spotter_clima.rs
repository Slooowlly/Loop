//! Spotter de CLIMA — a pista molhando, molhada, e secando.
//!
//! O menor detector da família, e o mais raro. Não há inferência: o `TrackWetness` vem
//! pronto do SDK numa escala de 1 (seca) a 7 (encharcada), e o app já tem a própria
//! definição de "molhada o bastante para pneu de chuva" em
//! [`crate::iracing_sdk::WET_SURFACE_MIN`]. Este módulo só decide quando vale falar.
//!
//! ## O que o acervo tem
//!
//! **As duas corridas de formato 3 são inteiramente secas** — `track_wetness` constante em
//! 1, `precipitation` em zero, `weather_declared_wet` sempre falso, zero transições em 34
//! minutos. A única progressão real do acervo é a de Ledenon, a corrida em que o humano
//! dirigiu:
//!
//! ```text
//! t= 927 s   wetness 1 → 2   (seca → quase seca)
//! t=1018 s   wetness 2 → 3   (→ levemente molhada)
//! ```
//!
//! Duas transições limpas em 917 s, sem oscilação. É pouca amostra, e é por isso que a
//! histerese abaixo é generosa: contra oscilação **não observada**, não contra uma medida.
//!
//! ## Três falas, sem variação
//!
//! Cada uma sai no máximo uma vez por travessia de limiar, e uma corrida inteira raramente
//! produz mais que duas. Variação existe para o que se repete — aqui não há repetição a
//! quebrar, e três redações seriam três chances de confundir uma informação que muda o
//! pneu que o piloto vai pedir.
//!
//! ## O que fica para o engenheiro
//!
//! A consequência de estratégia — trocar para chuva, quando, quanto se perde — **não é
//! deste módulo**. O spotter diz o que a pista está fazendo agora; o que fazer a respeito
//! é conversa de box, e o engenheiro ainda não existe.

use std::sync::{Mutex, MutexGuard, OnceLock};

pub const CHAVE_MOLHANDO: &str = "pista_molhando";
pub const CHAVE_MOLHADA: &str = "pista_molhada";
pub const CHAVE_SECANDO: &str = "pista_secando";

const ESTADO_CORRIDA: i32 = 4;

/// `TrackWetness == 1` é pista seca. Qualquer coisa acima é umidade que já se sente.
const SECA: i32 = 1;

/// Por quanto tempo o novo nível precisa se sustentar antes de virar fala.
///
/// Três segundos. O canal não oscilou nas duas transições do acervo, mas a amostra é de
/// uma corrida só — e uma fala de clima que pisca é pior que tardia, porque a decisão que
/// ela alimenta (o pneu) não se desfaz. Três segundos custam nada num evento que muda a
/// corrida por minutos.
const CONFIRMA_S: f64 = 3.0;

const SALTO_MAX_S: f64 = 5.0;

pub struct AmostraClima {
    pub tempo_s: f64,
    pub estado_sessao: i32,
    pub umidade: i32,
    pub jogador_no_carro: bool,
}

/// Em que faixa a pista está. É o que o piloto precisa distinguir, e são só três.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Faixa {
    Seca,
    /// Úmida mas ainda de pneu seco.
    Umida,
    /// Molhada o bastante para pneu de chuva — o limiar é do próprio app.
    Molhada,
}

fn faixa(umidade: i32) -> Faixa {
    if umidade >= crate::iracing_sdk::WET_SURFACE_MIN {
        Faixa::Molhada
    } else if umidade > SECA {
        Faixa::Umida
    } else {
        Faixa::Seca
    }
}

#[derive(Debug, Default)]
pub struct ObservadorClima {
    /// A faixa que já foi anunciada (ou a inicial, que nunca é anunciada).
    faixa_dita: Option<Faixa>,
    /// A faixa candidata e desde quando ela se sustenta.
    candidata: Option<(Faixa, f64)>,
    ultimo_tempo_s: Option<f64>,
    pendente: Option<Faixa>,
}

impl ObservadorClima {
    pub fn novo() -> Self {
        Self::default()
    }

    fn zerar(&mut self) {
        self.faixa_dita = None;
        self.candidata = None;
        self.pendente = None;
    }

    pub fn observar(&mut self, a: AmostraClima) -> Option<&'static str> {
        if let Some(ant) = self.ultimo_tempo_s {
            if a.tempo_s < ant || a.tempo_s - ant > SALTO_MAX_S {
                self.zerar();
            }
        }
        self.ultimo_tempo_s = Some(a.tempo_s);
        self.pendente = None;

        if a.estado_sessao != ESTADO_CORRIDA || !a.jogador_no_carro {
            self.zerar();
            return None;
        }

        let agora = faixa(a.umidade);

        // A PRIMEIRA leitura da corrida não é notícia: ela descreve como a prova começou,
        // e o piloto estava lá. Só a MUDANÇA é notícia.
        let Some(dita) = self.faixa_dita else {
            self.faixa_dita = Some(agora);
            return None;
        };
        if agora == dita {
            self.candidata = None;
            return None;
        }

        match self.candidata {
            Some((f, desde)) if f == agora => {
                if a.tempo_s - desde < CONFIRMA_S {
                    return None;
                }
            }
            _ => {
                self.candidata = Some((agora, a.tempo_s));
                return None;
            }
        }

        // Subir de faixa e descer são notícias diferentes: uma manda ter cuidado, a outra
        // devolve a pista. Descer sempre diz "secando", seja de qual faixa for.
        let chave = if agora == Faixa::Seca || (agora == Faixa::Umida && dita == Faixa::Molhada) {
            CHAVE_SECANDO
        } else if agora == Faixa::Molhada {
            CHAVE_MOLHADA
        } else {
            CHAVE_MOLHANDO
        };
        // NÃO confirma aqui — quem sabe se a fala saiu é o chamador.
        self.pendente = Some(agora);
        Some(chave)
    }

    pub fn confirmar_aviso(&mut self) {
        if let Some(f) = self.pendente.take() {
            self.faixa_dita = Some(f);
            self.candidata = None;
        }
    }
}

// ─────────────────────────── fachada ───────────────────────────

fn observador() -> &'static Mutex<ObservadorClima> {
    static OBS: OnceLock<Mutex<ObservadorClima>> = OnceLock::new();
    OBS.get_or_init(|| Mutex::new(ObservadorClima::novo()))
}

fn lock() -> MutexGuard<'static, ObservadorClima> {
    observador().lock().unwrap_or_else(|e| e.into_inner())
}

pub fn observar(t: &crate::iracing_sdk::IracingTelemetry) -> Option<&'static str> {
    lock().observar(AmostraClima {
        tempo_s: t.session_time,
        estado_sessao: t.session_state,
        umidade: t.track_wetness,
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

    struct Cena {
        obs: ObservadorClima,
        t: f64,
        umidade: i32,
        estado: i32,
        no_carro: bool,
        confirma: bool,
        chaves: Vec<&'static str>,
    }

    impl Cena {
        fn nova() -> Self {
            Cena {
                obs: ObservadorClima::novo(),
                t: 0.0,
                umidade: 1,
                estado: ESTADO_CORRIDA,
                no_carro: true,
                confirma: true,
                chaves: Vec::new(),
            }
        }

        fn rodar(&mut self, segundos: f64) {
            let mut restante = segundos;
            while restante > 0.0 {
                let a = AmostraClima {
                    tempo_s: self.t,
                    estado_sessao: self.estado,
                    umidade: self.umidade,
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
    fn a_progressao_de_ledenon_da_uma_fala() {
        // O caso real: 1 → 2 aos 927 s, 2 → 3 aos 1018 s. As duas travessias ficam DENTRO
        // da mesma faixa "úmida", então é um evento e uma fala — não duas.
        let mut c = Cena::nova();
        c.rodar(2.0);
        c.umidade = 2;
        c.rodar(CONFIRMA_S + 1.0);
        c.umidade = 3;
        c.rodar(CONFIRMA_S + 1.0);
        assert_eq!(c.conta(CHAVE_MOLHANDO), 1, "chaves: {:?}", c.chaves);
        assert_eq!(c.conta(CHAVE_MOLHADA), 0);
    }

    #[test]
    fn cruzar_o_limiar_do_app_vira_pista_molhada() {
        // `WET_SURFACE_MIN` é a definição do PRÓPRIO app de "precisa de pneu de chuva".
        // Se ela mudar, esta fala muda junto — é o ponto de ler a constante em vez de 4.
        let mut c = Cena::nova();
        c.rodar(2.0);
        c.umidade = 2;
        c.rodar(CONFIRMA_S + 1.0);
        c.umidade = crate::iracing_sdk::WET_SURFACE_MIN;
        c.rodar(CONFIRMA_S + 1.0);
        assert_eq!(c.conta(CHAVE_MOLHANDO), 1);
        assert_eq!(c.conta(CHAVE_MOLHADA), 1, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn a_pista_secando_e_noticia_de_volta() {
        let mut c = Cena::nova();
        c.umidade = 6;
        c.rodar(2.0);
        c.umidade = 2;
        c.rodar(CONFIRMA_S + 1.0);
        assert_eq!(c.conta(CHAVE_SECANDO), 1, "chaves: {:?}", c.chaves);
        c.umidade = 1;
        c.rodar(CONFIRMA_S + 1.0);
        assert_eq!(c.conta(CHAVE_SECANDO), 2, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn comecar_a_corrida_na_chuva_nao_vira_fala() {
        // A primeira leitura descreve como a prova começou, e o piloto estava lá.
        let mut c = Cena::nova();
        c.umidade = 6;
        c.rodar(30.0);
        assert!(c.chaves.is_empty(), "anunciou o estado inicial: {:?}", c.chaves);
    }

    #[test]
    fn um_pisca_curto_nao_vira_fala() {
        let mut c = Cena::nova();
        c.rodar(2.0);
        c.umidade = 3;
        c.rodar(CONFIRMA_S - 0.5);
        c.umidade = 1;
        c.rodar(10.0);
        assert!(c.chaves.is_empty(), "chaves: {:?}", c.chaves);
    }

    #[test]
    fn oscilar_dentro_da_faixa_nao_repete() {
        // 2 e 3 são a mesma faixa. O canal indo e voltando entre eles não é evento.
        let mut c = Cena::nova();
        c.rodar(2.0);
        c.umidade = 2;
        c.rodar(CONFIRMA_S + 1.0);
        for _ in 0..5 {
            c.umidade = 3;
            c.rodar(CONFIRMA_S + 0.5);
            c.umidade = 2;
            c.rodar(CONFIRMA_S + 0.5);
        }
        assert_eq!(c.conta(CHAVE_MOLHANDO), 1, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn fora_de_corrida_nao_fala() {
        let mut c = Cena::nova();
        c.estado = 3;
        c.rodar(2.0);
        c.umidade = 5;
        c.rodar(30.0);
        assert!(c.chaves.is_empty(), "chaves: {:?}", c.chaves);
    }

    #[test]
    fn um_aviso_que_nao_virou_fala_volta_no_tick_seguinte() {
        let mut c = Cena::nova();
        c.confirma = false;
        c.rodar(2.0);
        c.umidade = 3;
        c.rodar(CONFIRMA_S + 1.0);
        assert!(c.conta(CHAVE_MOLHANDO) > 1, "sem confirmação deveria insistir");
    }

    #[test]
    fn um_salto_de_tempo_reinicia_sem_inventar_fala() {
        let mut c = Cena::nova();
        c.rodar(2.0);
        c.umidade = 3;
        c.rodar(CONFIRMA_S + 1.0);
        let antes = c.chaves.len();
        c.t += SALTO_MAX_S + 1.0;
        c.rodar(CONFIRMA_S + 1.0);
        // Zerou: a faixa atual vira o novo ponto de partida e não é anunciada.
        assert_eq!(c.chaves.len(), antes, "inventou fala após o salto: {:?}", c.chaves);
    }
}
