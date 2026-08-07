//! Pressão de campeonato → ajuste de performance (clutch/choke). **Lógica pura.**
//!
//! Design (travado com o user). Pressão = INTENSIDADE × DIREÇÃO:
//! - **Intensidade** (o quanto está em jogo): 0 se o piloto não tem chance
//!   matemática de título ou se o título já está decidido. Nas ~5 últimas corridas
//!   sobe conforme acaba; **líder** que abriu vantagem → ×2 (defendendo); **última
//!   corrida** com título aberto → ×3 (pressão imensa, NÃO acumula com o ×2).
//! - **Direção** (ajuda ou atrapalha) pela RESILIÊNCIA = mentalidade + experiência
//!   (experiência vale metade). Resiliente (mental forte / veterano) → **clutch**
//!   (mais rápido, erra menos). Frágil (mental fraco / jovem) → **choke**.
//!
//! Mesma fonte na simulação offline E no export pro iRacing.

use serde::{Deserialize, Serialize};

/// Ponto neutro da resiliência: acima vira clutch, abaixo vira choke.
const NEUTRAL: f64 = 0.55;
/// Tamanho do swing de RITMO (pontos de skill) na intensidade máxima.
const PACE_K: f64 = 3.0;
/// Tamanho do swing na taxa de ERRO.
const ERROR_K: f64 = 0.15;
/// A partir de quantas corridas restantes a pressão começa.
const PRESSURE_WINDOW: u32 = 5;
/// Perseguidor (2º/3º com chance) tem o clutch um pouco mais fácil — baixa o neutro.
const CHASER_NEUTRAL_SHIFT: f64 = 0.07;

/// Situação do piloto na luta pelo título (da matemática do campeonato).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct TitleContext {
    /// Tem chance matemática de título (alcança o líder no que falta).
    pub in_contention: bool,
    /// É o líder SOLITÁRIO (abriu vantagem sobre todos).
    pub is_leader: bool,
    /// Título já decidido (o 2º não alcança o líder nem ganhando tudo).
    pub title_decided: bool,
}

/// Resiliência à pressão (0–1): mentalidade + experiência (experiência vale metade).
pub fn pressure_resilience(mentalidade: f64, experiencia: f64) -> f64 {
    ((mentalidade * 2.0 + experiencia) / 3.0).clamp(0.0, 100.0) / 100.0
}

/// Deriva a situação de título de um piloto a partir dos pontos do grid.
/// `all_points` = pontos de TODOS (incluindo ele); `max_points_per_race` = pontos
/// do vencedor; `races_left` = corridas restantes (incluindo a atual).
pub fn title_context(
    my_points: f64,
    all_points: &[f64],
    races_left: u32,
    max_points_per_race: f64,
) -> TitleContext {
    let max_gain = races_left as f64 * max_points_per_race;
    let leader = all_points.iter().cloned().fold(f64::MIN, f64::max);
    let second = all_points
        .iter()
        .cloned()
        .filter(|&p| p < leader - 1e-6)
        .fold(f64::MIN, f64::max);

    let in_contention = my_points + max_gain >= leader - 1e-6;
    // Líder solitário = ninguém tem pontos ≥ os meus além de mim (abri vantagem).
    let count_top = all_points
        .iter()
        .filter(|&&p| p >= my_points - 1e-6)
        .count();
    let is_leader = count_top == 1;
    let title_decided =
        races_left == 0 || (second != f64::MIN && second + max_gain < leader - 1e-6);

    TitleContext {
        in_contention,
        is_leader,
        title_decided,
    }
}

/// Intensidade da pressão (0 = nenhuma). Ver regras no topo do módulo.
pub fn pressure_intensity(ctx: &TitleContext, races_left: u32) -> f64 {
    if !ctx.in_contention || ctx.title_decided {
        return 0.0;
    }
    if races_left <= 1 {
        return 3.0; // última corrida = pressão imensa (não acumula com o ×2 do líder)
    }
    if races_left > PRESSURE_WINDOW {
        return 0.0; // ainda longe do fim
    }
    // 2..5 corridas: sobe conforme acaba; líder defende = ×2.
    let ramp = (PRESSURE_WINDOW as f64 + 1.0 - races_left as f64) / (PRESSURE_WINDOW as f64 - 1.0);
    ramp * if ctx.is_leader { 2.0 } else { 1.0 }
}

