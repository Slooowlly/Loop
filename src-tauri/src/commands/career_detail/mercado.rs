//! Valor de mercado do piloto: salario estimado, valor e chance de transferencia.

use super::*;

pub(super) fn build_driver_market_block(
    conn: &Connection,
    driver: &Driver,
    contract: Option<&Contract>,
    team: Option<&Team>,
    current_season: i32,
) -> DriverMarketBlock {
    let category_id = resolve_driver_category(driver, contract, team);
    let avaliacao = avaliar_mercado(&EntradasDeMercado {
        categoria: category_id.as_deref(),
        skill: driver.atributos.skill,
        midia: driver.atributos.midia,
        desenvolvimento: driver.atributos.desenvolvimento,
        titulos: driver.stats_carreira.titulos,
        vitorias: driver.stats_carreira.vitorias,
        podios: driver.stats_carreira.podios,
    });
    let forcas = transfer_forces_for_driver(driver, contract, current_season);
    let posicao = category_id
        .as_deref()
        .and_then(|categoria| rank_no_grid_por_valor(conn, driver, categoria, team));

    DriverMarketBlock {
        valor_mercado: Some(avaliacao.valor),
        salario_estimado: Some(avaliacao.salario),
        chance_transferencia: Some(forcas.total),
        forcas_transferencia: forcas.parcelas,
        posicao_valor: posicao.map(|(posicao, _)| posicao),
        total_valor: posicao.map(|(_, total)| total),
        categoria_valor: posicao.and(category_id),
    }
}

/// Em que degrau da tabela de valor do grid ele esta, e de quantos.
///
/// Todo mundo e avaliado pela MESMA categoria — a do piloto da ficha — e nao
/// cada um pela sua: o grid e um so campeonato, o baseline do tier e comum, e
/// deixar cada rival puxar o proprio baseline compararia dolares de escadas
/// diferentes. Como a base entra multiplicando igual para todos, a ordem que sai
/// daqui e a ordem do talento + curriculo + midia dentro daquele pelotao.
///
/// `None` quando o piloto nao esta no grid (aposentado, sem assento, entre
/// categorias): sem lugar na lista nao ha ordinal a inventar.
fn rank_no_grid_por_valor(
    conn: &Connection,
    driver: &Driver,
    categoria: &str,
    team: Option<&Team>,
) -> Option<(i32, i32)> {
    let grid = pilotos_do_grid(conn, categoria, team);
    if grid.len() < 2 || !grid.iter().any(|value| value.id == driver.id) {
        return None;
    }

    let valor_de = |piloto: &Driver| {
        avaliar_mercado(&EntradasDeMercado {
            categoria: Some(categoria),
            skill: piloto.atributos.skill,
            midia: piloto.atributos.midia,
            desenvolvimento: piloto.atributos.desenvolvimento,
            titulos: piloto.stats_carreira.titulos,
            vitorias: piloto.stats_carreira.vitorias,
            podios: piloto.stats_carreira.podios,
        })
        .valor
    };

    let meu = valor_de(driver);
    // Empate nao empurra ninguem para tras: quem vale o mesmo divide a posicao,
    // entao o ordinal e "quantos valem MAIS que ele" + 1. Dois pilotos iguais
    // sao os dois 5o, e nao um 5o e um 6o decidido por ordem alfabetica.
    let acima = grid.iter().filter(|rival| valor_de(rival) > meu).count();

    Some((acima as i32 + 1, grid.len() as i32))
}

/// Tudo que o modelo de avaliacao precisa saber sobre o piloto NAQUELE momento.
///
/// E uma struct, e nao os campos do `Driver`, exatamente porque a curva historica
/// alimenta o mesmo modelo com atributos arquivados de uma temporada passada.
pub(super) struct EntradasDeMercado<'a> {
    pub categoria: Option<&'a str>,
    pub skill: f64,
    pub midia: f64,
    pub desenvolvimento: f64,
    pub titulos: u32,
    pub vitorias: u32,
    pub podios: u32,
}

