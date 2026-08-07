//! Spotter de SAÍDA DE BOX — carro entrando na pista devagar, à sua frente.
//!
//! O carro que acaba de deixar a via de box entra na trajetória a 60 ou 90 km/h enquanto o
//! resto passa a 200, e sem espelho útil para quem chega. É a situação de "carro lento à
//! frente" com uma diferença que muda tudo: aqui a causa é uma **transição de estado**, não
//! um limiar num contínuo.
//!
//! Isso importa porque a família `spotter_lento` foi construída, calibrada e **engavetada**
//! justamente por ser um limiar num contínuo: ela não disparou uma única vez em corrida
//! verde e 100% dos avisos dela saíram sob amarela. Esta é o subconjunto da mesma situação
//! que sobrevive à prova — em Lime Rock, uma corrida **sem bandeira nenhuma**, ela dispara
//! 19 vezes.
//!
//! ## Medido nas três capturas
//!
//! | | entradas | por piloto | máximo por piloto |
//! |---|---|---|---|
//! | Lime Rock (17 min, 41 carros, sem bandeira) | 19 | 0,46 | 2 |
//! | Okayama (17 min, 41 carros) | 23 | 0,56 | 2 |
//! | Ledenon (12 carros) | 0 | — | — |
//!
//! Faixa da família `fora` (0,38 a 0,63) e nenhum piloto ouvindo mais que duas vezes.
//!
//! ## Por que é um ESTADO e não um aviso por carro
//!
//! A IA vai aos boxes **em bloco**. Lido carro a carro, o detector dá 34 avisos em Lime
//! Rock com **16 deles numa só janela de 30 segundos**, e um piloto ouvindo **7 vezes**. É
//! a mesma metralhadora que definiu o desenho do [`crate::iracing_sdk::spotter_tras`], pelo
//! mesmo motivo: muitos alvos, um evento. Colapsado em estado, cai para 19 e nenhum piloto
//! passa de duas.
//!
//! ## Sem lembrete, e isso é medido
//!
//! Nas duas corridas o estado **nunca** durou o bastante para um lembrete sair. Um carro
//! saindo do box é ultrapassado depressa; a situação se resolve sozinha em segundos. Um
//! lembrete aqui seria mecanismo sem caso — e mecanismo sem caso é bug esperando data.
//!
//! ## `on_pit_road` não basta
//!
//! Medido em Okayama pela frente que escreveu o `spotter_tras`: um carro passou **20 s** com
//! `on_pit_road == false` e `track_surface == ApproachingPits`. Quem detectar a saída pelo
//! campo de nome óbvio perde a janela inteira. Daí [`dentro_do_box`] olhar os três sinais.

use std::sync::{Mutex, MutexGuard, OnceLock};

pub const CHAVE_SAINDO_BOX: &str = "carro_saindo_box";

const SUP_NA_CAIXA: i32 = 1;
const SUP_ENTRANDO_BOX: i32 = 2;
const SUP_NA_PISTA: i32 = 3;
const ESTADO_CORRIDA: i32 = 4;

/// Janela da velocidade derivada. A mesma dos irmãos — não existe `CarIdxSpeed`.
const JANELA_VEL_S: f64 = 0.25;

/// Por quanto tempo depois de deixar a via de box o carro ainda conta como "saindo".
///
/// Medido: a mediana do tempo entre a saída e o aviso é de 2,1 a 2,7 s, e o máximo do
/// acervo é 14,3 s. Quinze segundos cobre a cauda sem transformar um carro já relançado em
/// obstáculo perpétuo.
const SAIDA_JANELA_S: f64 = 15.0;

/// A faixa de aviso, herdada do detector de obstáculo e não reinventada.
const TTA_MAX_S: f64 = 5.0;
const DIST_MAX_M: f64 = 200.0;
/// Piso de distância. Com fechamento lento, 2 s podem ser 15 m — e a 15 m não há aviso a
/// dar, só susto. É a mesma correção que a família `lento` precisou e que o detector de
/// obstáculo não precisa, porque lá o alvo está parado.
const DIST_MIN_M: f64 = 40.0;
/// Taxa de fechamento mínima. Quem não fecha não chega.
const FECHAMENTO_MIN_MS: f64 = 3.0;

