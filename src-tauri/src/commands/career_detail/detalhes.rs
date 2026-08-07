//! O detalhe que abre no hover de cada numero do dossie de carreira.
//!
//! O dossie e uma parede de contadores: "4 equipes", "2 promocoes", "19 poles",
//! "74a corrida". Cada um deles esconde a resposta que o jogador realmente quer —
//! QUAIS equipes, de qual para qual, por qual time, em que dia. Este modulo
//! devolve essa resposta como um mapa `chave da linha -> linhas do detalhe`.
//!
//! Tudo sai das MESMAS duas leituras que o resto do dossie ja faz (o arquivo de
//! temporadas e o corrida-a-corrida), mais um punhado de consultas pequenas para
//! nomes de equipe, companheiros e lesoes. Nada aqui abre uma varredura nova do
//! mundo.

use rusqlite::OptionalExtension;

use super::*;

type Detalhes = HashMap<String, Vec<DriverCareerDetailEntry>>;

pub(super) fn build_career_details(
    conn: &Connection,
    driver_id: &str,
    seasons: &[CareerSeasonArchiveRow],
    races: &[CareerRaceHistoryRow],
    active_seasons: &[&CareerSeasonArchiveRow],
    auge: &DriverCareerPeakBlock,
    queda: &DriverCareerDroughtBlock,
    confiabilidade: &DriverCareerReliabilityBlock,
    duelos: &DriverCareerTeammateBlock,
) -> Result<Detalhes, String> {
    let equipes = load_team_identities(conn, races)?;
    let divisao_por_temporada: HashMap<i32, String> = seasons
        .iter()
        .filter_map(|season| {
            timeline_division_key(&season.categoria, season.classe.as_deref())
                .map(|chave| (season.season_number, chave))
        })
        .collect();

    let mut detalhes: Detalhes = HashMap::new();
    detalhes.insert("categorias".into(), categorias(active_seasons));
    let passagens = passagens_por_equipe(races);
    detalhes.insert("equipes".into(), linhas_de_equipe(&passagens, &equipes));
    // A mesma lista, da passagem mais longa para a mais curta. E a media
    // decomposta: da para ver se o "2,5 anos" vem de estabilidade ou de um
    // vinculo longo carregando tres curtos.
    let mut por_duracao = passagens.clone();
    por_duracao.sort_by(|a, b| {
        (b.2 - b.1)
            .cmp(&(a.2 - a.1))
            .then_with(|| b.3.cmp(&a.3))
            .then_with(|| a.1.cmp(&b.1))
    });
    detalhes.insert(
        "tempo_medio_por_equipe".into(),
        linhas_de_equipe(&por_duracao, &equipes),
    );
    let (promocoes, rebaixamentos) = mobilidade(active_seasons);
    detalhes.insert("promocoes".into(), promocoes);
    detalhes.insert("rebaixamentos".into(), rebaixamentos);

    // A carreira inteira em duas linhas: a estreia e a ultima corrida. Sem elas o
    // "17 anos" nao diz nem onde comecou nem onde parou.
    if let (Some(estreia), Some(ultima)) = (races.first(), races.last()) {
        let mut pontas = vec![linha_de_corrida(estreia, &equipes, &divisao_por_temporada)];
        if ultima.race_index != estreia.race_index {
            pontas.push(linha_de_corrida(ultima, &equipes, &divisao_por_temporada));
        }
        detalhes.insert("tempo_carreira".into(), pontas);
    }

    detalhes.insert(
        "temporadas".into(),
        active_seasons
            .iter()
            .map(|season| linha_de_temporada(season, &equipes))
            .collect(),
    );
    detalhes.insert(
        "temporadas_no_top3".into(),
        active_seasons
            .iter()
            .filter(|season| matches!(season.posicao_campeonato, Some(posicao) if posicao <= 3))
            .map(|season| linha_de_temporada(season, &equipes))
            .collect(),
    );
    detalhes.insert(
        "temporadas_sem_podio".into(),
        active_seasons
            .iter()
            .filter(|season| season.podios == 0)
            .map(|season| {
                let mut linha = linha_de_temporada(season, &equipes);
                linha.melhor_resultado = melhor_chegada(races, season.season_number);
                linha
            })
            .collect(),
    );
    detalhes.insert(
        "anos_parados".into(),
        anos_parados(seasons, races, &equipes),
    );
    let fatias = fatias_por_equipe_e_divisao(races, &divisao_por_temporada);
    detalhes.insert("grid_medio".into(), grid_por_divisao(&fatias, &equipes));
    detalhes.insert(
        "taxa_abandono".into(),
        abandono_por_divisao(&fatias, &equipes),
    );

    for (chave, corrida) in [
        (
            "primeira_vitoria",
            races.iter().find(|r| !r.is_dnf && r.position == 1),
        ),
        (
            "primeiro_podio",
            races.iter().find(|r| !r.is_dnf && r.position <= 3),
        ),
        ("primeiro_dnf", races.iter().find(|r| r.is_dnf)),
    ] {
        if let Some(corrida) = corrida {
            detalhes.insert(
                chave.into(),
                vec![linha_de_corrida(corrida, &equipes, &divisao_por_temporada)],
            );
        }
    }

    if let Some(titulo) = seasons
        .iter()
        .find(|season| archived_season_is_title(season))
    {
        detalhes.insert(
            "primeiro_titulo".into(),
            vec![linha_de_temporada(titulo, &equipes)],
        );
    }

    // Os tres numeros de conquista dos cards de carreira, abertos ano a ano.
    // Eles nao pertencem ao dossie — sao os cards grandes do topo do Historico —
    // mas saem das MESMAS duas leituras, entao moram no mesmo mapa.
    for (chave, condicao) in [("vitorias", Condicao::Vitoria), ("podios", Condicao::Podio)] {
        detalhes.insert(
            chave.into(),
            conquistas_por_temporada(races, condicao, &equipes, &divisao_por_temporada),
        );
    }
    // Titulo e coisa de temporada FECHADA: nao existe campeao no meio do ano, e
    // por isso este e o unico dos tres que sai do arquivo e nao do corrida-a-corrida.
    detalhes.insert(
        "titulos".into(),
        seasons
            .iter()
            .filter(|season| archived_season_is_title(season))
            .map(|season| {
                let mut linha = linha_de_temporada(season, &equipes);
                // Numa lista de titulos toda linha e P1 por definicao — uma
                // coluna de "P1" repetido nao acrescenta nada. A campanha do ano
                // (`resumo`) e que da tamanho a cada um deles.
                linha.texto = None;
                linha
            })
            .collect(),
    );
    if let Some(melhor) = temporada_por_ano(
        active_seasons,
        auge.melhor_temporada.as_ref().map(|s| s.ano),
    ) {
        detalhes.insert(
            "melhor_temporada".into(),
            vec![linha_de_temporada(melhor, &equipes)],
        );
    }
    if let Some(pior) =
        temporada_por_ano(active_seasons, queda.pior_temporada.as_ref().map(|s| s.ano))
    {
        detalhes.insert(
            "pior_temporada".into(),
            vec![linha_de_temporada(pior, &equipes)],
        );
    }

    // Cada sequencia devolve as equipes que ela atravessou. Uma sequencia de 46
    // podios que cruza tres equipes conta uma historia bem diferente de uma que
    // cabe inteira numa so.
    for (chave, tamanho, inicio, fim, condicao) in [
        (
            "sequencia_vitorias",
            auge.maior_sequencia_vitorias,
            auge.sequencia_ano_inicio,
            auge.sequencia_ano_fim,
            Condicao::Vitoria,
        ),
        (
            "sequencia_podios",
            auge.maior_sequencia_podios,
            auge.sequencia_podios_ano_inicio,
            auge.sequencia_podios_ano_fim,
            Condicao::Podio,
        ),
        (
            "jejum_vitorias",
            queda.maior_seca_vitorias,
            queda.seca_ano_inicio,
            queda.seca_ano_fim,
            Condicao::SemVitoria,
        ),
        (
            "jejum_podios",
            queda.maior_seca_podios,
            queda.seca_podios_ano_inicio,
            queda.seca_podios_ano_fim,
            Condicao::SemPodio,
        ),
        (
            "sequencia_chegadas",
            confiabilidade.maior_sequencia_chegadas,
            None,
            None,
            Condicao::Chegada,
        ),
    ] {
        let linhas = equipes_da_sequencia(
            races,
            tamanho,
            inicio,
            fim,
            condicao,
            &equipes,
            &divisao_por_temporada,
        );
        if !linhas.is_empty() {
            detalhes.insert(chave.into(), linhas);
        }
    }

    for (chave, filtro) in [
        ("poles", Filtro::Pole),
        ("poles_convertidas", Filtro::PoleConvertida),
        ("voltas_rapidas", Filtro::VoltaRapida),
        ("abandonos", Filtro::Abandono),
    ] {
        let linhas = agrupado_por_equipe_e_divisao(races, filtro, &equipes, &divisao_por_temporada);
        if !linhas.is_empty() {
            detalhes.insert(chave.into(), linhas);
        }
    }

    if duelos.companheiros > 0 {
        let (companheiros, vencidas, rival) =
            detalhe_dos_duelos(conn, driver_id, &equipes, &divisao_por_temporada)?;
        detalhes.insert("companheiros".into(), companheiros);
        detalhes.insert("temporadas_vencidas".into(), vencidas);
        detalhes.insert("rival_mais_duro".into(), rival);
    }
    // Uma chave por gravidade, e nao uma lista so nas tres linhas: "Leves 2"
    // abrindo a lesao grave de 2019 junto e a linha mentindo sobre o proprio
    // numero. O agrupamento segue o mesmo de `count_injuries_by_severity`, que e
    // quem escreve os contadores — `Critica` conta como grave nos dois lugares.
    let lesoes = detalhe_das_lesoes(conn, driver_id, &equipes)?;
    for (chave, gravidades) in [
        ("lesoes_leves", &["Leve"][..]),
        ("lesoes_moderadas", &["Moderada"][..]),
        ("lesoes_graves", &["Grave", "Critica"][..]),
    ] {
        let filtradas: Vec<DriverCareerDetailEntry> = lesoes
            .iter()
            .filter(|lesao| {
                lesao
                    .resumo
                    .as_deref()
                    .is_some_and(|tipo| gravidades.contains(&tipo))
            })
            .cloned()
            .collect();
        detalhes.insert(chave.into(), filtradas);
    }

    detalhes.retain(|_, linhas| !linhas.is_empty());
    Ok(detalhes)
}

