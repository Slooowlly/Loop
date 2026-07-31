//! Leituras qualitativas: escala tecnica, estrelato (fama + carisma) e a leitura de entrega (esperado vs entregue) com o rank de carro do grid.

use super::*;

pub(super) fn build_driver_technical_read_block(driver: &Driver) -> DriverTechnicalReadBlock {
    let resistencia = driver.atributos.fitness * 0.65 + driver.atributos.gestao_pneus * 0.35;

    DriverTechnicalReadBlock {
        itens: vec![
            build_technical_read_item("velocidade", "Velocidade", driver.atributos.skill),
            build_technical_read_item(
                "consistencia",
                "Consistencia",
                driver.atributos.consistencia,
            ),
            build_technical_read_item("racecraft", "Racecraft", driver.atributos.racecraft),
            build_technical_read_item("resistencia", "Resistencia", resistencia),
        ],
    }
}

/// Monta o bloco de ESTRELATO (fama + carisma) para a ficha do piloto.
/// Fama (`midia`) usa a MESMA classificação de tier do mercado (visibility.rs) pra
/// ficar coerente com o resto do jogo; carisma tem escala descritiva própria; e o
/// `resumo` traduz a DINÂMICA (carisma modula a fama) em uma linha.
pub(super) fn build_driver_stardom_block(driver: &Driver) -> DriverStardomBlock {
    let fama = driver.atributos.midia.clamp(0.0, 100.0);
    let carisma = driver.atributos.carisma.clamp(0.0, 100.0);

    let (nivel_fama, tom_fama) = fama_level_for_value(fama);
    let (nivel_carisma, tom_carisma) = carisma_level_for_value(carisma);
    let resumo = stardom_reading(fama, carisma);

    DriverStardomBlock {
        fama: fama.round() as u8,
        carisma: carisma.round() as u8,
        nivel_fama: nivel_fama.to_string(),
        tom_fama: tom_fama.to_string(),
        nivel_carisma: nivel_carisma.to_string(),
        tom_carisma: tom_carisma.to_string(),
        resumo,
    }
}

/// Escala de FAMA para exibição — 6 níveis, mais rica que os 4 tiers de mercado
/// internos (o display pode ser mais granular que a lógica comercial de
/// salário/patrocínio). Vai de Anônimo a Ídolo; o topo é aspiracional e raro.
pub(super) fn fama_level_for_value(value: f64) -> (String, &'static str) {
    let value = value.clamp(0.0, 100.0);
    let (key, tom) = if value <= 15.0 {
        ("anonimo", "neutral")
    } else if value <= 30.0 {
        ("discreto", "neutral")
    } else if value <= 50.0 {
        ("conhecido", "info")
    } else if value <= 70.0 {
        ("nome_forte", "info")
    } else if value <= 87.0 {
        ("estrela", "success")
    } else {
        ("idolo", "elite")
    };
    let full = format!("driver_read.fama.{key}");
    (rust_i18n::t!(&full).to_string(), tom)
}

pub(super) fn carisma_level_for_value(value: f64) -> (String, &'static str) {
    let value = value.clamp(0.0, 100.0);
    let (key, tom) = if value < 30.0 {
        ("apagado", "danger")
    } else if value < 45.0 {
        ("reservado", "warning")
    } else if value < 60.0 {
        ("cativante", "neutral")
    } else if value < 75.0 {
        ("magnetico", "info")
    } else if value < 88.0 {
        ("carismatico", "success")
    } else {
        ("idolo_natural", "elite")
    };
    let full = format!("driver_read.carisma.{key}");
    (rust_i18n::t!(&full).to_string(), tom)
}

/// Leitura de uma linha: como o carisma (retenção/conversão) conversa com a fama
/// (estoque). Alto carisma + baixa fama = pólvora seca; baixo carisma + alta fama =
/// holofote volátil construído só pelo resultado.
pub(super) fn stardom_reading(fama: f64, carisma: f64) -> String {
    let fama_alta = fama >= 60.0;
    let carisma_alto = carisma >= 60.0;

    let key = match (fama_alta, carisma_alto) {
        (true, true) => "idol_consolidated",
        (false, true) => "powder_keg",
        (true, false) => "volatile_spotlight",
        (false, false) => "off_radar",
    };
    let full = format!("driver_read.stardom.{key}");
    rust_i18n::t!(&full).to_string()
}

pub(super) fn build_technical_read_item(
    chave: &str,
    label: &str,
    value: f64,
) -> DriverTechnicalReadItem {
    let (nivel, tom) = technical_level_for_value(value);

    DriverTechnicalReadItem {
        chave: chave.to_string(),
        label: label.to_string(),
        nivel: nivel.to_string(),
        tom: tom.to_string(),
    }
}