/// Diferença de velocidade mínima para o carro ser NOTÍCIA.
///
/// Sem ela o detector fala de dois carros a 67 e 65 km/h separados por 3 m — um trem sob
/// amarela, não um perigo. A varredura mostrou 0,88/piloto sem piso, 0,83 a 70 km/h em
/// Lime Rock e 1,24 → 0,51 em Okayama: é o piso que corta o trem sem tocar no caso real,
/// onde a diferença mediana é de 91 km/h.
const DIF_MIN_KMH: f64 = 70.0;

/// Quanto tempo sem nenhum carro na janela antes de o estado poder abrir de novo. Impede
/// que a mesma leva de saídas produza duas falas seguidas.
const LIBERA_S: f64 = 5.0;

const SALTO_MAX_S: f64 = 5.0;
const MAX_CARROS: usize = 64;

pub struct AmostraBoxe<'a> {
    pub tempo_s: f64,
    pub estado_sessao: i32,
    pub comprimento_m: f64,
    pub jogador_idx: i32,
    pub jogador_na_pista: bool,
    pub carros: &'a [crate::iracing_sdk::CarSnapshot],
}

/// Um carro está na via de box em qualquer um dos três sinais — ver o cabeçalho sobre o
/// `on_pit_road` sozinho não bastar.
fn dentro_do_box(c: &crate::iracing_sdk::CarSnapshot) -> bool {
    c.on_pit_road || c.track_surface == SUP_NA_CAIXA || c.track_surface == SUP_ENTRANDO_BOX
}

#[derive(Default, Clone)]
struct Carro {
    hist: Vec<(f64, f64)>,
    vel_kmh: f64,
    no_box: bool,
    /// Quando deixou a via de box, se foi nos últimos [`SAIDA_JANELA_S`].
    saiu_em_s: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct EpisodioBoxe {
    pub inicio_s: f64,
    pub alvo_idx: i32,
    pub distancia_m: f64,
    pub tta_s: f64,
    pub diferenca_kmh: f64,
    /// Segundos entre o alvo deixar o box e o aviso sair.
    pub desde_a_saida_s: f64,
}

#[derive(Default)]
pub struct ObservadorBoxe {
    carros: Vec<Carro>,
    ultimo_tempo_s: Option<f64>,
    /// O estado está aberto: já falamos e ainda há (ou houve há pouco) alvo na janela.
    aberto: bool,
    limpa_desde_s: Option<f64>,
    pendente: Option<EpisodioBoxe>,
    encerrados: Vec<EpisodioBoxe>,
}

fn adiante(de_pct: f64, para_pct: f64, comprimento_m: f64) -> f64 {
    let mut d = para_pct - de_pct;
    if d < 0.0 {
        d += 1.0;
    }
    d * comprimento_m
}

impl ObservadorBoxe {
    pub fn novo() -> Self {
        Self {
            carros: vec![Carro::default(); MAX_CARROS],
            ..Default::default()
        }
    }

    fn zerar(&mut self) {
        self.carros = vec![Carro::default(); MAX_CARROS];
        self.aberto = false;
        self.limpa_desde_s = None;
        self.pendente = None;
    }

