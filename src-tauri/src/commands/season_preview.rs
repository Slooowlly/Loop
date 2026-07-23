//! "O Que Esperar" — matéria de expectativas de PRÉ-TEMPORADA.
//!
//! Design completo em `docs/season-preview-design.md`. Resumo do que este módulo faz:
//!
//! * Traduz **número em qualidade**: nenhum skill/nível/salário cru sai daqui. Cada sinal
//!   vira um *token* qualitativo, relativo ao grid (percentil).
//! * Aplica **assimetria de informação**: o jornalista enxerga o que é PÚBLICO (resultados,
//!   fama, estilo de pilotagem) e apenas *intui* o que é OCULTO (ritmo/skill). Por isso a
//!   ordem dos favoritos é a **percepção pública**, não o ranking de skill — quem já tem
//!   pódio pesa mais que um talento oculto ainda não provado.
//! * Descreve **traços de estilo** a partir dos atributos que o iRacing consome
//!   (`aggression`, `smoothness`, `confianca`), que são observáveis em pista.
//! * Levanta as **relações do grid** (quem já correu com quem) — do grid inteiro, sem
//!   privilegiar o jogador.
//!
//! O texto final vem do servidor (`/season-preview`). Se ele falhar, um **montador
//! determinístico** produz a matéria a partir dos MESMOS tokens, respeitando as mesmas
//! regras editoriais (3ª pessoa, sem números) — a aba nunca quebra o personagem.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use serde::Serialize;
use tauri::Manager;

use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::ai_story;
use crate::models::driver::Driver;
use crate::narrative::client::{self, SeasonPreview, StoryError};

// ── Curadoria ────────────────────────────────────────────────────────────────────
/// Quantos nomes entram em FAVORITOS (o topo da percepção pública).
const FAVORITES_COUNT: usize = 4;
/// Quantos entram em PROMESSAS / INCÓGNITAS (o segundo pelotão).
const PROMISES_COUNT: usize = 3;
/// Teto de relações do grid citadas (§5.5) — mais que isso vira lista.
const MAX_RELATIONS: usize = 3;
/// Percentis que definem um traço de estilo "marcante" (fora disso, o piloto é mediano
/// naquele eixo e não ganha traço — evita ficha técnica).
const TRAIT_HIGH_PCT: f64 = 0.85;
const TRAIT_LOW_PCT: f64 = 0.15;
/// Máximo de traços por piloto (dá cor sem virar planilha).
const MAX_TRAITS: usize = 2;
/// Intensidade percebida mínima para uma rivalidade virar notícia.
const RIVALRY_MIN_INTENSITY: f64 = 35.0;
/// Fama (escala de exibição) a partir da qual o piloto é "nome de público".
const STAR_MIN_FAMA: f64 = 71.0;

// ── Pesos da PERCEPÇÃO pública (§5.2) ────────────────────────────────────────────
// Resultado ≫ reputação > experiência, e o skill entra só como um empurrão fraco.
// É de propósito: a imprensa não conhece o ritmo real, ela lê o que já aconteceu.
const W_TITLE: f64 = 100.0;
const W_WIN: f64 = 40.0;
const W_PODIUM: f64 = 18.0;
const W_FAME: f64 = 0.35;
const W_CHARISMA: f64 = 0.15;
const W_EXPERIENCE: f64 = 0.8;
/// Experiência satura: a partir daqui, mais corridas não mudam a percepção.
const EXPERIENCE_CAP: f64 = 40.0;
/// O "vazamento" do skill oculto na percepção. Fraco de propósito.
const W_SKILL_HINT: f64 = 0.25;
/// Amplitude do ruído determinístico (±metade), para a percepção não virar um espelho
/// exato do skill quando ninguém tem resultado (arranque de temporada).
const JITTER_RANGE: f64 = 6.0;

/// Ruído estável por piloto (hash FNV-1a do id). Determinístico: a mesma carreira gera
/// sempre a mesma percepção, mas ela não é uma função limpa do skill.
fn jitter(id: &str) -> f64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    ((h % 1000) as f64 / 1000.0 - 0.5) * JITTER_RANGE
}

/// Percentil de um valor dentro do grid (0 = pior, 1 = melhor).
fn percentile(value: f64, all: &[f64]) -> f64 {
    if all.is_empty() {
        return 0.5;
    }
    let at_or_below = all.iter().filter(|v| **v <= value).count() as f64;
    at_or_below / all.len() as f64
}