// ── Blocos ───────────────────────────────────────────────────────────────────

fn categorias(active_seasons: &[&CareerSeasonArchiveRow]) -> Vec<DriverCareerDetailEntry> {
    let mut por_divisao: Vec<(String, i32, i32, i32)> = Vec::new();
    for season in active_seasons {
        let Some(chave) = timeline_division_key(&season.categoria, season.classe.as_deref()) else {
            continue;
        };
        match por_divisao.iter_mut().find(|(atual, ..)| *atual == chave) {
            Some(entrada) => {
                entrada.1 = entrada.1.min(season.ano);
                entrada.2 = entrada.2.max(season.ano);
                entrada.3 += season.corridas;
            }
            None => por_divisao.push((chave, season.ano, season.ano, season.corridas)),
        }
    }
    por_divisao.sort_by_key(|(_, inicio, ..)| *inicio);
    por_divisao
        .into_iter()
        .map(
            |(categoria, inicio, fim, corridas)| DriverCareerDetailEntry {
                categoria: Some(categoria),
                periodo: Some(faixa(inicio, fim)),
                contagem: Some(corridas),
                ..Default::default()
            },
        )
        .collect()
}

/// Uma passagem por equipe: `(id, primeiro ano, ultimo ano, corridas)`.
type Passagem = (String, i32, i32, i32);

