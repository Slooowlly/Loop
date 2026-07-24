//! Dossiê por piloto: os sinais crus do grid já traduzidos em tokens qualitativos.

use super::*;

// ── Dossiê por piloto (já em linguagem qualitativa) ──────────────────────────────

pub(super) struct Dossier {
    pub(super) id: String,
    pub(super) nome: String,
    pub(super) equipe: Option<String>,
    /// Score da percepção PÚBLICA — ordena os favoritos (NÃO é o skill).
    pub(super) perception: f64,
    /// Rótulo do currículo (títulos / vitórias / pódio / ainda sem).
    pub(super) curriculo: String,
    /// Rótulo de experiência (estreante … veterano).
    pub(super) experiencia: String,
    /// 0–2 traços de estilo (observáveis em pista).
    pub(super) tracos: Vec<String>,
    /// Ganchos extras (nome de público, contratação mais cara…).
    pub(super) ganchos: Vec<String>,
    /// Sinais crus guardados só para o fallback determinístico decidir frases.
    pub(super) tem_vitoria: bool,
    pub(super) tem_podio: bool,
    pub(super) estreante: bool,
}

impl Dossier {
    /// Linha do bundle: `nome | equipe | percepção | currículo | traço | gancho`.
    pub(super) fn fact_line(&self, perc_label: &str) -> String {
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
pub(super) fn style_traits(d: &Driver, agg: &[f64], smo: &[f64], conf: &[f64]) -> Vec<String> {
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

pub(super) fn curriculo_token(d: &Driver) -> String {
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

pub(super) fn experiencia_token(d: &Driver) -> String {
    match d.stats_carreira.corridas {
        0 => tk("token.exp_debut"),
        1..=15 => tk("token.exp_rising"),
        16..=60 => tk("token.exp_established"),
        _ => tk("token.exp_veteran"),
    }
}

/// Rótulo de percepção conforme a posição na fila pública + o que sustenta o nome.
pub(super) fn perception_label(rank: usize, d: &Dossier, is_market_top: bool) -> String {
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
