//! Pontuação de segmento e degradação: os pesos por trecho da corrida, o quanto o carro pesa
//! por categoria e o desgaste de pneu e de físico cobrado ao fim de cada segmento.

use crate::simulation::context::{SimDriver, SimulationContext};
use crate::simulation::math::{
    car_weight_scale, category_car_performance, normalize_car_performance,
};
use crate::simulation::track_profile::TrackCharacter;

use super::tipos::{RaceSegment, RaceState};

// ── Os números INTERNOS do ritmo ──────────────────────────────────────────────────────────
//
// Mesmo movimento que `incidents::risco` já fez com os sorteios, e pela mesma razão: enquanto
// um coeficiente vive solto no corpo da função, ele está fora do alcance de qualquer varredura
// — a de `calibracao::varredura` enxerga campo de contexto, e a leitura humana enxerga nome.
// Um `0.15` no meio de uma expressão de sete fatores não é auditável nem por uma nem por
// outra.
//
// **Nenhum valor mudou ao ganhar nome.** O que muda é que agora dá para vê-los, comparar dois
// e discutir a razão de cada um ao lado dele.
//
// Estado de calibração: **nenhum destes foi medido**. Eles vieram do desenho original do
// motor de pontos e sobreviveram à troca de moeda sem revisão. Ficam declarados aqui como não
// medidos em vez de ficarem implícitos — a mesma honestidade do bloco de `race::trafego`.

/// Quanto o pneu no fim da vida custa de ritmo, como fração do score.
const PESO_DA_PENALIDADE_DE_PNEU: f64 = 0.15;
/// Quanto o piloto exausto custa de ritmo, como fração do score. Só em `Late` e `Finish`.
const PESO_DA_PENALIDADE_DE_FADIGA: f64 = 0.10;
/// Quanto a adaptabilidade rende por ponto de dificuldade acima de 1,0.
const BONUS_DE_ADAPTABILIDADE_EM_PISTA_DIFICIL: f64 = 0.05;
/// Idem para a consistência. Menor que o da adaptabilidade de propósito: pista difícil cobra
/// mais de quem lê a pista do que de quem repete a volta.
const BONUS_DE_CONSISTENCIA_EM_PISTA_DIFICIL: f64 = 0.03;
/// Ponto fixo em torno do qual o `race_pace_spread_multiplier` comprime ou abre o campo.
/// Vive perto do ritmo mediano do grid — mexer nele desloca o pelotão inteiro, não o spread.
const MEIO_DA_ESCALA_DE_RITMO: f64 = 60.0;
/// Quantas corridas na categoria até o piloto deixar de pagar pedágio de novato.
const CORRIDAS_ATE_CONHECER_A_CATEGORIA: i32 = 10;
/// Fração de ritmo perdida por corrida que falta para [`CORRIDAS_ATE_CONHECER_A_CATEGORIA`].
const PENALIDADE_POR_CORRIDA_QUE_FALTA: f64 = 0.003;
/// Piso do ritmo: nem o pior fim de semana anda "para trás".
const PISO_DO_RITMO: f64 = 5.0;

/// Amplitude do ruído no piloto de consistência ZERO, em pontos. É o topo da escala — a
/// consistência do piloto desce dali linearmente até 0.
const AMPLITUDE_MAXIMA_DE_RUIDO_PONTOS: f64 = 5.0;

/// Quanto a `gestao_pneus` chega a segurar a degradação, no máximo.
const GESTAO_SEGURA_DO_DESGASTE: f64 = 0.50;
/// Quanto a `smoothness` chega a segurar a degradação, por cima da gestão.
const SUAVIDADE_SEGURA_DO_DESGASTE: f64 = 0.20;
/// Quanto o `fitness` chega a segurar o desgaste FÍSICO.
const FITNESS_SEGURA_DO_DESGASTE: f64 = 0.60;
/// Duração de referência da corrida, em minutos. A degradação escala pela razão contra ela.
const DURACAO_DE_REFERENCIA_MIN: f64 = 30.0;
/// Piso do fator de duração — corrida curta demais ainda desgasta alguma coisa.
const PISO_DO_FATOR_DE_DURACAO: f64 = 0.25;
/// Quanto de pneu sobra no pior caso (1,0 = pneu novo).
const PISO_DO_PNEU: f64 = 0.1;
/// Quanto de condição física sobra no pior caso.
const PISO_DA_CONDICAO_FISICA: f64 = 0.2;

