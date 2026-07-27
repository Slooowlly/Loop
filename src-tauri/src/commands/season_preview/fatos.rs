//! Montagem do bundle de fatos: lê o mundo e devolve tudo já qualitativo.

use super::*;

// ── Montagem do bundle de fatos ──────────────────────────────────────────────────

/// Tudo que a matéria precisa, já qualitativo. Serve tanto ao prompt quanto ao fallback.
pub(super) struct PreviewData {
    pub(super) facts: String,
    pub(super) teams: serde_json::Value,
    pub(super) thesis: Thesis,
    pub(super) material: Material,
    /// Dossiês na ordem da PERCEPÇÃO pública (favoritos primeiro).
    pub(super) ranked: Vec<Dossier>,
    pub(super) relations: Vec<String>,
    pub(super) opening_track: Option<String>,
    pub(super) rounds: usize,
    pub(super) champion: Option<String>,
    pub(super) throne_vacant: bool,
    pub(super) cat_label: String,
    pub(super) year: i32,
}

pub(super) fn player_category(conn: &rusqlite::Connection, player: &Driver) -> String {
    use crate::db::queries::contracts;
    contracts::get_active_contract_for_pilot(conn, &player.id)
        .ok()
        .flatten()
        .map(|c| c.categoria)
        .or_else(|| player.categoria_atual.clone())
        .unwrap_or_default()
}

