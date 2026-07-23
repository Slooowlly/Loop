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

use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};
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
    /// Batida/rodada que NÃO terminou em abandono — o piloto seguiu na prova. Os
    /// abandonos continuam em `Abandono`; aqui entra o que antes sumia do boletim.
    Acidente,
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
    /// Incidentes CRUS da corrida (jogador + IA). O peso e a redação por gravidade
    /// são resolvidos aqui dentro — a curadoria mora no motor, não na IA.
    pub incidents: &'a [IncidentResult],
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
        .unwrap_or_else(|| rust_i18n::t!("narrative.beat.dnf_fallback").to_string())
}

/// Ordem de gravidade, para escolher o PIOR incidente de cada piloto.
fn severity_rank(s: IncidentSeverity) -> u8 {
    match s {
        IncidentSeverity::Minor => 0,
        IncidentSeverity::Major => 1,
        IncidentSeverity::Critical => 2,
    }
}

/// "Batida" no sentido do boletim: contato entre carros, ou erro do piloto que passou
/// de um susto. Falha mecânica NÃO conta — o carro quebrou, ninguém bateu.
fn is_crash(inc: &IncidentResult) -> bool {
    match inc.incident_type {
        IncidentType::Collision => true,
        IncidentType::DriverError => inc.severity != IncidentSeverity::Minor,
        IncidentType::Mechanical => false,
    }
}

/// Rótulo de ESCALA do incidente. Carrega, no próprio texto, a instrução de
/// proporcionalidade: um toque leve tem que ser narrado como toque leve. É isto que
/// impede a IA de transformar um encostão em tragédia.
fn scale_label(sev: IncidentSeverity) -> String {
    let key = match sev {
        IncidentSeverity::Minor => "narrative.beat.incident_scale_minor",
        IncidentSeverity::Major => "narrative.beat.incident_scale_major",
        IncidentSeverity::Critical => "narrative.beat.incident_scale_critical",
    };
    rust_i18n::t!(key).to_string()
}

/// Peso de um incidente NÃO-DNF. O piloto do leitor tem escala própria e mais alta:
/// mesmo um toque leve (32) passa do limiar, porque o pedido é que a batida DELE seja
/// sempre citada — na medida certa. Para a IA, só batida de verdade vira notícia;
/// rodada leve de meio de pelotão continua fora, senão o boletim vira lista de sustos.
fn incident_weight(inc: &IncidentResult, is_player: bool) -> f64 {
    if is_player {
        return match inc.severity {
            IncidentSeverity::Critical => 58.0,
            IncidentSeverity::Major => 44.0,
            IncidentSeverity::Minor => 32.0,
        };
    }
    match (inc.severity, inc.incident_type) {
        (IncidentSeverity::Critical, _) => 40.0,
        (IncidentSeverity::Major, IncidentType::Collision) => 33.0,
        (IncidentSeverity::Major, _) => 26.0,
        (IncidentSeverity::Minor, _) => 16.0,
    }
}

/// Trecho ", em contato com X" quando o incidente envolveu dois carros e o outro
/// piloto está no resultado. Vazio quando foi incidente solo.
fn contact_link(rows: &[RaceDriverResult], inc: &IncidentResult) -> String {
    let Some(other_id) = inc.linked_pilot_id.as_deref() else {
        return String::new();
    };
    match find(rows, other_id) {
        Some(o) => {
            rust_i18n::t!("narrative.beat.incident_link", other = o.pilot_name.as_str()).to_string()
        }
        None => String::new(),
    }
}

/// O PIOR incidente não-DNF de cada piloto (um piloto pode se meter em vários; o
/// boletim só quer o mais marcante de cada um).
fn worst_non_dnf_incident_per_pilot(incidents: &[IncidentResult]) -> Vec<&IncidentResult> {
    let mut best: Vec<&IncidentResult> = Vec::new();
    for inc in incidents.iter().filter(|i| !i.is_dnf) {
        match best.iter().position(|b| b.pilot_id == inc.pilot_id) {
            Some(i) if severity_rank(inc.severity) > severity_rank(best[i].severity) => {
                best[i] = inc;
            }
            Some(_) => {}
            None => best.push(inc),
        }
    }
    best
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

/// A TESE JORNALÍSTICA do boletim — o ângulo dominante da matéria (voz de revista,
/// grid inteiro). Diferente da prévia/debrief (que giram no piloto do leitor), aqui o
/// eixo é a HISTÓRIA DA CORRIDA: caos, vitória improvável, pole que afundou, remontada,
/// domínio, ou dia de administração. O piloto do leitor segue citado, nunca protagonista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaceThesis {
    Caos,
    VitoriaImprovavel,
    PoleFrustrada,
    Remontada,
    Dominio,
    CorridaLimpa,
}

