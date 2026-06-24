#![allow(dead_code)]

//! Motor de contexto narrativo do boletim de IA.
//!
//! Transforma o resultado de uma corrida em "beats" (pedaços de história já
//! avaliados, cada um com um peso). Em seguida filtra pelo limiar de relevância
//! e renderiza um CONTEXTO CURADO — denso em narrativa, enxuto em dados — que é
//! o que será enviado ao servidor → Gemini.
//!
//! Filosofia: a inteligência de "o que é interessante" mora AQUI, não na IA.
//! A IA só redige em cima dos fatos que escolhermos (zero invenção de resultado).
//!
//! Esta é a Etapa A (MVP): só os beats que saem do próprio `RaceResult`.
//! Os beats de carreira/forma (lesão, rookie, rivalidade-arco, forma das últimas
//! 5 corridas) entram na Etapa B, alimentados pela base do app.

use crate::simulation::race::{RaceDriverResult, RaceResult};

pub mod client;

/// Limiar padrão: entram no boletim os beats com peso >= isto.
pub const THRESHOLD: f64 = 30.0;
/// O beat do nosso piloto tem um limiar próprio (mais baixo): queremos que ele
/// seja CITADO quase sempre que fez algo minimamente notável — mas nunca como
/// protagonista.
pub const PLAYER_THRESHOLD: f64 = 25.0;
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
    Lesao,
    NossoPiloto,
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

    fn passes(&self) -> bool {
        self.weight >= self.threshold()
    }
}

/// Metadados da corrida que não vêm dentro do `RaceResult`.
pub struct RaceContextInput<'a> {
    /// Nome de exibição da categoria (ex.: "Mazda Rookie"), não o id interno.
    pub category_name: &'a str,
    /// Ano calendário da temporada (ex.: 2027) — NÃO o contador interno de temporadas.
    pub year: i32,
    pub round: i32,
    /// Lesões ocorridas na corrida, já renderizadas como fato (nome resolvido).
    pub injuries: &'a [String],
    /// Pano de fundo (rookie/veterano, histórico, sequência...) já renderizado.
    /// Vai numa seção "Contexto" — cor pra IA usar quando ajudar, sem virar manchete.
    pub context_facts: &'a [String],
}

/// Resultado: o contexto curado pronto pra enviar, + alguns sinais úteis.
#[derive(Debug, Clone)]
pub struct RaceContext {
    /// Texto de fatos enviado ao servidor (campo `facts`).
    pub facts: String,
    /// Quantos beats sobreviveram ao limiar (proxy da "densidade" da corrida).
    pub beat_count: usize,
    /// Se o nosso piloto entrou nos fatos.
    pub has_player: bool,
}

fn find<'a>(results: &'a [RaceDriverResult], id: &str) -> Option<&'a RaceDriverResult> {
    results.iter().find(|d| d.pilot_id == id)
}

fn dnf_reason_of(d: &RaceDriverResult) -> String {
    d.notable_incident
        .clone()
        .or_else(|| d.dnf_reason.clone())
        .unwrap_or_else(|| "abandono".to_string())
}

