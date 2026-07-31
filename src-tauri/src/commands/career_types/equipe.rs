//! DTOs da equipe: resumo, classificação de construtores, finanças e dossiê histórico.

use serde::{Deserialize, Serialize};

use crate::commands::race_history::TrophyInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSummary {
    pub id: String,
    pub nome: String,
    pub nome_curto: String,
    pub cor_primaria: String,
    pub cor_secundaria: String,
    pub categoria: String,
    #[serde(default)]
    pub classe: Option<String>,
    pub car_performance: f64,
    /// Nível do Carro (1–10) do Sistema de Nível do Carro — a ÚNICA leitura de carro que
    /// o jogador vê. Derivado das 11 peças (média dos níveis).
    #[serde(default)]
    pub car_level: u8,
    pub confiabilidade: f64,
    #[serde(default)]
    pub pit_strategy_risk: f64,
    #[serde(default)]
    pub pit_crew_quality: f64,
    pub budget: f64,
    #[serde(default)]
    pub spending_power: f64,
    #[serde(default)]
    pub salary_ceiling: f64,
    #[serde(default)]
    pub budget_index: f64,
    #[serde(default)]
    pub cash_balance: f64,
    #[serde(default)]
    pub debt_balance: f64,
    #[serde(default)]
    pub financial_state: String,
    #[serde(default)]
    pub season_strategy: String,
    #[serde(default)]
    pub last_round_income: f64,
    #[serde(default)]
    pub last_round_expenses: f64,
    #[serde(default)]
    pub last_round_net: f64,
    #[serde(default)]
    pub parachute_payment_remaining: f64,
    pub piloto_1_id: Option<String>,
    pub piloto_1_nome: Option<String>,
    #[serde(default)]
    pub piloto_1_salario_anual: Option<f64>,
    pub piloto_2_id: Option<String>,
    pub piloto_2_nome: Option<String>,
    #[serde(default)]
    pub piloto_2_salario_anual: Option<f64>,
    /// Política interna da garagem (módulo `hierarchy`). Quem é N1 aqui é a hierarquia
    /// REAL — pode divergir de `piloto_1_id`/`piloto_2_id`, que é só a ordem dos slots,
    /// depois de uma inversão no meio da temporada.
    #[serde(default)]
    pub hierarquia_n1_id: Option<String>,
    #[serde(default)]
    pub hierarquia_n2_id: Option<String>,
    /// Clima: `estavel` | `competitivo` | `tensao` | `reavaliacao` | `inversao` | `crise`.
    #[serde(default)]
    pub hierarquia_status: String,
    /// Tensão acumulada (0–100). Acima de 50 já pesa na moral da equipe
    /// ([`crate::finance::morale::advance_team_morale`]).
    #[serde(default)]
    pub hierarquia_tensao: f64,
    #[serde(default)]
    pub hierarquia_inversoes_temporada: i32,
    /// PRESENÇA PÚBLICA da equipe (0–100): perfil de audiência derivado da mídia do
    /// lineup ativo (`public_presence::team`, 70% do mais midiático + 30% do segundo).
    /// Multiplica linearmente o patrocínio da rodada em `commands/race/financas.rs` —
    /// é o que torna a escolha de companheiro uma decisão financeira.
    ///
    /// NÃO confundir com `DriverCareerPresenceBlock.presenca` (piloto.rs), que é TEMPO
    /// DE CARREIRA, nem com `market::visibility::MarketVisibilityTier`, que classifica
    /// a `midia` de um piloto individual. Grandezas distintas e sem tier aqui.
    #[serde(default)]
    pub presenca_publica: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamStanding {
    pub posicao: i32,
    pub id: String,
    pub nome: String,
    pub nome_curto: String,
    pub cor_primaria: String,
    #[serde(default)]
    pub cash_balance: f64,
    #[serde(default)]
    pub car_performance: f64,
    /// Nível do Carro (1–10) — Sistema de Nível do Carro (a leitura de carro do jogador).
    #[serde(default)]
    pub car_level: u8,
    /// Confiabilidade (0–100) e qualidade do pit crew (0–100) da equipe — leituras reais do
    /// modelo Team, para o comparativo de gestão/performance no ranking da categoria.
    #[serde(default)]
    pub confiabilidade: f64,
    #[serde(default)]
    pub pit_crew_quality: f64,
    #[serde(default)]
    pub founded_year: i32,
    pub pontos: i32,
    pub vitorias: i32,
    pub piloto_1_nome: Option<String>,
    pub piloto_1_tenure_seasons: Option<i32>,
    pub piloto_2_nome: Option<String>,
    pub piloto_2_tenure_seasons: Option<i32>,
    pub trofeus: Vec<TrophyInfo>,
    pub classe: Option<String>,
    pub temp_posicao: i32,
    pub categoria_anterior: Option<String>,
    /// Históricos acumulados de carreira da equipe (reais, do modelo Team). Alimentam o
    /// ranking e o fallback do drawer sem estimativas fabricadas no front.
    #[serde(default)]
    pub historico_vitorias: i32,
    #[serde(default)]
    pub historico_podios: i32,
    #[serde(default)]
    pub historico_titulos_construtores: i32,
}

/// Resposta do comando `get_team_finance_report`: a divisão financeira REAL da equipe,
/// lida da tabela `team_finance_history`. Números crus (o front cuida de rótulos, cores
/// e formatação). Substitui os valores fabricados na aba My Team.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamFinanceReport {
    /// Total de rodadas com histórico registrado (0 = save antigo / sem corridas ainda).
    pub rounds_recorded: i32,
    /// Última rodada com histórico (para os ledgers de entradas/saídas).
    pub latest: Option<TeamFinanceRound>,
    /// Acumulado da temporada corrente (para a rosca de custos e a leitura de receita).
    /// Aqui `round` carrega a CONTAGEM de rodadas somadas na temporada.
    pub season: Option<TeamFinanceRound>,
    /// Caixa ao fim de cada rodada recente, em ordem cronológica (para o gráfico de caixa).
    pub cash_timeline: Vec<TeamFinanceCashPoint>,
    /// Prêmio de construtores ESTIMADO se a temporada terminasse agora (pela posição atual
    /// no campeonato). Só projeção de exibição — NÃO entra no caixa nem nas decisões da IA.
    pub expected_constructor_prize: f64,
    /// Posição atual da equipe no campeonato de construtores (0 = indisponível).
    pub current_position: i32,
    /// Nº de equipes no grupo do campeonato (para contextualizar a posição/projeção).
    pub grid_size: i32,
}

