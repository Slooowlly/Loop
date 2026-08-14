//! Spotter de RETORNO À PISTA — "segura, vem carro" / "pode ir".
//!
//! É a chamada arquetípica do ofício. O piloto rodou ou saiu, está parado ou de ré, sem
//! espelho útil e sem visão do que vem, e precisa decidir se entra. É onde acontecem os
//! acidentes mais feios de uma corrida, e é a única situação em que o spotter não está
//! complementando a visão do piloto — está substituindo.
//!
//! ## O que a medição decidiu
//!
//! Doze retornos reais em duas corridas com 41 carros — **0,15 por piloto**, mediana de
//! 4,7 s. Faixa das famílias raras, como as outras.
//!
//! **Em 11 dos 16 episódios brutos a pista estava limpa no instante do retorno.** É o
//! número que define o desenho: um "pode ir" automático erraria cinco vezes, e errar para
//! esse lado é o erro do `livre` — o piloto entra na frente de alguém porque nós dissemos
//! que dava. Daí a regra abaixo.
//!
//! ## A regra que sai disso
//!
//! **A liberação só existe se houve ocupação.** Sair na grama sem ninguém por perto não
//! rende fala nenhuma: o piloto simplesmente volta, e um "pode ir" ali seria ruído numa
//! situação que não tinha pergunta. O [`CHAVE_PODE_IR`] só sai depois de um
//! [`CHAVE_SEGURA`] — como o `livre` do lateral, que só existe porque houve alguém do lado.
//!
//! ## Por que aqui NÃO se suprime sob amarela
//!
//! O [`crate::iracing_sdk::spotter_tras`] emudece sob amarela de propósito: "deixa passar"
//! é instrução errada quando ninguém pode passar. Aqui é o contrário — sob amarela o campo
//! inteiro desfila devagar, e cortar o trem da caution é justamente o que não se pode
//! fazer. "Segura" fica MAIS certo, não menos.
//!
//! ## O guincho
//!
//! Um carro guinchado some de `cars[]` por dezenas de segundos. Na medição que originou
//! este módulo, os dois carros rebocados de Okayama produziram episódios de 172 e 173 s —
//! o mesmo buraco que o `spotter_frente` já tapou uma vez. Ver [`AUSENCIA_MAX_S`].

use crate::iracing_sdk::spotter_base::{
    adiante, empurrar_com_teto, saltou_desde, spotter_singleton, AUSENCIA_MAX_S, ESTADO_CORRIDA,
    JANELA_VEL_S, MAX_CARROS, SUP_FORA_DA_PISTA, SUP_NA_PISTA,
};

pub const CHAVE_SEGURA: &str = "segura_volta";
pub const CHAVE_PODE_IR: &str = "pode_voltar";

/// Janela do pico de velocidade do próprio carro.
const PICO_JANELA_S: f64 = 10.0;
/// Só conta como retorno quem ESTAVA ANDANDO. Sem isto, os 40 carros parados na largada
/// entram todos em estado de retorno ao mesmo tempo — o falso positivo que o
/// `SessionState` não pega e que já mordeu duas famílias deste sistema.
const PICO_MIN_KMH: f64 = 50.0;

/// Abaixo disto, na pista, o carro está em condição de retorno mesmo sem estar na grama:
/// rodou e ficou atravessado, ou parou e vai sair de novo.
const LENTO_KMH: f64 = 30.0;
/// Fração do próprio pico acima da qual o carro voltou à corrida e o estado fecha.
const RECUPEROU: f64 = 0.60;

/// Até onde olhar para trás, e em quanto tempo o carro de trás chega.
const DIST_MAX_M: f64 = 200.0;
const TTA_MAX_S: f64 = 6.0;
/// Taxa de fechamento mínima. Quem não fecha não chega, e um carro que anda no mesmo ritmo
/// 150 m atrás não é notícia para quem vai entrar.
const FECHAMENTO_MIN_MS: f64 = 3.0;

/// Por quanto tempo a pista precisa ficar limpa antes do "pode ir".
///
/// É a constante mais cara de errar do módulo, e por isso é generosa: a fala que libera é a
/// que faz o piloto entrar. Dois segundos cobrem o carro que ainda não entrou na janela de
/// 200 m mas entra logo.
const LIBERA_S: f64 = 2.0;

