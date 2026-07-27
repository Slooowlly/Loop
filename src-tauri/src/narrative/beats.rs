//! Os "beats": pedaços de história já avaliados (com peso) que saem do resultado
//! da corrida, mais o limiar que decide quais deles entram no boletim.

use super::consulta::{dnf_reason_of, find};
use super::incidentes::{
    contact_link, incident_weight, is_crash, scale_label, worst_non_dnf_incident_per_pilot,
};
use crate::simulation::incidents::{IncidentResult, IncidentType};
use crate::simulation::race::RaceResult;

/// Limiar padrão: entram no boletim os beats com peso >= isto.
pub const THRESHOLD: f64 = 30.0;
/// O beat do nosso piloto tem um limiar próprio (mais baixo): queremos que ele
/// seja CITADO quase sempre que fez algo minimamente notável — mas nunca como
/// protagonista.
pub const PLAYER_THRESHOLD: f64 = 25.0;
/// Um arco de rivalidade acima deste peso sobe para DESTAQUES mesmo quando a tese
/// da corrida não o pediu: a novela é boa demais para virar rodapé, e ela é o único
/// ângulo que a tese — que só lê o `RaceResult` — não tem como enxergar sozinha.
pub const ARC_HIGHLIGHT_WEIGHT: f64 = 60.0;
/// Teto de segurança de tokens. A seleção é dinâmica (quanto mais caótica a
/// corrida, mais beats), mas nunca passa disto. Raramente morde.
pub const MAX_BEATS: usize = 14;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BeatKind {
    Vitoria,
    Podio,
    VoltaRapida,
    Recuperacao,
    Decepcao,
    Abandono,
    /// Batida/rodada que NÃO terminou em abandono — o piloto seguiu na prova. Os
    /// abandonos continuam em `Abandono`; aqui entra o que antes sumia do boletim.
    Acidente,
    Lesao,
    NossoPiloto,
    /// Capítulo de uma rivalidade em curso — o único beat com MEMÓRIA entre corridas.
    /// Nasce fora daqui (o callsite tem o banco) e chega por `career_beats`.
    RivalidadeArco,
}

/// Um pedaço de história já avaliado e renderizado como fato em PT neutro.
#[derive(Debug, Clone)]
pub struct Beat {
    pub kind: BeatKind,
    pub weight: f64,
    /// Frase factual pronta. A IA redige em cima disto (e traduz pro idioma do jogador).
    pub text: String,
    pub driver_id: Option<String>,
    pub team_name: Option<String>,
}

impl Beat {
    fn threshold(&self) -> f64 {
        match self.kind {
            BeatKind::NossoPiloto => PLAYER_THRESHOLD,
            _ => THRESHOLD,
        }
    }

    pub(crate) fn passes(&self) -> bool {
        self.weight >= self.threshold()
    }

    /// Sobe para DESTAQUES mesmo sem a tese ter pedido. Só o arco de rivalidade forte:
    /// a tese só lê o `RaceResult` e por isso é cega para a novela entre corridas.
    pub(crate) fn forces_highlight(&self) -> bool {
        self.kind == BeatKind::RivalidadeArco && self.weight >= ARC_HIGHLIGHT_WEIGHT
    }
}

