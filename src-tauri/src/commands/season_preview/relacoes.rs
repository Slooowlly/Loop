//! Relações do grid (§5.5): rivalidades e histórico de companheirismo entre os pilotos.

use super::*;

/// Pares de pilotos do grid com história em comum, do enredo mais forte ao mais fraco.
/// Player-agnostic: o jogador é só mais um nome. No máximo uma relação por piloto.
pub(super) fn build_relations(
    conn: &rusqlite::Connection,
    grid: &[Driver],
    team_of: &HashMap<String, (String, String)>,
) -> Vec<String> {
    use crate::db::queries::{contracts, rivalries};

    let names: HashMap<&str, &str> = grid
        .iter()
        .map(|d| (d.id.as_str(), d.nome.as_str()))
        .collect();
    let in_grid: HashSet<&str> = names.keys().copied().collect();

    // (prioridade, texto, piloto_a, piloto_b) — prioridade menor = enredo mais forte.
    let mut cands: Vec<(u8, String, String, String)> = Vec::new();
    let mut seen_pairs: HashSet<(String, String)> = HashSet::new();

    let pair_key = |a: &str, b: &str| {
        if a < b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        }
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