pub(super) struct AvaliacaoDeMercado {
    pub salario: f64,
    pub valor: f64,
}

/// O modelo de avaliacao num lugar so.
///
/// A curva historica e o numero do card TEM que sair daqui os dois: se cada um
/// tivesse a sua copia da formula, o ultimo ponto do grafico deixaria de bater
/// com o valor impresso ao lado dele na primeira vez que alguem mexesse num
/// coeficiente — e ninguem perceberia.
pub(super) fn avaliar_mercado(entradas: &EntradasDeMercado) -> AvaliacaoDeMercado {
    let base_salary = salary_baseline_for_category(entradas.categoria);
    // O MESMO modificador que uma equipe aplica ao montar uma proposta
    // ([`crate::finance::salary::calculate_offer_salary_from_money`]). A pergunta
    // da aba e "o que o mercado pagaria" — entao a resposta tem que sair do
    // modificador que o mercado usa, e nao de uma curva propria daqui. Em skill
    // 70 ele vale 1.0: o piloto mediano do tier ganha exatamente a base do tier.
    let skill_factor = (entradas.skill.clamp(0.0, 100.0) / 70.0).clamp(0.70, 1.75);
    // O curriculo SATURA em vez de bater num teto.
    //
    // Somar 0.16 por titulo sem limite fazia um veterano multiplicar a base por
    // 4; cortar isso num `.min()` seco empilharia o bicampeao e a lenda no mesmo
    // numero, e a curva — que existe para mostrar crescimento — viraria uma reta
    // a partir do primeiro titulo. A forma hiperbolica cresce sempre e nunca
    // passa de +60%: 1 titulo/8 vitorias/20 podios da ~1.29, uma carreira de
    // lenda chega perto de 1.55 sem nunca encostar no limite.
    let curriculo = entradas.titulos as f64 * 3.0
        + entradas.vitorias as f64 * 0.5
        + entradas.podios as f64 * 0.2;
    let career_factor = 1.0 + 0.6 * (curriculo / (curriculo + 12.0));
    let media_factor = 0.9 + entradas.midia.clamp(0.0, 100.0) / 500.0;
    // O estimado sai SEMPRE do modelo, nunca do contrato em vigor.
    //
    // Antes ele caia no `salario_anual` quando havia contrato, e com isso a ficha
    // imprimia o mesmo numero duas vezes: "Salario" no card do contrato e
    // "Salario" no card de mercado, identicos. Separados, os dois viram uma
    // comparacao — o que ele ganha contra o que o mercado pagaria — que e a
    // pergunta que a aba existe para responder.
    let salario = (base_salary * skill_factor * career_factor)
        .max(5_000.0)
        .round();
    let value_multiplier = 2.2 + entradas.desenvolvimento.clamp(0.0, 100.0) / 70.0;

    AvaliacaoDeMercado {
        valor: (salario * value_multiplier * media_factor).round(),
        salario,
    }
}

