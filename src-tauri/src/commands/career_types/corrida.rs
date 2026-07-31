//! DTOs da próxima corrida e do briefing de fim de semana.

use serde::{Deserialize, Serialize};

use crate::event_interest::EventInterestSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceSummary {
    pub id: String,
    pub rodada: i32,
    pub track_name: String,
    pub clima: String,
    pub duracao_corrida_min: i32,
    pub status: String,
    pub temperatura: f64,
    pub horario: String,
    pub week_of_year: i32,
    pub season_phase: String,
    pub display_date: String,
    /// Papel narrativo da corrida (ex.: "FinalDaTemporada"/"FinalEspecial" marcam
    /// o final de campeonato). Usado pela UI para decidir a aba pós-corrida.
    pub thematic_slot: String,
    pub event_interest: Option<EventInterestSummary>,
    /// Cota do público/bilheteria que a equipe do JOGADOR captura neste evento (Fase 3
    /// do Estrelato): piso + prêmio de fama do lineup, ∈ [0,1]. `None` quando o jogador
    /// não tem equipe. Alimenta a linha "sua estrela puxa ~Y% do público" na Sala de
    /// Estratégia.
    pub public_fame_share: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractWarningInfo {
    pub temporada_fim: i32,
    pub equipe_nome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextRaceBriefingSummary {
    pub track_history: Option<TrackHistorySummary>,
    pub primary_rival: Option<PrimaryRivalSummary>,
    #[serde(default)]
    pub weekend_stories: Vec<BriefingStorySummary>,
    pub contract_warning: Option<ContractWarningInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackHistorySummary {
    pub has_data: bool,
    pub starts: i32,
    pub best_finish: Option<i32>,
    pub last_finish: Option<i32>,
    pub dnfs: i32,
    pub last_visit_season: Option<i32>,
    pub last_visit_round: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryRivalSummary {
    pub driver_id: String,
    pub driver_name: String,
    pub championship_position: i32,
    pub gap_points: i32,
    pub is_ahead: bool,
    pub rivalry_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingStorySummary {
    pub id: String,
    pub icon: String,
    pub title: String,
    pub summary: String,
    pub importance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BriefingPhraseHistory {
    pub season_number: i32,
    #[serde(default)]
    pub entries: Vec<BriefingPhraseEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingPhraseEntry {
    #[serde(default)]
    pub season_number: i32,
    pub round_number: i32,
    pub driver_id: String,
    pub bucket_key: String,
    pub phrase_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingPhraseEntryInput {
    pub round_number: i32,
    pub driver_id: String,
    pub bucket_key: String,
    pub phrase_id: String,
}

/// A LEITURA de uma corrida: o dado que o motor calculou pra explicar cada posição
/// final e que, antes da v55, não sobrevivia ao save (ver `db::migrations`).
///
/// É o insumo do traçado de posição por trecho, do custo do box, do trânsito e do
/// safety car na tela pós-corrida. Corrida gravada antes da v55 volta com tudo
/// neutro — a tela distingue "não aconteceu" de "não foi gravado" pelo vetor VAZIO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceReading {
    pub race_id: String,
    /// Total de voltas da etapa — o eixo X do traçado precisa dele pra mapear trecho
    /// em volta (o motor divide a corrida em 5 trechos iguais).
    pub total_laps: i32,
    /// Quantos trechos o motor usa. Vem da simulação, não é chumbado na tela.
    pub total_segments: i32,
    pub cars: Vec<RaceReadingCar>,
    pub safety_cars: Vec<RaceReadingSafetyCar>,
}

/// A leitura de UM carro na corrida.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceReadingCar {
    pub pilot_id: String,
    pub pilot_name: String,
    pub is_jogador: bool,
    pub grid_position: i32,
    pub finish_position: i32,
    pub is_dnf: bool,
    /// Posição ao fim de cada trecho. Vazio = corrida anterior à v55.
    pub segment_positions: Vec<i32>,
    /// Gap pro carro da frente ao fim de cada trecho, em ms. `null` = era o líder.
    pub segment_gaps_ms: Vec<Option<f64>>,
    pub dirty_air_segments: i32,
    pub overtake_attempts: i32,
    pub overtakes_completed: i32,
    pub attempts_suffered: i32,
    pub longest_stuck_streak: i32,
    /// Rótulo da estratégia da equipe (ex.: `"1-parada-cedo"`). Vazio = não gravado.
    pub strategy_id: String,
    /// Os três vetores são PARALELOS: o i-ésimo elemento descreve a i-ésima parada.
    pub pit_laps: Vec<u32>,
    pub position_before_pit: Vec<i32>,
    pub position_after_pit: Vec<i32>,
    /// A leitura do fim de semana ANUNCIADA antes desta corrida (v56), como foi gravada.
    ///
    /// Vem do banco, NÃO é recomputada na exibição — a faixa anunciada é fato histórico do
    /// fim de semana, e recomputá-la faria uma recalibração do σ mudar a leitura de uma
    /// corrida antiga, deixando o pós em desacordo com o que o pré anunciou.
    ///
    /// `None` = não anunciado (corrida anterior à v56, import do iRacing, ou motor ainda
    /// sem a leitura). A tela não mostra nada — melhor calar do que divergir.
    pub announced_weekend_reading: Option<WeekendReading>,
}

/// Um safety car da corrida.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceReadingSafetyCar {
    pub lap: u32,
    /// A classificação no instante em que ele entrou, em ordem de posição. É o que
    /// permite mostrar quanto a amarela embaralhou a etapa.
    pub order_before: Vec<String>,
}

/// A LEITURA DO FIM DE SEMANA — as três camadas intermediárias de performance
/// (`simulation::forma`) reduzidas ao que o jogador pode ler.
///
/// Existe para fechar a distância entre "variação atribuível a qualidade" e "o jogador
/// consegue atribuir". Atribuível é propriedade do modelo; atribuir é ato do jogador, e
/// sem este DTO a segunda metade nunca acontece — o acerto do fim de semana é
/// provavelmente a camada de maior impacto das três e hoje não aparece em lugar nenhum.
///
/// **Três camadas, não duas.** O critério é a ESCALA DE TEMPO, que é o princípio
/// organizador do próprio `forma.rs`, e nela as três têm assinaturas distintas:
///
/// | camada | continuidade | ρ no intervalo que importa |
/// |---|---|---|
/// | `track_affinity` | periódica (visita → visita à mesma pista) | 1,0 — é hash de `(piloto, pista)`, idêntica todo ano |
/// | `driver_form` | serial (etapa → etapa) | 0,65 — o AR(1) |
/// | `car_setup` | nenhuma | 0 — sorteada por `(equipe, evento)` |
///
/// Fundir duas quaisquer mistura HISTÓRIA com RUÍDO: `car_setup` é a única sem
/// informação sobre qualquer corrida futura, então é ela a que precisa ficar sozinha.
/// Ver `docs/fase3-fim-de-semana-atribuivel.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekendReading {
    pub race_id: String,
    /// `false` = o motor não forneceu a leitura (save antigo, categoria sem o dado).
    /// A tela não desenha NADA — a mesma regra do vazio da v55: melhor ausente que
    /// errado.
    pub available: bool,
    /// Permanente por pista. É a única camada verificável contra a memória do jogador
    /// ("sempre fui bem aqui") e contra dado que o jogo já mostra
    /// (`historico_circuitos`) — o que a torna o antídoto mais forte contra "é azar".
    pub track_affinity: WeekendLayer,
    /// O momento do piloto. A ÚNICA com autocorrelação serial, e por isso a única que
    /// carrega tendência.
    pub driver_form: WeekendLayer,
    /// O acerto deste fim de semana. Evento isolado: não diz nada sobre a próxima etapa.
    pub car_setup: WeekendLayer,
}

/// Uma camada de fim de semana, já reduzida a faixa qualitativa.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekendLayer {
    /// Faixa ORDINAL em [-2, 2] no canal de RITMO: -2 muito contra, 0 neutro,
    /// +2 muito a favor. É a manchete da camada.
    ///
    /// Ordinal de propósito — o valor BRUTO não cruza a ponte. Número exato na tela
    /// convida engenharia reversa (em quatro corridas o jogador deduz a escala e passa a
    /// calcular em vez de correr) e promete uma precisão que a campanha de calibração vai
    /// desmentir, porque ela vai mexer nas três camadas.
    pub band: i8,
    /// A mesma faixa no canal de CLASSIFICAÇÃO. Vem separada porque o motor trata os dois
    /// canais de forma assimétrica (a afinidade tem multiplicador de quali e as outras
    /// duas não), e é essa assimetria que explica voar no sábado e não converter no
    /// domingo. Colapsar os canais num número esconderia exatamente isso.
    pub qualifying_band: i8,
    /// Tendência em [-1, 1] (caindo / estável / subindo). `None` nas camadas SEM
    /// autocorrelação: prometer tendência onde ρ = 0 seria inventar arco a partir de
    /// ruído, que é o oposto do que este pacote existe pra fazer.
    ///
    /// # ⚠ Condicional: hoje esta seta é uma PROMESSA, não uma leitura
    ///
    /// O harness mediu o excesso de sequência da forma com a amplitude atual e deu **0,02
    /// corrida** — estatisticamente indistinguível de uma forma sem memória nenhuma. Quem
    /// sustenta memória PERCEPTÍVEL é a amplitude, não o ρ, e a amplitude ainda está nos
    /// valores de chute inicial.
    ///
    /// Ou seja: `driver_form.trend` descreve um mecanismo que existe no modelo e que o
    /// jogador **ainda não consegue sentir**. Isso virou critério de aceitação da fase 1 da
    /// campanha de calibração.
    ///
    /// **Se a fase 1 fechar sem entregar a perceptibilidade, este campo e a seta na tela
    /// SAEM.** Interface que afirma mecanismo inexistente é exatamente a falha que este
    /// pacote existe para evitar — só que cometida por nós, e é pior, porque desta vez a
    /// causa ilegível seria inventada em vez de apenas escondida.
    pub trend: Option<i8>,
    /// Fato de apoio já resolvido no backend, verificável pelo jogador (ex.: "3 corridas
    /// aqui, melhor resultado P4"). É o que torna a afirmação checável contra a memória
    /// dele em vez de uma alegação do jogo sobre si mesmo. `None` = sem fato a citar.
    pub support: Option<String>,
}

// ── Faixa por σ ───────────────────────────────────────────────────────────────
//
// A faixa é definida em MÚLTIPLOS DO σ DA PRÓPRIA CAMADA, nunca em pontos de skill
// absolutos. Não é refinamento: é o que impede a tela de mentir depois de uma
// recalibração.
//
// A campanha de calibração vai redistribuir as três escalas do `forma.rs` mantendo a
// soma — baixar a da afinidade (hoje 3,0, a maior das três, e a análise diz que deveria
// ser a menor) e subir a do acerto. Com limiares absolutos, a afinidade passaria a marcar
// "neutro" quase sempre e o acerto a saturar em ±2: as duas camadas continuariam corretas
// no dado e MENTIRIAM na leitura, sem quebrar teste nenhum — inclusive o teste de que
// nenhum número chega à tela continuaria passando.
//
// Em σ, a faixa significa "incomum PARA ESTA CAMADA", que é exatamente o que o jogador
// precisa saber, e recalibrar a escala não muda o significado de "muito a favor".

/// Limiar da faixa intermediária (±1), em desvios-padrão da própria camada.
const FAIXA_1_SIGMA: f64 = 1.0;
/// Limiar da faixa extrema (±2), em desvios-padrão da própria camada.
const FAIXA_2_SIGMA: f64 = 2.0;

/// Reduz um valor bruto de camada à faixa ordinal em [-2, 2], medindo em σ DAQUELA
/// camada.
///
/// `sigma` é o desvio-padrão da distribuição da camada — o mesmo parâmetro que a campanha
/// de calibração ajusta, então ele está disponível de qualquer forma.
///
/// `sigma` não-positivo ou não-finito devolve 0 (neutro): sem escala não existe "incomum",
/// e chutar uma faixa a partir de uma escala inválida é precisamente o tipo de leitura
/// errada que a regra do vazio existe para evitar.
pub fn faixa_por_sigma(valor: f64, sigma: f64) -> i8 {
    if !valor.is_finite() || !sigma.is_finite() || sigma <= 0.0 {
        return 0;
    }
    let z = valor / sigma;
    let magnitude = z.abs();
    let grau = if magnitude >= FAIXA_2_SIGMA {
        2
    } else if magnitude >= FAIXA_1_SIGMA {
        1
    } else {
        0
    };
    if grau == 0 || z > 0.0 {
        grau
    } else {
        -grau
    }
}

impl WeekendLayer {
    /// Monta a camada a partir dos valores BRUTOS por canal e do σ da camada.
    ///
    /// É o único caminho para construir uma `WeekendLayer` a partir do motor — o valor
    /// bruto entra aqui e não sai, o que garante por construção que ele nunca cruza a
    /// ponte.
    ///
    /// # Os valores são o PRETENDIDO, não o aplicado
    ///
    /// A API do motor expõe os dois (`pretendido_de(elo, canal)` e o aplicado). Esta tela
    /// usa o **pretendido**, por quatro razões, a última das quais é impeditiva:
    ///
    /// 1. É o que o jogador está lendo. No sábado ele recebe a leitura da EQUIPE sobre o
    ///    fim de semana, não o efeito residual depois da aritmética interna da esteira. A
    ///    tela descreve o mundo, não o motor.
    /// 2. O aplicado carrega o arredondamento para `u8`, que é artefato de REPRESENTAÇÃO e
    ///    não do mundo. Uma faixa que se move por quantização seria variação
    ///    NÃO-atribuível exibida como se fosse atribuível — exatamente a inversão que este
    ///    pacote existe para impedir. E seria não-monotônica: dois pretendidos diferentes
    ///    caem no mesmo aplicado, e uma mudança mínima pode virar um degrau inteiro, então
    ///    a leitura tremeria sem causa que o jogador possa nomear.
    /// 3. Não existe aplicado POR CAMADA. As três são somadas e arredondadas uma vez só —
    ///    de propósito, é o que as protege do arredondamento. Mapear do aplicado tornaria a
    ///    decomposição em três camadas impossível, e com ela cai o argumento inteiro de
    ///    história-vs-ruído.
    /// 4. **No momento do anúncio o aplicado ainda não existe.** A faixa é anunciada ANTES
    ///    da corrida, e o aplicado só passa a existir quando a esteira soma para aquela
    ///    etapa. Só o pretendido está disponível quando a frase é dita.
    ///
    /// # `sigma` é UM por camada, não um por canal
    ///
    /// Cuidado ao "corrigir" isto: passar um σ por canal parece mais rigoroso e **apaga
    /// silenciosamente a única coisa que `qualifying_band` existe para mostrar**.
    ///
    /// O canal de classificação da afinidade é o de corrida multiplicado por
    /// `MULT_AFINIDADE_QUALI = 1,5`. Se cada canal fosse normalizado pelo σ da SUA própria
    /// distribuição, o σ da classificação também seria 1,5× maior, os dois z sairiam
    /// idênticos e `qualifying_band` empataria com `band` **sempre** — em todas as camadas,
    /// virando campo morto.
    ///
    /// Normalizando os dois canais pelo MESMO σ da camada (o do pretendido no canal de
    /// corrida), a assimetria sobrevive como fato visível: a afinidade sai mais forte na
    /// classificação, que é a afirmação verdadeira "a sua afinidade pesa mais no sábado que
    /// no domingo" — e é ela que sustenta a frase "voou no sábado e não converteu no
    /// domingo". Ver `um_sigma_por_camada_preserva_a_assimetria_de_canal`.
    pub fn from_sigma(
        valor_ritmo: f64,
        valor_classificacao: f64,
        sigma: f64,
        trend: Option<i8>,
        support: Option<String>,
    ) -> Self {
        Self {
            band: faixa_por_sigma(valor_ritmo, sigma),
            qualifying_band: faixa_por_sigma(valor_classificacao, sigma),
            trend: trend.map(|t| t.clamp(-1, 1)),
            support,
        }
    }

    /// Camada neutra, sem tendência e sem fato de apoio.
    pub fn neutra() -> Self {
        Self {
            band: 0,
            qualifying_band: 0,
            trend: None,
            support: None,
        }
    }
}

#[cfg(test)]
mod tests_faixa {
    use super::*;

    #[test]
    fn faixa_respeita_os_limiares_de_sigma_e_o_sinal() {
        // Dentro de 1σ é neutro; a partir de 1σ é ±1; a partir de 2σ é ±2.
        assert_eq!(faixa_por_sigma(0.0, 2.0), 0);
        assert_eq!(faixa_por_sigma(1.9, 2.0), 0);
        assert_eq!(faixa_por_sigma(2.0, 2.0), 1);
        assert_eq!(faixa_por_sigma(3.9, 2.0), 1);
        assert_eq!(faixa_por_sigma(4.0, 2.0), 2);
        assert_eq!(faixa_por_sigma(40.0, 2.0), 2);
        assert_eq!(faixa_por_sigma(-2.0, 2.0), -1);
        assert_eq!(faixa_por_sigma(-4.0, 2.0), -2);
    }

    /// **O teste que a calibração exige.** Escalar a camada (valor E σ pelo mesmo fator) NÃO
    /// pode mudar a faixa. É o que garante que redistribuir as escalas do `forma.rs` — baixar
    /// a afinidade, subir o acerto — não mexe no significado de "muito a favor".
    ///
    /// Com limiares absolutos em pontos este teste falharia, e é justamente o buraco que
    /// nenhum teste anterior pegava.
    #[test]
    fn faixa_e_invariante_a_escala_da_camada() {
        let casos = [(0.4, 1.0), (1.2, 1.0), (2.5, 1.0), (-1.5, 1.0), (-3.0, 1.0)];
        for fator in [0.25, 0.5, 2.0, 4.0, 10.0] {
            for (valor, sigma) in casos {
                assert_eq!(
                    faixa_por_sigma(valor * fator, sigma * fator),
                    faixa_por_sigma(valor, sigma),
                    "faixa mudou ao escalar camada por {fator} (valor {valor}, sigma {sigma})"
                );
            }
        }
    }

    /// **Trava da decisão de UM σ por camada.** Normalizar os dois canais pelo mesmo σ é o
    /// que faz a assimetria de canal aparecer; normalizar cada canal pelo σ da sua própria
    /// distribuição a apagaria, e `qualifying_band` viraria campo morto.
    ///
    /// Cenário real: a afinidade recebe `MULT_AFINIDADE_QUALI = 1,5` no canal de
    /// classificação, e as outras duas camadas não recebem nada.
    #[test]
    fn um_sigma_por_camada_preserva_a_assimetria_de_canal() {
        const MULT_QUALI: f64 = 1.5;
        let sigma = 1.0;
        let pretendido_corrida = 1.4;

        // Afinidade: mesmo σ nos dois canais → a classificação sai uma faixa acima.
        let afinidade = WeekendLayer::from_sigma(
            pretendido_corrida,
            pretendido_corrida * MULT_QUALI,
            sigma,
            None,
            None,
        );
        assert_eq!(afinidade.band, 1);
        assert_eq!(
            afinidade.qualifying_band, 2,
            "a assimetria de canal da afinidade tem de sobreviver à normalização"
        );

        // O erro que este teste existe para impedir. Ele NÃO é expressável via
        // `from_sigma` — que aceita um σ só, justamente para tornar o erro impossível —,
        // então é modelado no nível de baixo: normalizar cada canal pelo σ DELE.
        let banda_corrida = faixa_por_sigma(pretendido_corrida, sigma);
        let banda_quali_errada =
            faixa_por_sigma(pretendido_corrida * MULT_QUALI, sigma * MULT_QUALI);
        assert_eq!(
            banda_quali_errada, banda_corrida,
            "σ por canal empata os canais — é exatamente o que NÃO queremos"
        );
        assert_ne!(
            afinidade.qualifying_band, banda_quali_errada,
            "e é por isso que a assimetria some: a faixa certa e a errada divergem"
        );

        // Camada sem multiplicador de quali (forma, acerto): os canais empatam por serem
        // iguais no dado, não por normalização — e aí a tela simplesmente não cita o canal.
        let forma = WeekendLayer::from_sigma(1.4, 1.4, sigma, Some(0), None);
        assert_eq!(forma.qualifying_band, forma.band);
    }

    /// Sem escala válida não existe "incomum" — devolve neutro em vez de chutar.
    #[test]
    fn sigma_invalido_e_valor_invalido_caem_em_neutro() {
        assert_eq!(faixa_por_sigma(5.0, 0.0), 0);
        assert_eq!(faixa_por_sigma(5.0, -1.0), 0);
        assert_eq!(faixa_por_sigma(5.0, f64::NAN), 0);
        assert_eq!(faixa_por_sigma(f64::NAN, 2.0), 0);
        assert_eq!(faixa_por_sigma(f64::INFINITY, 2.0), 0);
    }

    #[test]
    fn from_sigma_separa_os_canais_e_limita_a_tendencia() {
        // Canal de classificação com multiplicador (o MULT_AFINIDADE_QUALI da afinidade):
        // mesmo σ, valores diferentes → faixas diferentes. É a assimetria que a tela cita.
        let camada = WeekendLayer::from_sigma(1.2, 2.4, 1.0, Some(7), None);
        assert_eq!(camada.band, 1);
        assert_eq!(camada.qualifying_band, 2);
        assert_eq!(
            camada.trend,
            Some(1),
            "tendência fora de faixa deve ser limitada"
        );
        assert_eq!(WeekendLayer::neutra().trend, None);
    }
}
