//! Fallback determinístico: a mesma matéria sem IA, a partir dos mesmos tokens.

use super::*;

// ── Fallback determinístico (§9) ─────────────────────────────────────────────────

/// Como a equipe entra numa frase: forma VERBAL ("corre pela X"), pra fechar uma oração.
fn team_verb(d: &Dossier) -> String {
    match &d.equipe {
        Some(t) => rust_i18n::t!("season_preview.fb.team_verb", team = t.as_str()).to_string(),
        None => tk("fb.team_verb_none"),
    }
}

/// Como a equipe entra numa frase: forma de APOSTO ("da X"), entre vírgulas.
fn team_apposto(d: &Dossier) -> String {
    match &d.equipe {
        Some(t) => rust_i18n::t!("season_preview.fb.team_apposto", team = t.as_str()).to_string(),
        None => tk("fb.team_apposto_none"),
    }
}

/// O traço de estilo colado ao fim da frase, com travessão. Só os dois primeiros nomes
/// ganham — a partir do terceiro isso viraria ficha técnica.
fn traco_suffix(d: &Dossier, usados: &mut HashSet<&'static str>) -> String {
    match d.traco_clause(usados) {
        Some(c) => rust_i18n::t!("season_preview.fb.trait_suffix", style = c.as_str()).to_string(),
        None => String::new(),
    }
}

/// Qual variante do currículo sustenta a frase do favorito.
fn cv_variant(d: &Dossier) -> &'static str {
    if d.tem_titulo {
        "titles"
    } else if d.tem_vitoria {
        "wins"
    } else if d.tem_podio {
        "podium"
    } else {
        "none"
    }
}

/// Monta a matéria sem IA, a partir dos MESMOS tokens. Mais curta que a versão do
/// servidor, mas obedece às mesmas regras: 3ª pessoa, sem número de atributo, sem se
/// dirigir ao jogador. Três parágrafos — cenário, o topo, o resto — e cada piloto citado
/// ganha uma frase de forma DIFERENTE, pra não soar lista preenchida por máquina.
pub(super) fn deterministic_article(p: &PreviewData) -> SeasonPreview {
    let headline = rust_i18n::t!(
        &format!("season_preview.fb.headline.{}", p.thesis.id()),
        category = p.cat_label.as_str(),
        year = p.year as i64
    )
    .to_string();
    let standfirst = tk(&format!("fb.standfirst.{}", p.thesis.id()));

    let mut body = String::new();

    // P1 — cenário: a tese abre, o material qualifica, o título fecha.
    let _ = write!(
        body,
        "{}",
        rust_i18n::t!(
            &format!("season_preview.fb.scene.{}", p.thesis.id()),
            category = p.cat_label.as_str(),
            year = p.year as i64
        )
    );
    match &p.material {
        Material::Uniform => {
            let _ = write!(body, " {}", tk("fb.material_uniform"));
        }
        Material::Unequal { best, worst } => {
            let _ = write!(
                body,
                " {}",
                rust_i18n::t!(
                    "season_preview.fb.material_unequal",
                    best = best.as_str(),
                    worst = worst.as_str()
                )
            );
        }
        Material::Unknown => {}
    }
    if let Some(name) = &p.champion {
        let key = if p.throne_vacant {
            "season_preview.fb.throne_vacant"
        } else {
            "season_preview.fb.champion_stays"
        };
        let _ = write!(body, " {}", rust_i18n::t!(key, name = name.as_str()));
    }

    // P2 — o topo da percepção pública, três frases de formas distintas.
    let mut top = String::new();
    let mut tracos_usados: HashSet<&'static str> = HashSet::new();
    if let Some(d) = p.ranked.first() {
        let _ = write!(
            top,
            "{}",
            rust_i18n::t!(
                &format!("season_preview.fb.lead.{}", cv_variant(d)),
                name = d.nome.as_str(),
                team = team_verb(d).as_str(),
                trait_suffix = traco_suffix(d, &mut tracos_usados).as_str()
            )
        );
    }
    if let Some(d) = p.ranked.get(1) {
        let variant = if d.tem_vitoria || d.tem_podio {
            "proven"
        } else {
            "unproven"
        };
        let _ = write!(
            top,
            " {}",
            rust_i18n::t!(
                &format!("season_preview.fb.second.{variant}"),
                name = d.nome.as_str(),
                team = team_apposto(d).as_str(),
                trait_suffix = traco_suffix(d, &mut tracos_usados).as_str()
            )
        );
    }
    if let Some(d) = p.ranked.get(2) {
        let variant = if d.tem_vitoria || d.tem_podio {
            "proven"
        } else {
            "unproven"
        };
        let _ = write!(
            top,
            " {}",
            rust_i18n::t!(
                &format!("season_preview.fb.third.{variant}"),
                name = d.nome.as_str(),
                team = team_apposto(d).as_str()
            )
        );
    }
    if !top.trim().is_empty() {
        let _ = write!(body, "\n\n{}", top.trim());
    }

    // P3 — o resto do grid, uma história que o paddock carrega e a data do 1º veredito.
    let mut tail = String::new();
    let promises: Vec<&str> = p
        .ranked
        .iter()
        .skip(FAVORITES_COUNT)
        .take(FB_PACK_COUNT)
        .map(|d| d.nome.as_str())
        .collect();
    if !promises.is_empty() {
        // Um nome só não "formam" um grupo — a frase muda de número, não de conteúdo.
        let key = if promises.len() == 1 {
            "season_preview.fb.pack_one"
        } else {
            "season_preview.fb.pack"
        };
        let _ = write!(
            tail,
            "{}",
            rust_i18n::t!(key, names = promises.join(", ").as_str())
        );
    }
    if let Some(rel) = p.relations.first() {
        if !tail.is_empty() {
            tail.push(' ');
        }
        tail.push_str(rel);
    }
    if let Some(track) = &p.opening_track {
        if !tail.is_empty() {
            tail.push(' ');
        }
        let _ = write!(
            tail,
            "{}",
            rust_i18n::t!(
                "season_preview.fb.closing",
                track = track.as_str(),
                rounds = p.rounds as i64
            )
        );
    }
    if !tail.trim().is_empty() {
        let _ = write!(body, "\n\n{}", tail.trim());
    }

    SeasonPreview {
        headline,
        standfirst,
        body,
    }
}
