//! Spotter — carro chegando por TRÁS.
//!
//! O terceiro lado do spotter. O LADO vem pronto do SDK (`CarLeftRight`), a FRENTE é
//! deduzida em [`crate::iracing_sdk::spotter_frente`], e atrás não havia nada — foi
//! exatamente ali que aconteceu o único acidente do acervo com telemetria completa. Em
//! Okayama o jogador saiu do box e rodou 42 s a 17–53 km/h com o campo a 133; o `#37`,
//! num ritmo perfeitamente normal, veio por trás e o acertou. Aviso não faltava: dezoito
//! segundos antes ele estava a 578 m, fechando a 28 m/s.
//!
//! **A armadilha é o projeto inteiro.** Nos 57 s em que o jogador rodava lento, DEZESSETE
//! carros diferentes entraram na janela de 2 a 5 s por trás dele. Um aviso por carro seria
//! uma metralhadora — pior que silêncio —, e espelhar o detector de obstáculo é a solução
//! errada que vem primeiro à cabeça. O que a situação pede é um ESTADO SUSTENTADO, da
//! mesma forma do spotter lateral: entra uma vez, lembra de vez em quando enquanto dura,
//! e libera quando o trem passa.
//!
//! Três coisas que os dados forçaram e que não estavam na aposta inicial:
//!
//! 1. **A velocidade crua não separa nada.** Dois carros em pontos diferentes do traçado
//!    têm velocidades muito diferentes sem que ninguém esteja alcançando ninguém: medida
//!    sobre Okayama, a taxa de fechamento instantânea de quem vem atrás tem p90 de
//!    10 m/s. Quem normaliza o traçado é o `CarIdxEstTime` — ele é o tempo de referência
//!    da volta NA POSIÇÃO do carro, então `d(est_time)/dt` já é "quanto do ritmo de
//!    referência este carro está fazendo", e a curva sai da conta.
//! 2. **Referência absoluta também não serve**, porque metade daquela corrida foi sob
//!    amarela e sob amarela todo mundo é lento. O ritmo tem de ser comparado com a
//!    MEDIANA DO CAMPO no mesmo instante. As duas normalizações juntas apertam o ruído
//!    por quinze vezes: em Lime Rock, uma corrida limpa, a fração cai abaixo de 0,6 em
//!    0,20% das amostras-carro contra 3,15% da versão por velocidade.
//! 3. **A saída do box é o falso positivo de verdade**, não a largada. Toda parada produz
//!    um carro a 45 km/h com o campo a 170, e são quarenta por corrida. O que separa a
//!    saída do box do acidente é o TEMPO: um carro que acabou de parar volta ao ritmo em
//!    ~4 s, e o de Okayama ficou abaixo de 60% do campo por 238 s seguidos.
//!
//! Números medidos sobre as duas capturas, com cada carro da IA simulado como jogador:
//! **3 estados em 82 pilotos-corrida** — 0,02 a 0,05 por piloto por corrida —, e os três
//! são as duas escapadas de verdade de Lime Rock (as mesmas que o `spotter_frente` acha
//! do outro lado, vistas de dentro do carro que escapou) mais o acidente de Okayama.
//! Ver `docs/spotter-frentes/frente-a-tras.md`.

use std::collections::VecDeque;

use serde::Serialize;

use crate::iracing_sdk::spotter_base::{
    adiante, saltou, spotter_singleton, AUSENCIA_MAX_S, ESTADO_CORRIDA, MAX_CARROS,
    SUP_ENTRANDO_BOX, SUP_FORA_DA_PISTA, SUP_NA_CAIXA, SUP_NA_PISTA,
};
use crate::iracing_sdk::CarSnapshot;

/// ENTRADA — vem gente por trás, e vem mais rápido.
pub const CHAVE_TRAFEGO_ATRAS: &str = "carro_atras";
/// LEMBRETE — ainda está vindo. Só sai com alguém de fato na janela de chegada.
pub const CHAVE_LEMBRETE_ATRAS: &str = "ainda_atras";
/// LIBERAÇÃO — o trem passou.
pub const CHAVE_LIVRE_ATRAS: &str = "livre_atras";

// As superfícies do `irsdk_TrkLoc` e o estado de corrida vivem em
// [`crate::iracing_sdk::spotter_base`]: são contrato do SDK, não calibração deste módulo.

// ───────────────────────────── irsdk_Flags ─────────────────────────────
/// Bandeira azul: "vem um carro muito mais rápido, deixe passar". É literalmente o
/// resumo dos dezessete carros num sinal só, e é para isso que ela existe.
const BANDEIRA_AZUL: i32 = 0x20;
/// Amarela em qualquer forma (local, agitada, `caution`, `caution` agitada).
const BANDEIRAS_AMARELA: i32 = 0x8 | 0x100 | 0x4000 | 0x8000;

/// Janela de suavização do ritmo (s).
///
/// Longa de propósito, e o mecanismo importa: o ritmo sai de um ACUMULADOR (quanto o
/// carro andou e quanto de tempo de referência ele consumiu na janela), não de uma
/// derivada entre duas amostras. A diferença aparece quando `cars[]` chega a 20 Hz e o
/// amostrador roda a 60: repetir a mesma leitura três vezes zeraria uma derivada
/// instantânea, e não muda um deslocamento acumulado.
const JANELA_RITMO_S: f64 = 3.0;

/// Abaixo de que fração da mediana do campo o jogador conta como lento, e acima de que
/// fração ele conta como recuperado. A folga entre as duas é a histerese que impede a
/// condição de piscar em cima do limiar.
const FRACAO_LENTO: f64 = 0.60;
const FRACAO_RETOMOU: f64 = 0.75;

/// Por quanto tempo seguido o jogador precisa estar lento (s).
///
/// É este número, e não um limiar de velocidade, que separa a saída do box do acidente.
/// Medido em Lime Rock, a 0,60 da mediana: com 3 s de espera saem 19 estados, com 4 s
/// saem 3, e a diferença é toda saída de box. Subir mais custa caro do outro lado — a 6 s
/// a escapada do `#13`, que ficou 4 s fora da pista, deixa de ser vista.
const CONFIRMA_S: f64 = 4.0;

/// Quanto a azul precisa se manter para valer (s). Ela pisca: em Okayama ligou em 899,6,
/// desligou em 901,7 e voltou em 903,5.
const CONFIRMA_AZUL_S: f64 = 0.5;

/// Silêncio depois de deixar a via de box (s).
///
/// Sair do box devagar com o campo a 170 é o mesmo quadro do acidente, e acontece uma vez
/// por parada para cada um dos quarenta carros. O piloto que acabou de sair do box sabe
/// que está devagar; avisá-lo disso é a definição de ruído. Dez segundos cobrem a
/// aceleração de volta e ainda deixam a folga que o caso de Okayama pede: lá o aviso saiu
/// 13,5 s depois de o jogador deixar a via de box.
const SAIDA_BOX_S: f64 = 10.0;

