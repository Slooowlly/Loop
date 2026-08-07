//! Spotter do Loop — a camada de INFORMAÇÃO.
//!
//! O iRacing publica UM canal sobre quem está do seu lado: `CarLeftRight`. Ele é
//! cru e nervoso — na borda da zona de detecção pisca de quadro em quadro, e a
//! 60 Hz isso viraria um spotter gago dizendo "esquerda, livre, esquerda, livre".
//! Todo o valor deste módulo está na HISTERESE: confirmar antes de falar, e não
//! repetir o que o piloto já sabe.
//!
//! O que sai daqui são EVENTOS com chave de fala (`esquerda`, `direita`,
//! `tres_largos`, `livre`, …), não áudio: quem fala é outra camada. Assim dá pra
//! validar a DETECÇÃO — que é a parte que depende do sim e não do nosso gosto —
//! antes de existir uma única linha gravada.
//!
//! Ver [`crate::iracing_sdk::spotter_control`] para o outro lado da moeda: calar
//! o spotter NATIVO do iRacing enquanto este aqui assume o papel.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde::Serialize;

use crate::iracing_sdk::IracingTelemetry;

// ───────────────────────── irsdk_CarLeftRight ─────────────────────────
/// Canal em repouso — nada a reportar (fora da pista, garagem, replay).
pub const LR_DESLIGADO: i32 = 0;
/// Ninguém ao lado.
pub const LR_LIVRE: i32 = 1;
/// Um carro à esquerda.
pub const LR_ESQUERDA: i32 = 2;
/// Um carro à direita.
pub const LR_DIREITA: i32 = 3;
/// Um de cada lado — três largos.
pub const LR_DOIS_LADOS: i32 = 4;
/// Dois carros à esquerda — três largos, todos do mesmo lado.
pub const LR_DUAS_ESQUERDA: i32 = 5;
/// Dois carros à direita — idem, do outro lado.
pub const LR_DUAS_DIREITA: i32 = 6;

/// Quanto um LADO precisa se manter para virar verdade (s). Curto de propósito:
/// aqui o atraso custa segurança, e 120 ms já mata o pisca-pisca de um quadro.
const CONFIRMA_LADO_S: f64 = 0.12;
/// Quanto o LIVRE precisa se manter para virar verdade (s). Bem mais longo que a
/// entrada: soltar o "livre" cedo demais é o erro caro — o piloto fecha a porta
/// em cima de quem ainda estava lá.
const CONFIRMA_LIVRE_S: f64 = 0.30;
/// Salto de `SessionTime` que denuncia troca de sessão, replay ou rebobinada (s).
/// Acima disso a máquina reinicia em vez de medir uma duração fantasma.
const SALTO_MAX_S: f64 = 5.0;
/// Quantos eventos o histórico guarda para inspeção.
const MAX_EVENTOS: usize = 80;

/// A vizinhança lateral do carro do jogador, já nomeada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vizinhanca {
    Desligado,
    Livre,
    Esquerda,
    Direita,
    DoisLados,
    DuasEsquerda,
    DuasDireita,
}

impl Vizinhanca {
    /// Traduz o valor cru do SDK. Qualquer coisa fora da tabela vira `Desligado` —
    /// um valor desconhecido não pode virar um "livre" que o piloto obedeceria.
    pub fn de_bruto(bruto: i32) -> Self {
        match bruto {
            LR_LIVRE => Vizinhanca::Livre,
            LR_ESQUERDA => Vizinhanca::Esquerda,
            LR_DIREITA => Vizinhanca::Direita,
            LR_DOIS_LADOS => Vizinhanca::DoisLados,
            LR_DUAS_ESQUERDA => Vizinhanca::DuasEsquerda,
            LR_DUAS_DIREITA => Vizinhanca::DuasDireita,
            _ => Vizinhanca::Desligado,
        }
    }

    /// Chave estável — é o que o front lê e o que a camada de voz vai usar para
    /// escolher o arquivo.
    pub fn chave(self) -> &'static str {
        match self {
            Vizinhanca::Desligado => "desligado",
            Vizinhanca::Livre => "livre",
            Vizinhanca::Esquerda => "esquerda",
            Vizinhanca::Direita => "direita",
            Vizinhanca::DoisLados => "tres_largos",
            Vizinhanca::DuasEsquerda => "duas_esquerda",
            Vizinhanca::DuasDireita => "duas_direita",
        }
    }

    /// Tem alguém ao lado.
    pub fn ocupada(self) -> bool {
        !matches!(self, Vizinhanca::Desligado | Vizinhanca::Livre)
    }

    /// Você e mais dois na mesma largura de pista — inclusive quando os dois estão
    /// do MESMO lado, que é o caso que mais assusta e o que o `CarLeftRight`
    /// distingue de graça.
    pub fn tres_largos(self) -> bool {
        matches!(
            self,
            Vizinhanca::DoisLados | Vizinhanca::DuasEsquerda | Vizinhanca::DuasDireita
        )
    }

    /// Quais flancos estão ocupados: `(esquerda, direita)`.
    pub fn flancos(self) -> (bool, bool) {
        match self {
            Vizinhanca::Esquerda | Vizinhanca::DuasEsquerda => (true, false),
            Vizinhanca::Direita | Vizinhanca::DuasDireita => (false, true),
            Vizinhanca::DoisLados => (true, true),
            _ => (false, false),
        }
    }

    /// Qual lembrete cabe nesta vizinhança. `None` quando não há ninguém ao lado.
    pub fn chave_lembrete(self) -> Option<&'static str> {
        match self.flancos() {
            (true, true) => Some(CHAVE_LEMBRETE_AMBOS),
            (true, false) => Some(CHAVE_LEMBRETE_ESQUERDA),
            (false, true) => Some(CHAVE_LEMBRETE_DIREITA),
            (false, false) => None,
        }
    }

    /// Quantos carros ao lado (0..2) — a escala do "ficou mais apertado".
    fn largura(self) -> u8 {
        match self {
            Vizinhanca::Esquerda | Vizinhanca::Direita => 1,
            Vizinhanca::DoisLados | Vizinhanca::DuasEsquerda | Vizinhanca::DuasDireita => 2,
            _ => 0,
        }
    }
}

/// A situação PIOROU: mais um carro, ou um flanco que antes estava limpo.
fn escalou(anterior: Vizinhanca, novo: Vizinhanca) -> bool {
    let (ae, ad) = anterior.flancos();
    let (ne, nd) = novo.flancos();
    let flanco_novo = (ne && !ae) || (nd && !ad);
    novo.largura() > anterior.largura() || flanco_novo
}

