//! DTOs do piloto: resumo na carreira e o dossiê completo (blocos do drawer).

use serde::{Deserialize, Serialize};

use crate::commands::race_history::RoundResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverSummary {
    pub id: String,
    pub nome: String,
    pub nacionalidade: String,
    pub idade: i32,
    pub skill: u8,
    /// Fama/estrelato (`midia`, 0–100). Só para a camada de apresentação (a prévia
    /// pré-corrida comenta o estrelato do jogador); `#[serde(default)]` p/ saves antigos.
    #[serde(default)]
    pub midia: u8,
    #[serde(default)]
    pub categoria_especial_ativa: Option<String>,
    pub equipe_id: Option<String>,
    pub equipe_nome: Option<String>,
    pub equipe_nome_curto: Option<String>,
    pub equipe_cor: String,
    #[serde(default)]
    pub classe: Option<String>,
    pub is_jogador: bool,
    #[serde(default)]
    pub is_estreante: bool,
    #[serde(default)]
    pub is_estreante_da_vida: bool,
    #[serde(default)]
    pub lesao_ativa_tipo: Option<String>,
    /// Piloto que encerrou a carreira (aposentado). Fica congelado na classificação da
    /// temporada com os pontos que somou, mas não volta à pista. Ganha selo na UI.
    #[serde(default)]
    pub is_aposentado: bool,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
    pub posicao_campeonato: i32,
    pub results: Vec<Option<RoundResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverDetail {
    pub id: String,
    pub nome: String,
    pub nacionalidade: String,
    pub idade: i32,
    pub genero: String,
    pub is_jogador: bool,
    /// Piloto favoritado pelo jogador (watchlist). Comanda a estrela no dossiê.
    #[serde(default)]
    pub is_favorito: bool,
    pub status: String,
    pub equipe_id: Option<String>,
    pub equipe_nome: Option<String>,
    pub equipe_cor_primaria: Option<String>,
    pub equipe_cor_secundaria: Option<String>,
    pub papel: Option<String>,
    pub personalidade_primaria: Option<PersonalityInfo>,
    pub personalidade_secundaria: Option<PersonalityInfo>,
    pub motivacao: u8,
    pub tags: Vec<TagInfo>,
    pub stats_temporada: StatsBlock,
    pub stats_carreira: StatsBlock,
    pub contrato: Option<ContractDetail>,
    pub perfil: DriverProfileBlock,
    pub competitivo: DriverCompetitiveBlock,
    #[serde(default)]
    pub leitura_tecnica: DriverTechnicalReadBlock,
    #[serde(default)]
    pub estrelato: DriverStardomBlock,
    pub performance: DriverPerformanceBlock,
    pub forma: DriverFormBlock,
    #[serde(default)]
    pub resumo_atual: DriverCurrentSummaryBlock,
    #[serde(default)]
    pub leitura_desempenho: DriverPerformanceReadBlock,
    pub trajetoria: DriverCareerPathBlock,
    #[serde(default)]
    pub rankings_carreira: DriverCareerRankBlock,
    #[serde(default)]
    pub rivais: DriverRivalsBlock,
    pub contrato_mercado: DriverContractMarketBlock,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relacionamentos: Option<DriverRelationshipsBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reputacao: Option<DriverReputationBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saude: Option<DriverHealthBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityInfo {
    pub tipo: String,
    pub emoji: String,
    pub descricao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInfo {
    pub attribute_name: String,
    pub tag_text: String,
    pub level: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsBlock {
    pub corridas: i32,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
    pub poles: i32,
    pub melhor_resultado: i32,
    pub dnfs: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDetail {
    pub equipe_nome: String,
    pub papel: String,
    pub salario_anual: f64,
    pub temporada_inicio: i32,
    pub temporada_fim: i32,
    pub ano_inicio: i32,
    pub ano_fim: i32,
    pub anos_restantes: i32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverProfileBlock {
    pub nome: String,
    pub bandeira: String,
    pub nacionalidade: String,
    pub idade: i32,
    pub genero: String,
    pub status: String,
    pub is_jogador: bool,
    pub equipe_nome: Option<String>,
    pub papel: Option<String>,
    pub licenca: DriverLicenseInfo,
    pub badges: Vec<DriverBadge>,
    pub equipe_cor_primaria: Option<String>,
    pub equipe_cor_secundaria: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverLicenseInfo {
    pub nivel: String,
    pub sigla: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverBadge {
    pub label: String,
    pub variant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverCompetitiveBlock {
    pub personalidade_primaria: Option<PersonalityInfo>,
    pub personalidade_secundaria: Option<PersonalityInfo>,
    pub motivacao: u8,
    pub qualidades: Vec<TagInfo>,
    pub defeitos: Vec<TagInfo>,
    pub neutro: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverTechnicalReadBlock {
    pub itens: Vec<DriverTechnicalReadItem>,
}

/// Bloco de ESTRELATO — a segunda moeda ("Nasce um astro") na ficha do piloto.
/// `fama` é o estoque público (atributo `midia`, 0–100), `carisma` o traço estável
/// (0–100) que modula quão rápido a fama sobe/desce. Os níveis são rótulos legíveis
/// e `tom` mapeia para as cores de tom já usadas no dossiê (neutral/info/success/elite…).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverStardomBlock {
    pub fama: u8,
    pub carisma: u8,
    pub nivel_fama: String,
    pub tom_fama: String,
    pub nivel_carisma: String,
    pub tom_carisma: String,
    /// Leitura de uma linha combinando fama × carisma (a dinâmica, não os números).
    pub resumo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverTechnicalReadItem {
    pub chave: String,
    pub label: String,
    pub nivel: String,
    pub tom: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverPerformanceBlock {
    pub temporada: PerformanceStatsBlock,
    pub carreira: PerformanceStatsBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStatsBlock {
    pub vitorias: i32,
    pub podios: i32,
    pub top_10: Option<i32>,
    pub fora_top_10: Option<i32>,
    pub poles: i32,
    pub voltas_rapidas: Option<i32>,
    pub hat_tricks: Option<i32>,
    pub corridas: i32,
    pub dnfs: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverFormBlock {
    pub ultimas_10: Vec<FormResultEntry>,
    pub ultimas_5: Vec<FormResultEntry>,
    pub media_chegada: Option<f64>,
    pub tendencia: String,
    pub momento: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contexto: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCurrentSummaryBlock {
    pub veredito: String,
    pub tom: String,
    pub posicao_campeonato: Option<i32>,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
    pub top_10: Option<i32>,
    pub media_recente: Option<f64>,
    pub tendencia: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverPerformanceReadBlock {
    pub esperado_posicao: Option<i32>,
    pub entregue_posicao: Option<i32>,
    pub delta_posicao: Option<i32>,
    pub car_performance: Option<f64>,
    pub companheiro_nome: Option<String>,
    pub companheiro_pontos: Option<i32>,
    pub piloto_pontos: i32,
    pub leitura: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormResultEntry {
    pub rodada: i32,
    pub chegada: Option<i32>,
    pub dnf: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverCareerPathBlock {
    pub ano_estreia: i32,
    pub equipe_estreia: Option<String>,
    pub categoria_atual: Option<String>,
    #[serde(default)]
    pub categorias_timeline: Vec<DriverCareerCategoryStint>,
    pub temporadas_na_categoria: i32,
    pub corridas_na_categoria: i32,
    pub titulos: i32,
    pub foi_campeao: bool,
    #[serde(default)]
    pub historico: DriverCareerHistoryBlock,
    pub marcos: Vec<CareerMilestone>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerCategoryStint {
    pub categoria: String,
    pub ano_inicio: i32,
    pub ano_fim: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerHistoryBlock {
    pub presenca: DriverCareerPresenceBlock,
    pub primeiros_marcos: DriverCareerFirstMarksBlock,
    pub auge: DriverCareerPeakBlock,
    pub mobilidade: DriverCareerMobilityBlock,
    pub lesoes: DriverCareerInjuryBlock,
    pub eventos_especiais: DriverCareerSpecialEventsBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerPresenceBlock {
    pub tempo_carreira: i32,
    pub temporadas_disputadas: i32,
    pub anos_desempregado: i32,
    pub periodos_desempregado: Vec<String>,
    pub corridas: i32,
    pub categorias_disputadas: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerFirstMarksBlock {
    pub primeiro_podio_corrida: Option<i32>,
    pub primeira_vitoria_corrida: Option<i32>,
    pub primeiro_dnf_corrida: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerPeakBlock {
    pub melhor_temporada: Option<DriverBestSeasonBlock>,
    pub maior_sequencia_vitorias: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverBestSeasonBlock {
    pub ano: i32,
    pub categoria: String,
    pub posicao_campeonato: Option<i32>,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerMobilityBlock {
    pub promocoes: i32,
    pub rebaixamentos: i32,
    pub equipes_defendidas: i32,
    pub tempo_medio_por_equipe: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerInjuryBlock {
    pub leves: i32,
    pub moderadas: i32,
    pub graves: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerSpecialEventsBlock {
    pub participacoes: i32,
    pub convocacoes: i32,
    pub vitorias: i32,
    pub podios: i32,
    #[serde(default)]
    pub rankings: DriverSpecialEventRankBlock,
    pub melhor_campanha: Option<DriverSpecialCampaignBlock>,
    pub ultimo_evento: Option<DriverSpecialEventEntry>,
    pub timeline: Vec<DriverSpecialEventEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverSpecialEventRankBlock {
    pub participacoes: Option<i32>,
    pub convocacoes: Option<i32>,
    pub vitorias: Option<i32>,
    pub podios: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverSpecialCampaignBlock {
    pub ano: i32,
    pub categoria: String,
    pub classe: Option<String>,
    pub equipe: Option<String>,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverSpecialEventEntry {
    pub ano: i32,
    pub categoria: String,
    pub classe: Option<String>,
    pub equipe: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerRankBlock {
    pub corridas: Option<i32>,
    pub vitorias: Option<i32>,
    pub podios: Option<i32>,
    pub titulos: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CareerMilestone {
    pub tipo: String,
    pub titulo: String,
    pub descricao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverContractMarketBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrato: Option<ContractDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mercado: Option<DriverMarketBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverMarketBlock {
    pub valor_mercado: Option<f64>,
    pub salario_estimado: Option<f64>,
    pub chance_transferencia: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverRelationshipsBlock {
    pub rival_principal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverRivalsBlock {
    pub itens: Vec<DriverRivalInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverRivalInfo {
    pub driver_id: String,
    pub nome: String,
    pub tipo: String,
    pub intensidade: u8,
    pub intensidade_historica: u8,
    pub atividade_recente: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverReputationBlock {
    pub popularidade: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverHealthBlock {
    pub saude_geral: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lesao_ativa: Option<DriverActiveInjuryBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverActiveInjuryBlock {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nome: Option<String>,
    pub tipo: String,
    pub corrida_ocorrida_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrida_ocorrida_rotulo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrida_ocorrida_rodada: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrida_ocorrida_pista: Option<String>,
    pub corridas_total: i32,
    pub corridas_restantes: i32,
}