/// Ajuste relativo de `skill`, `car_performance` e `adaptabilidade` pelo caráter da pista.
///
/// Estava inline no `match` de [`calculate_segment_score`]. Virou tabela para que o desenho
/// fique legível como desenho: lida em coluna, ela diz que pista de fluxo premia carro e
/// velocidade e castiga quem depende de se adaptar, e que pista travada faz o oposto.
fn bias_do_carater(character: TrackCharacter) -> (f64, f64, f64) {
    match character {
        TrackCharacter::Flowing => (0.02, 0.02, -0.03),
        TrackCharacter::Technical => (0.00, 0.00, 0.00),
        TrackCharacter::Tight => (-0.03, -0.04, 0.05),
        TrackCharacter::Roval => (0.04, 0.03, -0.05),
    }
}

#[derive(Debug, Clone, Copy)]
struct SegmentWeights {
    skill: f64,
    habilidade_largada: f64,
    racecraft: f64,
    car_performance: f64,
    gestao_pneus: f64,
    fitness: f64,
    mentalidade: f64,
    confianca: f64,
}

fn segment_weights(segment: RaceSegment) -> SegmentWeights {
    match segment {
        RaceSegment::Start => SegmentWeights {
            skill: 0.20,
            habilidade_largada: 0.35,
            racecraft: 0.25,
            car_performance: 0.20,
            gestao_pneus: 0.0,
            fitness: 0.0,
            mentalidade: 0.0,
            confianca: 0.0,
        },
        RaceSegment::Early => SegmentWeights {
            skill: 0.35,
            habilidade_largada: 0.0,
            racecraft: 0.20,
            car_performance: 0.30,
            gestao_pneus: 0.15,
            fitness: 0.0,
            mentalidade: 0.0,
            confianca: 0.0,
        },
        RaceSegment::Mid => SegmentWeights {
            skill: 0.35,
            habilidade_largada: 0.0,
            racecraft: 0.0,
            car_performance: 0.30,
            gestao_pneus: 0.20,
            fitness: 0.15,
            mentalidade: 0.0,
            confianca: 0.0,
        },
        RaceSegment::Late => SegmentWeights {
            skill: 0.25,
            habilidade_largada: 0.0,
            racecraft: 0.0,
            car_performance: 0.20,
            gestao_pneus: 0.25,
            fitness: 0.20,
            mentalidade: 0.10,
            confianca: 0.0,
        },
        RaceSegment::Finish => SegmentWeights {
            skill: 0.25,
            habilidade_largada: 0.0,
            racecraft: 0.25,
            car_performance: 0.20,
            gestao_pneus: 0.0,
            fitness: 0.0,
            mentalidade: 0.10,
            confianca: 0.20,
        },
    }
}

/// Escala o peso do `car_performance` por um fator e redistribui o delta
/// proporcionalmente aos demais pesos (de piloto), preservando a soma = 1.0.
/// fator < 1 desloca influência do carro para o piloto (rookie); fator > 1
/// aumenta a do carro (topo).
fn scale_segment_car_weight(mut w: SegmentWeights, scale: f64) -> SegmentWeights {
    let car = w.car_performance;
    let new_car = car * scale;
    let non_car_total = w.skill
        + w.habilidade_largada
        + w.racecraft
        + w.gestao_pneus
        + w.fitness
        + w.mentalidade
        + w.confianca;
    if non_car_total > 0.0 {
        // Tudo que sai (ou entra) no carro é redistribuído nos pesos de piloto.
        let factor = (non_car_total + (car - new_car)) / non_car_total;
        w.skill *= factor;
        w.habilidade_largada *= factor;
        w.racecraft *= factor;
        w.gestao_pneus *= factor;
        w.fitness *= factor;
        w.mentalidade *= factor;
        w.confianca *= factor;
    }
    w.car_performance = new_car;
    w
}

