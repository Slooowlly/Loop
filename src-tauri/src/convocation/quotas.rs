/// Distribuição de assentos por fonte de convocação.
#[derive(Debug, Clone)]
pub struct Cotas {
    pub merito_regular: usize,
    pub continuidade: usize,
    pub pool_global: usize,
    pub wildcard: usize,
}

/// Calcula cotas para um total de assentos.
///
/// Distribuição:
/// - D (Wildcard): sempre 1
/// - B (Continuidade): 20% do total, mínimo 1
/// - C (Pool): 20% do total, mínimo 1
/// - A (Mérito): restante
///
/// Exemplos:
/// - 10 assentos → A=5, B=2, C=2, D=1
/// - 12 assentos → A=7, B=2, C=2, D=1
pub fn calcular_cotas(total_assentos: usize) -> Cotas {
    if total_assentos == 0 {
        return Cotas {
            merito_regular: 0,
            continuidade: 0,
            pool_global: 0,
            wildcard: 0,
        };
    }

    let wildcard = 1_usize.min(total_assentos);
    let remaining_after_wildcard = total_assentos.saturating_sub(wildcard);

    let continuidade = if remaining_after_wildcard == 0 {
        0
    } else {
        ((total_assentos as f64 * 0.20) as usize)
            .max(1)
            .min(remaining_after_wildcard)
    };

    let remaining_after_continuidade = remaining_after_wildcard.saturating_sub(continuidade);
    let pool = if remaining_after_continuidade == 0 {
        0
    } else {
        ((total_assentos as f64 * 0.20) as usize)
            .max(1)
            .min(remaining_after_continuidade)
    };

    let merito = total_assentos.saturating_sub(wildcard + continuidade + pool);
    Cotas {
        merito_regular: merito,
        continuidade,
        pool_global: pool,
        wildcard,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calcular_cotas_10_assentos() {
        let cotas = calcular_cotas(10);
        assert_eq!(cotas.merito_regular, 5, "A deve ser 5");
        assert_eq!(cotas.continuidade, 2, "B deve ser 2");
        assert_eq!(cotas.pool_global, 2, "C deve ser 2");
        assert_eq!(cotas.wildcard, 1, "D deve ser 1");
        assert_eq!(
            cotas.merito_regular + cotas.continuidade + cotas.pool_global + cotas.wildcard,
            10
        );
    }

    #[test]
    fn test_calcular_cotas_12_assentos() {
        let cotas = calcular_cotas(12);
        assert_eq!(cotas.merito_regular, 7, "A deve ser 7");
        assert_eq!(cotas.continuidade, 2, "B deve ser 2");
        assert_eq!(cotas.pool_global, 2, "C deve ser 2");
        assert_eq!(cotas.wildcard, 1, "D deve ser 1");
        assert_eq!(
            cotas.merito_regular + cotas.continuidade + cotas.pool_global + cotas.wildcard,
            12
        );
    }

    #[test]
    fn test_calcular_cotas_total_correct() {
        for total in [6, 8, 10, 12, 14, 20] {
            let cotas = calcular_cotas(total);
            let soma =
                cotas.merito_regular + cotas.continuidade + cotas.pool_global + cotas.wildcard;
            assert_eq!(
                soma, total,
                "total errado para {} assentos: {}",
                total, soma
            );
        }
    }

    #[test]
    fn test_calcular_cotas_wildcard_always_one() {
        for total in [4, 6, 8, 10, 12, 20] {
            let cotas = calcular_cotas(total);
            assert_eq!(cotas.wildcard, 1, "wildcard deve ser sempre 1");
        }
    }

    #[test]
    fn test_calcular_cotas_minimum_one_per_source() {
        // Para totais pequenos, B e C devem ter pelo menos 1
        let cotas = calcular_cotas(4);
        assert!(cotas.continuidade >= 1);
        assert!(cotas.pool_global >= 1);
        assert_eq!(cotas.wildcard, 1);
    }

    #[test]
    fn test_calcular_cotas_zero_assentos() {
        let cotas = calcular_cotas(0);
        assert_eq!(cotas.merito_regular, 0);
        assert_eq!(cotas.continuidade, 0);
        assert_eq!(cotas.pool_global, 0);
        assert_eq!(cotas.wildcard, 0);
    }

    #[test]
    fn test_calcular_cotas_um_assento() {
        let cotas = calcular_cotas(1);
        assert_eq!(cotas.merito_regular, 0);
        assert_eq!(cotas.continuidade, 0);
        assert_eq!(cotas.pool_global, 0);
        assert_eq!(cotas.wildcard, 1);
        assert_eq!(
            cotas.merito_regular + cotas.continuidade + cotas.pool_global + cotas.wildcard,
            1
        );
    }

    #[test]
    fn test_calcular_cotas_dois_assentos() {
        let cotas = calcular_cotas(2);
        assert_eq!(cotas.merito_regular, 0);
        assert_eq!(cotas.continuidade, 1);
        assert_eq!(cotas.pool_global, 0);
        assert_eq!(cotas.wildcard, 1);
        assert_eq!(
            cotas.merito_regular + cotas.continuidade + cotas.pool_global + cotas.wildcard,
            2
        );
    }

    #[test]
    fn test_calcular_cotas_tres_assentos() {
        let cotas = calcular_cotas(3);
        assert_eq!(cotas.merito_regular, 0);
        assert_eq!(cotas.continuidade, 1);
        assert_eq!(cotas.pool_global, 1);
        assert_eq!(cotas.wildcard, 1);
        assert_eq!(
            cotas.merito_regular + cotas.continuidade + cotas.pool_global + cotas.wildcard,
            3
        );
    }
}