/// Até onde atrás um carro ainda é assunto (m) e em quanto tempo ele precisa chegar (s).
/// A faixa útil medida para a frente — 100–200 m, 2 a 5 s — vale igual aqui; o teto de
/// tempo é um pouco mais largo porque o estado descreve uma situação e não um ponto.
const DIST_MAX_M: f64 = 200.0;
const TTA_MAX_S: f64 = 6.0;

/// Fechamento mínimo para o carro de trás contar como CHEGANDO (m/s).
///
/// Um carro dois segundos atrás numa briga de posição não é notícia — quando ele chegar
/// ao lado, o spotter lateral fala. Notícia é quem vem muito mais rápido. Este piso é
/// baixo de propósito: quem carrega a separação é a fração de ritmo, e um piso alto aqui
/// só cortaria o caso do jogador quase parado, em que qualquer um "vem muito mais rápido".
const FECHAMENTO_MIN_MS: f64 = 3.0;

/// Sem ninguém chegando por tanto tempo, o trem passou (s).
const LIBERA_S: f64 = 8.0;

/// Cadência do lembrete: primeiro intervalo, quanto cresce e onde para (s).
///
/// Mais larga que a do spotter lateral, e por um motivo: lá a disputa dura segundos, aqui
/// o estado durou 22 s no caso real e pode durar minutos. A regra que segura o volume não
/// é o relógio e sim a condição — o lembrete só sai com alguém DE FATO na janela de
/// chegada, então um vão no tráfego é silêncio, não metrônomo.
const LEMBRETE_S: f64 = 5.0;
const LEMBRETE_PASSO_S: f64 = 2.5;
const LEMBRETE_MAX_S: f64 = 12.0;

/// Piso de ritmo do CAMPO para a comparação valer (m/s) e quantos carros a mediana exige.
///
/// É a tradução da regra "só conta quem estava andando" para um detector que compara com
/// o campo: no verde da largada parada os quarenta carros estão a 0 km/h, em asfalto, na
/// pista, com o estado já em corrida — e ali "mais lento que a mediana" não quer dizer
/// nada porque a mediana é zero.
const CAMPO_MIN_MS: f64 = 8.0;
const CAMPO_MIN_CARROS: usize = 5;

/// Quantos estados encerrados o histórico guarda.
const MAX_EPISODIOS: usize = 40;

/// De onde veio a entrada no estado.
///
/// Uma família só, duas portas. A ação do piloto é a MESMA nas duas — segura a linha,
/// não defende, deixa passar —, e dar dois nomes ao mesmo conselho só dividiria um evento
/// já raro em dois mais raros ainda. A origem fica registrada porque as duas não são
/// mensuráveis do mesmo jeito: a azul só existe no `session_flags`, que é do JOGADOR, e
/// portanto não pode ser simulada carro a carro sobre uma captura. Guardá-la separada é o
/// que vai permitir responder, na próxima gravação, se ela alguma vez viu algo que a
/// geometria não viu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrigemTras {
    /// O jogador está muito mais lento que o campo e alguém está chegando.
    Ritmo,
    /// Bandeira azul confirmada, com alguém de fato na janela de chegada.
    Azul,
}

/// Como o estado terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FimTras {
    /// Ninguém mais chegando por [`LIBERA_S`]. **É o único fim que vira fala** — os
    /// outros são o mundo mudando de assunto, e anunciar "livre atrás" depois de uma
    /// bandeirada ou de um guincho seria responder a uma pergunta que ninguém fez.
    TremPassou,
    /// O jogador entrou na via de box, saiu do carro ou o replay assumiu.
    SaiuDaPista,
    /// Amarela. Sob amarela ninguém passa ninguém, e "deixa passar" vira instrução errada.
    Amarela,
    /// A sessão deixou de estar em corrida.
    SessaoAcabou,
    /// O jogador sumiu de `cars[]` — guincho (ver [`AUSENCIA_MAX_S`]).
    SumiuDoMundo,
}

/// Um estado de tráfego por trás, aberto ou encerrado. É o material de calibração.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodioTras {
    pub id: u64,
    pub origem: OrigemTras,
    /// `SessionTime` da abertura.
    pub inicio_s: f64,
    /// Duração até o encerramento; enquanto aberto, o que já durou.
    pub duracao_s: f64,
    /// Fração da mediana do campo em que o jogador estava na abertura.
    pub fracao_entrada: f64,
    /// Quem estava chegando na abertura, a que distância (m) e em quanto tempo (s).
    pub perseguidor_idx: i32,
    pub perseguidor_m: f64,
    pub perseguidor_s: f64,
    /// Quantos carros DIFERENTES entraram na janela de chegada durante o estado.
    ///
    /// É o número que justifica a forma deste detector existir: em Okayama foram
    /// dezessete em 57 s. Se um dia ele vier perto de 1, a máquina de estado está
    /// sobrando e um aviso por carro bastaria.
    pub carros_na_janela: usize,
    /// Falas confirmadas: entrada mais lembretes. A liberação não entra na conta porque
    /// ela sai depois do fechamento (ver [`FimTras::TremPassou`]).
    pub falas: usize,
    /// `None` enquanto aberto.
    pub fim: Option<FimTras>,
}

/// O aviso que a última amostra decidiu e que ainda não virou fala.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pendente {
    Entrada,
    Lembrete,
    Liberacao,
}

impl Pendente {
    fn chave(self) -> &'static str {
        match self {
            Pendente::Entrada => CHAVE_TRAFEGO_ATRAS,
            Pendente::Lembrete => CHAVE_LEMBRETE_ATRAS,
            Pendente::Liberacao => CHAVE_LIVRE_ATRAS,
        }
    }
}

/// Uma amostra do mundo inteiro, do ponto de vista do spotter de trás.
#[derive(Debug, Clone, Copy)]
pub struct AmostraTras<'a> {
    pub tempo_s: f64,
    /// `SessionState`.
    pub estado_sessao: i32,
    /// `SessionFlags` GLOBAL. É o canal do jogador: a azul e a amarela vivem aqui, e
    /// nenhuma das duas chega pelo `CarIdxSessionFlags` (medido em Okayama).
    pub bandeiras: i32,
    /// Comprimento da pista (m). Zero = desconhecido → nada é detectado.
    pub comprimento_m: f64,
    pub jogador_idx: i32,
    /// O piloto está no carro, no mundo, sem replay rodando. O resto da condição
    /// (superfície, via de box) sai do `CarSnapshot` dele.
    pub jogador_no_carro: bool,
    pub carros: &'a [CarSnapshot],
}

/// O que este módulo sabe de um carro: dois acumuladores e quando ele foi visto.
#[derive(Debug, Clone, Default)]
struct Carro {
    /// `(tempo, metros andados, tempo de referência consumido)`, cobrindo a janela.
    hist: VecDeque<(f64, f64, f64)>,
    pct_ant: Option<f64>,
    est_ant: Option<f64>,
    metros: f64,
    referencia: f64,
    visto_em_s: Option<f64>,
}

impl Carro {
    fn zerar(&mut self) {
        *self = Carro::default();
    }