/// Sinais destilados do resultado que alimentam a tese (puros/testáveis, sem depender
/// da estrutura inteira do `RaceResult`).
pub struct RaceThesisSignals {
    pub total_dnfs: i32,
    pub field_size: i32,
    pub winner_name: String,
    pub winner_team: String,
    pub winner_grid: i32,
    /// (nome, chegada) da POLE quando ela NÃO venceu e afundou (>= P5).
    pub pole_flopped: Option<(String, i32)>,
    /// (nome, largada, chegada, ganho) da maior recuperação, se NÃO foi o vencedor.
    pub biggest_recovery: Option<(String, i32, i32, i32)>,
}

/// Extrai os sinais da tese do resultado bruto.
pub fn race_thesis_signals(result: &RaceResult) -> RaceThesisSignals {
    let rows = &result.race_results;
    let winner = find(rows, &result.winner_id);
    let pole_flopped = find(rows, &result.pole_sitter_id).and_then(|p| {
        if p.pilot_id != result.winner_id && !p.is_dnf && p.finish_position >= 5 {
            Some((p.pilot_name.clone(), p.finish_position))
        } else {
            None
        }
    });
    let biggest_recovery = result
        .most_positions_gained_id
        .as_ref()
        .and_then(|id| find(rows, id))
        .and_then(|d| {
            if d.pilot_id != result.winner_id && d.positions_gained >= 1 {
                Some((
                    d.pilot_name.clone(),
                    d.grid_position,
                    d.finish_position,
                    d.positions_gained,
                ))
            } else {
                None
            }
        });
    RaceThesisSignals {
        total_dnfs: result.total_dnfs,
        field_size: rows.len() as i32,
        winner_name: winner
            .map(|w| w.pilot_name.clone())
            .unwrap_or_else(|| rust_i18n::t!("narrative.beat.winner_fallback").to_string()),
        winner_team: winner.map(|w| w.team_name.clone()).unwrap_or_default(),
        winner_grid: winner.map(|w| w.grid_position).unwrap_or(0),
        pole_flopped,
        biggest_recovery,
    }
}

/// Elege a tese dominante (1ª que casar vence). Devolve o statement do EIXO + os
/// `BeatKind` que devem subir para a camada de DESTAQUES; o resto vira pano de fundo.
pub fn select_race_thesis(s: &RaceThesisSignals) -> (RaceThesis, String, Vec<BeatKind>) {
    use BeatKind::*;
    // Caos: muitos abandonos redesenharam o grid.
    let caos_gate = 4.max(s.field_size / 4);
    if s.total_dnfs >= caos_gate {
        return (
            RaceThesis::Caos,
            rust_i18n::t!("narrative.thesis.caos", dnfs = s.total_dnfs).to_string(),
            vec![Abandono, Acidente, Vitoria],
        );
    }
    // Vitória improvável: o vencedor veio lá de trás.
    if s.winner_grid >= 6 {
        return (
            RaceThesis::VitoriaImprovavel,
            rust_i18n::t!(
                "narrative.thesis.improbable_win",
                name = s.winner_name.as_str(),
                team = s.winner_team.as_str(),
                grid = s.winner_grid
            )
            .to_string(),
            vec![Vitoria, Recuperacao],
        );
    }
    // Pole frustrada: o favorito da pole afundou.
    if let Some((pole_name, finish)) = &s.pole_flopped {
        return (
            RaceThesis::PoleFrustrada,
            rust_i18n::t!(
                "narrative.thesis.pole_flop",
                pole = pole_name.as_str(),
                finish = finish,
                winner = s.winner_name.as_str()
            )
            .to_string(),
            vec![Decepcao, Vitoria],
        );
    }
    // Remontada épica de um não-vencedor.
    if let Some((name, grid, finish, gained)) = &s.biggest_recovery {
        if *gained >= 8 && *finish <= 6 {
            return (
                RaceThesis::Remontada,
                rust_i18n::t!(
                    "narrative.thesis.comeback",
                    name = name.as_str(),
                    grid = grid,
                    finish = finish,
                    gained = gained
                )
                .to_string(),
                vec![Recuperacao, Vitoria],
            );
        }
    }
    // Domínio de quem largou na frente.
    if s.winner_grid >= 1 && s.winner_grid <= 2 {
        return (
            RaceThesis::Dominio,
            rust_i18n::t!(
                "narrative.thesis.dominance",
                name = s.winner_name.as_str(),
                team = s.winner_team.as_str(),
                grid = s.winner_grid
            )
            .to_string(),
            vec![Vitoria, VoltaRapida],
        );
    }
    // Baseline: dia de administração.
    (
        RaceThesis::CorridaLimpa,
        rust_i18n::t!(
            "narrative.thesis.clean_race",
            name = s.winner_name.as_str(),
            team = s.winner_team.as_str()
        )
        .to_string(),
        vec![Vitoria, Podio],
    )
}

