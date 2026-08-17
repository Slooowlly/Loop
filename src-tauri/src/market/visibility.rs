use crate::models::driver::Driver;
use crate::models::enums::TeamRole;

// ── Visibilidade baseada em performance esportiva (sistema existente) ────────
// `calculate_visibility()` — derivada de posição/vitórias/títulos/papel/idade.
// Dinâmica, event-driven. Usada para filtrar e pontuar candidatos no mercado.

pub fn calculate_visibility(
    driver: &Driver,
    posicao_campeonato: i32,
    total_pilotos: i32,
    category_tier: u8,
    vitorias: i32,
    titulos: i32,
    poles: i32,
    papel: &TeamRole,
    categoria: &str,
) -> f64 {
    let mut vis = 3.0;

    if posicao_campeonato <= 3 {
        vis += 4.0;
    } else if posicao_campeonato <= 5 {
        vis += 3.0;
    } else if posicao_campeonato <= 10 {
        vis += 2.0;
    } else if total_pilotos > 0 && posicao_campeonato <= total_pilotos / 2 {
        vis += 1.0;
    }

    vis += category_tier as f64 * 0.3;

    if driver.idade < 23 {
        vis += 2.0;
    } else if driver.idade <= 28 {
        vis += 1.0;
    } else if driver.idade > 35 {
        vis -= 1.0;
    }

    vis += (vitorias.max(0) as f64 * 0.5).min(1.5);
    vis += (titulos.max(0) as f64 * 2.0).min(4.0);
    vis += (poles.max(0) as f64 * 0.2).min(0.4);

    if *papel == TeamRole::Numero2 {
        vis -= 2.0;
    }

    // Categoria de estreia tem holofote menor — mas o TETO duro de 3.0 que valia até
    // 17/08/2026 ficava abaixo do limiar de shortlist (4.0 em `generate_team_proposals`)
    // e tornava TODO piloto de rookie invisível para o mercado: zero propostas de mérito
    // no tier 0, medido em 80 janelas de Monte Carlo. A ESCALA no lugar do teto preserva
    // a intenção (rookie vale menos que o equivalente das categorias de cima) sem achatar
    // a ordem: o meio do grid continua fora da vitrine, e só o fora da curva — top ~5,
    // jovem, com vitória — passa do limiar e vira alvo de proposta.
    //
    // Desligar `IRACER_ROOKIE_NA_VITRINE` devolve o teto antigo, que é o braço "antes"
    // do A/B — não é uma opção de jogo.
    if categoria == "mazda_rookie" || categoria == "toyota_rookie" {
        if crate::constants::flags_experimentais::booleana("IRACER_ROOKIE_NA_VITRINE") {
            vis *= 0.5;
        } else {
            vis = vis.min(3.0);
        }
    }

    vis.clamp(0.0, 10.0)
}

// ── Perfil de visibilidade pública persistente para mercado ──────────────────
// `derive_market_visibility_profile()` — derivada do campo persistente `midia`
// (longitudinal, pública). Distinta de performance: apelo de mercado ≠ talento.
// Este perfil está ATIVO: alimenta seleção de candidatos, geração de propostas,
// aceitação do piloto e renovação (ver lista de integrações abaixo).

/// Tier de visibilidade pública de mercado derivado do campo persistente `midia`.
///
/// Representa apelo/atenção pública de mercado — NÃO representa qualidade esportiva
/// nem mérito competitivo. Um piloto `Elite` aqui é uma figura pública forte;
/// um piloto `Baixa` tem pouca presença pública. Isso é ortogonal ao talento.
///
/// Thresholds alinhados com o sistema de labels existente em `models/driver.rs`
/// (`get_attribute_tag`):
/// - 25 e 85 correspondem a boundaries canônicos das tags "Discreto" e "Queridinho da Mídia"
/// - 60 é convenção interna que divide a faixa neutra (26–84, sem tag visual)
///   em dois subníveis de mercado, sem contradizer o sistema de labels.
///
/// NOTA: o teto do `Defeito` subiu de 25 para 32 em `driver_tags.rs` (o defeito
/// do rookie nascia em `U{25..=32}` e a tag nunca disparava). O boundary de 25
/// AQUI ficou onde estava de proposito: mover o tier de mercado mexeria em
/// salario e patrocinio, que e outra conversa. As duas escalas se encostam nos
/// extremos e nao precisam coincidir no meio.
#[derive(Debug, Clone, PartialEq)]
pub enum MarketVisibilityTier {
    Baixa,     // 0..=25  — pouca presença pública (Invisível/Discreto)
    Relevante, // 26..=59 — presença pública reconhecível (zona neutra inferior)
    Alta,      // 60..=84 — nome público forte (zona neutra superior + Carismático)
    Elite,     // 85..=100 — figura pública excepcional (Queridinho da Mídia/Estrela)
}

