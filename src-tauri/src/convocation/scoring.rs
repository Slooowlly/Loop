use super::eligibility::FonteConvocacao;
use crate::models::driver::Driver;

// ── Pesos por fonte ───────────────────────────────────────────────────────────
//
// Os pesos viviam soltos no corpo de cada função, então reequilibrar o grid especial
// exigia caçar literais. Não há medição por trás deles: são a divisão de critério
// escolhida no desenho de cada fonte, e cada bloco soma 1,0 (a fonte A soma 0,8 e
// completa com a base fixa).
//
// Fonte A (mérito regular): quem foi bem na temporada regular.
const A_PESO_DESEMPENHO: f64 = 0.45;
const A_PESO_PERFIL: f64 = 0.25;
const A_PESO_DISPONIBILIDADE: f64 = 0.10;
/// Piso da fonte A: convocado por mérito entra com crédito, não do zero.
const A_BASE: f64 = 20.0;

// Fonte B (continuidade histórica): quem já correu o bloco especial antes.
const B_PESO_HISTORICO: f64 = 0.40;
const B_PESO_DESEMPENHO: f64 = 0.25;
const B_PESO_PERFIL: f64 = 0.20;
const B_PESO_DISPONIBILIDADE: f64 = 0.10;
const B_PESO_NARRATIVO: f64 = 0.05;

// Fonte C (pool global): a régua é o piloto, não a temporada dele.
const C_PESO_PERFIL: f64 = 0.50;
const C_PESO_DESEMPENHO: f64 = 0.25;
const C_PESO_DISPONIBILIDADE: f64 = 0.25;

// Fonte D (wildcard): a história pesa mais que a planilha.
const D_PESO_NARRATIVO: f64 = 0.50;
const D_PESO_DESEMPENHO: f64 = 0.30;
const D_PESO_PERFIL: f64 = 0.20;

/// Pontos da temporada que valem o componente cheio de desempenho.
const DESEMPENHO_PONTOS_CHEIOS: f64 = 200.0;

/// Score composto 0–100 para ordenação de candidatos na convocação.
/// Os pesos variam por fonte para refletir critérios diferentes por origem.
pub fn calcular_score(driver: &Driver, fonte: &FonteConvocacao, historico_count: u32) -> f64 {
    match fonte {
        FonteConvocacao::MeritoRegular => score_fonte_a(driver),
        FonteConvocacao::ContinuidadeHistorica => score_fonte_b(driver, historico_count),
        FonteConvocacao::PoolGlobal => score_fonte_c(driver),
        FonteConvocacao::Wildcard => score_fonte_d(driver),
    }
}

// ── Fonte A: MeritoRegular ────────────────────────────────────────────────────
// Desempenho 45% + Perfil 25% + Disponibilidade 10% + base 20%
fn score_fonte_a(driver: &Driver) -> f64 {
    let desempenho = score_desempenho(driver) * A_PESO_DESEMPENHO;
    let perfil = score_perfil(driver) * A_PESO_PERFIL;
    let disponibilidade = score_disponibilidade(driver) * A_PESO_DISPONIBILIDADE;
    (desempenho + perfil + disponibilidade + A_BASE).clamp(0.0, 100.0)
}

// ── Fonte B: ContinuidadeHistorica ────────────────────────────────────────────
// Histórico 40% + Desempenho 25% + Perfil 20% + Disponibilidade 10% + Narrativo 5%
fn score_fonte_b(driver: &Driver, historico_count: u32) -> f64 {
    let historico = score_historico(historico_count) * B_PESO_HISTORICO;
    let desempenho = score_desempenho(driver) * B_PESO_DESEMPENHO;
    let perfil = score_perfil(driver) * B_PESO_PERFIL;
    let disponibilidade = score_disponibilidade(driver) * B_PESO_DISPONIBILIDADE;
    let narrativo = score_narrativo(driver) * B_PESO_NARRATIVO;
    (historico + desempenho + perfil + disponibilidade + narrativo).clamp(0.0, 100.0)
}

// ── Fonte C: PoolGlobal ───────────────────────────────────────────────────────
// Perfil 50% + Desempenho 25% + Disponibilidade 25%
fn score_fonte_c(driver: &Driver) -> f64 {
    let perfil = score_perfil(driver) * C_PESO_PERFIL;
    let desempenho = score_desempenho(driver) * C_PESO_DESEMPENHO;
    let disponibilidade = score_disponibilidade(driver) * C_PESO_DISPONIBILIDADE;
    (perfil + desempenho + disponibilidade).clamp(0.0, 100.0)
}

// ── Fonte D: Wildcard ─────────────────────────────────────────────────────────
// Narrativo 50% + Desempenho 30% + Perfil 20%
fn score_fonte_d(driver: &Driver) -> f64 {
    let narrativo = score_narrativo(driver) * D_PESO_NARRATIVO;
    let desempenho = score_desempenho(driver) * D_PESO_DESEMPENHO;
    let perfil = score_perfil(driver) * D_PESO_PERFIL;
    (narrativo + desempenho + perfil).clamp(0.0, 100.0)
}

// ── Componentes ──────────────────────────────────────────────────────────────

