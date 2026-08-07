//! Onde cada numero do dossie coloca o piloto — no grid de hoje e no mundo.
//!
//! O dossie conta a carreira em absoluto: "19 poles", "1,5% de abandono", "46
//! podios seguidos". Sozinhos esses numeros nao dizem se aquilo e raro. Este
//! modulo responde a pergunta seguinte — QUANTOS estao a frente dele — em duas
//! populacoes: o grid da categoria em que ele corre hoje e o mundo inteiro.
//!
//! A regra que nao se negocia: cada numero e recomputado para todo mundo com a
//! MESMA funcao que desenha a ficha. Um rank calculado por uma regra propria
//! divergiria do numero ao lado dele na mesma linha, e a divergencia so
//! apareceria no save de alguem, meses depois.

use std::collections::HashSet;

use rusqlite::OptionalExtension;

use super::*;

/// O sentido em que o numero e "melhor". Abandono e jejum sao os casos em que
/// menos e mais: um piloto com 1 abandono esta a frente de um com 20.
#[derive(Clone, Copy, PartialEq)]
enum Sentido {
    Maior,
    Menor,
}

/// Tudo o que o dossie conta sobre um piloto, num pacote so.
///
/// `Option` onde o numero pode nao existir: sem largada registrada nao ha grid
/// medio, e sem corrida nenhuma nao ha taxa de abandono. Quem nao tem o numero
/// fica fora do ranking daquela linha em vez de entrar como zero — um estreante
/// com 0% de abandono nao e o piloto mais confiavel do grid.
#[derive(Default)]
struct NumerosDaCarreira {
    tempo_carreira: Option<f64>,
    temporadas: Option<f64>,
    categorias: Option<f64>,
    promocoes: Option<f64>,
    rebaixamentos: Option<f64>,
    equipes: Option<f64>,
    anos_por_equipe: Option<f64>,
    sequencia_vitorias: Option<f64>,
    sequencia_podios: Option<f64>,
    temporadas_top3: Option<f64>,
    poles: Option<f64>,
    poles_convertidas: Option<f64>,
    grid_medio: Option<f64>,
    voltas_rapidas: Option<f64>,
    jejum_vitorias: Option<f64>,
    jejum_podios: Option<f64>,
    temporadas_sem_podio: Option<f64>,
    abandonos: Option<f64>,
    taxa_abandono: Option<f64>,
    sequencia_chegadas: Option<f64>,
}

/// A chave da linha no dossie, de onde tirar o numero e para que lado ele conta.
const METRICAS: &[(&str, fn(&NumerosDaCarreira) -> Option<f64>, Sentido)] = &[
    ("tempo_carreira", |n| n.tempo_carreira, Sentido::Maior),
    ("temporadas", |n| n.temporadas, Sentido::Maior),
    ("categorias", |n| n.categorias, Sentido::Maior),
    ("promocoes", |n| n.promocoes, Sentido::Maior),
    ("rebaixamentos", |n| n.rebaixamentos, Sentido::Menor),
    ("equipes", |n| n.equipes, Sentido::Maior),
    (
        "tempo_medio_por_equipe",
        |n| n.anos_por_equipe,
        Sentido::Maior,
    ),
    (
        "sequencia_vitorias",
        |n| n.sequencia_vitorias,
        Sentido::Maior,
    ),
    ("sequencia_podios", |n| n.sequencia_podios, Sentido::Maior),
    ("temporadas_no_top3", |n| n.temporadas_top3, Sentido::Maior),
    ("poles", |n| n.poles, Sentido::Maior),
    ("poles_convertidas", |n| n.poles_convertidas, Sentido::Maior),
    ("grid_medio", |n| n.grid_medio, Sentido::Menor),
    ("voltas_rapidas", |n| n.voltas_rapidas, Sentido::Maior),
    ("jejum_vitorias", |n| n.jejum_vitorias, Sentido::Menor),
    ("jejum_podios", |n| n.jejum_podios, Sentido::Menor),
    (
        "temporadas_sem_podio",
        |n| n.temporadas_sem_podio,
        Sentido::Menor,
    ),
    ("abandonos", |n| n.abandonos, Sentido::Menor),
    ("taxa_abandono", |n| n.taxa_abandono, Sentido::Menor),
    (
        "sequencia_chegadas",
        |n| n.sequencia_chegadas,
        Sentido::Maior,
    ),
];

