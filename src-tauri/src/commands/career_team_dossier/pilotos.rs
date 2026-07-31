//! Quem pilotou pela equipe e quanto o carro aguentou.
//!
//! Duas leituras que o resto do dossie nao da. Os agregados de `fatos.rs`
//! trabalham por CORRIDA da equipe (o melhor carro dela naquele domingo) e nao
//! sabem quem estava dentro; e nenhum bloco olhava para `race_results.dnf`, de
//! modo que uma equipe que abandona metade das corridas e uma que termina todas
//! em decimo apareciam iguais em "fora do top 10".

use std::collections::{BTreeMap, HashMap, HashSet};

use super::*;
use crate::commands::career_types::{TeamHistoryLineupTerm, TeamHistoryReliability};

/// Origens de abandono que sao falha da MAQUINA, e nao do piloto. O catalogo
/// (`incident_catalog.incident_source`) tambem tem `DriverError` e
/// `PostCollision`, que sao o outro lado da conta; o que nao cair em nenhum dos
/// dois lados fica em "outros" em vez de ser chutado para um deles.
const MECHANICAL_SOURCES: [&str; 3] = ["Mechanical", "Operational", "RaceSpec"];
const DRIVER_SOURCES: [&str; 2] = ["DriverError", "PostCollision"];

/// Vaga sem dono conhecido. `team_season_archive` so guarda temporada encerrada,
/// entao um piloto que correu como substituto — ou uma equipe cujo save nao tem
/// arquivo nenhum — nao aparece em vaga 1 nem em 2. Em vez de sumir da galeria,
/// ele cai aqui e a tela o mostra numa faixa separada.
const SLOT_UNKNOWN: i32 = 0;

/// Dados por piloto que nao dependem da vaga: nome e para onde ele foi.
struct DriverProfile {
    name: String,
    nationality: String,
    is_player: bool,
    still_here: bool,
    current_team_name: String,
    current_team_color: String,
    current_label: String,
}

/// Uma passagem contínua de um piloto por uma vaga da equipe.
struct Term {
    slot: i32,
    driver_id: String,
    first_year: i32,
    last_year: i32,
}