fn tk(key: &str) -> String {
    // A chave precisa viver enquanto o macro a empresta (não pode ser temporário).
    let full = format!("season_preview.{key}");
    rust_i18n::t!(&full).to_string()
}

// ── Dossiê por piloto (já em linguagem qualitativa) ──────────────────────────────

struct Dossier {
    id: String,
    nome: String,
    equipe: Option<String>,
    /// Score da percepção PÚBLICA — ordena os favoritos (NÃO é o skill).
    perception: f64,
    /// Rótulo do currículo (títulos / vitórias / pódio / ainda sem).
    curriculo: String,
    /// Rótulo de experiência (estreante … veterano).
    experiencia: String,
    /// 0–2 traços de estilo (observáveis em pista).
    tracos: Vec<String>,
    /// Ganchos extras (nome de público, contratação mais cara…).
    ganchos: Vec<String>,
    /// Sinais crus guardados só para o fallback determinístico decidir frases.
    tem_vitoria: bool,
    tem_podio: bool,
    estreante: bool,
}

impl Dossier {
    /// Linha do bundle: `nome | equipe | percepção | currículo | traço | gancho`.
    fn fact_line(&self, perc_label: &str) -> String {
        let mut parts = vec![self.nome.clone()];
        parts.push(self.equipe.clone().unwrap_or_else(|| tk("token.no_team")));
        parts.push(perc_label.to_string());
        parts.push(self.curriculo.clone());
        parts.extend(self.tracos.iter().cloned());
        parts.extend(self.ganchos.iter().cloned());
        format!("- {}", parts.join(" | "))
    }
}

/// Traços de estilo a partir dos atributos que vão para o iRacing (§5.3). Só os extremos
/// do grid viram traço; piloto mediano em tudo fica sem — e isso é proposital.
fn style_traits(d: &Driver, agg: &[f64], smo: &[f64], conf: &[f64]) -> Vec<String> {
    let axes: [(f64, &[f64], &str, &str); 3] = [
        (d.atributos.aggression, agg, "token.trait_aggressive", "token.trait_cautious"),
        (d.atributos.smoothness, smo, "token.trait_smooth", "token.trait_ragged"),
        (d.atributos.confianca, conf, "token.trait_bold", "token.trait_measured"),
    ];
    // (quão extremo, token) — os mais extremos primeiro.
    let mut found: Vec<(f64, String)> = Vec::new();
    for (value, all, high_key, low_key) in axes {
        let p = percentile(value, all);
        if p >= TRAIT_HIGH_PCT {
            found.push(((p - 0.5).abs(), tk(high_key)));
        } else if p <= TRAIT_LOW_PCT {
            found.push(((p - 0.5).abs(), tk(low_key)));
        }
    }
    found.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    found.into_iter().take(MAX_TRAITS).map(|(_, t)| t).collect()
}

fn curriculo_token(d: &Driver) -> String {
    let c = &d.stats_carreira;
    if c.titulos > 0 {
        tk("token.cv_titles")
    } else if c.vitorias > 0 {
        tk("token.cv_wins")
    } else if c.podios > 0 {
        tk("token.cv_podium")
    } else {
        tk("token.cv_none")
    }
}

fn experiencia_token(d: &Driver) -> String {
    match d.stats_carreira.corridas {
        0 => tk("token.exp_debut"),
        1..=15 => tk("token.exp_rising"),
        16..=60 => tk("token.exp_established"),
        _ => tk("token.exp_veteran"),
    }
}

/// Rótulo de percepção conforme a posição na fila pública + o que sustenta o nome.
fn perception_label(rank: usize, d: &Dossier, is_market_top: bool) -> String {
    if d.tem_vitoria {
        return if rank == 0 { tk("token.perc_press_favorite") } else { tk("token.perc_contender") };
    }
    if d.tem_podio {
        return tk("token.perc_rising");
    }
    if is_market_top {
        return tk("token.perc_expensive_bet");
    }
    if d.estreante {
        return tk("token.perc_unknown");
    }
    tk("token.perc_midfield")
}

// ── Contexto da temporada (material, título, tese) ───────────────────────────────

/// Paridade de material entre as equipes da categoria — SEM número.
enum Material {
    Uniform,
    Unequal { best: String, worst: String },
    Unknown,
}

/// A tese dominante da temporada (§4 bloco 1).
enum Thesis {
    RookieSeason,
    VacantThrone,
    UnequalMachinery,
    OpenOnTalent,
}

