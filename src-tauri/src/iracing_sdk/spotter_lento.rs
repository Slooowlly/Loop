//! Spotter — carro LENTO à frente.
//!
//! O terceiro da família. O irmão [`crate::iracing_sdk::spotter_frente`] sabe dizer que
//! há um carro FORA DA PISTA e que há um carro PARADO; os dois são leituras de estado
//! (`TrackSurface == 0`, velocidade ≈ 0). Este é um limiar num contínuo, e é o caso mais
//! comum dos três: o carro que está andando, na pista, muito mais devagar que o resto.
//!
//! Todo número daqui saiu das duas corridas gravadas — Lime Rock (2369 m, 40 carros,
//! 17,1 min) e Okayama (1929 m, 41 carros, 17,3 min), cada carro da IA simulado como
//! jogador. Nenhum foi escolhido por parecer razoável. As quatro coisas que a medição
//! mudou em relação ao que se supunha antes de olhar:
//!
//! 1. **A mediana instantânea do campo não serve de referência.** Ela mistura carros em
//!    pontos DIFERENTES da volta: um está na reta a 200, outro na curva lenta a 60. Nas
//!    duas capturas a razão `v / mediana_do_campo` varia 0,45 só em função do trecho, e o
//!    p5 dela é 0,62 — ou seja, um carro perfeitamente normal passa a vida a 62% da
//!    mediana. Medido: com essa referência o detector dispara **150 vezes por piloto por
//!    corrida**. A referência tem de ser o ritmo esperado NAQUELE PONTO DA PISTA.
//! 2. **O que normaliza a amarela é o fator do campo, não a referência de posição.** Com
//!    o mapa por trecho, a mediana das razões do campo (o [`ObservadorLento::fator`]) fica
//!    em 1,00 no verde e em 0,99 sob amarela — a queda global de 30% do ritmo é absorvida
//!    inteira. O que sobra sob amarela é DISPERSÃO: o p1 da razão normalizada cai de 0,78
//!    (verde) para 0,48. O campo sanfona, e é daí que vêm 84% dos avisos.
//! 3. **O tempo até chegar é de FECHAMENTO, não `distância / velocidade do jogador`.** Um
//!    obstáculo parado o jogador alcança à própria velocidade; um carro lento, só à
//!    diferença. Usar a fórmula do irmão aqui gera aviso para carro que o jogador nunca
//!    encontra: 40% dos avisos ficavam sem chegada e a taxa de inúteis ia a 68%.
//! 4. **Aqui a permanência mínima AJUDA** — ao contrário do detector de obstáculo, onde
//!    foi testada e não ajudou. 1,0 s corta o volume pela metade (1,66 → 0,82 aviso por
//!    piloto) sem piorar a utilidade. Acima disso começa a piorar (2,0 s: 26% de inúteis).
//!
//! E a janela **não** pôde ser mais generosa que a do irmão, que era a hipótese: abrir de
//! 2–5 s para 2–7 s triplica o volume e leva os inúteis de 30% para 47%.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde::Serialize;

use crate::iracing_sdk::CarSnapshot;

/// A fala. Duas chaves porque o grau saiu dos dados e não do gosto: abaixo de
/// [`CORTE_GRAVE`] estão 18% dos avisos e **nenhum** deles era inútil nas duas corridas.
/// As variações (`_2`, `_3`) são resolvidas pelo rodízio da camada de voz.
pub const CHAVE_LENTO_FRENTE: &str = "carro_lento_frente";
pub const CHAVE_MUITO_LENTO_FRENTE: &str = "carro_muito_lento_frente";

// ───────────────────────────── irsdk_TrkLoc ─────────────────────────────
const SUP_FORA_DO_MUNDO: i32 = -1;
const SUP_FORA_DA_PISTA: i32 = 0;
const SUP_NA_CAIXA: i32 = 1;
const SUP_ENTRANDO_BOX: i32 = 2;
const SUP_NA_PISTA: i32 = 3;

/// `irsdk_SessionState::Racing`.
const ESTADO_CORRIDA: i32 = 4;

/// Janela para derivar a velocidade de um carro (s). Não existe `CarIdxSpeed`; a mesma
/// janela do irmão, pelo mesmo motivo — trocá-la invalidaria a calibração inteira.
const JANELA_VEL_S: f64 = 0.25;

/// Ritmo recente, em baldes rotativos de 1 s. Ver o irmão.
const PICO_JANELA_S: f64 = 10.0;
const PICO_BALDES: usize = 10;

/// Piso do "estava andando" (km/h). É a regra que mata a largada parada, onde os 40
/// carros estão a 0 km/h, em asfalto, na pista, com `SessionState` já em Correndo.
const PICO_MIN_KMH: f64 = 50.0;

/// Abaixo disto o carro é PARADO, e parado é assunto do outro detector (km/h).
///
/// É metade de como as três famílias se excluem, e a metade que é barata: o critério é
/// interno e observável, sem consultar o estado de outro módulo. A outra metade é a
/// superfície — fora da pista também é de lá. O que sobra de sobreposição real é o carro
/// que ACABOU de voltar da grama e ainda está lento: 15% dos avisos daqui são de carros
/// que estiveram fora da pista ou abaixo de 5 km/h nos 10 s anteriores. Não dá para
/// resolver isso aqui sem espiar o estado do irmão; fica para a tabela de prioridade.
const PARADO_KMH: f64 = 5.0;

// ───────────────────── O mapa de ritmo por trecho da volta ─────────────────────

/// Em quantos trechos a volta é dividida. 100 dá 24 m em Lime Rock e 19 m em Okayama —
/// menos que o passo de um carro a 200 km/h entre dois quadros de `cars[]`, que é o que
/// importa: um trecho tem de ser pequeno o bastante para não misturar reta com curva.
const TRECHOS: usize = 100;

/// Quantas travessias cada trecho guarda, e quantas bastam para ele valer de referência.
///
/// 20 travessias com 40 carros são meia volta de campo — recente o bastante para
/// acompanhar pista secando e pneu caindo, longo o bastante para a mediana ser mediana.
/// O piso de 8 é o que segura a volta 1: enquanto um trecho não viu 8 carros, ele não
/// tem opinião, e um trecho sem opinião não gera aviso.
const AMOSTRAS_TRECHO: usize = 20;
const MIN_AMOSTRAS_TRECHO: usize = 8;

/// Quantos carros o campo precisa ter para a mediana das razões valer alguma coisa.
const MIN_CAMPO: usize = 5;

/// Fração do ritmo do campo abaixo da qual o carro é LENTO.
///
/// 0,50 = metade do ritmo que o campo faz naquele ponto da pista naquele instante. Saiu
/// da varredura sobre as duas capturas: 0,45 dá 0,63 aviso por piloto com 19% de inúteis,
/// 0,50 dá 0,82 com **18%**, 0,55 dá 0,96 com 23%, 0,60 dá 1,30 com 30%. O joelho está
/// aqui — daqui para cima o volume sobe e a utilidade cai junto.
const CORTE: f64 = 0.50;

