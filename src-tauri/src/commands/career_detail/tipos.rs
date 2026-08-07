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
    pub(super) season_number: i32,
    pub(super) ano: i32,
    pub(super) categoria: String,
    /// Classe dentro da categoria multiclasse (`gt3`/`lmp2` na Endurance,
    /// `mazda`/`toyota`/`bmw` na Production). E o que transforma "endurance" na
    /// divisao de verdade que ele disputou naquele ano.
    pub(super) classe: Option<String>,
    pub(super) posicao_campeonato: Option<i32>,
    pub(super) pontos: f64,
    pub(super) corridas: i32,
    pub(super) vitorias: i32,
    pub(super) podios: i32,
    pub(super) poles: i32,
    /// Titulos declarados no snapshot daquela temporada. `None` = snapshot antigo
    /// sem o campo; quem decide nesse caso e a posicao no campeonato.
    pub(super) titulos: Option<i32>,
    /// Equipe com que ele correu a temporada, do snapshot. E o que permite dizer
    /// COM QUEM o titulo foi conquistado — o arquivo do piloto nao tem coluna de
    /// equipe, so o snapshot.
    pub(super) equipe_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct CareerRaceHistoryRow {
    pub(super) race_index: i32,
    pub(super) season_number: i32,
    pub(super) team_id: String,
    pub(super) position: i32,
    pub(super) is_dnf: bool,
    /// Posicao de largada. `0` quando a corrida nao registrou grid — o sabado
    /// nao pode contar uma pole que ninguem sabe se existiu.
    pub(super) grid_position: i32,
    pub(super) has_fastest_lap: bool,
    /// Identidade da corrida no calendario, para o detalhe que abre no hover
    /// responder QUANDO e ONDE em vez de so "74a corrida".
    pub(super) race_id: String,
    pub(super) ano: i32,
    pub(super) rodada: i32,
    pub(super) pista: Option<String>,
    pub(super) categoria: Option<String>,
    pub(super) data: Option<String>,
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
