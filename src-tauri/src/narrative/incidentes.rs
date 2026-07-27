//! Curadoria de incidentes: qual batida vira notícia, quanto ela pesa e como o
//! tamanho dela é dito em palavras. A proporcionalidade mora aqui — não na IA.

use super::consulta::find;
use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};
use crate::simulation::race::RaceDriverResult;

/// Ordem de gravidade, para escolher o PIOR incidente de cada piloto.
pub(crate) fn severity_rank(s: IncidentSeverity) -> u8 {
    match s {
        IncidentSeverity::Minor => 0,
        IncidentSeverity::Major => 1,
        IncidentSeverity::Critical => 2,
    }
}

/// Rótulo de ESCALA do incidente. Carrega, no próprio texto, a instrução de
/// proporcionalidade: um toque leve tem que ser narrado como toque leve. É isto que
/// impede a IA de transformar um encostão em tragédia.
pub(crate) fn scale_label(sev: IncidentSeverity) -> String {
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
pub(crate) fn incident_weight(inc: &IncidentResult, is_player: bool) -> f64 {
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
pub(crate) fn contact_link(rows: &[RaceDriverResult], inc: &IncidentResult) -> String {
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
pub(crate) fn worst_non_dnf_incident_per_pilot(
    incidents: &[IncidentResult],
) -> Vec<&IncidentResult> {
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