/// Intervalo do lembrete enquanto o tráfego não passa.
const LEMBRETE_S: f64 = 2.5;

/// Teto da fila de episódios guardados para calibração.
const MAX_EPISODIOS: usize = 60;

/// Como o estado terminou. Só um deles vira fala.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FimVolta {
    /// A pista limpou e o piloto pode entrar. **O único que fala.**
    Liberou,
    /// O piloto voltou ao ritmo sem esperar — resolveu sozinho, e anunciar agora é
    /// comentar o passado.
    VoltouSozinho,
    /// Foi para o box, sumiu do mundo (guincho) ou a sessão acabou.
    SaiuDeCena,
}

/// Um episódio de retorno, para calibração.
#[derive(Debug, Clone)]
pub struct EpisodioVolta {
    pub inicio_s: f64,
    pub duracao_s: f64,
    /// Quantos carros distintos passaram pela janela durante o estado.
    pub carros_na_janela: usize,
    /// O maior número de carros vindo ao mesmo tempo.
    pub pico_simultaneo: usize,
    pub falas: usize,
    pub fim: Option<FimVolta>,
}

pub struct AmostraVoltar<'a> {
    pub tempo_s: f64,
    pub estado_sessao: i32,
    pub comprimento_m: f64,
    pub jogador_idx: i32,
    pub jogador_na_pista: bool,
    pub carros: &'a [crate::iracing_sdk::CarSnapshot],
}

#[derive(Default, Clone)]
struct Carro {
    hist: Vec<(f64, f64)>,
    vel_kmh: f64,
    pico_kmh: f64,
    visto_em_s: Option<f64>,
}

#[derive(Default)]
struct Aberto {
    inicio_s: f64,
    vistos: Vec<i32>,
    pico_simultaneo: usize,
    falas: usize,
    /// Houve tráfego alguma vez. **Sem isto não há liberação** — ver o cabeçalho.
    ocupou: bool,
    /// Já disse "segura" neste episódio.
    avisou_entrada: bool,
    /// Desde quando a pista está limpa. `None` enquanto há alguém vindo.
    limpa_desde_s: Option<f64>,
    ultima_fala_s: f64,
}

#[derive(Default)]
pub struct ObservadorVoltar {
    carros: Vec<Carro>,
    aberto: Option<Aberto>,
    ultimo_tempo_s: Option<f64>,
    encerrados: Vec<EpisodioVolta>,
    /// A chave devolvida na última chamada, à espera de confirmação.
    pendente: Option<&'static str>,
}

impl ObservadorVoltar {
    pub fn novo() -> Self {
        Self {
            carros: vec![Carro::default(); MAX_CARROS],
            ..Default::default()
        }
    }

    fn zerar(&mut self) {
        self.carros = vec![Carro::default(); MAX_CARROS];
        self.aberto = None;
        self.pendente = None;
    }

    fn fechar(&mut self, agora_s: f64, fim: FimVolta) {
        let Some(a) = self.aberto.take() else { return };
        empurrar_com_teto(
            &mut self.encerrados,
            EpisodioVolta {
                inicio_s: a.inicio_s,
                duracao_s: agora_s - a.inicio_s,
                carros_na_janela: a.vistos.len(),
                pico_simultaneo: a.pico_simultaneo,
                falas: a.falas,
                fim: Some(fim),
            },
            MAX_EPISODIOS,
        );
    }

