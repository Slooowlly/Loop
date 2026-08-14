//! Spotter — obstáculo à FRENTE.
//!
//! O irmão de [`crate::iracing_sdk::spotter`], que cuida do que está do LADO. Aqui o
//! canal não é um valor pronto: o SDK não diz "há um carro fora da pista à sua frente".
//! Diz onde cada carro está na volta (`CarIdxLapDistPct`) e em que superfície pisa
//! (`CarIdxTrackSurface`), e o obstáculo tem de ser deduzido daí.
//!
//! Todo número deste módulo saiu de UMA corrida gravada — Lime Rock, 40 carros de IA,
//! 17 minutos, analisada em [`docs/spotter-obstaculo.md`]. Nenhum foi escolhido por
//! parecer razoável, e nenhum deve ser mexido sem repetir a medição, porque foram
//! exatamente eles que separaram os dois obstáculos reais dos dois cortes de grama a
//! 200 km/h. As três armadilhas que a corrida revelou:
//!
//! 1. **`OffTrack` puro é limite de pista.** Metade dos episódios acima de 1 s eram
//!    carros passando por cima da grama na saída da curva, a 167 e 202 km/h. Acontece
//!    toda volta e não é obstáculo nenhum. Só vira obstáculo quem também PERDEU ritmo.
//! 2. **`SessionState == Correndo` não protege da largada parada.** No instante do
//!    verde os 40 carros estão a 0 km/h, em asfalto, na pista, com o estado já em
//!    corrida. Quem resolve é a regra "estava andando": parado só conta se o carro
//!    passou de 50 km/h nos últimos 10 s. Quem nunca andou é grid.
//! 3. **Avisar cedo demais é pior que não avisar.** Os obstáculos duraram 4 e 9 s.
//!    Anunciar a 400 m (≈12 s) é anunciar algo que já terá saído da frente — o piloto
//!    freia por nada. A faixa útil medida é 100–200 m, 2 a 5 segundos.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use crate::iracing_sdk::spotter_base::{
    adiante, saltou, spotter_singleton, AUSENCIA_MAX_S, ESTADO_CORRIDA, JANELA_VEL_S, MAX_CARROS,
    SUP_ENTRANDO_BOX, SUP_FORA_DA_PISTA, SUP_FORA_DO_MUNDO, SUP_NA_CAIXA, SUP_NA_PISTA,
};
use crate::iracing_sdk::CarSnapshot;

/// A fala. Uma só chave: as variações (`carro_fora_frente_2`, `_3`) são resolvidas
/// pelo rodízio da camada de voz, como no resto do pacote.
pub const CHAVE_FORA_FRENTE: &str = "carro_fora_frente";
/// Carro parado na pista à frente.
///
/// Ganhou áudio depois de Okayama. Até ali a família era só observação, porque a primeira
/// corrida gravada não teve um único carro parado em 17 minutos e faltava caso positivo
/// para calibrar por quanto tempo abaixo de [`PARADO_KMH`] um carro vira notícia. Okayama
/// deu quatro episódios — 4,18 / 4,65 / 7,98 / 19,70 s — e a resposta foi que **piso
/// nenhum é preciso**: não há ruído curto para cortar em 34 min de corrida. Medida sobre
/// as duas capturas, é a família de MAIOR confiança do sistema: 15% de avisos inúteis,
/// contra 35% de [`CHAVE_FORA_FRENTE`]. Um carro parado tende a continuar parado.
pub const CHAVE_PARADO_FRENTE: &str = "carro_parado_frente";

// As superfícies, o estado de corrida e a [`JANELA_VEL_S`] vivem em
// [`crate::iracing_sdk::spotter_base`] — são contrato do SDK e infraestrutura da família,
// não calibração deste módulo. O que a medição DESTE detector definiu está abaixo.
//
// Sobre a janela de velocidade, que é compartilhada mas foi medida aqui: 0,25 s são as
// mesmas 5 amostras a 20 Hz que a análise usou, e trocá-la invalidaria a separação que ela
// produziu. Medida contra a `Speed` do próprio jogador, erra 2 km/h na mediana —
// irrelevante contra um limiar de 40%.

/// Quanto do passado conta como "ritmo recente" (s) e em quantos baldes ele é medido.
///
/// Baldes rotativos de 1 s em vez de um histórico completo: o pico de 10 s sai de um
/// `max` sobre 10 números, e o custo por carro fica constante. A precisão perdida é a
/// borda do balde (até 1 s), que não muda nada num teste de "estava andando".
const PICO_JANELA_S: f64 = 10.0;
const PICO_BALDES: usize = 10;

/// Piso do "estava andando" (km/h). Abaixo disso o carro não perdeu ritmo — ele nunca
/// teve ritmo, e é grid, box ou formação.
const PICO_MIN_KMH: f64 = 50.0;

/// Fração do pico recente abaixo da qual a saída deixa de ser limite de pista.
/// 0,60 = perdeu mais de 40%. Nos dados: os dois reais caíram para 51 e 42 km/h vindo
/// de ritmo de corrida; os dois falsos passaram pela grama a 167 e 202 km/h.
const FRACAO_RITMO: f64 = 0.60;

/// A janela de disparo, em tempo até chegar (s) e em distância (m).
///
/// Sem mínimo de distância de propósito: o piso em TEMPO já cobre o caso do obstáculo
/// que aparece a 3 metros — que aconteceu, e para o qual nenhum rádio ajuda.
const TTA_MIN_S: f64 = 2.0;
const TTA_MAX_S: f64 = 5.0;
const DIST_MAX_M: f64 = 200.0;

/// Velocidade abaixo da qual um carro conta como parado (km/h). Só para a família de
/// OBSERVAÇÃO — não há áudio ligado a ela (ver [`Episodio`]).
const PARADO_KMH: f64 = 5.0;

/// Quantos episódios encerrados o histórico guarda.
const MAX_EPISODIOS: usize = 60;

/// Que tipo de obstáculo o episódio descreve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoObstaculo {
    /// Fora da pista com perda de ritmo. Fala [`CHAVE_FORA_FRENTE`].
    Fora,
    /// Parado na pista tendo andado antes. Fala [`CHAVE_PARADO_FRENTE`].
    Parado,
}

/// Como o episódio terminou. É metade do valor do registro: "ficou 4 s fora e retomou"
/// e "ficou 4 s fora e foi para o box" pedem leituras opostas de corrida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Desfecho {
    /// Voltou para uma superfície válida.
    Retomou,
    /// Entrou no pit road / na caixa.
    FoiParaOBox,
    /// O carro deixou de aparecer em `cars[]` — guincho, garagem, desconexão.
    ///
    /// A assinatura, medida em Okayama: o carro para, some do array por ~145 s e
    /// reaparece já com `on_pit_road`. `CarIdxTrackSurface == -1` não aparece uma única
    /// vez na corrida inteira — mas **isso é obra nossa**, não do SDK: o `retain` em
    /// `imp/leitura.rs` descarta os carros fora do mundo antes de montar o array. Do
    /// ponto de vista deste módulo o efeito é o mesmo, e a ausência é o único sinal
    /// disponível; vale saber de onde ele vem antes de mexer naquele filtro.
    SumiuDoMundo,
    /// O jogador passou pelo ponto.
    Ultrapassado,
    /// A sessão deixou de estar em corrida (bandeirada, replay, fim).
    SessaoAcabou,
}