/// As 9 linhas reais de receita/despesa + totais. Serve tanto para a última rodada
/// quanto para o acumulado da temporada.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamFinanceRound {
    pub season_number: i32,
    pub round: i32,
    pub sponsorship_income: f64,
    /// Bilheteria/portão da rodada (Fase 3 do Estrelato): público que a fama do lineup
    /// atrai, escalado pelo prestígio do evento. 0 nas linhas legadas.
    pub gate_income: f64,
    pub result_bonus: f64,
    pub partial_prize_income: f64,
    pub aid_income: f64,
    pub salary_expense: f64,
    pub event_operations_cost: f64,
    pub structural_maintenance_cost: f64,
    pub technical_investment_cost: f64,
    pub debt_service_cost: f64,
    /// Prêmio de construtores creditado (só > 0 na linha de encerramento da temporada).
    pub constructor_prize_income: f64,
    pub income_total: f64,
    pub expenses_total: f64,
    pub net: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamFinanceCashPoint {
    pub season_number: i32,
    pub round: i32,
    pub cash_balance: f64,
    pub net: f64,
    /// `true` na linha de ENCERRAMENTO (prêmio de construtores) — o front rotula/colore
    /// esse ponto de forma distinta no gráfico de caixa.
    pub is_season_close: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryDossier {
    pub team_id: String,
    pub category: String,
    pub record_scope: String,
    pub has_history: bool,
    pub records: Vec<TeamHistoryRecord>,
    pub sport: TeamHistorySport,
    pub identity: TeamHistoryIdentity,
    pub management: TeamHistoryManagement,
    pub timeline: Vec<TeamHistoryTimelineItem>,
    pub title_categories: Vec<TeamHistoryTitleCategory>,
    pub category_path: Vec<TeamHistoryCategoryStep>,
    /// Eventos de propriedade/diretoria (ex.: venda por colapso). Consumido pelas
    /// abas Identidade (como "eras" do time) e Gestão (como evento financeiro).
    pub ownership_events: Vec<TeamHistoryOwnershipEvent>,
    /// Superlativos da equipe (melhor temporada, maior sequência de títulos...).
    /// Enriquecem a aba Record além dos rankings comparativos.
    pub highlights: Vec<TeamHistoryHighlight>,
    /// Marcos da história (primeira vitória, primeiro título...) com o ano.
    pub milestones: Vec<TeamHistoryMilestone>,
    /// Resultados temporada a temporada — alimenta a aba Esportivo (resultados
    /// ao longo do tempo), distinguindo-a de Categorias (movimento entre tiers).
    pub season_results: Vec<TeamHistorySeasonResult>,
    /// Últimas corridas registradas, da mais antiga para a mais nova. É o único
    /// recorte do dossiê que fala do PRESENTE — todo o resto é história agregada,
    /// e agregado de 87 corridas não mostra que a equipe subiu de categoria mês
    /// passado e não anda mais perto do pódio.
    pub recent_form: Vec<TeamHistoryFormRace>,
    /// Distribuição de TODAS as corridas por faixa de colocação. A taxa de pódio
    /// em Records diz quanto; isto diz a forma — separa a equipe que vence ou
    /// abandona da que vive em quarto, que têm a mesma taxa.
    pub result_spread: TeamHistoryResultSpread,
    /// A sucessão de pilotos nas DUAS vagas da equipe, uma entrada por passagem
    /// contínua. É o único recorte do dossiê que fala de GENTE — todo o resto
    /// trata a equipe como um carro só — e, repartido por vaga, também responde
    /// quanto essa casa troca de piloto.
    pub lineup: Vec<TeamHistoryLineupTerm>,
    /// Quantas largadas viraram chegada e o que levou o resto para o box. Sem
    /// isso, a equipe que abandona metade das corridas e a que termina todas em
    /// décimo caem na mesma faixa da assinatura de resultados.
    pub reliability: TeamHistoryReliability,
    /// Anos em que a equipe correu fora do recorte de categorias deste dossiê.
    /// A faixa temporada a temporada precisa deles para não marcar como
    /// "não disputou" um ano em que a equipe estava em outra escada.
    pub outside_scope_seasons: Vec<TeamHistoryOutsideSeason>,
    /// Resumo de movimento entre categorias (real) — alimenta a aba Categorias.
    pub movement: TeamHistoryMovement,
    /// Primeiro e último ano do MUNDO (não da equipe). A faixa temporada a
    /// temporada desenha uma coluna por ano deste intervalo, marcando com "×" os
    /// anos em que a equipe não correu — sem isso, uma equipe nova ocupava três
    /// colunas num gráfico largo e não dava para ver que ela é nova.
    /// Zero quando o save ainda não tem temporada alguma.
    pub world_first_year: i32,
    pub world_last_year: i32,
    /// A campanha da temporada MAIS RECENTE em que a equipe correu, rodada a
    /// rodada, com a linha de todas as outras equipes do mesmo campeonato.
    ///
    /// É a leitura que a curva de posição final por temporada não dá: a posição
    /// no campeonato é o resultado, isto é a disputa. Vinte pontos de vantagem
    /// construídos na primeira metade e defendidos, ou uma virada na última
    /// rodada, terminam os dois em "P1" — e desenham completamente diferente.
    ///
    /// `None` quando a equipe não tem nenhuma corrida no recorte.
    pub championship_run: Option<TeamHistoryChampionshipRun>,
}

/// A tabela de recordes de TODAS as equipes de um grupo de categorias.
///
/// É o mesmo agregado que alimenta os cards de record do dossiê — o "9º de 19"
/// de um card e a posição da equipe nesta lista têm de ser o mesmo número, então
/// os dois saem do mesmo recorte e da mesma contagem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRecordsRanking {
    /// Rótulo do recorte ("Grupo GT3"), o mesmo que o rodapé do dossiê mostra.
    pub scope: String,
    /// Categoria pedida, crua — o frontend usa para manter o filtro coerente.
    pub category: String,
    /// Amplitude aplicada: "category", "group" ou "world". Volta ecoada porque um
    /// valor desconhecido cai em "group", e a tela precisa marcar o que de fato
    /// aconteceu, não o que foi pedido.
    pub scope_kind: String,
    /// Os rótulos das categorias que ENTRARAM na conta.
    ///
    /// A tela nomeia esta lista porque "grupo" não tem tamanho fixo: o Grupo
    /// Mazda são duas categorias (Rookie e Championship) e o Grupo Production são
    /// seis — a Production é onde as três escadas de entrada convergem, e a
    /// história de uma equipe dela inclui os anos de Mazda, Toyota e BMW. Sem a
    /// lista à vista, alargar o recorte a partir da Mazda parece deixar coisa de
    /// fora, quando na verdade os dois grupos são de tamanhos diferentes.
    pub scope_categories: Vec<String>,
    /// A marca que recorta as categorias multiclasse deste escopo ("mazda",
    /// "toyota", "bmw"), ou vazio. Na Production, o Grupo Mazda conta só as
    /// equipes de classe Mazda — e a tela precisa dizer isso, senão a Production
    /// aparece na lista de categorias e parece que Toyota e BMW entraram junto.
    pub scope_family: String,
    /// A escada de categorias que o seletor oferece, em ordem. Vem do backend
    /// porque é regra de domínio: duplicá-la no frontend criaria uma segunda
    /// fonte de verdade para quem compara com quem.
    pub categories: Vec<TeamRecordsCategory>,
    pub rows: Vec<TeamRecordsRow>,
}