    pub fn observar(&mut self, a: AmostraVoltar<'_>) -> Option<&'static str> {
        if saltou_desde(self.ultimo_tempo_s, a.tempo_s) {
            self.zerar();
        }
        self.ultimo_tempo_s = Some(a.tempo_s);
        self.pendente = None;

        if a.comprimento_m <= 0.0 {
            return None;
        }

        // Passo 1: velocidade derivada e pico de todo mundo. Roda sempre — o pico é o que
        // separa "rodou" de "nunca andou", e um tique pulado é um buraco nessa memória.
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS {
                continue;
            }
            let carro = &mut self.carros[i];
            carro.visto_em_s = Some(a.tempo_s);
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
            // Pico com decaimento: uma média móvel exata custaria um anel por carro, e a
            // pergunta aqui é só "andou nos últimos segundos?".
            let decaimento = 1.0 - (1.0 / (PICO_JANELA_S * 60.0));
            carro.pico_kmh = (carro.pico_kmh * decaimento).max(carro.vel_kmh);
        }

        // O jogador sumiu de `cars[]`? É assim que o guincho se manifesta — o array encolhe e
        // o índice deixa de existir. Mas ele também some por um punhado de quadros com o carro
        // FORA DA PISTA, que é exatamente a situação que este módulo existe para cobrir: o
        // piloto rodou, está atravessado na grama e o array pisca.
        //
        // Quem decide o fechamento é [`AUSENCIA_MAX_S`], e é ele quem tem que decidir. Enquanto
        // a primeira ausência fechava na hora, um piscar de um quadro destruía o episódio no
        // meio: o estado renascia limpo no quadro seguinte, com `avisou_entrada` em falso, e o
        // "segura" REARMAVA sobre o mesmo tráfego — enquanto o "pode voltar", que depende de
        // `ocupou` e `avisou_entrada` do episódio antigo, morria junto e nunca saía. O piloto
        // ouvia "segura" duas vezes e ficava esperando uma liberação que não vinha.
        //
        // É a mesma ordem do `spotter_tras`: mede a ausência primeiro, e só então procura o
        // jogador. Roda ANTES do resto porque o passo 1 acima já carimbou `visto_em_s` de quem
        // está presente neste tique.
        self.fechar_se_ausente(a.tempo_s, a.jogador_idx);

        let jog = a.carros.iter().find(|c| c.idx == a.jogador_idx);
        let em_corrida = a.estado_sessao == ESTADO_CORRIDA;

        // Ausente neste tique: nada a medir, e o episódio fica de pé. Quem o fecha é a linha
        // acima, quando a ausência passar de [`AUSENCIA_MAX_S`].
        let Some(jog) = jog else {
            return None;
        };
        if !em_corrida || !a.jogador_na_pista || jog.on_pit_road {
            self.fechar(a.tempo_s, FimVolta::SaiuDeCena);
            return None;
        }

        let ji = a.jogador_idx as usize;
        if ji >= MAX_CARROS {
            return None;
        }
        let vel_jog = self.carros[ji].vel_kmh;
        let pico_jog = self.carros[ji].pico_kmh;
        let andou = pico_jog > PICO_MIN_KMH;
        let fora = jog.track_surface == SUP_FORA_DA_PISTA;
        let lento = jog.track_surface == SUP_NA_PISTA && vel_jog < LENTO_KMH;
        let em_retorno = andou && (fora || lento);

        // Passo 2: quem vem por trás, fechando, e chega dentro da janela.
        let mut vindo: Vec<i32> = Vec::new();
        if em_retorno || self.aberto.is_some() {
            for o in a.carros {
                let oi = o.idx as usize;
                if oi >= MAX_CARROS || o.idx == a.jogador_idx {
                    continue;
                }
                if o.on_pit_road || o.track_surface != SUP_NA_PISTA {
                    continue;
                }
                let atras = adiante(o.lap_dist_pct, jog.lap_dist_pct, a.comprimento_m);
                if atras <= 0.0 || atras > DIST_MAX_M {
                    continue;
                }
                let fechamento = (self.carros[oi].vel_kmh - vel_jog) / 3.6;
                if fechamento < FECHAMENTO_MIN_MS {
                    continue;
                }
                if atras / fechamento <= TTA_MAX_S {
                    vindo.push(o.idx);
                }
            }
        }

        // Passo 3: a máquina de estado.
        if em_retorno && self.aberto.is_none() {
            self.aberto = Some(Aberto {
                inicio_s: a.tempo_s,
                ultima_fala_s: f64::NEG_INFINITY,
                ..Default::default()
            });
        }
        let recuperou = jog.track_surface == SUP_NA_PISTA && vel_jog > RECUPEROU * pico_jog;
        if !em_retorno && recuperou {
            // Voltou ao ritmo. Se a pista tinha limpado e ele já ouviu, o estado morre
            // liberado; senão, resolveu sozinho e não há o que dizer.
            let liberou = self
                .aberto
                .as_ref()
                .map(|x| x.ocupou && x.limpa_desde_s.is_some())
                .unwrap_or(false);
            self.fechar(
                a.tempo_s,
                if liberou {
                    FimVolta::Liberou
                } else {
                    FimVolta::VoltouSozinho
                },
            );
            return None;
        }

        let Some(ab) = self.aberto.as_mut() else {
            return None;
        };
        for idx in &vindo {
            if !ab.vistos.contains(idx) {
                ab.vistos.push(*idx);
            }
        }
        ab.pico_simultaneo = ab.pico_simultaneo.max(vindo.len());
        if vindo.is_empty() {
            if ab.limpa_desde_s.is_none() {
                ab.limpa_desde_s = Some(a.tempo_s);
            }
        } else {
            ab.ocupou = true;
            ab.limpa_desde_s = None;
        }

        // Entrada: alguém vindo e ainda não falamos.
        if !vindo.is_empty() && !ab.avisou_entrada {
            self.pendente = Some(CHAVE_SEGURA);
            return Some(CHAVE_SEGURA);
        }
        // Liberação: houve ocupação, e a pista está limpa há tempo bastante.
        if ab.ocupou && ab.avisou_entrada {
            if let Some(desde) = ab.limpa_desde_s {
                if a.tempo_s - desde >= LIBERA_S {
                    self.pendente = Some(CHAVE_PODE_IR);
                    return Some(CHAVE_PODE_IR);
                }
            }
        }
        // Lembrete: só enquanto o tráfego persiste. Um vão é silêncio, não metrônomo.
        if !vindo.is_empty() && ab.avisou_entrada && a.tempo_s - ab.ultima_fala_s >= LEMBRETE_S {
            self.pendente = Some(CHAVE_SEGURA);
            return Some(CHAVE_SEGURA);
        }
        None
    }