/// Um episódio de obstáculo, aberto ou encerrado.
///
/// O episódio é o material de calibração, e foi ele que tirou a família `Parado` do mudo.
/// Ela nasceu só como observação porque a primeira corrida gravada não teve um único carro
/// parado em 17 minutos — `ReasonOutStr` foi `Running` para os 41 — e sem caso positivo não
/// havia como responder por quanto tempo abaixo de [`PARADO_KMH`] um carro vira notícia.
/// Okayama deu quatro casos e a resposta foi *nenhum piso*; hoje as duas famílias falam.
/// A lição que fica é o mecanismo: gravar antes de anunciar deixou a decisão sair de dado
/// em vez de gosto, sem mexer no gravador nem neste arquivo duas vezes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Episodio {
    pub id: u64,
    pub car_idx: i32,
    pub tipo: TipoObstaculo,
    /// `SessionTime` da abertura.
    pub inicio_s: f64,
    /// Duração até o encerramento; enquanto aberto, o que já durou.
    pub duracao_s: f64,
    /// Pico dos 10 s anteriores à abertura (km/h) — a `velocidade_anterior_máxima`.
    pub pico_kmh: f64,
    /// Menor velocidade vista durante o episódio (km/h).
    pub minima_kmh: f64,
    /// `CarIdxTrackSurface` na abertura.
    pub superficie: i32,
    /// Todo `CarIdxTrackSurfaceMaterial` pisado, na ordem em que apareceu. É o que
    /// separaria brita de grama — em Lime Rock não separou nada, porque não há brita.
    pub materiais: Vec<i32>,
    /// Distância do perseguidor mais próximo na abertura (m) e quanto ele levaria para
    /// chegar (s). `None` quando não havia ninguém atrás no alcance.
    pub perseguidor_m: Option<f64>,
    pub perseguidor_s: Option<f64>,
    /// Índice do perseguidor mais próximo.
    pub perseguidor_idx: Option<i32>,
    /// Distância com sinal do JOGADOR até este carro na abertura e no encerramento (m):
    /// positiva à frente, negativa atrás. A diferença entre as duas é o terreno que o
    /// jogador ganhou — e é ela, não a duração, que diz se a escapada teve consequência.
    ///
    /// "Ficou 4 s fora" e "perdeu 4 s para você" são coisas diferentes: dá para passar
    /// quatro segundos com duas rodas na grama e perder meio segundo.
    pub gap_inicio_m: f64,
    pub gap_fim_m: f64,
    /// Posição do carro e do jogador na abertura e no encerramento.
    pub posicao_inicio: i32,
    pub posicao_fim: i32,
    pub posicao_jogador_inicio: i32,
    pub posicao_jogador_fim: i32,
    /// O aviso de voz já saiu neste episódio. Um por episódio, sempre.
    pub avisado: bool,
    /// `None` enquanto aberto.
    pub desfecho: Option<Desfecho>,
}

impl Episodio {
    /// Quanto o jogador ganhou de terreno sobre este carro durante o episódio (m).
    /// Positivo = aproximou (ou abriu, se o carro estava atrás).
    pub fn ganho_do_jogador_m(&self) -> f64 {
        self.gap_inicio_m - self.gap_fim_m
    }
}

/// Uma amostra do mundo inteiro, do ponto de vista do spotter de frente.
#[derive(Debug, Clone, Copy)]
pub struct AmostraFrente<'a> {
    pub tempo_s: f64,
    /// `SessionState`.
    pub estado_sessao: i32,
    /// Comprimento da pista (m), do YAML. Zero = desconhecido → nada é detectado.
    pub comprimento_m: f64,
    pub jogador_idx: i32,
    pub jogador_pct: f64,
    pub jogador_vel_ms: f64,
    pub jogador_posicao: i32,
    /// No carro, na pista, fora do box e sem replay rodando.
    pub jogador_na_pista: bool,
    pub carros: &'a [CarSnapshot],
}

/// O que este módulo sabe de um carro.
#[derive(Debug, Clone)]
struct Carro {
    /// `(tempo, pct)` recentes, o bastante para cobrir [`JANELA_VEL_S`].
    hist: VecDeque<(f64, f64)>,
    /// Pico de velocidade por balde de 1 s, rotativo.
    baldes: [f64; PICO_BALDES],
    balde: usize,
    balde_ate_s: f64,
    vel_kmh: Option<f64>,
    /// `SessionTime` da última amostra em que este carro apareceu em `cars[]`.
    /// `None` = nunca foi visto nesta sessão.
    visto_em_s: Option<f64>,
    episodio: Option<Episodio>,
    /// Um episódio acabou de fechar e a condição física do carro AINDA é de obstáculo.
    ///
    /// Sem isto, todo encerramento que não vem da física reabre no tick seguinte. O caso
    /// que expõe: o jogador ultrapassa um carro enterrado na grama — o episódio fecha
    /// como `Ultrapassado`, o carro continua exatamente onde estava, e 16 ms depois um
    /// episódio novo nasce. Em 10 s de escapada isso são centenas de episódios idênticos,
    /// e o histórico de calibração — que é o produto deste módulo — vira lixo.
    ///
    /// A trava cai sozinha na primeira amostra em que o carro deixa de ser obstáculo.
    /// Ou seja: **um episódio novo exige uma volta ao normal no meio.**
    aguardando_normalizar: bool,
}

impl Default for Carro {
    fn default() -> Self {
        Carro {
            hist: VecDeque::new(),
            baldes: [0.0; PICO_BALDES],
            balde: 0,
            balde_ate_s: f64::NEG_INFINITY,
            vel_kmh: None,
            visto_em_s: None,
            episodio: None,
            aguardando_normalizar: false,
        }
    }
}

impl Carro {
    fn zerar(&mut self) {
        *self = Carro::default();
    }

    /// Velocidade derivada sobre [`JANELA_VEL_S`]. `None` até haver histórico bastante —
    /// e `None` NÃO é zero: um carro sem histórico não pode ser confundido com um carro
    /// parado, que é o erro que transformaria toda entrada de carro no mundo em obstáculo.
    fn atualizar_velocidade(&mut self, tempo_s: f64, pct: f64, comprimento_m: f64) {
        self.hist.push_back((tempo_s, pct));
        while let Some(&(t0, _)) = self.hist.front() {
            if tempo_s - t0 > JANELA_VEL_S * 2.0 {
                self.hist.pop_front();
            } else {
                break;
            }
        }
        // A amostra mais antiga que já cobre a janela.
        let base = self
            .hist
            .iter()
            .find(|&&(t, _)| tempo_s - t >= JANELA_VEL_S)
            .copied();
        self.vel_kmh = base.and_then(|(t0, p0)| {
            let dt = tempo_s - t0;
            if dt <= 0.0 {
                return None;
            }
            let mut d = pct - p0;
            // Cruzou a linha de chegada dentro da janela.
            if d < -0.5 {
                d += 1.0;
            }
            if d > 0.5 {
                d -= 1.0;
            }
            Some(d * comprimento_m / dt * 3.6)
        });
    }