impl Thesis {
    fn id(&self) -> &'static str {
        match self {
            Thesis::RookieSeason => "rookies",
            Thesis::VacantThrone => "vacant_throne",
            Thesis::UnequalMachinery => "unequal",
            Thesis::OpenOnTalent => "open_talent",
        }
    }
}

/// Elege a tese: grid majoritariamente estreante > material desigual > trono vago > aberta.
fn select_thesis(rookie_share: f64, material: &Material, throne_vacant: bool) -> Thesis {
    if rookie_share >= 0.6 {
        Thesis::RookieSeason
    } else if matches!(material, Material::Unequal { .. }) {
        Thesis::UnequalMachinery
    } else if throne_vacant {
        Thesis::VacantThrone
    } else {
        Thesis::OpenOnTalent
    }
}

// ── Relações do grid (§5.5) ──────────────────────────────────────────────────────

/// Pares de pilotos do grid com história em comum, do enredo mais forte ao mais fraco.
/// Player-agnostic: o jogador é só mais um nome. No máximo uma relação por piloto.
fn build_relations(
    conn: &rusqlite::Connection,
    grid: &[Driver],
    team_of: &HashMap<String, (String, String)>,
) -> Vec<String> {
    use crate::db::queries::{contracts, rivalries};

    let names: HashMap<&str, &str> = grid.iter().map(|d| (d.id.as_str(), d.nome.as_str())).collect();
    let in_grid: HashSet<&str> = names.keys().copied().collect();

    // (prioridade, texto, piloto_a, piloto_b) — prioridade menor = enredo mais forte.
    let mut cands: Vec<(u8, String, String, String)> = Vec::new();
    let mut seen_pairs: HashSet<(String, String)> = HashSet::new();

    let pair_key = |a: &str, b: &str| {
        if a < b { (a.to_string(), b.to_string()) } else { (b.to_string(), a.to_string()) }
    };

    // 1) Rivalidades estabelecidas entre dois nomes do grid.
    if let Ok(all) = rivalries::get_all_rivalries(conn) {
        for r in all {
            if r.perceived_intensity() < RIVALRY_MIN_INTENSITY {
                continue;
            }
            let (a, b) = (r.piloto1_id.as_str(), r.piloto2_id.as_str());
            if !in_grid.contains(a) || !in_grid.contains(b) {
                continue;
            }
            let key = pair_key(a, b);
            if !seen_pairs.insert(key) {
                continue;
            }
            cands.push((
                0,
                rust_i18n::t!(
                    "season_preview.relation.rivalry",
                    a = names[a],
                    b = names[b]
                )
                .to_string(),
                a.to_string(),
                b.to_string(),
            ));
        }
    }

    // 2/3) Quem já dividiu equipe: separados hoje (enredo forte) ou dupla atual.
    for d in grid {
        let Ok(mates) = contracts::get_former_teammates(conn, &d.id) else {
            continue;
        };
        for (mate_id, mate_nome) in mates {
            if !in_grid.contains(mate_id.as_str()) {
                continue;
            }
            let key = pair_key(&d.id, &mate_id);
            if !seen_pairs.insert(key) {
                continue;
            }
            let same_team_now = match (team_of.get(&d.id), team_of.get(&mate_id)) {
                (Some((ta, _)), Some((tb, _))) => ta == tb,
                _ => false,
            };
            if same_team_now {
                let team = team_of
                    .get(&d.id)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_default();
                cands.push((
                    2,
                    rust_i18n::t!(
                        "season_preview.relation.current_duo",
                        a = d.nome.as_str(),
                        b = mate_nome.as_str(),
                        team = team.as_str()
                    )
                    .to_string(),
                    d.id.clone(),
                    mate_id.clone(),
                ));
            } else {
                cands.push((
                    1,
                    rust_i18n::t!(
                        "season_preview.relation.former_mates",
                        a = d.nome.as_str(),
                        b = mate_nome.as_str()
                    )
                    .to_string(),
                    d.id.clone(),
                    mate_id.clone(),
                ));
            }
        }
    }

    cands.sort_by_key(|(prio, _, _, _)| *prio);
    let mut used: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for (_, text, a, b) in cands {
        if out.len() >= MAX_RELATIONS {
            break;
        }
        // No máximo uma relação por piloto — não repete o mesmo nome em vários ganchos.
        if used.contains(&a) || used.contains(&b) {
            continue;
        }
        used.insert(a);
        used.insert(b);
        out.push(text);
    }
    out
}