/// Que flanco(s) ficaram limpos nesta transição, e qual liberação anunciar.
///
/// Foi por aqui que a primeira versão errou duas vezes.
///
/// A primeira regra era "afrouxar não se anuncia", pelo raciocínio de que o piloto
/// continua lado a lado com alguém e narrar cada melhora vira ruído. Só que o caso que
/// mais importa é exatamente esse: sair de três largos para um carro só é o instante em
/// que o piloto DECIDE para onde ir, e ele precisa saber qual porta abriu. Silêncio aí
/// não é discrição, é esconder a única informação que ele ia usar.
///
/// A segunda foi mais sutil: corrigido o caso dos três largos, a saída para pista limpa
/// continuou caindo num `livre` genérico. Ou seja, um carro só à esquerda que ia embora
/// virava "Livre." — a fala mais ambígua do pacote — quando havia um lado a nomear.
/// Agora a decisão é uma só, e depende de QUANTOS flancos abriram: os dois abriram,
/// `livre`; um abriu, o lado dele. O `livre` puro passa a significar o que sempre se
/// pretendeu que significasse — não há mais ninguém, de nenhum lado.
///
/// Nenhum flanco chega a ficar limpo sem ter sido anunciado antes: todo flanco nasce de
/// uma entrada ou de uma escalada, e as duas falam. Isso torna a simetria uma
/// invariante — **toda chegada anunciada tem uma saída anunciada** —, e é ela que
/// impede o pior defeito possível aqui, que é o piloto ficar segurando linha por um
/// carro que já foi embora.
fn liberacao(anterior: Vizinhanca, novo: Vizinhanca) -> Option<&'static str> {
    let (ae, ad) = anterior.flancos();
    let (ne, nd) = novo.flancos();
    match (ae && !ne, ad && !nd) {
        (true, true) => Some(Vizinhanca::Livre.chave()),
        (true, false) => Some(CHAVE_LIVRE_ESQUERDA),
        (false, true) => Some(CHAVE_LIVRE_DIREITA),
        (false, false) => None,
    }
}

/// Chave do TESTE DE RÁDIO. Não é uma vizinhança — é o spotter se apresentando
/// quando o piloto senta no carro, antes da largada.
///
/// Existe porque o silêncio é ambíguo: um spotter que não fala pode estar certo
/// (ninguém ao lado) ou morto (saída de áudio errada, volume no zero, canal do
/// sim não preenchido). Descobrir isso no meio de uma disputa a três é tarde. Uma
/// frase na entrada do carro troca essa ambiguidade por uma certeza, e é barata:
/// acontece uma vez por sessão, quando não há nada a perder ouvindo.
pub const CHAVE_TESTE: &str = "teste";

/// LEMBRETES — o carro continua ali. Um por lado, mais um para os dois lados.
///
/// Repetidos enquanto o carro do lado não sai. Sem eles, o silêncio depois do
/// primeiro aviso é ambíguo: o piloto não sabe se o outro já foi embora ou se o
/// spotter parou de olhar, e no meio de uma disputa essa dúvida é o que faz fechar a
/// porta cedo.
pub const CHAVE_LEMBRETE_ESQUERDA: &str = "ainda_esquerda";
pub const CHAVE_LEMBRETE_DIREITA: &str = "ainda_direita";
/// Os dois lados ocupados — aqui não há um lado a nomear, e a fala é sobre segurar a
/// posição ("Ainda aí", "Segura, segura", "Mantém a posição").
pub const CHAVE_LEMBRETE_AMBOS: &str = "ainda_ai";

/// LIBERAÇÃO de um lado, com o outro ainda ocupado.
pub const CHAVE_LIVRE_ESQUERDA: &str = "livre_esquerda";
pub const CHAVE_LIVRE_DIREITA: &str = "livre_direita";

/// Intervalo do PRIMEIRO lembrete (s). Qualquer anúncio zera a contagem — o lembrete
/// só preenche silêncio, nunca atropela notícia.
const LEMBRETE_S: f64 = 2.5;
/// Quanto o intervalo cresce a cada repetição (s), e onde ele para de crescer.
///
/// Uma disputa longa não pode virar metrônomo. A primeira repetição responde à dúvida
/// ("ele ainda está aí?"); da terceira em diante o piloto já sabe, e insistir no mesmo
/// ritmo passa de informação a alarme de carro. Crescer é a forma de continuar
/// presente sem continuar falando.
///
/// A faixa 2,5 → 4,0 é a que o projeto da fase 2 pediu. Os dois extremos são
/// deliberados: o primeiro lembrete sai cedo porque é ele que responde à dúvida ainda
/// quente, e o teto para em 4 s porque acima disso o lembrete deixa de parecer resposta
/// e passa a parecer esquecimento.
const LEMBRETE_PASSO_S: f64 = 0.5;
const LEMBRETE_MAX_S: f64 = 4.0;

/// A chave é de um lembrete?
pub fn eh_lembrete(chave: &str) -> bool {
    matches!(
        chave,
        CHAVE_LEMBRETE_ESQUERDA | CHAVE_LEMBRETE_DIREITA | CHAVE_LEMBRETE_AMBOS
    )
}

/// Uma amostra de telemetria, do ponto de vista do spotter.
#[derive(Debug, Clone, Copy)]
pub struct Amostra {
    /// `CarLeftRight` cru.
    pub bruto: i32,
    /// `SessionTime`.
    pub tempo_s: f64,
    /// O piloto está no carro, no mundo — inclusive parado no box. É o gatilho do
    /// teste de rádio.
    pub no_carro: bool,
    /// No carro e FORA do pit road: onde a vizinhança lateral quer dizer alguma
    /// coisa. No box, carro ao lado é o vizinho de garagem.
    pub na_pista: bool,
}

/// Um anúncio confirmado do spotter.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventoSpotter {
    /// Crescente e nunca reaproveitado — o front usa como cursor para saber o que
    /// já mostrou.
    pub id: u64,
    /// Chave da fala (ver [`Vizinhanca::chave`]).
    pub chave: &'static str,
    /// `SessionTime` do sim no instante da confirmação.
    pub sessao_s: f64,
    /// Há quanto tempo o piloto estava lado a lado quando o evento saiu. Só é
    /// diferente de zero no `livre`; é a métrica que diz se a histerese está
    /// calibrada ou se está engolindo disputas curtas.
    pub duracao_s: f64,
}