/// A mesma fração para SAIR do episódio — histerese, para o carro não piscar em volta do
/// limiar. Medida: entre 0,02 e 0,30 de margem o número de AVISOS não muda (105 a 107),
/// só o número de episódios registrados. Fica em 0,10 para manter o histórico limpo, e
/// está registrado que ela não é o que segura o ruído.
const CORTE_SAIDA: f64 = 0.60;

/// Abaixo disto o aviso muda de palavra. 0,30 = menos de um terço do ritmo do campo.
/// São 18% dos avisos das duas corridas, e **nenhum** deles chegou sem problema; acima,
/// em 0,35, já aparecem 5% de inúteis. A diferença bruta de velocidade na chegada tem
/// mediana de 64 km/h — é o tipo de fechada que merece a palavra mais forte.
const CORTE_GRAVE: f64 = 0.30;

/// Quanto o carro precisa ficar abaixo do corte antes de o episódio abrir (s).
///
/// Ao contrário do irmão, onde o piso de permanência foi testado e NÃO ajudou, aqui ele
/// é necessário: o sinal é ruidoso por natureza e uma freada forte derruba a razão por um
/// instante. Medido nas duas corridas, com o resto igual: 0 s dá 1,66 aviso por piloto
/// (21% inúteis), 1,0 s dá 0,82 (18%), 2,0 s dá 0,51 (26%). 1,0 s corta metade do volume
/// de graça; 2,0 s começa a cortar caso bom.
const PERMANENCIA_S: f64 = 1.0;

/// A janela de disparo, em tempo até o FECHAMENTO (s) e em distância (m).
///
/// O tempo é `distância / (velocidade com que o vão se fecha)`, não `distância /
/// velocidade do jogador`: um carro lento não fica parado esperando. Ver o item 3 do topo.
///
/// 2–5 s é a mesma faixa do irmão, e a hipótese de que aqui ela pudesse ser mais generosa
/// foi medida e recusada: 2–7 s triplica o volume (1,30 → 3,10 por piloto) e leva os
/// inúteis de 30% a 47%; 3–8 s chega a 57%. O teto de distância nem chega a morder — com
/// a janela de fechamento, 150 m, 200 m e 300 m dão exatamente o mesmo resultado.
const TTA_MIN_S: f64 = 2.0;
const TTA_MAX_S: f64 = 5.0;
const DIST_MAX_M: f64 = 200.0;

/// Piso de distância (m). O irmão não tem um de propósito — lá o piso em TEMPO já cobria
/// o obstáculo que aparece a 3 metros. Aqui não cobre: com fechamento lento, 2 s podem
/// ser 15 metros. Medido, o piso tira 21% dos avisos e baixa os inúteis de 26% para 18%.
const DIST_MIN_M: f64 = 40.0;

/// Salto de `SessionTime` que denuncia replay, rebobinada ou troca de sessão (s).
const SALTO_MAX_S: f64 = 5.0;

/// Quanto um carro pode faltar de `cars[]` antes de contar como sumido (s). Um guinchado
/// some por ~145 s; um quadro perdido é ordens de grandeza menor.
const AUSENCIA_MAX_S: f64 = 1.0;

/// Teto do array de carros do SDK. Casa com `IRSDK_MAX_CARS` em `imp/util.rs`.
const MAX_CARROS: usize = 64;

/// Quantos episódios encerrados o histórico guarda.
const MAX_EPISODIOS: usize = 60;

/// Como o episódio terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Desfecho {
    /// Voltou a andar no ritmo do campo.
    Retomou,
    /// Caiu abaixo de [`PARADO_KMH`] — daqui em diante é assunto da família `Parado`.
    Parou,
    /// Saiu da pista — daqui em diante é assunto da família `Fora`.
    SaiuDaPista,
    /// Entrou no pit road / na caixa.
    FoiParaOBox,
    /// Deixou de aparecer em `cars[]` — guincho, garagem, desconexão.
    SumiuDoMundo,
    /// O jogador passou pelo ponto.
    Ultrapassado,
    /// A sessão deixou de estar em corrida.
    SessaoAcabou,
}

/// Um episódio de carro lento, aberto ou encerrado.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Episodio {
    pub id: u64,
    pub car_idx: i32,
    /// `SessionTime` do instante em que o carro CRUZOU o corte — não do instante em que o
    /// episódio abriu. A diferença é a permanência, e contá-la como parte do episódio é o
    /// que faz a duração registrada descrever o fenômeno em vez de descrever o filtro.
    pub inicio_s: f64,
    pub duracao_s: f64,
    /// Razão contra o ritmo do campo na abertura e a pior durante o episódio. 1,0 = no
    /// ritmo; 0,4 = 40% do que o campo faz naquele ponto.
    pub razao_inicio: f64,
    pub razao_minima: f64,
    /// Ritmo esperado no trecho, na abertura (km/h) — o denominador, guardado para que o
    /// número acima possa ser conferido depois sem reproduzir o mapa inteiro.
    pub referencia_kmh: f64,
    /// Menor velocidade vista durante o episódio (km/h).
    pub minima_kmh: f64,
    /// Pico dos 10 s anteriores à abertura (km/h).
    pub pico_kmh: f64,
    /// Distância com sinal do JOGADOR até este carro na abertura e no encerramento (m):
    /// positiva à frente, negativa atrás.
    pub gap_inicio_m: f64,
    pub gap_fim_m: f64,
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
    pub fn ganho_do_jogador_m(&self) -> f64 {
        self.gap_inicio_m - self.gap_fim_m
    }
}

/// Uma amostra do mundo inteiro, do ponto de vista do spotter de carro lento.
#[derive(Debug, Clone, Copy)]
pub struct AmostraLento<'a> {
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

/// O ritmo típico de um trecho da volta, em travessias recentes.
#[derive(Debug, Clone, Copy)]
struct Trecho {
    amostras: [f64; AMOSTRAS_TRECHO],
    /// Quantas posições valem (satura em [`AMOSTRAS_TRECHO`]).
    n: usize,
    cursor: usize,
}

impl Default for Trecho {
    fn default() -> Self {
        Trecho {
            amostras: [0.0; AMOSTRAS_TRECHO],
            n: 0,
            cursor: 0,
        }
    }
}

impl Trecho {
    /// Uma amostra por TRAVESSIA, nunca por quadro. A diferença não é de eficiência: um
    /// carro parado dentro de um trecho entregaria 20 amostras por segundo de velocidade
    /// zero e afogaria a própria referência que o denunciaria.
    fn registrar(&mut self, kmh: f64) {
        self.amostras[self.cursor] = kmh;
        self.cursor = (self.cursor + 1) % AMOSTRAS_TRECHO;
        self.n = (self.n + 1).min(AMOSTRAS_TRECHO);
    }