/// Perfil de visibilidade pública de mercado, derivado do campo persistente `midia`.
///
/// Distinto de `calculate_visibility()` (visibilidade baseada em performance esportiva).
/// `midia` é longitudinal e pública — este perfil traduz essa leitura para o
/// vocabulário do mercado sem expor o número bruto diretamente.
///
/// `marketability_bias`: normalização linear de `midia` (raw_media / 100.0).
/// Usado apenas como critério de desempate contínuo DENTRO de um mesmo tier — não
/// entra diretamente em score de proposta nem em salário. Quem carrega o peso das
/// decisões é o `tier`; o bias só refina a ordenação.
///
/// Pontos de integração ativos hoje:
/// - `team_ai::candidate_score()` — bônus de apelo público por tier
///   (`market_public_visibility_bonus`, 0.0/1.5/3.0/5.0)
/// - `team_ai::generate_team_proposals()` — ordenação dos candidatos: mérito esportivo,
///   depois tier, depois `marketability_bias` como desempate fino
/// - `driver_ai::evaluate_proposal()` — `market_visibility_acceptance_adjustment()`
///   ajusta a componente contínua de aceitação; não contorna hard rejections
/// - `renewal::process_renewal()` — `market_visibility_renewal_resistance()` (resistência
///   à continuidade, teto de 8%) e `fame_salary_premium()` (1.00/1.03/1.08/1.15) dentro
///   de `renewal_target_salary()` → `calculate_renewal_salary()`. Ponto sensível: o prêmio
///   é sempre ≥ 1.0 e fica POR CIMA do valor esportivo, nunca o substitui.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketVisibilityProfile {
    pub raw_media: f64,
    pub tier: MarketVisibilityTier,
    pub marketability_bias: f64, // 0.0..=1.0, monotônico, normalização linear preparatória
}