    /// Alimenta os baldes de pico. Cada balde cobre 1 s; ao virar, o mais velho é zerado.
    fn atualizar_pico(&mut self, tempo_s: f64) {
        let v = self.vel_kmh.unwrap_or(0.0).max(0.0);
        if self.balde_ate_s == f64::NEG_INFINITY {
            self.balde_ate_s = tempo_s + PICO_JANELA_S / PICO_BALDES as f64;
        }
        while tempo_s >= self.balde_ate_s {
            self.balde = (self.balde + 1) % PICO_BALDES;
            self.baldes[self.balde] = 0.0;
            self.balde_ate_s += PICO_JANELA_S / PICO_BALDES as f64;
        }
        if v > self.baldes[self.balde] {
            self.baldes[self.balde] = v;
        }
    }

    fn pico_kmh(&self) -> f64 {
        self.baldes.iter().copied().fold(0.0, f64::max)
    }
}

/// Distância COM SINAL, de `de` até `para`: positiva à frente, negativa atrás, sempre
/// dentro de meia volta. É a forma certa de guardar um gap que pode inverter — e ele
/// inverte, que é justamente o caso "o jogador passou".
fn com_sinal(de_pct: f64, para_pct: f64, comprimento_m: f64) -> f64 {
    let mut d = para_pct - de_pct;
    if d > 0.5 {
        d -= 1.0;
    }
    if d < -0.5 {
        d += 1.0;
    }
    d * comprimento_m
}

/// A máquina. Pura: recebe amostras e devolve, no máximo, uma chave de fala por amostra.
#[derive(Debug)]
pub struct ObservadorFrente {
    carros: Vec<Carro>,
    ultimo_tempo_s: f64,
    proximo_id: u64,
    encerrados: VecDeque<Episodio>,
    /// Episódios fechados nesta amostra, à espera de quem os registre. Drenado a cada
    /// tick pela fachada — o histórico em memória some quando o app fecha, e sem esta
    /// saída a calibração da família `Parado`, que é a razão de ela existir, se perderia
    /// justamente na corrida que a produziu.
    recem_encerrados: Vec<Episodio>,
    /// Carro e EPISÓDIO cujo aviso a última amostra devolveu, à espera de confirmação.
    ///
    /// O id do episódio junto e não só o índice: entre a detecção e a confirmação o
    /// episódio daquele carro pode ter fechado e outro ter nascido, e aí a confirmação
    /// calaria um episódio novo que nunca chegou a falar.
    alvo_pendente: Option<(usize, u64)>,
}

impl Default for ObservadorFrente {
    fn default() -> Self {
        Self::novo()
    }
}

impl ObservadorFrente {
    pub fn novo() -> Self {
        ObservadorFrente {
            carros: vec![Carro::default(); MAX_CARROS],
            ultimo_tempo_s: 0.0,
            proximo_id: 1,
            encerrados: VecDeque::new(),
            recem_encerrados: Vec::new(),
            alvo_pendente: None,
        }
    }

    fn zerar(&mut self) {
        for c in self.carros.iter_mut() {
            c.zerar();
        }
    }

    /// Uma amostra. Devolve a chave de fala quando um episódio entra na janela de aviso
    /// pela primeira vez.
    ///
    /// Se o aviso não sair nesta amostra (porque o chamador já emitiu algo lateral, que
    /// tem prioridade), o episódio continua marcado como não avisado e tenta de novo na
    /// próxima — 16 ms depois, a 60 Hz. **Nunca descarta, no máximo adia**: um aviso de
    /// segurança que some para não atropelar a cadência é o defeito que este projeto já
    /// cometeu uma vez, no spotter lateral.
    pub fn observar(&mut self, a: AmostraFrente<'_>) -> Option<&'static str> {
        let salto = saltou(self.ultimo_tempo_s, a.tempo_s);
        self.ultimo_tempo_s = a.tempo_s;
        if salto {
            self.zerar();
            return None;
        }
        if a.comprimento_m <= 0.0 {
            return None;
        }

        let em_corrida = a.estado_sessao == ESTADO_CORRIDA;

        // Passo 1: velocidade e pico de todo mundo. Sai antes da detecção porque o pico
        // de um carro precisa continuar sendo medido enquanto ele está na pista, mesmo
        // com a sessão fora de corrida — senão, no primeiro tick de corrida ninguém teria
        // histórico e a regra "estava andando" reprovaria todo mundo.
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS {
                continue;
            }
            self.carros[i].visto_em_s = Some(a.tempo_s);
            if c.track_surface == SUP_FORA_DO_MUNDO || c.lap_dist_pct < 0.0 {
                self.carros[i].hist.clear();
                self.carros[i].vel_kmh = None;
                continue;
            }
            self.carros[i].atualizar_velocidade(a.tempo_s, c.lap_dist_pct, a.comprimento_m);
            self.carros[i].atualizar_pico(a.tempo_s);
        }

        // Passo 2: abre, mantém e encerra episódios.
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS || c.is_player || c.idx == a.jogador_idx {
                continue;
            }
            self.passo_episodio(c, &a, em_corrida);
        }

        // Passo 2b: quem sumiu do array. O guincho do iRacing não avisa — o carro
        // simplesmente deixa de aparecer em `cars[]` (ver [`Desfecho::SumiuDoMundo`]).
        // Sem esta varredura, um episódio aberto num carro guinchado nunca fecharia: o
        // laço acima só visita quem está presente. O que sobra é um obstáculo eterno com
        // uma duração inventada — e a duração é justamente o número que este módulo
        // existe para medir.
        self.fechar_ausentes(a.tempo_s);

