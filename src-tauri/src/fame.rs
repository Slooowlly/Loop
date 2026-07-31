//! Dinâmica da FAMA (`midia`) modulada pelo CARISMA. **Lógica pura.**
//!
//! Dois eixos, travados com o user:
//! - **Carisma** (atributo estável da pessoa): magnetismo/qualidade de estrela.
//! - **Fama / midia** (estoque 0–100): atenção pública acumulada, sobe e desce.
//!
//! O carisma governa TRÊS taxas sobre a fama:
//! 1. **Ganho** (bom resultado): carismático converte conquista em fama melhor.
//! 2. **Amortecedor** (mau resultado): carismático quase não perde fama (teflon) —
//!    a estrela pode terminar em último e mal sentir.
//! 3. **Decaimento passivo** (tempo/sumiço): fama de carismático é grudenta; a de
//!    um piloto sem graça esvai rápido.
//!
//! Emerge o que o user pediu: estrela ganha rápido e não cai; apagado ganha só com
//! resultado forte e sangra depressa. Números na mesa de balanceamento (tunáveis aqui).

/// Multiplicador do GANHO de fama (delta ≥ 0) pelo carisma: 0.6× (apagado) a
/// 1.4× (estrela). Carismático capitaliza mais cada conquista.
pub fn fame_gain_mult(carisma: f64) -> f64 {
    0.6 + 0.8 * (carisma.clamp(0.0, 100.0) / 100.0)
}

/// Multiplicador da PERDA de fama (delta < 0) pelo carisma: 1.4× (apagado, sangra)
/// a 0.4× (estrela, quase não sente). É o amortecedor do "terminou em último".
pub fn fame_loss_mult(carisma: f64) -> f64 {
    1.4 - (carisma.clamp(0.0, 100.0) / 100.0)
}

/// Multiplicador do DECAIMENTO passivo da fama (tempo/sumiço) pelo carisma: 1.5×
/// (apagado, esvai rápido) a 0.5× (estrela, grudenta).
pub fn fame_decay_mult(carisma: f64) -> f64 {
    1.5 - (carisma.clamp(0.0, 100.0) / 100.0)
}

/// Aplica o carisma a um delta de fama BRUTO. Ganho e perda usam multiplicadores
/// distintos — o amortecedor só age no lado negativo, o ganho só no positivo.
pub fn apply_carisma_to_fame_delta(raw_delta: f64, carisma: f64) -> f64 {
    if raw_delta >= 0.0 {
        raw_delta * fame_gain_mult(carisma)
    } else {
        raw_delta * fame_loss_mult(carisma)
    }
}

// ── Constantes de balanceamento (tunáveis; mesa de balanceamento) ─────────────
/// Piso da fama no decaimento passivo — todo piloto retém ao menos isto (nome que
/// não some de vez). Alinhado ao limiar "Baixa/Discreto" do mercado.
pub const FAME_DECAY_FLOOR: f64 = 25.0;
/// Fração base da distância até o piso perdida por corrida (antes do carisma).
pub const FAME_DECAY_BASE_RATE: f64 = 0.025;
/// Deriva de carisma por marco de carreira — PEQUENA de propósito (carisma é
/// estável; só uma vida inteira de drama move de verdade).
pub const CARISMA_DRIFT_INCIDENT: f64 = 0.4; // vilão/incidente notável — "drama vende"
pub const CARISMA_DRIFT_COMEBACK: f64 = 0.5; // remontada de muitas posições
pub const CARISMA_DRIFT_BIG_WIN: f64 = 0.6; // vitória num evento grande
/// Posições ganhadas (grid − chegada) a partir das quais conta como remontada.
pub const COMEBACK_MIN_POSITIONS: i32 = 6;

/// Um passo de DECAIMENTO passivo da fama rumo a um piso, escalado pelo carisma
/// (carismático decai mais devagar). `base_rate` = fração base da distância até o
/// piso percorrida por passo (ex.: por corrida). Nunca desce abaixo do piso nem
/// sobe (só decai).
pub fn decay_fame_toward(current: f64, floor: f64, base_rate: f64, carisma: f64) -> f64 {
    if current <= floor {
        return current;
    }
    let rate = (base_rate * fame_decay_mult(carisma)).clamp(0.0, 1.0);
    current - (current - floor) * rate
}

// ── FASE 2a: fama como VALOR COMERCIAL no mercado ─────────────────────────────
//
// A fama vira "segunda moeda" que o time AGE em cima ao contratar: um piloto
// famoso projeta patrocínio (ver Chunk B), então vale dinheiro além do mérito
// esportivo. O peso desse valor escala com a NECESSIDADE do time (pobre pesa
// alto — precisa da grana; dinastia pesa baixo — quer resultado).