fn passagens_por_equipe(races: &[CareerRaceHistoryRow]) -> Vec<Passagem> {
    let mut por_equipe: Vec<Passagem> = Vec::new();
    for race in races.iter().filter(|race| race.team_id != "-") {
        match por_equipe.iter_mut().find(|(id, ..)| *id == race.team_id) {
            Some(entrada) => {
                entrada.1 = entrada.1.min(race.ano);
                entrada.2 = entrada.2.max(race.ano);
                entrada.3 += 1;
            }
            None => por_equipe.push((race.team_id.clone(), race.ano, race.ano, 1)),
        }
    }
    por_equipe.sort_by_key(|(_, inicio, ..)| *inicio);
    por_equipe
}

fn linhas_de_equipe(
    passagens: &[Passagem],
    equipes: &HashMap<String, (String, String)>,
) -> Vec<DriverCareerDetailEntry> {
    passagens
        .iter()
        .map(|(id, inicio, fim, corridas)| {
            let (nome, cor, id_equipe) = identidade(equipes, id);
            DriverCareerDetailEntry {
                equipe: nome,
                equipe_cor: cor,
                equipe_id: id_equipe,
                periodo: Some(faixa(*inicio, *fim)),
                contagem: Some(*corridas),
                ..Default::default()
            }
        })
        .collect()
}

/// Os anos sem assento, cada um com a equipe de onde ele saiu e o ano em que
/// voltou. Um buraco na carreira so significa alguma coisa com as duas pontas:
/// "2021" e um numero, "saiu da Ferrari em 2020, voltou em 2022" e uma queda.
fn anos_parados(
    seasons: &[CareerSeasonArchiveRow],
    races: &[CareerRaceHistoryRow],
    equipes: &HashMap<String, (String, String)>,
) -> Vec<DriverCareerDetailEntry> {
    let parado = |season: &CareerSeasonArchiveRow| {
        season.corridas == 0 && season.categoria.trim().is_empty()
    };

    let mut linhas = Vec::new();
    let mut inicio: Option<i32> = None;
    let mut fim = 0;
    for (indice, season) in seasons.iter().enumerate() {
        if parado(season) {
            inicio.get_or_insert(season.ano);
            fim = season.ano;
            if indice + 1 < seasons.len() {
                continue;
            }
        }
        let Some(primeiro) = inicio.take() else {
            continue;
        };
        // A ultima corrida ANTES do buraco: e ela que diz de onde ele saiu.
        let ultima = races.iter().rev().find(|race| race.ano < primeiro);
        let (equipe, cor, equipe_id) = ultima
            .map(|race| identidade(equipes, &race.team_id))
            .unwrap_or((None, None, None));
        let volta = seasons
            .iter()
            .find(|outra| outra.ano > fim && !parado(outra))
            .map(|outra| outra.ano);
        linhas.push(DriverCareerDetailEntry {
            periodo: Some(faixa(primeiro, fim)),
            equipe,
            equipe_cor: cor,
            equipe_id,
            categoria: ultima.and_then(|race| race.categoria.clone()),
            contagem: Some(fim - primeiro + 1),
            resumo: volta.map(|ano| format!("→ {ano}")),
            ..Default::default()
        });
    }
    linhas
}

/// Uma fatia da carreira: o que ele fez numa divisao por uma equipe.
///
/// O recorte e equipe + divisao, e nao so divisao, pela mesma razao das poles: o
/// grid medio de 71 corridas de GT3 nao diz nada se elas foram por tres equipes
/// diferentes, e a linha sem equipe fica sem logo no meio de uma lista que tem.
#[derive(Default)]
struct FatiaDaCarreira {
    equipe_id: String,
    divisao: Option<String>,
    inicio: i32,
    fim: i32,
    corridas: i32,
    largadas: i32,
    soma_grid: i32,
    abandonos: i32,
}