    /// A mediana das travessias recentes. `None` enquanto o trecho não viu gente bastante
    /// — e `None` aqui significa "sem opinião", que é diferente de "todo mundo é lento".
    fn referencia_kmh(&self) -> Option<f64> {
        if self.n < MIN_AMOSTRAS_TRECHO {
            return None;
        }
        let mut v = [0.0f64; AMOSTRAS_TRECHO];
        v[..self.n].copy_from_slice(&self.amostras[..self.n]);
        mediana(&mut v[..self.n]).filter(|m| *m > 0.0)
    }
}

/// O que este módulo sabe de um carro.
#[derive(Debug, Clone)]
struct Carro {
    /// `(tempo, pct)` recentes, o bastante para cobrir [`JANELA_VEL_S`].
    hist: VecDeque<(f64, f64)>,
    baldes: [f64; PICO_BALDES],
    balde: usize,
    balde_ate_s: f64,
    vel_kmh: Option<f64>,
    /// Velocidade dividida pelo ritmo esperado do TRECHO em que o carro está. Ainda não é
    /// a razão final: falta dividir pelo fator do campo.
    razao_bruta: Option<f64>,
    /// Onde a JANELA de velocidade começou — a posição de 0,25 s atrás, não a de agora.
    ///
    /// É a posição que dá o trecho, e isso não é detalhe. A velocidade derivada descreve
    /// o pedaço de pista que o carro acabou de percorrer, não o ponto onde ele está; usar
    /// o `pct` atual atribui a um trecho a velocidade do trecho anterior. Nos 19 m de
    /// trecho de Okayama isso não se nota, mas na entrada de uma freada o erro é o
    /// tamanho da freada — e ele **não** se cancela entre a amostra e a razão, porque a
    /// amostra é gravada na entrada do trecho e a razão é calculada no meio dele. Casar
    /// as duas na mesma posição faz o viés, seja ele qual for, sair da divisão.
    base_pct: Option<f64>,
    /// Último trecho visitado, para amostrar o mapa uma vez por travessia.
    trecho: Option<usize>,
    /// Desde quando este carro está abaixo do corte. É o relógio da permanência.
    lento_desde_s: Option<f64>,
    /// `SessionTime` da última amostra em que o carro apareceu em `cars[]`.
    visto_em_s: Option<f64>,
    episodio: Option<Episodio>,
    /// Um episódio fechou e a condição física do carro AINDA é de carro lento.
    ///
    /// Sem isto, todo encerramento que não vem da física reabre no tick seguinte — o caso
    /// que expõe é `Ultrapassado`, que é um fim do ponto de vista do JOGADOR enquanto o
    /// carro segue exatamente no mesmo ritmo. A trava cai na primeira amostra em que o
    /// carro deixa de ser lento: **um episódio novo exige uma volta ao normal no meio.**
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
            razao_bruta: None,
            base_pct: None,
            trecho: None,
            lento_desde_s: None,
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
    /// lento, que é o erro que transformaria toda entrada no mundo em aviso.
    fn atualizar_velocidade(&mut self, tempo_s: f64, pct: f64, comprimento_m: f64) {
        self.hist.push_back((tempo_s, pct));
        while let Some(&(t0, _)) = self.hist.front() {
            if tempo_s - t0 > JANELA_VEL_S * 2.0 {
                self.hist.pop_front();
            } else {
                break;
            }
        }
        let base = self
            .hist
            .iter()
            .find(|&&(t, _)| tempo_s - t >= JANELA_VEL_S)
            .copied();
        self.base_pct = base.map(|(_, p0)| p0);
        self.vel_kmh = base.and_then(|(t0, p0)| {
            let dt = tempo_s - t0;
            if dt <= 0.0 {
                return None;
            }
            let mut d = pct - p0;
            if d < -0.5 {
                d += 1.0;
            }
            if d > 0.5 {
                d -= 1.0;
            }
            Some(d * comprimento_m / dt * 3.6)
        });
    }

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

/// Mediana no lugar. `None` para fatia vazia.
fn mediana(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

/// Distância para a FRENTE, de `de` até `para`, em metros (0..comprimento).
fn adiante(de_pct: f64, para_pct: f64, comprimento_m: f64) -> f64 {
    let mut d = para_pct - de_pct;
    if d < 0.0 {
        d += 1.0;
    }
    d * comprimento_m
}

/// Distância COM SINAL: positiva à frente, negativa atrás, dentro de meia volta.
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

fn trecho_de(pct: f64) -> usize {
    ((pct.fract() + 1.0).fract() * TRECHOS as f64) as usize % TRECHOS
}

/// A máquina. Pura: recebe amostras e devolve, no máximo, uma chave de fala por amostra.
#[derive(Debug)]
pub struct ObservadorLento {
    carros: Vec<Carro>,
    mapa: Vec<Trecho>,
    ultimo_tempo_s: f64,
    proximo_id: u64,
    /// Mediana das razões brutas do campo na última amostra. 1,0 = o campo está no ritmo
    /// do mapa. Cai junto sob amarela, e é ele — não o mapa — que absorve a queda global.
    fator: Option<f64>,
    encerrados: VecDeque<Episodio>,
    recem_encerrados: Vec<Episodio>,
    /// Carro e EPISÓDIO cujo aviso a última amostra devolveu, à espera de confirmação.
    alvo_pendente: Option<(usize, u64)>,
}

impl Default for ObservadorLento {
    fn default() -> Self {
        Self::novo()
    }
}

impl ObservadorLento {
    pub fn novo() -> Self {
        ObservadorLento {
            carros: vec![Carro::default(); MAX_CARROS],
            mapa: vec![Trecho::default(); TRECHOS],
            ultimo_tempo_s: 0.0,
            proximo_id: 1,
            fator: None,
            encerrados: VecDeque::new(),
            recem_encerrados: Vec::new(),
            alvo_pendente: None,
        }
    }

    /// Zera carros E mapa. O mapa vai junto de propósito: um salto de tempo é replay,
    /// rebobinada ou pista nova, e um mapa de ritmo de outra pista é pior que nenhum.
    fn zerar(&mut self) {
        for c in self.carros.iter_mut() {
            c.zerar();
        }
        for t in self.mapa.iter_mut() {
            *t = Trecho::default();
        }
        self.fator = None;
    }

    /// A razão final de um carro: quanto do ritmo do campo ele está fazendo naquele ponto
    /// da pista naquele instante. `None` quando falta mapa ou falta campo.
    fn razao(&self, i: usize) -> Option<f64> {
        let bruta = self.carros[i].razao_bruta?;
        let fator = self.fator?;
        (fator > 0.0).then(|| bruta / fator)
    }

    /// O fator do campo. Ver [`Self::fator`].
    pub fn fator_do_campo(&self) -> Option<f64> {
        self.fator
    }

    /// Uma amostra. Devolve a chave de fala quando um episódio entra na janela de aviso
    /// pela primeira vez.
    ///
    /// Se o aviso não sair nesta amostra (porque o chamador já emitiu algo com mais
    /// prioridade), o episódio continua marcado como não avisado e tenta de novo na
    /// próxima. **Nunca descarta, no máximo adia.**
    pub fn observar(&mut self, a: AmostraLento<'_>) -> Option<&'static str> {
        let salto =
            a.tempo_s < self.ultimo_tempo_s || a.tempo_s - self.ultimo_tempo_s > SALTO_MAX_S;
        self.ultimo_tempo_s = a.tempo_s;
        if salto {
            self.zerar();
            return None;
        }
        if a.comprimento_m <= 0.0 {
            return None;
        }

        let em_corrida = a.estado_sessao == ESTADO_CORRIDA;

        // Passo 1: velocidade e pico de todo mundo, e o mapa de ritmo da pista. Sai antes
        // da detecção e vale mesmo fora de corrida: o mapa precisa estar quente quando a
        // corrida começar, e a regra "estava andando" precisa de passado.
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS {
                continue;
            }
            self.carros[i].visto_em_s = Some(a.tempo_s);
            if c.track_surface == SUP_FORA_DO_MUNDO || c.lap_dist_pct < 0.0 {
                self.carros[i].hist.clear();
                self.carros[i].vel_kmh = None;
                self.carros[i].razao_bruta = None;
                continue;
            }
            self.carros[i].atualizar_velocidade(a.tempo_s, c.lap_dist_pct, a.comprimento_m);
            self.carros[i].atualizar_pico(a.tempo_s);

            // O trecho vem da base da janela, não do `pct` de agora. Ver [`Carro::base_pct`].
            let trecho = self.carros[i].base_pct.map(trecho_de);
            let valido = c.track_surface == SUP_NA_PISTA && !c.on_pit_road;
            match (valido, trecho, self.carros[i].vel_kmh) {
                (true, Some(trecho), Some(v)) => {
                    if self.carros[i].trecho != Some(trecho) {
                        self.carros[i].trecho = Some(trecho);
                        self.mapa[trecho].registrar(v);
                    }
                    self.carros[i].razao_bruta = self.mapa[trecho].referencia_kmh().map(|r| v / r);
                }
                _ => {
                    self.carros[i].trecho = trecho;
                    self.carros[i].razao_bruta = None;
                }
            }
        }

        // Passo 2: o fator do campo — a mediana das razões brutas AGORA.
        //
        // O jogador fica de fora. Não é preciosismo: na captura de Okayama ele rodou a
        // 22 km/h contra um campo de 133 por quase quatro minutos, e num grid pequeno um
        // outlier desses move a mediana. A mediana já é robusta a um; tirar o carro que
        // se sabe suspeito é de graça.
        let mut razoes = [0.0f64; MAX_CARROS];
        let mut n = 0usize;
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS || c.is_player || c.idx == a.jogador_idx {
                continue;
            }
            if let Some(r) = self.carros[i].razao_bruta {
                razoes[n] = r;
                n += 1;
            }
        }
        self.fator = if n >= MIN_CAMPO {
            mediana(&mut razoes[..n])
        } else {
            None
        };

        // Passo 3: abre, mantém e encerra episódios.
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS || c.is_player || c.idx == a.jogador_idx {
                continue;
            }
            self.passo_episodio(c, &a, em_corrida);
        }

        // Passo 3b: quem sumiu do array. O guincho do iRacing não avisa — o carro
        // simplesmente deixa de aparecer em `cars[]`. Sem esta varredura, um episódio
        // aberto num carro guinchado nunca fecharia, porque o laço acima só visita quem
        // está presente.
        self.fechar_ausentes(a.tempo_s);

        // Passo 4: quem merece o rádio.
        if !a.jogador_na_pista || !em_corrida || a.jogador_vel_ms <= 1.0 {
            return None;
        }
        let rp = self.razao(a.jogador_idx as usize)?;
        if rp <= 0.0 {
            return None;
        }
        let mut alvo: Option<(f64, usize, f64)> = None;
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS {
                continue;
            }
            match &self.carros[i].episodio {
                Some(ep) if !ep.avisado => {}
                _ => continue,
            }
            let dist = adiante(a.jogador_pct, c.lap_dist_pct, a.comprimento_m);
            if !(DIST_MIN_M..=DIST_MAX_M).contains(&dist) {
                continue;
            }
            let Some(rc) = self.razao(i) else { continue };
            // O vão se fecha à DIFERENÇA de ritmo, não à velocidade do jogador. E a
            // diferença é medida em ritmo relativo porque os dois carros estão em pontos
            // diferentes da volta: comparar `v_jogador − v_alvo` cru daria fechamento
            // enorme com o jogador na reta e o alvo na curva, sem nada estar acontecendo.
            if rc >= rp {
                continue;
            }
            let fechamento_ms = a.jogador_vel_ms * (1.0 - rc / rp);
            if fechamento_ms <= 0.0 {
                continue;
            }
            let tta = dist / fechamento_ms;
            if !(TTA_MIN_S..=TTA_MAX_S).contains(&tta) {
                continue;
            }
            if alvo.map(|(d, _, _)| dist < d).unwrap_or(true) {
                alvo = Some((dist, i, rc));
            }
        }
        let (_, i, rc) = alvo?;
        // NÃO marca como avisado aqui. Quem sabe se a fala saiu de verdade é o chamador;
        // marcar na detecção transformaria o adiamento em descarte.
        self.alvo_pendente = self.carros[i].episodio.as_ref().map(|ep| (i, ep.id));
        Some(if rc < CORTE_GRAVE {
            CHAVE_MUITO_LENTO_FRENTE
        } else {
            CHAVE_LENTO_FRENTE
        })
    }

    /// O aviso devolvido pela última [`ObservadorLento::observar`] realmente virou fala.
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
    /// A duração é contada até a última vez em que o carro foi VISTO, não até agora — a
    /// diferença entre "ficou 3 s lento e foi guinchado" e "ficou 150 s lento".
    fn fechar_ausentes(&mut self, agora_s: f64) {
        for i in 0..self.carros.len() {
            let Some(visto) = self.carros[i].visto_em_s else {
                continue;
            };
            if agora_s - visto < AUSENCIA_MAX_S {
                continue;
            }
            self.carros[i].aguardando_normalizar = false;
            self.carros[i].lento_desde_s = None;
            let Some(mut ep) = self.carros[i].episodio.take() else {
                continue;
            };
            ep.duracao_s = visto - ep.inicio_s;
            ep.desfecho = Some(Desfecho::SumiuDoMundo);
            self.arquivar(ep);
        }
    }

    fn arquivar(&mut self, ep: Episodio) {
        self.recem_encerrados.push(ep.clone());
        self.encerrados.push_back(ep);
        while self.encerrados.len() > MAX_EPISODIOS {
            self.encerrados.pop_front();
        }
    }

    /// Abre, atualiza ou encerra o episódio de um carro.
    fn passo_episodio(&mut self, c: &CarSnapshot, a: &AmostraLento<'_>, em_corrida: bool) {
        let i = c.idx as usize;
        let vel = self.carros[i].vel_kmh;
        let razao = self.razao(i);
        let gap = com_sinal(a.jogador_pct, c.lap_dist_pct, a.comprimento_m);
        let na_pista = c.track_surface == SUP_NA_PISTA && !c.on_pit_road;

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
            if let Some(r) = razao {
                if r < ep.razao_minima {
                    ep.razao_minima = r;
                }
            }

            // Encerramento. As três famílias se excluem AQUI também, e não só na
            // abertura: um carro lento que para vira notícia da família `Parado`, e um
            // que sai da pista vira da família `Fora`. Deixar o episódio aberto nos dois
            // casos faria o piloto ouvir duas coisas sobre o mesmo carro.
            let desfecho = if !em_corrida {
                Some(Desfecho::SessaoAcabou)
            } else if c.track_surface == SUP_FORA_DO_MUNDO {
                // Inalcançável na prática: o `retain` em `imp/leitura.rs` descarta os
                // carros fora do mundo antes de montar `cars[]`. Quem fecha esses
                // episódios é `fechar_ausentes`. Fica aqui porque a regra é certa se um
                // dia aquele filtro mudar.
                Some(Desfecho::SumiuDoMundo)
            } else if c.on_pit_road
                || c.track_surface == SUP_NA_CAIXA
                || c.track_surface == SUP_ENTRANDO_BOX
            {
                Some(Desfecho::FoiParaOBox)
            } else if c.track_surface == SUP_FORA_DA_PISTA {
                Some(Desfecho::SaiuDaPista)
            } else if vel.map(|v| v < PARADO_KMH).unwrap_or(false) {
                Some(Desfecho::Parou)
            } else if gap < 0.0 && ep.gap_inicio_m > 0.0 {
                Some(Desfecho::Ultrapassado)
            } else if razao.map(|r| r >= CORTE_SAIDA).unwrap_or(false) {
                Some(Desfecho::Retomou)
            } else {
                None
            };
            if let Some(d) = desfecho {
                let mut ep = self.carros[i].episodio.take().expect("acabou de existir");
                ep.desfecho = Some(d);
                self.arquivar(ep);
                self.carros[i].lento_desde_s = None;
                // Fechou, mas o carro pode continuar exatamente na mesma situação — ver
                // [`Carro::aguardando_normalizar`].
                self.carros[i].aguardando_normalizar = true;
            }
            return;
        }

        // Abertura. Só em corrida, só para quem estava andando, só na pista, e só para
        // quem não é assunto das outras duas famílias.
        let lento = em_corrida
            && na_pista
            && self.carros[i].pico_kmh() >= PICO_MIN_KMH
            && vel.map(|v| v >= PARADO_KMH).unwrap_or(false)
            && razao.map(|r| r < CORTE).unwrap_or(false);
        if !lento {
            self.carros[i].lento_desde_s = None;
            self.carros[i].aguardando_normalizar = false;
            return;
        }
        if self.carros[i].aguardando_normalizar {
            return;
        }
        // A permanência. O carro precisa ter ficado abaixo do corte por [`PERMANENCIA_S`]
        // SEGUIDOS: uma freada forte numa curva lenta derruba a razão por um instante, e
        // sem isto cada uma delas seria um episódio.
        let desde = *self.carros[i].lento_desde_s.get_or_insert(a.tempo_s);
        if a.tempo_s - desde < PERMANENCIA_S {
            return;
        }

        let (Some(v), Some(r)) = (vel, razao) else {
            return;
        };
        let referencia = self.carros[i]
            .trecho
            .and_then(|t| self.mapa[t].referencia_kmh())
            .unwrap_or(0.0);
        let id = self.proximo_id;
        self.proximo_id += 1;
        self.carros[i].episodio = Some(Episodio {
            id,
            car_idx: c.idx,
            // O início é quando ele cruzou o corte, não quando o filtro deixou passar.
            inicio_s: desde,
            duracao_s: a.tempo_s - desde,
            razao_inicio: r,
            razao_minima: r,
            referencia_kmh: referencia,
            minima_kmh: v,
            pico_kmh: self.carros[i].pico_kmh(),
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

    /// Retira os episódios fechados na última amostra.
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

    /// Quantos trechos da volta já têm ritmo de referência. Zero = o detector ainda está
    /// cego, e é assim que ele passa a volta 1.
    pub fn trechos_com_referencia(&self) -> usize {
        self.mapa
            .iter()
            .filter(|t| t.referencia_kmh().is_some())
            .count()
    }
}

// ─────────────────────────── O observador global ───────────────────────────

/// Comprimento da pista corrente (m), em bits de `f64`. Zero desliga a detecção inteira —
/// sem escala, "6% da volta" não vira "150 m".
static COMPRIMENTO_M: AtomicU64 = AtomicU64::new(0);

/// Registra o comprimento da pista. Chamado pelo amostrador junto com o resto do que ele
/// já extrai do YAML.
pub fn definir_comprimento_m(m: f64) {
    COMPRIMENTO_M.store(m.to_bits(), Ordering::Relaxed);
}

pub fn comprimento_m() -> f64 {
    f64::from_bits(COMPRIMENTO_M.load(Ordering::Relaxed))
}

fn observador() -> &'static Mutex<ObservadorLento> {
    static OBS: OnceLock<Mutex<ObservadorLento>> = OnceLock::new();
    OBS.get_or_init(|| Mutex::new(ObservadorLento::novo()))
}