/// A base salarial da categoria, tirada da MESMA escada que a economia usa.
///
/// Era uma tabela escrita a mao aqui — 10k/27.5k/55k/105k/200k/165k — que nasceu
/// calibrada e envelheceu sozinha ao lado da escada real
/// ([`crate::finance::salary::category_salary_base_for_tier`], derivada do custo
/// operacional do tier). Os tres primeiros degraus ainda batiam; o gt4 pagava
/// 60% do real, o gt3 pagava 20%, e o **endurance nao tinha degrau nenhum**:
/// tier 6, sem braco no `match`, caia no `_` de categoria desconhecida ($20k) —
/// ABAIXO do amador. A ficha imprimia "o mercado pagaria $100k" para um piloto
/// cujo contrato no endurance vale entre $524k e $1.5M
/// ([`crate::models::contract::salary_range_for_tier`]).
///
/// Derivar mata a tabela paralela na origem: existe uma escada so, e um tier novo
/// entra nela sem precisar lembrar deste arquivo.
pub(super) fn salary_baseline_for_category(category_id: Option<&str>) -> f64 {
    // O que chega aqui pode ser a chave de divisao competitiva ("endurance:lmp2",
    // vinda de `resolve_driver_category`), que `get_category_config` nao conhece.
    // Sem tirar a classe, TODO piloto de categoria multiclasse caia no
    // desconhecido mesmo com o tier certo mapeado.
    let tier = category_id
        .map(base_category_of)
        .and_then(categories::get_category_config)
        .map(|config| config.tier)
        // Categoria desconhecida e o degrau de entrada, nao um fundo de poco
        // inventado: e o unico chute que nao afirma nada sobre o piloto.
        .unwrap_or(1);

    crate::finance::salary::category_salary_base_for_tier(tier)
}

pub(super) fn transfer_chance_for_driver(
    driver: &Driver,
    contract: Option<&Contract>,
    current_season: i32,
) -> u8 {
    transfer_forces_for_driver(driver, contract, current_season).total
}

/// A chance de troca com as tres forcas que a compoem preservadas.
///
/// O calculo sempre somou tres pressoes distintas e serializava so o total, o
/// que deixava a ficha com um "57%" sem explicacao. Aqui elas sobrevivem ate a
/// tela: "vence agora" pesa diferente de "esta desmotivado", e o jogador precisa
/// saber qual dos dois esta puxando.
pub(super) struct TransferForces {
    pub total: u8,
    /// `None` para agente livre: sem contrato nao ha o que decompor, o 100% e a
    /// propria ausencia de vinculo.
    pub parcelas: Option<TransferForceBreakdown>,
}

pub(super) fn transfer_forces_for_driver(
    driver: &Driver,
    contract: Option<&Contract>,
    current_season: i32,
) -> TransferForces {
    let Some(contract) = contract else {
        return TransferForces {
            total: 100,
            parcelas: None,
        };
    };

    let remaining = contract.anos_restantes(current_season);
    let contract_pressure = if remaining <= 0 {
        54.0
    } else if remaining == 1 {
        34.0
    } else {
        14.0
    };
    let motivation_pressure = (70.0 - driver.motivacao).max(0.0) * 0.45;
    let market_pull = (driver.atributos.skill - 60.0).max(0.0) * 0.28;

    let bruto = contract_pressure + motivation_pressure + market_pull;
    let total = bruto.round().clamp(5.0, 95.0);

    // O clamp e o arredondamento fazem o total divergir da soma crua, e uma barra
    // empilhada que nao fecha no numero ao lado dela e pior que nao ter barra.
    // As parcelas sao reescaladas na proporcao original e a ultima recebe o
    // residuo, entao contrato + motivacao + mercado == total sempre.
    let fator = if bruto > 0.0 { total / bruto } else { 0.0 };
    let contrato = (contract_pressure * fator).round().clamp(0.0, total) as u8;
    let motivacao = (motivation_pressure * fator)
        .round()
        .clamp(0.0, total - contrato as f64) as u8;
    let mercado = total as u8 - contrato - motivacao;

    TransferForces {
        total: total as u8,
        parcelas: Some(TransferForceBreakdown {
            contrato,
            motivacao,
            mercado,
            anos_restantes: remaining,
        }),
    }
}