/// O efeito da pressão na performance.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PressureEffect {
    /// Pontos de skill a SOMAR (+ clutch, − choke).
    pub pace_delta: f64,
    /// Multiplicador da taxa de erro (<1 clutch, >1 choke).
    pub error_mult: f64,
}

impl PressureEffect {
    pub const NONE: PressureEffect = PressureEffect {
        pace_delta: 0.0,
        error_mult: 1.0,
    };
}

/// Combina intensidade + resiliência no efeito final. `is_chaser` = briga de baixo
/// (2º/3º com chance), que tem o clutch um pouco mais fácil que o líder.
pub fn pressure_effect(intensity: f64, resilience: f64, is_chaser: bool) -> PressureEffect {
    if intensity <= 0.0 {
        return PressureEffect::NONE;
    }
    let neutral = if is_chaser {
        NEUTRAL - CHASER_NEUTRAL_SHIFT
    } else {
        NEUTRAL
    };
    let dir = resilience - neutral; // + clutch, − choke
    PressureEffect {
        pace_delta: intensity * dir * PACE_K,
        error_mult: (1.0 - intensity * dir * ERROR_K).clamp(0.5, 2.0),
    }
}

/// Atalho: do contexto + atributos direto ao efeito.
pub fn pressure_for(
    ctx: &TitleContext,
    races_left: u32,
    mentalidade: f64,
    experiencia: f64,
) -> PressureEffect {
    let intensity = pressure_intensity(ctx, races_left);
    let is_chaser = ctx.in_contention && !ctx.is_leader;
    pressure_effect(
        intensity,
        pressure_resilience(mentalidade, experiencia),
        is_chaser,
    )
}

// ── Pressão de "casa cheia" (interesse do evento) ─────────────────────────────
//
// Design (travado com o user). Diferente da pressão de TÍTULO em três pontos:
// - **Universal**: pesa em TODO piloto do grid, não só em quem briga pelo título.
// - **Palco = oportunidade**: o holofote é mais chance de se provar que ameaça, então
//   o ponto neutro é MAIS BAIXO que o do título — a maioria sobe (clutch), só o
//   genuinamente frágil trava (choke).
// - **"Algo a provar"**: quem vem em MÁ FASE sente o evento com mais força (amplitude
//   maior, pros dois lados). Quem vem voando já está acostumado ao holofote → pesa menos.
//
// A intensidade da casa cheia SOMA-SE à do título no mesmo piloto (ver `combine`).

/// Ponto neutro da casa cheia — mais baixo que `NEUTRAL` do título: no palco grande
/// mais gente rende do que trava (oportunidade > ameaça).
const EVENT_NEUTRAL: f64 = 0.42;
/// Peso da casa cheia. Calibrado para um swing máximo comparável ao do título
/// (~4 pontos de skill), como decidido com o user. Baixe para atenuar.
const EVENT_PESO: f64 = 1.5;
/// Chegada média de referência: nela o fator de forma é ~1.0 (nem cobrado, nem blindado).
const FORM_REF_FINISH: f64 = 6.0;
/// Quanto cada posição média acima/abaixo da referência mexe no fator de forma.
const FORM_SLOPE: f64 = 0.057;

/// Fator de forma recente: >1 para quem vem mal (tem algo a provar → o evento pesa
/// mais), <1 para quem vem bem. `None` (sem histórico) = neutro (1.0). Clamp [0.7, 1.6].
pub fn form_factor(recent_avg_finish: Option<f64>) -> f64 {
    match recent_avg_finish {
        Some(avg) => (1.0 + (avg - FORM_REF_FINISH) * FORM_SLOPE).clamp(0.7, 1.6),
        None => 1.0,
    }
}

