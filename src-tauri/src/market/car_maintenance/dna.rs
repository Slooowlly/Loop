//! DNA de construção de carro do time — o viés permanente (potência, handling,
//! aceleração ou balanceado) que a média do calendário não lava.

// ===================== Identidade / DNA de carro do time =====================

/// Viés inato de construção de carro do time — **PERSISTENTE** (não muda por temporada).
/// É a fonte de foco que a média do calendário NÃO lava: um time "de potência" sempre puxa
/// o carro pra potência, independente das pistas do calendário. O jogador não vê (o shape
/// continua oculto; isto é identidade de bastidor). Relaciona com a identidade viva do time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarFocus {
    Balanced,
    Power,
    Handling,
    Acceleration,
}

/// Peso do DNA na demanda efetiva (o resto vem do calendário). Alto o bastante para um time
/// focado cruzar o gatilho de especialização mesmo num calendário diverso (que lava pra
/// balanceado). Calibrável.
pub(super) const DNA_DEMAND_WEIGHT: f64 = 0.6;

/// Intensidade do pico do DNA focado (fração do eixo dominante; o resto é dividido igual).
pub(super) const DNA_PEAK: f64 = 0.70;

/// DNA determinístico e **estável** por time (sem temporada → não re-rola; é permanente).
/// Distribuição: 40% balanceado / 20% potência / 20% handling / 20% aceleração — foco é
/// maioria, mas generalistas continuam existindo.
pub fn team_car_focus(team_id: &str) -> CarFocus {
    let mut seed: u32 = 0x85EB_CA6B;
    for byte in team_id.bytes() {
        seed = seed.wrapping_mul(31).wrapping_add(byte as u32);
    }
    // avalanche (descorrelaciona o módulo 100 do input)
    seed ^= seed >> 15;
    seed = seed.wrapping_mul(0x2c1b_3c6d);
    seed ^= seed >> 13;

    match seed % 100 {
        0..=39 => CarFocus::Balanced,
        40..=59 => CarFocus::Power,
        60..=79 => CarFocus::Handling,
        _ => CarFocus::Acceleration,
    }
}

/// Demanda PHA `(P, H, A)` que o DNA sozinho pediria.
pub(super) fn focus_demand(focus: CarFocus) -> (f64, f64, f64) {
    let lo = (1.0 - DNA_PEAK) / 2.0;
    match focus {
        CarFocus::Balanced => (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0),
        CarFocus::Power => (DNA_PEAK, lo, lo),
        CarFocus::Handling => (lo, DNA_PEAK, lo),
        CarFocus::Acceleration => (lo, lo, DNA_PEAK),
    }
}

/// Mistura a demanda do calendário com o DNA (persistente) do time. O DNA domina; o
/// calendário só modula. Para times balanceados, o resultado fica ~equilibrado (não foca).
pub(super) fn blend_with_focus(calendar: (f64, f64, f64), focus: CarFocus) -> (f64, f64, f64) {
    let (cp, ch, ca) = calendar;
    let (fp, fh, fa) = focus_demand(focus);
    let w = DNA_DEMAND_WEIGHT;
    (
        w * fp + (1.0 - w) * cp,
        w * fh + (1.0 - w) * ch,
        w * fa + (1.0 - w) * ca,
    )
}