fn fatias_por_equipe_e_divisao(
    races: &[CareerRaceHistoryRow],
    divisoes: &HashMap<i32, String>,
) -> Vec<FatiaDaCarreira> {
    let mut fatias: Vec<FatiaDaCarreira> = Vec::new();
    for race in races {
        let divisao = divisao_da_corrida(race, divisoes);
        let indice = fatias
            .iter()
            .position(|fatia| fatia.equipe_id == race.team_id && fatia.divisao == divisao)
            .unwrap_or_else(|| {
                fatias.push(FatiaDaCarreira {
                    equipe_id: race.team_id.clone(),
                    divisao,
                    inicio: race.ano,
                    fim: race.ano,
                    ..Default::default()
                });
                fatias.len() - 1
            });
        let fatia = &mut fatias[indice];
        fatia.inicio = fatia.inicio.min(race.ano);
        fatia.fim = fatia.fim.max(race.ano);
        fatia.corridas += 1;
        // Grid `0` e corrida sem registro de largada, e nao a pole: entra no
        // denominador de nada.
        if race.grid_position > 0 {
            fatia.largadas += 1;
            fatia.soma_grid += race.grid_position;
        }
        if race.is_dnf {
            fatia.abandonos += 1;
        }
    }
    fatias.sort_by_key(|fatia| fatia.inicio);
    fatias
}

/// O grid medio quebrado por equipe e divisao. O numero unico da ficha mistura o
/// rookie que largava em 12o com o veterano que largava em 2o; aqui da para ver
/// onde — e por quem — ele era rapido no sabado.
fn grid_por_divisao(
    fatias: &[FatiaDaCarreira],
    equipes: &HashMap<String, (String, String)>,
) -> Vec<DriverCareerDetailEntry> {
    fatias
        .iter()
        .filter(|fatia| fatia.largadas > 0)
        .map(|fatia| DriverCareerDetailEntry {
            contagem: Some(fatia.largadas),
            resumo: Some(format!(
                "P{:.1}",
                fatia.soma_grid as f64 / fatia.largadas as f64
            )),
            ..linha_de_fatia(fatia, equipes)
        })
        .collect()
}

/// A taxa de abandono por equipe e divisao, com o numerador e o denominador a
/// vista — sem eles "4,3%" nao diz se sao 3 em 70 ou 1 em 23.
fn abandono_por_divisao(
    fatias: &[FatiaDaCarreira],
    equipes: &HashMap<String, (String, String)>,
) -> Vec<DriverCareerDetailEntry> {
    fatias
        .iter()
        .filter(|fatia| fatia.corridas > 0)
        .map(|fatia| DriverCareerDetailEntry {
            contagem: Some(fatia.corridas),
            resumo: Some(format!(
                "{}/{} · {:.1}%",
                fatia.abandonos,
                fatia.corridas,
                fatia.abandonos as f64 * 100.0 / fatia.corridas as f64
            )),
            ..linha_de_fatia(fatia, equipes)
        })
        .collect()
}

fn linha_de_fatia(
    fatia: &FatiaDaCarreira,
    equipes: &HashMap<String, (String, String)>,
) -> DriverCareerDetailEntry {
    let (equipe, cor, equipe_id) = identidade(equipes, &fatia.equipe_id);
    DriverCareerDetailEntry {
        equipe,
        equipe_cor: cor,
        equipe_id,
        categoria: fatia.divisao.clone(),
        periodo: Some(faixa(fatia.inicio, fatia.fim)),
        ..Default::default()
    }
}

/// A melhor chegada de uma temporada. Abandono nao e chegada.
fn melhor_chegada(races: &[CareerRaceHistoryRow], season_number: i32) -> Option<i32> {
    races
        .iter()
        .filter(|race| race.season_number == season_number && !race.is_dnf && race.position > 0)
        .map(|race| race.position)
        .min()
}

/// As trocas de degrau, cada uma com de-onde, para-onde e o ano.
fn mobilidade(
    active_seasons: &[&CareerSeasonArchiveRow],
) -> (Vec<DriverCareerDetailEntry>, Vec<DriverCareerDetailEntry>) {
    let mut promocoes = Vec::new();
    let mut rebaixamentos = Vec::new();
    let mut anterior: Option<(&str, u8)> = None;

    for season in active_seasons {
        let Some(config) = categories::get_category_config(&season.categoria) else {
            continue;
        };
        if let Some((categoria_anterior, tier_anterior)) = anterior {
            if config.tier != tier_anterior {
                let linha = DriverCareerDetailEntry {
                    categoria_origem: Some(categoria_anterior.to_string()),
                    categoria: Some(season.categoria.clone()),
                    periodo: Some(season.ano.to_string()),
                    ..Default::default()
                };
                if config.tier > tier_anterior {
                    promocoes.push(linha);
                } else {
                    rebaixamentos.push(linha);
                }
            }
        }
        anterior = Some((season.categoria.as_str(), config.tier));
    }

    (promocoes, rebaixamentos)
}

#[derive(Clone, Copy)]
enum Condicao {
    Vitoria,
    Podio,
    SemVitoria,
    SemPodio,
    Chegada,
}

impl Condicao {
    fn casa(self, race: &CareerRaceHistoryRow) -> bool {
        match self {
            Condicao::Vitoria => !race.is_dnf && race.position == 1,
            Condicao::Podio => !race.is_dnf && race.position <= 3,
            Condicao::SemVitoria => race.is_dnf || race.position != 1,
            Condicao::SemPodio => race.is_dnf || race.position > 3,
            Condicao::Chegada => !race.is_dnf,
        }
    }
}