/// O RITMO DE VOLTA deste piloto neste trecho, em pontos (maior = mais rápido).
///
/// Continua sendo o cérebro da simulação — a combinação de piloto, carro, pneu, físico,
/// clima e caráter de pista. O que mudou é o que ele produz: era uma parcela de um saldo de
/// pontos, virou um ritmo que o motor converte em tempo. **Determinístico**: o ruído saiu
/// daqui (ver [`amplitude_de_ritmo`]).
pub(crate) fn calculate_segment_score(
    driver: &SimDriver,
    state: &RaceState,
    segment: RaceSegment,
    ctx: &SimulationContext,
) -> f64 {
    // Peso do carro escalado por categoria (rookie baixo, topo alto); carro spec
    // no rookie (todos idênticos). Pilar A do redesign carro/dinastias.
    let car_scale = car_weight_scale(&ctx.category_id);
    let weights = scale_segment_car_weight(segment_weights(segment), car_scale);
    let car_norm = normalize_car_performance(category_car_performance(
        &ctx.category_id,
        driver.car_performance,
    ));
    let mut score = driver.skill as f64 * weights.skill
        + driver.habilidade_largada as f64 * weights.habilidade_largada
        + driver.racecraft as f64 * weights.racecraft
        + car_norm * weights.car_performance
        + driver.gestao_pneus as f64 * weights.gestao_pneus
        + driver.fitness as f64 * weights.fitness
        + driver.mentalidade as f64 * weights.mentalidade
        + driver.confianca as f64 * weights.confianca;

    // Penalidade de pneu
    let tire_penalty = (1.0 - state.tire_wear) * PESO_DA_PENALIDADE_DE_PNEU;
    score *= 1.0 - tire_penalty;

    // Penalidade de fadiga (apenas Late e Finish)
    if matches!(segment, RaceSegment::Late | RaceSegment::Finish) {
        let fatigue_penalty = (1.0 - state.physical_condition) * PESO_DA_PENALIDADE_DE_FADIGA;
        score *= 1.0 - fatigue_penalty;
    }

    // Chuva: MESMA penalidade de skill por piloto do export iRacing (curva validada
    // `rain_skill_penalty`). Seco = 0. Rain-good (fator alto) perde MENOS pontos →
    // re-rank consistente: o pelotão todo cai e os bons-de-chuva sobem relativos.
    // (O score está na escala ~0–100 do skill, então subtrair os pontos casa com o export.)
    // A `rain_sensitivity` da pista/categoria ESCALA a curva validada (pacote G). Antes ela era
    // calculada no perfil, guardada no contexto e nunca lida — chuva rendia igual em Spa e em
    // Tsukuba. Sensibilidade 1,0 devolve exatamente a penalidade de antes.
    score -= crate::simulation::math::rain_penalty_escalada(
        ctx.weather,
        driver.fator_chuva as f64,
        ctx.rain_sensitivity,
    );

    // Bônus contextual em pista difícil: adaptabilidade vale mais
    if ctx.track_difficulty_multiplier > 1.0 {
        let difficulty_bonus = (driver.adaptabilidade as f64 / 100.0)
            * (ctx.track_difficulty_multiplier - 1.0)
            * BONUS_DE_ADAPTABILIDADE_EM_PISTA_DIFICIL;
        let consistency_bonus = (driver.consistencia as f64 / 100.0)
            * (ctx.track_difficulty_multiplier - 1.0)
            * BONUS_DE_CONSISTENCIA_EM_PISTA_DIFICIL;
        score += difficulty_bonus + consistency_bonus;
    }

    // Bias de caráter de pista: pequenos ajustes relativos de atributos (skill, car, adaptabilidade)
    let (char_skill_bias, char_car_bias, char_adapt_bias) = bias_do_carater(ctx.track_character);
    score += driver.skill as f64 * char_skill_bias
        + car_norm * char_car_bias * car_scale
        + driver.adaptabilidade as f64 * char_adapt_bias;

    // Comprime ou expande spread de habilidade (endurance = campo mais fechado, rookie = mais aberto)
    score = MEIO_DA_ESCALA_DE_RITMO
        + (score - MEIO_DA_ESCALA_DE_RITMO) * ctx.race_pace_spread_multiplier;

    if driver.corridas_na_categoria < CORRIDAS_ATE_CONHECER_A_CATEGORIA {
        let inexperience_factor = (CORRIDAS_ATE_CONHECER_A_CATEGORIA - driver.corridas_na_categoria)
            .max(0) as f64
            * PENALIDADE_POR_CORRIDA_QUE_FALTA;
        score *= 1.0 - inexperience_factor;
    }

    score.max(PISO_DO_RITMO)
}