/// Renderiza o contexto curado final a partir do resultado + metadados.
pub fn build_race_context(result: &RaceResult, input: &RaceContextInput) -> RaceContext {
    let mut beats = build_beats(result, input.incidents);
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

    let mut header = rust_i18n::t!(
        "narrative.context.header",
        category = input.category_name,
        year = input.year,
        round = input.round,
        track = result.track_name.as_str(),
        laps = result.total_laps,
        weather = result.weather.as_str()
    )
    .to_string();
    if result.total_dnfs >= 2 {
        header.push_str(&rust_i18n::t!("narrative.context.header_dnfs", dnfs = result.total_dnfs));
    }
    // Bandeira amarela: quantas vezes a prova foi neutralizada. Em corrida simulada vem
    // derivada dos incidentes; em corrida importada fica vazia aqui, porque lá a amarela
    // REAL do SDK entra pelos fatos de telemetria (senão a revista contaria duas vezes).
    if !result.caution_segments.is_empty() {
        header.push_str(&rust_i18n::t!(
            "narrative.context.header_cautions",
            n = result.caution_segments.len()
        ));
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
            facts.push_str(
                &rust_i18n::t!("narrative.context.reader_pilot", name = name.as_str()).to_string(),
            );
        }
    }
    // Tese jornalística: elege o ÂNGULO da matéria e hierarquiza os beats em
    // DESTAQUES (que sustentam o ângulo) vs PANO DE FUNDO (cor). Antes era uma lista
    // plana "Fatos" e o servidor tinha que adivinhar a manchete.
    let sig = race_thesis_signals(result);
    let (_thesis, statement, support) = select_race_thesis(&sig);

    facts.push_str(&rust_i18n::t!("narrative.context.axis_label").to_string());
    facts.push_str(&statement);
    facts.push('\n');

    let (apoio, fundo): (Vec<&Beat>, Vec<&Beat>) =
        selected.iter().partition(|b| support.contains(&b.kind));

    if !apoio.is_empty() {
        facts.push_str(&rust_i18n::t!("narrative.context.highlights_label").to_string());
        for beat in &apoio {
            facts.push_str(&format!("- {}\n", beat.text));
        }
    }
    if !fundo.is_empty() {
        facts.push_str(&rust_i18n::t!("narrative.context.background_label").to_string());
        for beat in &fundo {
            facts.push_str(&format!("- {}\n", beat.text));
        }
    }

    if !input.context_facts.is_empty() {
        facts.push_str(&rust_i18n::t!("narrative.context.extra_context_label").to_string());
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

    /// Fatos + tese resolvem nos dois locales, com interpolação (sem `%{...}` cru).
    /// `#[serial]` porque troca o locale global. Parity garante as chaves; isto garante
    /// que os NOMES dos placeholders casam.
    #[test]
    #[serial_test::serial]
    fn fatos_e_tese_resolvem_nos_dois_locales() {
        rust_i18n::set_locale("pt-BR");
        let pt = rust_i18n::t!("narrative.beat.winner", name = "Ana", team = "Alfa", grid = 4, extra = "").to_string();
        assert!(pt.contains("Ana") && pt.contains("Vencedor") && !pt.contains("%{"), "{pt}");
        let pt_t = rust_i18n::t!("narrative.thesis.improbable_win", name = "Ana", team = "Alfa", grid = 8).to_string();
        assert!(pt_t.contains("Ana") && pt_t.contains("P8") && !pt_t.contains("%{"), "{pt_t}");

        rust_i18n::set_locale("en-US");
        let en = rust_i18n::t!("narrative.beat.winner", name = "Ana", team = "Alfa", grid = 4, extra = "").to_string();
        assert!(en.contains("Ana") && en.contains("Winner") && !en.contains("%{"), "{en}");
        assert_ne!(pt, en);

        rust_i18n::set_locale("pt-BR");
    }

    fn inc(
        pilot: &str,
        typ: IncidentType,
        sev: IncidentSeverity,
        is_dnf: bool,
        positions_lost: i32,
    ) -> IncidentResult {
        crate::simulation::incidents::make_incident(
            pilot.to_string(),
            typ,
            sev,
            "Mid",
            positions_lost,
            is_dnf,
            "toque na entrada da curva".to_string(),
            None,
            false,
            None,
            None,
        )
    }

    /// O CORAÇÃO do pedido: a batida do jogador é sempre citada, mas o peso — e com
    /// ele o tom — acompanha o tamanho do impacto.
    #[test]
    fn batida_do_jogador_escala_com_a_gravidade() {
        let leve = inc("p", IncidentType::Collision, IncidentSeverity::Minor, false, 0);
        let forte = inc("p", IncidentType::Collision, IncidentSeverity::Major, false, 2);
        let grave = inc("p", IncidentType::Collision, IncidentSeverity::Critical, false, 5);

        let (wl, wf, wg) = (
            incident_weight(&leve, true),
            incident_weight(&forte, true),
            incident_weight(&grave, true),
        );
        assert!(wl < wf && wf < wg, "gradação: {wl} < {wf} < {wg}");
        // Mesmo o toque leve passa do limiar — ele TEM que ser mencionado.
        assert!(wl >= THRESHOLD, "toque leve do jogador entra: {wl}");
    }

    /// A IA não pode inundar o boletim: susto leve fica fora, batida de verdade entra.
    #[test]
    fn batida_de_ia_so_entra_quando_e_de_verdade() {
        let leve = inc("ai", IncidentType::Collision, IncidentSeverity::Minor, false, 0);
        let forte = inc("ai", IncidentType::Collision, IncidentSeverity::Major, false, 3);
        assert!(incident_weight(&leve, false) < THRESHOLD, "susto leve de IA fica fora");
        assert!(incident_weight(&forte, false) >= THRESHOLD, "batida de IA entra");
    }

    /// O mesmo incidente pesa mais no piloto do leitor do que num rival — é a revista
    /// do jogador, o acidente dele importa mais.
    #[test]
    fn jogador_pesa_mais_que_ia_no_mesmo_incidente() {
        let i = inc("x", IncidentType::Collision, IncidentSeverity::Major, false, 2);
        assert!(incident_weight(&i, true) > incident_weight(&i, false));
    }

    /// "Batida" exclui pane mecânica: o carro quebrar não é alguém bater.
    #[test]
    fn pane_mecanica_nao_conta_como_batida() {
        let mecanico = inc("a", IncidentType::Mechanical, IncidentSeverity::Critical, true, 0);
        let colisao = inc("b", IncidentType::Collision, IncidentSeverity::Minor, true, 0);
        let erro_grave = inc("c", IncidentType::DriverError, IncidentSeverity::Major, true, 0);
        let erro_leve = inc("d", IncidentType::DriverError, IncidentSeverity::Minor, false, 0);
        assert!(!is_crash(&mecanico));
        assert!(is_crash(&colisao));
        assert!(is_crash(&erro_grave));
        assert!(!is_crash(&erro_leve));
    }

    /// Um piloto que se meteu em três confusões rende UMA linha — a pior delas.
    #[test]
    fn cada_piloto_rende_so_o_pior_incidente() {
        let incidents = vec![
            inc("a", IncidentType::Collision, IncidentSeverity::Minor, false, 0),
            inc("a", IncidentType::Collision, IncidentSeverity::Critical, false, 4),
            inc("a", IncidentType::Collision, IncidentSeverity::Major, false, 1),
            inc("b", IncidentType::Collision, IncidentSeverity::Minor, false, 0),
            // DNF não entra aqui — quem abandonou já tem o beat de Abandono.
            inc("c", IncidentType::Collision, IncidentSeverity::Critical, true, 9),
        ];
        let worst = worst_non_dnf_incident_per_pilot(&incidents);
        assert_eq!(worst.len(), 2, "um por piloto, sem os DNFs");
        let a = worst.iter().find(|i| i.pilot_id == "a").unwrap();
        assert_eq!(a.severity, IncidentSeverity::Critical);
        assert!(!worst.iter().any(|i| i.pilot_id == "c"), "DNF fica de fora");
    }

    /// A amarela é consequência de batida que para carro na pista — não de pane, nem
    /// de susto leve. Incidentes no mesmo segmento são a MESMA bandeira.
    #[test]
    fn amarela_so_nasce_de_batida_que_para_carro() {
        use crate::simulation::race::derive_caution_segments;
        use IncidentSeverity::{Critical, Major, Minor};
        use IncidentType::{Collision, Mechanical};

        let caso = |i: IncidentResult| derive_caution_segments(&vec![i]).len();

        // Pane mecânica grave: o carro recolhe pro box, não neutraliza.
        assert_eq!(caso(inc("a", Mechanical, Critical, true, 0)), 0);
        // Susto leve: não neutraliza.
        assert_eq!(caso(inc("a", Collision, Minor, false, 0)), 0);
        // Batida forte: neutraliza mesmo sem abandono.
        assert_eq!(caso(inc("a", Collision, Critical, false, 3)), 1);
        // Batida média só neutraliza se tirou o carro da corrida.
        assert_eq!(caso(inc("a", Collision, Major, false, 2)), 0);
        assert_eq!(caso(inc("a", Collision, Major, true, 0)), 1);
    }

    /// Duas batidas no mesmo segmento = uma bandeira só (seria a mesma amarela).
    #[test]
    fn incidentes_no_mesmo_segmento_sao_uma_amarela_so() {
        use crate::simulation::race::derive_caution_segments;

        let incidents = vec![
            inc("a", IncidentType::Collision, IncidentSeverity::Critical, true, 0),
            inc("b", IncidentType::Collision, IncidentSeverity::Critical, true, 0),
        ];
        assert_eq!(derive_caution_segments(&incidents).len(), 1);
    }

    /// O beat de batida usa o limiar padrão (não o do jogador): quem separa jogador de
    /// IA é o PESO, não o limiar.
    #[test]
    fn beat_de_acidente_usa_o_limiar_padrao() {
        assert!(beat(BeatKind::Acidente, 32.0).passes());
        assert!(!beat(BeatKind::Acidente, 16.0).passes());
    }

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

    fn sig() -> RaceThesisSignals {
        // Base: vencedor de P3, sem caos, sem pole frustrada, sem remontada.
        RaceThesisSignals {
            total_dnfs: 1,
            field_size: 20,
            winner_name: "A. Vega".to_string(),
            winner_team: "Aurora".to_string(),
            winner_grid: 3,
            pole_flopped: None,
            biggest_recovery: None,
        }
    }

    fn thesis_of(s: &RaceThesisSignals) -> RaceThesis {
        select_race_thesis(s).0
    }

    #[test]
    fn caos_quando_muitos_abandonos() {
        let mut s = sig();
        s.total_dnfs = 6; // >= max(4, 20/4=5)
        assert_eq!(thesis_of(&s), RaceThesis::Caos);
    }

    #[test]
    fn caos_vence_ate_a_vitoria_improvavel() {
        let mut s = sig();
        s.total_dnfs = 7;
        s.winner_grid = 9; // seria VitoriaImprovavel, mas o caos é o ângulo
        assert_eq!(thesis_of(&s), RaceThesis::Caos);
    }

    #[test]
    fn vitoria_improvavel_quando_vencedor_veio_de_tras() {
        let mut s = sig();
        s.winner_grid = 8;
        let (t, stmt, _) = select_race_thesis(&s);
        assert_eq!(t, RaceThesis::VitoriaImprovavel);
        assert!(stmt.contains("P8"));
    }

    #[test]
    fn pole_frustrada_quando_pole_afunda() {
        let mut s = sig();
        s.pole_flopped = Some(("R. Silva".to_string(), 9));
        let (t, stmt, _) = select_race_thesis(&s);
        assert_eq!(t, RaceThesis::PoleFrustrada);
        assert!(stmt.contains("R. Silva"));
    }

    #[test]
    fn remontada_quando_recuperacao_epica_de_nao_vencedor() {
        let mut s = sig();
        s.biggest_recovery = Some(("K. Novak".to_string(), 18, 4, 14));
        assert_eq!(thesis_of(&s), RaceThesis::Remontada);
    }

    #[test]
    fn recuperacao_pequena_nao_vira_remontada() {
        let mut s = sig();
        s.biggest_recovery = Some(("K. Novak".to_string(), 8, 5, 3)); // gained 3 < 8
        assert_eq!(thesis_of(&s), RaceThesis::CorridaLimpa);
    }

    #[test]
    fn dominio_quando_vencedor_larga_na_frente() {
        let mut s = sig();
        s.winner_grid = 1;
        assert_eq!(thesis_of(&s), RaceThesis::Dominio);
    }

    #[test]
    fn corrida_limpa_como_piso() {
        assert_eq!(thesis_of(&sig()), RaceThesis::CorridaLimpa);
    }
}
