//! Adaptação à categoria → penalidade de skill. **Lógica pura, testável.**
//!
//! Design: subir de categoria custa. Carro mais rápido, campo mais forte, corridas
//! mais longas — o piloto recém-promovido corre ABAIXO do próprio número nas
//! primeiras etapas e vai se ajustando corrida a corrida. É o freio que faltava no
//! canal do PILOTO: o campeão do Rookie sobe junto com a equipe e, sem isso,
//! atropelava o Amador inteiro na temporada seguinte (8 vitórias em 8), levando a
//! equipe a um segundo título e a uma segunda promoção encadeada.
//!
//! Regras (mesmo espírito de [`super::track_knowledge`]):
//! - Só PENALIDADE, nunca bônus, e só na temporada de estreia na categoria — some
//!   depois de [`RACES_TO_ADAPT`] corridas.
//! - Cai a cada corrida (aprende correndo), linearmente.
//! - `adaptabilidade` alta encurta a adaptação; `experiencia` ajuda menos, mas ajuda
//!   (o veterano que sobe sofre menos que o garoto que queimou etapa).
//! - Quem DESCE não é penalizado — a antiguidade nem é zerada na descida
//!   (`Driver::mover_para_categoria`), então a penalidade sequer é avaliada.

/// Penalidade máxima (pontos de skill) — primeira corrida na categoria nova, piloto
/// pouco adaptável e sem experiência.
const MAX_PENALTY: f64 = 12.0;
/// Corridas até a adaptação completa (uma temporada de Rookie).
const RACES_TO_ADAPT: u32 = 5;
/// Quanto a adaptabilidade reduz a penalidade (adapt 100 → ×0.5; adapt 0 → ×1.0).
const ADAPT_REDUCTION: f64 = 0.5;
/// Quanto a experiência reduz a penalidade (exp 100 → ×0.75; exp 0 → ×1.0).
const EXP_REDUCTION: f64 = 0.25;

/// Penalidade de skill (pontos a SUBTRAIR) por ainda estar se adaptando à categoria.
///
/// `corridas_na_categoria` é o contador do piloto — zerado na promoção por
/// `Driver::mover_para_categoria`. Retorna 0.0 assim que ele passa de
/// [`RACES_TO_ADAPT`].
pub fn category_adaptation_penalty(
    corridas_na_categoria: u32,
    adaptabilidade: f64,
    experiencia: f64,
) -> f64 {
    if corridas_na_categoria >= RACES_TO_ADAPT {
        return 0.0;
    }
    let adapt_scale = (1.0 - adaptabilidade.clamp(0.0, 100.0) / 100.0 * ADAPT_REDUCTION).max(0.0);
    let exp_scale = (1.0 - experiencia.clamp(0.0, 100.0) / 100.0 * EXP_REDUCTION).max(0.0);
    let progress = corridas_na_categoria as f64 / RACES_TO_ADAPT as f64;
    MAX_PENALTY * (1.0 - progress) * adapt_scale * exp_scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estreia_leva_penalidade_maxima() {
        // 0 corridas, piloto pouco adaptável e sem experiência → penalidade cheia.
        assert_eq!(category_adaptation_penalty(0, 0.0, 0.0), MAX_PENALTY);
    }

    #[test]
    fn penalidade_cai_a_cada_corrida() {
        let p0 = category_adaptation_penalty(0, 0.0, 0.0);
        let p2 = category_adaptation_penalty(2, 0.0, 0.0);
        let p4 = category_adaptation_penalty(4, 0.0, 0.0);
        assert!(p0 > p2 && p2 > p4 && p4 > 0.0);
        // Decaimento linear: 12 → 7.2 → 2.4.
        assert!((p2 - 7.2).abs() < 1e-9, "{p2}");
        assert!((p4 - 2.4).abs() < 1e-9, "{p4}");
    }

    #[test]
    fn some_depois_de_adaptado() {
        assert_eq!(category_adaptation_penalty(RACES_TO_ADAPT, 0.0, 0.0), 0.0);
        assert_eq!(category_adaptation_penalty(40, 0.0, 0.0), 0.0);
    }

    #[test]
    fn adaptabilidade_e_experiencia_reduzem() {
        let cru = category_adaptation_penalty(0, 0.0, 0.0);
        let adaptavel = category_adaptation_penalty(0, 100.0, 0.0);
        let veterano = category_adaptation_penalty(0, 0.0, 100.0);
        // Adapt 100 → ×0.5 = 6.0; exp 100 → ×0.75 = 9.0.
        assert!((adaptavel - 6.0).abs() < 1e-9, "{adaptavel}");
        assert!((veterano - 9.0).abs() < 1e-9, "{veterano}");
        assert!(adaptavel < cru && veterano < cru);
    }

    #[test]
    fn nunca_vira_bonus() {
        for corridas in 0..RACES_TO_ADAPT + 3 {
            let p = category_adaptation_penalty(corridas, 100.0, 100.0);
            assert!(p >= 0.0, "corridas={corridas} p={p}");
        }
    }
}