/// Normaliza o score de interesse "de local" do evento (sem protagonismo do jogador
/// nem drama de título) em stakes 0..1. ~20 (rookie no miolo) → 0; ~90 (evento
/// principal) → 1.
pub fn event_stakes_from_score(venue_score: f64) -> f64 {
    ((venue_score - 20.0) / 70.0).clamp(0.0, 1.0)
}

/// Intensidade da casa cheia: stakes do evento × peso × fator de forma. Universal
/// (nunca zera por falta de briga de título; só zera se o evento não desperta público).
pub fn event_pressure_intensity(event_stakes: f64, recent_avg_finish: Option<f64>) -> f64 {
    event_stakes.clamp(0.0, 1.0) * EVENT_PESO * form_factor(recent_avg_finish)
}

/// Efeito da casa cheia: mesma mecânica clutch/choke do título, mas com o neutro
/// mais baixo (`EVENT_NEUTRAL`).
pub fn event_pressure_effect(intensity: f64, resilience: f64) -> PressureEffect {
    if intensity <= 0.0 {
        return PressureEffect::NONE;
    }
    let dir = resilience - EVENT_NEUTRAL; // + clutch, − choke
    PressureEffect {
        pace_delta: intensity * dir * PACE_K,
        error_mult: (1.0 - intensity * dir * ERROR_K).clamp(0.5, 2.0),
    }
}

/// Atalho: dos stakes + forma + atributos direto ao efeito de casa cheia.
pub fn event_pressure_for(
    event_stakes: f64,
    recent_avg_finish: Option<f64>,
    mentalidade: f64,
    experiencia: f64,
) -> PressureEffect {
    let intensity = event_pressure_intensity(event_stakes, recent_avg_finish);
    event_pressure_effect(intensity, pressure_resilience(mentalidade, experiencia))
}

/// Soma dois efeitos de pressão (título + casa cheia) num só: ritmo soma, taxa de
/// erro multiplica (ambos compõem sobre o mesmo piloto).
pub fn combine(a: PressureEffect, b: PressureEffect) -> PressureEffect {
    PressureEffect {
        pace_delta: a.pace_delta + b.pace_delta,
        error_mult: (a.error_mult * b.error_mult).clamp(0.5, 2.0),
    }
}

// ── Conversão skill → pace (headroom) ─────────────────────────────────────────
//
// Design (travado com o user). Skill NÃO é linear: 1 ponto no topo vale milésimos
// preciosos (o piloto já sabe tudo), 1 ponto no fundo é ruído (tem oceano a melhorar).
// Então o Δpace da pressão escala pela posição do piloto na curva, de forma
// ASSIMÉTRICA por direção:
// - SUBIR (clutch) é limitado pelo espaço até o TETO → azarão salta, craque só afina.
// - CAIR (choke) é limitado pelo espaço até o CHÃO → craque desaba, azarão nem sente.
// Vale pro Δpace COMBINADO (título + casa cheia).

/// Piso do skill na curva de headroom (abaixo disso, saturado).
const SKILL_FLOOR: f64 = 40.0;
/// Teto do skill na curva de headroom (acima disso, saturado).
const SKILL_CEIL: f64 = 95.0;
/// Multiplicador mínimo (perto do teto no clutch / perto do chão no choke).
const HEADROOM_MIN: f64 = 0.4;
/// Multiplicador máximo (espaço cheio na direção do movimento).
const HEADROOM_MAX: f64 = 2.5;

/// Multiplicador de headroom do Δpace da pressão, pela posição do piloto na curva de
/// skill e pela direção. `is_clutch` = movimento pra cima (ganho de pace).
pub fn headroom_pace_mult(skill: f64, is_clutch: bool) -> f64 {
    let norm = ((skill - SKILL_FLOOR) / (SKILL_CEIL - SKILL_FLOOR)).clamp(0.0, 1.0);
    let room = if is_clutch { 1.0 - norm } else { norm };
    HEADROOM_MIN + room * (HEADROOM_MAX - HEADROOM_MIN)
}