pub fn derive_market_visibility_profile(media: f64) -> MarketVisibilityProfile {
    let raw_media = media.clamp(0.0, 100.0);
    let tier = if raw_media <= 25.0 {
        MarketVisibilityTier::Baixa
    } else if raw_media <= 59.0 {
        MarketVisibilityTier::Relevante
    } else if raw_media <= 84.0 {
        MarketVisibilityTier::Alta
    } else {
        MarketVisibilityTier::Elite
    };
    let marketability_bias = raw_media / 100.0;
    MarketVisibilityProfile {
        raw_media,
        tier,
        marketability_bias,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_champion_high_visibility() {
        let driver = sample_driver(25);

        let visibility =
            calculate_visibility(&driver, 1, 20, 4, 5, 1, 3, &TeamRole::Numero1, "gt3");

        assert!(visibility >= 8.0);
    }

    /// A escala do rookie substitui o teto: o campeão jovem PASSA do limiar de
    /// shortlist (4.0) — antes ninguém do tier 0 passava, nunca —, o meio do grid
    /// continua abaixo dele, e um rookie sempre vale menos que o mesmo currículo
    /// numa categoria de cima.
    #[test]
    fn a_escala_do_rookie_poe_o_fora_da_curva_na_vitrine_e_so_ele() {
        if std::env::var("IRACER_ROOKIE_NA_VITRINE").is_ok() {
            return; // o harness pode estar rodando o braço "antes"
        }
        let campeao = sample_driver(19);
        let vis_campeao = calculate_visibility(
            &campeao,
            1,
            12,
            0,
            5,
            0,
            4,
            &TeamRole::Numero1,
            "mazda_rookie",
        );
        assert!(
            vis_campeao >= 4.0,
            "o campeão jovem do rookie tem que passar do limiar de shortlist: {vis_campeao}"
        );

        let meio_de_grid = sample_driver(19);
        let vis_meio = calculate_visibility(
            &meio_de_grid,
            6,
            12,
            0,
            1,
            0,
            0,
            &TeamRole::Numero1,
            "mazda_rookie",
        );
        assert!(
            vis_meio < 4.0,
            "o meio do grid continua fora da vitrine: {vis_meio}"
        );

        // O mesmo currículo numa categoria de cima vale mais: a escala mantém o
        // rookie como a categoria de menor holofote, que era a intenção do teto.
        let vis_gt3 =
            calculate_visibility(&campeao, 1, 12, 4, 5, 0, 4, &TeamRole::Numero1, "gt3");
        assert!(vis_campeao < vis_gt3);
    }

    #[test]
    fn test_young_driver_visibility_bonus() {
        let young = sample_driver(20);
        let prime = sample_driver(27);

        let young_visibility =
            calculate_visibility(&young, 6, 20, 2, 1, 0, 1, &TeamRole::Numero1, "bmw_m2");
        let prime_visibility =
            calculate_visibility(&prime, 6, 20, 2, 1, 0, 1, &TeamRole::Numero1, "bmw_m2");

        assert!(young_visibility > prime_visibility);
    }

    #[test]
    fn test_n2_visibility_penalty() {
        let driver = sample_driver(25);

        let n1 = calculate_visibility(&driver, 4, 20, 3, 1, 0, 0, &TeamRole::Numero1, "gt4");
        let n2 = calculate_visibility(&driver, 4, 20, 3, 1, 0, 0, &TeamRole::Numero2, "gt4");

        assert!(n1 > n2);
    }

    fn sample_driver(age: u32) -> Driver {
        Driver::new(
            "P001".to_string(),
            "Piloto".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            age,
            2020,
        )
    }

    // ── Testes de derive_market_visibility_profile ────────────────────────────

    #[test]
    fn test_media_zero_is_baixa() {
        let p = derive_market_visibility_profile(0.0);
        assert_eq!(p.tier, MarketVisibilityTier::Baixa);
    }

    #[test]
    fn test_media_25_boundary_is_baixa() {
        let p = derive_market_visibility_profile(25.0);
        assert_eq!(p.tier, MarketVisibilityTier::Baixa);
    }

    #[test]
    fn test_media_26_boundary_is_relevante() {
        let p = derive_market_visibility_profile(26.0);
        assert_eq!(p.tier, MarketVisibilityTier::Relevante);
    }

    #[test]
    fn test_media_59_is_relevante() {
        let p = derive_market_visibility_profile(59.0);
        assert_eq!(p.tier, MarketVisibilityTier::Relevante);
    }

    #[test]
    fn test_media_60_is_alta() {
        let p = derive_market_visibility_profile(60.0);
        assert_eq!(p.tier, MarketVisibilityTier::Alta);
    }

    #[test]
    fn test_media_84_is_alta() {
        let p = derive_market_visibility_profile(84.0);
        assert_eq!(p.tier, MarketVisibilityTier::Alta);
    }

    #[test]
    fn test_media_85_is_elite() {
        let p = derive_market_visibility_profile(85.0);
        assert_eq!(p.tier, MarketVisibilityTier::Elite);
    }

    #[test]
    fn test_media_100_is_elite() {
        let p = derive_market_visibility_profile(100.0);
        assert_eq!(p.tier, MarketVisibilityTier::Elite);
    }

    #[test]
    fn test_marketability_bias_monotonic() {
        let b0 = derive_market_visibility_profile(0.0).marketability_bias;
        let b25 = derive_market_visibility_profile(25.0).marketability_bias;
        let b60 = derive_market_visibility_profile(60.0).marketability_bias;
        let b100 = derive_market_visibility_profile(100.0).marketability_bias;
        assert!(b0 < b25);
        assert!(b25 < b60);
        assert!(b60 < b100);
    }

    #[test]
    fn test_marketability_bias_bounds() {
        assert!((derive_market_visibility_profile(0.0).marketability_bias - 0.0).abs() < 1e-9);
        assert!((derive_market_visibility_profile(100.0).marketability_bias - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_market_visibility_profile_clamps_out_of_range_media() {
        let high = derive_market_visibility_profile(150.0);
        assert_eq!(high.tier, MarketVisibilityTier::Elite);
        assert!((high.raw_media - 100.0).abs() < 1e-9);

        let low = derive_market_visibility_profile(-5.0);
        assert_eq!(low.tier, MarketVisibilityTier::Baixa);
        assert!((low.raw_media - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_raw_media_preserved() {
        let inputs = [0.0_f64, 10.0, 50.0, 84.0, 100.0];
        for &v in &inputs {
            let p = derive_market_visibility_profile(v);
            assert!((p.raw_media - v.clamp(0.0, 100.0)).abs() < 1e-9);
        }
    }
}