/// Gera todos os beats candidatos a partir do resultado da corrida.
pub fn build_beats(result: &RaceResult) -> Vec<Beat> {
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
            "; também cravou a volta mais rápida"
        } else {
            ""
        };
        beats.push(Beat {
            kind: BeatKind::Vitoria,
            weight,
            text: format!(
                "Vencedor: {} ({}), largou em P{}{}",
                w.pilot_name, w.team_name, w.grid_position, extra
            ),
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
                text: format!("P{}: {} ({})", pos, d.pilot_name, d.team_name),
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
                    text: format!(
                        "Maior recuperação: {}, de P{} a P{} ({} posições ganhas)",
                        d.pilot_name, d.grid_position, d.finish_position, d.positions_gained
                    ),
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
                text: format!("Volta mais rápida: {} ({})", d.pilot_name, d.team_name),
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
                text: format!(
                    "Decepção: {} saiu da pole e terminou apenas em P{}",
                    p.pilot_name, p.finish_position
                ),
                driver_id: Some(p.pilot_id.clone()),
                team_name: Some(p.team_name.clone()),
            });
        }
    }

    // ── Abandonos ────────────────────────────────────────────────────────────
    // Base baixa de propósito: um abandono de meio de pelotão (22) fica abaixo do
    // limiar e não polui o boletim; só os de quem largou na frente (40) passam.
    for d in rows.iter().filter(|d| d.is_dnf) {
        let mut weight = 22.0;
        if d.grid_position <= 3 {
            weight += 18.0;
        }
        if d.notable_incident.is_some() {
            weight += 8.0;
        }
        beats.push(Beat {
            kind: BeatKind::Abandono,
            weight,
            text: format!(
                "Abandono: {} ({}) — {}",
                d.pilot_name,
                d.team_name,
                dnf_reason_of(d)
            ),
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
            format!("abandonou ({})", dnf_reason_of(p))
        } else {
            format!(
                "largou em P{} e terminou em P{} ({} pts)",
                p.grid_position, p.finish_position, p.points_earned
            )
        };
        beats.push(Beat {
            kind: BeatKind::NossoPiloto,
            weight,
            // Sem tag inline: o piloto do leitor é marcado numa linha rotulada
            // separada em build_race_context (à prova de modelo — nada para copiar).
            text: format!("{} ({}) {}", p.pilot_name, p.team_name, status),
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

/// Renderiza o contexto curado final a partir do resultado + metadados.
pub fn build_race_context(result: &RaceResult, input: &RaceContextInput) -> RaceContext {
    let mut beats = build_beats(result);
    // Lesões da corrida entram como beats de drama (já vêm renderizadas).
    for injury_text in input.injuries {
        beats.push(Beat {
            kind: BeatKind::Lesao,
            weight: 50.0,
            text: injury_text.clone(),
            driver_id: None,
            team_name: None,
        });
    }
    let selected = select(beats);
    let has_player = selected.iter().any(|b| b.kind == BeatKind::NossoPiloto);

    let mut header = format!(
        "Corrida: {} — temporada de {}, etapa {}\nPista: {}, {} voltas, clima: {}",
        input.category_name, input.year, input.round, result.track_name, result.total_laps,
        result.weather
    );
    if result.total_dnfs >= 2 {
        header.push_str(&format!(". A corrida teve {} abandonos", result.total_dnfs));
    }

    let mut facts = header;
    // Marca, numa linha rotulada, qual piloto é o do leitor (só se ele aparece nos
    // fatos). O servidor usa isso para citá-lo pelo nome, nunca como protagonista,
    // e é instruído a NÃO imprimir esta linha.
    if has_player {
        if let Some(name) = result
            .race_results
            .iter()
            .find(|d| d.is_jogador)
            .map(|d| d.pilot_name.clone())
        {
            facts.push_str(&format!("\nPiloto acompanhado pelo leitor: {name}"));
        }
    }
    facts.push_str("\n\nFatos (não invente nada além destes):\n");
    for beat in &selected {
        facts.push_str(&format!("- {}\n", beat.text));
    }

    if !input.context_facts.is_empty() {
        facts.push_str("\nContexto (pano de fundo — use para dar cor quando fizer sentido, sem virar o assunto principal):\n");
        for fact in input.context_facts {
            facts.push_str(&format!("- {fact}\n"));
        }
    }

    RaceContext {
        facts: facts.trim_end().to_string(),
        beat_count: selected.len(),
        has_player,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beat(kind: BeatKind, weight: f64) -> Beat {
        Beat {
            kind,
            weight,
            text: "x".to_string(),
            driver_id: None,
            team_name: None,
        }
    }

    #[test]
    fn limiar_corta_beats_fracos_mas_preserva_o_nosso_piloto() {
        let beats = vec![
            beat(BeatKind::Vitoria, 70.0),
            beat(BeatKind::VoltaRapida, 15.0), // abaixo de 30 → fora
            beat(BeatKind::NossoPiloto, 27.0), // abaixo de 30, mas acima do limiar do jogador (25) → entra
        ];
        let sel = select(beats);
        assert!(sel.iter().any(|b| b.kind == BeatKind::Vitoria));
        assert!(!sel.iter().any(|b| b.kind == BeatKind::VoltaRapida));
        assert!(sel.iter().any(|b| b.kind == BeatKind::NossoPiloto));
    }

    #[test]
    fn selecao_ordena_por_peso_decrescente() {
        let beats = vec![
            beat(BeatKind::Podio, 30.0),
            beat(BeatKind::Vitoria, 70.0),
            beat(BeatKind::Recuperacao, 45.0),
        ];
        let sel = select(beats);
        assert_eq!(sel.first().map(|b| b.kind.clone()), Some(BeatKind::Vitoria));
        assert_eq!(sel.last().map(|b| b.kind.clone()), Some(BeatKind::Podio));
    }
}
