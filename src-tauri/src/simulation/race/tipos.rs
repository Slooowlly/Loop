//! Tipos da corrida simulada: o segmento, o estado vivo de cada carro e os DTOs de resultado
//! que atravessam a ponte para o React.

use serde::{Deserialize, Serialize};

use crate::simulation::incidents::{IncidentResult, PendingDamage};
use crate::simulation::qualifying::QualifyingResult;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RaceSegment {
    Start,
    Early,
    Mid,
    Late,
    Finish,
}

impl RaceSegment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "START",
            Self::Early => "EARLY",
            Self::Mid => "MID",
            Self::Late => "LATE",
            Self::Finish => "FINISH",
        }
    }

    /// Ordinal para comparação de segmento em DNF ordering (maior = mais tarde na corrida).
    pub(crate) fn ordinal(self) -> u8 {
        match self {
            Self::Start => 0,
            Self::Early => 1,
            Self::Mid => 2,
            Self::Late => 3,
            Self::Finish => 4,
        }
    }

    /// Em qual dos 5 segmentos uma volta cai. O cérebro da quebra raciocina POR VOLTA; a
    /// simulação raciocina por segmento — é aqui que os dois se encontram. Volta 1 é a
    /// largada; a última volta é o FINISH.
    pub(crate) fn from_lap(lap: u32, total_laps: i32) -> Self {
        let total = total_laps.max(1) as f64;
        let idx = (((lap.max(1) - 1) as f64 / total) * 5.0).floor() as usize;
        match idx.min(4) {
            0 => Self::Start,
            1 => Self::Early,
            2 => Self::Mid,
            3 => Self::Late,
            _ => Self::Finish,
        }
    }
}

/// Desfecho de QUEBRA DE PEÇA pré-rolado pelo cérebro (`car::breakdown`) e injetado na corrida
/// simulada — a Fase 7 do Sistema de Quebra. A simulação não sabe de peça, desgaste nem
/// economia: recebe só "este piloto ficou N segundos parado (ou abandonou) nesta volta" e
/// cobra o preço na moeda dela. Quem sabe de peça é quem rola, em `commands::race`.
#[derive(Debug, Clone)]
pub struct MechanicalOutcome {
    pub pilot_id: String,
    /// Volta em que a peça largou (1-based).
    pub lap: u32,
    /// A quebra encerrou a corrida do carro.
    pub is_dnf: bool,
    /// Segundos parados no box consertando. 0 quando `is_dnf`.
    pub penalty_secs: u32,
    /// Frase do problema ("motor fundiu por superaquecimento") — vira o `dnf_reason` no abandono.
    pub label: String,
    /// A CHAVE de i18n da mesma frase (`car::breakdown::problem_key`) — vira o `dnf_reason_key`
    /// no abandono. Anda junto do `label` porque é ela, e não a prosa, que sobrevive à troca de
    /// idioma do jogo. Vazia só para peça que esta versão não reconhece mais.
    pub problem_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassificationStatus {
    Finished,
    Dnf,
}

#[derive(Debug, Clone)]
pub struct RaceState {
    pub driver_id: String,
    pub tire_wear: f64,
    pub physical_condition: f64,
    /// **A moeda da corrida.** Tempo acumulado em ms desde a largada, medido contra um carro
    /// de referência teórico (ver `RITMO_DE_REFERENCIA`) — MENOR é melhor, e a diferença
    /// entre dois carros é literalmente o gap entre eles na pista.
    ///
    /// Substituiu o `cumulative_score` (pontos, MAIOR era melhor). A troca é o que destrava
    /// tudo o que depende de distância entre carros: ar sujo, trem de carros, undercut,
    /// safety car. Nenhum deles existe ainda — mas agora a pergunta "quanto falta pro carro
    /// da frente?" tem resposta, via [`pelotao_ordenado`].
    pub tempo_acumulado_ms: f64,
    /// Estado do AR(1) do ruído de ritmo, em desvios-padrão. Dá MEMÓRIA ao ruído: um piloto
    /// num trecho ruim tende a seguir num trecho ruim, o que impede o ruído de se
    /// auto-cancelar na soma da corrida.
    pub desvio_de_ritmo: f64,
    /// Dado cru de POSIÇÃO NA PISTA acumulado durante a corrida: gaps, ar sujo, tentativas
    /// de ultrapassagem e trem. Sai inteiro no `RaceDriverResult` — métrica é do harness.
    pub trafego: super::trafego::TrafegoDoCarro,
    /// Plano e histórico de paradas deste carro (pacote G).
    pub paradas: super::estrategia::ParadasDoCarro,
    pub is_dnf: bool,
    pub current_position: i32,
    pub incidents: Vec<IncidentResult>,
    pub dnf_reason: Option<String>,
    /// Chave de i18n do motivo, quando o abandono veio do sistema de QUEBRA. Ver
    /// [`RaceDriverResult::dnf_reason_key`].
    pub dnf_reason_key: Option<String>,
    pub dnf_segment: Option<RaceSegment>,
    /// Danos latentes pós-colisão aguardando manifestação.
    pub pending_damage: Vec<PendingDamage>,
}

impl RaceState {
    /// Gap deste carro para `outro`, em ms. Positivo = este está ATRÁS.
    pub fn gap_para(&self, outro: &RaceState) -> f64 {
        self.tempo_acumulado_ms - outro.tempo_acumulado_ms
    }
}

/// Onde um carro está no pelotão neste instante da corrida — a foto que o modelo de posição
/// na pista vai consumir.
#[derive(Debug, Clone)]
pub struct PosicaoNaPista<'a> {
    pub driver_id: &'a str,
    pub posicao: i32,
    pub tempo_acumulado_ms: f64,
    /// Gap para o carro imediatamente à frente, em ms. `None` para o líder.
    pub gap_para_da_frente_ms: Option<f64>,
    /// Gap para o líder, em ms. 0 para o próprio líder.
    pub gap_para_o_lider_ms: f64,
}

