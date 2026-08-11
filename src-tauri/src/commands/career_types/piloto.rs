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
    /// Gravidade da lesão ativa. Vai no fio como chave estável ("light"/"moderate"/"severe"/
    /// "critical") — a grafia do banco fica no banco.
    #[serde(default)]
    pub lesao_ativa_tipo: Option<crate::models::enums::InjuryType>,
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
    #[serde(default)]
    pub arco: DriverCareerArcBlock,
    pub performance: DriverPerformanceBlock,
    pub forma: DriverFormBlock,
    #[serde(default)]
    pub resumo_atual: DriverCurrentSummaryBlock,
    #[serde(default)]
    pub leitura_desempenho: DriverPerformanceReadBlock,
    pub trajetoria: DriverCareerPathBlock,
    #[serde(default)]
    pub rankings_carreira: DriverCareerRankBlock,
    /// As MESMAS posicoes, recontadas so entre os pilotos do grid atual.
    ///
    /// "570º de 610" responde onde ele esta na historia do mundo e nao responde
    /// a pergunta que o jogador faz toda semana: onde ele esta entre os caras
    /// contra quem ele corre no domingo. `None` para quem nao tem grid — sem
    /// contrato ou aposentado —, e a ficha esconde o seletor em vez de comparar
    /// com um grid a que ele nao pertence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rankings_grid: Option<DriverCareerRankBlock>,
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
    /// Em que parte do fim de semana o eixo se manifesta: `volta_seca`, `corrida`
    /// ou `condicoes`. Doze eixos numa lista só viram um paredão de rótulos.
    #[serde(default)]
    pub grupo: String,
    pub label: String,
    pub nivel: String,
    pub tom: String,
    /// Onde o eixo cai na régua de 0–100 — o MESMO número que escolheu o `nivel`.
    /// A tela precisa dele porque rótulo empilhado não tem magnitude: "Forte" e
    /// "Muito forte" só se separam lendo, e ler doze vezes era o que fazia deste
    /// bloco um paredão.
    #[serde(default)]
    pub escala: u8,
    /// Eixo de ESTILO (agressividade, suavidade): a régua POSICIONA, não pontua.
    /// A tela desenha marcador em vez de barra cheia — barra cheia diria "mais é
    /// melhor", que é exatamente o julgamento que estes dois não fazem.
    #[serde(default)]
    pub estilo: bool,
    /// Os dois polos de um eixo de estilo ("Calculista" ↔ "Agressivo"). O eixo É
    /// o par: não há nota, há um lado para o qual ele pende. Num eixo de
    /// qualidade não existem — lá a cor já diz para que lado é bom.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polo_min: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polo_max: Option<String>,
    /// Mediana do GRID neste eixo — os pilotos com assento na categoria (e, em
    /// multiclasse, na classe), não todo mundo com a ficha lá. "Instável" só quer
    /// dizer alguma coisa contra alguém: 45 de ritmo na F4 e 45 na GT3 descrevem
    /// dois pilotos completamente diferentes, e a ficha nunca dizia qual.
    /// `None` quando não há grid para comparar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referencia: Option<u8>,
    /// Quanto o eixo andou desde o fim da temporada passada, em pontos da régua.
    /// `None` quando não há temporada arquivada — ou quando não mudou, que é o
    /// caso da maioria dos eixos e não merece um "+0" ocupando espaço.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<i8>,
}