/// A curva de mercado da carreira: o que ele ganhou contra o que valia, ano a ano.
///
/// Os dois lados vem de fontes diferentes e nenhuma delas precisou de migracao:
///
/// - o que ele **ganhou** sai da tabela `contracts`, que nunca apaga nada — cada
///   contrato ja assinado continua la com salario, equipe e vigencia;
/// - o que ele **valia** e reconstruido aplicando o modelo de hoje aos atributos
///   ARQUIVADOS daquela temporada, que o `snapshot_json` do
///   `driver_season_archive` guarda por inteiro.
///
/// A reconstrucao e honesta sobre o que e: nao existe um valor de mercado que
/// tenha sido registrado a epoca, existe o modelo atual aplicado ao piloto que
/// ele era. Se a formula mudar, o passado muda junto — e por isso a formula e
/// uma so ([`avaliar_mercado`]).
pub(super) fn build_driver_market_curve(
    conn: &Connection,
    driver: &Driver,
    contract: Option<&Contract>,
    team: Option<&Team>,
    season: &Season,
) -> Result<Vec<DriverMarketCurvePoint>, String> {
    let arquivadas = load_market_archive_rows(conn, &driver.id)?;
    let salarios = salarios_por_temporada(conn, &driver.id, &arquivadas)?;
    // Uma leitura so do plantel para colorir a fita inteira. Por id, e nao por
    // nome, porque nome de equipe nao e chave — e uma consulta por temporada
    // seria uma consulta por ponto do grafico.
    //
    // A falha e engolida de proposito: a cor da casa e acabamento do chip, e uma
    // curva inteira que morre porque o plantel nao pode ser lido troca um grafico
    // menos bonito por grafico nenhum. Sem cores, os chips caem no neutro.
    let cores: HashMap<String, String> = crate::db::queries::teams::get_all_teams(conn)
        .unwrap_or_default()
        .into_iter()
        .map(|equipe| (equipe.id, equipe.cor_primaria))
        .collect();

    // Titulos, vitorias e podios sao ACUMULADOS ate aquela temporada — o modelo
    // le stats de carreira, e o snapshot guarda os do ano. Somar caminhando para
    // frente e o que reconstroi a carreira como ela era naquele ponto.
    let mut titulos = 0;
    let mut vitorias = 0;
    let mut podios = 0;
    // A categoria arrastada da ultima temporada que teve uma. Um snapshot sem
    // categoria cairia no baseline de fundo de escada (`_ => 20_000`) e
    // desenharia um piloto de $1,4M valendo $39k — o degrau que a curva tem que
    // recusar a inventar.
    let mut ultima_categoria: Option<String> = None;
    let mut pontos = Vec::with_capacity(arquivadas.len() + 1);

    for linha in &arquivadas {
        titulos += linha.titulos;
        vitorias += linha.vitorias;
        podios += linha.podios;

        if !linha.categoria.is_empty() {
            ultima_categoria = Some(linha.categoria.clone());
        }

        // Sem os atributos daquela temporada nao ha reconstrucao possivel — e
        // preencher com uma media inventaria uma linha reta que o jogador leria
        // como medicao. Temporada sem dado vira buraco, e a tela conta quantas.
        let avaliacao = linha.atributos.as_ref().map(|atributos| {
            avaliar_mercado(&EntradasDeMercado {
                categoria: ultima_categoria.as_deref(),
                skill: atributos.skill,
                midia: atributos.midia,
                desenvolvimento: atributos.desenvolvimento,
                titulos,
                vitorias,
                podios,
            })
        });
        let assinado = salarios.get(&linha.season_number);

        pontos.push(DriverMarketCurvePoint {
            season_number: linha.season_number,
            ano: linha.ano,
            categoria: linha.categoria.clone(),
            equipe_nome: assinado.map(|c| c.equipe_nome.clone()),
            equipe_cor: assinado.and_then(|c| cores.get(&c.equipe_id).cloned()),
            salario_contrato: assinado.map(|c| c.salario_anual),
            salario_mercado: avaliacao.as_ref().map(|value| value.salario),
            valor_mercado: avaliacao.as_ref().map(|value| value.valor),
            atual: false,
            futuro: false,
        });
    }

    // A temporada corrente ainda nao foi arquivada. Sem ela a curva pararia no
    // ano passado e o ultimo ponto nao seria o numero que o card ao lado mostra —
    // que e justamente a leitura que o grafico existe para dar contexto.
    let categoria_hoje = resolve_driver_category(driver, contract, team);

    if let Some(corrente) = pontos.iter_mut().find(|p| p.season_number == season.numero) {
        corrente.atual = true;
    } else {
        let categoria = categoria_hoje.clone();
        let avaliacao = avaliar_mercado(&EntradasDeMercado {
            categoria: categoria.as_deref().or(ultima_categoria.as_deref()),
            skill: driver.atributos.skill,
            midia: driver.atributos.midia,
            desenvolvimento: driver.atributos.desenvolvimento,
            titulos: driver.stats_carreira.titulos,
            vitorias: driver.stats_carreira.vitorias,
            podios: driver.stats_carreira.podios,
        });

        pontos.push(DriverMarketCurvePoint {
            season_number: season.numero,
            ano: season.ano,
            categoria: categoria.unwrap_or_default(),
            equipe_nome: contract.map(|c| c.equipe_nome.clone()),
            // A equipe de hoje vem resolvida pelo chamador; o mapa cobre o caso
            // de o contrato apontar para uma casa que nao e a do `team`.
            equipe_cor: contract
                .and_then(|c| cores.get(&c.equipe_id).cloned())
                .or_else(|| team.map(|t| t.cor_primaria.clone())),
            salario_contrato: contract.map(|c| c.salario_anual),
            salario_mercado: Some(avaliacao.salario),
            valor_mercado: Some(avaliacao.valor),
            atual: true,
            futuro: false,
        });
    }

    // O contrato assinado e fato, nao previsao: os anos que faltam dele ja tem
    // salario, equipe e categoria definidos, e ficavam de fora so porque a curva
    // parava no calendario. Um piloto de base e quase todo futuro — sem eles a
    // ficha de um rookie sao tres pontos num quadro dimensionado para doze.
    //
    // So a linha do salario avanca. O que o mercado pagaria em 2028 depende de
    // quem ele vai ser em 2028, e isso o modelo nao sabe: a laranja para no
    // presente, e essa parada e a leitura — dali para frente ha salario
    // contratado e nenhuma medida contra a qual compara-lo.
    if let Some(contrato) = contract {
        for numero in (season.numero + 1)..=contrato.temporada_fim {
            pontos.push(DriverMarketCurvePoint {
                season_number: numero,
                ano: season.ano + (numero - season.numero),
                categoria: categoria_hoje
                    .clone()
                    .or_else(|| ultima_categoria.clone())
                    .unwrap_or_default(),
                equipe_nome: Some(contrato.equipe_nome.clone()),
                equipe_cor: cores.get(&contrato.equipe_id).cloned(),
                salario_contrato: Some(contrato.salario_anual),
                salario_mercado: None,
                valor_mercado: None,
                atual: false,
                futuro: true,
            });
        }
    }

    Ok(pontos)
}