/// Desempenho recente com base em stats da temporada corrente. Retorna 0–95.
fn score_desempenho(driver: &Driver) -> f64 {
    let pontos = (driver.stats_temporada.pontos / DESEMPENHO_PONTOS_CHEIOS).min(1.0) * 60.0;
    let vit = (driver.stats_temporada.vitorias as f64 * 5.0).min(20.0);
    let best = match driver.melhor_resultado_temp {
        Some(1) => 15.0,
        Some(p) if p <= 3 => 8.0,
        _ => 0.0,
    };
    pontos + vit + best
}

/// Histórico em corridas especiais (quantas vezes já participou). Retorna 0–60.
fn score_historico(historico_count: u32) -> f64 {
    (historico_count as f64 * 20.0).min(60.0)
}

/// Perfil técnico: skill + consistência. Retorna 0–100.
fn score_perfil(driver: &Driver) -> f64 {
    driver.atributos.skill * 0.6 + driver.atributos.consistencia * 0.4
}

/// Disponibilidade/fit: motivação alta indica disponibilidade real. Retorna 0–100.
fn score_disponibilidade(driver: &Driver) -> f64 {
    driver.motivacao
}

/// Fator narrativo para wildcards: jovem talentoso ou campeão recente. Retorna 0–80.
fn score_narrativo(driver: &Driver) -> f64 {
    let age_bonus = if driver.idade < 21 {
        (21 - driver.idade) as f64 * 8.0
    } else {
        0.0
    };
    let champion_bonus = if driver.melhor_resultado_temp == Some(1) {
        40.0
    } else {
        0.0
    };
    (age_bonus + champion_bonus).min(80.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::driver::Driver;
    use crate::models::enums::DriverStatus;

    fn make_driver(
        skill: f64,
        consistencia: f64,
        motivacao: f64,
        pontos: f64,
        vitorias: u32,
        idade: u32,
    ) -> Driver {
        let mut d = Driver::new(
            "P999".to_string(),
            "Test Driver".to_string(),
            "BR".to_string(),
            "Masculino".to_string(),
            idade,
            2020,
        );
        d.status = DriverStatus::Ativo;
        d.atributos.skill = skill;
        d.atributos.consistencia = consistencia;
        d.motivacao = motivacao;
        d.stats_temporada.pontos = pontos;
        d.stats_temporada.vitorias = vitorias;
        d
    }

    #[test]
    fn test_score_merito_regular_in_range() {
        let driver = make_driver(80.0, 70.0, 90.0, 150.0, 2, 25);
        let score = calcular_score(&driver, &FonteConvocacao::MeritoRegular, 0);
        assert!(
            score >= 0.0 && score <= 100.0,
            "score fora do range: {}",
            score
        );
    }

    #[test]
    fn test_score_merito_regular_includes_base_component() {
        let driver = make_driver(0.0, 0.0, 0.0, 0.0, 0, 25);
        let score = calcular_score(&driver, &FonteConvocacao::MeritoRegular, 0);
        assert_eq!(score, 20.0, "Fonte A deve manter a base fixa de 20 pontos");
    }

    #[test]
    fn test_score_continuidade_uses_historico() {
        let driver = make_driver(70.0, 70.0, 70.0, 100.0, 1, 30);
        let score_sem = calcular_score(&driver, &FonteConvocacao::ContinuidadeHistorica, 0);
        let score_com = calcular_score(&driver, &FonteConvocacao::ContinuidadeHistorica, 3);
        assert!(score_com > score_sem, "histórico deve aumentar o score");
    }

    #[test]
    fn test_score_pool_global_in_range() {
        let driver = make_driver(50.0, 50.0, 50.0, 50.0, 0, 35);
        let score = calcular_score(&driver, &FonteConvocacao::PoolGlobal, 0);
        assert!(
            score >= 0.0 && score <= 100.0,
            "score fora do range: {}",
            score
        );
    }

    #[test]
    fn test_score_wildcard_benefits_young_driver() {
        let young = make_driver(80.0, 70.0, 90.0, 100.0, 1, 19);
        let old = make_driver(80.0, 70.0, 90.0, 100.0, 1, 30);
        let score_young = calcular_score(&young, &FonteConvocacao::Wildcard, 0);
        let score_old = calcular_score(&old, &FonteConvocacao::Wildcard, 0);
        assert!(
            score_young > score_old,
            "piloto jovem deve ter score wildcard maior"
        );
    }

    #[test]
    fn test_score_all_sources_clamped_to_100() {
        let driver = make_driver(100.0, 100.0, 100.0, 500.0, 10, 18);
        for fonte in &[
            FonteConvocacao::MeritoRegular,
            FonteConvocacao::ContinuidadeHistorica,
            FonteConvocacao::PoolGlobal,
            FonteConvocacao::Wildcard,
        ] {
            let score = calcular_score(&driver, fonte, 10);
            assert!(score <= 100.0, "score {:?} excedeu 100: {}", fonte, score);
        }
    }

    #[test]
    fn test_score_merito_regular_beats_zero_profile_pool_global() {
        let driver = make_driver(0.0, 0.0, 0.0, 0.0, 0, 25);
        let merit_score = calcular_score(&driver, &FonteConvocacao::MeritoRegular, 0);
        let pool_score = calcular_score(&driver, &FonteConvocacao::PoolGlobal, 0);
        assert!(
            merit_score > pool_score,
            "Fonte A deve preservar a vantagem da base sobre o pool em caso neutro"
        );
    }
}