// ── Pressão de Duelo (rivalidade pessoal / Nemesis) ───────────────────────────
//
// Design (travado com o user, spec 2026-07-18-player-rivalry-nemesis-design §3). A
// rivalidade vivida é a **3ª fonte de pressão**, na mesma forma das outras duas e somada
// via `combine()`. Diferente do título/casa cheia em dois pontos:
// - **Gate de duelo:** só acende quando os dois vão brigar DE VERDADE nesta corrida —
//   mesma categoria (garantido no offline) e pace próximo (mesma faixa do grid). Nemesis
//   10 posições à frente → gate ~0; o arco continua vivo como meta, mas sem pressão hoje.
// - **Neutro MAIS ALTO** que o do título: contra o inimigo a emoção puxa pro erro; só o
//   genuinamente frio (mentalidade + experiência) converte em clutch. É o tempero que a
//   faz sentir como ÓDIO, não como campeonato.
//
// Substitui o antigo bônus fixo de skill (+2 Nemesis / +1 rival) que NÃO passava pela
// máquina de clutch/choke — agora o rival herda resiliência e o efeito é direcional.

/// Ponto neutro da Pressão de Duelo — mais ALTO que `NEUTRAL` do título (0.55) e oposto
/// filosófico do `EVENT_NEUTRAL` (0.42): contra o rival, a maioria tende ao erro.
const RIVALRY_NEUTRAL: f64 = 0.62;
/// Peso/escala da intensidade da rivalidade. Calibrado para um swing de ordem comparável
/// (não dominante) ao bônus fixo que substitui. Baixe para atenuar.
const RIVALRY_PESO: f64 = 1.5;
/// Faixa de pace (pontos de skill efetivo) dentro da qual o duelo está "quente" (gate 1.0).
const DUEL_PACE_BAND: f64 = 6.0;
/// Faixa de pace a partir da qual o duelo está frio (gate 0.0). Entre band e fade, decai linear.
const DUEL_PACE_FADE: f64 = 14.0;

/// Gate de duelo (0..1): 1.0 quando os dois pilotos têm pace próximo (vão brigar lado a
/// lado), decai a 0 conforme o pace se distancia. Traduz o "lado a lado" para a sim
/// probabilística sem precisar de tick-a-tick.
pub fn duel_gate(pace_self: f64, pace_rival: f64) -> f64 {
    let d = (pace_self - pace_rival).abs();
    if d <= DUEL_PACE_BAND {
        1.0
    } else if d >= DUEL_PACE_FADE {
        0.0
    } else {
        1.0 - (d - DUEL_PACE_BAND) / (DUEL_PACE_FADE - DUEL_PACE_BAND)
    }
}

/// Intensidade da Pressão de Duelo (0..~1.5): percebida do par (0..100) × gate de duelo ×
/// postura (Fase 1 = 1.0) × peso. Fria (par sem calor) ou rival distante → ~0.
pub fn rivalry_intensity(perceived: f64, gate: f64, stance_mult: f64) -> f64 {
    (perceived / 100.0).clamp(0.0, 1.0) * gate.clamp(0.0, 1.0) * stance_mult * RIVALRY_PESO
}

/// Efeito da Pressão de Duelo: mesma mecânica clutch/choke, com o neutro mais alto
/// (`RIVALRY_NEUTRAL`) — contra o rival só o frio vira clutch.
pub fn rivalry_pressure_effect(intensity: f64, resilience: f64) -> PressureEffect {
    if intensity <= 0.0 {
        return PressureEffect::NONE;
    }
    let dir = resilience - RIVALRY_NEUTRAL; // + clutch, − choke (afobação contra o rival)
    PressureEffect {
        pace_delta: intensity * dir * PACE_K,
        error_mult: (1.0 - intensity * dir * ERROR_K).clamp(0.5, 2.0),
    }
}

/// Atalho: da percebida do par + pace dos dois + atributos direto ao efeito de duelo.
/// `stance_mult` fica 1.0 na Fase 1 (sem postura pré-corrida do jogador ainda).
pub fn rivalry_pressure_for(
    perceived: f64,
    pace_self: f64,
    pace_rival: f64,
    mentalidade: f64,
    experiencia: f64,
    stance_mult: f64,
) -> PressureEffect {
    let intensity = rivalry_intensity(perceived, duel_gate(pace_self, pace_rival), stance_mult);
    rivalry_pressure_effect(intensity, pressure_resilience(mentalidade, experiencia))
}