    pub fn observar(&mut self, a: AmostraBoxe<'_>) -> Option<&'static str> {
        if let Some(ant) = self.ultimo_tempo_s {
            if a.tempo_s < ant || a.tempo_s - ant > SALTO_MAX_S {
                self.zerar();
            }
        }
        self.ultimo_tempo_s = Some(a.tempo_s);
        self.pendente = None;
        if a.comprimento_m <= 0.0 {
            return None;
        }

        // Passo 1: velocidade e transição de box de todo mundo. Roda SEMPRE — a borda de
        // saída acontece num quadro só, e perdê-la é perder a janela inteira.
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS {
                continue;
            }
            let carro = &mut self.carros[i];
            carro.hist.push((a.tempo_s, c.lap_dist_pct));
            while carro.hist.len() > 2 && a.tempo_s - carro.hist[0].0 > JANELA_VEL_S {
                carro.hist.remove(0);
            }
            if carro.hist.len() > 1 {
                let (t0, p0) = carro.hist[0];
                let (t1, p1) = *carro.hist.last().unwrap();
                let dt = t1 - t0;
                if dt > 0.05 {
                    let mut d = p1 - p0;
                    if d < -0.5 {
                        d += 1.0;
                    }
                    if d > 0.5 {
                        d -= 1.0;
                    }
                    carro.vel_kmh = (d * a.comprimento_m / dt) * 3.6;
                }
            }
            let dentro = dentro_do_box(c);
            if carro.no_box && !dentro && c.track_surface == SUP_NA_PISTA {
                carro.saiu_em_s = Some(a.tempo_s);
            }
            carro.no_box = dentro;
        }

        if a.estado_sessao != ESTADO_CORRIDA || !a.jogador_na_pista {
            self.aberto = false;
            self.limpa_desde_s = None;
            return None;
        }
        let Some(jog) = a.carros.iter().find(|c| c.idx == a.jogador_idx) else {
            return None;
        };
        if jog.on_pit_road || jog.track_surface != SUP_NA_PISTA {
            return None;
        }
        let ji = a.jogador_idx as usize;
        if ji >= MAX_CARROS {
            return None;
        }
        let vel_jog = self.carros[ji].vel_kmh;

        // Passo 2: o alvo MAIS PRÓXIMO que acabou de sair do box e está na janela.
        let mut alvo: Option<EpisodioBoxe> = None;
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS || c.idx == a.jogador_idx || c.track_surface != SUP_NA_PISTA {
                continue;
            }
            let Some(saiu) = self.carros[i].saiu_em_s else { continue };
            if a.tempo_s - saiu > SAIDA_JANELA_S {
                continue;
            }
            let d = adiante(jog.lap_dist_pct, c.lap_dist_pct, a.comprimento_m);
            if !(DIST_MIN_M..=DIST_MAX_M).contains(&d) {
                continue;
            }
            let dif = vel_jog - self.carros[i].vel_kmh;
            if dif < DIF_MIN_KMH {
                continue;
            }
            let fechamento = dif / 3.6;
            if fechamento < FECHAMENTO_MIN_MS {
                continue;
            }
            let tta = d / fechamento;
            if tta > TTA_MAX_S {
                continue;
            }
            if alvo.as_ref().map(|x| d < x.distancia_m).unwrap_or(true) {
                alvo = Some(EpisodioBoxe {
                    inicio_s: a.tempo_s,
                    alvo_idx: c.idx,
                    distancia_m: d,
                    tta_s: tta,
                    diferenca_kmh: dif,
                    desde_a_saida_s: a.tempo_s - saiu,
                });
            }
        }

        // Passo 3: o estado. Ele existe SÓ para colapsar a rajada — a IA vai aos boxes em
        // bloco, e sem isto um piloto ouve sete vezes em trinta segundos.
        match alvo {
            Some(ep) => {
                self.limpa_desde_s = None;
                if self.aberto {
                    return None;
                }
                // NÃO marca como aberto aqui: quem sabe se a fala saiu é o chamador.
                self.pendente = Some(ep);
                Some(CHAVE_SAINDO_BOX)
            }
            None => {
                if self.aberto {
                    match self.limpa_desde_s {
                        None => self.limpa_desde_s = Some(a.tempo_s),
                        Some(t) if a.tempo_s - t >= LIBERA_S => {
                            self.aberto = false;
                            self.limpa_desde_s = None;
                        }
                        _ => {}
                    }
                }
                None
            }
        }
    }

    /// A chave devolvida pela última [`ObservadorBoxe::observar`] virou fala de fato.
    pub fn confirmar_aviso(&mut self) {
        let Some(ep) = self.pendente.take() else { return };
        self.aberto = true;
        self.encerrados.push(ep);
        if self.encerrados.len() > 60 {
            self.encerrados.remove(0);
        }
    }

    pub fn drenar_encerrados(&mut self) -> Vec<EpisodioBoxe> {
        std::mem::take(&mut self.encerrados)
    }
}

// ─────────────────────────── fachada ───────────────────────────

fn observador() -> &'static Mutex<ObservadorBoxe> {
    static OBS: OnceLock<Mutex<ObservadorBoxe>> = OnceLock::new();
    OBS.get_or_init(|| Mutex::new(ObservadorBoxe::novo()))
}

fn lock() -> MutexGuard<'static, ObservadorBoxe> {
    observador().lock().unwrap_or_else(|e| e.into_inner())
}

