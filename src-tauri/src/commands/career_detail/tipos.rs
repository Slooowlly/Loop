//! Linhas cruas lidas do banco e agregados internos da ficha do piloto — resultado historico, arquivo de temporada, corrida-a-corrida e campanha especial.

#[derive(Debug, Clone)]
pub(super) struct HistoricalRaceResult {
    pub(super) rodada: i32,
    pub(super) position: i32,
    pub(super) is_dnf: bool,
    pub(super) has_fastest_lap: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ArchivedRecentResults {
    pub(super) results: Vec<HistoricalRaceResult>,
    pub(super) form_context: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct CareerSeasonArchiveRow {
    pub(super) ano: i32,
    pub(super) categoria: String,
    pub(super) posicao_campeonato: Option<i32>,
    pub(super) pontos: f64,
    pub(super) corridas: i32,
    pub(super) vitorias: i32,
    pub(super) podios: i32,
}

#[derive(Debug, Clone)]
pub(super) struct CareerRaceHistoryRow {
    pub(super) race_index: i32,
    pub(super) season_number: i32,
    pub(super) team_id: String,
    pub(super) position: i32,
    pub(super) is_dnf: bool,
}

#[derive(Debug, Clone)]
pub(super) struct SpecialContractRow {
    pub(super) season_number: i32,
    pub(super) year: i32,
    pub(super) category: String,
    pub(super) class_name: Option<String>,
    pub(super) team_id: String,
    pub(super) team_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct SpecialCampaignAggregate {
    pub(super) year: i32,
    pub(super) category: String,
    pub(super) class_name: Option<String>,
    pub(super) team_name: Option<String>,
    pub(super) points: i32,
    pub(super) wins: i32,
    pub(super) podiums: i32,
}

