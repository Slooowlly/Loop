//! Buscas pontuais no resultado da corrida, compartilhadas pelos beats, pela
//! tese e pelo contexto.

use crate::simulation::race::RaceDriverResult;

pub(crate) fn find<'a>(results: &'a [RaceDriverResult], id: &str) -> Option<&'a RaceDriverResult> {
    results.iter().find(|d| d.pilot_id == id)
}

pub(crate) fn dnf_reason_of(d: &RaceDriverResult) -> String {
    d.notable_incident
        .clone()
        .or_else(|| d.dnf_reason.clone())
        .unwrap_or_else(|| rust_i18n::t!("narrative.beat.dnf_fallback").to_string())
}