/// As equipes que a sequencia atravessou.
///
/// Reencontra a sequencia percorrendo as corridas de novo e ficando com a
/// PRIMEIRA que bate no tamanho e no intervalo de anos — a mesma regra de
/// desempate do calculo original, que fica com a primeira ocorrencia.
fn equipes_da_sequencia(
    races: &[CareerRaceHistoryRow],
    tamanho: i32,
    ano_inicio: Option<i32>,
    ano_fim: Option<i32>,
    condicao: Condicao,
    equipes: &HashMap<String, (String, String)>,
    divisoes: &HashMap<i32, String>,
) -> Vec<DriverCareerDetailEntry> {
    if tamanho <= 0 {
        return Vec::new();
    }
    let mut atual: Vec<&CareerRaceHistoryRow> = Vec::new();
    let mut encontrada: Option<Vec<&CareerRaceHistoryRow>> = None;
    for race in races {
        if condicao.casa(race) {
            atual.push(race);
            if atual.len() as i32 == tamanho && encontrada.is_none() {
                let bate_inicio = ano_inicio.is_none_or(|ano| atual[0].ano == ano);
                let bate_fim = ano_fim.is_none_or(|ano| race.ano == ano);
                if bate_inicio && bate_fim {
                    encontrada = Some(atual.clone());
                }
            }
        } else {
            atual.clear();
        }
    }
    let Some(sequencia) = encontrada else {
        return Vec::new();
    };

    let mut por_equipe: Vec<(String, i32, i32, i32, Option<String>)> = Vec::new();
    for race in sequencia {
        let divisao = divisoes.get(&race.season_number).cloned();
        match por_equipe.iter_mut().find(|(id, ..)| *id == race.team_id) {
            Some(entrada) => {
                entrada.1 = entrada.1.min(race.ano);
                entrada.2 = entrada.2.max(race.ano);
                entrada.3 += 1;
            }
            None => por_equipe.push((race.team_id.clone(), race.ano, race.ano, 1, divisao)),
        }
    }
    por_equipe
        .into_iter()
        .map(|(id, inicio, fim, corridas, divisao)| {
            let (nome, cor, id_equipe) = identidade(equipes, &id);
            DriverCareerDetailEntry {
                equipe: nome,
                equipe_cor: cor,
                equipe_id: id_equipe,
                categoria: divisao,
                periodo: Some(faixa(inicio, fim)),
                contagem: Some(corridas),
                ..Default::default()
            }
        })
        .collect()
}

#[derive(Clone, Copy)]
enum Filtro {
    Pole,
    PoleConvertida,
    VoltaRapida,
    Abandono,
}

impl Filtro {
    fn casa(self, race: &CareerRaceHistoryRow) -> bool {
        match self {
            Filtro::Pole => race.grid_position == 1,
            Filtro::PoleConvertida => race.grid_position == 1 && !race.is_dnf && race.position == 1,
            Filtro::VoltaRapida => race.has_fastest_lap,
            Filtro::Abandono => race.is_dnf,
        }
    }

    /// O abandono vale mais corrida a corrida — data e pista dizem mais que
    /// "3 na McLaren". As marcas de sabado sao muitas e viram uma lista longa
    /// demais sem agrupar.
    fn por_corrida(self) -> bool {
        matches!(self, Filtro::Abandono)
    }
}

fn agrupado_por_equipe_e_divisao(
    races: &[CareerRaceHistoryRow],
    filtro: Filtro,
    equipes: &HashMap<String, (String, String)>,
    divisoes: &HashMap<i32, String>,
) -> Vec<DriverCareerDetailEntry> {
    let casos: Vec<&CareerRaceHistoryRow> = races.iter().filter(|race| filtro.casa(race)).collect();
    if filtro.por_corrida() {
        return casos
            .into_iter()
            .map(|race| linha_de_corrida(race, equipes, divisoes))
            .collect();
    }

    let mut agrupado: Vec<(String, Option<String>, i32, i32, i32)> = Vec::new();
    for race in casos {
        let divisao = divisao_da_corrida(race, divisoes);
        match agrupado
            .iter_mut()
            .find(|(id, atual, ..)| *id == race.team_id && *atual == divisao)
        {
            Some(entrada) => {
                entrada.2 = entrada.2.min(race.ano);
                entrada.3 = entrada.3.max(race.ano);
                entrada.4 += 1;
            }
            None => agrupado.push((race.team_id.clone(), divisao, race.ano, race.ano, 1)),
        }
    }
    agrupado.sort_by_key(|(_, _, inicio, ..)| *inicio);
    agrupado
        .into_iter()
        .map(|(id, divisao, inicio, fim, total)| {
            let (nome, cor, id_equipe) = identidade(equipes, &id);
            DriverCareerDetailEntry {
                equipe: nome,
                equipe_cor: cor,
                equipe_id: id_equipe,
                categoria: divisao,
                periodo: Some(faixa(inicio, fim)),
                contagem: Some(total),
                ..Default::default()
            }
        })
        .collect()
}