/// ARCO — onde o piloto está na própria curva.
///
/// A ficha respondia "quão bom ele é" por todos os lados e nunca "ainda tem teto?",
/// que é a pergunta de quem contrata. `margem` é a distância do talento atual até o
/// teto pessoal (`potencial`), `desenvolvimento` a velocidade com que ele anda essa
/// distância, e `experiencia` o que já foi rodado. O `resumo` combina os três com a
/// idade numa linha — subindo, no platô ou descendo.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerArcBlock {
    pub idade: i32,
    pub fase: String,
    /// Chave da fase (`formacao`…`crepusculo`). `fase` já vem traduzida, e a tela
    /// desenha as cinco em sequência para situar a atual — "Em ascensão" não diz
    /// sozinho que ainda faltam três.
    #[serde(default)]
    pub fase_chave: String,
    pub tom_fase: String,
    pub nivel_experiencia: String,
    pub nivel_desenvolvimento: String,
    /// Quanto falta para o teto pessoal, em rótulo. `None` quando o potencial
    /// ainda não foi derivado (jogador e saves antigos guardam 0.0 ali) — sem
    /// isso a ficha diria "chegou ao teto" para quem nunca teve um teto medido.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nivel_margem: Option<String>,
    pub resumo: String,
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
    /// A temporada anterior INTEIRA e a atual ate aqui, uma entrada por
    /// temporada, em ordem cronologica.
    ///
    /// Os dois campos acima recortam as ultimas N corridas e perdem a fronteira
    /// entre um ano e outro — cinco quadradinhos que tanto podem ser o fim de uma
    /// temporada quanto o comeco da seguinte. Aqui a divisao vem explicita, e a
    /// ficha desenha o calendario passado completo com a temporada em curso
    /// separada dele.
    #[serde(default)]
    pub temporadas: Vec<FormSeasonBlock>,
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
    /// Pontos que faltam para o lider (0 para quem lidera). `None` antes da
    /// primeira largada, quando a tabela inteira esta zerada e a distancia nao
    /// significa nada. Ver `ChampionshipContext`.
    #[serde(default)]
    pub gap_lider: Option<i32>,
    /// Vantagem sobre o piloto imediatamente atras. `None` para o lanterna.
    #[serde(default)]
    pub gap_proximo: Option<i32>,
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
    /// Posicao do companheiro no campeonato da categoria, pela mesma ordenacao
    /// que da a do proprio piloto. Pontos sozinhos nao dizem se o duelo interno
    /// acontece na briga do titulo ou no fundo do grid.
    #[serde(default)]
    pub companheiro_posicao: Option<i32>,
    #[serde(default)]
    pub companheiro_nacionalidade: Option<String>,
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
pub struct FormSeasonBlock {
    pub season_number: i32,
    pub ano: i32,
    /// Se esta e a temporada em curso — o que separa "ja aconteceu" de "esta
    /// acontecendo" na faixa de forma.
    pub atual: bool,
    pub resultados: Vec<FormResultEntry>,
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
    /// Um titulo por entrada, do mais recente para o mais antigo: em que ano,
    /// em que categoria e COM QUAL equipe. Sai do arquivo de temporadas, entao
    /// piloto historico pre-gerado (que tem o total de titulos mas nao tem
    /// arquivo) vem com a lista vazia — a ficha mostra o total e omite a linha.
    #[serde(default)]
    pub titulos_detalhe: Vec<DriverTitleEntry>,
    #[serde(default)]
    pub historico: DriverCareerHistoryBlock,
    pub marcos: Vec<CareerMilestone>,
    /// A carreira em POSICAO DE CAMPEONATO, uma temporada por ponto. Vazia para
    /// quem nunca fechou uma temporada — e a ficha nao desenha o grafico.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curva_campeonato: Vec<DriverChampionshipCurvePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverTitleEntry {
    pub ano: i32,
    pub categoria: String,
    /// Nome e cor resolvidos pela identidade ATUAL do `team_id` — o nome
    /// historico da equipe naquele ano nao e arquivado. `None` quando o time nao
    /// existe mais no banco (fechou, foi renomeado para outro id).
    pub equipe: Option<String>,
    pub equipe_cor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerCategoryStint {
    pub categoria: String,
    pub ano_inicio: i32,
    pub ano_fim: i32,
}

/// Uma temporada da curva de campeonato: onde ele TERMINOU naquele ano.
///
/// Espelha a curva de mercado no formato — mesma unidade de tempo (uma
/// temporada por ponto), mesmos campos de categoria e equipe — porque as duas
/// sao desenhadas pela MESMA moldura no frontend: colunas de categoria ao
/// fundo, fita de equipes no rodape, reguas nas trocas de casa. O que muda e a
/// serie por cima.
///
/// Nao ha ponto de FUTURO aqui, ao contrario da curva de mercado: salario de
/// contrato assinado e fato antes da temporada acontecer, posicao de campeonato
/// nao e.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverChampionshipCurvePoint {
    pub season_number: i32,
    pub ano: i32,
    /// Chave de divisao (`gt3`, `endurance:lmp2`), a mesma da linha do tempo de
    /// categorias. Vazia na temporada em que ele nao correu por ninguem.
    pub categoria: String,
    pub equipe_nome: Option<String>,
    pub equipe_cor: Option<String>,
    /// Onde fechou o campeonato. `None` no ano fora do grid e em temporada
    /// arquivada sem classificacao (piloto que nao pontuou em campeonato algum).
    pub posicao: Option<i32>,
    /// Quantos pilotos disputavam AQUELE campeonato. E o denominador que separa
    /// um P8 entre doze de um P8 entre trinta, e vem arquivado junto com a
    /// posicao (`total_pilotos` no snapshot da temporada). `None` em save antigo,
    /// cujo snapshot nao guardava o campo — e ai o chao do grid simplesmente se
    /// parte naquele ano em vez de inventar um numero.
    pub grid: Option<i32>,
    /// Onde o CARRO daquele ano deveria ter terminado, na mesma unidade da
    /// posicao acima. E a linha de referencia do grafico: a distancia entre ela
    /// e o resultado e o que o piloto colocou (ou nao) de si.
    ///
    /// Sai da classificacao de construtores arquivada, pela MESMA regra de
    /// [`super::super::career_detail`] usa para a temporada corrente
    /// (`expected_position_from_grid`): assentos das equipes que terminaram na
    /// frente, mais o meio da faixa de assentos da propria equipe. O que muda e
    /// a fonte do ranking — carro efetivo hoje, resultado do construtor no
    /// passado —, porque o pacote tecnico de uma temporada encerrada nao e
    /// arquivado em lugar nenhum.
    ///
    /// `None` quando o ano nao tem arquivo de construtores (save antigo, ano
    /// fora do grid): a linha se parte ali em vez de inventar referencia.
    #[serde(default)]
    pub esperado: Option<i32>,
    /// A categoria daquele ano era monomarca — todo mundo no mesmo carro.
    ///
    /// A expectativa continua existindo (o construtor tem classificacao, e a
    /// equipe ainda importa por gente e por dinheiro), mas ela nao esta mais
    /// medindo MAQUINA: num grid de MX-5 identicos, "o que o carro dava" e uma
    /// referencia bem mais fraca do que na GT3. A tela desenha essa ressalva
    /// como linha tracejada no trecho monomarca, em vez de esconder.
    #[serde(default)]
    pub monomarca: bool,
    pub pontos: Option<f64>,
    pub vitorias: i32,
    pub podios: i32,
    pub corridas: i32,
    /// A temporada valeu titulo, pela MESMA regra do ranking mundial
    /// (`valid_archived_title_count`): posicao 1 sozinha nao basta, tem que ter
    /// havido campeonato disputado.
    pub titulo: bool,
    /// A temporada em curso — ainda nao arquivada, montada com a classificacao
    /// de hoje. E parcial por natureza, e a tela precisa saber disso.
    pub atual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerHistoryBlock {
    pub presenca: DriverCareerPresenceBlock,
    pub primeiros_marcos: DriverCareerFirstMarksBlock,
    pub auge: DriverCareerPeakBlock,
    #[serde(default)]
    pub queda: DriverCareerDroughtBlock,
    #[serde(default)]
    pub confiabilidade: DriverCareerReliabilityBlock,
    #[serde(default)]
    pub sabado: DriverCareerQualifyingBlock,
    #[serde(default)]
    pub duelos: DriverCareerTeammateBlock,
    #[serde(default)]
    pub referencias: DriverCareerBenchmarkBlock,
    /// O detalhe de cada numero do dossie, indexado pela linha que o abre
    /// (`equipes`, `promocoes`, `poles`, `lesoes`...). Um mapa em vez de um campo
    /// por linha: sao vinte detalhes, e a ficha so precisa saber procurar pela
    /// chave da linha que o mouse tocou.
    #[serde(default)]
    pub detalhes: std::collections::HashMap<String, Vec<DriverCareerDetailEntry>>,
    /// Onde cada numero coloca o piloto, indexado pela mesma chave de linha dos
    /// detalhes. Vazio quando o mundo nao tem historico para comparar.
    #[serde(default)]
    pub recordes: std::collections::HashMap<String, DriverCareerRankEntry>,
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
    /// A temporada do primeiro titulo. E o unico marco que nao cabe num numero
    /// de corrida — um campeonato nao e ganho numa largada — entao vem como a
    /// temporada inteira, no mesmo formato de melhor e pior temporada.
    #[serde(default)]
    pub primeiro_titulo: Option<DriverBestSeasonBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerPeakBlock {
    pub melhor_temporada: Option<DriverBestSeasonBlock>,
    pub maior_sequencia_vitorias: i32,
    /// Ano da primeira e da ultima vitoria da sequencia. Iguais quando ela coube
    /// numa temporada so; a sequencia e contada em corridas consecutivas, entao
    /// ela pode atravessar a virada do ano. `None` em save antigo (sem o campo)
    /// ou quando a temporada da vitoria nao resolve para um ano.
    #[serde(default)]
    pub sequencia_ano_inicio: Option<i32>,
    #[serde(default)]
    pub sequencia_ano_fim: Option<i32>,
    /// Corridas consecutivas NO podio. Espelha o jejum de podios da queda: as
    /// duas marcas so significam alguma coisa lado a lado.
    #[serde(default)]
    pub maior_sequencia_podios: i32,
    #[serde(default)]
    pub sequencia_podios_ano_inicio: Option<i32>,
    #[serde(default)]
    pub sequencia_podios_ano_fim: Option<i32>,
    /// Temporadas terminadas no podio do CAMPEONATO (top 3). O espelho de
    /// "temporadas sem podio" na queda.
    #[serde(default)]
    pub temporadas_no_top3: i32,
}

/// O sabado, que a ficha inteira ignorava.
///
/// O modelo separa `ritmo_classificacao` de `racecraft` como dois atributos, o
/// ranking mundial tem coluna de poles — e a ficha nao mencionava pole nenhuma
/// vez. Metade do fim de semana nao existia aqui.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerQualifyingBlock {
    pub poles: i32,
    /// Poles que viraram vitoria. Largar na frente e converter sao habilidades
    /// diferentes, e a diferenca entre as duas e o retrato do piloto.
    pub poles_convertidas: i32,
    /// Media da posicao de largada na carreira. `None` sem nenhuma largada
    /// registrada.
    pub grid_medio: Option<f64>,
    pub voltas_rapidas: i32,
}

/// O companheiro de equipe: o unico adversario com o MESMO carro.
///
/// E a comparacao mais justa que o jogo tem, e a unica que responde "ele e bom,
/// ou o carro e bom?". Nao existia em lugar nenhum da ficha.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerTeammateBlock {
    pub companheiros: i32,
    /// Temporadas em que houve um companheiro identificavel — o denominador do
    /// confronto, que nao e o mesmo que temporadas disputadas.
    pub temporadas: i32,
    pub temporadas_vencidas: i32,
    /// O companheiro que mais o venceu no confronto direto. `None` quando ele
    /// nunca dividiu box com ninguem.
    pub rival_mais_duro: Option<DriverTeammateDuel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverTeammateDuel {
    pub nome: String,
    pub temporadas: i32,
    pub vitorias: i32,
    pub derrotas: i32,
}

/// Uma linha do detalhe que abre no hover de qualquer numero do dossie.
///
/// E UM tipo para os vinte detalhes diferentes de proposito. Os campos sao os
/// mesmos pedacos recombinados — quando aconteceu, onde, por qual equipe, em que
/// categoria — e cada detalhe preenche os que fazem sentido para ele. Vinte
/// structs quase iguais seriam vinte lugares para esquecer a cor da equipe.
///
/// O que atravessa a ponte e DADO, nao prosa: `categoria` e a chave da divisao e
/// nao o rotulo, `data` e ISO. Quem escreve a frase e o frontend, que ja tem os
/// formatadores e o i18n.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerDetailEntry {
    /// Ano ou faixa ("2018", "2018-2020").
    pub periodo: Option<String>,
    /// Data real da corrida, ISO. So quando o detalhe e de UMA corrida.
    pub data: Option<String>,
    pub rodada: Option<i32>,
    pub pista: Option<String>,
    /// Chave da divisao (`gt3`, `endurance:lmp2`), nunca o rotulo.
    pub categoria: Option<String>,
    /// Divisao de ORIGEM, so nas linhas de promocao e rebaixamento.
    pub categoria_origem: Option<String>,
    pub equipe: Option<String>,
    pub equipe_cor: Option<String>,
    /// Id da equipe, para a linha virar porta de entrada do Atlas. Vem junto com
    /// o nome ou nao vem: equipe que sumiu do mundo nao tem tela para abrir.
    pub equipe_id: Option<String>,
    /// Nome livre: o companheiro de equipe, o nome da lesao.
    pub texto: Option<String>,
    /// Numero que qualifica a linha: corridas naquela equipe, poles naquela
    /// categoria, temporadas ao lado daquele companheiro.
    pub contagem: Option<i32>,
    /// Segunda linha ja pronta quando ela e puramente numerica e nao precisa de
    /// traducao — o placar de pontos de um duelo, por exemplo.
    pub resumo: Option<String>,
    pub idade: Option<i32>,
    pub nacionalidade: Option<String>,
    /// Melhor chegada da temporada, so onde a ausencia de podio e o assunto: numa
    /// temporada sem podio o "P5" e o unico resultado que da tamanho ao ano.
    pub melhor_resultado: Option<i32>,
}