pub(super) fn technical_level_for_value(value: f64) -> (String, &'static str) {
    let value = value.clamp(0.0, 100.0);
    let (key, tom) = if value < 12.5 {
        ("muito_fraco", "danger")
    } else if value < 25.0 {
        ("fraco", "danger")
    } else if value < 37.5 {
        ("abaixo", "warning")
    } else if value < 50.0 {
        ("instavel", "warning")
    } else if value < 62.5 {
        ("competente", "neutral")
    } else if value < 75.0 {
        ("forte", "info")
    } else if value < 87.5 {
        ("muito_forte", "success")
    } else {
        ("elite", "elite")
    };
    let full = format!("driver_read.technical.{key}");
    (rust_i18n::t!(&full).to_string(), tom)
}

pub(super) fn build_performance_read_block(
    conn: &Connection,
    driver: &Driver,
    team: Option<&Team>,
    teammate: Option<&Driver>,
    championship_position: Option<i32>,
) -> DriverPerformanceReadBlock {
    let expected = team.and_then(|value| expected_position_for_team(conn, value));
    let delta = match (expected, championship_position) {
        (Some(expected_position), Some(position)) => Some(expected_position - position),
        _ => None,
    };
    let teammate_points = teammate.map(|value| value.stats_temporada.pontos.round() as i32);
    let reading = match delta {
        Some(value) if value >= 3 => rust_i18n::t!("driver_read.delivery.above"),
        Some(value) if value <= -3 => rust_i18n::t!("driver_read.delivery.below"),
        Some(_) => rust_i18n::t!("driver_read.delivery.within"),
        None => rust_i18n::t!("driver_read.delivery.no_context"),
    };

    DriverPerformanceReadBlock {
        esperado_posicao: expected,
        entregue_posicao: championship_position,
        delta_posicao: delta,
        car_performance: team.map(|value| value.effective_car_performance()),
        companheiro_nome: teammate.map(|value| value.nome.clone()),
        companheiro_pontos: teammate_points,
        piloto_pontos: driver.stats_temporada.pontos.round() as i32,
        leitura: reading.to_string(),
    }
}

/// Dois carros dentro desta distância de `car_performance` estão em EMPATE TÉCNICO — o
/// pacote não separa os dois. Carros de mesmo nível dão magnitude idêntica, então a margem
/// só existe pro caminho legado (equipe sem peças persistidas, escalar contínuo).
pub(super) const CAR_TIE_EPSILON: f64 = 1e-6;

/// Assentos OCUPADOS da equipe (0–2) — o grid REAL, não a capacidade nominal.
pub(super) fn filled_seats(team: &Team) -> i32 {
    i32::from(team.piloto_1_id.is_some()) + i32::from(team.piloto_2_id.is_some())
}

/// Posição ESPERADA pelo pacote (carro), RELATIVA ao grid da categoria.
///
/// Era uma tabela de limiares ABSOLUTOS sobre o escalar de carro, e ela mentia por dois
/// lados: o escalar não tem escala comum entre categorias (as de cima estouram o topo da
/// tabela e o grid inteiro "espera" P2), e num grid SPEC — rookie, todo carro no nível 1 —
/// o escalar é IDÊNTICO pra todo mundo, então a tabela dava posição de fundo pra todas as
/// equipes de uma vez. Aqui a equipe é ranqueada pelo carro EFETIVO ([`Team::effective_car_performance`])
/// contra as rivais do mesmo grid (categoria + classe) e a expectativa é o meio da faixa de
/// assentos do seu rank. Grid spec (todo mundo empatado) → todo mundo espera o meio do grid,
/// que é a leitura honesta quando o carro não separa ninguém e o resultado é só piloto.
pub(super) fn expected_position_for_team(conn: &Connection, team: &Team) -> Option<i32> {
    let rivals = team_queries::get_teams_by_category(conn, &team.categoria).ok()?;
    let grid: Vec<(f64, i32)> = rivals
        .iter()
        .filter(|rival| rival.classe == team.classe)
        .map(|rival| (rival.effective_car_performance(), filled_seats(rival)))
        .collect();

    expected_position_from_grid(team.effective_car_performance(), &grid)
}

/// Núcleo PURO do rank: dado o carro da equipe e o grid `(carro, assentos ocupados)`, cai no
/// MEIO da faixa de assentos do bloco em que ela está. Assentos com carro estritamente melhor
/// ficam à frente; o bloco do empate técnico inclui a própria equipe. `None` quando o bloco
/// está vazio (equipe sem assento ocupado — não há expectativa a dar).
pub(super) fn expected_position_from_grid(mine: f64, grid: &[(f64, i32)]) -> Option<i32> {
    let mut seats_ahead = 0;
    let mut seats_tied = 0;
    for &(perf, seats) in grid {
        let delta = perf - mine;
        if delta > CAR_TIE_EPSILON {
            seats_ahead += seats;
        } else if delta >= -CAR_TIE_EPSILON {
            seats_tied += seats;
        }
    }

    if seats_tied == 0 {
        return None;
    }
    Some(seats_ahead + (seats_tied + 1) / 2)
}