// ── Montagem do bundle de fatos ──────────────────────────────────────────────────

/// Tudo que a matéria precisa, já qualitativo. Serve tanto ao prompt quanto ao fallback.
struct PreviewData {
    facts: String,
    teams: serde_json::Value,
    thesis: Thesis,
    material: Material,
    /// Dossiês na ordem da PERCEPÇÃO pública (favoritos primeiro).
    ranked: Vec<Dossier>,
    perc_labels: Vec<String>,
    relations: Vec<String>,
    opening_track: Option<String>,
    rounds: usize,
    champion: Option<String>,
    throne_vacant: bool,
    cat_label: String,
    year: i32,
}

fn player_category(conn: &rusqlite::Connection, player: &Driver) -> String {
    use crate::db::queries::contracts;
    contracts::get_active_contract_for_pilot(conn, &player.id)
        .ok()
        .flatten()
        .map(|c| c.categoria)
        .or_else(|| player.categoria_atual.clone())
        .unwrap_or_default()
}

fn build_preview_data(
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
            let perception = W_TITLE * c.titulos as f64
                + W_WIN * c.vitorias as f64
                + W_PODIUM * c.podios as f64
                + W_FAME * d.atributos.midia
                + W_CHARISMA * d.atributos.carisma
                + W_EXPERIENCE * (c.corridas as f64).min(EXPERIENCE_CAP)
                + W_SKILL_HINT * d.atributos.skill
                + jitter(&d.id);

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
        let _ = writeln!(f, "\n{}", tk("block.promises_head"));
        for (i, d) in ranked
            .iter()
            .enumerate()
            .skip(FAVORITES_COUNT)
            .take(PROMISES_COUNT)
        {
            let mut line = d.fact_line(&perc_labels[i]);
            line.push_str(&format!(" | {}", d.experiencia));
            let _ = writeln!(f, "{line}");
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
        perc_labels,
        relations,
        opening_track,
        rounds,
        champion,
        throne_vacant,
        cat_label,
        year: season.ano,
    })
}

// ── Fallback determinístico (§9) ─────────────────────────────────────────────────

/// Monta a matéria sem IA, a partir dos MESMOS tokens. Mais curta e mais template que a
/// versão do servidor, mas obedece às mesmas regras: 3ª pessoa, sem número de atributo,
/// sem se dirigir ao jogador.
fn deterministic_article(p: &PreviewData) -> SeasonPreview {
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

// ── Comando ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SeasonPreviewResult {
    pub headline: Option<String>,
    pub standfirst: Option<String>,
    pub body: Option<String>,
    /// "ai" | "template" — de onde veio o texto exibido.
    pub source: String,
    /// ok | cached | unavailable | rate_limited | error
    pub status: String,
    /// Mapa nome da equipe → cor (para o front colorir os nomes citados).
    pub teams: Option<serde_json::Value>,
}

/// Serializa/desserializa o trio no campo `story` do cache (evita nova tabela).
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedPreview {
    headline: String,
    standfirst: String,
    body: String,
}