/// Amplitude do ruído de ritmo deste piloto neste trecho, em pontos de ritmo.
///
/// Era a faixa do sorteio que vivia dentro de [`calculate_segment_score`]. Saiu de lá porque
/// o ruído deixou de ser "um número por segmento": agora ele tem ESCALA (por volta, não por
/// segmento) e MEMÓRIA (correlacionado entre trechos), e as duas coisas são do laço da
/// corrida, não do cálculo de ritmo. A fórmula em si não mudou.
pub(crate) fn amplitude_de_ritmo(
    driver: &SimDriver,
    ctx: &SimulationContext,
    segment: RaceSegment,
) -> f64 {
    amplitude_de_ritmo_com_relancamento(driver, ctx, segment, false)
}

/// Idem, com a opção de tratar o trecho como RELANÇAMENTO de safety car.
///
/// Um relançamento é uma segunda largada — pneu frio, pelotão colado, acordeão — então ele
/// reusa exatamente a amplificação de caos que o segmento de largada já tinha, em vez de um
/// mecanismo novo. É o que impede o safety car de ser só uma animação que comprime gaps sem
/// mudar nada (ver `estrategia::CAOS_DO_RELANCAMENTO`).
pub(crate) fn amplitude_de_ritmo_com_relancamento(
    driver: &SimDriver,
    ctx: &SimulationContext,
    segment: RaceSegment,
    relancamento: bool,
) -> f64 {
    let base = (100.0 - driver.consistencia as f64) / 100.0 * AMPLITUDE_MAXIMA_DE_RUIDO_PONTOS;
    let escalada = base * ctx.race_variance_multiplier;

    // Caos extra na largada — e no relançamento — amplificado por densidade do pelotão.
    if segment == RaceSegment::Start {
        escalada * ctx.start_chaos_multiplier * ctx.pack_density_factor
    } else if relancamento {
        escalada
            * ctx.start_chaos_multiplier
            * ctx.pack_density_factor
            * super::estrategia::CAOS_DO_RELANCAMENTO
    } else {
        escalada
    }
}

pub(crate) fn apply_tire_degradation(
    state: &mut RaceState,
    driver: &SimDriver,
    ctx: &SimulationContext,
) {
    let mgmt_factor = 1.0 - (driver.gestao_pneus as f64 / 100.0 * GESTAO_SEGURA_DO_DESGASTE);
    let smoothness_factor = 1.0 - (driver.smoothness as f64 / 100.0 * SUAVIDADE_SEGURA_DO_DESGASTE);
    let duration_factor = (ctx.race_duration_minutes as f64 / DURACAO_DE_REFERENCIA_MIN)
        .max(PISO_DO_FATOR_DE_DURACAO);
    let actual_degradation =
        ctx.tire_degradation_rate * mgmt_factor * smoothness_factor * duration_factor;
    state.tire_wear = (state.tire_wear - actual_degradation).max(PISO_DO_PNEU);
}

pub(crate) fn apply_physical_degradation(
    state: &mut RaceState,
    driver: &SimDriver,
    ctx: &SimulationContext,
) {
    let fit_factor = 1.0 - (driver.fitness as f64 / 100.0 * FITNESS_SEGURA_DO_DESGASTE);
    let duration_factor = (ctx.race_duration_minutes as f64 / DURACAO_DE_REFERENCIA_MIN)
        .max(PISO_DO_FATOR_DE_DURACAO);
    let actual_degradation = ctx.physical_degradation_rate * fit_factor * duration_factor;
    state.physical_condition =
        (state.physical_condition - actual_degradation).max(PISO_DA_CONDICAO_FISICA);
}
