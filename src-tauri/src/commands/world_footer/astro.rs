//! Nota de ASTRO (Fase 3 do Estrelato): o maior nome de público da categoria.

use super::*;

/// Fama mínima (escala de EXIBIÇÃO da ficha) para um piloto ser "astro" digno de nota:
/// tier Estrela+. A régua canônica (`fame::active_interest_team_count`) corta em
/// Nome forte ≤70 / Estrela ≤87 / Ídolo >87; aqui os cortes são o INTEIRO seguinte
/// (71 e 88), então uma fama fracionária na fresta (70,5 ou 87,5) cai um tier abaixo
/// do que a ficha mostra. É de propósito: a nota é conservadora, só sai com o tier
/// cheio. Abaixo disso não há estrela de verdade e a categoria não rende manchete.
const STAR_MIN_FAMA: f64 = 71.0;
const IDOL_MIN_FAMA: f64 = 88.0;

/// Nota de ASTRO (Fase 3 do Estrelato): o maior nome de PÚBLICO da categoria vira
/// manchete de bastidores — a fama arrasta arquibancada e patrocínio. VOZ de revista
/// (3ª pessoa). `None` quando ninguém tem fama de Estrela+ (categoria sem astro não é
/// notícia) ou quando o maior nome já virou nota em outro passo (dedup por `used_drivers`).
pub(super) fn star_of_category_note(
    conn: &rusqlite::Connection,
    categoria: &str,
    used_drivers: &mut HashSet<String>,
) -> Option<WorldNote> {
    use crate::db::queries::drivers;

    let field = drivers::get_drivers_by_category(conn, categoria).ok()?;
    let star = field
        .into_iter()
        .filter(|d| d.status == DriverStatus::Ativo && !used_drivers.contains(&d.id))
        .max_by(|a, b| {
            a.atributos
                .midia
                .partial_cmp(&b.atributos.midia)
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    if star.atributos.midia < STAR_MIN_FAMA {
        return None;
    }

    let text = if star.atributos.midia >= IDOL_MIN_FAMA {
        rust_i18n::t!("world_footer.star.idol", name = star.nome.as_str()).to_string()
    } else {
        rust_i18n::t!("world_footer.star.star", name = star.nome.as_str()).to_string()
    };

    used_drivers.insert(star.id.clone());
    Some(WorldNote {
        id: format!("star:{}", star.id),
        tag: tag_label("backstage"),
        subject: star.nome.clone(),
        kind: "astro_da_categoria".to_string(),
        tone: "neutro".to_string(),
        text,
    })
}