/// Valor comercial da fama em "unidades de score" — os MESMOS 6 níveis da ficha
/// (Anônimo→Ídolo), com escalada CONVEXA: o topo pesa muito mais que proporcional
/// (um Ídolo é desproporcionalmente valioso). Some ao skill no desempate da
/// contratação, depois de ponderado pela necessidade do time.
pub fn fame_commercial_units(fama: f64) -> f64 {
    let fama = fama.clamp(0.0, 100.0);
    if fama <= 15.0 {
        0.0
    } else if fama <= 30.0 {
        3.0
    } else if fama <= 50.0 {
        8.0
    } else if fama <= 70.0 {
        16.0
    } else if fama <= 87.0 {
        30.0
    } else {
        55.0
    }
}

/// Fator de NECESSIDADE do time pelo dinheiro da fama, de `strength` financeira.
/// Vai de [`TEAM_NEED_MIN`] (dinastia rica, quase ignora fama) a [`TEAM_NEED_MAX`]
/// (time carente, pesa a fama pesado). `budget_index` e `reputacao` são 0–100.
pub fn team_need_factor(budget_index: f64, reputacao: f64) -> f64 {
    let strength =
        (0.6 * budget_index.clamp(0.0, 100.0) + 0.4 * reputacao.clamp(0.0, 100.0)) / 100.0;
    TEAM_NEED_MAX - strength * (TEAM_NEED_MAX - TEAM_NEED_MIN)
}

/// Piso/teto do fator de necessidade. Dinastia rica: 0.25 (fama quase irrelevante);
/// time carente: 1.15 (fama pode superar a velocidade num candidato elegível).
pub const TEAM_NEED_MIN: f64 = 0.25;
pub const TEAM_NEED_MAX: f64 = 1.15;
/// Prêmio salarial da oferta de um time com interesse ativo (apelo comercial paga
/// mais pra fisgar o nome). +30% sobre a oferta normal.
pub const ACTIVE_INTEREST_SALARY_PREMIUM: f64 = 1.30;

// ── INTERESSE ATIVO visível ao JOGADOR — DECOPLADO da economia da IA ──────────
//
// A economia acima (need_factor) é da IA: no mercado, é o time CARENTE que valoriza
// a fama (patrocínio). Mas mostrar isso ao jogador confunde — "essa equipe me quer"
// lê como "essa equipe é boa", quando na verdade é a mais pobre. Então o destaque
// que o JOGADOR vê (badge + N1 + prêmio + e-mail) é dos POUCOS MELHORES times, e
// quantos deles escala com a fama. Poucos, e os melhores.

/// Quantos dos MELHORES times cortejam o jogador, pela fama. Escala pelos níveis da
/// ficha: até Conhecido→0, Nome forte→1, Estrela→2, Ídolo→3. Poucos de propósito.
pub fn active_interest_team_count(fama: f64) -> usize {
    let fama = fama.clamp(0.0, 100.0);
    if fama <= 50.0 {
        0 // Anônimo / Discreto / Conhecido — ninguém cobiça o nome ainda
    } else if fama <= 70.0 {
        1 // Nome forte — o melhor time da categoria
    } else if fama <= 87.0 {
        2 // Estrela — os 2 melhores
    } else {
        3 // Ídolo — os 3 melhores
    }
}