    fn atualizar(&mut self, tempo_s: f64, pct: f64, est_time: f64, comprimento_m: f64) {
        if let Some(anterior) = self.pct_ant {
            let mut d = pct - anterior;
            // Cruzou a linha de chegada entre as duas amostras.
            if d < -0.5 {
                d += 1.0;
            }
            if d > 0.5 {
                d -= 1.0;
            }
            self.metros += d * comprimento_m;
        }
        self.pct_ant = Some(pct);

        if est_time > 0.0 {
            if let Some(anterior) = self.est_ant {
                let d = est_time - anterior;
                // O `est_time` volta a zero na linha, e é ele que dá a escala da volta —
                // que não conhecemos. Em vez de adivinhar o tempo de referência da volta
                // para desfazer o salto, a amostra da virada é simplesmente descartada:
                // custa um quadro por volta numa janela de três segundos.
                if (0.0..=5.0).contains(&d) {
                    self.referencia += d;
                }
            }
            self.est_ant = Some(est_time);
        }

        self.hist.push_back((tempo_s, self.metros, self.referencia));
        while let Some(&(t0, _, _)) = self.hist.front() {
            if tempo_s - t0 > JANELA_RITMO_S * 1.5 {
                self.hist.pop_front();
            } else {
                break;
            }
        }
    }

    /// `(velocidade média em m/s, ritmo em frações do tempo de referência)` sobre a
    /// janela. `None` até haver histórico bastante — e `None` NÃO é zero: um carro que
    /// acabou de aparecer não pode ser confundido com um carro parado.
    fn medias(&self, tempo_s: f64) -> Option<(f64, f64)> {
        // A amostra MAIS NOVA que já cobre a janela, procurando do fim para o começo. Pela
        // outra ponta a janela efetiva seria a do buffer inteiro, e a folga de histórico
        // (que existe para o caso de quadros irregulares) viraria atraso de reação — cada
        // meio segundo a mais aqui é meio segundo a mais para o aviso sair.
        let base = self
            .hist
            .iter()
            .rev()
            .find(|&&(t, _, _)| tempo_s - t >= JANELA_RITMO_S)
            .copied()?;
        let (t0, m0, r0) = base;
        let (t1, m1, r1) = *self.hist.back()?;
        let dt = t1 - t0;
        if dt <= 0.0 {
            return None;
        }
        Some(((m1 - m0) / dt, (r1 - r0) / dt))
    }
}

/// O carro está num lugar onde o tráfego de corrida passa por ele.
///
/// A grama CONTA — quem está fora da pista precisa mais do aviso, não menos. Quem não
/// conta é a via de box, e `on_pit_road` sozinho não a cobre: na saída do box de Okayama
/// o jogador passou 20 s com `on_pit_road = false` e `track_surface == ApproachingPits`.
fn na_pista(c: &CarSnapshot) -> bool {
    (c.track_surface == SUP_NA_PISTA || c.track_surface == SUP_FORA_DA_PISTA) && !c.on_pit_road
}

/// O carro está na via de box (caixa, aproximação ou pit road).
fn na_via_de_box(c: &CarSnapshot) -> bool {
    c.on_pit_road || c.track_surface == SUP_NA_CAIXA || c.track_surface == SUP_ENTRANDO_BOX
}

/// Quem está chegando por trás.
#[derive(Debug, Clone, Copy)]
struct Perseguidor {
    idx: i32,
    distancia_m: f64,
    chega_em_s: f64,
}

/// O estado aberto.
#[derive(Debug)]
struct Ativo {
    ep: EpisodioTras,
    /// A entrada já virou fala. Enquanto for `false` ela continua sendo devolvida a cada
    /// amostra — nunca descarta, no máximo adia.
    entrada_anunciada: bool,
    ultimo_anuncio_s: f64,
    intervalo_s: f64,
    /// Quem já apareceu na janela de chegada durante este estado.
    vistos: [bool; MAX_CARROS],
}

/// A máquina. Pura: recebe amostras e devolve, no máximo, uma chave de fala por amostra.
#[derive(Debug)]
pub struct ObservadorTras {
    carros: Vec<Carro>,
    ultimo_tempo_s: f64,
    proximo_id: u64,
    ativo: Option<Ativo>,
    /// Desde quando o jogador está abaixo de [`FRACAO_LENTO`].
    lento_desde_s: Option<f64>,
    /// Desde quando a azul está acesa.
    azul_desde_s: Option<f64>,
    /// Última vez que o jogador esteve na via de box.
    via_de_box_em_s: Option<f64>,
    /// Última vez que havia alguém chegando.
    ultimo_perseguidor_s: Option<f64>,
    pendente: Option<Pendente>,
    encerrados: VecDeque<EpisodioTras>,
    recem_encerrados: Vec<EpisodioTras>,
}

impl Default for ObservadorTras {
    fn default() -> Self {
        Self::novo()
    }
}

impl ObservadorTras {
    pub fn novo() -> Self {
        ObservadorTras {
            carros: vec![Carro::default(); MAX_CARROS],
            ultimo_tempo_s: 0.0,
            proximo_id: 1,
            ativo: None,
            lento_desde_s: None,
            azul_desde_s: None,
            via_de_box_em_s: None,
            ultimo_perseguidor_s: None,
            pendente: None,
            encerrados: VecDeque::new(),
            recem_encerrados: Vec::new(),
        }
    }

    fn zerar(&mut self) {
        for c in self.carros.iter_mut() {
            c.zerar();
        }
        self.ativo = None;
        self.lento_desde_s = None;
        self.azul_desde_s = None;
        self.via_de_box_em_s = None;
        self.ultimo_perseguidor_s = None;
        self.pendente = None;
    }