/// Onde cada numero da carreira coloca o piloto, no grid atual e no mundo.
///
/// NAO entra no payload da ficha. Ela varre `race_results` e o arquivo de
/// temporadas do mundo INTEIRO — 503ms num save de 27 mil resultados, medido em
/// debug, contra 512ms do bloco de historico completo. Era 98% do custo de abrir
/// a ficha, pago tambem a cada troca de piloto, para alimentar um toggle que
/// nasce desligado. Hoje quem paga e quem liga o toggle, via
/// `get_driver_dossier_ranks`.
pub(crate) fn build_dossier_ranks(
    conn: &Connection,
    driver_id: &str,
) -> Result<HashMap<String, DriverCareerRankEntry>, String> {
    let corridas = load_all_race_rows(conn)?;
    let temporadas = load_all_season_rows(conn)?;
    let grid = load_current_grid(conn, driver_id)?;

    // Um piloto entra na conta se tem QUALQUER historico. Quem so existe na
    // tabela de pilotos, sem corrida nem temporada, nao e concorrencia — e um
    // rookie que ainda nao largou inflaria todo denominador da ficha.
    let mut pilotos: HashSet<&String> = HashSet::new();
    pilotos.extend(corridas.keys());
    pilotos.extend(temporadas.keys());

    let vazio_corridas: Vec<CareerRaceHistoryRow> = Vec::new();
    let vazio_temporadas: Vec<CareerSeasonArchiveRow> = Vec::new();
    let numeros: HashMap<&String, NumerosDaCarreira> = pilotos
        .into_iter()
        .map(|id| {
            let n = numeros_da_carreira(
                corridas.get(id).unwrap_or(&vazio_corridas),
                temporadas.get(id).unwrap_or(&vazio_temporadas),
            );
            (id, n)
        })
        .collect();

    let meu_id = driver_id.to_string();
    let Some(meus_numeros) = numeros.get(&meu_id) else {
        return Ok(HashMap::new());
    };

    let mut ranks = HashMap::new();
    for (chave, extrai, sentido) in METRICAS {
        let Some(meu) = extrai(meus_numeros) else {
            continue;
        };
        let (mundo, mundo_total) = posicao(&numeros, None, *extrai, *sentido, meu);
        let (posicao_grid, grid_total) = posicao(&numeros, Some(&grid), *extrai, *sentido, meu);
        ranks.insert(
            (*chave).to_string(),
            DriverCareerRankEntry {
                grid: posicao_grid,
                grid_total,
                mundo,
                mundo_total,
            },
        );
    }
    Ok(ranks)
}

/// Posicao por competicao: quem empata divide o lugar, e o proximo pula. Tres
/// pilotos com 19 poles sao os tres primeiros, e o de 18 e o quarto.
fn posicao(
    numeros: &HashMap<&String, NumerosDaCarreira>,
    populacao: Option<&HashSet<String>>,
    extrai: fn(&NumerosDaCarreira) -> Option<f64>,
    sentido: Sentido,
    meu: f64,
) -> (Option<i32>, i32) {
    let mut total = 0;
    let mut a_frente = 0;
    for (id, n) in numeros {
        if populacao.is_some_and(|filtro| !filtro.contains(*id)) {
            continue;
        }
        let Some(valor) = extrai(n) else { continue };
        total += 1;
        let melhor = match sentido {
            Sentido::Maior => valor > meu,
            Sentido::Menor => valor < meu,
        };
        if melhor {
            a_frente += 1;
        }
    }
    if total == 0 {
        return (None, 0);
    }
    (Some(a_frente + 1), total)
}