/// Vitorias e podios ANO A ANO: em que temporada vieram, por qual equipe e em
/// qual divisao, com quantos vieram ali.
///
/// O agrupamento e por TEMPORADA, e nao por equipe como as marcas de sabado: o
/// que o card de vitorias esconde e justamente o QUANDO, e uma linha unica
/// "2014-2023 · 18" devolve a equipe mas engole o ano. Por temporada tambem tem
/// teto — sao os anos de carreira, nunca as duzentas corridas.
///
/// A equipe entra na chave junto com o ano: quem trocou de equipe no meio do ano
/// ganha duas linhas naquela temporada, que e a leitura honesta de por quem cada
/// vitoria foi conquistada.
///
/// A fonte e o corrida-a-corrida, e nao o arquivo de temporadas: o arquivo so
/// existe para temporada FECHADA, e a vitoria de domingo passado — a que acabou
/// de subir o numero do card — ficaria contada no card e ausente do hover.
///
/// A colocacao no campeonato daquele ano fica de FORA, embora esteja a mao. Ela
/// vinha como "P1" no comeco da linha e, numa lista de vitorias, se lia como a
/// posicao de chegada — dizendo o obvio. Na lista de podios era pior: "P10" ao
/// lado de um podio nao e ambiguo, e simplesmente errado aos olhos de quem le.
fn conquistas_por_temporada(
    races: &[CareerRaceHistoryRow],
    condicao: Condicao,
    equipes: &HashMap<String, (String, String)>,
    divisoes: &HashMap<i32, String>,
) -> Vec<DriverCareerDetailEntry> {
    let mut agrupado: Vec<(i32, i32, String, Option<String>, i32)> = Vec::new();
    for race in races.iter().filter(|race| condicao.casa(race)) {
        let divisao = divisao_da_corrida(race, divisoes);
        match agrupado.iter_mut().find(|(temporada, _, id, atual, _)| {
            *temporada == race.season_number && *id == race.team_id && *atual == divisao
        }) {
            Some(entrada) => entrada.4 += 1,
            None => agrupado.push((
                race.season_number,
                race.ano,
                race.team_id.clone(),
                divisao,
                1,
            )),
        }
    }
    agrupado.sort_by_key(|(temporada, ano, ..)| (*ano, *temporada));
    agrupado
        .into_iter()
        .map(|(_, ano, id, divisao, total)| {
            let (nome, cor, id_equipe) = identidade(equipes, &id);
            DriverCareerDetailEntry {
                // Corrida orfa (sem temporada ligada) vem com ano 0: melhor uma
                // linha sem periodo do que um "0" ao lado do nome da equipe.
                periodo: (ano > 0).then(|| ano.to_string()),
                categoria: divisao,
                equipe: nome,
                equipe_cor: cor,
                equipe_id: id_equipe,
                contagem: Some(total),
                ..Default::default()
            }
        })
        .collect()
}

/// Companheiros com a identidade de HOJE (equipe atual, idade, nacionalidade);
/// a parte, as temporadas de confronto vencidas; e, por ultimo, o ano a ano
/// contra o rival mais duro — o "6-2" da ficha virado historia.
fn detalhe_dos_duelos(
    conn: &Connection,
    driver_id: &str,
    equipes: &HashMap<String, (String, String)>,
    divisoes: &HashMap<i32, String>,
) -> Result<
    (
        Vec<DriverCareerDetailEntry>,
        Vec<DriverCareerDetailEntry>,
        Vec<DriverCareerDetailEntry>,
    ),
    String,