/// A máquina de estados. Pura: recebe valor cru + relógio e devolve o evento (no
/// máximo um por amostra). Sem isso não haveria como testar a histerese, que é
/// exatamente a parte que não dá para conferir olhando a tela.
#[derive(Debug)]
pub struct Observador {
    /// Vizinhança que já passou pela confirmação — a "verdade" corrente.
    confirmada: Vizinhanca,
    /// Vizinhança crua que está tentando se confirmar.
    candidata: Vizinhanca,
    /// Quando a candidata apareceu.
    candidata_desde_s: f64,
    /// Quando o piloto ficou lado a lado pela primeira vez nesta disputa. NÃO é
    /// reiniciado quando a vizinhança escala (esquerda → três largos): a disputa
    /// é a mesma, e é a duração dela que decide se o "livre" vale anúncio.
    ocupada_desde_s: Option<f64>,
    ultimo_tempo_s: f64,
    /// Quando saiu o último anúncio, de qualquer tipo. Serve SÓ para agendar o
    /// lembrete — nunca para suprimir um anúncio (ver a nota em [`Observador::emitir`]).
    ultimo_anuncio_s: f64,
    /// Espera até o próximo lembrete. Cresce a cada repetição e volta ao mínimo
    /// quando sai qualquer notícia — porque aí a dúvida recomeça do zero.
    intervalo_lembrete: f64,
    proximo_id: u64,
    bruto: i32,
    /// O teste de rádio já saiu nesta sessão.
    testado: bool,
    /// `no_carro` da amostra anterior — o teste dispara na SUBIDA, não enquanto
    /// o piloto estiver sentado.
    no_carro_antes: bool,
    eventos: VecDeque<EventoSpotter>,
}

impl Default for Observador {
    fn default() -> Self {
        Self::novo()
    }
}

impl Observador {
    pub fn novo() -> Self {
        Observador {
            confirmada: Vizinhanca::Desligado,
            candidata: Vizinhanca::Desligado,
            candidata_desde_s: 0.0,
            ocupada_desde_s: None,
            ultimo_tempo_s: 0.0,
            ultimo_anuncio_s: 0.0,
            intervalo_lembrete: LEMBRETE_S,
            proximo_id: 1,
            bruto: LR_DESLIGADO,
            testado: false,
            no_carro_antes: false,
            eventos: VecDeque::new(),
        }
    }

    /// Registra um anúncio e devolve o evento. Único caminho de saída — assim id e
    /// histórico nunca ficam fora de sincronia.
    ///
    /// NÃO existe intervalo mínimo entre anúncios aqui, e isso é deliberado. Havia
    /// um (350 ms), e ele DESCARTAVA o evento em vez de adiá-lo: o "esquerda"
    /// confirmava, o "três largos" chegava 120 ms depois — o tempo da própria
    /// confirmação — e sumia, deixando o piloto sem saber do carro que apareceu do
    /// outro lado. Trocar informação de segurança por elegância de cadência é o
    /// negócio errado. Quando duas falas se atropelam, quem resolve é o
    /// reprodutor, cortando a anterior: lá a mais nova é a mais urgente, e nada
    /// se perde.
    fn emitir(&mut self, chave: &'static str, tempo_s: f64, duracao_s: f64) -> EventoSpotter {
        let evento = EventoSpotter {
            id: self.proximo_id,
            chave,
            sessao_s: tempo_s,
            duracao_s,
        };
        self.proximo_id += 1;
        self.ultimo_anuncio_s = tempo_s;
        // Notícia nova reinicia a paciência: a dúvida que o lembrete responde
        // recomeça do zero a cada mudança de situação.
        if !eh_lembrete(chave) {
            self.intervalo_lembrete = LEMBRETE_S;
        }
        self.eventos.push_back(evento.clone());
        while self.eventos.len() > MAX_EVENTOS {
            self.eventos.pop_front();
        }
        evento
    }

    /// Zera a máquina sem apagar o histórico — o histórico é justamente o que se
    /// olha depois de uma sessão para saber o que foi detectado.
    fn zerar(&mut self, tempo_s: f64) {
        self.confirmada = Vizinhanca::Desligado;
        self.candidata = Vizinhanca::Desligado;
        self.candidata_desde_s = tempo_s;
        self.ocupada_desde_s = None;
    }

    /// Uma amostra. No máximo um evento por chamada.
    pub fn observar(&mut self, a: Amostra) -> Option<EventoSpotter> {
        let Amostra {
            bruto,
            tempo_s,
            no_carro,
            na_pista,
        } = a;

        // Relógio andou para trás ou pulou: troca de sessão, replay, rebobinada.
        // Medir duração através desse buraco daria disputas de horas.
        let salto = tempo_s < self.ultimo_tempo_s || tempo_s - self.ultimo_tempo_s > SALTO_MAX_S;
        self.ultimo_tempo_s = tempo_s;
        self.bruto = bruto;
        if salto {
            self.zerar(tempo_s);
            // Sessão nova merece um teste novo: é justamente a troca de sessão que
            // pode ter mudado o dispositivo de saída ou derrubado o áudio.
            self.testado = false;
            self.no_carro_antes = no_carro;
            // O relógio da sessão nova não tem nada a ver com o da anterior; sem isto,
            // a diferença ficaria negativa e o lembrete nunca mais dispararia.
            self.ultimo_anuncio_s = tempo_s;
            return None;
        }

        // Teste de rádio na SUBIDA de "entrou no carro" — antes de qualquer coisa,
        // e de propósito sem exigir `na_pista`: na largada parada o piloto ainda
        // está no box, e esperar ele sair de lá entregaria o teste depois do verde.
        let entrou = no_carro && !self.no_carro_antes;
        self.no_carro_antes = no_carro;
        if entrou && !self.testado {
            self.testado = true;
            return Some(self.emitir(CHAVE_TESTE, tempo_s, 0.0));
        }

        if !na_pista {
            self.zerar(tempo_s);
            self.bruto = LR_DESLIGADO;
            return None;
        }

        if let Some(evento) = self.avaliar(bruto, tempo_s) {
            return Some(evento);
        }

        // Nada mudou, mas o carro CONTINUA do lado. Um spotter que anuncia uma vez e
        // cala vira ambíguo depois de três segundos: o piloto não sabe se o outro
        // saiu ou se o spotter parou de olhar. O lembrete responde a isso sem
        // pretender ser notícia — e some sozinho no instante em que o "livre" sai.
        if let Some(chave) = self.confirmada.chave_lembrete() {
            if tempo_s - self.ultimo_anuncio_s >= self.intervalo_lembrete {
                self.intervalo_lembrete = (self.intervalo_lembrete + LEMBRETE_PASSO_S).min(LEMBRETE_MAX_S);
                return Some(self.emitir(chave, tempo_s, 0.0));
            }
        }
        None
    }