        // Passo 3: quem merece o rádio. Só o mais próximo — dois obstáculos na janela ao
        // mesmo tempo não existiram nos dados, mas se existirem o que importa é o primeiro
        // que o piloto vai encontrar, seja de que família for.
        if !a.jogador_na_pista || !em_corrida || a.jogador_vel_ms <= 1.0 {
            return None;
        }
        let mut alvo: Option<(f64, usize)> = None;
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS {
                continue;
            }
            let Some(ep) = &self.carros[i].episodio else {
                continue;
            };
            if ep.avisado {
                continue;
            }
            let dist = adiante(a.jogador_pct, c.lap_dist_pct, a.comprimento_m);
            if dist > DIST_MAX_M {
                continue;
            }
            let tta = dist / a.jogador_vel_ms;
            if !(TTA_MIN_S..=TTA_MAX_S).contains(&tta) {
                continue;
            }
            if alvo.map(|(d, _)| dist < d).unwrap_or(true) {
                alvo = Some((dist, i));
            }
        }
        let (_, i) = alvo?;
        // NÃO marca como avisado aqui. Quem sabe se a fala saiu de verdade é o chamador —
        // uma entrada lateral pode ganhar o tick, e nesse caso o aviso tem de continuar
        // pendente para tentar de novo 16 ms depois. Marcar na detecção transformaria o
        // adiamento em descarte, que é exatamente o defeito que este projeto já cometeu
        // uma vez no spotter lateral. Ver [`ObservadorFrente::confirmar_aviso`].
        let ep = self.carros[i].episodio.as_ref()?;
        let chave = match ep.tipo {
            TipoObstaculo::Fora => CHAVE_FORA_FRENTE,
            TipoObstaculo::Parado => CHAVE_PARADO_FRENTE,
        };
        self.alvo_pendente = Some((i, ep.id));
        Some(chave)
    }

    /// O aviso devolvido pela última [`ObservadorFrente::observar`] realmente virou fala.
    /// Só aqui o episódio passa a contar como avisado.
    pub fn confirmar_aviso(&mut self) {
        let Some((i, id)) = self.alvo_pendente.take() else {
            return;
        };
        match self.carros[i].episodio.as_mut() {
            Some(ep) if ep.id == id => ep.avisado = true,
            // O episódio trocou no meio. Não é o que falou; deixa quieto para que ele
            // ainda possa ter o aviso dele.
            _ => {}
        }
    }

    /// Encerra os episódios de carros que sumiram do array `cars[]`.
    ///
    /// A duração é contada até a última vez em que o carro foi VISTO, não até agora. É a
    /// diferença entre "ficou 3 segundos parado e foi guinchado" e "ficou 150 segundos
    /// parado", que é o que sai quando se conta o tempo de ausência como tempo de
    /// obstáculo. Um dos dois números é falso por cinquenta vezes.
    fn fechar_ausentes(&mut self, agora_s: f64) {
        for i in 0..self.carros.len() {
            let Some(visto) = self.carros[i].visto_em_s else {
                continue;
            };
            if agora_s - visto < AUSENCIA_MAX_S {
                continue;
            }
            self.carros[i].aguardando_normalizar = false;
            let Some(mut ep) = self.carros[i].episodio.take() else {
                continue;
            };
            ep.duracao_s = visto - ep.inicio_s;
            ep.desfecho = Some(Desfecho::SumiuDoMundo);
            self.recem_encerrados.push(ep.clone());
            self.encerrados.push_back(ep);
            while self.encerrados.len() > MAX_EPISODIOS {
                self.encerrados.pop_front();
            }
        }
    }

    /// Abre, atualiza ou encerra o episódio de um carro.
    fn passo_episodio(&mut self, c: &CarSnapshot, a: &AmostraFrente<'_>, em_corrida: bool) {
        let i = c.idx as usize;
        let vel = self.carros[i].vel_kmh;
        let pico = self.carros[i].pico_kmh();
        let gap = com_sinal(a.jogador_pct, c.lap_dist_pct, a.comprimento_m);

        if let Some(ep) = self.carros[i].episodio.as_mut() {
            ep.duracao_s = a.tempo_s - ep.inicio_s;
            ep.gap_fim_m = gap;
            ep.posicao_fim = c.position;
            ep.posicao_jogador_fim = a.jogador_posicao;
            if let Some(v) = vel {
                if v < ep.minima_kmh {
                    ep.minima_kmh = v;
                }
            }
            if !ep.materiais.contains(&c.track_surface_material) {
                ep.materiais.push(c.track_surface_material);
            }

            // Encerramento. A velocidade NÃO entra aqui de propósito: a perda de ritmo é
            // o filtro de ENTRADA, que separa a escapada do corte de grama. Depois de
            // aberto, o episódio é a permanência fora da pista — um carro que recupera
            // ritmo ainda enterrado na grama continua sendo o mesmo obstáculo, e fechar
            // ali só faria o episódio piscar e reabrir.
            let desfecho = if !em_corrida {
                Some(Desfecho::SessaoAcabou)
            } else if c.track_surface == SUP_FORA_DO_MUNDO {
                // Inalcançável na prática, e de propósito: nosso próprio leitor descarta
                // os carros fora do mundo antes de montar `cars[]` (ver o `retain` em
                // `imp/leitura.rs`). Quem fecha esses episódios é `fechar_ausentes`.
                // Fica aqui porque a regra é certa se um dia o filtro mudar.
                Some(Desfecho::SumiuDoMundo)
            } else if c.on_pit_road
                || c.track_surface == SUP_NA_CAIXA
                || c.track_surface == SUP_ENTRANDO_BOX
            {
                Some(Desfecho::FoiParaOBox)
            } else if gap < 0.0 && ep.gap_inicio_m > 0.0 {
                Some(Desfecho::Ultrapassado)
            } else {
                match ep.tipo {
                    TipoObstaculo::Fora if c.track_surface == SUP_NA_PISTA => {
                        Some(Desfecho::Retomou)
                    }
                    TipoObstaculo::Parado if vel.map(|v| v >= PARADO_KMH).unwrap_or(false) => {
                        Some(Desfecho::Retomou)
                    }
                    _ => None,
                }
            };
            if let Some(d) = desfecho {
                let mut ep = self.carros[i].episodio.take().expect("acabou de existir");
                ep.desfecho = Some(d);
                self.recem_encerrados.push(ep.clone());
                self.encerrados.push_back(ep);
                while self.encerrados.len() > MAX_EPISODIOS {
                    self.encerrados.pop_front();
                }
                // Fechou, mas o carro pode continuar exatamente na mesma situação —
                // encerramentos como `Ultrapassado` são do ponto de vista do jogador,
                // não do carro. Ver [`Carro::aguardando_normalizar`].
                self.carros[i].aguardando_normalizar = true;
            }
            return;
        }

        // Abertura. Só em corrida, e só para quem estava andando.
        if !em_corrida || pico < PICO_MIN_KMH {
            self.carros[i].aguardando_normalizar = false;
            return;
        }
        let Some(v) = vel else { return };
        let tipo = if c.track_surface == SUP_FORA_DA_PISTA && v < FRACAO_RITMO * pico {
            TipoObstaculo::Fora
        } else if c.track_surface == SUP_NA_PISTA && !c.on_pit_road && v < PARADO_KMH {
            TipoObstaculo::Parado
        } else {
            // Condição normal: é aqui que a trava cai.
            self.carros[i].aguardando_normalizar = false;
            return;
        };
        if self.carros[i].aguardando_normalizar {
            return;
        }

        let (perseguidor_idx, perseguidor_m, perseguidor_s) = self.perseguidor(c, a);
        let id = self.proximo_id;
        self.proximo_id += 1;
        self.carros[i].episodio = Some(Episodio {
            id,
            car_idx: c.idx,
            tipo,
            inicio_s: a.tempo_s,
            duracao_s: 0.0,
            pico_kmh: pico,
            minima_kmh: v,
            superficie: c.track_surface,
            materiais: vec![c.track_surface_material],
            perseguidor_idx,
            perseguidor_m,
            perseguidor_s,
            gap_inicio_m: gap,
            gap_fim_m: gap,
            posicao_inicio: c.position,
            posicao_fim: c.position,
            posicao_jogador_inicio: a.jogador_posicao,
            posicao_jogador_fim: a.jogador_posicao,
            avisado: false,
            desfecho: None,
        });
    }

    /// Quem vem atrás do obstáculo, mais próximo, andando. É a medida que a calibração
    /// do "parado" vai pedir — e ela é entre CARROS, não a partir do jogador: numa
    /// captura com o jogador parado no box, tudo medido a partir dele seria inútil.
    fn perseguidor(
        &self,
        obst: &CarSnapshot,
        a: &AmostraFrente<'_>,
    ) -> (Option<i32>, Option<f64>, Option<f64>) {
        let mut melhor: Option<(f64, i32, f64)> = None;
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS || c.idx == obst.idx {
                continue;
            }
            if c.track_surface != SUP_NA_PISTA || c.on_pit_road {
                continue;
            }
            let Some(v) = self.carros[i].vel_kmh else {
                continue;
            };
            if v < 30.0 {
                continue;
            }
            let d = adiante(c.lap_dist_pct, obst.lap_dist_pct, a.comprimento_m);
            if d <= 0.0 || d > a.comprimento_m / 2.0 {
                continue;
            }
            if melhor.map(|(md, _, _)| d < md).unwrap_or(true) {
                melhor = Some((d, c.idx, d / (v / 3.6)));
            }
        }
        match melhor {
            Some((d, idx, tta)) => (Some(idx), Some(d), Some(tta)),
            None => (None, None, None),
        }
    }

    /// Retira os episódios fechados na última amostra. Ver [`Self::recem_encerrados`].
    pub fn drenar_encerrados(&mut self) -> Vec<Episodio> {
        std::mem::take(&mut self.recem_encerrados)
    }

    /// Episódios encerrados, mais antigo primeiro. É o material de calibração.
    pub fn encerrados(&self) -> Vec<Episodio> {
        self.encerrados.iter().cloned().collect()
    }

    /// Episódios abertos AGORA.
    pub fn abertos(&self) -> Vec<Episodio> {
        self.carros
            .iter()
            .filter_map(|c| c.episodio.clone())
            .collect()
    }
}

