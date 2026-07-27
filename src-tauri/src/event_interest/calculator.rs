use crate::models::enums::{SeasonPhase, ThematicSlot};

use super::models::{
    EventInterestContext, EventInterestSummary, EventRepercussionSummary, ExpectedEventInterest,
    HeadlineStrength, InterestTier, RealizedEventInterest,
};

// ── Cálculo principal ─────────────────────────────────────────────────────────

pub fn calculate_expected_event_interest(ctx: &EventInterestContext) -> ExpectedEventInterest {
    let score = base_score_for_category(&ctx.categoria)
        + phase_bonus(ctx.season_phase)
        + round_importance_bonus(ctx.rodada, ctx.total_rodadas)
        + thematic_slot_bonus(ctx.thematic_slot)
        + competitive_context_bonus(ctx)
        + player_prominence_bonus(ctx);

    let display_value = (score * 450.0).round() as i32;
    let tier = score_to_tier(score);
    let pressure_modifier = 1.0 + (score / 100.0) * 0.20;
    let media_multiplier = 1.0 + (score / 100.0) * 0.35;
    let motivation_multiplier = 1.0 + (score / 100.0) * 0.25;

    ExpectedEventInterest {
        score,
        display_value,
        tier,
        pressure_modifier,
        media_multiplier,
        motivation_multiplier,
    }
}

// ── Utilitários públicos ──────────────────────────────────────────────────────

pub fn to_summary(result: &ExpectedEventInterest) -> EventInterestSummary {
    EventInterestSummary {
        display_value: result.display_value,
        tier: result.tier.clone(),
        tier_label: tier_label(&result.tier),
    }
}

/// Resumo público da repercussão: o confronto esperado × realizado que a tela de
/// resultado mostra. Só o que a UI usa — ver a nota em `EventRepercussionSummary`.
pub fn to_repercussion_summary(realized: &RealizedEventInterest) -> EventRepercussionSummary {
    EventRepercussionSummary {
        expected_display_value: realized.expected_display_value,
        expected_tier: realized.expected_tier.clone(),
        expected_tier_label: tier_label(&realized.expected_tier),
        final_display_value: realized.final_display_value,
        final_tier: realized.final_tier.clone(),
        final_tier_label: tier_label(&realized.final_tier),
        delta_display_value: realized.final_display_value - realized.expected_display_value,
        headline_strength: realized.headline_strength.clone(),
        headline_strength_label: headline_strength_label(&realized.headline_strength),
    }
}

pub fn tier_label(tier: &InterestTier) -> String {
    let key = match tier {
        InterestTier::Baixo => "event_interest.tier.baixo",
        InterestTier::Moderado => "event_interest.tier.moderado",
        InterestTier::Alto => "event_interest.tier.alto",
        InterestTier::MuitoAlto => "event_interest.tier.muito_alto",
        InterestTier::EventoPrincipal => "event_interest.tier.evento_principal",
    };
    rust_i18n::t!(key).to_string()
}

pub fn headline_strength_label(strength: &HeadlineStrength) -> String {
    let key = match strength {
        HeadlineStrength::Normal => "event_interest.headline.normal",
        HeadlineStrength::Forte => "event_interest.headline.forte",
        HeadlineStrength::Principal => "event_interest.headline.principal",
    };
    rust_i18n::t!(key).to_string()
}

// ── Blocos internos do score ──────────────────────────────────────────────────

fn base_score_for_category(categoria: &str) -> f32 {
    match categoria {
        "mazda_rookie" | "toyota_rookie" => 18.0,
        "mazda_amador" | "toyota_amador" => 28.0,
        "bmw_m2" => 40.0,
        "gt4" => 52.0,
        "production_challenger" => 62.0,
        "gt3" => 68.0,
        "endurance" => 82.0,
        _ => 30.0,
    }
}

/// Bônus aditivo pelo papel narrativo da corrida.
/// Opera em paralelo com round_importance_bonus (não o substitui).
/// NaoClassificado e slots regulares recebem 0 — compatibilidade com saves legados.
fn thematic_slot_bonus(slot: ThematicSlot) -> f32 {
    match slot {
        ThematicSlot::AberturaDaTemporada => 4.0,
        ThematicSlot::FinalDaTemporada => 6.0,
        ThematicSlot::TensaoPreFinal => 4.0,
        ThematicSlot::MidpointPrestigio => 3.0,
        ThematicSlot::VisitanteRegional => 2.0,
        ThematicSlot::AberturaEspecial => 3.0,
        ThematicSlot::FinalEspecial => 7.0,
        ThematicSlot::RodadaRegular
        | ThematicSlot::RodadaEspecial
        | ThematicSlot::NaoClassificado => 0.0,
    }
}