/// Comando: matéria "O Que Esperar" da temporada. O front chama ao abrir a revista
/// enquanto não há edição de corrida. Cacheia por temporada+categoria. Se a IA falhar,
/// devolve a versão determinística (`source: "template"`) — a aba nunca fica vazia.
#[tauri::command]
pub async fn enrich_season_preview_ai(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<SeasonPreviewResult, String> {
    // async + spawn_blocking: o fetch de IA é bloqueante (até 45s) e travaria a THREAD
    // PRINCIPAL do Tauri se rodasse síncrono. Ver enrich_race_news_ai.
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

        let mut config = AppConfig::load_or_default(&base_dir);
        let install_id = config.get_or_create_install_id();
        let lang = config.language.clone();

        let db_path = config.saves_dir().join(&career_id).join("career.db");
        let db =
            Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

        let Some(data) = build_preview_data(&db.conn, &base_dir, &career_id) else {
            return Ok(SeasonPreviewResult {
                headline: None,
                standfirst: None,
                body: None,
                source: "template".to_string(),
                status: "unavailable".to_string(),
                teams: None,
            });
        };

        let teams = if data.teams.as_object().map_or(true, |m| m.is_empty()) {
            None
        } else {
            Some(data.teams.clone())
        };

        let season_id = crate::db::queries::seasons::get_active_season(&db.conn)
            .ok()
            .flatten()
            .map(|s| s.id)
            .unwrap_or_default();
        let category = crate::db::queries::drivers::get_player_driver(&db.conn)
            .ok()
            .map(|p| player_category(&db.conn, &p))
            .unwrap_or_default();
        let cache_key = format!("season-preview:{season_id}:{category}");

        // Cache → reabrir a revista não regenera (sem custo, sem cooldown).
        if let Ok(Some(row)) = ai_story::get_story(&db.conn, &cache_key) {
            if let Some(raw) = row.story {
                if let Ok(c) = serde_json::from_str::<CachedPreview>(&raw) {
                    return Ok(SeasonPreviewResult {
                        headline: Some(c.headline),
                        standfirst: Some(c.standfirst),
                        body: Some(c.body),
                        source: "ai".to_string(),
                        status: "cached".to_string(),
                        teams,
                    });
                }
            }
        }

        // 1ª vez: gera no servidor. Em QUALQUER falha cai no determinístico (que NÃO é
        // cacheado — assim uma próxima abertura ainda pode conseguir a versão de IA).
        match client::fetch_season_preview(&data.facts, &lang, &install_id) {
            Ok(p) => {
                let cached = CachedPreview {
                    headline: p.headline.clone(),
                    standfirst: p.standfirst.clone(),
                    body: p.body.clone(),
                };
                if let Ok(json) = serde_json::to_string(&cached) {
                    let teams_json =
                        serde_json::to_string(&data.teams).unwrap_or_else(|_| "{}".into());
                    if let Err(e) =
                        ai_story::store_race_facts(&db.conn, &cache_key, &data.facts, &teams_json)
                    {
                        eprintln!("[season-preview] Falha ao guardar fatos: {e:?}");
                    }
                    if let Err(e) = ai_story::set_story(&db.conn, &cache_key, &json) {
                        eprintln!("[season-preview] Falha ao cachear matéria: {e:?}");
                    }
                }
                Ok(SeasonPreviewResult {
                    headline: Some(p.headline),
                    standfirst: Some(p.standfirst),
                    body: Some(p.body),
                    source: "ai".to_string(),
                    status: "ok".to_string(),
                    teams,
                })
            }
            Err(err) => {
                let fb = deterministic_article(&data);
                Ok(SeasonPreviewResult {
                    headline: Some(fb.headline),
                    standfirst: Some(fb.standfirst),
                    body: Some(fb.body),
                    source: "template".to_string(),
                    status: match err {
                        StoryError::RateLimited => "rate_limited".to_string(),
                        _ => "error".to_string(),
                    },
                    teams,
                })
            }
        }
    })
    .await
    .map_err(|e| format!("Falha ao executar matéria de pré-temporada: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentil_ordena_do_pior_ao_melhor() {
        let all = vec![10.0, 20.0, 30.0, 40.0];
        assert!(percentile(40.0, &all) > percentile(10.0, &all));
        assert_eq!(percentile(40.0, &all), 1.0);
    }

    #[test]
    fn jitter_e_deterministico_e_limitado() {
        let a = jitter("P123");
        assert_eq!(a, jitter("P123"), "mesmo id → mesmo ruído");
        assert!(a.abs() <= JITTER_RANGE / 2.0 + 1e-9);
        assert_ne!(jitter("P123"), jitter("P124"));
    }

    /// O CURRÍCULO tem que pesar mais que o skill oculto: um piloto com pódio precisa
    /// ficar à frente de outro sem resultado, mesmo com skill bem menor. É a assimetria
    /// de informação do design (§5.2) — a imprensa lê resultado, não ritmo.
    #[test]
    fn podio_pesa_mais_que_skill_na_percepcao() {
        let com_podio = W_PODIUM * 1.0 + W_SKILL_HINT * 40.0;
        let so_skill = W_SKILL_HINT * 50.0;
        assert!(
            com_podio > so_skill + JITTER_RANGE,
            "um pódio deve superar a vantagem de skill mesmo no pior caso de ruído"
        );
    }

    #[test]
    fn tese_de_estreantes_vence_quando_grid_e_novato() {
        assert!(matches!(
            select_thesis(0.8, &Material::Uniform, false),
            Thesis::RookieSeason
        ));
        assert!(matches!(
            select_thesis(0.1, &Material::Uniform, true),
            Thesis::VacantThrone
        ));
        assert!(matches!(
            select_thesis(0.1, &Material::Uniform, false),
            Thesis::OpenOnTalent
        ));
    }
}