/// A sucessao de pilotos nas duas vagas da equipe, do primeiro ao ultimo.
///
/// Uma lista chapada de nomes respondia "quem passou por aqui" mas nao "quanto
/// essa equipe troca de piloto" — e as duas colunas respondem: uma casa que
/// mantem o mesmo titular por seis anos e outra que troca todo ano desenham
/// diferente antes de qualquer numero ser lido.
pub(super) fn load_team_lineup(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<Vec<TeamHistoryLineupTerm>, String> {
    let ano_atual = current_year(conn)?;
    let ocupacao = load_slot_occupancy(conn, team_id)?;
    let mut terms = build_terms(&ocupacao);
    let resultados = load_driver_results(conn, team_id)?;

    // Quem correu pela equipe e nunca apareceu como titular de uma temporada
    // arquivada. Sem isto, um save novo (nada arquivado ainda) mostraria a
    // galeria vazia mesmo com corridas disputadas.
    let com_vaga: HashSet<&str> = terms.iter().map(|t| t.driver_id.as_str()).collect();
    let mut avulsos: BTreeMap<String, (i32, i32)> = BTreeMap::new();
    for (driver_id, year, _) in &resultados {
        if com_vaga.contains(driver_id.as_str()) {
            continue;
        }
        let faixa = avulsos.entry(driver_id.clone()).or_insert((*year, *year));
        faixa.0 = faixa.0.min(*year);
        faixa.1 = faixa.1.max(*year);
    }
    for (driver_id, (primeiro, ultimo)) in avulsos {
        terms.push(Term {
            slot: SLOT_UNKNOWN,
            driver_id,
            first_year: primeiro,
            last_year: ultimo,
        });
    }
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<String> = terms
        .iter()
        .map(|t| t.driver_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let perfis = load_driver_profiles(conn, team_id, &ids)?;

    let mut lineup: Vec<TeamHistoryLineupTerm> = terms
        .into_iter()
        .map(|term| {
            // As estatisticas sao do MANDATO, nao da carreira do piloto na
            // equipe: quem voltou depois de dois anos fora tem duas passagens, e
            // repetir o total nas duas contaria a mesma corrida duas vezes.
            let (races, wins, podiums, best) = tally(&resultados, &term);
            let perfil = perfis.get(&term.driver_id);
            TeamHistoryLineupTerm {
                slot: term.slot,
                driver_id: term.driver_id.clone(),
                name: perfil
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| term.driver_id.clone()),
                nationality: perfil.map(|p| p.nationality.clone()).unwrap_or_default(),
                first_year: term.first_year.to_string(),
                last_year: term.last_year.to_string(),
                races,
                wins,
                podiums,
                best_position: best,
                is_player: perfil.map(|p| p.is_player).unwrap_or(false),
                // "Ainda na equipe" e do MANDATO, e nao do piloto: quem trocou
                // de vaga continua na casa, e o perfil dele diria que sim para as
                // duas passagens — a antiga aparecia marcada como vigente e a
                // vaga 1 exibia dois titulares atuais.
                still_here: perfil.map(|p| p.still_here).unwrap_or(false)
                    && term.last_year == ano_atual,
                current_team_name: perfil
                    .map(|p| p.current_team_name.clone())
                    .unwrap_or_default(),
                current_team_color: perfil
                    .map(|p| p.current_team_color.clone())
                    .unwrap_or_default(),
                current_label: perfil.map(|p| p.current_label.clone()).unwrap_or_default(),
            }
        })
        // Mandato sem corrida nenhuma e contrato que nao virou pista: o piloto
        // foi anunciado e a temporada nao rodou. Nao e passagem — EXCETO a
        // vigente: antes da primeira corrida do save, o titular de hoje (o
        // jogador, inclusive) tinha zero corridas e sumia da galeria, que e
        // justamente quando ela deveria dizer quem esta no carro agora.
        .filter(|term| term.races > 0 || term.still_here)
        .collect();

    // Ordem CRONOLOGICA dentro de cada vaga: a galeria conta a passagem do tempo
    // pela equipe, e a coluna so se le como sucessao se for de cima para baixo.
    lineup.sort_by(|a, b| {
        a.slot
            .cmp(&b.slot)
            .then(a.first_year.cmp(&b.first_year))
            .then(a.last_year.cmp(&b.last_year))
            .then(a.name.cmp(&b.name))
    });
    Ok(lineup)
}

/// O ano da temporada mais recente do save. Zero num save sem temporada alguma —
/// e aí nenhum mandato é vigente, que é a leitura correta.
fn current_year(conn: &rusqlite::Connection) -> Result<i32, String> {
    conn.query_row("SELECT COALESCE(MAX(ano), 0) FROM seasons", [], |row| {
        row.get(0)
    })
    .map_err(|e| format!("Falha ao consultar o ano corrente: {e}"))
}

/// Quem ocupou cada vaga em cada ano: `(ano, slot) -> piloto`.
///
/// `team_season_archive` guarda o par titular de cada temporada ENCERRADA; a
/// temporada em curso ainda nao esta la, e e a mais importante da lista — sem
/// ela ninguem aparece como "ainda na equipe". O lineup de hoje vem de `teams`.
fn load_slot_occupancy(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<BTreeMap<(i32, i32), String>, String> {
    let mut ocupacao: BTreeMap<(i32, i32), String> = BTreeMap::new();
    let mut stmt = conn
        .prepare(
            // Uma equipe pode ter linha em mais de uma categoria no mesmo ano
            // (bloco especial). O par titular e o mesmo, entao agrupar por ano
            // resolve sem escolher categoria.
            "SELECT ano, MAX(piloto_1_id), MAX(piloto_2_id)
             FROM team_season_archive
             WHERE team_id = ?1
             GROUP BY ano
             ORDER BY ano ASC",
        )
        .map_err(|e| format!("Falha ao preparar histórico de vagas da equipe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![team_id], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar histórico de vagas da equipe: {e}"))?;
    for row in rows {
        let (ano, p1, p2) =
            row.map_err(|e| format!("Falha ao ler histórico de vagas da equipe: {e}"))?;
        insert_slot(&mut ocupacao, ano, 1, p1);
        insert_slot(&mut ocupacao, ano, 2, p2);
    }

    // Temporada em curso, direto do lineup vigente.
    let atual: Option<(i32, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT (SELECT MAX(ano) FROM seasons), t.piloto_1_id, t.piloto_2_id
             FROM teams t WHERE t.id = ?1",
            rusqlite::params![team_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|e| format!("Falha ao consultar lineup atual da equipe: {e}"))?;
    if let Some((ano, p1, p2)) = atual {
        insert_slot(&mut ocupacao, ano, 1, p1);
        insert_slot(&mut ocupacao, ano, 2, p2);
    }
    Ok(ocupacao)
}

fn insert_slot(
    ocupacao: &mut BTreeMap<(i32, i32), String>,
    ano: i32,
    slot: i32,
    piloto: Option<String>,
) {
    if let Some(id) = piloto.filter(|id| !id.trim().is_empty()) {
        ocupacao.insert((ano, slot), id);
    }
}

/// Anos consecutivos do mesmo piloto na mesma vaga viram UMA passagem. Um ano de
/// intervalo quebra a passagem em duas: o piloto saiu e voltou, e essa e a
/// historia — juntar as duas pontas apagaria o ano em que outro correu ali.
fn build_terms(ocupacao: &BTreeMap<(i32, i32), String>) -> Vec<Term> {
    let mut por_slot: BTreeMap<i32, Vec<(i32, &String)>> = BTreeMap::new();
    for ((ano, slot), piloto) in ocupacao {
        por_slot.entry(*slot).or_default().push((*ano, piloto));
    }
    let mut terms = Vec::new();
    for (slot, mut anos) in por_slot {
        anos.sort_by_key(|(ano, _)| *ano);
        let mut atual: Option<Term> = None;
        for (ano, piloto) in anos {
            match atual.as_mut() {
                Some(term) if term.driver_id == *piloto && ano <= term.last_year + 1 => {
                    term.last_year = ano;
                }
                _ => {
                    if let Some(term) = atual.take() {
                        terms.push(term);
                    }
                    atual = Some(Term {
                        slot,
                        driver_id: piloto.clone(),
                        first_year: ano,
                        last_year: ano,
                    });
                }
            }
        }
        if let Some(term) = atual.take() {
            terms.push(term);
        }
    }
    terms
}

/// Corridas, vitorias, podios e melhor colocacao do piloto dentro do mandato.
fn tally(resultados: &[(String, i32, i32)], term: &Term) -> (i32, i32, i32, i32) {
    let mut races = 0;
    let mut wins = 0;
    let mut podiums = 0;
    let mut best = 0;
    for (driver_id, year, position) in resultados {
        if driver_id != &term.driver_id || *year < term.first_year || *year > term.last_year {
            continue;
        }
        races += 1;
        if *position == 1 {
            wins += 1;
        }
        if (1..=3).contains(position) {
            podiums += 1;
        }
        // `posicao_final = 0` e linha sem classificacao (abandono antes da
        // bandeira); tomada como melhor resultado, ela diria que o piloto venceu.
        if *position > 0 && (best == 0 || *position < best) {
            best = *position;
        }
    }
    (races, wins, podiums, best)
}

/// Um registro por resultado do piloto vestindo esta equipe.
fn load_driver_results(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<Vec<(String, i32, i32)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT rr.piloto_id, s.ano, rr.posicao_final
             FROM race_results rr
             JOIN calendar c ON c.id = rr.race_id
             JOIN seasons s ON s.id = c.temporada_id
             WHERE rr.equipe_id = ?1",
        )
        .map_err(|e| format!("Falha ao preparar corridas dos pilotos da equipe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![team_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|e| format!("Falha ao consultar corridas dos pilotos da equipe: {e}"))?;
    let mut resultados = Vec::new();
    for row in rows {
        resultados
            .push(row.map_err(|e| format!("Falha ao ler corridas dos pilotos da equipe: {e}"))?);
    }
    Ok(resultados)
}

/// Nome e destino de cada piloto da galeria.
///
/// Tres estados possiveis: ativo (`drivers`), aposentado (`retired`) ou apenas
/// um id herdado de um save antigo. Quem esta em qual equipe HOJE nao sai de
/// `drivers` — o vinculo mora nas duas vagas de `teams`.
fn load_driver_profiles(
    conn: &rusqlite::Connection,
    team_id: &str,
    ids: &[String],
) -> Result<HashMap<String, DriverProfile>, String> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = format!(
        "SELECT
            piloto.id,
            COALESCE(d.nome, ret.nome, piloto.id) AS nome,
            COALESCE(d.is_jogador, 0) AS jogador,
            d.status,
            d.categoria_atual,
            atual.id,
            atual.nome,
            atual.cor_primaria,
            ret.categoria_final,
            ret.temporada_aposentadoria,
            -- Só `drivers` guarda o país: `retired` não tem a coluna. Quem já
            -- pendurou o capacete e saiu da tabela de ativos fica sem bandeira,
            -- e a galeria desenha a linha sem ela em vez de chutar um país.
            d.nacionalidade
         FROM (SELECT ? AS id{extras}) piloto
         LEFT JOIN drivers d ON d.id = piloto.id
         LEFT JOIN retired ret ON ret.piloto_id = piloto.id
         LEFT JOIN teams atual
                ON atual.piloto_1_id = piloto.id OR atual.piloto_2_id = piloto.id",
        // A lista de ids vira uma tabela derivada com UNION ALL: a alternativa
        // seria consultar `drivers` com IN, mas ai o piloto que so existe em
        // `retired` (ou em nenhuma das duas) sumiria do resultado.
        extras = (1..ids.len())
            .map(|_| " UNION ALL SELECT ?".to_string())
            .collect::<String>(),
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar perfis dos pilotos da equipe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), |row| {
            let id: String = row.get(0)?;
            let status: Option<String> = row.get(3)?;
            let categoria_atual: Option<String> = row.get(4)?;
            let equipe_atual: Option<String> = row.get(5)?;
            let categoria_final: Option<String> = row.get(8)?;
            let temporada_aposentadoria: Option<String> = row.get(9)?;
            let still_here = equipe_atual.as_deref() == Some(team_id);
            // A equipe de hoje so e "outra equipe" quando nao e esta: quem
            // continua na casa ja e dito pelo destaque da propria linha, e
            // repetir o brasao da casa em metade da galeria nao distingue nada.
            let em_outra = !still_here && equipe_atual.is_some();
            Ok((
                id,
                DriverProfile {
                    name: row.get(1)?,
                    nationality: row.get::<_, Option<String>>(10)?.unwrap_or_default(),
                    is_player: row.get::<_, i32>(2)? > 0,
                    still_here,
                    current_team_name: if em_outra {
                        row.get::<_, Option<String>>(6)?.unwrap_or_default()
                    } else {
                        String::new()
                    },
                    current_team_color: if em_outra {
                        row.get::<_, Option<String>>(7)?.unwrap_or_default()
                    } else {
                        String::new()
                    },
                    current_label: destino_label(
                        still_here,
                        status.as_deref(),
                        categoria_atual.as_deref(),
                        categoria_final.as_deref(),
                        temporada_aposentadoria.as_deref(),
                    ),
                },
            ))
        })
        .map_err(|e| format!("Falha ao consultar perfis dos pilotos da equipe: {e}"))?;

    let mut perfis = HashMap::new();
    for row in rows {
        let (id, perfil) =
            row.map_err(|e| format!("Falha ao ler perfis dos pilotos da equipe: {e}"))?;
        perfis.insert(id, perfil);
    }
    Ok(perfis)
}

/// Onde o piloto esta HOJE. E o que transforma a galeria numa historia — sem
/// isso ela e so uma lista de nomes que passaram por ali.
fn destino_label(
    ainda_aqui: bool,
    status: Option<&str>,
    categoria_atual: Option<&str>,
    categoria_final: Option<&str>,
    temporada_aposentadoria: Option<&str>,
) -> String {
    if ainda_aqui {
        return rust_i18n::t!("team_dossier.alumni.still_here").to_string();
    }
    // `retired` e a fonte de quem pendurou o capacete; o `status` em `drivers`
    // pode continuar como estava se o piloto so foi arquivado.
    if categoria_final.is_some() || status == Some("Aposentado") {
        let ano = temporada_aposentadoria
            .unwrap_or_default()
            .trim()
            .to_string();
        if ano.is_empty() {
            return rust_i18n::t!("team_dossier.alumni.retired").to_string();
        }
        return rust_i18n::t!("team_dossier.alumni.retired_year", year = ano).to_string();
    }
    match categoria_atual.map(str::trim).filter(|c| !c.is_empty()) {
        Some(categoria) => rust_i18n::t!(
            "team_dossier.alumni.now_at",
            category = team_history_category_label(categoria)
        )
        .to_string(),
        None => rust_i18n::t!("team_dossier.alumni.unknown").to_string(),
    }
}

/// Quantas largadas viraram chegada, e o que levou o resto para o box.
///
/// Recortado pelo GRUPO de categorias, igual aos records: a taxa so quer dizer
/// alguma coisa contra a media de quem corre o mesmo carro. Uma equipe de
/// endurance abandona mais que uma de sprint por motivos que nao sao dela.
pub(super) fn load_team_reliability(
    conn: &rusqlite::Connection,
    team_id: &str,
    category_ids: &[String],
) -> Result<TeamHistoryReliability, String> {
    if category_ids.is_empty() {
        return Ok(TeamHistoryReliability::default());
    }
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    // Uma linha por CARRO — nao por corrida da equipe. Confiabilidade e do
    // conjunto carro+piloto: com dois carros, um abandono e meia corrida
    // perdida, e agregar por corrida esconderia isso.
    let sql = format!(
        "SELECT
            rr.equipe_id,
            COUNT(*) AS largadas,
            SUM(CASE WHEN rr.dnf = 1 THEN 1 ELSE 0 END) AS abandonos,
            SUM(CASE WHEN rr.dnf = 1 AND ic.incident_source IN ({mecanicas}) THEN 1 ELSE 0 END) AS mecanicos,
            SUM(CASE WHEN rr.dnf = 1 AND ic.incident_source IN ({pilotagem}) THEN 1 ELSE 0 END) AS pilotagem
         FROM race_results rr
         JOIN calendar c ON c.id = rr.race_id
         LEFT JOIN incident_catalog ic ON ic.id = rr.dnf_catalog_id
         WHERE c.categoria IN ({placeholders})
         GROUP BY rr.equipe_id",
        mecanicas = MECHANICAL_SOURCES
            .iter()
            .map(|s| format!("'{s}'"))
            .collect::<Vec<_>>()
            .join(", "),
        pilotagem = DRIVER_SOURCES
            .iter()
            .map(|s| format!("'{s}'"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar confiabilidade da equipe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(category_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(4)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar confiabilidade da equipe: {e}"))?;

    let mut selecionada = (0, 0, 0, 0);
    let mut grupo_largadas = 0;
    let mut grupo_chegadas = 0;
    for row in rows {
        let (equipe, largadas, abandonos, mecanicos, pilotagem) =
            row.map_err(|e| format!("Falha ao ler confiabilidade da equipe: {e}"))?;
        grupo_largadas += largadas;
        grupo_chegadas += largadas - abandonos;
        if equipe == team_id {
            selecionada = (largadas, abandonos, mecanicos, pilotagem);
        }
    }

    let (largadas, abandonos, mecanicos, pilotagem) = selecionada;
    if largadas <= 0 {
        return Ok(TeamHistoryReliability::default());
    }
    let chegadas = largadas - abandonos;
    Ok(TeamHistoryReliability {
        races: largadas,
        finished: chegadas,
        finish_rate: percentage(chegadas, largadas),
        group_finish_rate: percentage(grupo_chegadas, grupo_largadas),
        mechanical: mecanicos,
        driver_error: pilotagem,
        // O que sobra sao abandonos sem incidente catalogado (saves antigos,
        // corrida importada do iRacing). Some em vez de ser empurrado para uma
        // das duas causas — chutar a causa e pior do que admitir que nao sabe.
        other: (abandonos - mecanicos - pilotagem).max(0),
        worst_part: load_worst_part(conn, team_id, category_ids)?,
    })
}

/// A peca que mais tirou a equipe da corrida. Vem de `race_breakdowns`, que
/// guarda a falha por piloto e volta; o vinculo com a equipe passa por
/// `race_results` porque a tabela de quebras nao carrega `equipe_id`.
fn load_worst_part(
    conn: &rusqlite::Connection,
    team_id: &str,
    category_ids: &[String],
) -> Result<String, String> {
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT b.part, COUNT(*) AS total
         FROM race_breakdowns b
         JOIN race_results rr ON rr.race_id = b.race_id AND rr.piloto_id = b.driver_id
         JOIN calendar c ON c.id = rr.race_id
         WHERE rr.equipe_id = ?1 AND b.problem = 1 AND c.categoria IN ({placeholders})
         GROUP BY b.part
         ORDER BY total DESC, b.part ASC
         LIMIT 1"
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&team_id];
    for id in category_ids {
        params.push(id);
    }
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar peça mais frágil da equipe: {e}"))?;
    let parte: Option<String> = stmt
        .query_row(params.as_slice(), |row| row.get(0))
        .optional()
        .map_err(|e| format!("Falha ao consultar peça mais frágil da equipe: {e}"))?;
    Ok(parte.unwrap_or_default())
}