/// Os numeros de UM piloto, pelas mesmas funcoes que desenham o dossie dele.
fn numeros_da_carreira(
    races: &[CareerRaceHistoryRow],
    seasons: &[CareerSeasonArchiveRow],
) -> NumerosDaCarreira {
    let ativas: Vec<&CareerSeasonArchiveRow> = seasons.iter().filter(|s| s.corridas > 0).collect();
    if ativas.is_empty() && races.is_empty() {
        return NumerosDaCarreira::default();
    }

    let categorias: HashSet<&str> = ativas
        .iter()
        .map(|s| s.categoria.trim())
        .filter(|c| !c.is_empty())
        .collect();
    let (promocoes, rebaixamentos) = count_category_mobility(&ativas);
    let (equipes, anos_por_equipe) = summarize_team_mobility(races);
    let sabado = build_qualifying_block(races);
    let abandonos = races.iter().filter(|r| r.is_dnf).count() as i32;
    let corridas = races.len() as i32;

    NumerosDaCarreira {
        tempo_carreira: Some(career_duration_from_archive(seasons) as f64),
        temporadas: Some(ativas.len() as f64),
        categorias: Some(categorias.len() as f64),
        promocoes: Some(promocoes as f64),
        rebaixamentos: Some(rebaixamentos as f64),
        equipes: Some(equipes as f64),
        anos_por_equipe,
        sequencia_vitorias: Some(longest_win_streak_span(races).0 as f64),
        sequencia_podios: Some(longest_podium_streak_span(races).0 as f64),
        temporadas_top3: Some(
            ativas
                .iter()
                .filter(|s| matches!(s.posicao_campeonato, Some(p) if p <= 3))
                .count() as f64,
        ),
        poles: Some(sabado.poles as f64),
        poles_convertidas: Some(sabado.poles_convertidas as f64),
        grid_medio: sabado.grid_medio,
        voltas_rapidas: Some(sabado.voltas_rapidas as f64),
        jejum_vitorias: Some(longest_winless_streak_span(races).0 as f64),
        jejum_podios: Some(longest_podiumless_streak_span(races).0 as f64),
        temporadas_sem_podio: Some(ativas.iter().filter(|s| s.podios == 0).count() as f64),
        abandonos: Some(abandonos as f64),
        taxa_abandono: (corridas > 0).then(|| abandonos as f64 * 100.0 / corridas as f64),
        sequencia_chegadas: Some(longest_finish_streak(races) as f64),
    }
}