/// Uma categoria da escada, como o seletor a mostra.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRecordsCategory {
    /// Chave única da entrada. Igual ao `id` nas monomarca; `"categoria:classe"`
    /// nas multiclasse, onde a mesma categoria vira três entradas.
    pub key: String,
    pub id: String,
    /// Classe desta entrada ("mazda", "gt3"...), vazia nas monomarca.
    pub class: String,
    pub label: String,
    /// Rótulo do grupo a que ela pertence ("Grupo Mazda"). A tela mostra isto na
    /// opção de amplitude, para o jogador ver o que "grupo" significa NESTA
    /// categoria antes de escolher.
    pub group_label: String,
}

/// Uma equipe na tabela de recordes. Sem pontos: eles não se comparam entre
/// calendários e categorias, pelo mesmo motivo que os tiraram da faixa de
/// Records do dossiê.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRecordsRow {
    pub team_id: String,
    pub team: String,
    pub color: String,
    /// Categoria em que a equipe corre HOJE — pode não ser a do recorte, porque
    /// o grupo junta escadas vizinhas e a equipe pode ter subido.
    pub category: String,
    pub category_id: String,
    pub active: bool,
    pub titles: i32,
    pub wins: i32,
    pub podiums: i32,
    pub races: i32,
    pub podium_rate: i32,
    pub win_rate: i32,
    /// Os mesmos quatro números, agora da CARREIRA inteira — todas as categorias,
    /// todos os anos.
    ///
    /// Existem para o filtro ser visível no próprio dado: "5 de 87 corridas" diz
    /// sozinho que o recorte está agindo, enquanto um "5" solto se parece com uma
    /// equipe que mal correu. Taxas não têm par porque proporção não se lê como
    /// fração de outra proporção.
    pub total_titles: i32,
    pub total_wins: i32,
    pub total_podiums: i32,
    pub total_races: i32,
    /// Primeiro e último ano da equipe DENTRO do recorte.
    ///
    /// É o que dá sentido à contagem: "5 corridas" sozinho não diz se a equipe é
    /// nova, se passou correndo ou se está lá há uma década. Com "2024–2025" ao
    /// lado, uma linha de 5 corridas na Mazda Rookie deixa de parecer uma
    /// carreira inteira e vira o que é — uma passagem curta antes da promoção.
    pub first_year: String,
    pub last_year: String,
}