    /// Uma amostra. Devolve a chave de fala pendente, se houver.
    ///
    /// A chave continua sendo devolvida a cada amostra até que
    /// [`ObservadorTras::confirmar_aviso`] diga que ela virou fala de verdade. Um aviso
    /// que some porque o lateral ganhou o tick é o defeito que este projeto já cometeu
    /// uma vez: nunca descarta, no máximo adia.
    pub fn observar(&mut self, a: AmostraTras<'_>) -> Option<&'static str> {
        let salto = saltou(self.ultimo_tempo_s, a.tempo_s);
        self.ultimo_tempo_s = a.tempo_s;
        if salto {
            self.zerar();
            return None;
        }
        if a.comprimento_m <= 0.0 {
            return None;
        }

        // Passo 1: os acumuladores de todo mundo. Fora da condição de corrida também —
        // uma janela de 3 s sem histórico reprovaria o campo inteiro no primeiro tick.
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS {
                continue;
            }
            self.carros[i].visto_em_s = Some(a.tempo_s);
            if c.lap_dist_pct < 0.0 {
                continue;
            }
            self.carros[i].atualizar(a.tempo_s, c.lap_dist_pct, c.est_time, a.comprimento_m);
        }

        // Passo 2: o jogador sumiu de `cars[]`? É assim que o guincho se manifesta — o
        // array encolhe, o índice deixa de existir, e um laço que só visita quem está
        // presente deixaria o estado aberto para sempre.
        let jidx = a.jogador_idx as usize;
        if jidx < MAX_CARROS {
            if let Some(visto) = self.carros[jidx].visto_em_s {
                if a.tempo_s - visto >= AUSENCIA_MAX_S {
                    self.fechar(FimTras::SumiuDoMundo, visto);
                }
            }
        }

        let Some(jogador) = a
            .carros
            .iter()
            .find(|c| c.is_player || c.idx == a.jogador_idx)
        else {
            return self.pendente.map(Pendente::chave);
        };
        if na_via_de_box(jogador) {
            self.via_de_box_em_s = Some(a.tempo_s);
        }

        // Passo 3: o campo está correndo? A mediana é sobre quem está na pista e fora do
        // box — é ela que define "ritmo de corrida agora", e é ela que faz a comparação
        // sobreviver a uma amarela, à chuva e a um traçado lento.
        let (mediana_ritmo, mediana_ms) = self.mediana_do_campo(a.carros, a.tempo_s);
        let amarela = a.bandeiras & BANDEIRAS_AMARELA != 0;
        let campo_em_ritmo = mediana_ritmo > 0.05 && mediana_ms >= CAMPO_MIN_MS;

        // Passo 4: o mundo ainda comporta o estado?
        let fim = if a.estado_sessao != ESTADO_CORRIDA {
            Some(FimTras::SessaoAcabou)
        } else if amarela {
            Some(FimTras::Amarela)
        } else if !a.jogador_no_carro || !na_pista(jogador) {
            Some(FimTras::SaiuDaPista)
        } else {
            None
        };
        if let Some(fim) = fim {
            self.fechar(fim, a.tempo_s);
            self.lento_desde_s = None;
            self.azul_desde_s = None;
            self.ultimo_perseguidor_s = None;
            return self.pendente.map(Pendente::chave);
        }
        if !campo_em_ritmo {
            // O campo não está em ritmo (largada parada, fila de amarela que acabou de
            // limpar). Sem mediana não há comparação; o estado aberto continua, porque
            // quem o fecha é o tráfego ter passado e não a mediana ter oscilado.
            self.lento_desde_s = None;
            return self.pendente.map(Pendente::chave);
        }

        // Passo 5: onde o jogador está em relação ao campo, e quem vem vindo.
        let Some((jogador_ms, jogador_ritmo)) = self.carros[jidx].medias(a.tempo_s) else {
            return self.pendente.map(Pendente::chave);
        };
        let fracao = jogador_ritmo / mediana_ritmo;
        if fracao < FRACAO_LENTO {
            self.lento_desde_s.get_or_insert(a.tempo_s);
        } else if fracao >= FRACAO_RETOMOU {
            self.lento_desde_s = None;
        }
        if a.bandeiras & BANDEIRA_AZUL != 0 {
            self.azul_desde_s.get_or_insert(a.tempo_s);
        } else {
            self.azul_desde_s = None;
        }

        let perseguidor = self.perseguidor(&a, jogador, jogador_ms);
        if perseguidor.is_some() {
            self.ultimo_perseguidor_s = Some(a.tempo_s);
        }

        // Passo 6: abrir, lembrar ou liberar.
        match self.ativo.as_mut() {
            None => {
                let Some(p) = perseguidor else {
                    return self.pendente.map(Pendente::chave);
                };
                let sustentado = |desde: Option<f64>, espera: f64| {
                    desde.map(|d| a.tempo_s - d >= espera).unwrap_or(false)
                };
                let origem = if sustentado(self.lento_desde_s, CONFIRMA_S) {
                    OrigemTras::Ritmo
                } else if sustentado(self.azul_desde_s, CONFIRMA_AZUL_S) {
                    OrigemTras::Azul
                } else {
                    return self.pendente.map(Pendente::chave);
                };
                // A saída do box é o único falso positivo com volume nos dados, e é o
                // piloto que menos precisa ser informado de que está devagar.
                if let Some(box_em) = self.via_de_box_em_s {
                    if a.tempo_s - box_em < SAIDA_BOX_S {
                        return self.pendente.map(Pendente::chave);
                    }
                }
                self.abrir(a.tempo_s, origem, fracao, p);
            }
            Some(ativo) => {
                ativo.ep.duracao_s = a.tempo_s - ativo.ep.inicio_s;
                if let Some(p) = perseguidor {
                    let i = p.idx as usize;
                    if i < MAX_CARROS && !ativo.vistos[i] {
                        ativo.vistos[i] = true;
                        ativo.ep.carros_na_janela += 1;
                    }
                }
                let vazio = self
                    .ultimo_perseguidor_s
                    .map(|t| a.tempo_s - t >= LIBERA_S)
                    .unwrap_or(false);
                if vazio {
                    self.fechar(FimTras::TremPassou, a.tempo_s);
                } else if !ativo.entrada_anunciada {
                    self.pendente = Some(Pendente::Entrada);
                } else if perseguidor.is_some()
                    && a.tempo_s - ativo.ultimo_anuncio_s >= ativo.intervalo_s
                {
                    self.pendente = Some(Pendente::Lembrete);
                }
            }
        }

        self.pendente.map(Pendente::chave)
    }

    /// O aviso devolvido pela última [`ObservadorTras::observar`] virou fala.
    pub fn confirmar_aviso(&mut self) {
        let Some(pendente) = self.pendente.take() else {
            return;
        };
        let agora = self.ultimo_tempo_s;
        let Some(ativo) = self.ativo.as_mut() else {
            // Era a liberação: o estado já fechou e a fala saiu. Nada a marcar.
            return;
        };
        match pendente {
            Pendente::Entrada => {
                ativo.entrada_anunciada = true;
                ativo.ultimo_anuncio_s = agora;
                ativo.ep.falas += 1;
            }
            Pendente::Lembrete => {
                ativo.ultimo_anuncio_s = agora;
                ativo.intervalo_s = (ativo.intervalo_s + LEMBRETE_PASSO_S).min(LEMBRETE_MAX_S);
                ativo.ep.falas += 1;
            }
            Pendente::Liberacao => {}
        }
    }

    fn abrir(&mut self, tempo_s: f64, origem: OrigemTras, fracao: f64, p: Perseguidor) {
        let id = self.proximo_id;
        self.proximo_id += 1;
        let mut vistos = [false; MAX_CARROS];
        let i = p.idx as usize;
        if i < MAX_CARROS {
            vistos[i] = true;
        }
        self.ativo = Some(Ativo {
            ep: EpisodioTras {
                id,
                origem,
                inicio_s: tempo_s,
                duracao_s: 0.0,
                fracao_entrada: fracao,
                perseguidor_idx: p.idx,
                perseguidor_m: p.distancia_m,
                perseguidor_s: p.chega_em_s,
                carros_na_janela: 1,
                falas: 0,
                fim: None,
            },
            entrada_anunciada: false,
            ultimo_anuncio_s: tempo_s,
            intervalo_s: LEMBRETE_S,
            vistos,
        });
        self.pendente = Some(Pendente::Entrada);
    }