fn phase_bonus(phase: SeasonPhase) -> f32 {
    match phase {
        SeasonPhase::BlocoEspecial => 10.0,
        _ => 0.0,
    }
}

fn round_importance_bonus(rodada: i32, total_rodadas: i32) -> f32 {
    if total_rodadas <= 0 {
        return 0.0;
    }
    if rodada == 1 {
        return 6.0;
    }
    if rodada == total_rodadas {
        return 12.0;
    }
    if rodada == total_rodadas - 1 {
        return 8.0;
    }
    let progress = rodada as f32 / total_rodadas as f32;
    if progress > 0.5 {
        2.0
    } else {
        0.0
    }
}

fn competitive_context_bonus(ctx: &EventInterestContext) -> f32 {
    let mut bonus = 0.0_f32;
    if ctx.is_title_decider_candidate {
        bonus += 10.0;
    }
    if let Some(gap) = ctx.championship_gap_to_leader {
        if gap <= 10 {
            bonus += 6.0;
        } else if gap <= 20 {
            bonus += 3.0;
        }
    }
    bonus
}

fn player_prominence_bonus(ctx: &EventInterestContext) -> f32 {
    if !ctx.is_player_event {
        return 0.0;
    }
    let mut bonus = 0.0_f32;
    if let Some(pos) = ctx.player_championship_position {
        bonus += match pos {
            1..=3 => 8.0,
            4..=5 => 5.0,
            6..=10 => 2.0,
            _ => 0.0,
        };
    }
    if let Some(media) = ctx.player_media {
        if media >= 80.0 {
            bonus += 5.0;
        } else if media >= 65.0 {
            bonus += 3.0;
        }
    }
    bonus
}

fn score_to_tier(score: f32) -> InterestTier {
    if score >= 85.0 {
        InterestTier::EventoPrincipal
    } else if score >= 65.0 {
        InterestTier::MuitoAlto
    } else if score >= 45.0 {
        InterestTier::Alto
    } else if score >= 25.0 {
        InterestTier::Moderado
    } else {
        InterestTier::Baixo
    }
}

// ── Cálculo de repercussão pós-corrida ────────────────────────────────────────