/// Gera todos os beats candidatos a partir do resultado da corrida.
pub fn build_beats(result: &RaceResult, incidents: &[IncidentResult]) -> Vec<Beat> {
    let mut beats: Vec<Beat> = Vec::new();
    let rows = &result.race_results;

    // ── Vitória (espinha) ────────────────────────────────────────────────────
    if let Some(w) = find(rows, &result.winner_id) {
        let mut weight = 70.0;
        // Vencer largando lá atrás vale mais.
        if w.grid_position >= 6 {
            weight += ((w.grid_position - 5) as f64).min(15.0);
        }
        let extra = if w.has_fastest_lap {
            rust_i18n::t!("narrative.beat.winner_fastest_extra").to_string()
        } else {
            String::new()
        };
        beats.push(Beat {
            kind: BeatKind::Vitoria,
            weight,
            text: rust_i18n::t!(
                "narrative.beat.winner",
                name = w.pilot_name.as_str(),
                team = w.team_name.as_str(),
                grid = w.grid_position,
                extra = extra.as_str()
            )
            .to_string(),
            driver_id: Some(w.pilot_id.clone()),
            team_name: Some(w.team_name.clone()),
        });
    }

    // ── Pódio (2º e 3º) ──────────────────────────────────────────────────────
    for pos in [2, 3] {
        if let Some(d) = rows.iter().find(|d| !d.is_dnf && d.finish_position == pos) {
            beats.push(Beat {
                kind: BeatKind::Podio,
                weight: 30.0,
                text: rust_i18n::t!(
                    "narrative.beat.podium",
                    pos = pos,
                    name = d.pilot_name.as_str(),
                    team = d.team_name.as_str()
                )
                .to_string(),
                driver_id: Some(d.pilot_id.clone()),
                team_name: Some(d.team_name.clone()),
            });
        }
    }

    // ── Maior recuperação ────────────────────────────────────────────────────
    if let Some(id) = &result.most_positions_gained_id {
        if let Some(d) = find(rows, id) {
            if d.positions_gained > 0 {
                let weight = (30.0 + d.positions_gained as f64 * 2.0).min(70.0);
                beats.push(Beat {
                    kind: BeatKind::Recuperacao,
                    weight,
                    text: rust_i18n::t!(
                        "narrative.beat.recovery",
                        name = d.pilot_name.as_str(),
                        grid = d.grid_position,
                        finish = d.finish_position,
                        gained = d.positions_gained
                    )
                    .to_string(),
                    driver_id: Some(d.pilot_id.clone()),
                    team_name: Some(d.team_name.clone()),
                });
            }
        }
    }

    // ── Volta mais rápida (só se não for o vencedor — senão já está na vitória) ─
    if let Some(d) = find(rows, &result.fastest_lap_id) {
        if d.pilot_id != result.winner_id {
            let mut weight = 15.0;
            if d.is_jogador {
                weight += 10.0;
            }
            beats.push(Beat {
                kind: BeatKind::VoltaRapida,
                weight,
                text: rust_i18n::t!(
                    "narrative.beat.fastest_lap",
                    name = d.pilot_name.as_str(),
                    team = d.team_name.as_str()
                )
                .to_string(),
                driver_id: Some(d.pilot_id.clone()),
                team_name: Some(d.team_name.clone()),
            });
        }
    }

    // ── Decepção: pole que não venceu e terminou mal ─────────────────────────
    if let Some(p) = find(rows, &result.pole_sitter_id) {
        if p.pilot_id != result.winner_id && !p.is_dnf && p.finish_position >= 5 {
            let weight = (30.0 + (p.finish_position - 1) as f64 * 2.0).min(60.0);
            beats.push(Beat {
                kind: BeatKind::Decepcao,
                weight,
                text: rust_i18n::t!(
                    "narrative.beat.disappointment",
                    name = p.pilot_name.as_str(),
                    finish = p.finish_position
                )
                .to_string(),
                driver_id: Some(p.pilot_id.clone()),
                team_name: Some(p.team_name.clone()),
            });
        }
    }

    // ── Abandonos ────────────────────────────────────────────────────────────
    // Base baixa de propósito: um abandono de meio de pelotão (22) fica abaixo do
    // limiar e não polui o boletim; só os de quem largou na frente (40) passam.
    // EXCEÇÃO: se o abandono veio de BATIDA (não de peça quebrada), ele sempre sobe
    // acima do limiar — uma batida que tira alguém da prova é notícia mesmo no meio
    // do pelotão. Pane mecânica anônima continua sendo pano de fundo.
    for d in rows.iter().filter(|d| d.is_dnf) {
        let dnf_inc = incidents
            .iter()
            .find(|i| i.pilot_id == d.pilot_id && i.is_dnf);
        let mut weight = 22.0;
        if d.grid_position <= 3 {
            weight += 18.0;
        }
        if d.notable_incident.is_some() {
            weight += 8.0;
        }
        if dnf_inc.map(is_crash).unwrap_or(false) {
            weight += 10.0;
        }
        let link = dnf_inc.map(|i| contact_link(rows, i)).unwrap_or_default();
        beats.push(Beat {
            kind: BeatKind::Abandono,
            weight,
            text: rust_i18n::t!(
                "narrative.beat.dnf",
                name = d.pilot_name.as_str(),
                team = d.team_name.as_str(),
                reason = dnf_reason_of(d),
                link = link.as_str()
            )
            .to_string(),
            driver_id: Some(d.pilot_id.clone()),
            team_name: Some(d.team_name.clone()),
        });
    }

    // ── Batidas SEM abandono (o buraco que existia) ──────────────────────────
    // Quem bateu e seguiu na prova não aparecia em lugar nenhum do boletim. Agora
    // aparece, com a escala explícita para a IA dosar o tom.
    for inc in worst_non_dnf_incident_per_pilot(incidents) {
        let Some(d) = find(rows, &inc.pilot_id) else {
            continue;
        };
        // Soluço mecânico sem custo é ruído: o carro tossiu e nada aconteceu. Erro de
        // pilotagem e contato ficam, mesmo leves e sem custo — é justamente o "algo
        // pequeno" que a matéria deve citar como pequeno. Para a IA isso não vira spam
        // porque o PESO de um incidente leve dela (16) não passa do limiar.
        if inc.incident_type == IncidentType::Mechanical && inc.positions_lost == 0 {
            continue;
        }
        let cost = if inc.positions_lost > 0 {
            rust_i18n::t!("narrative.beat.incident_cost", n = inc.positions_lost).to_string()
        } else {
            String::new()
        };
        let text = if d.is_jogador {
            rust_i18n::t!(
                "narrative.beat.incident_player",
                scale = scale_label(inc.severity).as_str(),
                desc = inc.description.as_str(),
                link = contact_link(rows, inc).as_str(),
                cost = cost.as_str()
            )
            .to_string()
        } else {
            rust_i18n::t!(
                "narrative.beat.incident_ai",
                scale = scale_label(inc.severity).as_str(),
                name = d.pilot_name.as_str(),
                team = d.team_name.as_str(),
                desc = inc.description.as_str(),
                link = contact_link(rows, inc).as_str(),
                cost = cost.as_str()
            )
            .to_string()
        };
        beats.push(Beat {
            kind: BeatKind::Acidente,
            weight: incident_weight(inc, d.is_jogador),
            text,
            driver_id: Some(d.pilot_id.clone()),
            team_name: Some(d.team_name.clone()),
        });
    }

    // ── Nosso piloto (citado, nunca protagonista) ────────────────────────────
    if let Some(p) = rows.iter().find(|d| d.is_jogador) {
        let mut weight = 40.0;
        if !p.is_dnf && p.finish_position <= 3 {
            weight += 20.0;
        }
        if p.positions_gained >= 4 {
            weight += 10.0;
        }
        if p.is_dnf {
            weight += 10.0;
        }
        if p.has_fastest_lap {
            weight += 8.0;
        }
        let status = if p.is_dnf {
            rust_i18n::t!("narrative.beat.player_dnf_status", reason = dnf_reason_of(p)).to_string()
        } else {
            rust_i18n::t!(
                "narrative.beat.player_status",
                grid = p.grid_position,
                finish = p.finish_position,
                pts = p.points_earned
            )
            .to_string()
        };
        beats.push(Beat {
            kind: BeatKind::NossoPiloto,
            weight,
            // Sem tag inline: o piloto do leitor é marcado numa linha rotulada
            // separada em build_race_context (à prova de modelo — nada para copiar).
            text: rust_i18n::t!(
                "narrative.beat.player",
                name = p.pilot_name.as_str(),
                team = p.team_name.as_str(),
                status = status.as_str()
            )
            .to_string(),
            driver_id: Some(p.pilot_id.clone()),
            team_name: Some(p.team_name.clone()),
        });
    }

    beats
}

/// Aplica o limiar (dinâmico) e ordena por peso. O tamanho do resultado é
/// consequência da corrida: morna → poucos beats; caótica → muitos.
pub fn select(mut beats: Vec<Beat>) -> Vec<Beat> {
    beats.retain(|b| b.passes());
    beats.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
    beats.truncate(MAX_BEATS);
    beats
}
