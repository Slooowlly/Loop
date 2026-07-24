//! Fallback determinístico: a mesma matéria sem IA, a partir dos mesmos tokens.

use super::*;

// ── Fallback determinístico (§9) ─────────────────────────────────────────────────

/// Monta a matéria sem IA, a partir dos MESMOS tokens. Mais curta e mais template que a
/// versão do servidor, mas obedece às mesmas regras: 3ª pessoa, sem número de atributo,
/// sem se dirigir ao jogador.
pub(super) fn deterministic_article(p: &PreviewData) -> SeasonPreview {
    let headline = rust_i18n::t!(
        &format!("season_preview.fb.headline.{}", p.thesis.id()),
        category = p.cat_label.as_str(),
        year = p.year as i64
    )
    .to_string();
    let standfirst = tk(&format!("fb.standfirst.{}", p.thesis.id()));

    let mut body = String::new();

    // P1 — cenário + material.
    let material_line = match &p.material {
        Material::Uniform => tk("fb.material_uniform"),
        Material::Unequal { best, worst } => rust_i18n::t!(
            "season_preview.fb.material_unequal",
            best = best.as_str(),
            worst = worst.as_str()
        )
        .to_string(),
        Material::Unknown => String::new(),
    };
    let _ = write!(
        body,
        "{}",
        rust_i18n::t!(
            "season_preview.fb.p1",
            category = p.cat_label.as_str(),
            year = p.year as i64,
            material = material_line.as_str()
        )
    );

    // P2 — os favoritos, um a um.
    let mut favs = String::new();
    for (i, d) in p.ranked.iter().take(3).enumerate() {
        let traco = if d.tracos.is_empty() {
            String::new()
        } else {
            rust_i18n::t!("season_preview.fb.trait_suffix", style = d.tracos[0].as_str())
                .to_string()
        };
        let _ = write!(
            favs,
            "{} ",
            rust_i18n::t!(
                "season_preview.fb.driver_line",
                name = d.nome.as_str(),
                team = d.equipe.clone().unwrap_or_else(|| tk("token.no_team")).as_str(),
                perc = p.perc_labels[i].as_str(),
                cv = d.curriculo.as_str(),
                trait_suffix = traco.as_str()
            )
        );
    }
    if !favs.trim().is_empty() {
        let _ = write!(body, "\n\n{}", favs.trim());
    }

    // P3 — segundo pelotão + uma relação do grid, se houver.
    let promises: Vec<&str> = p
        .ranked
        .iter()
        .skip(FAVORITES_COUNT)
        .take(PROMISES_COUNT)
        .map(|d| d.nome.as_str())
        .collect();
    if !promises.is_empty() {
        let _ = write!(
            body,
            "\n\n{}",
            rust_i18n::t!("season_preview.fb.promises", names = promises.join(", ").as_str())
        );
    }
    if let Some(rel) = p.relations.first() {
        let _ = write!(body, " {rel}");
    }

    // P4 — trono + abertura.
    let mut closing = String::new();
    if let Some(name) = &p.champion {
        let _ = write!(
            closing,
            "{} ",
            if p.throne_vacant {
                rust_i18n::t!("season_preview.fb.throne_vacant", name = name.as_str()).to_string()
            } else {
                rust_i18n::t!("season_preview.fb.champion_stays", name = name.as_str()).to_string()
            }
        );
    }
    if let Some(track) = &p.opening_track {
        let _ = write!(
            closing,
            "{}",
            rust_i18n::t!(
                "season_preview.fb.closing",
                track = track.as_str(),
                rounds = p.rounds as i64
            )
        );
    }
    if !closing.trim().is_empty() {
        let _ = write!(body, "\n\n{}", closing.trim());
    }

    SeasonPreview {
        headline,
        standfirst,
        body,
    }
}