/// Onde um numero do dossie coloca o piloto, nas duas populacoes que importam.
///
/// `grid` e o pelotao da categoria em que ele corre HOJE — quem ele encontra no
/// domingo. `mundo` e a escada inteira. Um piloto pode ser o 2o em poles no grid
/// da GT4 e o 180o do mundo, e as duas coisas sao verdade sobre ele.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerRankEntry {
    pub grid: Option<i32>,
    pub grid_total: i32,
    pub mundo: Option<i32>,
    pub mundo_total: i32,
}

/// A referencia do mundo para os numeros que sozinhos nao dizem nada.
///
/// "1,5% de abandono" e bom ou ruim? "Grid medio 4,2" e frente ou fundo? Os
/// cards de recorde no topo da aba tem barra e "2o de 610"; o dossie inteiro nao
/// tinha referencia nenhuma. Sao medias do mundo, agregadas em uma consulta.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerBenchmarkBlock {
    pub taxa_abandono: Option<f64>,
    pub grid_medio: Option<f64>,
}

/// O contrapeso do auge: o que a carreira cobrou.
///
/// Toda ficha contava so a subida — melhor temporada, maior sequencia, titulos.
/// Uma carreira longa tem os dois lados, e os anos ruins sao o que da tamanho aos
/// bons: quatro temporadas sem podio antes do primeiro titulo dizem mais sobre o
/// piloto do que o titulo sozinho.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerDroughtBlock {
    /// Corridas consecutivas sem vencer. Espelha `maior_sequencia_vitorias`: a
    /// mesma varredura, a condicao invertida.
    pub maior_seca_vitorias: i32,
    /// As temporadas que a seca atravessou, como no auge. `None` quando a
    /// temporada nao resolve para um ano.
    #[serde(default)]
    pub seca_ano_inicio: Option<i32>,
    #[serde(default)]
    pub seca_ano_fim: Option<i32>,
    /// Corridas consecutivas sem subir ao podio. Mede uma queda mais funda que a
    /// de vitorias: um piloto de meio de grid passa a carreira inteira sem
    /// vencer, e o jejum de vitorias dele nao diz nada. Ficar sem NENHUM podio,
    /// esse sim, e o sintoma.
    #[serde(default)]
    pub maior_seca_podios: i32,
    #[serde(default)]
    pub seca_podios_ano_inicio: Option<i32>,
    #[serde(default)]
    pub seca_podios_ano_fim: Option<i32>,
    /// A pior temporada pela MESMA regra que elege a melhor, invertida. `None`
    /// com uma temporada so — a melhor e a pior seriam a mesma linha, e chamar a
    /// unica temporada de um estreante de "a pior" e ruido.
    pub pior_temporada: Option<DriverBestSeasonBlock>,
    pub temporadas_sem_podio: i32,
}