    /// A máquina de vizinhança propriamente dita: confirma o estado e devolve o
    /// anúncio, se houver.
    fn avaliar(&mut self, bruto: i32, tempo_s: f64) -> Option<EventoSpotter> {
        let alvo = Vizinhanca::de_bruto(bruto);
        if alvo != self.candidata {
            self.candidata = alvo;
            self.candidata_desde_s = tempo_s;
        }
        if self.candidata == self.confirmada {
            return None;
        }

        // O canal em repouso confirma na hora: não é uma leitura de "está livre",
        // é a ausência de leitura, e segurá-la só atrasaria o reinício.
        let espera = match self.candidata {
            Vizinhanca::Desligado => 0.0,
            Vizinhanca::Livre => CONFIRMA_LIVRE_S,
            _ => CONFIRMA_LADO_S,
        };
        if tempo_s - self.candidata_desde_s < espera {
            return None;
        }

        let anterior = self.confirmada;
        let inicio_candidata = self.candidata_desde_s;
        self.confirmada = self.candidata;

        // A disputa acabou no instante em que o LIVRE começou, não quando ele foi
        // confirmado — senão a duração carregaria os 600 ms da própria histerese.
        let duracao = self
            .ocupada_desde_s
            .map(|inicio| inicio_candidata - inicio)
            .unwrap_or(0.0);

        // O que dizer da transição. Duas famílias, nesta ordem de prioridade:
        //   1. ENTRADA — ficou pior (chegou alguém, ou abriu flanco novo): nomeia a
        //      situação. É a fala de maior prioridade do sistema inteiro.
        //   2. LIBERAÇÃO — algum flanco abriu: nomeia o que abriu, ou `livre` se os
        //      dois abriram.
        // A ordem importa: em "esquerda → direita" as duas valem ao mesmo tempo, e o
        // que o piloto precisa ouvir é o carro que CHEGOU.
        //
        // `Desligado` é a ausência de leitura, não uma leitura de pista limpa, e por
        // isso não fala nada: anunciar "livre" porque o canal foi dormir seria inventar
        // informação exatamente no formato mais perigoso.
        let chave = if matches!(self.confirmada, Vizinhanca::Desligado) {
            None
        } else if self.confirmada.ocupada() && (!anterior.ocupada() || escalou(anterior, self.confirmada)) {
            Some(self.confirmada.chave())
        } else {
            liberacao(anterior, self.confirmada)
        };

        if self.confirmada.ocupada() {
            if !anterior.ocupada() {
                self.ocupada_desde_s = Some(inicio_candidata);
            }
        } else {
            self.ocupada_desde_s = None;
        }

        let chave = chave?;
        let duracao_do_livre = if matches!(self.confirmada, Vizinhanca::Livre) {
            duracao
        } else {
            0.0
        };
        Some(self.emitir(chave, tempo_s, duracao_do_livre))
    }

    pub fn estado(&self) -> EstadoSpotter {
        let (esquerda, direita) = self.confirmada.flancos();
        EstadoSpotter {
            bruto: self.bruto,
            vizinhanca: self.confirmada.chave(),
            ao_lado: self.confirmada.ocupada(),
            tres_largos: self.confirmada.tres_largos(),
            esquerda,
            direita,
            eventos: self.eventos.iter().cloned().collect(),
        }
    }
}

/// O que o front lê: a vizinhança AGORA mais o histórico de anúncios.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoSpotter {
    /// Valor cru de `CarLeftRight`, exposto de propósito: é o que responde
    /// "o canal está vivo nesta sessão?" sem depender da nossa interpretação.
    pub bruto: i32,
    pub vizinhanca: &'static str,
    pub ao_lado: bool,
    pub tres_largos: bool,
    pub esquerda: bool,
    pub direita: bool,
    /// Mais antigo primeiro.
    pub eventos: Vec<EventoSpotter>,
}

fn observador() -> &'static Mutex<Observador> {
    static OBSERVADOR: OnceLock<Mutex<Observador>> = OnceLock::new();
    OBSERVADOR.get_or_init(|| Mutex::new(Observador::novo()))
}

fn lock() -> MutexGuard<'static, Observador> {
    observador().lock().unwrap_or_else(|e| e.into_inner())
}