pub fn calculate_realized_event_interest(
    expected: &ExpectedEventInterest,
    ctx: &EventInterestContext,
    finish_position: Option<i32>,
    grid_position: Option<i32>,
    player_won: bool,
    player_podium: bool,
    player_dnf: bool,
    is_final_round_decider: bool,
) -> RealizedEventInterest {
    // Marcos grossos até o top-5; do P6 para trás, uma RAMPA contínua de meio ponto por
    // posição, com piso. A rampa existe porque as faixas antigas davam 0.0 para todo
    // P6–P10: somado ao termo posicional (que zerava de -4 a +1), o pelotão do meio
    // caía em `final_score == expected.score` e a repercussão realizada empatava,
    // corrida após corrida, com a esperada. P5=3.0 → P6=2.5 emenda sem degrau.
    let result_bonus = if player_won {
        10.0
    } else if player_podium {
        6.0
    } else if finish_position.is_some_and(|p| p <= 5) {
        3.0
    } else if player_dnf {
        -8.0
    } else if let Some(p) = finish_position {
        (2.5 - (p - 6) as f32 * 0.5).max(-5.0)
    } else {
        0.0
    };

    let positions_gained = match (finish_position, grid_position) {
        (Some(f), Some(g)) => g - f,
        _ => 0,
    };
    // Termo CONTÍNUO: qualquer posição ganha ou perdida move a repercussão. Os limites
    // repetem o teto/piso das faixas antigas (remontada de 10+ não vira evento por si),
    // mas o JOELHO da curva desceu de propósito: as faixas davam 4,0 já a partir de +5 e
    // 2,0 a partir de +2, valores que agora só chegam em +10 e +5. Trocar o degrau por
    // rampa e manter o antigo salto de +5 seria dar 0,8/posição no miolo — repercussão
    // demais para uma corrida em que só o tráfego se desfez. Calibração cravada em
    // `contribuicao_posicional_e_de_04_por_posicao`.
    let positional_bonus = (positions_gained as f32 * 0.4).clamp(-3.0, 4.0);

    let big_event_bonus = if (expected.tier == InterestTier::MuitoAlto
        || expected.tier == InterestTier::EventoPrincipal)
        && (player_won || player_podium)
    {
        5.0
    } else {
        0.0
    };

    let title_bonus = if is_final_round_decider {
        8.0
    } else if ctx.is_title_decider_candidate {
        5.0
    } else {
        0.0
    };

    let final_score =
        (expected.score + result_bonus + positional_bonus + big_event_bonus + title_bonus)
            .clamp(0.0, 120.0);

    let media_delta_modifier = (0.75 + (final_score / 100.0) * 0.85).clamp(0.75, 1.60);
    let motivation_delta_modifier = (0.85 + (final_score / 100.0) * 0.65).clamp(0.85, 1.50);

    let news_importance_bias = if final_score >= 85.0 {
        2
    } else if final_score >= 55.0 {
        1
    } else {
        0
    };

    let headline_strength = match news_importance_bias {
        2 => HeadlineStrength::Principal,
        1 => HeadlineStrength::Forte,
        _ => HeadlineStrength::Normal,
    };

    RealizedEventInterest {
        expected_display_value: expected.display_value,
        expected_tier: expected.tier.clone(),
        final_score,
        final_display_value: (final_score * 450.0).round() as i32,
        final_tier: score_to_tier(final_score),
        delta_vs_expected: final_score - expected.score,
        media_delta_modifier,
        motivation_delta_modifier,
        news_importance_bias,
        headline_strength,
    }
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_interest::models::EventInterestContext;
    use crate::models::enums::ThematicSlot;

    fn base_ctx(categoria: &str) -> EventInterestContext {
        EventInterestContext {
            categoria: categoria.to_string(),
            season_phase: SeasonPhase::BlocoRegular,
            rodada: 5,
            total_rodadas: 14,
            week_of_year: 20,
            track_id: 1,
            track_name: "Spa-Francorchamps".to_string(),
            is_player_event: false,
            player_championship_position: None,
            player_media: None,
            championship_gap_to_leader: None,
            is_title_decider_candidate: false,
            thematic_slot: ThematicSlot::NaoClassificado,
        }
    }

    // ── Testes de categoria ───────────────────────────────────────────────────

    #[test]
    fn endurance_maior_que_gt3() {
        let endurance = calculate_expected_event_interest(&base_ctx("endurance"));
        let gt3 = calculate_expected_event_interest(&base_ctx("gt3"));
        assert!(endurance.score > gt3.score);
    }

    #[test]
    fn gt3_maior_que_gt4() {
        let gt3 = calculate_expected_event_interest(&base_ctx("gt3"));
        let gt4 = calculate_expected_event_interest(&base_ctx("gt4"));
        assert!(gt3.score > gt4.score);
    }

    #[test]
    fn gt4_maior_que_bmw_m2() {
        let gt4 = calculate_expected_event_interest(&base_ctx("gt4"));
        let bmw = calculate_expected_event_interest(&base_ctx("bmw_m2"));
        assert!(gt4.score > bmw.score);
    }

    // ── Testes de fase ────────────────────────────────────────────────────────

    #[test]
    fn bloco_especial_maior_que_bloco_regular() {
        let mut ctx_regular = base_ctx("gt3");
        let mut ctx_especial = base_ctx("gt3");
        ctx_regular.season_phase = SeasonPhase::BlocoRegular;
        ctx_especial.season_phase = SeasonPhase::BlocoEspecial;
        let regular = calculate_expected_event_interest(&ctx_regular);
        let especial = calculate_expected_event_interest(&ctx_especial);
        assert!(especial.score > regular.score);
    }

    // ── Testes de rodada ──────────────────────────────────────────────────────

    #[test]
    fn ultima_rodada_maior_que_intermediaria() {
        let mut ctx_final = base_ctx("gt3");
        let mut ctx_meio = base_ctx("gt3");
        ctx_final.rodada = 14;
        ctx_meio.rodada = 7;
        let final_result = calculate_expected_event_interest(&ctx_final);
        let meio_result = calculate_expected_event_interest(&ctx_meio);
        assert!(final_result.score > meio_result.score);
    }

    #[test]
    fn abertura_maior_que_intermediaria() {
        let mut ctx_abertura = base_ctx("gt3");
        let mut ctx_meio = base_ctx("gt3");
        ctx_abertura.rodada = 1;
        ctx_meio.rodada = 7;
        let abertura = calculate_expected_event_interest(&ctx_abertura);
        let meio = calculate_expected_event_interest(&ctx_meio);
        assert!(abertura.score > meio.score);
    }

    // ── Testes de campeonato ──────────────────────────────────────────────────

    #[test]
    fn title_decider_aumenta_score() {
        let mut ctx_normal = base_ctx("gt3");
        let mut ctx_decisivo = base_ctx("gt3");
        ctx_normal.is_title_decider_candidate = false;
        ctx_decisivo.is_title_decider_candidate = true;
        let normal = calculate_expected_event_interest(&ctx_normal);
        let decisivo = calculate_expected_event_interest(&ctx_decisivo);
        assert!(decisivo.score > normal.score);
    }

    #[test]
    fn gap_pequeno_aumenta_score() {
        let mut ctx_longe = base_ctx("gt3");
        let mut ctx_perto = base_ctx("gt3");
        ctx_longe.championship_gap_to_leader = Some(50);
        ctx_perto.championship_gap_to_leader = Some(8);
        let longe = calculate_expected_event_interest(&ctx_longe);
        let perto = calculate_expected_event_interest(&ctx_perto);
        assert!(perto.score > longe.score);
    }

    // ── Testes de protagonismo do jogador ─────────────────────────────────────

    #[test]
    fn jogador_top3_com_midia_alta_maior_que_sem_destaque() {
        let mut ctx_destaque = base_ctx("gt3");
        ctx_destaque.is_player_event = true;
        ctx_destaque.player_championship_position = Some(2);
        ctx_destaque.player_media = Some(85.0);

        let mut ctx_sem = base_ctx("gt3");
        ctx_sem.is_player_event = true;
        ctx_sem.player_championship_position = Some(15);
        ctx_sem.player_media = Some(40.0);

        let destaque = calculate_expected_event_interest(&ctx_destaque);
        let sem = calculate_expected_event_interest(&ctx_sem);
        assert!(destaque.score > sem.score);
    }

    // ── Testes de tier ────────────────────────────────────────────────────────

    #[test]
    fn rookie_miolo_temporada_cai_em_baixo_ou_moderado() {
        let mut ctx = base_ctx("mazda_rookie");
        ctx.rodada = 5;
        ctx.total_rodadas = 14;
        let result = calculate_expected_event_interest(&ctx);
        assert!(
            result.tier == InterestTier::Baixo || result.tier == InterestTier::Moderado,
            "Esperado Baixo ou Moderado, mas foi {:?} (score={})",
            result.tier,
            result.score
        );
    }

    #[test]
    fn endurance_bloco_especial_title_decider_cai_em_evento_principal() {
        let mut ctx = base_ctx("endurance");
        ctx.season_phase = SeasonPhase::BlocoEspecial;
        ctx.is_title_decider_candidate = true;
        ctx.championship_gap_to_leader = Some(5);
        let result = calculate_expected_event_interest(&ctx);
        assert_eq!(
            result.tier,
            InterestTier::EventoPrincipal,
            "Score={}, esperado EventoPrincipal",
            result.score
        );
    }

    #[test]
    fn display_value_cresce_com_score() {
        let rookie = calculate_expected_event_interest(&base_ctx("mazda_rookie"));
        let gt3 = calculate_expected_event_interest(&base_ctx("gt3"));
        let endurance = calculate_expected_event_interest(&base_ctx("endurance"));
        assert!(gt3.display_value > rookie.display_value);
        assert!(endurance.display_value > gt3.display_value);
    }

    // ── Helpers para testes de repercussão ───────────────────────────────────

    fn realized_ctx(categoria: &str) -> (ExpectedEventInterest, EventInterestContext) {
        let ctx = base_ctx(categoria);
        let expected = calculate_expected_event_interest(&ctx);
        (expected, ctx)
    }

    fn realized_with(
        categoria: &str,
        finish: i32,
        grid: i32,
        won: bool,
        podium: bool,
        dnf: bool,
        final_decider: bool,
    ) -> RealizedEventInterest {
        let (expected, ctx) = realized_ctx(categoria);
        calculate_realized_event_interest(
            &expected,
            &ctx,
            Some(finish),
            Some(grid),
            won,
            podium,
            dnf,
            final_decider,
        )
    }

    // ── Testes de repercussão — resultado ────────────────────────────────────

    #[test]
    fn vitoria_aumenta_score_final() {
        let vitoria = realized_with("gt3", 1, 3, true, true, false, false);
        let decimo = realized_with("gt3", 10, 10, false, false, false, false);
        assert!(vitoria.final_score > decimo.final_score);
    }

    #[test]
    fn dnf_reduz_score_final() {
        let normal = realized_with("gt3", 8, 8, false, false, false, false);
        let dnf = realized_with("gt3", 20, 5, false, false, true, false);
        assert!(dnf.final_score < normal.final_score);
    }

    #[test]
    fn podio_maior_que_resultado_medio() {
        let podio = realized_with("gt3", 3, 5, false, true, false, false);
        let medio = realized_with("gt3", 8, 8, false, false, false, false);
        assert!(podio.final_score > medio.final_score);
    }

    // ── Testes de repercussão — contexto ────────────────────────────────────

    #[test]
    fn final_decider_aumenta_repercussao() {
        let mut ctx_decider = base_ctx("gt3");
        ctx_decider.rodada = ctx_decider.total_rodadas;
        ctx_decider.is_title_decider_candidate = true;
        let expected_decider = calculate_expected_event_interest(&ctx_decider);
        let com = calculate_realized_event_interest(
            &expected_decider,
            &ctx_decider,
            Some(1),
            Some(3),
            true,
            true,
            false,
            true,
        );

        let ctx_normal = base_ctx("gt3");
        let expected_normal = calculate_expected_event_interest(&ctx_normal);
        let sem = calculate_realized_event_interest(
            &expected_normal,
            &ctx_normal,
            Some(1),
            Some(3),
            true,
            true,
            false,
            false,
        );
        assert!(com.final_score > sem.final_score);
    }

    #[test]
    fn expected_tier_alto_com_vitoria_gera_impacto_maior() {
        let endurance = realized_with("endurance", 1, 2, true, true, false, false);
        let rookie = realized_with("mazda_rookie", 1, 2, true, true, false, false);
        assert!(endurance.final_score > rookie.final_score);
    }

    // ── Testes de repercussão — derivados ───────────────────────────────────

    #[test]
    fn media_delta_modifier_cresce_com_final_score() {
        let fraco = realized_with("mazda_rookie", 12, 12, false, false, false, false);
        let forte = realized_with("endurance", 1, 1, true, true, false, false);
        assert!(forte.media_delta_modifier > fraco.media_delta_modifier);
    }

    #[test]
    fn motivation_delta_modifier_cresce_com_final_score() {
        let fraco = realized_with("mazda_rookie", 12, 12, false, false, false, false);
        let forte = realized_with("endurance", 1, 1, true, true, false, false);
        assert!(forte.motivation_delta_modifier > fraco.motivation_delta_modifier);
    }

    #[test]
    fn headline_strength_sobe_em_grandes_eventos() {
        let pequeno = realized_with("mazda_rookie", 8, 8, false, false, false, false);
        assert_eq!(pequeno.headline_strength, HeadlineStrength::Normal);
    }

    // ── Testes do DTO público de repercussão ────────────────────────────────

    #[test]
    #[serial_test::serial]
    fn repercussao_expoe_delta_em_valor_de_exibicao() {
        rust_i18n::set_locale("pt-BR"); // os labels resolvem no locale ativo.
        let vitoria = realized_with("gt3", 1, 8, true, true, false, false);
        let resumo = to_repercussion_summary(&vitoria);
        assert_eq!(resumo.expected_display_value, vitoria.expected_display_value);
        assert_eq!(resumo.final_display_value, vitoria.final_display_value);
        assert_eq!(
            resumo.delta_display_value,
            vitoria.final_display_value - vitoria.expected_display_value
        );
        // Vitória vinda de P8: a corrida entregou mais do que prometia.
        assert!(resumo.delta_display_value > 0);
    }

    #[test]
    fn pelotao_do_meio_nao_empata_com_o_esperado() {
        // A regressão que motivou a rampa: P6–P10 sem troca de posição zerava TODOS os
        // termos, e o realizado saía idêntico ao esperado em toda corrida.
        for pos in 6..=10 {
            let r = realized_with("mazda_rookie", pos, pos, false, false, false, false);
            assert!(
                (r.final_score - r.expected_display_value as f32 / 450.0).abs() > 1e-4,
                "P{pos} empatou com o esperado (final={})",
                r.final_score
            );
            assert_ne!(r.final_display_value, r.expected_display_value, "P{pos}");
        }
    }

    #[test]
    fn rampa_do_miolo_e_monotonica_e_emenda_no_top5() {
        // Terminar melhor nunca pode render menos repercussão.
        let scores: Vec<f32> = (5..=14)
            .map(|p| realized_with("gt3", p, p, false, false, false, false).final_score)
            .collect();
        for par in scores.windows(2) {
            assert!(par[0] > par[1], "rampa não é decrescente: {scores:?}");
        }
    }

    /// CONTRATO DE CALIBRAÇÃO do termo posicional — o único teste desta suíte que crava
    /// NÚMERO em vez de relação. Existe porque a troca das faixas antigas por rampa
    /// contínua mexeu no joelho da curva (+5 valia 4,0 e passou a valer 2,0) sem que
    /// nenhuma asserção relacional pudesse notar: monotonicidade e sinal continuam
    /// valendo em qualquer inclinação. Mexer nestes números é decisão de design, não
    /// refactor — se este teste quebrar, atualize junto o comentário da linha do
    /// `positional_bonus` e o registro em `docs/briefings/D09-despacho-r1-r2-r4.md`.
    #[test]
    fn contribuicao_posicional_e_de_04_por_posicao() {
        // Chegada fixa em todos os casos: o result_bonus não se mexe, então a diferença
        // contra o carro que não trocou de posição é o termo posicional puro.
        let base_ganho = realized_with("gt3", 8, 8, false, false, false, false).final_score;
        for (ganho, esperado) in [(1, 0.4), (2, 0.8), (4, 1.6), (5, 2.0), (10, 4.0), (15, 4.0)] {
            let r = realized_with("gt3", 8, 8 + ganho, false, false, false, false);
            let contrib = r.final_score - base_ganho;
            assert!(
                (contrib - esperado).abs() < 1e-4,
                "+{ganho} posições deveria valer {esperado}, valeu {contrib}"
            );
        }

        // Perdas, com chegada lá atrás para o grid nunca ficar negativo.
        let base_perda = realized_with("gt3", 14, 14, false, false, false, false).final_score;
        for (perda, esperado) in [(1, -0.4), (4, -1.6), (5, -2.0), (8, -3.0), (12, -3.0)] {
            let r = realized_with("gt3", 14, 14 - perda, false, false, false, false);
            let contrib = r.final_score - base_perda;
            assert!(
                (contrib - esperado).abs() < 1e-4,
                "-{perda} posições deveria valer {esperado}, valeu {contrib}"
            );
        }
    }

    #[test]
    fn posicoes_ganhas_movem_mesmo_de_uma_em_uma() {
        // O termo posicional era em faixas e zerava de -4 a +1; agora é contínuo.
        let parado = realized_with("gt3", 8, 8, false, false, false, false);
        let uma_a_mais = realized_with("gt3", 8, 9, false, false, false, false);
        let uma_a_menos = realized_with("gt3", 8, 7, false, false, false, false);
        assert!(uma_a_mais.final_score > parado.final_score);
        assert!(uma_a_menos.final_score < parado.final_score);
    }

    #[test]
    #[serial_test::serial]
    fn repercussao_de_dnf_entrega_menos_que_o_esperado() {
        rust_i18n::set_locale("pt-BR");
        let dnf = realized_with("gt3", 20, 4, false, false, true, false);
        let resumo = to_repercussion_summary(&dnf);
        assert!(resumo.delta_display_value < 0);
    }

    #[test]
    #[serial_test::serial]
    fn labels_de_tier_e_manchete_traduzem_pelo_locale() {
        rust_i18n::set_locale("pt-BR");
        assert_eq!(tier_label(&InterestTier::EventoPrincipal), "Evento principal");
        assert_eq!(
            headline_strength_label(&HeadlineStrength::Principal),
            "Manchete principal"
        );

        rust_i18n::set_locale("en-US");
        assert_eq!(tier_label(&InterestTier::EventoPrincipal), "Main event");
        assert_eq!(
            headline_strength_label(&HeadlineStrength::Principal),
            "Lead headline"
        );

        rust_i18n::set_locale("pt-BR"); // devolve o locale-base pros demais testes.
    }

    #[test]
    fn bias_2_em_evento_principal_com_vitoria() {
        let mut ctx = base_ctx("endurance");
        ctx.season_phase = SeasonPhase::BlocoEspecial;
        ctx.is_title_decider_candidate = true;
        let expected = calculate_expected_event_interest(&ctx);
        let realized = calculate_realized_event_interest(
            &expected,
            &ctx,
            Some(1),
            Some(2),
            true,
            true,
            false,
            true,
        );
        assert_eq!(realized.news_importance_bias, 2);
        assert_eq!(realized.headline_strength, HeadlineStrength::Principal);
    }
}