/// Uma campanha de campeonato: o eixo de rodadas e uma linha por equipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryChampionshipRun {
    pub year: String,
    pub category: String,
    pub category_id: String,
    /// Rodadas em ordem — o eixo horizontal do desenho.
    pub rounds: Vec<i32>,
    /// Uma linha por equipe, já ordenada da primeira à última colocada.
    pub lines: Vec<TeamHistoryChampionshipLine>,
    /// Se o campeonato ainda está em andamento (a temporada mais recente do save
    /// é esta e ela tem rodadas por correr). Muda a leitura da ponta da linha.
    pub live: bool,
}

/// A linha de uma equipe: pontuação ACUMULADA ao fim de cada rodada.
///
/// O acumulado, e não os pontos da corrida, é o que faz o desenho responder
/// "quem está ganhando" — pontos soltos por corrida desenham serrote em todo
/// mundo e não somam visualmente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryChampionshipLine {
    pub team_id: String,
    pub team: String,
    /// Se é a equipe do dossiê — a única que o desenho acende.
    pub selected: bool,
    /// Colocação no campeonato ao fim das rodadas disputadas.
    pub position: i32,
    pub total: String,
    /// Um valor por rodada de `rounds`, na mesma ordem. Equipe que entrou depois
    /// carrega o acumulado anterior (zero) — a linha nunca tem buraco.
    pub points: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamHistoryMovement {
    pub promotions: i32,
    pub relegations: i32,
    pub time_by_category: String,
    pub best_category: String,
    pub hardest_category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryHighlight {
    pub label: String,
    pub value: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryMilestone {
    pub label: String,
    pub year: String,
    /// Identidade do marco ("first_podium", "first_win", "first_title"). Existe
    /// para o frontend fundir marcos e linha do tempo sem casar prosa traduzida:
    /// os dois blocos contavam a primeira vitória, cada um com uma frase.
    pub kind: String,
}

/// Um ano em que a equipe correu em outra escada de categorias.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryOutsideSeason {
    pub year: String,
    pub category: String,
    pub category_id: String,
}

/// Uma corrida na fita de forma recente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryFormRace {
    pub year: String,
    pub round: i32,
    pub category: String,
    pub category_id: String,
    /// Melhor colocação da equipe na corrida. `None` quando o resultado não
    /// registrou posição — o quadrado aparece vazio, e não como abandono.
    pub position: Option<i32>,
}