/// Confiabilidade: com que frequencia ele nao chega.
///
/// A ficha so dizia QUANDO ele quebrou pela primeira vez ("primeiro DNF na 74a
/// corrida"), que nao responde se ele quebra sempre. Abandonar duas vezes em 136
/// corridas e abandonar duas vezes em oito sao carreiras opostas com o mesmo
/// numero.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCareerReliabilityBlock {
    pub abandonos: i32,
    pub corridas: i32,
    /// Percentual de 0 a 100. `None` sem nenhuma largada — dividir por zero viraria
    /// um "0%" que se le como "nunca abandona".
    pub taxa_abandono: Option<f64>,
    /// Maior sequencia de corridas consecutivas COMPLETADAS.
    pub maior_sequencia_chegadas: i32,
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
    /// Quantos pilotos entram na conta — o denominador das posicoes acima.
    ///
    /// Sem ele a ficha sabia que o piloto era o 12º e nao sabia 12º de quantos,
    /// entao so dava para imprimir o numero solto. Com o total, a barra de
    /// posicao do dossie v2 desenha na proporcao certa: 1º de 240 enche, 240º de
    /// 240 fica vazia. `Option` + `serde(default)` porque payload antigo (save
    /// aberto por build anterior) simplesmente nao traz o campo, e a UI ja sabe
    /// esconder a barra quando ele falta.
    #[serde(default)]
    pub total: Option<i32>,
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
    /// A carreira em dinheiro, uma temporada por ponto. Vazia para quem nunca
    /// fechou uma temporada — e a ficha simplesmente nao desenha o grafico.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curva: Vec<DriverMarketCurvePoint>,
}