pub fn observar(t: &crate::iracing_sdk::IracingTelemetry) -> Option<&'static str> {
    let (chave, encerrados) = {
        let mut obs = lock();
        let chave = obs.observar(AmostraBoxe {
            tempo_s: t.session_time,
            estado_sessao: t.session_state,
            comprimento_m: crate::iracing_sdk::spotter_frente::comprimento_m(),
            jogador_idx: t.player_car_idx,
            jogador_na_pista: t.on_track && !t.is_replay_playing,
            carros: &t.cars,
        });
        (chave, obs.drenar_encerrados())
    };
    for e in encerrados {
        crate::diagnostico::linha(
            "spotter_boxe",
            &format!(
                "saida t={:.1} alvo={} dist={:.0} tta={:.1} dif={:.0} desde_saida={:.1}",
                e.inicio_s, e.alvo_idx, e.distancia_m, e.tta_s, e.diferenca_kmh, e.desde_a_saida_s
            ),
        );
    }
    chave
}

pub fn confirmar_aviso() {
    lock().confirmar_aviso();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iracing_sdk::CarSnapshot;

    const DT: f64 = 1.0 / 60.0;
    const PISTA: f64 = 2400.0;

    fn carro(idx: i32, pct: f64, sup: i32, box_road: bool) -> CarSnapshot {
        CarSnapshot {
            idx,
            lap_dist_pct: pct,
            track_surface: sup,
            on_pit_road: box_road,
            ..Default::default()
        }
    }

    struct Cena {
        obs: ObservadorBoxe,
        t: f64,
        jog: f64,
        jog_kmh: f64,
        /// (pct, km/h, superfície, on_pit_road)
        alvos: Vec<(f64, f64, i32, bool)>,
        estado: i32,
        confirma: bool,
        chaves: Vec<&'static str>,
    }

    impl Cena {
        /// Jogador a 200 km/h; alvos declarados pelo teste.
        fn nova() -> Self {
            Cena {
                obs: ObservadorBoxe::novo(),
                t: 0.0,
                jog: 0.10,
                jog_kmh: 200.0,
                alvos: Vec::new(),
                estado: ESTADO_CORRIDA,
                confirma: true,
                chaves: Vec::new(),
            }
        }

        /// Um carro `metros` à frente, a `kmh`, ainda na via de box.
        fn no_box_a_frente(&mut self, metros: f64, kmh: f64) {
            self.alvos.push((self.jog + metros / PISTA, kmh, SUP_ENTRANDO_BOX, true));
        }

        /// Todos os alvos deixam a via de box neste instante.
        fn saem_do_box(&mut self) {
            for a in self.alvos.iter_mut() {
                a.2 = SUP_NA_PISTA;
                a.3 = false;
            }
        }

        fn rodar(&mut self, segundos: f64) {
            let mut restante = segundos;
            while restante > 0.0 {
                let mut cs = vec![carro(0, self.jog, SUP_NA_PISTA, false)];
                for (i, (pct, _, sup, box_road)) in self.alvos.iter().enumerate() {
                    cs.push(carro(i as i32 + 1, *pct, *sup, *box_road));
                }
                let a = AmostraBoxe {
                    tempo_s: self.t,
                    estado_sessao: self.estado,
                    comprimento_m: PISTA,
                    jogador_idx: 0,
                    jogador_na_pista: true,
                    carros: &cs,
                };
                if let Some(c) = self.obs.observar(a) {
                    if self.confirma {
                        self.obs.confirmar_aviso();
                    }
                    self.chaves.push(c);
                }
                self.jog = (self.jog + (self.jog_kmh / 3.6) * DT / PISTA).fract();
                for a in self.alvos.iter_mut() {
                    a.0 = (a.0 + (a.1 / 3.6) * DT / PISTA).fract();
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
    fn carro_saindo_do_box_a_frente_vira_aviso() {
        let mut c = Cena::nova();
        c.no_box_a_frente(150.0, 90.0);
        c.rodar(0.5);
        c.saem_do_box();
        c.rodar(1.0);
        assert_eq!(c.conta(CHAVE_SAINDO_BOX), 1, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn quem_nunca_esteve_no_box_nao_conta() {
        // Um carro lento à frente que não veio dos boxes é assunto da família `lento`,
        // engavetada. Esta família é sobre a TRANSIÇÃO, e é isso que a torna defensável.
        let mut c = Cena::nova();
        c.alvos.push((c.jog + 150.0 / PISTA, 90.0, SUP_NA_PISTA, false));
        c.rodar(3.0);
        assert!(c.chaves.is_empty(), "falou sem saída de box: {:?}", c.chaves);
    }

    #[test]
    fn a_leva_inteira_de_saidas_da_uma_fala_so() {
        // A armadilha da família: a IA vai aos boxes EM BLOCO. Carro a carro, um piloto
        // ouvia sete vezes em trinta segundos nos dados reais.
        let mut c = Cena::nova();
        for k in 0..8 {
            c.no_box_a_frente(60.0 + k as f64 * 18.0, 90.0);
        }
        c.rodar(0.5);
        c.saem_do_box();
        c.rodar(2.0);
        assert_eq!(c.conta(CHAVE_SAINDO_BOX), 1, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn o_trem_lento_nao_vira_aviso() {
        // Dois carros a 67 e 65 km/h separados por poucos metros: sob amarela isso é o
        // campo inteiro, e sem o piso de diferença o detector falaria de todos eles.
        let mut c = Cena::nova();
        c.jog_kmh = 67.0;
        c.no_box_a_frente(120.0, 65.0);
        c.rodar(0.5);
        c.saem_do_box();
        c.rodar(3.0);
        assert!(c.chaves.is_empty(), "falou do trem: {:?}", c.chaves);
    }

    #[test]
    fn colado_demais_nao_e_avisavel() {
        // Com fechamento lento, 2 s podem ser 15 m — e a 15 m não há aviso a dar.
        let mut c = Cena::nova();
        c.no_box_a_frente(20.0, 90.0);
        c.rodar(0.5);
        c.saem_do_box();
        c.rodar(1.0);
        assert!(c.chaves.is_empty(), "avisou colado: {:?}", c.chaves);
    }

    #[test]
    fn longe_demais_ainda_nao_e_aviso() {
        let mut c = Cena::nova();
        c.no_box_a_frente(400.0, 90.0);
        c.rodar(0.5);
        c.saem_do_box();
        c.rodar(0.5);
        assert!(c.chaves.is_empty(), "avisou a 400 m: {:?}", c.chaves);
    }

    #[test]
    fn depois_da_janela_o_carro_deixa_de_ser_novidade() {
        let mut c = Cena::nova();
        c.no_box_a_frente(3000.0, 190.0);
        c.rodar(0.5);
        c.saem_do_box();
        // Passa a janela inteira com o alvo fora de alcance, e só então ele fica lento.
        c.rodar(SAIDA_JANELA_S + 1.0);
        c.alvos[0] = (c.jog + 150.0 / PISTA, 90.0, SUP_NA_PISTA, false);
        c.rodar(1.0);
        assert!(c.chaves.is_empty(), "avisou fora da janela: {:?}", c.chaves);
    }

    #[test]
    fn fora_de_corrida_nao_fala() {
        let mut c = Cena::nova();
        c.estado = 3;
        c.no_box_a_frente(150.0, 90.0);
        c.rodar(0.5);
        c.saem_do_box();
        c.rodar(2.0);
        assert!(c.chaves.is_empty(), "chaves: {:?}", c.chaves);
    }

    #[test]
    fn um_aviso_que_nao_virou_fala_volta_no_tick_seguinte() {
        let mut c = Cena::nova();
        c.confirma = false;
        c.no_box_a_frente(150.0, 90.0);
        c.rodar(0.5);
        c.saem_do_box();
        c.rodar(0.5);
        assert!(c.conta(CHAVE_SAINDO_BOX) > 1, "sem confirmação deveria insistir");
    }

    #[test]
    fn um_salto_de_tempo_reinicia_sem_inventar_aviso() {
        let mut c = Cena::nova();
        c.no_box_a_frente(150.0, 90.0);
        c.rodar(0.5);
        c.saem_do_box();
        c.rodar(0.5);
        let antes = c.chaves.len();
        c.t += SALTO_MAX_S + 1.0;
        c.rodar(0.5);
        // Zerou: a borda de saída se perdeu com a máquina, e ninguém "acabou de sair".
        assert_eq!(c.chaves.len(), antes, "inventou fala após o salto: {:?}", c.chaves);
    }
}