/// Uma passagem contínua de um piloto por uma vaga da equipe, com o que ele fez
/// lá e onde está hoje.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeamHistoryLineupTerm {
    /// 1 ou 2 conforme a vaga; 0 para quem correu pela equipe sem constar como
    /// titular de uma temporada arquivada (substituto, ou save sem arquivo).
    pub slot: i32,
    pub driver_id: String,
    pub name: String,
    /// Nacionalidade como está em `drivers` — a galeria a desenha como bandeira.
    /// Vazia para quem só existe em `retired` (a tabela não guarda o país) ou
    /// para id herdado de save antigo: aí a galeria mostra o piloto sem bandeira.
    pub nationality: String,
    pub first_year: String,
    pub last_year: String,
    /// Números da PASSAGEM, não da carreira: quem saiu e voltou tem duas
    /// entradas, e somar tudo nas duas contaria a mesma corrida duas vezes.
    pub races: i32,
    pub wins: i32,
    pub podiums: i32,
    /// Melhor colocação que o piloto tirou nesta passagem. Zero quando nenhuma
    /// corrida dele teve classificação.
    pub best_position: i32,
    /// O piloto do save. A galeria o destaca: é a única linha em que o jogador
    /// se vê dentro da história da equipe.
    pub is_player: bool,
    /// Continua na equipe hoje — a galeria separa quem está de quem passou.
    pub still_here: bool,
    /// Equipe onde o piloto corre HOJE, quando é outra. Vazio para quem ficou,
    /// aposentou ou sumiu — repetir o brasão da própria equipe não distingue
    /// nada, e é justamente o destino de quem saiu que interessa.
    pub current_team_name: String,
    pub current_team_color: String,
    /// Onde o piloto está agora, já em prosa traduzida: na equipe, em outra
    /// categoria, ou aposentado (com o ano, quando registrado).
    pub current_label: String,
}

/// Largadas, chegadas e a repartição dos abandonos por causa.
///
/// `mechanical + driver_error + other` soma os abandonos; `finished + abandonos`
/// soma `races`. `other` é abandono sem incidente catalogado — não é uma terceira
/// causa, é a admissão de que a causa não foi registrada.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeamHistoryReliability {
    pub races: i32,
    pub finished: i32,
    pub finish_rate: i32,
    /// Média do GRUPO de categorias. Uma taxa de 88% não diz nada sozinha: em
    /// endurance ela pode ser excelente e em sprint, medíocre.
    pub group_finish_rate: i32,
    pub mechanical: i32,
    pub driver_error: i32,
    pub other: i32,
    /// A peça que mais tirou a equipe da corrida, vinda de `race_breakdowns`.
    /// Vazio quando o save não tem quebra registrada.
    pub worst_part: String,
}