// ── Motivação → ritmo ─────────────────────────────────────────────────────────
//
// Design (travado com o user): a motivação (recalculada no fim de temporada em
// `evolution/motivation.rs`) hoje só decide aposentadoria; um craque desmotivado corre a
// todo vapor. Aqui ela vira um modificador MODESTO e calibrável de pace: um piloto
// desmotivado rende ABAIXO do próprio skill. Assimétrico de propósito — a desmotivação
// arrasta bem mais do que a motivação bonifica (empolgação não te deixa mais rápido que
// o teto do talento, mas desânimo claramente pesa).

/// Motivação "neutra" (default do piloto, sem efeito). Acima dá um empurrãozinho, abaixo pesa.
///
/// É também a linha de base para onde a motivação DECAI a cada virada
/// ([`crate::evolution::motivation`]). Os dois modelos precisam concordar sobre o
/// que é "neutro": se o pace considera 70 o zero e a evolução puxasse para outro
/// número, o grid inteiro viveria de um lado só da régua.
pub(crate) const MOTIVATION_REF: f64 = 70.0;
/// Pace (pontos de skill) PERDIDO na motivação mínima (0).
const MOTIVATION_DEFICIT_SPAN: f64 = 2.5;
/// Pace (pontos de skill) GANHO na motivação máxima (100). Bem menor que o déficit.
const MOTIVATION_SURPLUS_SPAN: f64 = 0.8;