fn lock() -> MutexGuard<'static, ObservadorLento> {
    observador().lock().unwrap_or_else(|e| e.into_inner())
}

/// Alimenta o observador global com uma amostra. Devolve a chave de fala, se houver.
pub fn observar(t: &crate::iracing_sdk::IracingTelemetry) -> Option<&'static str> {
    let no_carro = t.on_track && !t.is_replay_playing;
    let jogador = t
        .cars
        .iter()
        .find(|c| c.is_player || c.idx == t.player_car_idx);
    let (chave, encerrados) = {
        let mut obs = lock();
        let chave = obs.observar(AmostraLento {
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
            "spotter_lento",
            &format!(
                "episodio carro={} t={:.1} dur={:.2} razao={:.2} razao_min={:.2} ref={:.0} \
                 min={:.0} pico={:.0} gap_ini={:.0} gap_fim={:.0} pos={}->{} \
                 pos_jogador={}->{} avisado={} desfecho={:?}",
                e.car_idx,
                e.inicio_s,
                e.duracao_s,
                e.razao_inicio,
                e.razao_minima,
                e.referencia_kmh,
                e.minima_kmh,
                e.pico_kmh,
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

/// O aviso da última amostra virou fala. Ver [`ObservadorLento::confirmar_aviso`].
pub fn confirmar_aviso() {
    lock().confirmar_aviso();
}

/// Episódios encerrados — o material de calibração.
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

    /// Okayama Short — a corrida que produziu quase todos os números deste módulo.
    const PISTA: f64 = 1929.0;
    const DT: f64 = 1.0 / 60.0;
    const CARROS: usize = 12;

    /// O perfil de velocidade da pista: reta, freada, curva lenta, retomada, curva média.
    /// É o que separa este detector do que não funciona — contra a mediana instantânea do
    /// campo, TODO carro na curva lenta é um carro lento, e foi assim que a primeira
    /// versão chegou a 150 avisos por piloto por corrida.
    ///
    /// As transições são RAMPAS, não degraus, e isso não é capricho: num degrau, o trecho
    /// que contém a fronteira recebe amostras de 170 e de 60 km/h conforme onde cada
    /// carro estava, e a referência dele deixa de descrever coisa alguma — um carro
    /// perfeitamente normal aparece a 73% do "ritmo do trecho". Pista real freia ao longo
    /// de uma distância; pista de teste com degrau inventa um problema que não existe.
    fn perfil(pct: f64) -> f64 {
        const PONTOS: [(f64, f64); 7] = [
            (0.00, 170.0),
            (0.30, 170.0),
            (0.42, 60.0),
            (0.52, 60.0),
            (0.66, 150.0),
            (0.80, 95.0),
            (1.00, 170.0),
        ];
        let p = (pct.fract() + 1.0).fract();
        let mut i = 1;
        while i < PONTOS.len() - 1 && PONTOS[i].0 < p {
            i += 1;
        }
        let (x0, y0) = PONTOS[i - 1];
        let (x1, y1) = PONTOS[i];
        y0 + (y1 - y0) * ((p - x0) / (x1 - x0))
    }

    /// Um campo de 12 carros dando voltas de verdade, cada um com um multiplicador de
    /// ritmo próprio. O jogador é o índice 0.
    ///
    /// Que todos andem não é conforto: um carro congelado num `pct` fixo é, por definição
    /// do detector, um carro PARADO — outra família —, e um campo congelado não alimenta
    /// o mapa de ritmo, sem o qual nada aqui existe.
    struct Cena {
        obs: ObservadorLento,
        t: f64,
        pct: [f64; CARROS],
        ritmo: [f64; CARROS],
        sup: [i32; CARROS],
        pit: [bool; CARROS],
        presente: [bool; CARROS],
        estado: i32,
        jog_na_pista: bool,
        avisos: usize,
        chaves: Vec<&'static str>,
        dist_no_aviso: Vec<f64>,
        /// Confirmar o aviso é o que o chamador real faz quando a fala sai de fato.
        /// Desligar isto simula o tick em que uma entrada lateral roubou a vez.
        confirma: bool,
    }

    impl Cena {
        /// Os 12 carros espalhados pela volta, todos no ritmo da pista.
        fn nova() -> Self {
            let mut pct = [0.0; CARROS];
            for (i, p) in pct.iter_mut().enumerate() {
                *p = i as f64 / CARROS as f64;
            }
            Cena {
                obs: ObservadorLento::novo(),
                t: 0.0,
                pct,
                ritmo: [1.0; CARROS],
                sup: [SUP_NA_PISTA; CARROS],
                pit: [false; CARROS],
                presente: [true; CARROS],
                estado: ESTADO_CORRIDA,
                jog_na_pista: true,
                avisos: 0,
                chaves: Vec::new(),
                dist_no_aviso: Vec::new(),
                confirma: true,
            }
        }

        fn dist_ate(&self, i: usize) -> f64 {
            adiante(self.pct[0], self.pct[i], PISTA)
        }

        fn passo(&mut self) {
            let carros: Vec<CarSnapshot> = (0..CARROS)
                .filter(|&i| self.presente[i])
                .map(|i| CarSnapshot {
                    idx: i as i32,
                    is_player: i == 0,
                    lap_dist_pct: self.pct[i],
                    track_surface: self.sup[i],
                    track_surface_material: 1,
                    on_pit_road: self.pit[i],
                    position: i as i32 + 1,
                    ..Default::default()
                })
                .collect();
            let d = self.dist_ate(1);
            let chave = self.obs.observar(AmostraLento {
                tempo_s: self.t,
                estado_sessao: self.estado,
                comprimento_m: PISTA,
                jogador_idx: 0,
                jogador_pct: self.pct[0],
                jogador_vel_ms: perfil(self.pct[0]) * self.ritmo[0] / 3.6,
                jogador_posicao: 1,
                jogador_na_pista: self.jog_na_pista,
                carros: &carros,
            });
            if let Some(chave) = chave {
                if self.confirma {
                    self.obs.confirmar_aviso();
                }
                self.avisos += 1;
                self.chaves.push(chave);
                self.dist_no_aviso.push(d);
            }
            for i in 0..CARROS {
                if !self.presente[i] {
                    continue;
                }
                let v = perfil(self.pct[i]) * self.ritmo[i];
                self.pct[i] = (self.pct[i] + (v / 3.6) * DT / PISTA).fract();
            }
            self.t += DT;
        }

        fn rodar(&mut self, segundos: f64) {
            let mut restante = segundos;
            while restante > 0.0 {
                self.passo();
                restante -= DT;
            }
        }

        /// Uma volta inteira de campo — o bastante para todo trecho ver 8 carros e o mapa
        /// de ritmo passar a ter opinião.
        fn aquecer(&mut self) {
            self.rodar(70.0);
            assert_eq!(
                self.obs.trechos_com_referencia(),
                TRECHOS,
                "o mapa de ritmo não aqueceu"
            );
        }
    }

    #[test]
    fn o_campo_inteiro_no_ritmo_da_pista_nao_gera_episodio() {
        // O teste que a primeira versão deste detector reprovava. Doze carros perfeitos,
        // e a curva lenta a 60 km/h contra uma reta a 170: contra a mediana instantânea
        // do campo, cada passagem pela curva é um "carro lento".
        let mut c = Cena::nova();
        c.aquecer();
        c.rodar(60.0);
        assert!(
            c.obs.abertos().is_empty(),
            "a curva lenta virou {} episódios",
            c.obs.abertos().len()
        );
        assert!(c.obs.encerrados().is_empty());
        assert_eq!(c.avisos, 0);
    }

    #[test]
    fn um_carro_a_45_por_cento_do_ritmo_abre_episodio_e_fala() {
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.45;
        c.rodar(15.0);
        let ep = c
            .obs
            .encerrados()
            .into_iter()
            .chain(c.obs.abertos())
            .find(|e| e.car_idx == 1)
            .expect("nenhum episódio para o carro lento");
        assert!(
            (0.40..0.50).contains(&ep.razao_inicio),
            "razão {:.2}",
            ep.razao_inicio
        );
        assert!(ep.pico_kmh > PICO_MIN_KMH);
        assert_eq!(c.chaves, vec![CHAVE_LENTO_FRENTE]);
    }

    #[test]
    fn um_carro_a_um_quinto_do_ritmo_muda_a_palavra() {
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.20;
        c.rodar(15.0);
        assert_eq!(
            c.chaves,
            vec![CHAVE_MUITO_LENTO_FRENTE],
            "o grau tem de sair da razão medida"
        );
    }

    #[test]
    fn o_campo_inteiro_desacelerando_junto_nao_gera_episodio() {
        // A amarela. Medido nas capturas: o fator do campo fica em 0,99 sob amarela
        // contra 1,00 no verde — a queda global de 30% do ritmo é absorvida inteira, e é
        // por isso que o corte não pode ser absoluto.
        let mut c = Cena::nova();
        c.aquecer();
        for r in c.ritmo.iter_mut() {
            *r = 0.60;
        }
        c.rodar(30.0);
        assert!(
            c.obs.abertos().is_empty() && c.obs.encerrados().is_empty(),
            "a amarela virou {} episódios",
            c.obs.abertos().len() + c.obs.encerrados().len()
        );
        assert_eq!(c.avisos, 0);
        let fator = c.obs.fator_do_campo().unwrap();
        assert!((fator - 0.60).abs() < 0.15, "fator {fator:.2}");
    }

    #[test]
    fn a_largada_parada_nao_vira_doze_carros_lentos() {
        // A armadilha que o `SessionState` não pega: o grid a 0 km/h, em asfalto, na
        // pista, com o estado já em Correndo.
        let mut c = Cena::nova();
        for (i, p) in c.pct.iter_mut().enumerate() {
            *p = 0.98 + i as f64 * 0.0001;
        }
        for r in c.ritmo.iter_mut() {
            *r = 0.0;
        }
        c.rodar(10.0);
        assert!(
            c.obs.abertos().is_empty(),
            "o grid parado virou {} episódios",
            c.obs.abertos().len()
        );
    }

    #[test]
    fn carro_parado_e_da_familia_parado() {
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.0;
        c.rodar(10.0);
        assert!(
            !c.obs.abertos().iter().any(|e| e.car_idx == 1),
            "carro parado abriu episódio de LENTO"
        );
        // E se estava lento antes de parar, o episódio fecha com o desfecho certo.
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.40;
        c.rodar(3.0);
        assert!(c.obs.abertos().iter().any(|e| e.car_idx == 1));
        c.ritmo[1] = 0.0;
        c.rodar(2.0);
        assert_eq!(
            c.obs.encerrados().last().unwrap().desfecho,
            Some(Desfecho::Parou)
        );
    }

    #[test]
    fn carro_fora_da_pista_e_da_familia_fora() {
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.40;
        c.sup[1] = SUP_FORA_DA_PISTA;
        c.rodar(10.0);
        assert!(
            !c.obs.abertos().iter().any(|e| e.car_idx == 1),
            "carro fora da pista abriu episódio de LENTO"
        );
        assert_eq!(c.avisos, 0);
    }

    #[test]
    fn carro_no_pit_road_nao_e_carro_lento() {
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.30;
        c.pit[1] = true;
        c.rodar(10.0);
        assert!(c.obs.abertos().is_empty());
        assert_eq!(c.avisos, 0);
        // E um episódio já aberto fecha ao entrar no box.
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.40;
        c.rodar(3.0);
        c.pit[1] = true;
        c.rodar(0.5);
        assert_eq!(
            c.obs.encerrados().last().unwrap().desfecho,
            Some(Desfecho::FoiParaOBox)
        );
    }

    #[test]
    fn uma_freada_isolada_nao_abre_episodio() {
        // A permanência. Meio segundo abaixo do corte é uma travada de roda, não um carro
        // com problema — e é exatamente isso que o sinal contínuo produz o tempo todo.
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.30;
        c.rodar(0.6);
        c.ritmo[1] = 1.0;
        c.rodar(3.0);
        assert!(
            c.obs.abertos().is_empty() && c.obs.encerrados().is_empty(),
            "a freada de 0,6 s virou episódio"
        );
    }

    #[test]
    fn o_episodio_conta_desde_o_cruzamento_do_corte_e_nao_desde_a_abertura() {
        // Se a duração começasse a contar no fim da permanência, todo episódio nasceria
        // um segundo mais curto do que foi — e a duração é um dos números que este
        // registro existe para medir.
        let mut c = Cena::nova();
        c.aquecer();
        let t0 = c.t;
        c.ritmo[1] = 0.40;
        c.rodar(4.0);
        let ep = c.obs.abertos().into_iter().find(|e| e.car_idx == 1).unwrap();
        assert!(
            (ep.inicio_s - t0).abs() < 0.5,
            "início {:.2} contra {:.2} — a permanência foi descontada",
            ep.inicio_s,
            t0
        );
        assert!(ep.duracao_s > 3.0, "duração {:.2}", ep.duracao_s);
    }

    #[test]
    fn o_aviso_sai_com_dois_a_cinco_segundos_de_fechamento() {
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.45;
        c.rodar(15.0);
        assert_eq!(c.avisos, 1, "esperava um aviso, saíram {}", c.avisos);
        let d = c.dist_no_aviso[0];
        assert!(
            (DIST_MIN_M..=DIST_MAX_M).contains(&d),
            "avisou a {d:.0} m, fora da faixa"
        );
    }

    #[test]
    fn nao_avisa_quando_o_jogador_nao_esta_fechando() {
        // O jogador tão lento quanto o alvo. O episódio existe — o carro está lento para
        // o campo —, mas o jogador nunca vai encontrá-lo, e um aviso ali é ruído puro.
        // É esta a diferença entre `distância / fechamento` e `distância / velocidade`.
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.45;
        c.ritmo[0] = 0.45;
        c.rodar(20.0);
        assert!(
            c.obs.abertos().iter().any(|e| e.car_idx == 1),
            "o episódio devia existir de qualquer forma"
        );
        assert_eq!(c.avisos, 0, "avisou de um carro que não vai alcançar");
    }

    #[test]
    fn avisa_uma_vez_so_por_episodio() {
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.45;
        c.rodar(40.0);
        assert_eq!(c.avisos, 1, "o mesmo episódio falou {} vezes", c.avisos);
    }

    #[test]
    fn um_aviso_que_nao_virou_fala_volta_no_tick_seguinte() {
        // O tick em que uma entrada lateral rouba a vez. O aviso não pode sumir por isso
        // — precisa continuar pendente e sair assim que houver espaço.
        let mut c = Cena::nova();
        c.aquecer();
        c.confirma = false;
        c.ritmo[1] = 0.45;
        c.rodar(15.0);
        assert!(
            c.avisos > 1,
            "sem confirmação o aviso deveria insistir, saiu {} vez(es)",
            c.avisos
        );
    }

    #[test]
    fn ultrapassar_encerra_o_episodio_e_nao_reabre_no_tick_seguinte() {
        // `Ultrapassado` é um fim do ponto de vista do JOGADOR: o carro continua no mesmo
        // ritmo. Sem a trava de normalização nasceriam centenas de episódios idênticos.
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.30;
        c.rodar(40.0);
        let encerrados = c.obs.encerrados();
        assert!(
            encerrados.iter().any(|e| e.desfecho == Some(Desfecho::Ultrapassado)),
            "desfechos: {:?}",
            encerrados.iter().map(|e| e.desfecho).collect::<Vec<_>>()
        );
        assert!(
            encerrados.len() <= 2,
            "reabriu {} vezes depois da ultrapassagem",
            encerrados.len()
        );
    }

    #[test]
    fn retomar_o_ritmo_encerra_o_episodio() {
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.40;
        c.rodar(3.0);
        c.ritmo[1] = 1.0;
        c.rodar(3.0);
        assert_eq!(
            c.obs.encerrados().last().unwrap().desfecho,
            Some(Desfecho::Retomou)
        );
        assert!(c.obs.abertos().is_empty());
    }

    #[test]
    fn o_carro_que_some_do_array_fecha_com_a_duracao_certa() {
        // A assinatura do guincho, medida em Okayama: o carro some de `cars[]` por
        // dezenas de segundos e reaparece no box. `NotInWorld` não chega até aqui.
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.40;
        c.rodar(3.0);
        assert!(c.obs.abertos().iter().any(|e| e.car_idx == 1));
        let visto_por_ultimo = c.t;
        c.presente[1] = false;
        c.rodar(60.0);
        assert!(
            !c.obs.abertos().iter().any(|e| e.car_idx == 1),
            "carro lento eterno"
        );
        let ep = c
            .obs
            .encerrados()
            .into_iter()
            .rev()
            .find(|e| e.car_idx == 1)
            .unwrap();
        assert_eq!(ep.desfecho, Some(Desfecho::SumiuDoMundo));
        assert!(
            (ep.duracao_s - (visto_por_ultimo - ep.inicio_s)).abs() < 0.2,
            "duração {:.1}s — contou o tempo de ausência",
            ep.duracao_s
        );
    }

    #[test]
    fn um_quadro_perdido_nao_parte_o_episodio_em_dois() {
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.40;
        c.rodar(3.0);
        c.presente[1] = false;
        c.rodar(0.1);
        c.presente[1] = true;
        c.rodar(1.0);
        assert_eq!(c.obs.abertos().len(), 1, "o engasgo fechou o episódio");
        assert!(c.obs.encerrados().is_empty());
    }

    #[test]
    fn sem_mapa_de_ritmo_nada_e_detectado() {
        // A volta 1. Enquanto um trecho não viu 8 carros ele não tem opinião, e sem
        // opinião não há razão nem aviso — que é como este detector atravessa a largada.
        let mut c = Cena::nova();
        c.ritmo[1] = 0.30;
        c.rodar(5.0);
        assert!(c.obs.trechos_com_referencia() < TRECHOS);
        assert!(c.obs.abertos().is_empty());
        assert_eq!(c.avisos, 0);
    }

    #[test]
    fn sem_comprimento_de_pista_nada_e_detectado() {
        let mut obs = ObservadorLento::novo();
        let carros: Vec<CarSnapshot> = (0..CARROS)
            .map(|i| CarSnapshot {
                idx: i as i32,
                is_player: i == 0,
                lap_dist_pct: i as f64 / CARROS as f64,
                track_surface: SUP_NA_PISTA,
                position: i as i32 + 1,
                ..Default::default()
            })
            .collect();
        assert_eq!(
            obs.observar(AmostraLento {
                tempo_s: 1.0,
                estado_sessao: ESTADO_CORRIDA,
                comprimento_m: 0.0,
                jogador_idx: 0,
                jogador_pct: 0.0,
                jogador_vel_ms: 40.0,
                jogador_posicao: 1,
                jogador_na_pista: true,
                carros: &carros,
            }),
            None
        );
        assert_eq!(obs.trechos_com_referencia(), 0);
    }

    #[test]
    fn fora_de_corrida_nao_abre_episodio() {
        let mut c = Cena::nova();
        c.aquecer();
        c.estado = 5; // Bandeirada
        c.ritmo[1] = 0.30;
        c.rodar(5.0);
        assert!(c.obs.abertos().is_empty());
        assert_eq!(c.avisos, 0);
    }

    #[test]
    fn a_bandeirada_encerra_o_que_estava_aberto() {
        let mut c = Cena::nova();
        c.aquecer();
        c.ritmo[1] = 0.40;
        c.rodar(3.0);
        assert_eq!(c.obs.abertos().len(), 1);
        c.estado = 5;
        c.rodar(0.2);
        assert_eq!(
            c.obs.encerrados().last().unwrap().desfecho,
            Some(Desfecho::SessaoAcabou)
        );
    }

    #[test]
    fn um_salto_de_tempo_reinicia_sem_inventar_episodio() {
        let mut c = Cena::nova();
        c.aquecer();
        c.t += 600.0;
        c.ritmo[1] = 0.30;
        c.rodar(0.5);
        assert!(c.obs.abertos().is_empty());
        assert_eq!(c.avisos, 0);
        // E o mapa de ritmo vai junto: um mapa de outra pista é pior que nenhum.
        assert_eq!(c.obs.trechos_com_referencia(), 0);
    }

    #[test]
    fn com_o_jogador_no_box_o_detector_registra_mas_nao_fala() {
        let mut c = Cena::nova();
        c.aquecer();
        c.jog_na_pista = false;
        c.sup[0] = SUP_NA_CAIXA;
        c.ritmo[1] = 0.40;
        c.rodar(5.0);
        assert_eq!(c.avisos, 0);
        assert_eq!(
            c.obs.abertos().len(),
            1,
            "a captura de dados não pode depender de o jogador estar correndo"
        );
    }

    #[test]
    fn o_jogador_lento_nao_arrasta_a_referencia_do_campo() {
        // O caso de Okayama: o jogador rodou a 22 km/h contra um campo de 133 por quase
        // quatro minutos. Com campo pequeno, deixá-lo dentro da mediana move o fator o
        // bastante para calar o detector — aqui, seis carros válidos com o jogador a 20%
        // e dois a 45% dariam fator 0,73, e 0,45/0,73 = 0,62, acima do corte.
        let mut c = Cena::nova();
        c.aquecer();
        for i in 6..CARROS {
            c.presente[i] = false;
        }
        c.ritmo[0] = 0.20;
        c.ritmo[4] = 0.45;
        c.ritmo[5] = 0.45;
        c.rodar(6.0);
        let fator = c.obs.fator_do_campo().unwrap();
        assert!(
            (fator - 1.0).abs() < 0.1,
            "fator {fator:.2} — o jogador entrou na mediana"
        );
        assert!(
            c.obs.abertos().iter().any(|e| e.car_idx == 4),
            "o carro lento sumiu porque o jogador afundou a referência"
        );
    }
}