    /// A chave devolvida pela última [`ObservadorVoltar::observar`] virou fala de fato.
    pub fn confirmar_aviso(&mut self) {
        let Some(chave) = self.pendente.take() else {
            return;
        };
        let agora = self.ultimo_tempo_s.unwrap_or(0.0);
        let Some(ab) = self.aberto.as_mut() else {
            return;
        };
        ab.falas += 1;
        ab.ultima_fala_s = agora;
        if chave == CHAVE_SEGURA {
            ab.avisou_entrada = true;
        }
        if chave == CHAVE_PODE_IR {
            // Liberou: o estado cumpriu o que tinha a dizer.
            ab.ocupou = false;
        }
    }

    pub fn drenar_encerrados(&mut self) -> Vec<EpisodioVolta> {
        std::mem::take(&mut self.encerrados)
    }

    /// Fecha o estado se o jogador sumiu de `cars[]` por mais que [`AUSENCIA_MAX_S`].
    ///
    /// Chamada de DENTRO de [`ObservadorVoltar::observar`], e não pela fachada. Enquanto ela
    /// rodava por fora, o `observar` já tinha fechado o episódio na primeira ausência e esta
    /// função nunca encontrava nada para fechar — ela existia, era chamada, e não controlava
    /// coisa nenhuma.
    fn fechar_se_ausente(&mut self, agora_s: f64, jogador_idx: i32) {
        let i = jogador_idx as usize;
        if i >= MAX_CARROS {
            return;
        }
        let Some(visto) = self.carros[i].visto_em_s else {
            return;
        };
        if agora_s - visto >= AUSENCIA_MAX_S {
            self.fechar(visto, FimVolta::SaiuDeCena);
        }
    }
}

// ─────────────────────────── fachada ───────────────────────────

spotter_singleton!(ObservadorVoltar, ObservadorVoltar::novo());