pub(super) fn build_preview_data(
    conn: &rusqlite::Connection,
    base_dir: &std::path::Path,
    career_id: &str,
) -> Option<PreviewData> {
    use crate::db::queries::{calendar, contracts, drivers, seasons, team_car, teams};

    let player = drivers::get_player_driver(conn).ok()?;
    let categoria = player_category(conn, &player);
    if categoria.is_empty() {
        return None;
    }
    let season = seasons::get_active_season(conn).ok().flatten()?;
    let grid = drivers::get_drivers_by_active_category(conn, &categoria).unwrap_or_default();
    if grid.is_empty() {
        return None;
    }

    // Equipe ativa por piloto + cores de todas as equipes da categoria.
    let mut team_of: HashMap<String, (String, String)> = HashMap::new();
    let mut salary_of: HashMap<String, f64> = HashMap::new();
    for d in &grid {
        if let Ok(Some(ct)) = contracts::get_active_contract_for_pilot(conn, &d.id) {
            team_of.insert(d.id.clone(), (ct.equipe_id.clone(), ct.equipe_nome.clone()));
            salary_of.insert(d.id.clone(), ct.salario_anual);
        }
    }
    let teams_in_cat = teams::get_teams_by_category(conn, &categoria).unwrap_or_default();
    let mut teams_map = serde_json::Map::new();
    let mut car_levels: Vec<(String, u8)> = Vec::new();
    for tm in &teams_in_cat {
        teams_map
            .entry(tm.nome.clone())
            .or_insert_with(|| serde_json::json!(tm.cor_primaria));
        if let Ok(Some(car)) = team_car::get_team_car(conn, &tm.id) {
            car_levels.push((tm.nome.clone(), car.display_level()));
        }
    }

    // Paridade de material — vira qualidade, nunca número.
    let material = if car_levels.len() < 2 {
        Material::Unknown
    } else {
        let max = car_levels.iter().map(|(_, l)| *l).max().unwrap_or(1);
        let min = car_levels.iter().map(|(_, l)| *l).min().unwrap_or(1);
        if max == min {
            Material::Uniform
        } else {
            Material::Unequal {
                best: car_levels.iter().max_by_key(|(_, l)| *l).unwrap().0.clone(),
                worst: car_levels.iter().min_by_key(|(_, l)| *l).unwrap().0.clone(),
            }
        }
    };

    // Distribuições do grid, para percentis de estilo.
    let agg: Vec<f64> = grid.iter().map(|d| d.atributos.aggression).collect();
    let smo: Vec<f64> = grid.iter().map(|d| d.atributos.smoothness).collect();
    let conf: Vec<f64> = grid.iter().map(|d| d.atributos.confianca).collect();

    // Topo do mercado: só vira gancho se houver um topo DESTACADO (não empate geral).
    let market_top: Option<String> = {
        let mut sorted: Vec<(&String, &f64)> = salary_of.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        match (sorted.first(), sorted.get(1)) {
            (Some((id, top)), Some((_, second))) if **top > 0.0 && **top > **second * 1.05 => {
                Some((*id).clone())
            }
            _ => None,
        }
    };

    // Dossiês + percepção pública.
    let mut ranked: Vec<Dossier> = grid
        .iter()
        .map(|d| {
            let c = &d.stats_carreira;
            let perception = perception_score(d);

            let mut ganchos = Vec::new();
            if market_top.as_deref() == Some(d.id.as_str()) {
                ganchos.push(tk("token.market_top"));
            }
            if d.atributos.midia >= STAR_MIN_FAMA {
                ganchos.push(tk("token.fame_star"));
            }

            Dossier {
                id: d.id.clone(),
                nome: d.nome.clone(),
                equipe: team_of.get(&d.id).map(|(_, n)| n.clone()),
                perception,
                curriculo: curriculo_token(d),
                experiencia: experiencia_token(d),
                tracos: style_traits(d, &agg, &smo, &conf),
                ganchos,
                tem_titulo: c.titulos > 0,
                tem_vitoria: c.vitorias > 0,
                tem_podio: c.podios > 0,
                estreante: c.corridas == 0,
            }
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.perception
            .partial_cmp(&a.perception)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let perc_labels: Vec<String> = ranked
        .iter()
        .enumerate()
        .map(|(i, d)| perception_label(i, d, market_top.as_deref() == Some(d.id.as_str())))
        .collect();

    // Calendário: abertura + tamanho.
    let entries = calendar::get_calendar(conn, &season.id, &categoria).unwrap_or_default();
    let opening_track = entries.first().map(|e| e.track_name.clone());
    let rounds = entries.len();

    // Campeão anterior: ainda está na categoria (defende) ou subiu (trono vago)?
    let champ_id = crate::commands::career::get_previous_champions_in_base_dir(
        base_dir, career_id, &categoria,
    )
    .ok()
    .and_then(|c| c.driver_champion_id);
    let champion = champ_id
        .as_ref()
        .and_then(|id| drivers::get_driver(conn, id).ok())
        .map(|d| d.nome);
    let throne_vacant = match &champ_id {
        Some(id) => !grid.iter().any(|d| &d.id == id),
        None => true,
    };

    let rookie_share =
        ranked.iter().filter(|d| d.estreante).count() as f64 / ranked.len().max(1) as f64;
    let thesis = select_thesis(rookie_share, &material, throne_vacant);

    let cat_label = crate::constants::categories::get_category_config(&categoria)
        .map(|c| c.nome.to_string())
        .unwrap_or_else(|| categoria.clone());

    let relations = build_relations(conn, &grid, &team_of);

    // ── Bundle de fatos (blocos nomeados, tudo qualitativo) ──
    let mut f = String::new();
    let _ = writeln!(
        f,
        "{}",
        rust_i18n::t!(
            "season_preview.block.season",
            category = cat_label.as_str(),
            season = season.numero as i64,
            year = season.ano as i64
        )
    );
    if let Some(track) = &opening_track {
        let _ = writeln!(
            f,
            "{}",
            rust_i18n::t!(
                "season_preview.block.opening",
                track = track.as_str(),
                rounds = rounds as i64
            )
        );
    }
    let _ = writeln!(
        f,
        "{}",
        match (&champion, throne_vacant) {
            (Some(n), true) => {
                rust_i18n::t!("season_preview.block.throne_vacant", name = n.as_str()).to_string()
            }
            (Some(n), false) => {
                rust_i18n::t!("season_preview.block.champion_stays", name = n.as_str()).to_string()
            }
            (None, _) => tk("block.no_champion"),
        }
    );
    let _ = writeln!(
        f,
        "{}",
        match &material {
            Material::Uniform => tk("block.material_uniform"),
            Material::Unequal { best, worst } => rust_i18n::t!(
                "season_preview.block.material_unequal",
                best = best.as_str(),
                worst = worst.as_str()
            )
            .to_string(),
            Material::Unknown => tk("block.material_unknown"),
        }
    );
    let _ = writeln!(
        f,
        "{}",
        rust_i18n::t!(
            "season_preview.block.thesis",
            thesis = tk(&format!("thesis.{}", thesis.id())).as_str()
        )
    );

    let _ = writeln!(f, "\n{}", tk("block.favorites_head"));
    for (i, d) in ranked.iter().take(FAVORITES_COUNT).enumerate() {
        let _ = writeln!(f, "{}", d.fact_line(&perc_labels[i]));
    }

    if ranked.len() > FAVORITES_COUNT {
        // O segundo pelotão estica até o bundle ter MIN_PROFILED dossiês: o modelo escreve
        // uma matéria rasa quando só recebe meia dúzia de nomes.
        let quantos = PROMISES_COUNT.max(MIN_PROFILED.saturating_sub(FAVORITES_COUNT));
        let _ = writeln!(f, "\n{}", tk("block.promises_head"));
        for (i, d) in ranked
            .iter()
            .enumerate()
            .skip(FAVORITES_COUNT)
            .take(quantos)
        {
            let _ = writeln!(f, "{}", d.fact_line(&perc_labels[i]));
        }
    }

    if !relations.is_empty() {
        let _ = writeln!(f, "\n{}", tk("block.relations_head"));
        for r in &relations {
            let _ = writeln!(f, "- {r}");
        }
    }

    // INTUIÇÃO: o skill oculto só vira sussurro QUANDO contradiz a percepção pública —
    // é aí que existe tensão ("no papel o mais rápido, mas ainda sem provar").
    if let Some(fastest) = grid.iter().max_by(|a, b| {
        a.atributos
            .skill
            .partial_cmp(&b.atributos.skill)
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        if ranked.first().map(|d| d.id.as_str()) != Some(fastest.id.as_str()) {
            let _ = writeln!(
                f,
                "\n{}",
                rust_i18n::t!("season_preview.block.hunch", name = fastest.nome.as_str())
            );
        }
    }

    let _ = writeln!(
        f,
        "\n{}",
        rust_i18n::t!("season_preview.block.grid", n = ranked.len() as i64)
    );

    Some(PreviewData {
        facts: f,
        teams: serde_json::Value::Object(teams_map),
        thesis,
        material,
        ranked,
        relations,
        opening_track,
        rounds,
        champion,
        throne_vacant,
        cat_label,
        year: season.ano,
    })
}