/// O pelotão ordenado por tempo, já com os gaps calculados.
///
/// **Porta aberta de propósito.** O próximo passo (ar sujo, dificuldade de ultrapassagem)
/// precisa perguntar, a cada trecho, "quanto falta pro carro da frente?" — com a moeda em
/// pontos essa pergunta não tinha resposta; com tempo, é uma subtração. Abandonos ficam de
/// fora: quem parou não ocupa mais pista.
pub fn pelotao_ordenado(states: &[RaceState]) -> Vec<PosicaoNaPista<'_>> {
    let mut vivos: Vec<&RaceState> = states.iter().filter(|s| !s.is_dnf).collect();
    vivos.sort_by(|a, b| {
        a.tempo_acumulado_ms
            .partial_cmp(&b.tempo_acumulado_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let lider = vivos.first().map(|s| s.tempo_acumulado_ms).unwrap_or(0.0);
    vivos
        .iter()
        .enumerate()
        .map(|(idx, state)| PosicaoNaPista {
            driver_id: state.driver_id.as_str(),
            posicao: idx as i32 + 1,
            tempo_acumulado_ms: state.tempo_acumulado_ms,
            gap_para_da_frente_ms: idx
                .checked_sub(1)
                .and_then(|anterior| vivos.get(anterior))
                .map(|frente| state.tempo_acumulado_ms - frente.tempo_acumulado_ms),
            gap_para_o_lider_ms: state.tempo_acumulado_ms - lider,
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceDriverResult {
    pub pilot_id: String,
    pub pilot_name: String,
    pub team_id: String,
    pub team_name: String,
    pub grid_position: i32,
    pub finish_position: i32,
    pub positions_gained: i32,
    pub best_lap_time_ms: f64,
    pub total_race_time_ms: f64,
    pub gap_to_winner_ms: f64,
    pub is_dnf: bool,
    /// O motivo do abandono como ele foi RENDERIZADO no idioma da corrida.
    ///
    /// Continua sendo a única coisa que existe para abandono que não é quebra (batida, erro de
    /// pilotagem, pane do catálogo de incidentes) e para todo save anterior à v68. Ver
    /// [`RaceDriverResult::dnf_reason_key`] e [`RaceDriverResult::dnf_reason_no_locale`].
    pub dnf_reason: Option<String>,
    /// A CHAVE de i18n do motivo, quando o abandono veio do sistema de QUEBRA.
    ///
    /// **É ela que atravessa o save, não a prosa.** Até 12/08/2026 o `dnf_reason` de uma quebra
    /// era gravado no idioma ativo na hora da corrida e ficava assim para sempre: quem jogava em
    /// pt-BR e trocava para en-US via o histórico estruturado da quebra em inglês
    /// (`RaceBreakdownRow::label_no_locale` já recompunha) e o motivo do resultado, ao lado,
    /// congelado em português.
    ///
    /// `None` em três casos, todos legítimos: abandono que NÃO é quebra, linha gravada antes da
    /// v68, e peça que esta versão não conhece mais. Nos três a leitura cai no `dnf_reason`.
    #[serde(default)]
    pub dnf_reason_key: Option<String>,
    pub dnf_segment: Option<String>,
    #[serde(default)]
    pub incidents_count: i32,
    #[serde(default)]
    pub incidents: Vec<IncidentResult>,
    pub has_fastest_lap: bool,
    pub points_earned: i32,
    pub is_jogador: bool,
    pub laps_completed: i32,
    pub final_tire_wear: f64,
    pub final_physical: f64,
    pub classification_status: ClassificationStatus,
    /// Descrição de conveniência do pior incidente (narrative_importance_hint >= 2).
    /// Campo derivado — não é fonte factual primária.
    #[serde(default)]
    pub notable_incident: Option<String>,
    /// ID da entry do catálogo do incidente que causou o DNF.
    #[serde(default)]
    pub dnf_catalog_id: Option<String>,
    /// Segmento de origem do dano (pode diferir do segmento do DNF para dano latente).
    #[serde(default)]
    pub damage_origin_segment: Option<String>,

    // ── Dado CRU de posição na pista (pacote D) ──
    //
    // Os nomes e os tipos abaixo seguem à risca o que o harness pediu em
    // `calibracao::atrito::{DADO_PENDENTE, DADO_PEDIDO_AO_D}`. É dado cru de propósito:
    // agregação e alvo são do harness, não daqui.
    /// Posição ao fim de cada um dos 5 segmentos. Existia em `RaceState::current_position` e
    /// morria em `build_race_results`; agora sobrevive.
    #[serde(default)]
    pub posicoes_por_segmento: Vec<i32>,
    /// Gap para o carro da frente ao fim de cada segmento, em ms. `None` = era o líder.
    #[serde(default)]
    pub gaps_para_da_frente_ms: Vec<Option<f64>>,
    /// Em quantos dos 5 segmentos terminou dentro da janela de ar sujo.
    #[serde(default)]
    pub segmentos_em_ar_sujo: i32,
    /// Quantas vezes tentou passar alguém.
    #[serde(default)]
    pub tentativas_ultrapassagem: i32,
    /// Quantas dessas tentativas vieram. A razão entre os dois é a taxa de sucesso — que
    /// antes deste pacote era implicitamente 100%.
    #[serde(default)]
    pub ultrapassagens_concluidas: i32,
    /// Quantas vezes foi atacado.
    #[serde(default)]
    pub tentativas_sofridas: i32,
    /// Maior sequência de segmentos consecutivos preso atrás de um carro mais lento.
    #[serde(default)]
    pub maior_sequencia_preso: i32,

    // ── Dado CRU de estratégia (pacote G) ──
    //
    // Nomes e tipos exatamente como o harness pediu na seção "Dados pedidos ao G" do
    // BASELINE.md. Dado cru: contar estratégias distintas e medir o undercut é dele.
    /// Voltas em que este carro parou. Vazio = foi de ponta a ponta.
    #[serde(default)]
    pub volta_da_parada: Vec<u32>,
    /// Posição imediatamente antes de cada parada.
    #[serde(default)]
    pub posicao_antes_da_parada: Vec<i32>,
    /// Posição depois de cada parada. O par com o campo acima é o que torna o undercut
    /// mensurável sem reconstruir a corrida.
    #[serde(default)]
    pub posicao_depois: Vec<i32>,
    /// Rótulo da estratégia escolhida pela equipe (ex.: `"1-parada-cedo"`).
    #[serde(default)]
    pub estrategia_id: String,
}

impl RaceDriverResult {
    /// Carimba o abandono POR QUEBRA: a frase no idioma de agora E a chave que a recompõe depois.
    ///
    /// **Ponto único de escrita do par.** Os dois campos existem justamente porque um sozinho não
    /// serve — só a prosa congela no idioma da corrida, só a chave deixa sem texto o save cuja
    /// peça esta versão não conhece mais. Escrever `dnf_reason` de uma quebra por fora deste
    /// método é o bug que a v68 veio fechar, e é o que a guarda
    /// `scripts/tests/dnf-de-quebra-guarda-chave.test.mjs` procura.
    pub fn marcar_dnf_de_quebra(&mut self, label: String, chave: Option<String>) {
        self.dnf_reason = Some(label);
        self.dnf_reason_key = chave;
    }

    /// O motivo do abandono no idioma de AGORA.
    ///
    /// Recompõe da chave quando ela existe e o locale a conhece; cai na prosa gravada em todo o
    /// resto — abandono que não é quebra, save anterior à v68, peça que saiu do jogo. Nunca tenta
    /// traduzir a prosa histórica: texto antigo sai como está.
    pub fn dnf_reason_no_locale(&self) -> Option<String> {
        self.dnf_reason_key
            .as_deref()
            .and_then(crate::car::breakdown::problem_text_from_key)
            .or_else(|| self.dnf_reason.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceResult {
    pub qualifying_results: Vec<QualifyingResult>,
    pub race_results: Vec<RaceDriverResult>,
    pub pole_sitter_id: String,
    pub winner_id: String,
    pub fastest_lap_id: String,
    pub total_laps: i32,
    pub weather: String,
    pub track_name: String,
    #[serde(default)]
    pub total_incidents: i32,
    #[serde(default)]
    pub total_dnfs: i32,
    /// Incidentes com narrative_importance_hint >= 1.
    #[serde(default)]
    pub main_incident_count: i32,
    /// Pilot IDs com incidente headline (hint >= 2).
    #[serde(default)]
    pub notable_incident_pilot_ids: Vec<String>,
    /// Piloto que mais ganhou posições.
    #[serde(default)]
    pub most_positions_gained_id: Option<String>,
    /// Segmentos da corrida NEUTRALIZADOS por bandeira amarela, derivados dos
    /// incidentes (ver `derive_caution_segments`). A amarela é REGISTRADA, não
    /// simulada: não agrupa o pelotão nem mexe em posição nenhuma.
    #[serde(default)]
    pub caution_segments: Vec<String>,
    /// Índices, no slice de [`MechanicalOutcome`] passado à simulação, das quebras que a corrida
    /// de fato COBROU. Um carro que já tinha abandonado por batida antes da volta da quebra não
    /// entra: a peça largaria num carro que não estava mais na pista. O caller persiste só isto
    /// em `race_breakdowns`, pra tela e notícia nunca falarem de quebra que não houve.
    ///
    /// TRANSITÓRIO: vale só no retorno da simulação. `#[serde(skip)]` de propósito — não é
    /// estado da corrida, não vai pro `race_screens.json` nem pro save.
    #[serde(skip)]
    pub applied_mechanicals: Vec<usize>,

    // ── Dado CRU de safety car (pacote G) ──
    /// Volta de entrada de cada safety car da corrida.
    #[serde(default)]
    pub safety_cars: Vec<u32>,
    /// A classificação no momento em que cada safety car entrou. É o que permite medir o
    /// embaralhamento — ρ(ordem pré-SC × chegada) — sem reconstruir a corrida.
    #[serde(default)]
    pub ordem_pre_safety_car: Vec<Vec<String>>,
}