/// Alimenta o observador global. O comprimento da pista vem de `spotter_frente`, que é
/// quem o amostrador alimenta — um registro só, para ninguém esquecer de alimentar o outro.
pub fn observar(t: &crate::iracing_sdk::IracingTelemetry) -> Option<&'static str> {
    let (chave, encerrados) = {
        let mut obs = lock();
        let chave = obs.observar(AmostraVoltar {
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
            "spotter_voltar",
            &format!(
                "retorno t={:.1} dur={:.2} carros_na_janela={} pico_simultaneo={} falas={} fim={:?}",
                e.inicio_s, e.duracao_s, e.carros_na_janela, e.pico_simultaneo, e.falas, e.fim
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
    use crate::iracing_sdk::spotter_base::SALTO_MAX_S;
    use crate::iracing_sdk::CarSnapshot;

    const DT: f64 = 1.0 / 60.0;
    const PISTA: f64 = 2400.0;

    fn carro(idx: i32, pct: f64, sup: i32) -> CarSnapshot {
        CarSnapshot {
            idx,
            lap_dist_pct: pct,
            track_surface: sup,
            ..Default::default()
        }
    }

    /// Jogador em 50% da volta; os outros carros atrás dele, andando de verdade.
    struct Cena {
        obs: ObservadorVoltar,
        t: f64,
        jog: f64,
        jog_kmh: f64,
        jog_sup: i32,
        /// (pct, km/h) de cada perseguidor.
        outros: Vec<(f64, f64)>,
        estado: i32,
        na_pista: bool,
        confirma: bool,
        /// O jogador aparece em `cars[]` neste tique? Falso simula o piscar do array — o
        /// mesmo buraco que o guincho abre, só que por alguns quadros.
        jog_presente: bool,
        chaves: Vec<&'static str>,
    }

    impl Cena {
        fn nova() -> Self {
            Cena {
                obs: ObservadorVoltar::novo(),
                t: 0.0,
                jog: 0.50,
                jog_kmh: 180.0,
                jog_sup: SUP_NA_PISTA,
                outros: Vec::new(),
                estado: ESTADO_CORRIDA,
                na_pista: true,
                confirma: true,
                jog_presente: true,
                chaves: Vec::new(),
            }
        }

        /// Um perseguidor a `metros` atrás do jogador, a `kmh`.
        fn atras(&mut self, metros: f64, kmh: f64) {
            self.outros.push((self.jog - metros / PISTA, kmh));
        }

        /// Roda com todo mundo andando. É o ponto que a primeira versão dos testes do
        /// `spotter_frente` errou: carro congelado num `pct` fixo é, por definição do
        /// detector, outra coisa.
        fn rodar(&mut self, segundos: f64) {
            let mut restante = segundos;
            while restante > 0.0 {
                let mut cs = Vec::new();
                if self.jog_presente {
                    cs.push(carro(0, self.jog, self.jog_sup));
                }
                for (i, (pct, _)) in self.outros.iter().enumerate() {
                    cs.push(carro(i as i32 + 1, *pct, SUP_NA_PISTA));
                }
                let a = AmostraVoltar {
                    tempo_s: self.t,
                    estado_sessao: self.estado,
                    comprimento_m: PISTA,
                    jogador_idx: 0,
                    jogador_na_pista: self.na_pista,
                    carros: &cs,
                };
                if let Some(c) = self.obs.observar(a) {
                    if self.confirma {
                        self.obs.confirmar_aviso();
                    }
                    self.chaves.push(c);
                }
                self.jog = (self.jog + (self.jog_kmh / 3.6) * DT / PISTA).fract();
                for (pct, kmh) in self.outros.iter_mut() {
                    *pct = (*pct + (*kmh / 3.6) * DT / PISTA).fract();
                }
                self.t += DT;
                restante -= DT;
            }
        }

        /// Leva todo mundo a ritmo de corrida para encher o histórico de pico.
        fn aquecer(&mut self) {
            self.rodar(2.0);
        }

        fn conta(&self, chave: &str) -> usize {
            self.chaves.iter().filter(|c| **c == chave).count()
        }
    }

    #[test]
    fn sair_com_gente_vindo_manda_segurar() {
        let mut c = Cena::nova();
        c.atras(150.0, 180.0);
        c.aquecer();
        c.jog_sup = SUP_FORA_DA_PISTA;
        c.jog_kmh = 30.0;
        c.rodar(1.0);
        assert_eq!(
            c.chaves.first(),
            Some(&CHAVE_SEGURA),
            "chaves: {:?}",
            c.chaves
        );
    }

    #[test]
    fn sair_sem_ninguem_por_perto_e_silencio() {
        // 2 de 5 episódios de Lime Rock foram assim. O piloto simplesmente volta, e um
        // "pode ir" ali seria resposta a uma pergunta que ninguém fez.
        let mut c = Cena::nova();
        c.aquecer();
        c.jog_sup = SUP_FORA_DA_PISTA;
        c.jog_kmh = 30.0;
        c.rodar(6.0);
        assert!(c.chaves.is_empty(), "falou sem tráfego: {:?}", c.chaves);
    }

    #[test]
    fn o_pode_ir_so_vem_depois_de_um_segura() {
        // A doutrina do `livre`: a liberação só existe se houve ocupação. Esta é a fala que
        // faz o piloto ENTRAR, e uma liberação sem ocupação prévia é ela sozinha no ar.
        let mut c = Cena::nova();
        c.atras(120.0, 220.0);
        c.aquecer();
        c.jog_sup = SUP_FORA_DA_PISTA;
        c.jog_kmh = 20.0;
        c.rodar(1.0);
        assert_eq!(c.conta(CHAVE_SEGURA), 1);
        // O perseguidor passa e some da janela de trás.
        c.rodar(LIBERA_S + 2.0);
        assert!(c.conta(CHAVE_PODE_IR) >= 1, "não liberou: {:?}", c.chaves);
        let i_seg = c.chaves.iter().position(|k| *k == CHAVE_SEGURA).unwrap();
        let i_ir = c.chaves.iter().position(|k| *k == CHAVE_PODE_IR).unwrap();
        assert!(i_seg < i_ir, "liberou antes de ocupar: {:?}", c.chaves);
    }

    #[test]
    fn um_piscar_de_cars_nao_rearma_o_segura_nem_perde_o_pode_voltar() {
        // O jogador some de `cars[]` por alguns quadros — é o que o array faz com o carro
        // fora da pista, que é a situação inteira deste módulo. Enquanto a primeira ausência
        // fechava o episódio, o quadro seguinte abria um limpo: o "segura" saía de novo sobre
        // o mesmo tráfego e o "pode voltar", que depende do episódio antigo ter ocupado e
        // avisado, nunca chegava. O piloto ouvia o alerta duas vezes e esperava para sempre a
        // liberação.
        let mut c = Cena::nova();
        c.atras(120.0, 220.0);
        c.aquecer();
        c.jog_sup = SUP_FORA_DA_PISTA;
        c.jog_kmh = 20.0;
        c.rodar(1.0);
        assert_eq!(c.conta(CHAVE_SEGURA), 1, "chaves: {:?}", c.chaves);

        // O piscar, abaixo do teto de ausência.
        c.jog_presente = false;
        c.rodar(AUSENCIA_MAX_S * 0.5);
        c.jog_presente = true;

        // O perseguidor passa e a janela limpa.
        c.rodar(LIBERA_S + 2.0);

        assert_eq!(
            c.conta(CHAVE_SEGURA),
            1,
            "o segura rearmou depois do piscar: {:?}",
            c.chaves
        );
        assert!(
            c.conta(CHAVE_PODE_IR) >= 1,
            "perdeu o pode voltar: {:?}",
            c.chaves
        );
    }

    #[test]
    fn a_ausencia_longa_fecha_o_estado_uma_vez_so() {
        // O outro lado da mesma moeda: guincho é ausência que não volta, e aí o episódio tem
        // de morrer — senão fica aberto para sempre, falando sobre um carro que saiu do mundo.
        // Quem decide é `AUSENCIA_MAX_S`, e é ele que tem de decidir nos dois sentidos.
        let mut c = Cena::nova();
        c.atras(120.0, 220.0);
        c.aquecer();
        c.jog_sup = SUP_FORA_DA_PISTA;
        c.jog_kmh = 20.0;
        c.rodar(1.0);
        assert_eq!(c.conta(CHAVE_SEGURA), 1, "chaves: {:?}", c.chaves);
        c.obs.drenar_encerrados();

        c.jog_presente = false;
        c.rodar(AUSENCIA_MAX_S + 0.5);

        let fins: Vec<_> = c
            .obs
            .drenar_encerrados()
            .into_iter()
            .map(|e| e.fim)
            .collect();
        assert_eq!(
            fins,
            vec![Some(FimVolta::SaiuDeCena)],
            "a ausência longa devia fechar exatamente um episódio"
        );
    }

    #[test]
    fn quinze_carros_nao_viram_quinze_avisos() {
        // Sob amarela em Okayama, até 15 carros entraram na janela ao mesmo tempo. É um
        // estado, não uma chamada por carro.
        let mut c = Cena::nova();
        for k in 0..15 {
            c.atras(60.0 + k as f64 * 10.0, 180.0);
        }
        c.aquecer();
        c.jog_sup = SUP_FORA_DA_PISTA;
        c.jog_kmh = 20.0;
        c.rodar(2.0);
        assert_eq!(c.conta(CHAVE_SEGURA), 1, "chaves: {:?}", c.chaves);
    }

    #[test]
    fn o_lembrete_sai_enquanto_o_trafego_dura() {
        let mut c = Cena::nova();
        for k in 0..12 {
            c.atras(40.0 + k as f64 * 12.0, 180.0);
        }
        c.aquecer();
        c.jog_sup = SUP_FORA_DA_PISTA;
        c.jog_kmh = 10.0;
        c.rodar(LEMBRETE_S * 2.0 + 0.5);
        assert!(c.conta(CHAVE_SEGURA) >= 2, "sem lembrete: {:?}", c.chaves);
    }

    #[test]
    fn a_largada_parada_nao_vira_estado_de_retorno() {
        // 40 carros a 0 km/h no asfalto com a sessão já em corrida. A regra "estava
        // andando" é o que resolve, e ela já mordeu duas outras famílias deste sistema.
        let mut c = Cena::nova();
        c.jog_kmh = 0.0;
        c.atras(80.0, 0.0);
        c.rodar(5.0);
        assert!(c.chaves.is_empty(), "a largada falou: {:?}", c.chaves);
    }

    #[test]
    fn parado_na_pista_tambem_e_retorno() {
        // Rodou e ficou atravessado no asfalto: não está na grama, e precisa do aviso
        // tanto quanto quem está.
        let mut c = Cena::nova();
        c.atras(150.0, 180.0);
        c.aquecer();
        c.jog_kmh = 10.0;
        c.rodar(1.0);
        assert_eq!(
            c.chaves.first(),
            Some(&CHAVE_SEGURA),
            "chaves: {:?}",
            c.chaves
        );
    }

    #[test]
    fn voltar_ao_ritmo_sozinho_nao_gera_liberacao() {
        let mut c = Cena::nova();
        c.aquecer();
        c.jog_sup = SUP_FORA_DA_PISTA;
        c.jog_kmh = 30.0;
        c.rodar(1.0);
        c.jog_sup = SUP_NA_PISTA;
        c.jog_kmh = 180.0;
        c.rodar(2.0);
        assert!(
            c.conta(CHAVE_PODE_IR) == 0,
            "liberou sem ocupação: {:?}",
            c.chaves
        );
    }

    #[test]
    fn quem_nao_fecha_nao_conta() {
        // Um carro 150 m atrás no mesmo ritmo do jogador nunca chega. Contá-lo faria o
        // "segura" durar para sempre e o "pode ir" nunca sair.
        let mut c = Cena::nova();
        c.atras(150.0, 30.0);
        c.aquecer();
        c.jog_sup = SUP_FORA_DA_PISTA;
        c.jog_kmh = 30.0;
        c.rodar(3.0);
        assert!(c.chaves.is_empty(), "contou quem não fecha: {:?}", c.chaves);
    }

    #[test]
    fn um_aviso_que_nao_virou_fala_volta_no_tick_seguinte() {
        let mut c = Cena::nova();
        c.atras(150.0, 180.0);
        c.aquecer();
        c.confirma = false;
        c.jog_sup = SUP_FORA_DA_PISTA;
        c.jog_kmh = 20.0;
        c.rodar(0.5);
        assert!(
            c.conta(CHAVE_SEGURA) > 1,
            "sem confirmação deveria insistir"
        );
    }

    #[test]
    fn um_salto_de_tempo_reinicia_sem_inventar_estado() {
        let mut c = Cena::nova();
        c.atras(150.0, 180.0);
        c.aquecer();
        c.jog_sup = SUP_FORA_DA_PISTA;
        c.jog_kmh = 20.0;
        c.rodar(1.0);
        let antes = c.chaves.len();
        c.t += SALTO_MAX_S + 1.0;
        c.rodar(0.5);
        // Zerou: sem pico, ninguém "estava andando", e nada é dito até o mundo reencher.
        assert_eq!(
            c.chaves.len(),
            antes,
            "inventou fala após o salto: {:?}",
            c.chaves
        );
    }
}
