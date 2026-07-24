//! Renderização do CONTEXTO CURADO: o texto de fatos (eixo + destaques + pano de
//! fundo) que será enviado ao servidor → Gemini.

use super::beats::{build_beats, select, Beat, BeatKind};
use super::tese::{race_thesis_signals, select_race_thesis};
use crate::simulation::incidents::IncidentResult;
use crate::simulation::race::RaceResult;

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