/// Uma temporada da curva de mercado.
///
/// Os dois lados podem faltar, por motivos diferentes: `salario_contrato` falta
/// quando o piloto passou a temporada sem equipe, e `salario_mercado` falta
/// quando o arquivo daquela temporada nao guardou atributos suficientes para
/// reconstruir a avaliacao. Nos dois casos a linha correspondente se PARTE no
/// grafico — nao ha valor a inventar, e uma reta chapada seria lida como medicao.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverMarketCurvePoint {
    pub season_number: i32,
    pub ano: i32,
    pub categoria: String,
    pub equipe_nome: Option<String>,
    /// A cor da casa, para o chip da fita de equipes herdar o tom da marca em vez
    /// de um cinza neutro igual para todo mundo. `None` quando a equipe nao e
    /// resolvivel — save antigo, equipe dissolvida — e ai o chip cai no neutro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipe_cor: Option<String>,
    pub salario_contrato: Option<f64>,
    pub salario_mercado: Option<f64>,
    /// O VALOR do passe naquela temporada, irmao de `salario_mercado` e vindo da
    /// mesma avaliacao. Nao entra no grafico — as duas series dele sao salario, e
    /// uma terceira em outra ordem de grandeza esmagaria as duas. Existe para o
    /// card ao lado poder dizer "+18% vs. 2025" sobre o numero que ele imprime,
    /// e nao sobre um proxy: `valor` e `salario` divergem quando midia ou
    /// desenvolvimento mudam, entao a variacao de um nao serve para o outro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valor_mercado: Option<f64>,
    /// A temporada em curso — ainda nao arquivada, montada com o piloto de hoje.
    pub atual: bool,
    /// Temporada que o contrato em vigor ja cobre mas que ainda nao aconteceu. O
    /// salario dela e fato assinado; o valor de mercado nao existe e nunca vai
    /// existir retroativamente, entao `salario_mercado` e sempre `None` aqui — e
    /// a UI precisa saber a diferenca entre "o arquivo nao guardou" e "ainda nao
    /// foi corrida".
    #[serde(default)]
    pub futuro: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverMarketBlock {
    pub valor_mercado: Option<f64>,
    /// O que o mercado pagaria por ele — nao o que ele ganha hoje. O salario do
    /// contrato em vigor mora em [`ContractDetail::salario_anual`]; e a distancia
    /// entre os dois que diz se ele esta barato ou caro.
    pub salario_estimado: Option<f64>,
    pub chance_transferencia: Option<u8>,
    /// De onde vem a chance acima. `None` para agente livre (nada a decompor) e
    /// para payload antigo, que a UI ja sabe desenhar sem a barra.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forcas_transferencia: Option<TransferForceBreakdown>,
    /// Onde ele cai na lista de valor DO GRID em que corre — 1 e o mais caro.
    ///
    /// Um valor absoluto nao se julga sozinho: "$23,016" nao diz se e muito ou
    /// pouco sem saber quanto valem os outros 23 carros da categoria. O ordinal
    /// e a regua que o numero grande nao tinha.
    ///
    /// `None` quando o piloto nao tem assento (o grid nao o contem) ou quando a
    /// categoria nao e resolvivel — ausencia de lugar, e nao ultimo lugar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posicao_valor: Option<i32>,
    /// O denominador da posicao acima, da MESMA lista que a gerou.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_valor: Option<i32>,
    /// A chave da divisao em que essa posicao vale, para a tela poder nomea-la.
    /// Anda junto com o ordinal porque "8o de 24" sem o campeonato e um ranking
    /// de populacao desconhecida.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub categoria_valor: Option<String>,
}