// ─────────────────────────── O observador global ───────────────────────────

/// Comprimento da pista corrente (m), em bits de `f64`. Vem do YAML de sessão, que o
/// amostrador relê de tempos em tempos; zero até a primeira leitura, e zero desliga a
/// detecção inteira — sem escala, "6% da volta" não vira "150 m".
static COMPRIMENTO_M: AtomicU64 = AtomicU64::new(0);

/// Registra o comprimento da pista. Chamado pelo amostrador junto com o resto do que
/// ele já extrai do YAML.
pub fn definir_comprimento_m(m: f64) {
    COMPRIMENTO_M.store(m.to_bits(), Ordering::Relaxed);
}

pub fn comprimento_m() -> f64 {
    f64::from_bits(COMPRIMENTO_M.load(Ordering::Relaxed))
}

spotter_singleton!(ObservadorFrente, ObservadorFrente::novo());

/// Alimenta o observador global com uma amostra. Devolve a chave de fala, se houver.
///
/// Chamado de dentro de [`crate::iracing_sdk::spotter::observar`], porque os dois
/// spotters compartilham a mesma fila de eventos: um cursor só no front, uma voz só,
/// e ids que não se atropelam.
pub fn observar(t: &crate::iracing_sdk::IracingTelemetry) -> Option<&'static str> {
    let no_carro = t.on_track && !t.is_replay_playing;
    let jogador = t
        .cars
        .iter()
        .find(|c| c.is_player || c.idx == t.player_car_idx);
    let (chave, encerrados) = {
        let mut obs = lock();
        let chave = obs.observar(AmostraFrente {
            tempo_s: t.session_time,
            estado_sessao: t.session_state,
            comprimento_m: comprimento_m(),
            jogador_idx: t.player_car_idx,
            jogador_pct: jogador.map(|c| c.lap_dist_pct).unwrap_or(t.lap_dist_pct),
            jogador_vel_ms: t.speed_ms,
            jogador_posicao: t.position,
            jogador_na_pista: no_carro && !t.player_on_pit_road,
            carros: &t.cars,
        });
        (chave, obs.drenar_encerrados())
    };
    // Fora do lock. O histórico em memória tem 60 posições e morre com o processo; a
    // corrida que produz o dado de calibração é justamente a que o perderia. O formato é
    // chave=valor de propósito, para um script extrair sem depender de prosa.
    for e in encerrados {
        crate::diagnostico::linha(
            "spotter_frente",
            &format!(
                "episodio tipo={:?} carro={} t={:.1} dur={:.2} pico={:.0} min={:.0} sup={} mat={:?} \
                 perseguidor={} perseguidor_m={} perseguidor_s={} gap_ini={:.0} gap_fim={:.0} \
                 pos={}->{} pos_jogador={}->{} avisado={} desfecho={:?}",
                e.tipo,
                e.car_idx,
                e.inicio_s,
                e.duracao_s,
                e.pico_kmh,
                e.minima_kmh,
                e.superficie,
                e.materiais,
                e.perseguidor_idx.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
                e.perseguidor_m.map(|v| format!("{v:.0}")).unwrap_or_else(|| "—".into()),
                e.perseguidor_s.map(|v| format!("{v:.1}")).unwrap_or_else(|| "—".into()),
                e.gap_inicio_m,
                e.gap_fim_m,
                e.posicao_inicio,
                e.posicao_fim,
                e.posicao_jogador_inicio,
                e.posicao_jogador_fim,
                e.avisado,
                e.desfecho,
            ),
        );
    }
    chave
}

/// O aviso da última amostra virou fala. Ver [`ObservadorFrente::confirmar_aviso`].
pub fn confirmar_aviso() {
    lock().confirmar_aviso();
}

/// Episódios encerrados — o material de calibração da família `Parado`, que ainda não
/// tem áudio, e a matéria-prima do comentário do engenheiro depois da passagem.
pub fn encerrados() -> Vec<Episodio> {
    lock().encerrados()
}