    /// Encerra o estado aberto. `ate_s` é quando ele de fato acabou, que não é sempre
    /// "agora": no guincho a duração é contada até a última vez em que o jogador foi
    /// visto, senão os segundos de ausência viram permanência.
    fn fechar(&mut self, fim: FimTras, ate_s: f64) {
        let Some(mut ativo) = self.ativo.take() else {
            return;
        };
        ativo.ep.duracao_s = ate_s - ativo.ep.inicio_s;
        ativo.ep.fim = Some(fim);
        self.recem_encerrados.push(ativo.ep.clone());
        self.encerrados.push_back(ativo.ep);
        while self.encerrados.len() > MAX_EPISODIOS {
            self.encerrados.pop_front();
        }
        // A liberação só vale se a chegada chegou a ser anunciada — a simetria do spotter
        // lateral, em que toda chegada anunciada tem uma saída anunciada, e nada mais que
        // isso. E só o trem ter passado é notícia: "livre atrás" depois de uma bandeirada
        // ou de um guincho responde a uma pergunta que ninguém fez.
        //
        // Um pendente que ainda não virou fala é substituído aqui, e isto é o oposto de
        // descartar por cadência: a notícia anterior deixou de ser verdade. Anunciar
        // "carro chegando" depois que o carro passou seria pior que o silêncio.
        self.pendente = if fim == FimTras::TremPassou && ativo.entrada_anunciada {
            Some(Pendente::Liberacao)
        } else {
            None
        };
    }

    /// Mediana do ritmo e da velocidade do campo — só quem está na pista, fora do box.
    fn mediana_do_campo(&self, carros: &[CarSnapshot], tempo_s: f64) -> (f64, f64) {
        let mut ritmos: Vec<f64> = Vec::with_capacity(carros.len());
        let mut velocidades: Vec<f64> = Vec::with_capacity(carros.len());
        for c in carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS || c.track_surface != SUP_NA_PISTA || c.on_pit_road {
                continue;
            }
            if let Some((ms, ritmo)) = self.carros[i].medias(tempo_s) {
                velocidades.push(ms);
                ritmos.push(ritmo);
            }
        }
        if ritmos.len() < CAMPO_MIN_CARROS {
            return (0.0, 0.0);
        }
        let mediana = |v: &mut Vec<f64>| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v[v.len() / 2]
        };
        (mediana(&mut ritmos), mediana(&mut velocidades))
    }

    /// Quem está chegando por trás, mais cedo. `None` quando não há ninguém na janela.
    fn perseguidor(
        &self,
        a: &AmostraTras<'_>,
        jogador: &CarSnapshot,
        jogador_ms: f64,
    ) -> Option<Perseguidor> {
        let mut melhor: Option<Perseguidor> = None;
        for c in a.carros {
            let i = c.idx as usize;
            if i >= MAX_CARROS || c.idx == jogador.idx || c.is_player {
                continue;
            }
            if !na_pista(c) {
                continue;
            }
            let Some((ms, _)) = self.carros[i].medias(a.tempo_s) else {
                continue;
            };
            let fechamento = ms - jogador_ms;
            if fechamento < FECHAMENTO_MIN_MS {
                continue;
            }
            let distancia_m = adiante(c.lap_dist_pct, jogador.lap_dist_pct, a.comprimento_m);
            if distancia_m <= 0.0 || distancia_m > DIST_MAX_M {
                continue;
            }
            let chega_em_s = distancia_m / fechamento;
            if chega_em_s > TTA_MAX_S {
                continue;
            }
            if melhor.map(|m| chega_em_s < m.chega_em_s).unwrap_or(true) {
                melhor = Some(Perseguidor {
                    idx: c.idx,
                    distancia_m,
                    chega_em_s,
                });
            }
        }
        melhor
    }

    /// Retira os estados fechados desde a última chamada.
    pub fn drenar_encerrados(&mut self) -> Vec<EpisodioTras> {
        std::mem::take(&mut self.recem_encerrados)
    }

    /// Estados encerrados, mais antigo primeiro.
    pub fn encerrados(&self) -> Vec<EpisodioTras> {
        self.encerrados.iter().cloned().collect()
    }

    /// O estado aberto AGORA, se houver.
    pub fn aberto(&self) -> Option<EpisodioTras> {
        self.ativo.as_ref().map(|a| a.ep.clone())
    }
}

// ─────────────────────────── O observador global ───────────────────────────

spotter_singleton!(ObservadorTras, ObservadorTras::novo());

/// Alimenta o observador global com uma amostra. Devolve a chave de fala, se houver.
///
/// O comprimento da pista vem de [`crate::iracing_sdk::spotter_frente::comprimento_m`] de
/// propósito: é o mesmo número, lido do mesmo YAML, e duplicar o registro criaria uma
/// segunda fonte para alguém esquecer de alimentar. Zero desliga a detecção inteira —
/// sem escala, "6% da volta" não vira "150 m".
pub fn observar(t: &crate::iracing_sdk::IracingTelemetry) -> Option<&'static str> {
    // Um lock só para as duas coisas. Com dois, um segundo amostrador poderia entrar entre
    // eles e drenar os episódios que esta chamada acabou de fechar — o diagnóstico sairia
    // atribuído ao tique errado. Hoje o amostrador é uma thread só e não acontece; a
    // assimetria com `spotter_frente`, que já fazia certo, é que não tinha defesa.
    let (chave, encerrados) = {
        let mut obs = lock();
        let chave = obs.observar(AmostraTras {
            tempo_s: t.session_time,
            estado_sessao: t.session_state,
            bandeiras: t.session_flags,
            comprimento_m: crate::iracing_sdk::spotter_frente::comprimento_m(),
            jogador_idx: t.player_car_idx,
            jogador_no_carro: t.on_track && !t.is_replay_playing,
            carros: &t.cars,
        });
        (chave, obs.drenar_encerrados())
    };
    // Fora do lock. O histórico em memória morre com o processo, e a corrida que produz o
    // dado de calibração é justamente a que o perderia. Formato chave=valor de propósito,
    // para um script extrair sem depender de prosa.
    for e in encerrados {
        crate::diagnostico::linha(
            "spotter_tras",
            &format!(
                "estado origem={:?} t={:.1} dur={:.2} fracao={:.2} perseguidor={} \
                 perseguidor_m={:.0} perseguidor_s={:.1} carros_na_janela={} falas={} fim={:?}",
                e.origem,
                e.inicio_s,
                e.duracao_s,
                e.fracao_entrada,
                e.perseguidor_idx,
                e.perseguidor_m,
                e.perseguidor_s,
                e.carros_na_janela,
                e.falas,
                e.fim,
            ),
        );
    }
    chave
}

/// O aviso da última amostra virou fala. Ver [`ObservadorTras::confirmar_aviso`].
pub fn confirmar_aviso() {
    lock().confirmar_aviso();
}

/// Estados encerrados — o material de calibração.
pub fn encerrados() -> Vec<EpisodioTras> {
    lock().encerrados()
}