/// As tres pressoes que somam a chance de troca, ja reescaladas para fechar
/// exatamente no total — a barra empilhada da ficha depende disso.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferForceBreakdown {
    /// Peso do fim de contrato se aproximando.
    pub contrato: u8,
    /// Peso da desmotivacao (so pesa abaixo de 70 de motivacao).
    pub motivacao: u8,
    /// Peso do assedio que o talento atrai (so pesa acima de 60 de skill).
    pub mercado: u8,
    /// Temporadas que ainda restam no contrato, para a tela nomear a fase.
    pub anos_restantes: i32,
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
    /// De onde a rivalidade NASCEU, na chave crua do motor (`Colisao`,
    /// `Companheiros`, `Campeonato`, `Pista`). Quem vira prosa é a tela.
    pub tipo: String,
    /// Faixa semântica da intensidade percebida (`atrito_leve`…`intensa`), para a
    /// tela nomear o que hoje é só um número solto sem escala à vista. Sai de
    /// `rivalry::intensity_level`, e não de um segundo jogo de limiares no JS.
    pub nivel_chave: String,
    pub intensidade: u8,
    pub intensidade_historica: u8,
    pub atividade_recente: u8,
    /// CONFRONTO DIRETO: corridas em que os dois largaram no mesmo grid, e em
    /// quantas cada um terminou à frente. Vitórias + derrotas < confrontos
    /// sempre que alguém abandonou — abandono é encontro, não é duelo.
    pub confrontos: i32,
    pub vitorias: i32,
    pub derrotas: i32,
    /// O MESMO confronto, medido no sábado: quem largou à frente de quem. Dois
    /// pilotos podem ter placar de corrida idêntico e serem pessoas opostas —
    /// um ganha a volta seca e perde no domingo, o outro o contrário. Só a
    /// classificação separa esses dois.
    pub vitorias_quali: i32,
    pub derrotas_quali: i32,
    /// Diferença média de tempo entre os dois nas corridas em que ambos
    /// terminaram, em segundos. Positiva = o rival à frente. É o que separa uma
    /// derrota de fotografia de uma surra: 12–33 pode ser meio segundo ou meia
    /// volta, e o placar não distingue.
    pub gap_medio: Option<f64>,
    /// Se um dia dividiram o mesmo box. É a ÚNICA comparação sem o carro no
    /// meio, e por isso vale mais que todo o resto junto.
    pub companheirismo: Option<DriverRivalTeammateSpell>,
    /// TODA corrida que os dois dividiram, da mais antiga para a mais recente.
    /// A tela desenha uma faixa com isto: uma marca por corrida, agrupada por
    /// temporada. Um placar diz "36 a 29"; a faixa diz em QUAIS, e quando a
    /// maré virou.
    pub encontros: Vec<DriverRivalMeeting>,
    /// Onde o rival está HOJE: categoria e equipe atuais, e se os dois ainda
    /// dividem grid. Rivalidade que morreu por promoção conta outra história.
    pub categoria_atual: Option<String>,
    pub mesma_categoria: bool,
    pub equipe_nome: Option<String>,
    pub equipe_cor: Option<String>,
}