/// Uma temporada arquivada com o que o modelo de avaliacao consome.
///
/// Nao reaproveita `CareerSeasonArchiveRow` porque aquela linha e compartilhada
/// com o ranking do mundo e nao carrega atributos — e sao os atributos daquele
/// ano que fazem a reconstrucao ser reconstrucao, e nao o numero de hoje repetido
/// para tras.
struct LinhaDeMercado {
    season_number: i32,
    ano: i32,
    categoria: String,
    /// A equipe por quem ele CORREU naquela temporada, do snapshot. E o unico
    /// registro de quem de fato o alinhou: o contrato diz o que foi assinado, o
    /// arquivo diz o que foi cumprido — e quando um contrato e rescindido no meio
    /// da vigencia, os dois deixam de contar a mesma historia.
    ///
    /// `None` na temporada em que ele nao correu (sem standings, sem equipe) e
    /// tambem em save antigo, cujo snapshot nao guardava `team_id`.
    team_id: Option<String>,
    /// `None` quando o snapshot daquela temporada nao guardou atributos — save
    /// antigo, mundo gerado com arquivo enxuto, linha escrita por caminho que
    /// nao passa por `archive_driver_season`. Sem eles a temporada nao tem
    /// avaliacao, e nao ter e diferente de valer pouco.
    atributos: Option<AtributosArquivados>,
    titulos: u32,
    vitorias: u32,
    podios: u32,
}