> {
    let mut stmt = conn
        .prepare(
            "SELECT
                CASE WHEN t.piloto_1_id = ?1 THEN t.piloto_2_id ELSE t.piloto_1_id END AS outro,
                t.ano,
                COALESCE(meu.pontos, 0),
                COALESCE(dele.pontos, 0),
                COALESCE(dele.nome, ''),
                COALESCE(t.team_id, ''),
                t.season_number
             FROM team_season_archive t
             LEFT JOIN driver_season_archive meu
                    ON meu.piloto_id = ?1 AND meu.season_number = t.season_number
             LEFT JOIN driver_season_archive dele
                    ON dele.piloto_id = CASE WHEN t.piloto_1_id = ?1
                                             THEN t.piloto_2_id ELSE t.piloto_1_id END
                   AND dele.season_number = t.season_number
             WHERE (t.piloto_1_id = ?1 OR t.piloto_2_id = ?1)
             ORDER BY t.season_number ASC",
        )
        .map_err(|e| format!("Falha ao preparar detalhe dos duelos: {e}"))?;
    let mapped = stmt
        .query_map(rusqlite::params![driver_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i32>(6)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar detalhe dos duelos: {e}"))?;

    let mut por_companheiro: Vec<(String, String, i32, i32, i32)> = Vec::new();
    let mut vencidas = Vec::new();
    let mut ano_a_ano: Vec<(String, DriverCareerDetailEntry)> = Vec::new();
    for row in mapped {
        let (outro, ano, meus, dele, nome, team_id, season_number) =
            row.map_err(|e| format!("Falha ao ler detalhe de duelo: {e}"))?;
        let Some(outro) = outro.filter(|id| !id.trim().is_empty() && id != driver_id) else {
            continue;
        };
        match por_companheiro.iter_mut().find(|(id, ..)| *id == outro) {
            Some(entrada) => entrada.2 += 1,
            None => por_companheiro.push((outro.clone(), nome.clone(), 1, 0, 0)),
        }
        let entrada = por_companheiro
            .iter_mut()
            .find(|(id, ..)| *id == outro)
            .expect("companheiro recem-inserido");
        let placar = format!("{} x {}", meus.round() as i64, dele.round() as i64);
        let (equipe, cor, equipe_id) = identidade(equipes, &team_id);
        ano_a_ano.push((
            outro.clone(),
            DriverCareerDetailEntry {
                periodo: Some(ano.to_string()),
                categoria: divisoes.get(&season_number).cloned(),
                equipe,
                equipe_cor: cor,
                equipe_id,
                resumo: Some(placar.clone()),
                ..Default::default()
            },
        ));
        if meus > dele {
            entrada.3 += 1;
            vencidas.push(DriverCareerDetailEntry {
                periodo: Some(ano.to_string()),
                texto: Some(nome.clone()),
                resumo: Some(placar),
                ..Default::default()
            });
        } else if meus < dele {
            entrada.4 += 1;
        }
    }
    vencidas.reverse();

    // O mais duro e quem mais o venceu — a MESMA regra do card, senao o painel
    // abriria a historia de outro companheiro que nao o do numero.
    let rival = por_companheiro
        .iter()
        .filter(|(.., derrotas)| *derrotas > 0)
        .max_by(|a, b| {
            a.4.cmp(&b.4)
                .then_with(|| a.2.cmp(&b.2))
                .then_with(|| b.1.cmp(&a.1))
        })
        .map(|(id, ..)| id.clone());
    let rival_ano_a_ano = rival
        .map(|id| {
            ano_a_ano
                .iter()
                .filter(|(companheiro, _)| *companheiro == id)
                .map(|(_, linha)| linha.clone())
                .collect()
        })
        .unwrap_or_default();

    let mut companheiros = Vec::new();
    for (id, nome_arquivado, temporadas, vitorias, derrotas) in por_companheiro {
        let atual = load_driver_identity(conn, &id)?;
        companheiros.push(DriverCareerDetailEntry {
            texto: Some(
                atual
                    .as_ref()
                    .map(|identidade| identidade.nome.clone())
                    .filter(|nome| !nome.trim().is_empty())
                    .unwrap_or(nome_arquivado),
            ),
            equipe: atual
                .as_ref()
                .and_then(|identidade| identidade.equipe.clone()),
            equipe_cor: atual
                .as_ref()
                .and_then(|identidade| identidade.equipe_cor.clone()),
            equipe_id: atual
                .as_ref()
                .and_then(|identidade| identidade.equipe_id.clone()),
            idade: atual.as_ref().map(|identidade| identidade.idade),
            nacionalidade: atual
                .as_ref()
                .map(|identidade| identidade.nacionalidade.clone()),
            contagem: Some(temporadas),
            resumo: Some(format!("{vitorias}-{derrotas}")),
            ..Default::default()
        });
    }
    companheiros.sort_by(|a, b| b.contagem.cmp(&a.contagem));

    Ok((companheiros, vencidas, rival_ano_a_ano))
}

struct IdentidadeAtual {
    nome: String,
    idade: i32,
    nacionalidade: String,
    equipe: Option<String>,
    equipe_cor: Option<String>,
    equipe_id: Option<String>,
}

/// Quem o companheiro e HOJE. `None` quando ele foi purgado do mundo — e ai o
/// nome do arquivo e tudo o que sobrou dele.
fn load_driver_identity(
    conn: &Connection,
    driver_id: &str,
) -> Result<Option<IdentidadeAtual>, String> {
    conn.query_row(
        "SELECT d.nome, d.idade, d.nacionalidade, t.nome, t.cor_primaria, t.id
         FROM drivers d
         LEFT JOIN teams t ON t.id = (
            SELECT id FROM teams WHERE piloto_1_id = d.id OR piloto_2_id = d.id LIMIT 1
         )
         WHERE d.id = ?1",
        rusqlite::params![driver_id],
        |row| {
            Ok(IdentidadeAtual {
                nome: row.get(0)?,
                idade: row.get(1)?,
                nacionalidade: row.get(2)?,
                equipe: row.get(3)?,
                equipe_cor: row.get(4)?,
                equipe_id: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(|e| format!("Falha ao resolver identidade de companheiro: {e}"))
}

/// Cada lesao com o nome, a corrida em que aconteceu e a equipe daquele dia.
fn detalhe_das_lesoes(
    conn: &Connection,
    driver_id: &str,
    equipes: &HashMap<String, (String, String)>,
) -> Result<Vec<DriverCareerDetailEntry>, String> {
    // A tabela de lesoes so nasce quando a primeira acontece: sem ela o detalhe
    // e vazio, e nao um erro que derruba a ficha inteira.
    let mut stmt = match conn.prepare(
        "SELECT
                i.injury_name,
                i.type,
                COALESCE(s.ano, 0),
                COALESCE(c.rodada, 0),
                COALESCE(NULLIF(c.track_name, ''), NULLIF(c.pista, '')),
                NULLIF(c.data, ''),
                COALESCE(NULLIF(r.equipe_id, ''), '-')
             FROM injuries i
             LEFT JOIN calendar c ON c.id = i.race_occurred
             LEFT JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
             LEFT JOIN race_results r ON r.race_id = i.race_occurred AND r.piloto_id = i.pilot_id
             WHERE i.pilot_id = ?1
             ORDER BY COALESCE(s.numero, i.season) DESC, COALESCE(c.rodada, 0) DESC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Ok(Vec::new()),
    };
    let mapped = stmt
        .query_map(rusqlite::params![driver_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar detalhe das lesoes: {e}"))?;

    let mut linhas = Vec::new();
    for row in mapped {
        let (nome, tipo, ano, rodada, pista, data, equipe_id) =
            row.map_err(|e| format!("Falha ao ler detalhe de lesao: {e}"))?;
        let (equipe, cor, id_equipe) = identidade(equipes, &equipe_id);
        linhas.push(DriverCareerDetailEntry {
            texto: nome.filter(|valor| !valor.trim().is_empty()),
            resumo: Some(tipo),
            periodo: (ano > 0).then(|| ano.to_string()),
            rodada: (rodada > 0).then_some(rodada),
            pista,
            data,
            equipe,
            equipe_cor: cor,
            equipe_id: id_equipe,
            ..Default::default()
        });
    }
    Ok(linhas)
}

// ── Utilitarios ──────────────────────────────────────────────────────────────

fn linha_de_corrida(
    race: &CareerRaceHistoryRow,
    equipes: &HashMap<String, (String, String)>,
    divisoes: &HashMap<i32, String>,
) -> DriverCareerDetailEntry {
    let (equipe, cor, equipe_id) = identidade(equipes, &race.team_id);
    DriverCareerDetailEntry {
        periodo: (race.ano > 0).then(|| race.ano.to_string()),
        data: race.data.clone(),
        rodada: (race.rodada > 0).then_some(race.rodada),
        pista: race.pista.clone(),
        categoria: divisao_da_corrida(race, divisoes),
        equipe,
        equipe_cor: cor,
        equipe_id,
        contagem: Some(race.race_index),
        ..Default::default()
    }
}

fn linha_de_temporada(
    season: &CareerSeasonArchiveRow,
    equipes: &HashMap<String, (String, String)>,
) -> DriverCareerDetailEntry {
    let (equipe, cor, equipe_id) = season
        .equipe_id
        .as_deref()
        .map(|id| identidade(equipes, id))
        .unwrap_or((None, None, None));
    DriverCareerDetailEntry {
        periodo: Some(season.ano.to_string()),
        categoria: timeline_division_key(&season.categoria, season.classe.as_deref()),
        equipe,
        equipe_cor: cor,
        equipe_id,
        // A colocacao no campeonato e o titulo da linha: numa lista de catorze
        // temporadas e o que separa o ano de titulo do ano de meio de tabela.
        texto: season
            .posicao_campeonato
            .map(|posicao| format!("P{posicao}")),
        contagem: Some(season.corridas),
        resumo: Some(format!(
            "{} pts · {}V · {}P",
            season.pontos.round() as i64,
            season.vitorias,
            season.podios
        )),
        ..Default::default()
    }
}

/// A divisao da corrida: a do arquivo daquela temporada e, na temporada em curso
/// (que ainda nao foi arquivada), a categoria do proprio calendario.
fn divisao_da_corrida(
    race: &CareerRaceHistoryRow,
    divisoes: &HashMap<i32, String>,
) -> Option<String> {
    divisoes
        .get(&race.season_number)
        .cloned()
        .or_else(|| race.categoria.clone())
}

fn temporada_por_ano<'a>(
    active_seasons: &[&'a CareerSeasonArchiveRow],
    ano: Option<i32>,
) -> Option<&'a CareerSeasonArchiveRow> {
    let ano = ano?;
    active_seasons
        .iter()
        .copied()
        .find(|season| season.ano == ano)
}

fn faixa(inicio: i32, fim: i32) -> String {
    if inicio == fim {
        inicio.to_string()
    } else {
        format!("{inicio}-{fim}")
    }
}

/// Nome, cor e id da equipe. O id vem junto com o nome ou nao vem: a linha de
/// uma equipe que sumiu do mundo continua contando o que aconteceu, mas nao tem
/// tela do Atlas para abrir no clique.
fn identidade(
    equipes: &HashMap<String, (String, String)>,
    team_id: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    equipes
        .get(team_id)
        .map(|(nome, cor)| {
            (
                Some(nome.clone()),
                Some(cor.clone()),
                Some(team_id.to_string()),
            )
        })
        .unwrap_or((None, None, None))
}

/// Nome e cor das equipes por que ele correu.
///
/// Le as TRES colunas que o detalhe usa, e nao a equipe inteira: montar o modelo
/// completo exigiria o schema todo de `teams` (carro, financas, hierarquia) para
/// desenhar um nome e uma cor. Equipe que nao existe mais fica de fora, e a linha
/// sai sem nome — o que aconteceu continua tendo acontecido.
fn load_team_identities(
    conn: &Connection,
    races: &[CareerRaceHistoryRow],
) -> Result<HashMap<String, (String, String)>, String> {
    let mut equipes = HashMap::new();
    let mut stmt = match conn.prepare("SELECT nome, cor_primaria FROM teams WHERE id = ?1") {
        Ok(stmt) => stmt,
        // Save sem a tabela (banco de teste, arquivo parcial): sem identidade, e
        // o detalhe ainda vale pelas datas e categorias.
        Err(_) => return Ok(equipes),
    };
    for race in races {
        if race.team_id == "-" || equipes.contains_key(&race.team_id) {
            continue;
        }
        let identidade = stmt
            .query_row(rusqlite::params![race.team_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .optional()
            .map_err(|e| format!("Falha ao carregar equipe do detalhe: {e}"))?;
        if let Some(identidade) = identidade {
            equipes.insert(race.team_id.clone(), identidade);
        }
    }
    Ok(equipes)
}