/// Alimenta o observador global com uma amostra de telemetria. Chamado pelo
/// amostrador de fundo (~60 Hz) — precisa ser barato, e é: um lock e um punhado
/// de comparações.
pub fn observar(t: &IracingTelemetry) {
    let no_carro = t.on_track && !t.is_replay_playing;
    let amostra = Amostra {
        bruto: t.car_left_right,
        tempo_s: t.session_time,
        no_carro,
        na_pista: no_carro && !t.player_on_pit_road,
    };
    // O spotter de FRENTE roda em toda amostra, aconteça o que acontecer do lado: ele
    // precisa acompanhar velocidade e pico de 40 carros continuamente, e um tick pulado
    // é um buraco no histórico que a regra "estava andando" leria como grid.
    let frente = crate::iracing_sdk::spotter_frente::observar(t);
    // O de TRÁS também roda sempre, e pelo mesmo motivo: ele mede a fração de ritmo do
    // jogador contra o campo numa janela de 3 s, e um tick pulado é um degrau na fração.
    // Ele lê o comprimento da pista de `spotter_frente` — se o de frente parar de ser
    // alimentado, este emudece em silêncio, e é por isso que os dois vivem lado a lado.
    let tras = crate::iracing_sdk::spotter_tras::observar(t);
    // Retorno à pista. Roda sempre — o pico de velocidade do próprio jogador é o que
    // separa "rodou" de "nunca andou", e um tick pulado é um buraco nessa memória.
    let voltar = crate::iracing_sdk::spotter_voltar::observar(t);
    // Bandeira: sem inferência, só o `session_flags` com histerese. Roda sempre porque a
    // máquina precisa ver o bit apagar para poder rearmar.
    let bandeira = crate::iracing_sdk::spotter_bandeira::observar(t);
    // Saída de box. Roda sempre: a borda "deixou a via de box" acontece num quadro só, e
    // perdê-la é perder a janela inteira de 15 s em que o carro ainda é notícia.
    let boxe = crate::iracing_sdk::spotter_boxe::observar(t);
    // Clima: o mais raro do sistema. Roda sempre porque a máquina precisa ver a primeira
    // leitura da corrida para ter de que a mudança seja mudança.
    let clima = crate::iracing_sdk::spotter_clima::observar(t);
    let evento = lock().observar(amostra);
    // QUEM GANHA O TICK. Nada aqui é descartado: cada detector só marca o aviso como dado
    // quando o anúncio realmente sai, então quem perde a vez volta 16 ms depois. A
    // arbitragem fina de quem interrompe quem é da camada de voz, que sabe o que ainda
    // está tocando. A ordem abaixo é só quem tenta primeiro.
    //
    // 1. LATERAL — o carro que já está do seu lado é o que machuca agora.
    // 2. RETORNO — o piloto está parado ou de ré, e a única pergunta que ele tem é "posso
    //    entrar?". Tudo o mais descreve um mundo em que ele ainda não está.
    // 3. OBSTÁCULO À FRENTE e 4. SAÍDA DE BOX — ambos são "coisa lenta na sua trajetória".
    //    Ficam nesta ordem por serem o mesmo assunto e nunca terem coincidido nas capturas;
    //    se um dia coincidirem, o certo seria o MAIS PRÓXIMO ganhar, e isso não está
    //    implementado entre módulos (dentro do `spotter_frente` está).
    // 5. TRÁS — chega até o piloto, e ele não está dirigindo para dentro dele.
    // 6. BANDEIRA e 7. CLIMA — estados que duram minutos e podem esperar um tick; os
    //    outros descrevem geometria que muda neste.
    let evento = match (evento, voltar, frente, boxe, tras, bandeira, clima) {
        (Some(e), _, _, _, _, _, _) => Some(e),
        (None, Some(chave), _, _, _, _, _) => {
            crate::iracing_sdk::spotter_voltar::confirmar_aviso();
            Some(lock().emitir(chave, t.session_time, 0.0))
        }
        (None, None, Some(chave), _, _, _, _) => {
            // Só agora o episódio conta como avisado: se o lateral tivesse ganhado o
            // tick, o aviso continuaria pendente e voltaria na próxima amostra.
            crate::iracing_sdk::spotter_frente::confirmar_aviso();
            Some(lock().emitir(chave, t.session_time, 0.0))
        }
        (None, None, None, Some(chave), _, _, _) => {
            crate::iracing_sdk::spotter_boxe::confirmar_aviso();
            Some(lock().emitir(chave, t.session_time, 0.0))
        }
        (None, None, None, None, Some(chave), _, _) => {
            crate::iracing_sdk::spotter_tras::confirmar_aviso();
            Some(lock().emitir(chave, t.session_time, 0.0))
        }
        (None, None, None, None, None, Some(chave), _) => {
            crate::iracing_sdk::spotter_bandeira::confirmar_aviso();
            Some(lock().emitir(chave, t.session_time, 0.0))
        }
        (None, None, None, None, None, None, Some(chave)) => {
            crate::iracing_sdk::spotter_clima::confirmar_aviso();
            Some(lock().emitir(chave, t.session_time, 0.0))
        }
        (None, None, None, None, None, None, None) => None,
    };
    // Fora do lock. O log é o que sobra de uma sessão de teste depois que o
    // jogador tirou o capacete — sem ele, só existiria o que deu para ver de
    // relance no canto da tela enquanto se dirigia.
    if let Some(e) = evento {
        crate::diagnostico::linha(
            "spotter",
            &format!("{} @ {:.1}s (lado a lado {:.1}s)", e.chave, e.sessao_s, e.duracao_s),
        );
    }
}