/// Corridas repartidas por faixa de colocação. As faixas são exclusivas e somam
/// `races`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeamHistoryResultSpread {
    pub races: i32,
    pub first: i32,
    pub podium: i32,
    pub near_miss: i32,
    pub top_ten: i32,
    pub outside: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistorySeasonResult {
    pub year: String,
    pub category: String,
    /// Id cru da categoria dominante ("gt4", "bmw_m2"). O `category` acima é
    /// rótulo traduzido, bom para ler e inútil para casar com a paleta de
    /// categorias do frontend — que é indexada por id.
    pub category_id: String,
    /// Posição final no campeonato naquela temporada ("P3", "—" se desconhecida).
    pub position: String,
    pub wins: i32,
    pub podiums: i32,
    pub points: String,
    /// Corridas disputadas na temporada. É o denominador que torna as temporadas
    /// comparáveis: pontos somados dependem do tamanho do calendário, "pódios em
    /// 12 corridas" não.
    pub races: i32,
    /// Corridas em que o melhor carro da equipe terminou em 2º.
    pub seconds: i32,
    /// Corridas em que o melhor carro da equipe terminou em 3º.
    pub thirds: i32,
    /// Corridas em que o melhor carro da equipe terminou em 4º.
    pub fourths: i32,
    /// Corridas em que o melhor carro da equipe terminou em 5º.
    ///
    /// 4º e 5º não são pódio, mas são o "quase" — e uma equipe que passou a
    /// temporada inteira ali tem uma história a contar que um gráfico só de
    /// pódios desenha como tela vazia.
    pub fifths: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryOwnershipEvent {
    pub year: String,
    pub event_type: String,
    /// Título curto para a aba Identidade (ex.: "Nova diretoria").
    pub title: String,
    /// Descrição contextual.
    pub detail: String,
    /// Resumo financeiro para a aba Gestão (ex.: "Dívida de $38M zerada · aporte de $7M").
    pub financial_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryRecord {
    /// Identificador estável da métrica ("titles", "wins", "podiums",
    /// "podium_rate", "win_rate"). O `label` é texto traduzido e muda com o
    /// idioma; a UI escolhe ícone e layout por ESTE campo.
    pub id: String,
    pub label: String,
    pub rank: String,
    pub value: String,
    /// Posição numérica do rank (o mesmo número que `rank` mostra por extenso).
    /// Separado porque a UI desenha uma barra com ele, e reparsear "10º" seria
    /// desfazer no front o que o backend já sabe.
    pub rank_position: i32,
    /// Quantas equipes disputam esse rank — o denominador do "10º de 24".
    pub rank_total: i32,
    /// Média do grupo na mesma métrica, já formatada como o `value`.
    pub group_average: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistorySport {
    pub seasons: String,
    pub current_streak: String,
    pub best_streak: String,
    pub podium_rate: String,
    pub win_rate: String,
    pub races: i32,
    pub wins: i32,
    pub podiums: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryIdentity {
    pub origin: String,
    pub current: String,
    /// Herança por experiência real (temporadas): Novata → Tradicional.
    pub heritage: String,
    pub profile: String,
    pub summary: String,
    pub rival: TeamHistoryRival,
    pub symbol_driver: String,
    pub symbol_driver_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryRival {
    pub name: String,
    pub current_category: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryManagement {
    pub operation_health: String,
    pub peak_cash: String,
    pub worst_crisis: String,
    pub healthy_years: String,
    pub efficiency: String,
    pub biggest_investment: String,
    pub summary: String,
    pub peak_cash_detail: String,
    pub worst_crisis_detail: String,
    pub healthy_years_detail: String,
    pub efficiency_detail: String,
    pub investment_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryTimelineItem {
    pub year: String,
    pub text: String,
    /// Mesma chave dos marcos (ver `TeamHistoryMilestone::kind`): quando os dois
    /// falam do mesmo fato, o frontend fica com esta versão, que traz categoria
    /// e rodada.
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryTitleCategory {
    pub category: String,
    /// Id cru da categoria — a cor do card vem da paleta de categorias, e não de
    /// uma paleta rotativa que pintava títulos da MESMA categoria de cores
    /// diferentes só porque estavam em linhas diferentes.
    pub category_id: String,
    pub year: String,
    pub color: String,
    /// Pontos e vitórias da equipe na temporada do título.
    pub points: String,
    pub wins: i32,
    /// Campeão de PILOTOS daquela temporada e categoria. É outro campeonato: a
    /// equipe pode ter levado o de construtores com regularidade enquanto o de
    /// pilotos ia para outra casa.
    pub champion_driver: String,
    /// Equipe do campeão de pilotos, quando não é a do dossiê.
    pub champion_team: String,
    /// O campeão de pilotos era da equipe — construtores e pilotos no mesmo ano.
    pub champion_is_team: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryCategoryStep {
    pub category: String,
    pub years: String,
    pub detail: String,
    pub color: String,
    /// "start" | "promotion" | "relegation" | "same" — marcador da escada.
    pub movement: String,
}