struct AtributosArquivados {
    skill: f64,
    midia: f64,
    desenvolvimento: f64,
}

fn load_market_archive_rows(
    conn: &Connection,
    piloto_id: &str,
) -> Result<Vec<LinhaDeMercado>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT season_number, ano, categoria, snapshot_json
             FROM driver_season_archive
             WHERE piloto_id = ?1
             ORDER BY season_number ASC",
        )
        .map_err(|e| format!("Falha ao preparar curva de mercado: {e}"))?;

    let linhas = stmt
        .query_map(rusqlite::params![piloto_id], |row| {
            let snapshot_json: String = row.get(3)?;
            let snapshot: serde_json::Value =
                serde_json::from_str(&snapshot_json).unwrap_or_default();
            // `skill` e o campo que decide se ha atributos: os outros dois tem
            // default sensato, ele nao — e um piloto sem skill arquivada nao e um
            // piloto mediano, e um piloto sobre o qual nao se sabe nada.
            let atributos = snapshot.get("atributos").and_then(|bloco| {
                let numero = |nome: &str, padrao: f64| {
                    bloco
                        .get(nome)
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(padrao)
                };
                bloco
                    .get("skill")
                    .and_then(serde_json::Value::as_f64)
                    .map(|skill| AtributosArquivados {
                        skill,
                        midia: numero("midia", 50.0),
                        desenvolvimento: numero("desenvolvimento", 50.0),
                    })
            });
            let inteiro = |nome: &str| {
                snapshot
                    .get(nome)
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0)
                    .max(0) as u32
            };
            let coluna_categoria: String = row.get(2)?;
            let categoria = snapshot
                .get("categoria")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(coluna_categoria.as_str());
            // A CLASSE junto, pela mesma chave de divisao do resto do dossie.
            //
            // Sem ela a curva era o unico lugar da ficha que lia o arquivo pela
            // categoria crua: a trilha do rodape pintava "Production" nas
            // temporadas passadas e "BMW Production" na atual — a mesma
            // Production aparecendo como duas na legenda, como se o piloto
            // tivesse trocado de campeonato ao assinar a renovacao. Production e
            // Endurance nao sao um campeonato so; a classe E a divisao.
            let classe = snapshot
                .get("classe")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());

            let team_id = snapshot
                .get("team_id")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);

            Ok(LinhaDeMercado {
                season_number: row.get(0)?,
                ano: row.get(1)?,
                categoria: timeline_division_key(categoria, classe).unwrap_or_default(),
                team_id,
                atributos,
                titulos: inteiro("titulos"),
                vitorias: inteiro("vitorias"),
                podios: inteiro("podios"),
            })
        })
        .map_err(|e| format!("Falha ao ler curva de mercado: {e}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| format!("Falha ao mapear curva de mercado: {e}"))?;

    Ok(linhas)
}

struct SalarioAssinado {
    salario_anual: f64,
    equipe_nome: String,
    /// Guardado ao lado do nome porque e por ELE que a cor da casa e resolvida:
    /// nome de equipe nao e chave, e duas casas podem ter passado pelo mesmo
    /// nome ao longo de uma carreira longa.
    equipe_id: String,
}

/// Espalha cada contrato pelas temporadas que ele cobre — pela vigencia que ele
/// de fato teve, e nao pela que foi assinada.
///
/// Um contrato e uma linha com vigencia, e a curva e por temporada: sem
/// espalhar, um contrato de tres anos viraria um ponto solto e dois buracos.
///
/// Mas espalhar pela vigencia CRUA inventa anos. Contrato rescindido nao tem a
/// janela encurtada nos saves ja gravados, entao ele segue marcado ate o ultimo
/// ano assinado — e a curva desenhava o piloto recebendo $1M de uma equipe que
/// ele havia deixado duas temporadas antes, num ano em que nao correu por
/// ninguem. Num save de 28 temporadas eram 646 celulas (piloto, temporada)
/// pintadas assim.
///
/// Duas regras cortam a vigencia, e as duas leem fato, nao intencao:
///
/// 1. **O proximo contrato manda.** Ninguem corre por duas equipes no mesmo ano;
///    onde duas vigencias se sobrepoem, a anterior acaba onde a seguinte comeca.
///    Antes o contrato mais recente so vencia NAS temporadas sobrepostas —
///    passada a janela dele, o antigo voltava a pintar sozinho.
/// 2. **Rescindido nao passa do que foi cumprido.** O arquivo diz por quem ele
///    correu: um contrato rescindido vale ate a ultima temporada da vigencia em
///    que o `team_id` arquivado e o da equipe do contrato. Se nunca bateu, ele
///    foi assinado e desfeito sem uma corrida, e nao pinta ano nenhum.
///
/// Contrato `Expirado` fica fora da regra 2 de proposito: ano contratado sem
/// correr existe (lesao, banco, equipe que saiu do grid), e apaga-lo seria
/// trocar um erro por outro.
fn salarios_por_temporada(
    conn: &Connection,
    piloto_id: &str,
    arquivadas: &[LinhaDeMercado],
) -> Result<HashMap<i32, SalarioAssinado>, String> {
    let contratos = crate::db::queries::contracts::get_contracts_for_pilot(conn, piloto_id)
        .map_err(|e| format!("Falha ao carregar contratos do piloto: {e}"))?;

    let mut por_temporada: HashMap<i32, SalarioAssinado> = HashMap::new();
    // Do mais antigo para o mais novo: quando duas vigencias se sobrepoem (troca
    // no meio da janela), quem vale para a temporada e o contrato mais recente.
    let mut ordenados: Vec<&Contract> = contratos.iter().collect();
    ordenados.sort_by(|a, b| {
        (a.temporada_inicio, &a.created_at).cmp(&(b.temporada_inicio, &b.created_at))
    });

    // Save antigo nao guardava `team_id` no snapshot. Sem esse campo a regra 2
    // nao distingue "nunca correu por essa equipe" de "o arquivo nao sabe", e
    // apagaria a carreira inteira de quem teve contrato rescindido.
    let arquivo_sabe_a_equipe = arquivadas.iter().any(|linha| linha.team_id.is_some());

    for (indice, contrato) in ordenados.iter().enumerate() {
        let mut fim = contrato.temporada_fim;

        if let Some(proximo) = ordenados.get(indice + 1) {
            fim = fim.min(proximo.temporada_inicio - 1);
        }

        if arquivo_sabe_a_equipe && contrato.status == ContractStatus::Rescindido {
            let cumprido = arquivadas
                .iter()
                .filter(|linha| {
                    linha.season_number >= contrato.temporada_inicio
                        && linha.season_number <= contrato.temporada_fim
                        && linha.team_id.as_deref() == Some(contrato.equipe_id.as_str())
                })
                .map(|linha| linha.season_number)
                .max();
            match cumprido {
                Some(ultima) => fim = fim.min(ultima),
                None => continue,
            }
        }

        for temporada in contrato.temporada_inicio..=fim {
            por_temporada.insert(
                temporada,
                SalarioAssinado {
                    salario_anual: contrato.salario_anual,
                    equipe_nome: contrato.equipe_nome.clone(),
                    equipe_id: contrato.equipe_id.clone(),
                },
            );
        }
    }

    Ok(por_temporada)
}