/// O grid de hoje: quem esta ativo na MESMA categoria que ele. Sem categoria
/// (aposentado, sem contrato) o conjunto sai vazio e so o rank mundial aparece.
///
/// Banco sem a tabela de pilotos (fixture, arquivo parcial) devolve conjunto
/// vazio: a ficha perde a coluna do grid e mantem a do mundo, em vez de morrer
/// inteira por causa de uma comparacao.
fn load_current_grid(conn: &Connection, driver_id: &str) -> Result<HashSet<String>, String> {
    let minha_categoria: Option<String> = match conn.query_row(
        "SELECT NULLIF(TRIM(COALESCE(categoria_atual, '')), '')
             FROM drivers WHERE id = ?1",
        rusqlite::params![driver_id],
        |row| row.get(0),
    ) {
        Ok(valor) => valor,
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(_) => return Ok(HashSet::new()),
    };
    let Some(categoria) = minha_categoria else {
        return Ok(HashSet::new());
    };

    let mut stmt = match conn.prepare(
        "SELECT id FROM drivers
             WHERE TRIM(COALESCE(categoria_atual, '')) = ?1
               AND LOWER(COALESCE(status, 'Ativo')) = 'ativo'",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Ok(HashSet::new()),
    };
    let mapped = stmt
        .query_map(rusqlite::params![categoria], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Falha ao consultar grid atual: {e}"))?;

    let mut grid = HashSet::new();
    for row in mapped {
        grid.insert(row.map_err(|e| format!("Falha ao ler piloto do grid: {e}"))?);
    }
    // O proprio piloto entra sempre: aposentado tem categoria mas nao status
    // ativo, e um rank "3 de 24" que nao inclui quem esta sendo medido mente.
    grid.insert(driver_id.to_string());
    Ok(grid)
}

/// O corrida-a-corrida do mundo inteiro, agrupado por piloto e na ordem
/// cronologica que as sequencias exigem. Uma consulta so: fazer uma por piloto
/// seria uma varredura do mundo multiplicada por duzentos.
fn load_all_race_rows(
    conn: &Connection,
) -> Result<HashMap<String, Vec<CareerRaceHistoryRow>>, String> {
    let stmt = conn
        .prepare(
            "SELECT
                r.piloto_id,
                COALESCE(s.numero, 0),
                COALESCE(NULLIF(r.equipe_id, ''), '-'),
                r.posicao_final,
                r.dnf,
                COALESCE(r.posicao_largada, 0),
                COALESCE(r.fastest_lap, 0),
                COALESCE(s.ano, 0)
             FROM race_results r
             INNER JOIN calendar c ON c.id = r.race_id
             LEFT JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
             ORDER BY r.piloto_id ASC, COALESCE(s.numero, 0) ASC, c.rodada ASC, r.id ASC",
        )
        .map_err(|e| format!("Falha ao preparar corrida-a-corrida do mundo: {e}"));
    // Sem a tabela nao ha mundo para comparar, e o dossie continua valendo pelos
    // numeros absolutos. O rank e uma leitura a mais, nunca uma condicao.
    let mut stmt = match stmt {
        Ok(stmt) => stmt,
        Err(_) => return Ok(HashMap::new()),
    };
    let mapped = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                CareerRaceHistoryRow {
                    race_index: 0,
                    season_number: row.get(1)?,
                    team_id: row.get(2)?,
                    position: row.get(3)?,
                    is_dnf: row.get::<_, i32>(4)? != 0,
                    grid_position: row.get(5)?,
                    has_fastest_lap: row.get::<_, i32>(6)? != 0,
                    race_id: String::new(),
                    ano: row.get(7)?,
                    rodada: 0,
                    pista: None,
                    categoria: None,
                    data: None,
                },
            ))
        })
        .map_err(|e| format!("Falha ao consultar corrida-a-corrida do mundo: {e}"))?;

    let mut por_piloto: HashMap<String, Vec<CareerRaceHistoryRow>> = HashMap::new();
    for row in mapped {
        let (piloto, mut corrida) =
            row.map_err(|e| format!("Falha ao ler corrida do mundo: {e}"))?;
        let lista = por_piloto.entry(piloto).or_default();
        corrida.race_index = lista.len() as i32 + 1;
        lista.push(corrida);
    }
    Ok(por_piloto)
}

/// O arquivo de temporadas do mundo inteiro, lido pelo MESMO mapeamento da ficha
/// — corridas, vitorias e podios moram dentro do `snapshot_json`, e uma segunda
/// leitura dele divergiria em silencio do numero que a linha mostra.
fn load_all_season_rows(
    conn: &Connection,
) -> Result<HashMap<String, Vec<CareerSeasonArchiveRow>>, String> {
    let stmt = conn
        .prepare(
            "SELECT
                piloto_id, season_number, ano, categoria, posicao_campeonato,
                pontos, snapshot_json
             FROM driver_season_archive
             ORDER BY piloto_id ASC, season_number ASC",
        )
        .map_err(|e| format!("Falha ao preparar arquivo de temporadas do mundo: {e}"));
    let mut stmt = match stmt {
        Ok(stmt) => stmt,
        Err(_) => return Ok(HashMap::new()),
    };
    let mapped = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, season_archive_row_from(row, 1)?))
        })
        .map_err(|e| format!("Falha ao consultar arquivo de temporadas do mundo: {e}"))?;

    let mut por_piloto: HashMap<String, Vec<CareerSeasonArchiveRow>> = HashMap::new();
    for row in mapped {
        let (piloto, temporada) =
            row.map_err(|e| format!("Falha ao ler temporada do mundo: {e}"))?;
        por_piloto.entry(piloto).or_default().push(temporada);
    }
    Ok(por_piloto)
}