/// Qualidade de um time para eleger quem cobiça o astro (os MELHORES primeiro):
/// prestígio (reputação) + carro + pedigree histórico de títulos. Escala ~0–100+.
pub fn team_prestige_quality(reputacao: f64, car_performance: f64, historic_titles: i32) -> f64 {
    reputacao.clamp(0.0, 100.0)
        + car_performance.clamp(0.0, 100.0) * 0.6
        + historic_titles.max(0) as f64 * 4.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ganho_estrela_maior_que_apagado() {
        assert!(fame_gain_mult(85.0) > fame_gain_mult(25.0));
        // Estrela converte acima de 1×, apagado abaixo.
        assert!(fame_gain_mult(85.0) > 1.0 && fame_gain_mult(25.0) < 1.0);
    }

    #[test]
    fn estrela_quase_nao_perde_terminando_em_ultimo() {
        // Mesma perda-base −3: estrela amortece pra bem menos que o apagado.
        let perda_estrela = apply_carisma_to_fame_delta(-3.0, 85.0);
        let perda_apagado = apply_carisma_to_fame_delta(-3.0, 25.0);
        assert!(
            perda_estrela > perda_apagado,
            "estrela perde menos (menos negativo)"
        );
        assert!(
            perda_estrela.abs() < 2.0,
            "estrela mal sente: {perda_estrela}"
        );
    }

    #[test]
    fn apagado_ganha_no_top5_mas_menos() {
        // Ganho-base +2 (top 5): apagado ganha, mas menos que 2.
        let ganho = apply_carisma_to_fame_delta(2.0, 25.0);
        assert!(ganho > 0.0 && ganho < 2.0, "ganha menos: {ganho}");
    }

    #[test]
    fn apagado_decai_mais_rapido_que_estrela() {
        let dec_apagado = decay_fame_toward(80.0, 30.0, 0.05, 25.0);
        let dec_estrela = decay_fame_toward(80.0, 30.0, 0.05, 85.0);
        // Ambos caem, mas o apagado cai mais (fica com menos fama).
        assert!(
            dec_apagado < dec_estrela,
            "apagado={dec_apagado}, estrela={dec_estrela}"
        );
        assert!(dec_estrela < 80.0, "estrela ainda decai um pouco");
    }

    #[test]
    fn decaimento_nunca_passa_do_piso() {
        // Já no piso (ou abaixo) não mexe.
        assert_eq!(decay_fame_toward(30.0, 30.0, 0.5, 25.0), 30.0);
        assert_eq!(decay_fame_toward(20.0, 30.0, 0.5, 25.0), 20.0);
        // Decaimento de um passo não ultrapassa o piso.
        let d = decay_fame_toward(31.0, 30.0, 1.0, 0.0); // rate satura em 1.0
        assert!(d >= 30.0, "não passou do piso: {d}");
    }

    #[test]
    fn ganho_e_amortecedor_sao_monotonicos_no_carisma() {
        // Faixas com folga de ponto flutuante (0.6+0.8 = 1.4000…1).
        const EPS: f64 = 1e-9;
        for c in [0.0, 30.0, 60.0, 100.0] {
            assert!((0.6 - EPS..=1.4 + EPS).contains(&fame_gain_mult(c)));
            assert!((0.4 - EPS..=1.4 + EPS).contains(&fame_loss_mult(c)));
            assert!((0.5 - EPS..=1.5 + EPS).contains(&fame_decay_mult(c)));
        }
        // Mais carisma → mais ganho, menos perda, menos decaimento.
        assert!(fame_gain_mult(100.0) > fame_gain_mult(0.0));
        assert!(fame_loss_mult(100.0) < fame_loss_mult(0.0));
        assert!(fame_decay_mult(100.0) < fame_decay_mult(0.0));
    }

    #[test]
    fn valor_comercial_e_convexo_no_topo() {
        // Anônimo não vale nada; sobe monotônico; e o salto pro topo é o MAIOR
        // (convexo — Ídolo desproporcionalmente valioso).
        assert_eq!(fame_commercial_units(10.0), 0.0); // Anônimo
        let discreto = fame_commercial_units(25.0);
        let conhecido = fame_commercial_units(45.0);
        let nome_forte = fame_commercial_units(65.0);
        let estrela = fame_commercial_units(80.0);
        let idolo = fame_commercial_units(95.0);
        assert!(discreto < conhecido && conhecido < nome_forte);
        assert!(nome_forte < estrela && estrela < idolo);
        // Salto Estrela→Ídolo é o maior de todos.
        assert!(idolo - estrela > estrela - nome_forte);
        assert!(idolo - estrela > nome_forte - conhecido);
    }

    #[test]
    fn need_factor_maior_para_time_carente() {
        let carente = team_need_factor(10.0, 15.0);
        let dinastia = team_need_factor(95.0, 90.0);
        assert!(carente > dinastia, "carente={carente}, dinastia={dinastia}");
        // Dentro dos limites travados.
        for (b, r) in [(0.0, 0.0), (50.0, 50.0), (100.0, 100.0)] {
            let n = team_need_factor(b, r);
            assert!((TEAM_NEED_MIN..=TEAM_NEED_MAX).contains(&n), "n={n}");
        }
    }

    #[test]
    fn time_carente_prefere_idolo_mediocre_a_rapido_anonimo() {
        // O caso-alvo do design: num time carente, o apelo de um Ídolo compensa um
        // gap de skill contra um rápido sem fama; numa dinastia, não. (É o mesmo termo
        // que a escada soma ao skill: fame_commercial_units × need_factor.)
        let idolo_appeal_carente = fame_commercial_units(95.0) * team_need_factor(10.0, 15.0);
        let idolo_appeal_dinastia = fame_commercial_units(95.0) * team_need_factor(95.0, 90.0);
        // Carente: 55 × ~1.05 ≈ 58 → cobre um gap de skill (60+58 > 90).
        assert!(60.0 + idolo_appeal_carente > 90.0);
        // Dinastia: 55 × ~0.28 ≈ 15 → não cobre (60+15 < 90).
        assert!(60.0 + idolo_appeal_dinastia < 90.0);
    }

    #[test]
    fn interesse_visivel_e_poucos_e_escala_com_a_fama() {
        // Poucos, e crescendo com a fama: ninguém até Conhecido, depois 1→2→3.
        assert_eq!(active_interest_team_count(40.0), 0); // Conhecido
        assert_eq!(active_interest_team_count(65.0), 1); // Nome forte
        assert_eq!(active_interest_team_count(80.0), 2); // Estrela
        assert_eq!(active_interest_team_count(95.0), 3); // Ídolo
                                                         // Nunca é a grade toda (o problema do screenshot).
        assert!(active_interest_team_count(100.0) <= 3);
    }

    #[test]
    fn qualidade_do_time_ordena_os_melhores_primeiro() {
        // Um time forte (reputação + carro + títulos) pontua mais que um fraco —
        // são esses os POUCOS que cobiçam o astro visivelmente.
        let forte = team_prestige_quality(85.0, 90.0, 7);
        let fraco = team_prestige_quality(30.0, 40.0, 0);
        assert!(forte > fraco);
    }
}