/// Episódios abertos agora.
pub fn abertos() -> Vec<Episodio> {
    lock().abertos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iracing_sdk::spotter_base::SALTO_MAX_S;

    /// Pista de 2369 m — Lime Rock, a mesma da corrida que calibrou tudo isto.
    const PISTA: f64 = 2369.0;
    const DT: f64 = 1.0 / 60.0;

    fn carro(idx: i32, pct: f64, superficie: i32) -> CarSnapshot {
        CarSnapshot {
            idx,
            lap_dist_pct: pct,
            track_surface: superficie,
            track_surface_material: 15,
            position: idx + 1,
            ..Default::default()
        }
    }

    /// Dois carros: o jogador (idx 0) e o alvo (idx 1), ambos ANDANDO.
    ///
    /// Que os dois andem não é detalhe de conforto: um carro congelado no lugar é, por
    /// definição do próprio detector, um obstáculo "parado" legítimo. A primeira versão
    /// destes testes segurava o alvo num `pct` fixo e media a coisa errada.
    struct Cena {
        obs: ObservadorFrente,
        t: f64,
        jog: f64,
        alvo: f64,
        jog_ms: f64,
        avisos: usize,
        /// Distância jogador→alvo no instante de cada aviso (m).
        dist_no_aviso: Vec<f64>,
        /// A chave de cada aviso, na ordem. Contar avisos não basta desde que a família
        /// `Parado` ganhou áudio: as duas famílias saem pelo mesmo caminho, e um teste que
        /// só conta não vê a diferença entre anunciar a coisa certa e a errada.
        chaves: Vec<&'static str>,
        estado: i32,
        jog_na_pista: bool,
        sup_jogador: i32,
        /// Confirmar o aviso é o que o chamador real faz quando a fala sai de fato.
        /// Desligar isto simula o tick em que uma entrada lateral roubou a vez.
        confirma: bool,
    }

    impl Cena {
        /// Jogador em 10% da volta, alvo `metros` à frente.
        fn nova(metros: f64) -> Self {
            Cena {
                obs: ObservadorFrente::novo(),
                t: 0.0,
                jog: 0.10,
                alvo: 0.10 + metros / PISTA,
                jog_ms: 40.0,
                avisos: 0,
                dist_no_aviso: Vec::new(),
                chaves: Vec::new(),
                estado: ESTADO_CORRIDA,
                jog_na_pista: true,
                sup_jogador: SUP_NA_PISTA,
                confirma: true,
            }
        }

        fn dist_m(&self) -> f64 {
            adiante(self.jog, self.alvo, PISTA)
        }

        /// Avança `segundos` a 60 Hz com o jogador a `jog_kmh` e o alvo a `alvo_kmh`,
        /// pisando em `sup`.
        fn rodar(&mut self, segundos: f64, jog_kmh: f64, alvo_kmh: f64, sup: i32) {
            self.jog_ms = jog_kmh / 3.6;
            let mut restante = segundos;
            while restante > 0.0 {
                let carros = [
                    carro(0, self.jog, self.sup_jogador),
                    carro(1, self.alvo, sup),
                ];
                let a = AmostraFrente {
                    tempo_s: self.t,
                    estado_sessao: self.estado,
                    comprimento_m: PISTA,
                    jogador_idx: 0,
                    jogador_pct: self.jog,
                    jogador_vel_ms: self.jog_ms,
                    jogador_posicao: 5,
                    jogador_na_pista: self.jog_na_pista,
                    carros: &carros,
                };
                let d = self.dist_m();
                if let Some(chave) = self.obs.observar(a) {
                    // Como o chamador de verdade: confirma só quando a fala sai mesmo.
                    if self.confirma {
                        self.obs.confirmar_aviso();
                    }
                    self.avisos += 1;
                    self.dist_no_aviso.push(d);
                    self.chaves.push(chave);
                }
                self.jog = (self.jog + (jog_kmh / 3.6) * DT / PISTA).fract();
                self.alvo = (self.alvo + (alvo_kmh / 3.6) * DT / PISTA).fract();
                self.t += DT;
                restante -= DT;
            }
        }

        /// 12 s de corrida normal — forma o pico dos dois, que é o "estava andando".
        fn aquecer(&mut self) {
            self.rodar(12.0, 200.0, 200.0, SUP_NA_PISTA);
        }
    }

    #[test]
    fn corte_de_grama_a_alta_velocidade_nao_e_obstaculo() {
        // O falso positivo real da captura: carro passando pela grama a 200 km/h.
        let mut c = Cena::nova(150.0);
        c.aquecer();
        c.rodar(2.0, 200.0, 200.0, SUP_FORA_DA_PISTA);
        assert!(
            c.obs.abertos().is_empty(),
            "limite de pista virou obstáculo"
        );
        assert_eq!(c.avisos, 0);
    }

    #[test]
    fn escapada_com_perda_de_ritmo_abre_episodio() {
        // O positivo real: vinha a 200, caiu para 45 na grama.
        let mut c = Cena::nova(600.0);
        c.aquecer();
        c.rodar(1.0, 200.0, 45.0, SUP_FORA_DA_PISTA);
        let abertos = c.obs.abertos();
        assert_eq!(abertos.len(), 1);
        assert_eq!(abertos[0].tipo, TipoObstaculo::Fora);
        assert!(
            abertos[0].pico_kmh > 150.0,
            "pico {:.0}",
            abertos[0].pico_kmh
        );
    }

    #[test]
    fn largada_parada_nao_vira_quarenta_obstaculos() {
        // A armadilha que o `SessionState` não pega: 40 carros a 0 km/h, em asfalto,
        // na pista, com o estado já em Correndo.
        let mut obs = ObservadorFrente::novo();
        let carros: Vec<CarSnapshot> = (0..40)
            .map(|i| carro(i, 0.98 + i as f64 * 0.0001, SUP_NA_PISTA))
            .collect();
        let mut t = 0.0;
        while t < 8.0 {
            obs.observar(AmostraFrente {
                tempo_s: t,
                estado_sessao: ESTADO_CORRIDA,
                comprimento_m: PISTA,
                jogador_idx: 0,
                jogador_pct: 0.9847,
                jogador_vel_ms: 0.0,
                jogador_posicao: 5,
                jogador_na_pista: true,
                carros: &carros,
            });
            t += DT;
        }
        assert!(
            obs.abertos().is_empty(),
            "o grid parado virou {} obstáculos",
            obs.abertos().len()
        );
    }

    #[test]
    fn parado_apos_andar_abre_episodio_e_fala_a_chave_de_parado() {
        // Este teste já foi `..._mas_nunca_fala`, de quando a família era só observação.
        // Ele passava com o alvo a 600 m e 3 s de corrida — fora da janela de aviso —, ou
        // seja, provava o silêncio pelo motivo errado: teria passado igual com o áudio
        // ligado. Agora o alvo entra na janela e a chave é conferida, não só contada.
        let mut c = Cena::nova(600.0);
        c.aquecer();
        c.rodar(3.0, 200.0, 0.0, SUP_NA_PISTA);
        let abertos = c.obs.abertos();
        assert_eq!(abertos.len(), 1);
        assert_eq!(abertos[0].tipo, TipoObstaculo::Parado);
        assert!(abertos[0].pico_kmh > 150.0);
        assert_eq!(c.avisos, 0, "a 430 m o obstáculo ainda está longe demais");

        // Segue fechando até entrar na janela de 2 a 5 s.
        c.rodar(10.0, 200.0, 0.0, SUP_NA_PISTA);
        assert_eq!(
            c.chaves,
            vec![CHAVE_PARADO_FRENTE],
            "chaves: {:?}",
            c.chaves
        );
        let d = c.dist_no_aviso[0];
        let tta = d / (200.0 / 3.6);
        assert!(d <= DIST_MAX_M, "avisou a {d:.0} m, acima do teto");
        assert!(
            (TTA_MIN_S..=TTA_MAX_S).contains(&tta),
            "avisou com {tta:.2} s de sobra, fora da janela"
        );
    }

    #[test]
    fn as_duas_familias_falam_chaves_diferentes() {
        // O caminho do rádio é um só desde que `Parado` ganhou áudio. Se as duas famílias
        // saíssem pela mesma chave, o piloto ouviria "carro fora da pista" sobre um carro
        // parado no meio da reta — descrição errada de um perigo real, que é pior que
        // silêncio porque manda procurar do lado errado.
        let mut c = Cena::nova(400.0);
        c.aquecer();
        c.rodar(20.0, 144.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.chaves, vec![CHAVE_FORA_FRENTE]);
        assert_ne!(CHAVE_FORA_FRENTE, CHAVE_PARADO_FRENTE);
    }

    #[test]
    fn o_aviso_sai_dentro_da_janela_de_dois_a_cinco_segundos() {
        // Alvo 400 m à frente sai da pista e desacelera; o jogador vai alcançando.
        let mut c = Cena::nova(400.0);
        c.aquecer();
        c.rodar(20.0, 144.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.avisos, 1, "esperava um aviso, saíram {}", c.avisos);
        let d = c.dist_no_aviso[0];
        let tta = d / 40.0;
        assert!(d <= DIST_MAX_M, "avisou a {d:.0} m, acima do teto");
        assert!(
            (TTA_MIN_S..=TTA_MAX_S).contains(&tta),
            "avisou com {tta:.2} s de sobra, fora da janela"
        );
    }

    #[test]
    fn obstaculo_que_aparece_colado_nao_gera_aviso_inutil() {
        // O caso real da captura: o obstáculo apareceu com um perseguidor a 3 m. Nenhum
        // rádio ajuda ali, e um aviso atrasado é pior que silêncio.
        let mut c = Cena::nova(20.0);
        c.aquecer();
        c.rodar(1.0, 144.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.avisos, 0);
        // O episódio existe — só não vira rádio. A 20 m e fechando a 27 m/s, o jogador
        // já passou por cima dele antes de o segundo acabar, e o desfecho prova isso.
        assert_eq!(c.obs.abertos().len() + c.obs.encerrados().len(), 1);
    }

    #[test]
    fn nao_avisa_enquanto_o_obstaculo_esta_longe_demais() {
        // 600 m: vai ter saído da frente quando o jogador chegar.
        let mut c = Cena::nova(600.0);
        c.aquecer();
        c.rodar(2.0, 144.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.avisos, 0, "avisou a {:.0} m", c.dist_m());
        // E avisa quando finalmente entra na janela.
        c.rodar(20.0, 144.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.avisos, 1);
    }

    #[test]
    fn avisa_uma_vez_so_por_episodio() {
        let mut c = Cena::nova(400.0);
        c.aquecer();
        c.rodar(30.0, 144.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.avisos, 1, "o mesmo episódio falou {} vezes", c.avisos);
    }

    #[test]
    fn um_aviso_que_nao_virou_fala_volta_no_tick_seguinte() {
        // O tick em que uma entrada lateral rouba a vez. O aviso não pode sumir por
        // isso — precisa continuar pendente e sair assim que houver espaço.
        let mut c = Cena::nova(400.0);
        c.aquecer();
        c.confirma = false;
        c.rodar(20.0, 144.0, 45.0, SUP_FORA_DA_PISTA);
        assert!(
            c.avisos > 1,
            "sem confirmação o aviso deveria insistir, saiu {} vez(es)",
            c.avisos
        );
        // E uma única confirmação basta para calar o episódio de vez.
        let mut c = Cena::nova(400.0);
        c.aquecer();
        c.rodar(20.0, 144.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.avisos, 1);
    }

    #[test]
    fn sair_e_voltar_a_sair_sao_dois_episodios() {
        let mut c = Cena::nova(400.0);
        c.aquecer();
        c.rodar(10.0, 144.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.avisos, 1);
        // Volta à pista e retoma o ritmo — normaliza.
        c.rodar(6.0, 144.0, 160.0, SUP_NA_PISTA);
        assert_eq!(
            c.obs.encerrados().last().unwrap().desfecho,
            Some(Desfecho::Retomou)
        );
        // E sai de novo.
        c.rodar(15.0, 144.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.avisos, 2, "a segunda saída devia ser um episódio novo");
    }

    #[test]
    fn ultrapassar_encerra_o_episodio_e_nao_reabre_no_tick_seguinte() {
        // O defeito que a trava de normalização corrige: `Ultrapassado` é um
        // encerramento do ponto de vista do JOGADOR, e o carro continua na grama.
        let mut c = Cena::nova(150.0);
        c.aquecer();
        c.rodar(6.0, 200.0, 20.0, SUP_FORA_DA_PISTA);
        let encerrados = c.obs.encerrados();
        assert_eq!(
            encerrados.last().unwrap().desfecho,
            Some(Desfecho::Ultrapassado)
        );
        assert_eq!(encerrados.len(), 1, "reabriu {} vezes", encerrados.len());
        assert!(c.obs.abertos().is_empty());
    }

    #[test]
    fn ir_para_o_box_e_sumir_do_mundo_sao_desfechos_distintos() {
        let mut c = Cena::nova(600.0);
        c.aquecer();
        c.rodar(1.0, 200.0, 45.0, SUP_FORA_DA_PISTA);
        c.rodar(0.2, 200.0, 45.0, SUP_ENTRANDO_BOX);
        assert_eq!(
            c.obs.encerrados().last().unwrap().desfecho,
            Some(Desfecho::FoiParaOBox)
        );

        let mut c = Cena::nova(600.0);
        c.aquecer();
        c.rodar(1.0, 200.0, 45.0, SUP_FORA_DA_PISTA);
        c.rodar(0.2, 200.0, 45.0, SUP_FORA_DO_MUNDO);
        assert_eq!(
            c.obs.encerrados().last().unwrap().desfecho,
            Some(Desfecho::SumiuDoMundo)
        );
    }

    #[test]
    fn o_carro_guinchado_some_do_array_e_o_episodio_fecha_com_a_duracao_certa() {
        // Assinatura medida em Okayama: o carro para, some de `cars[]` por dezenas de
        // segundos e reaparece no box. `NotInWorld` não aparece em momento nenhum.
        let mut c = Cena::nova(600.0);
        c.aquecer();
        c.rodar(3.0, 200.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.obs.abertos().len(), 1);
        let visto_por_ultimo = c.t;

        // Some do array: só o jogador continua sendo amostrado.
        let mut t = c.t;
        for _ in 0..(60 * 150) {
            let carros = [carro(0, c.jog, SUP_NA_PISTA)];
            c.obs.observar(AmostraFrente {
                tempo_s: t,
                estado_sessao: ESTADO_CORRIDA,
                comprimento_m: PISTA,
                jogador_idx: 0,
                jogador_pct: c.jog,
                jogador_vel_ms: 40.0,
                jogador_posicao: 5,
                jogador_na_pista: true,
                carros: &carros,
            });
            t += DT;
        }

        assert!(c.obs.abertos().is_empty(), "obstáculo eterno");
        let ep = c.obs.encerrados().last().cloned().unwrap();
        assert_eq!(ep.desfecho, Some(Desfecho::SumiuDoMundo));
        // O que importa: a duração é até a ÚLTIMA VEZ VISTO, não até agora. Contar a
        // ausência como permanência daria 150 s em vez de 3.
        assert!(
            (ep.duracao_s - (visto_por_ultimo - ep.inicio_s)).abs() < 0.1,
            "duração {:.1}s — contou o tempo de ausência",
            ep.duracao_s
        );
        assert!(ep.duracao_s < 5.0, "duração {:.1}s", ep.duracao_s);
    }

    #[test]
    fn o_episodio_fechado_sai_uma_vez_pela_drenagem() {
        // A drenagem é o único caminho do episódio para fora do processo — o histórico em
        // memória some quando o app fecha. Se ela devolvesse duas vezes, a calibração
        // contaria o mesmo caso em dobro; se não devolvesse, sumiria.
        let mut c = Cena::nova(600.0);
        c.aquecer();
        assert!(c.obs.drenar_encerrados().is_empty());
        c.rodar(1.0, 200.0, 45.0, SUP_FORA_DA_PISTA);
        c.rodar(0.5, 200.0, 200.0, SUP_NA_PISTA);
        let saiu = c.obs.drenar_encerrados();
        assert_eq!(saiu.len(), 1);
        assert_eq!(saiu[0].desfecho, Some(Desfecho::Retomou));
        assert!(c.obs.drenar_encerrados().is_empty(), "drenou duas vezes");
        // E continua no histórico para quem quiser olhar depois.
        assert_eq!(c.obs.encerrados().len(), 1);
    }

    #[test]
    fn um_quadro_perdido_nao_parte_o_episodio_em_dois() {
        let mut c = Cena::nova(600.0);
        c.aquecer();
        c.rodar(1.0, 200.0, 45.0, SUP_FORA_DA_PISTA);
        // Três quadros sem o carro — o que um engasgo do SDK produziria.
        let mut t = c.t;
        for _ in 0..3 {
            let carros = [carro(0, c.jog, SUP_NA_PISTA)];
            c.obs.observar(AmostraFrente {
                tempo_s: t,
                estado_sessao: ESTADO_CORRIDA,
                comprimento_m: PISTA,
                jogador_idx: 0,
                jogador_pct: c.jog,
                jogador_vel_ms: 40.0,
                jogador_posicao: 5,
                jogador_na_pista: true,
                carros: &carros,
            });
            t += DT;
        }
        c.t = t;
        c.rodar(1.0, 200.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.obs.abertos().len(), 1, "o engasgo fechou o episódio");
        assert!(c.obs.encerrados().is_empty());
    }

    #[test]
    fn a_bandeirada_encerra_o_que_estava_aberto() {
        let mut c = Cena::nova(600.0);
        c.aquecer();
        c.rodar(1.0, 200.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.obs.abertos().len(), 1);
        c.estado = 5; // Bandeirada
        c.rodar(0.2, 200.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(
            c.obs.encerrados().last().unwrap().desfecho,
            Some(Desfecho::SessaoAcabou)
        );
    }

    #[test]
    fn fora_de_corrida_nao_abre_episodio() {
        let mut c = Cena::nova(600.0);
        c.aquecer();
        c.estado = 5;
        c.rodar(3.0, 200.0, 45.0, SUP_FORA_DA_PISTA);
        assert!(c.obs.abertos().is_empty());
    }

    #[test]
    fn o_ganho_do_jogador_sai_do_gap_e_nao_da_duracao() {
        // Cinco segundos fora da pista NÃO são cinco segundos perdidos. Aqui o alvo
        // passa 5 s a 100 km/h enquanto o jogador segue a 200: ele perde ~139 m, que a
        // 200 km/h valem 2,5 s. Quem disser "perdeu cinco segundos" errou por dobro.
        let mut c = Cena::nova(600.0);
        c.aquecer();
        c.rodar(5.0, 200.0, 100.0, SUP_FORA_DA_PISTA);
        c.rodar(0.5, 200.0, 200.0, SUP_NA_PISTA);
        let ep = c.obs.encerrados().last().cloned().unwrap();
        assert!(ep.duracao_s > 4.0, "duração {:.1} s", ep.duracao_s);
        let ganho = ep.ganho_do_jogador_m();
        assert!(
            (120.0..165.0).contains(&ganho),
            "ganho {ganho:.1} m — a consequência tem de vir do gap, não do relógio"
        );
        let perdido_s = ganho / (200.0 / 3.6);
        assert!(
            perdido_s < ep.duracao_s * 0.7,
            "tempo perdido {perdido_s:.1} s ≈ duração {:.1} s — o teste não separa nada",
            ep.duracao_s
        );
    }

    #[test]
    fn com_o_jogador_no_box_o_detector_registra_mas_nao_fala() {
        let mut c = Cena::nova(150.0);
        c.aquecer();
        c.jog_na_pista = false;
        c.sup_jogador = SUP_NA_CAIXA;
        c.rodar(3.0, 0.0, 45.0, SUP_FORA_DA_PISTA);
        assert_eq!(c.avisos, 0);
        assert_eq!(
            c.obs.abertos().len(),
            1,
            "a captura de dados não pode depender de o jogador estar correndo"
        );
    }

    #[test]
    fn sem_comprimento_de_pista_nada_e_detectado() {
        let mut obs = ObservadorFrente::novo();
        let carros = [
            carro(0, 0.10, SUP_NA_PISTA),
            carro(1, 0.15, SUP_FORA_DA_PISTA),
        ];
        assert_eq!(
            obs.observar(AmostraFrente {
                tempo_s: 1.0,
                estado_sessao: ESTADO_CORRIDA,
                comprimento_m: 0.0,
                jogador_idx: 0,
                jogador_pct: 0.10,
                jogador_vel_ms: 40.0,
                jogador_posicao: 5,
                jogador_na_pista: true,
                carros: &carros,
            }),
            None
        );
        assert!(obs.abertos().is_empty());
    }

    #[test]
    fn um_salto_de_tempo_reinicia_sem_inventar_episodio() {
        let mut c = Cena::nova(150.0);
        c.aquecer();
        c.t += 600.0;
        c.rodar(0.1, 200.0, 45.0, SUP_FORA_DA_PISTA);
        assert!(c.obs.abertos().is_empty());
        assert_eq!(c.avisos, 0);
    }

    #[test]
    fn o_perseguidor_e_medido_entre_carros_e_nao_a_partir_do_jogador() {
        // O jogador fica no box a corrida inteira — como na captura de calibração.
        let mut obs = ObservadorFrente::novo();
        let mut t = 0.0;
        let mut pct1 = 0.30;
        let mut pct2 = 0.28; // ~47 m atrás do carro 1
        let passo = |kmh: f64| (kmh / 3.6) * DT / PISTA;
        let mut amostrar = |obs: &mut ObservadorFrente, t: f64, p1: f64, p2: f64, sup1: i32| {
            let carros = [
                carro(0, 0.90, SUP_NA_CAIXA),
                carro(1, p1, sup1),
                carro(2, p2, SUP_NA_PISTA),
            ];
            obs.observar(AmostraFrente {
                tempo_s: t,
                estado_sessao: ESTADO_CORRIDA,
                comprimento_m: PISTA,
                jogador_idx: 0,
                jogador_pct: 0.90,
                jogador_vel_ms: 0.0,
                jogador_posicao: 20,
                jogador_na_pista: false,
                carros: &carros,
            });
        };
        while t < 12.0 {
            amostrar(&mut obs, t, pct1, pct2, SUP_NA_PISTA);
            t += DT;
            pct1 += passo(200.0);
            pct2 += passo(200.0);
        }
        // O carro 1 sai da pista e perde ritmo; o 2 segue no ritmo, atrás.
        for _ in 0..30 {
            amostrar(&mut obs, t, pct1, pct2, SUP_FORA_DA_PISTA);
            t += DT;
            pct1 += passo(45.0);
            pct2 += passo(200.0);
        }
        let ep = obs.abertos().into_iter().find(|e| e.car_idx == 1).unwrap();
        assert_eq!(ep.perseguidor_idx, Some(2));
        let m = ep.perseguidor_m.unwrap();
        assert!((25.0..70.0).contains(&m), "perseguidor a {m:.0} m");
        assert!(ep.perseguidor_s.unwrap() < 2.0);
    }
}