/// Snapshot para a UI.
pub fn estado() -> EstadoSpotter {
    lock().estado()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Amostra "no carro e na pista" — o caso comum dos testes de vizinhança.
    fn na_pista(bruto: i32, tempo_s: f64) -> Amostra {
        Amostra {
            bruto,
            tempo_s,
            no_carro: true,
            na_pista: true,
        }
    }

    /// Roda uma sequência `(tempo_s, bruto)` e devolve TODAS as chaves anunciadas.
    ///
    /// Começa com o piloto JÁ no carro, para o teste de rádio (que dispara na
    /// entrada) não aparecer no meio das asserções de vizinhança.
    fn correr_tudo(passos: &[(f64, i32)]) -> Vec<&'static str> {
        let mut obs = sentado();
        let mut saida = Vec::new();
        for &(tempo, bruto) in passos {
            if let Some(e) = obs.observar(na_pista(bruto, tempo)) {
                saida.push(e.chave);
            }
        }
        saida
    }

    /// Só as MUDANÇAS de vizinhança, sem os lembretes.
    ///
    /// O lembrete é ortogonal: ele preenche silêncio, não descreve transição. Deixá-lo
    /// entrar nestas asserções faria cada teste de histerese depender também da
    /// cadência do lembrete, e mudar um dos dois quebraria os dois.
    fn correr(passos: &[(f64, i32)]) -> Vec<&'static str> {
        correr_tudo(passos)
            .into_iter()
            .filter(|c| !eh_lembrete(c))
            .collect()
    }

    /// Observador que já passou pela entrada no carro.
    fn sentado() -> Observador {
        let mut obs = Observador::novo();
        obs.no_carro_antes = true;
        obs.testado = true;
        obs
    }

    /// Amostra a 60 Hz mantendo `bruto` de `de` até `ate` (exclusivo).
    fn segurar(passos: &mut Vec<(f64, i32)>, de: f64, ate: f64, bruto: i32) {
        let mut t = de;
        while t < ate {
            passos.push((t, bruto));
            t += 1.0 / 60.0;
        }
    }

    #[test]
    fn traduz_a_tabela_do_sdk_e_o_desconhecido_nao_vira_livre() {
        assert_eq!(Vizinhanca::de_bruto(LR_LIVRE), Vizinhanca::Livre);
        assert_eq!(Vizinhanca::de_bruto(LR_DOIS_LADOS), Vizinhanca::DoisLados);
        assert_eq!(Vizinhanca::de_bruto(99), Vizinhanca::Desligado);
        assert_eq!(Vizinhanca::de_bruto(-1), Vizinhanca::Desligado);
    }

    #[test]
    fn tres_largos_vale_para_os_dois_do_mesmo_lado() {
        assert!(Vizinhanca::DoisLados.tres_largos());
        assert!(Vizinhanca::DuasEsquerda.tres_largos());
        assert!(Vizinhanca::DuasDireita.tres_largos());
        assert!(!Vizinhanca::Esquerda.tres_largos());
        assert!(!Vizinhanca::Livre.tres_largos());
    }

    #[test]
    fn carro_a_esquerda_e_depois_livre_a_esquerda() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 4.0, LR_ESQUERDA);
        segurar(&mut p, 4.0, 6.0, LR_LIVRE);
        // Havia UM lado ocupado, então a liberação nomeia esse lado. O "Livre." seco
        // aqui seria informação jogada fora.
        assert_eq!(correr(&p), vec!["esquerda", "livre_esquerda"]);
    }

    #[test]
    fn a_liberacao_de_um_lado_so_nomeia_o_lado_em_todos_os_casos() {
        let caso = |ocupado: i32| {
            let mut p = Vec::new();
            segurar(&mut p, 0.0, 1.0, LR_LIVRE);
            segurar(&mut p, 1.0, 4.0, ocupado);
            segurar(&mut p, 4.0, 7.0, LR_LIVRE);
            *correr(&p).last().expect("deveria anunciar a liberação")
        };
        assert_eq!(caso(LR_ESQUERDA), CHAVE_LIVRE_ESQUERDA);
        assert_eq!(caso(LR_DIREITA), CHAVE_LIVRE_DIREITA);
        // Dois do mesmo lado é um flanco só: quando ele abre, é o lado que abriu.
        assert_eq!(caso(LR_DUAS_ESQUERDA), CHAVE_LIVRE_ESQUERDA);
        assert_eq!(caso(LR_DUAS_DIREITA), CHAVE_LIVRE_DIREITA);
    }

    #[test]
    fn o_livre_seco_so_sai_quando_os_DOIS_lados_abrem() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 4.0, LR_DOIS_LADOS);
        segurar(&mut p, 4.0, 7.0, LR_LIVRE);
        assert_eq!(correr(&p), vec!["tres_largos", "livre"]);
    }

    #[test]
    fn pisca_de_um_quadro_nao_vira_anuncio() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        p.push((1.0, LR_ESQUERDA)); // um único quadro
        segurar(&mut p, 1.0 + 1.0 / 60.0, 3.0, LR_LIVRE);
        assert!(correr(&p).is_empty());
    }

    #[test]
    fn um_rocar_curto_e_anunciado_na_entrada_E_na_saida() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 1.3, LR_DIREITA);
        segurar(&mut p, 1.3, 4.0, LR_LIVRE);
        // Houve um mínimo de duração aqui (0,5 s ao lado) para o roçar não render o
        // par entrada+saída. Caiu: ele só conseguia calar a SAÍDA, nunca a entrada —
        // que confirma em 0,12 s. O piloto ouvia "Direita." e nunca ouvia o
        // desmentido, ficando a segurar linha por um carro que já tinha ido. Uma
        // sílaba a mais é barata; uma crença errada sobre quem está do lado, não.
        assert_eq!(correr(&p), vec!["direita", "livre_direita"]);
    }

    #[test]
    fn escalar_para_tres_largos_fala_na_hora() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 3.0, LR_ESQUERDA);
        segurar(&mut p, 3.0, 6.0, LR_DOIS_LADOS);
        segurar(&mut p, 6.0, 9.0, LR_LIVRE);
        assert_eq!(correr(&p), vec!["esquerda", "tres_largos", "livre"]);
    }

    #[test]
    fn dois_do_mesmo_lado_tambem_escalam() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 3.0, LR_DIREITA);
        segurar(&mut p, 3.0, 6.0, LR_DUAS_DIREITA);
        assert_eq!(correr(&p), vec!["direita", "duas_direita"]);
    }

    #[test]
    fn sair_de_tres_largos_diz_QUAL_lado_abriu() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 4.0, LR_DOIS_LADOS);
        // O da direita saiu; o da esquerda continua lá. É o instante em que o piloto
        // decide para onde ir, e a porta que abriu foi a da direita.
        segurar(&mut p, 4.0, 7.0, LR_ESQUERDA);
        assert_eq!(correr(&p), vec!["tres_largos", "livre_direita"]);
    }

    #[test]
    fn trocar_de_lado_e_noticia_nova() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 3.0, LR_ESQUERDA);
        segurar(&mut p, 3.0, 6.0, LR_DIREITA);
        assert_eq!(correr(&p), vec!["esquerda", "direita"]);
    }

    #[test]
    fn a_disputa_sobrevive_a_um_livre_curto() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 3.0, LR_ESQUERDA);
        // Perdeu a detecção por 0,2 s (curva, cume) — menos que a confirmação do
        // livre, então nada muda e o "esquerda" não é repetido na volta. A margem aqui
        // encolheu junto com a confirmação do livre (0,6 → 0,3 s): é o preço de anunciar
        // a porta aberta mais cedo.
        segurar(&mut p, 3.0, 3.2, LR_LIVRE);
        segurar(&mut p, 3.2, 6.0, LR_ESQUERDA);
        assert_eq!(correr(&p), vec!["esquerda"]);
    }

    #[test]
    fn a_duracao_do_livre_nao_carrega_a_histerese() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 5.0, LR_ESQUERDA);
        segurar(&mut p, 5.0, 8.0, LR_LIVRE);
        let mut obs = sentado();
        let mut livre = None;
        for (t, b) in p {
            if let Some(e) = obs.observar(na_pista(b, t)) {
                if e.chave == CHAVE_LIVRE_ESQUERDA {
                    livre = Some(e);
                }
            }
        }
        let e = livre.expect("o livre deveria ter sido anunciado");
        // Lado a lado de ~1 s a ~5 s: a duração é a disputa, não a disputa mais
        // os 0,3 s que o livre esperou para se confirmar.
        assert!(
            (e.duracao_s - 4.0).abs() < 0.1,
            "duração {} fora do esperado",
            e.duracao_s
        );
    }

    #[test]
    fn o_lembrete_repete_pelo_lado_certo_enquanto_o_carro_nao_sai() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 13.0, LR_ESQUERDA); // 12 s lado a lado
        segurar(&mut p, 13.0, 16.0, LR_LIVRE);
        let chaves = correr_tudo(&p);
        assert_eq!(chaves.first(), Some(&"esquerda"));
        assert_eq!(chaves.last(), Some(&CHAVE_LIVRE_ESQUERDA));
        // Todo lembrete nomeia o lado ocupado — nunca o genérico, que é dos dois lados.
        assert!(chaves[1..chaves.len() - 1]
            .iter()
            .all(|c| *c == CHAVE_LEMBRETE_ESQUERDA));
        // ~12 s com espera crescente (2,5 → 3 → 3,5 → 4): três lembretes, o quarto
        // cairia depois de o carro já ter saído.
        assert_eq!(chaves.len() - 2, 3, "chaves: {chaves:?}");
    }

    #[test]
    fn o_lembrete_dos_dois_lados_nao_nomeia_lado() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 8.0, LR_DOIS_LADOS);
        let chaves = correr_tudo(&p);
        assert_eq!(chaves[0], "tres_largos");
        assert!(chaves[1..].iter().all(|c| *c == CHAVE_LEMBRETE_AMBOS));
        assert!(chaves.len() > 1, "deveria haver lembrete");
    }

    #[test]
    fn a_espera_do_lembrete_cresce_e_para_de_crescer() {
        let mut obs = sentado();
        let mut t = 0.0;
        let mut quando = Vec::new();
        while t < 40.0 {
            if let Some(e) = obs.observar(na_pista(LR_DIREITA, t)) {
                if eh_lembrete(e.chave) {
                    quando.push(e.sessao_s);
                }
            }
            t += 1.0 / 60.0;
        }
        let esperas: Vec<f64> = quando.windows(2).map(|w| w[1] - w[0]).collect();
        // 2,5 → 3 → 3,5 → 4 e para em 4: presente sem virar metrônomo.
        assert!(esperas.len() >= 5, "poucos lembretes: {quando:?}");
        for (i, e) in esperas.iter().enumerate() {
            let esperado = (LEMBRETE_S + LEMBRETE_PASSO_S * (i as f64 + 1.0)).min(LEMBRETE_MAX_S);
            assert!(
                (e - esperado).abs() < 0.1,
                "espera {i} foi {e:.2}s, esperado {esperado:.2}s (todas: {esperas:?})"
            );
        }
    }

    #[test]
    fn sair_rapido_de_tres_largos_ainda_diz_qual_porta_abriu() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        // Três largos por 0,3 s e o da esquerda cai fora. Curto, mas a chegada foi
        // anunciada, então a saída também é — e é justo esse o instante em que o piloto
        // escolhe para onde ir.
        segurar(&mut p, 1.0, 1.3, LR_DOIS_LADOS);
        segurar(&mut p, 1.3, 3.0, LR_DIREITA);
        assert_eq!(correr(&p), vec!["tres_largos", "livre_esquerda"]);
    }

    /// A sequência que o projeto da fase 4 quer impedir:
    /// "Esquerda. / Livre à esquerda. / Esquerda. / Livre à esquerda."
    ///
    /// A defesa proposta era uma trava: bloquear uma entrada IDÊNTICA por 500 ms. Ela
    /// não entrou, porque as próprias confirmações já garantem mais que isso — e uma
    /// trava que descarta anúncio é o mesmo defeito que já foi removido daqui uma vez
    /// (ver a nota em [`Observador::emitir`]). Duas entradas iguais só podem se repetir
    /// depois de um `livre` no meio, e o caminho de volta custa
    /// `CONFIRMA_LIVRE_S + CONFIRMA_LADO_S` = 0,42 s no mínimo absoluto.
    ///
    /// Este teste é essa garantia escrita como verificação em vez de knob: se alguém
    /// baixar as confirmações a ponto de o gaguejar voltar a ser possível, ele quebra.
    #[test]
    fn duas_entradas_iguais_nunca_saem_coladas() {
        let mut obs = sentado();
        let mut t = 0.0;
        let mut quando = Vec::new();
        // Carro exatamente na borda da zona: entra e sai no ritmo mais rápido que a
        // máquina consegue acompanhar.
        while t < 20.0 {
            let bruto = if (t % 0.5) < 0.15 { LR_ESQUERDA } else { LR_LIVRE };
            if let Some(e) = obs.observar(na_pista(bruto, t)) {
                if e.chave == "esquerda" {
                    quando.push(e.sessao_s);
                }
            }
            t += 1.0 / 60.0;
        }
        assert!(quando.len() >= 2, "o cenário deveria repetir a entrada: {quando:?}");
        let minimo = CONFIRMA_LIVRE_S + CONFIRMA_LADO_S;
        for w in quando.windows(2) {
            let gap = w[1] - w[0];
            assert!(gap >= minimo - 0.02, "duas entradas a {gap:.3}s (mínimo {minimo:.2}s)");
        }
    }

    #[test]
    fn pisca_a_cada_quadro_nao_confirma_nada() {
        let mut obs = sentado();
        let mut t = 0.0;
        let mut n = 0;
        // Alterna a cada amostra: a candidata nunca sobrevive à própria confirmação.
        while t < 5.0 {
            let bruto = if n % 2 == 0 { LR_ESQUERDA } else { LR_LIVRE };
            assert!(obs.observar(na_pista(bruto, t)).is_none());
            t += 1.0 / 60.0;
            n += 1;
        }
    }

    #[test]
    fn trocar_de_lado_anuncia_quem_chegou_nao_quem_saiu() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 3.0, LR_ESQUERDA);
        // Some da esquerda e aparece na direita no mesmo instante. As duas notícias
        // valem, e a que o piloto precisa é a do carro que CHEGOU.
        segurar(&mut p, 3.0, 6.0, LR_DIREITA);
        assert_eq!(correr(&p), vec!["esquerda", "direita"]);
    }

    #[test]
    fn perder_um_dos_dois_do_mesmo_lado_continua_calado() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 4.0, LR_DUAS_ESQUERDA);
        // De dois para um à esquerda: nenhum flanco abriu, a porta continua fechada.
        segurar(&mut p, 4.0, 7.0, LR_ESQUERDA);
        assert_eq!(correr(&p), vec!["duas_esquerda"]);
    }

    #[test]
    fn o_lembrete_nao_sai_com_a_pista_livre() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 20.0, LR_LIVRE);
        assert!(correr_tudo(&p).is_empty());
    }

    #[test]
    fn uma_noticia_adia_o_lembrete_em_vez_de_somar() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        segurar(&mut p, 1.0, 3.0, LR_ESQUERDA);
        // Aos 3 s escala: o "três largos" é notícia e zera a contagem do lembrete,
        // então o próximo lembrete só cabe a partir de ~5,6 s.
        segurar(&mut p, 3.0, 5.5, LR_DOIS_LADOS);
        let chaves = correr_tudo(&p);
        assert_eq!(chaves, vec!["esquerda", "tres_largos"]);
    }

    #[test]
    fn o_lembrete_conta_do_ultimo_anuncio_nao_do_inicio_da_disputa() {
        let mut obs = sentado();
        let mut t = 0.0;
        let mut primeiro_lembrete = None;
        while t < 6.0 {
            if let Some(e) = obs.observar(na_pista(LR_DIREITA, t)) {
                if eh_lembrete(e.chave) && primeiro_lembrete.is_none() {
                    primeiro_lembrete = Some(e.sessao_s);
                }
            }
            t += 1.0 / 60.0;
        }
        // "direita" confirma em ~0,12 s; o lembrete vem 2,5 s DEPOIS disso, não 2,5 s
        // depois de o carro aparecer.
        let quando = primeiro_lembrete.expect("o lembrete deveria ter saído");
        assert!(
            (quando - 2.62).abs() < 0.1,
            "primeiro lembrete em {quando}s, esperado ~2,62s"
        );
    }

    #[test]
    fn uma_escalada_logo_depois_de_um_anuncio_nao_e_engolida() {
        let mut p = Vec::new();
        segurar(&mut p, 0.0, 1.0, LR_LIVRE);
        // Carro à esquerda e, 150 ms depois, outro à direita. A segunda notícia é a
        // que importa, e chega dentro do intervalo que uma trava de cadência
        // descartaria.
        segurar(&mut p, 1.0, 1.15, LR_ESQUERDA);
        segurar(&mut p, 1.15, 4.0, LR_DOIS_LADOS);
        assert_eq!(correr(&p), vec!["esquerda", "tres_largos"]);
    }

    #[test]
    fn o_teste_de_radio_sai_ao_sentar_no_carro_e_so_uma_vez() {
        let mut obs = Observador::novo();
        let no_box = |t: f64| Amostra {
            bruto: LR_LIVRE,
            tempo_s: t,
            no_carro: true,
            na_pista: false, // ainda parado no box, antes da largada
        };
        let primeiro = obs.observar(no_box(0.0)).expect("teste de rádio na entrada");
        assert_eq!(primeiro.chave, CHAVE_TESTE);
        let mut t = 1.0 / 60.0;
        while t < 5.0 {
            assert!(obs.observar(no_box(t)).is_none(), "o teste não pode repetir");
            t += 1.0 / 60.0;
        }
    }

    #[test]
    fn voltar_a_garagem_e_reentrar_nao_repete_o_teste() {
        let mut obs = Observador::novo();
        let amostra = |t: f64, dentro: bool| Amostra {
            bruto: LR_LIVRE,
            tempo_s: t,
            no_carro: dentro,
            na_pista: false,
        };
        assert!(obs.observar(amostra(0.0, true)).is_some());
        // Sai do carro e volta: mesma sessão, nada de repetir a apresentação.
        assert!(obs.observar(amostra(1.0, false)).is_none());
        assert!(obs.observar(amostra(2.0, true)).is_none());
    }

    #[test]
    fn sessao_nova_ganha_um_teste_novo() {
        let mut obs = Observador::novo();
        let amostra = |t: f64| Amostra {
            bruto: LR_LIVRE,
            tempo_s: t,
            no_carro: true,
            na_pista: false,
        };
        assert!(obs.observar(amostra(0.0)).is_some());
        assert!(obs.observar(amostra(1.0)).is_none());
        // Relógio volta ao começo = outra sessão. Pode ter mudado o áudio junto.
        assert!(obs.observar(amostra(0.0)).is_none()); // o salto em si só reinicia
        assert_eq!(
            obs.observar(amostra(0.1)).map(|e| e.chave),
            None,
            "ainda sentado: sem subida, sem teste"
        );
        // Agora sim: sai e senta de novo já na sessão nova.
        assert!(obs
            .observar(Amostra {
                bruto: LR_LIVRE,
                tempo_s: 0.2,
                no_carro: false,
                na_pista: false
            })
            .is_none());
        assert_eq!(obs.observar(amostra(0.3)).map(|e| e.chave), Some(CHAVE_TESTE));
    }

    #[test]
    fn o_teste_nao_atropela_o_primeiro_anuncio_de_vizinhanca() {
        let mut obs = Observador::novo();
        // Senta no carro (teste sai) e imediatamente já tem alguém à esquerda.
        assert_eq!(
            obs.observar(na_pista(LR_ESQUERDA, 0.0)).map(|e| e.chave),
            Some(CHAVE_TESTE)
        );
        let mut t = 1.0 / 60.0;
        let mut chaves = Vec::new();
        while t < 2.0 {
            if let Some(e) = obs.observar(na_pista(LR_ESQUERDA, t)) {
                chaves.push(e.chave);
            }
            t += 1.0 / 60.0;
        }
        assert_eq!(chaves, vec!["esquerda"]);
    }

    #[test]
    fn no_box_nao_gera_evento_e_zera_a_disputa() {
        let mut obs = sentado();
        let mut t = 0.0;
        while t < 3.0 {
            obs.observar(Amostra { bruto: LR_ESQUERDA, tempo_s: t, no_carro: true, na_pista: false });
            t += 1.0 / 60.0;
        }
        assert!(obs.estado().eventos.is_empty());
        assert_eq!(obs.estado().vizinhanca, "desligado");
        assert!(!obs.estado().ao_lado);
    }

    #[test]
    fn salto_de_sessao_reinicia_em_vez_de_medir_horas() {
        let mut obs = sentado();
        let mut t = 0.0;
        while t < 3.0 {
            obs.observar(na_pista(LR_ESQUERDA, t));
            t += 1.0 / 60.0;
        }
        assert!(obs.estado().ao_lado);
        // Nova sessão: o relógio volta para o começo.
        assert!(obs.observar(na_pista(LR_ESQUERDA, 0.0)).is_none());
        assert!(!obs.estado().ao_lado);
    }

    #[test]
    fn o_estado_expoe_os_flancos_para_a_tela() {
        let mut obs = sentado();
        let mut t = 0.0;
        while t < 2.0 {
            obs.observar(na_pista(LR_DOIS_LADOS, t));
            t += 1.0 / 60.0;
        }
        let e = obs.estado();
        assert!(e.esquerda && e.direita);
        assert!(e.tres_largos);
        assert_eq!(e.vizinhanca, "tres_largos");
        assert_eq!(e.bruto, LR_DOIS_LADOS);
    }

    #[test]
    fn os_ids_crescem_e_o_historico_e_limitado() {
        let mut obs = sentado();
        let mut t = 0.0;
        // Alterna esquerda/direita bem devagar: cada troca é notícia nova.
        for volta in 0..(MAX_EVENTOS + 20) {
            let lado = if volta % 2 == 0 {
                LR_ESQUERDA
            } else {
                LR_DIREITA
            };
            let ate = t + 1.0;
            while t < ate {
                obs.observar(na_pista(lado, t));
                t += 1.0 / 60.0;
            }
        }
        let e = obs.estado();
        assert_eq!(e.eventos.len(), MAX_EVENTOS);
        assert!(e.eventos.windows(2).all(|w| w[1].id > w[0].id));
    }
}
