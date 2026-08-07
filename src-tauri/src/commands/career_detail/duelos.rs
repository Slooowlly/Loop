//! Confronto direto com o companheiro de equipe, temporada a temporada.
//!
//! O companheiro e o unico adversario que corre com o MESMO carro. Toda outra
//! comparacao da ficha — posicao no campeonato, indice mundial, pontos — mistura
//! piloto e pacote; esta nao. E a unica que responde a pergunta que o jogador
//! realmente faz: ele e bom, ou o carro e bom?
//!
//! A dupla de cada temporada vem de `team_season_archive` (`piloto_1_id` e
//! `piloto_2_id`), e os pontos dos dois vem do arquivo de temporada de cada um.
//! O criterio de quem venceu o ano e PONTO, e nao posicao no campeonato: numa
//! categoria multiclasse os dois podem estar em classes diferentes, e ai a
//! colocacao nao compara.

use rusqlite::OptionalExtension;

use super::*;

/// O arquivo de equipes so existe depois da primeira temporada fechada: em save
/// novo a tabela nao esta la, e o card sai zerado em vez de derrubar a ficha.
fn tem_arquivo_de_equipes(conn: &Connection) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'team_season_archive'",
        [],
        |_| Ok(true),
    )
    .optional()
    .map(|encontrado| encontrado.unwrap_or(false))
    .map_err(|e| format!("Falha ao checar arquivo de equipes: {e}"))
}

/// Uma temporada de confronto: quem dividiu o box e quantos pontos cada um fez.
struct TemporadaDeDupla {
    companheiro_id: String,
    pontos_do_piloto: f64,
    pontos_do_companheiro: f64,
}

#[derive(Default)]
struct PlacarDoCompanheiro {
    nome: String,
    temporadas: i32,
    vitorias: i32,
    derrotas: i32,
}

pub(super) fn build_teammate_block(
    conn: &Connection,
    driver_id: &str,
) -> Result<DriverCareerTeammateBlock, String> {
    if !tem_arquivo_de_equipes(conn)? {
        return Ok(DriverCareerTeammateBlock::default());
    }

    let duplas = load_teammate_seasons(conn, driver_id)?;
    let mut placares: HashMap<String, PlacarDoCompanheiro> = HashMap::new();
    let mut temporadas_vencidas = 0;

    for dupla in &duplas {
        let placar = placares.entry(dupla.companheiro_id.clone()).or_default();
        placar.temporadas += 1;
        // Empate em pontos nao conta para nenhum lado: o ano nao decidiu nada.
        if dupla.pontos_do_piloto > dupla.pontos_do_companheiro {
            placar.vitorias += 1;
            temporadas_vencidas += 1;
        } else if dupla.pontos_do_piloto < dupla.pontos_do_companheiro {
            placar.derrotas += 1;
        }
    }

    let nomes = load_driver_names(conn, placares.keys())?;
    for (id, placar) in placares.iter_mut() {
        placar.nome = nomes.get(id).cloned().unwrap_or_else(|| id.clone());
    }

    // O mais duro e quem mais o venceu. Empate no numero de derrotas fica com
    // quem dividiu box por mais tempo — mesma marca, mais evidencia.
    let rival_mais_duro = placares
        .values()
        .filter(|placar| placar.derrotas > 0)
        .max_by(|a, b| {
            a.derrotas
                .cmp(&b.derrotas)
                .then_with(|| a.temporadas.cmp(&b.temporadas))
                .then_with(|| b.nome.cmp(&a.nome))
        })
        .map(|placar| DriverTeammateDuel {
            nome: placar.nome.clone(),
            temporadas: placar.temporadas,
            vitorias: placar.vitorias,
            derrotas: placar.derrotas,
        });

    Ok(DriverCareerTeammateBlock {
        companheiros: placares.len() as i32,
        temporadas: duplas.len() as i32,
        temporadas_vencidas,
        rival_mais_duro,
    })
}

/// As temporadas em que ele teve companheiro identificavel, com os pontos dos
/// dois. Temporada sem o par preenchido (equipe de um carro so, arquivo antigo)
/// fica de fora — ela nao e uma derrota, e sim uma comparacao que nao existe.
fn load_teammate_seasons(
    conn: &Connection,
    driver_id: &str,
) -> Result<Vec<TemporadaDeDupla>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                CASE WHEN t.piloto_1_id = ?1 THEN t.piloto_2_id ELSE t.piloto_1_id END,
                COALESCE(meu.pontos, 0),
                COALESCE(dele.pontos, 0)
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
        .map_err(|e| format!("Falha ao preparar confronto com companheiros: {e}"))?;
    let mapped = stmt
        .query_map(rusqlite::params![driver_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar confronto com companheiros: {e}"))?;

    let mut duplas = Vec::new();
    for row in mapped {
        let (companheiro_id, pontos_do_piloto, pontos_do_companheiro) =
            row.map_err(|e| format!("Falha ao ler confronto com companheiro: {e}"))?;
        let Some(companheiro_id) = companheiro_id.filter(|id| !id.trim().is_empty()) else {
            continue;
        };
        if companheiro_id == driver_id {
            continue;
        }
        duplas.push(TemporadaDeDupla {
            companheiro_id,
            pontos_do_piloto,
            pontos_do_companheiro,
        });
    }
    Ok(duplas)
}

/// O nome de cada companheiro. Sai do ARQUIVO, e nao da tabela de pilotos: um
/// companheiro de dez anos atras pode ter se aposentado e sumido do mundo, e o
/// duelo continua tendo acontecido.
fn load_driver_names<'a, I>(conn: &Connection, ids: I) -> Result<HashMap<String, String>, String>
where
    I: Iterator<Item = &'a String>,
{
    let mut nomes = HashMap::new();
    for id in ids {
        let nome = conn
            .query_row(
                "SELECT nome FROM driver_season_archive
                 WHERE piloto_id = ?1 AND TRIM(nome) <> ''
                 ORDER BY season_number DESC LIMIT 1",
                rusqlite::params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| format!("Falha ao resolver nome de companheiro: {e}"))?;
        if let Some(nome) = nome {
            nomes.insert(id.clone(), nome);
        }
    }
    Ok(nomes)
}