/// O período em que os dois foram companheiros de equipe, com o confronto
/// restrito a ele. Mesmo carro, mesma engenharia, mesma estratégia.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverRivalTeammateSpell {
    pub equipe: Option<String>,
    /// Os anos em que dividiram box, em ordem.
    pub anos: Vec<i32>,
    pub vitorias: i32,
    pub derrotas: i32,
}

/// Uma corrida que os dois dividiram: quando, onde e quem saiu na frente.
///
/// Sem colocação: "P6 contra P1" obriga a comparar dois números para chegar na
/// única coisa que interessa aqui, que é quem ganhou o dia. O ANO vai junto
/// porque "temporada 27" não é uma data que alguém reconheça — o ano é.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverRivalMeeting {
    pub ano: i32,
    pub season_number: i32,
    pub rodada: i32,
    pub pista: String,
    /// `piloto`, `rival` ou `nenhum` — abandono de um dos dois não decide nada.
    pub vencedor: String,
    /// Diferença de tempo entre os dois nesta corrida, em segundos, positiva
    /// quando o rival terminou à frente — a mesma convenção de `gap_medio`.
    /// `None` quando não dá para medir: alguém abandonou, ou é save antigo sem o
    /// campo gravado. A faixa desenha altura com isto, e sem ele desenha só o
    /// mínimo — a corrida aconteceu, a distância é que não foi registrada.
    pub gap: Option<f64>,
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