/// O estado aberto agora.
pub fn aberto() -> Option<EpisodioTras> {
    lock().aberto()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iracing_sdk::spotter_base::SALTO_MAX_S;

    /// Okayama Short — a pista da corrida que produziu este detector.
    const PISTA: f64 = 1929.0;
    const DT: f64 = 1.0 / 60.0;

    /// O perfil de referência da pista: quanto tempo a volta ideal leva até cada ponto.
    ///
    /// `DuasMetades` existe por causa da armadilha que definiu o desenho: a metade lenta
    /// de um traçado faz um carro em ritmo NORMAL parecer 50% mais devagar que o campo se
    /// a comparação for por velocidade. É o `est_time` que desfaz isso, e é aqui que a
    /// desfeita é verificada.
    #[derive(Clone, Copy)]
    enum Perfil {
        Plano,
        DuasMetades,
    }

    impl Perfil {
        /// Velocidade de referência (m/s) naquele ponto da volta.
        fn vel_ref_ms(self, pct: f64) -> f64 {
            match self {
                Perfil::Plano => 36.0,
                Perfil::DuasMetades if pct < 0.5 => 50.0,
                Perfil::DuasMetades => 25.0,
            }
        }

        /// `CarIdxEstTime` naquele ponto: o tempo de referência acumulado desde a linha.
        fn est_time(self, pct: f64) -> f64 {
            match self {
                Perfil::Plano => pct * PISTA / 36.0,
                Perfil::DuasMetades if pct < 0.5 => pct * PISTA / 50.0,
                Perfil::DuasMetades => 0.5 * PISTA / 50.0 + (pct - 0.5) * PISTA / 25.0,
            }
        }
    }

    /// Um carro do mundo de teste. `ritmo` é a fração do ritmo de referência — 1,0 é o
    /// ritmo do campo, 0,3 é o jogador de Okayama.
    #[derive(Clone, Copy)]
    struct Carrinho {
        idx: i32,
        pct: f64,
        ritmo: f64,
        superficie: i32,
        no_box: bool,
    }

    /// O mundo a 60 Hz com carros que DE FATO andam.
    ///
    /// Que eles andem não é conforto: um carro congelado num `pct` fixo tem ritmo zero e
    /// puxa a mediana do campo para baixo, o que faria o teste medir outra coisa.
    struct Cena {
        obs: ObservadorTras,
        t: f64,
        carros: Vec<Carrinho>,
        perfil: Perfil,
        estado: i32,
        bandeiras: i32,
        no_carro: bool,
        /// Confirmar é o que o chamador real faz quando a fala sai. Desligar simula o
        /// tick em que uma entrada lateral roubou a vez.
        confirma: bool,
        falas: Vec<(f64, &'static str)>,
    }

    impl Cena {
        /// Jogador (idx 0) em 25% da volta e `n` carros do campo espalhados atrás dele.
        fn nova(n: usize, perfil: Perfil) -> Self {
            let mut carros = vec![Carrinho {
                idx: 0,
                pct: 0.25,
                ritmo: 1.0,
                superficie: SUP_NA_PISTA,
                no_box: false,
            }];
            for i in 0..n {
                carros.push(Carrinho {
                    idx: i as i32 + 1,
                    // Espalhados pela volta inteira, como um campo de corrida de verdade:
                    // se sobrasse meia volta vazia, um jogador muito lento acabaria com um
                    // vão de tráfego artificial atrás dele e o estado fecharia sozinho.
                    pct: (0.25 - (i as f64 + 1.0) / (n as f64 + 1.0)).rem_euclid(1.0),
                    ritmo: 1.0,
                    superficie: SUP_NA_PISTA,
                    no_box: false,
                });
            }
            Cena {
                obs: ObservadorTras::novo(),
                t: 0.0,
                carros,
                perfil,
                estado: ESTADO_CORRIDA,
                bandeiras: 0,
                no_carro: true,
                confirma: true,
                falas: Vec::new(),
            }
        }

        fn jogador(&mut self) -> &mut Carrinho {
            &mut self.carros[0]
        }

        fn ritmo_do_jogador(&mut self, r: f64) {
            self.carros[0].ritmo = r;
        }

        /// Coloca o carro `idx` a `metros` atrás do jogador. **Só antes de rodar**: depois
        /// que o observador já viu o carro, mudar o `pct` na mão é um teleporte, e o
        /// acumulador de deslocamento leria isso como velocidade.
        fn atras(&mut self, idx: usize, metros: f64) {
            assert_eq!(self.t, 0.0, "posicionar carro depois de rodar é teleporte");
            let pct = (self.carros[0].pct - metros / PISTA).rem_euclid(1.0);
            self.carros[idx].pct = pct;
        }

        /// Roda até o estado abrir, no máximo `limite` segundos. As esperas deste módulo
        /// somam a janela de suavização à confirmação, e cravar o instante exato em cada
        /// teste faria todos eles quebrarem junto quando qualquer uma das duas mudasse.
        fn rodar_ate_abrir(&mut self, limite: f64) {
            let fim = self.t + limite;
            while self.t < fim && self.obs.aberto().is_none() {
                self.rodar(0.25);
            }
        }

        fn chaves(&self) -> Vec<&'static str> {
            self.falas.iter().map(|(_, c)| *c).collect()
        }

        fn conta(&self, chave: &str) -> usize {
            self.falas.iter().filter(|(_, c)| *c == chave).count()
        }

        fn rodar(&mut self, segundos: f64) {
            let mut restante = segundos;
            while restante > 0.0 {
                let snaps: Vec<CarSnapshot> = self
                    .carros
                    .iter()
                    .map(|c| CarSnapshot {
                        idx: c.idx,
                        is_player: c.idx == 0,
                        lap_dist_pct: c.pct,
                        est_time: self.perfil.est_time(c.pct),
                        track_surface: c.superficie,
                        on_pit_road: c.no_box,
                        position: c.idx + 1,
                        ..Default::default()
                    })
                    .collect();
                let chave = self.obs.observar(AmostraTras {
                    tempo_s: self.t,
                    estado_sessao: self.estado,
                    bandeiras: self.bandeiras,
                    comprimento_m: PISTA,
                    jogador_idx: 0,
                    jogador_no_carro: self.no_carro,
                    carros: &snaps,
                });
                if let Some(chave) = chave {
                    if self.confirma {
                        self.obs.confirmar_aviso();
                    }
                    self.falas.push((self.t, chave));
                }
                for c in self.carros.iter_mut() {
                    let v = self.perfil.vel_ref_ms(c.pct) * c.ritmo;
                    c.pct = (c.pct + v * DT / PISTA).rem_euclid(1.0);
                }
                self.t += DT;
                restante -= DT;
            }
        }

        /// Seis segundos de corrida normal — enche a janela de ritmo de todo mundo.
        fn aquecer(&mut self) {
            self.rodar(6.0);
        }
    }

    #[test]
    fn o_jogador_lento_com_alguem_chegando_abre_o_estado() {
        // O quadro de Okayama: campo em ritmo, jogador a 30% dele, tráfego atrás.
        let mut c = Cena::nova(10, Perfil::Plano);
        c.aquecer();
        c.ritmo_do_jogador(0.30);
        c.rodar_ate_abrir(15.0);
        assert_eq!(c.chaves().first(), Some(&CHAVE_TRAFEGO_ATRAS));
        let aberto = c.obs.aberto().expect("o estado deveria estar aberto");
        assert_eq!(aberto.origem, OrigemTras::Ritmo);
        assert!(
            aberto.fracao_entrada < FRACAO_LENTO,
            "fração {:.2}",
            aberto.fracao_entrada
        );
    }

    #[test]
    fn dezessete_carros_nao_viram_dezessete_avisos() {
        // A armadilha que define o módulo. O jogador arrasta a 25% do campo enquanto o
        // campo inteiro passa por ele; um detector por carro falaria uma vez por carro.
        let mut c = Cena::nova(17, Perfil::Plano);
        c.aquecer();
        c.ritmo_do_jogador(0.25);
        c.rodar(60.0);
        assert_eq!(
            c.conta(CHAVE_TRAFEGO_ATRAS),
            1,
            "a entrada saiu {} vezes: {:?}",
            c.conta(CHAVE_TRAFEGO_ATRAS),
            c.chaves()
        );
        let aberto = c.obs.aberto().expect("o estado deveria seguir aberto");
        assert!(
            aberto.carros_na_janela >= 8,
            "só {} carros passaram pela janela — o cenário não reproduz o caso real",
            aberto.carros_na_janela
        );
        // Um minuto de tráfego passando não pode virar um minuto de rádio.
        assert!(
            c.falas.len() <= 8,
            "{} falas em 60 s: {:?}",
            c.falas.len(),
            c.chaves()
        );
    }

    #[test]
    fn disputa_normal_nao_e_noticia() {
        // Carro dois segundos atrás, no mesmo ritmo. Quando ele chegar ao lado, quem fala
        // é o spotter lateral.
        let mut c = Cena::nova(10, Perfil::Plano);
        c.atras(1, 70.0);
        c.carros[1].ritmo = 1.02;
        c.aquecer();
        c.rodar(30.0);
        assert!(c.falas.is_empty(), "falou: {:?}", c.chaves());
    }

    #[test]
    fn o_perfil_da_pista_nao_vira_falso_positivo() {
        // A metade lenta do traçado põe o jogador a 90 km/h com metade do campo a 180.
        // Por velocidade crua ele "perdeu 50% do ritmo"; pelo `est_time` ele está em
        // ritmo, e é o `est_time` que manda.
        let mut c = Cena::nova(12, Perfil::DuasMetades);
        c.aquecer();
        c.rodar(90.0);
        assert!(
            c.falas.is_empty(),
            "o traçado virou aviso: {:?}",
            c.chaves()
        );
    }

    #[test]
    fn a_saida_do_box_nao_vira_aviso() {
        // Quarenta carros por corrida saem do box a 45 km/h com o campo a 170. O piloto
        // que acabou de parar sabe que está devagar.
        let mut c = Cena::nova(10, Perfil::Plano);
        c.aquecer();
        c.jogador().superficie = SUP_ENTRANDO_BOX;
        c.jogador().no_box = true;
        c.ritmo_do_jogador(0.25);
        c.rodar(3.0);
        c.jogador().superficie = SUP_NA_PISTA;
        c.jogador().no_box = false;
        // Acelerando de volta: quatro segundos lento e depois em ritmo, como nos dados.
        c.rodar(4.0);
        c.ritmo_do_jogador(1.0);
        c.rodar(6.0);
        assert!(c.falas.is_empty(), "a saída do box falou: {:?}", c.chaves());
    }

    #[test]
    fn ficar_lento_por_um_instante_nao_basta() {
        let mut c = Cena::nova(10, Perfil::Plano);
        c.atras(1, 120.0);
        c.aquecer();
        c.ritmo_do_jogador(0.30);
        c.rodar(2.0);
        c.ritmo_do_jogador(1.0);
        c.rodar(10.0);
        assert!(c.falas.is_empty(), "falou: {:?}", c.chaves());
    }

    #[test]
    fn o_grid_parado_nao_vira_aviso() {
        // `SessionState == Correndo` não protege da largada parada: os carros estão a
        // 0 km/h, em asfalto, na pista, com o estado já em corrida. Quem resolve é o piso
        // de ritmo do CAMPO — sem mediana não existe "mais lento que o campo".
        let mut c = Cena::nova(20, Perfil::Plano);
        for carro in c.carros.iter_mut() {
            carro.ritmo = 0.0;
            carro.pct = 0.98 + carro.idx as f64 * 0.0002;
        }
        c.rodar(10.0);
        assert!(c.falas.is_empty(), "o grid falou: {:?}", c.chaves());
        assert!(c.obs.aberto().is_none());
    }

    #[test]
    fn sob_amarela_o_estado_nao_abre_e_o_aberto_fecha() {
        let mut c = Cena::nova(10, Perfil::Plano);
        c.aquecer();
        c.ritmo_do_jogador(0.30);
        c.rodar_ate_abrir(15.0);
        assert!(c.obs.aberto().is_some(), "o estado deveria ter aberto");
        c.bandeiras = 0x4000;
        c.rodar(0.5);
        assert!(c.obs.aberto().is_none(), "a amarela não fechou o estado");
        assert_eq!(
            c.obs.encerrados().last().unwrap().fim,
            Some(FimTras::Amarela)
        );
        // E sob amarela ele não volta a abrir, por mais lento que o jogador esteja.
        let antes = c.falas.len();
        c.rodar(20.0);
        assert_eq!(c.falas.len(), antes, "abriu sob amarela: {:?}", c.chaves());
    }

    #[test]
    fn a_liberacao_sai_quando_o_trem_passa() {
        let mut c = Cena::nova(6, Perfil::Plano);
        c.atras(1, 260.0);
        c.aquecer();
        c.ritmo_do_jogador(0.30);
        c.rodar_ate_abrir(20.0);
        assert_eq!(c.chaves().first(), Some(&CHAVE_TRAFEGO_ATRAS));
        // Ninguém mais vem: o jogador volta ao ritmo e o gap para de fechar.
        c.ritmo_do_jogador(1.0);
        c.rodar(LIBERA_S + 3.0);
        assert_eq!(c.chaves().last(), Some(&CHAVE_LIVRE_ATRAS));
        assert_eq!(
            c.obs.encerrados().last().unwrap().fim,
            Some(FimTras::TremPassou)
        );
    }

    #[test]
    fn o_lembrete_so_sai_com_alguem_chegando() {
        // Um vão no tráfego é silêncio, não metrônomo: é isso que impede um estado de um
        // minuto de virar quinze falas.
        let mut c = Cena::nova(6, Perfil::Plano);
        c.atras(1, 240.0);
        c.aquecer();
        c.ritmo_do_jogador(0.30);
        c.rodar_ate_abrir(20.0);
        assert_eq!(c.conta(CHAVE_TRAFEGO_ATRAS), 1);
        // O carro passa e some da janela; ainda faltam segundos para a liberação.
        c.ritmo_do_jogador(1.0);
        let antes = c.falas.len();
        c.rodar(LIBERA_S - 2.0);
        assert_eq!(
            c.falas.len(),
            antes,
            "lembrete sem ninguém chegando: {:?}",
            c.chaves()
        );
    }

    #[test]
    fn um_aviso_que_nao_virou_fala_volta_no_tick_seguinte() {
        let mut c = Cena::nova(10, Perfil::Plano);
        c.aquecer();
        c.confirma = false;
        c.ritmo_do_jogador(0.30);
        c.rodar(12.0);
        assert!(
            c.conta(CHAVE_TRAFEGO_ATRAS) > 1,
            "sem confirmação a entrada deveria insistir, saiu {} vez(es)",
            c.conta(CHAVE_TRAFEGO_ATRAS)
        );
        // E uma confirmação basta para a entrada parar de insistir.
        c.confirma = true;
        c.rodar(0.1);
        let depois = c.conta(CHAVE_TRAFEGO_ATRAS);
        c.rodar(1.0);
        assert_eq!(c.conta(CHAVE_TRAFEGO_ATRAS), depois);
    }

    #[test]
    fn a_bandeira_azul_abre_o_estado_com_o_jogador_em_ritmo() {
        // O caso que a geometria não vê: o jogador está no ritmo dele, e quem chega é o
        // líder uma volta à frente.
        let mut c = Cena::nova(10, Perfil::Plano);
        c.atras(1, 240.0);
        c.carros[1].ritmo = 1.6;
        c.aquecer();
        c.bandeiras = BANDEIRA_AZUL;
        c.rodar_ate_abrir(6.0);
        assert_eq!(c.chaves().first(), Some(&CHAVE_TRAFEGO_ATRAS));
        assert_eq!(c.obs.aberto().unwrap().origem, OrigemTras::Azul);
    }

    #[test]
    fn o_pisca_da_azul_nao_fecha_o_estado() {
        // Em Okayama a azul ligou em 899,6, desligou em 901,7 e voltou em 903,5. Quem
        // fecha o estado é o tráfego ter passado, nunca a bandeira ter piscado.
        let mut c = Cena::nova(10, Perfil::Plano);
        c.atras(1, 240.0);
        c.carros[1].ritmo = 1.6;
        c.aquecer();
        c.bandeiras = BANDEIRA_AZUL;
        c.rodar_ate_abrir(6.0);
        assert!(c.obs.aberto().is_some());
        c.bandeiras = 0;
        c.rodar(2.0);
        assert!(c.obs.aberto().is_some(), "o pisca da azul fechou o estado");
    }

    #[test]
    fn fora_da_pista_o_jogador_continua_coberto() {
        // Quem está na grama precisa MAIS do aviso, não menos. A primeira versão exigia
        // `TrackSurface == OnTrack` e fechava o estado no quadro em que o jogador punha
        // uma roda fora — que é justo quando o carro de trás chega.
        let mut c = Cena::nova(10, Perfil::Plano);
        c.aquecer();
        c.ritmo_do_jogador(0.30);
        c.rodar_ate_abrir(15.0);
        assert!(c.obs.aberto().is_some());
        c.jogador().superficie = SUP_FORA_DA_PISTA;
        c.rodar(2.0);
        assert!(c.obs.aberto().is_some(), "a grama fechou o estado");
    }

    #[test]
    fn entrar_no_box_fecha_o_estado_calado() {
        let mut c = Cena::nova(10, Perfil::Plano);
        c.aquecer();
        c.ritmo_do_jogador(0.30);
        c.rodar_ate_abrir(15.0);
        let antes = c.falas.len();
        c.jogador().no_box = true;
        c.jogador().superficie = SUP_ENTRANDO_BOX;
        c.rodar(1.0);
        assert!(c.obs.aberto().is_none());
        assert_eq!(
            c.obs.encerrados().last().unwrap().fim,
            Some(FimTras::SaiuDaPista)
        );
        assert_eq!(c.falas.len(), antes, "anunciou liberação para o box");
    }

    #[test]
    fn o_jogador_guinchado_fecha_o_estado_com_a_duracao_certa() {
        // O guincho não avisa: o carro simplesmente some de `cars[]`. Sem esta varredura
        // o estado ficaria aberto para sempre e a duração contaria o tempo de ausência.
        let mut c = Cena::nova(10, Perfil::Plano);
        c.aquecer();
        c.ritmo_do_jogador(0.30);
        c.rodar_ate_abrir(15.0);
        assert!(c.obs.aberto().is_some());
        let visto_por_ultimo = c.t;
        c.carros.remove(0);
        c.rodar(30.0);
        let ep = c.obs.encerrados().last().cloned().unwrap();
        assert_eq!(ep.fim, Some(FimTras::SumiuDoMundo));
        assert!(
            (ep.duracao_s - (visto_por_ultimo - ep.inicio_s)).abs() < 0.2,
            "duração {:.1}s — contou o tempo de ausência",
            ep.duracao_s
        );
    }

    #[test]
    fn a_bandeirada_encerra_o_que_estava_aberto() {
        let mut c = Cena::nova(10, Perfil::Plano);
        c.aquecer();
        c.ritmo_do_jogador(0.30);
        c.rodar_ate_abrir(15.0);
        assert!(c.obs.aberto().is_some());
        c.estado = 5;
        c.rodar(0.5);
        assert_eq!(
            c.obs.encerrados().last().unwrap().fim,
            Some(FimTras::SessaoAcabou)
        );
    }

    #[test]
    fn um_salto_de_tempo_reinicia_sem_inventar_estado() {
        let mut c = Cena::nova(10, Perfil::Plano);
        c.aquecer();
        c.ritmo_do_jogador(0.30);
        c.rodar_ate_abrir(15.0);
        assert!(c.obs.aberto().is_some());
        c.t += 600.0;
        c.rodar(0.5);
        assert!(c.obs.aberto().is_none(), "o salto não reiniciou a máquina");
    }

    #[test]
    fn sem_comprimento_de_pista_nada_e_detectado() {
        let mut obs = ObservadorTras::novo();
        let carros: Vec<CarSnapshot> = (0..10)
            .map(|i| CarSnapshot {
                idx: i,
                is_player: i == 0,
                lap_dist_pct: 0.1 * i as f64,
                est_time: 5.0 * i as f64,
                track_surface: SUP_NA_PISTA,
                ..Default::default()
            })
            .collect();
        assert_eq!(
            obs.observar(AmostraTras {
                tempo_s: 1.0,
                estado_sessao: ESTADO_CORRIDA,
                bandeiras: 0,
                comprimento_m: 0.0,
                jogador_idx: 0,
                jogador_no_carro: true,
                carros: &carros,
            }),
            None
        );
        assert!(obs.aberto().is_none());
    }

    #[test]
    fn o_episodio_fechado_sai_uma_vez_so_pela_drenagem() {
        let mut c = Cena::nova(6, Perfil::Plano);
        c.atras(1, 260.0);
        c.aquecer();
        assert!(c.obs.drenar_encerrados().is_empty());
        c.ritmo_do_jogador(0.30);
        c.rodar_ate_abrir(20.0);
        c.ritmo_do_jogador(1.0);
        c.rodar(LIBERA_S + 2.0);
        let saiu = c.obs.drenar_encerrados();
        assert_eq!(saiu.len(), 1);
        assert_eq!(saiu[0].fim, Some(FimTras::TremPassou));
        assert!(c.obs.drenar_encerrados().is_empty(), "drenou duas vezes");
        assert_eq!(c.obs.encerrados().len(), 1);
    }
}
