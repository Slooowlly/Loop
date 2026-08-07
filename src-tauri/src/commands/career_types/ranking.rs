//! DTOs dos rankings globais: pilotos de todos os tempos e histórico de equipes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverRankingPayload {
    pub selected_driver_id: Option<String>,
    pub player_driver: Option<GlobalDriverRankingRow>,
    pub rows: Vec<GlobalDriverRankingRow>,
    pub leaders: GlobalDriverRankingLeaders,
}

/// A posição de UM piloto no ranking mundial, para a ficha.
///
/// A tabela global manda 200+ linhas com títulos por categoria aninhados dentro;
/// a ficha precisa de quatro números. O índice sai da MESMA régua e da mesma
/// ordenação do painel completo — se a ficha calculasse o seu próprio, os dois
/// números discordariam na primeira vez que a régua mudasse.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverWorldRank {
    pub indice: f64,
    pub posicao: i32,
    /// Quantos pilotos entram na conta — o denominador de "12º de 240".
    pub total: i32,
    /// Quantas posições ele subiu (+) ou caiu (-) desde a última corrida.
    pub delta: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverRankingLeaders {
    pub historical_index_driver_id: Option<String>,
    pub wins_driver_id: Option<String>,
    pub titles_driver_id: Option<String>,
    pub injuries_driver_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverRankingRow {
    pub id: String,
    pub nome: String,
    pub nacionalidade: String,
    pub idade: i32,
    pub status: String,
    pub status_tone: String,
    pub is_jogador: bool,
    /// Piloto favoritado pelo jogador (watchlist). Comanda a estrela inline + filtro.
    #[serde(default)]
    pub is_favorito: bool,
    pub is_lesionado: bool,
    pub lesao_ativa_tipo: Option<String>,
    pub equipe_nome: Option<String>,
    pub equipe_cor_primaria: Option<String>,
    pub categoria_atual: Option<String>,
    pub categorias_historicas: Vec<String>,
    pub salario_anual: Option<f64>,
    pub ano_inicio_carreira: Option<i32>,
    pub anos_carreira: Option<i32>,
    pub temporada_aposentadoria: Option<String>,
    pub anos_aposentado: Option<i32>,
    pub historical_index: f64,
    pub historical_rank: i32,
    pub historical_rank_delta: Option<i32>,
    /// Estrelato: fama (`midia`) e carisma atuais, 0–100. `fama_delta` = quanto a
    /// fama subiu/caiu desde o fim da temporada passada (snapshot arquivado); `None`
    /// quando não há histórico para comparar (ex.: 1ª temporada). Alimenta a seta ▲/▼.
    #[serde(default)]
    pub fama: i32,
    #[serde(default)]
    pub carisma: i32,
    #[serde(default)]
    pub fama_delta: Option<i32>,
    pub wins_rank: i32,
    pub titles_rank: i32,
    pub podiums_rank: i32,
    pub injuries_rank: i32,
    pub corridas: i32,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
    /// Pódios que NÃO foram vitória, quebrados por posição. Vêm dos resultados reais
    /// (`race_results`), então cobrem a carreira inteira jogada; pilotos históricos
    /// pré-gerados (sem `race_results`) ficam em 0 — o front trata como "sem detalhe".
    /// `segundos + terceiros + vitorias` pode ser < `podios` nesse caso.
    #[serde(default)]
    pub segundos: i32,
    #[serde(default)]
    pub terceiros: i32,
    pub poles: i32,
    pub titulos: i32,
    #[serde(default)]
    pub titulos_por_categoria: Vec<GlobalDriverTitleCategorySummary>,
    pub dnfs: i32,
    pub lesoes: i32,
    pub lesoes_leves: i32,
    pub lesoes_moderadas: i32,
    pub lesoes_graves: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverTitleCategorySummary {
    pub categoria: String,
    #[serde(default)]
    pub classe: Option<String>,
    pub titulos: i32,
    #[serde(default)]
    pub anos: Vec<i32>,
    /// Equipe com a qual o piloto conquistou cada título, na mesma ordem de `anos`.
    /// O nome é resolvido pela identidade atual do `team_id` (o nome histórico não é
    /// arquivado), e `equipe`/`equipe_cor` ficam nulos quando o time não é resolvível.
    #[serde(default)]
    pub anos_equipes: Vec<GlobalDriverTitleYearTeam>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDriverTitleYearTeam {
    pub ano: i32,
    #[serde(default)]
    pub equipe: Option<String>,
    #[serde(default)]
    pub equipe_cor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryPayload {
    pub selected_family: String,
    pub min_year: i32,
    pub max_year: i32,
    pub window_start: i32,
    pub window_end: i32,
    pub window_size: i32,
    /// Year of the currently-active season in the career.  Used by the frontend
    /// to set the axis end (current_year + 5) so the hatch zone extends a few
    /// years into the future.  Falls back to `max_year` when no active season
    /// exists (e.g. pre-game or historical draft).
    pub current_year: i32,
    /// True when `current_year` is a season that STARTED but has not finished —
    /// its column is provisional (partial points, no decided title). The frontend
    /// uses this to draw the live column differently from the archived ones.
    pub in_progress: bool,
    /// Last season already archived. While `in_progress`, this is the year that
    /// still owns the crown — the running season has no champion yet.
    pub last_completed_year: i32,
    pub families: Vec<GlobalTeamHistoryFamily>,
    pub bands: Vec<GlobalTeamHistoryBand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryFamily {
    pub id: String,
    pub label: String,
    pub bands: Vec<GlobalTeamHistoryFamilyBand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryFamilyBand {
    pub key: String,
    pub label: String,
    pub category: String,
    #[serde(default)]
    pub class_name: Option<String>,
    pub starts_year: i32,
    pub is_special: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryBand {
    pub key: String,
    pub label: String,
    pub category: String,
    #[serde(default)]
    pub class_name: Option<String>,
    pub starts_year: i32,
    pub is_special: bool,
    pub rows: Vec<GlobalTeamHistoryTeamRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamTitleCount {
    /// Matches a key in `TeamHistoryBandDef` (e.g. "mazda_rookie", "production_mazda").
    /// Stable identifier — the frontend maps this to a band-specific trophy image.
    pub band_key: String,
    pub band_label: String,
    pub count: i32,
}

/// Salão dos campeões de uma faixa: um título de construtores por linha, do mais
/// recente para o mais antigo, mais o agregado por equipe que abre o painel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandChampionsPayload {
    pub band_key: String,
    pub band_label: String,
    /// Equipes ordenadas por número de títulos (desempate: título mais recente).
    pub dynasties: Vec<BandDynasty>,
    pub seasons: Vec<BandChampionSeason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandDynasty {
    pub team_id: String,
    pub nome: String,
    pub cor_primaria: String,
    pub titles: i32,
    pub last_year: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandChampionSeason {
    pub year: i32,
    pub team_id: String,
    pub nome: String,
    pub cor_primaria: String,
    pub wins: i32,
    /// A dupla da equipe naquela temporada — quem de fato ganhou o título COM ela.
    /// Vazia quando o arquivo daquele ano não registrou pilotos.
    pub drivers: Vec<BandChampionDriver>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandChampionDriver {
    pub driver_id: String,
    pub nome: String,
    /// True quando esse piloto também foi o campeão de pilotos da categoria naquele
    /// ano. Os dois títulos são independentes: uma equipe pode ser campeã de
    /// construtores sem ter o campeão de pilotos, e é isso que a marca distingue.
    pub is_season_champion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryTeamRow {
    pub team_id: String,
    pub nome: String,
    pub nome_curto: String,
    pub cor_primaria: String,
    pub cor_secundaria: String,
    pub base_position: i32,
    /// All-time constructor titles for this team within the displayed family, grouped by
    /// band (level). Ordered from lowest band index to highest (e.g. Rookie → Cup →
    /// Production). Empty when the team has never won a championship in this family.
    pub titles: Vec<TeamTitleCount>,
    /// True when this team holds the championship of this band in the last year of the
    /// current window (`window_end`).
    pub is_reigning_champion: bool,
    pub points: Vec<GlobalTeamHistoryPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTeamHistoryPoint {
    pub year: i32,
    pub slot: String,
    pub position: i32,
    pub points: i32,
    pub wins: i32,
    pub titles: i32,
}