/// Δpace (pontos de skill) da motivação: 0 na referência, negativo abaixo (desânimo
/// arrasta), levemente positivo acima. Aplicado direto ao skill efetivo (não passa pelo
/// headroom — é um piso de rendimento, não um evento de clutch/choke).
pub fn motivation_pace_delta(motivacao: f64) -> f64 {
    let m = motivacao.clamp(0.0, 100.0);
    if m >= MOTIVATION_REF {
        (m - MOTIVATION_REF) / (100.0 - MOTIVATION_REF) * MOTIVATION_SURPLUS_SPAN
    } else {
        -((MOTIVATION_REF - m) / MOTIVATION_REF) * MOTIVATION_DEFICIT_SPAN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experiencia_vale_metade() {
        // Mentalidade 100 sozinha → 0.667; experiência 100 sozinha → 0.333 (metade).
        assert!((pressure_resilience(100.0, 0.0) - 2.0 / 3.0).abs() < 1e-9);
        assert!((pressure_resilience(0.0, 100.0) - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn fora_de_briga_sem_pressao() {
        // 30 pts, líder 90, falta 1 corrida (25 max) → não alcança.
        let ctx = title_context(30.0, &[90.0, 60.0, 30.0], 1, 25.0);
        assert!(!ctx.in_contention);
        assert_eq!(pressure_intensity(&ctx, 1), 0.0);
    }

    #[test]
    fn titulo_decidido_sem_pressao() {
        // Líder 100, 2º 70, falta 1 (25) → 70+25 < 100 → decidido.
        let ctx = title_context(100.0, &[100.0, 70.0, 50.0], 1, 25.0);
        assert!(ctx.title_decided);
        assert_eq!(pressure_intensity(&ctx, 1), 0.0);
    }

    #[test]
    fn ultima_corrida_em_aberto_intensidade_3() {
        // Líder 100, 2º 85, falta 1 (25) → 85+25=110 ≥ 100 → aberto.
        let ctx = title_context(100.0, &[100.0, 85.0], 1, 25.0);
        assert!(ctx.in_contention && !ctx.title_decided);
        assert_eq!(pressure_intensity(&ctx, 1), 3.0);
    }

    #[test]
    fn lider_no_meio_dobra() {
        // Líder solitário, 3 corridas restantes, em aberto.
        let ctx = title_context(100.0, &[100.0, 90.0], 3, 25.0);
        assert!(ctx.is_leader && ctx.in_contention && !ctx.title_decided);
        let solo = title_context(90.0, &[100.0, 90.0], 3, 25.0); // o 2º (não líder)
        let i_leader = pressure_intensity(&ctx, 3);
        let i_chaser = pressure_intensity(&solo, 3);
        assert!(
            (i_leader - 2.0 * i_chaser).abs() < 1e-9,
            "{i_leader} vs {i_chaser}"
        );
    }

    #[test]
    fn clutch_vs_choke() {
        // Mesma intensidade: resiliente vira clutch (+ritmo, −erro), frágil choke.
        let clutch = pressure_effect(3.0, 1.0, false); // resiliência máxima
        let choke = pressure_effect(3.0, 0.1, false); // frágil
        assert!(
            clutch.pace_delta > 0.0 && clutch.error_mult < 1.0,
            "{clutch:?}"
        );
        assert!(
            choke.pace_delta < 0.0 && choke.error_mult > 1.0,
            "{choke:?}"
        );
    }

    #[test]
    fn mental_forte_quase_neutro() {
        // Líder (×2) com mental forte fica perto do neutro (resiliência ~NEUTRAL).
        let res = pressure_resilience(82.0, 50.0); // ~0.71
        let eff = pressure_effect(2.0, res, false);
        assert!(eff.pace_delta.abs() < 1.5, "deveria ser leve: {eff:?}");
        assert!(
            eff.pace_delta > 0.0,
            "mental forte = leve clutch, não choke"
        );
    }

    #[test]
    fn perseguidor_clutch_mais_facil() {
        // Mesma resiliência logo abaixo do neutro do líder: líder dá choke leve,
        // perseguidor (neutro mais baixo) já vira clutch leve.
        let res = 0.50;
        let lider = pressure_effect(3.0, res, false);
        let chaser = pressure_effect(3.0, res, true);
        assert!(
            lider.pace_delta < 0.0,
            "líder em 0.50 = leve choke: {lider:?}"
        );
        assert!(
            chaser.pace_delta > 0.0,
            "perseguidor em 0.50 = leve clutch: {chaser:?}"
        );
        assert!(chaser.pace_delta > lider.pace_delta);
    }

    // ── Testes de casa cheia ─────────────────────────────────────────────────

    #[test]
    fn ma_fase_pesa_mais_que_boa_fase() {
        // Quem vem mal (média 15) tem fator maior que quem vem bem (média 2).
        let mal = form_factor(Some(15.0));
        let bem = form_factor(Some(2.0));
        assert!(mal > 1.0 && bem < 1.0, "mal={mal}, bem={bem}");
        assert!(mal > bem);
    }

    #[test]
    fn sem_historico_forma_neutra() {
        assert!((form_factor(None) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn evento_fraco_nao_gera_pressao() {
        // Rookie no miolo (score ~20) → stakes 0 → sem pressão de casa cheia.
        let stakes = event_stakes_from_score(20.0);
        assert!((stakes - 0.0).abs() < 1e-9);
        assert_eq!(event_pressure_intensity(stakes, Some(8.0)), 0.0);
    }

    #[test]
    fn casa_cheia_e_universal() {
        // Independe de briga de título: basta o evento despertar público.
        let stakes = event_stakes_from_score(90.0);
        assert!(event_pressure_intensity(stakes, Some(8.0)) > 0.0);
    }

    #[test]
    fn palco_e_oportunidade_neutro_mais_baixo() {
        // Resiliência 0.50: no palco (neutro 0.42) já é clutch; no título (neutro 0.55) é choke.
        let res = 0.50;
        let evento = event_pressure_effect(2.0, res);
        let titulo = pressure_effect(2.0, res, false);
        assert!(
            evento.pace_delta > 0.0,
            "casa cheia em 0.50 = clutch: {evento:?}"
        );
        assert!(
            titulo.pace_delta < 0.0,
            "título em 0.50 = choke: {titulo:?}"
        );
    }

    #[test]
    fn ma_fase_amplifica_os_dois_lados() {
        let stakes = event_stakes_from_score(90.0);
        // Durão (resiliência alta): má fase → clutch MAIOR que boa fase.
        let durao_mal = event_pressure_for(stakes, Some(15.0), 90.0, 60.0);
        let durao_bem = event_pressure_for(stakes, Some(2.0), 90.0, 60.0);
        assert!(durao_mal.pace_delta > durao_bem.pace_delta && durao_bem.pace_delta > 0.0);
        // Frágil: má fase → choke MAIS fundo que boa fase.
        let fragil_mal = event_pressure_for(stakes, Some(15.0), 20.0, 15.0);
        let fragil_bem = event_pressure_for(stakes, Some(2.0), 20.0, 15.0);
        assert!(fragil_mal.pace_delta < fragil_bem.pace_delta && fragil_bem.pace_delta < 0.0);
    }

    #[test]
    fn magnitude_comparavel_ao_titulo() {
        // Swing máximo da casa cheia (evento máximo, má fase, durão) é da ordem do
        // swing máximo do título (última corrida, resiliência máxima).
        let stakes = event_stakes_from_score(120.0);
        let evento_max = event_pressure_for(stakes, Some(20.0), 100.0, 100.0);
        let titulo_max = pressure_effect(3.0, 1.0, false);
        let ratio = evento_max.pace_delta / titulo_max.pace_delta;
        assert!(
            ratio > 0.7 && ratio < 1.4,
            "ratio={ratio} (evento={:?})",
            evento_max
        );
    }

    #[test]
    fn combine_soma_ritmo_multiplica_erro() {
        let a = PressureEffect {
            pace_delta: 2.0,
            error_mult: 0.9,
        };
        let b = PressureEffect {
            pace_delta: 1.0,
            error_mult: 0.8,
        };
        let c = combine(a, b);
        assert!((c.pace_delta - 3.0).abs() < 1e-9);
        assert!((c.error_mult - 0.72).abs() < 1e-9);
    }

    // ── Testes de headroom ───────────────────────────────────────────────────

    #[test]
    fn clutch_azarao_rende_mais_que_craque() {
        // Subir: quem tem espaço até o teto (skill baixo) ganha multiplicador maior.
        let azarao = headroom_pace_mult(55.0, true);
        let craque = headroom_pace_mult(92.0, true);
        assert!(azarao > craque, "azarao={azarao}, craque={craque}");
        assert!(craque < 1.0, "craque afina milésimos: {craque}");
    }

    #[test]
    fn choke_craque_custa_mais_que_azarao() {
        // Cair: quem tem espaço até o chão (skill alto) perde multiplicador maior.
        let craque = headroom_pace_mult(92.0, false);
        let azarao = headroom_pace_mult(55.0, false);
        assert!(craque > azarao, "craque={craque}, azarao={azarao}");
    }

    #[test]
    fn headroom_dentro_dos_limites() {
        for skill in [0.0, 40.0, 67.0, 95.0, 120.0] {
            for clutch in [true, false] {
                let m = headroom_pace_mult(skill, clutch);
                assert!(
                    (0.4..=2.5).contains(&m),
                    "skill={skill} clutch={clutch} m={m}"
                );
            }
        }
    }

    #[test]
    fn headroom_assimetrico_por_direcao() {
        // No mesmo piloto, clutch e choke têm multiplicadores complementares.
        let sk = 70.0;
        let c = headroom_pace_mult(sk, true);
        let k = headroom_pace_mult(sk, false);
        // soma dos "room" = 1 → soma dos mults = 2*MIN + (MAX-MIN) = 0.8 + 2.1 = 2.9
        assert!(((c + k) - 2.9).abs() < 1e-9, "c={c} k={k}");
    }

    // ── Testes de Pressão de Duelo (Nemesis) ─────────────────────────────────

    #[test]
    fn duel_gate_quente_perto_frio_longe() {
        // Pace igual → duelo quente (gate 1). Muito distante → gate 0. No meio, decai.
        assert!((duel_gate(80.0, 80.0) - 1.0).abs() < 1e-9);
        assert!((duel_gate(80.0, 84.0) - 1.0).abs() < 1e-9); // dentro da band
        assert!((duel_gate(80.0, 60.0)).abs() < 1e-9); // além do fade
        let meio = duel_gate(80.0, 90.0); // Δ=10, entre band(6) e fade(14)
        assert!(meio > 0.0 && meio < 1.0, "gate meio={meio}");
    }

    #[test]
    fn rivalry_intensity_zera_sem_calor_ou_frio() {
        // Percebida 0 → sem intensidade. Gate 0 (rival distante) → sem intensidade.
        assert!(rivalry_intensity(0.0, 1.0, 1.0).abs() < 1e-9);
        assert!(rivalry_intensity(90.0, 0.0, 1.0).abs() < 1e-9);
        // Percebida alta + gate cheio → intensidade ~= perceived/100 * PESO.
        let i = rivalry_intensity(100.0, 1.0, 1.0);
        assert!((i - RIVALRY_PESO).abs() < 1e-9, "i={i}");
    }

    #[test]
    fn duelo_neutro_alto_maioria_afoba() {
        // Resiliência 0.55 (mediana): no título (neutro 0.55) é neutro/leve; contra o
        // rival (neutro 0.62) já é CHOKE — a emoção contra o inimigo puxa pro erro.
        let dueleff = rivalry_pressure_effect(1.0, 0.55);
        assert!(
            dueleff.pace_delta < 0.0 && dueleff.error_mult > 1.0,
            "{dueleff:?}"
        );
        // Só o genuinamente frio (0.85) converte em clutch.
        let frio = rivalry_pressure_effect(1.0, 0.85);
        assert!(frio.pace_delta > 0.0 && frio.error_mult < 1.0, "{frio:?}");
    }

    #[test]
    fn duelo_herda_resiliencia_via_atributos() {
        // Mesma percebida + mesmo pace, o duelo herda mentalidade/experiência: o de
        // cabeça fria (mental alto) sofre menos/vira clutch; o frágil afoba.
        let quente = 90.0;
        let frio = rivalry_pressure_for(quente, 80.0, 80.0, 95.0, 80.0, 1.0);
        let fragil = rivalry_pressure_for(quente, 80.0, 80.0, 20.0, 20.0, 1.0);
        assert!(
            frio.pace_delta > fragil.pace_delta,
            "frio={frio:?} fragil={fragil:?}"
        );
        assert!(
            fragil.pace_delta < 0.0,
            "frágil contra o rival afoba: {fragil:?}"
        );
    }

    #[test]
    fn duelo_frio_por_distancia_de_pace_sem_efeito() {
        // Rival 20 pontos de pace mais lento → gate 0 → sem pressão de duelo, por mais
        // intensa que seja a rivalidade (o arco vive, o duelo não).
        let eff = rivalry_pressure_for(100.0, 85.0, 60.0, 50.0, 50.0, 1.0);
        assert_eq!(eff.pace_delta, 0.0);
        assert_eq!(eff.error_mult, 1.0);
    }

    // ── Testes de motivação → ritmo ──────────────────────────────────────────

    #[test]
    fn motivacao_referencia_sem_efeito() {
        assert!(motivation_pace_delta(MOTIVATION_REF).abs() < 1e-9);
    }

    #[test]
    fn motivacao_desmotivado_corre_abaixo() {
        // Desmotivado (0) perde pace; máximo (100) ganha um pouco. Déficit >> superávit.
        let fundo = motivation_pace_delta(0.0);
        let topo = motivation_pace_delta(100.0);
        assert!(
            (fundo + MOTIVATION_DEFICIT_SPAN).abs() < 1e-9,
            "fundo={fundo}"
        );
        assert!((topo - MOTIVATION_SURPLUS_SPAN).abs() < 1e-9, "topo={topo}");
        assert!(
            fundo.abs() > topo.abs(),
            "déficit deve pesar mais que superávit"
        );
    }

    #[test]
    fn motivacao_monotonica() {
        // Mais motivação nunca piora o pace.
        let mut prev = motivation_pace_delta(0.0);
        for m in [10.0, 30.0, 50.0, 70.0, 85.0, 100.0] {
            let cur = motivation_pace_delta(m);
            assert!(cur >= prev - 1e-9, "m={m} cur={cur} prev={prev}");
            prev = cur;
        }
    }
}
