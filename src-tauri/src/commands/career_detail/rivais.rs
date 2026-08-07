//! Rivais do piloto: quem são, de onde a briga nasceu e o que já aconteceu entre
//! os dois na pista.
//!
//! O motor de rivalidade (`crate::rivalry`) sabe QUANTO um rival pesa — dois eixos
//! 0–100, memória e calor — mas não sabe contar nenhum fato. Uma ficha que mostra
//! só os eixos entrega três números numa escala que o jogador não vê, e a aba
//! inteira fica sem nada que tenha acontecido de verdade.
//!
//! O confronto direto é o que falta, e ele está em `race_results`: os dois têm
//! linha na mesma corrida sempre que dividiram o grid. Daí saem as três coisas
//! que a tela precisa — quantas vezes se cruzaram, quem terminou à frente, e como
//! foi a última. É a mesma leitura que o dossiê de equipe já faz entre equipes
//! (`career_team_dossier::identidade`), aqui entre pilotos.

use rusqlite::OptionalExtension;

use super::*;

/// No máximo quatro rivais: a aba mostra um em destaque e os outros em linha, e
/// além disso a cauda da lista é sempre ruído de intensidade baixa.
const MAX_RIVAIS: usize = 4;

pub(super) fn build_driver_rivals_block(
    conn: &Connection,
    driver: &Driver,
) -> Result<DriverRivalsBlock, String> {
    let mut rivalries = crate::rivalry::get_pilot_rivalries(conn, &driver.id)
        .map_err(|e| format!("Falha ao carregar rivalidades do piloto: {e}"))?;
    // Ordenado pela percebida, e não pela ordem que o banco devolveu: a tela
    // promove o primeiro da lista a rival principal, e sem ordenar esse posto
    // saía por sorteio de `rowid`.
    rivalries.sort_by(|left, right| {
        right
            .perceived_intensity
            .total_cmp(&left.perceived_intensity)
            .then_with(|| left.rival_id.cmp(&right.rival_id))
    });

    let confrontos_disponiveis =
        sqlite_table_exists(conn, "race_results")? && sqlite_table_exists(conn, "calendar")?;

    let mut itens = Vec::new();
    for rivalry in rivalries.into_iter().take(MAX_RIVAIS) {
        let rival = driver_queries::get_driver(conn, &rivalry.rival_id).ok();
        let nome = rival
            .as_ref()
            .map(|value| value.nome.clone())
            .unwrap_or_else(|| rivalry.rival_id.clone());
        let categoria_rival = rival
            .as_ref()
            .and_then(|value| value.categoria_atual.clone());
        let equipe = load_rival_team(conn, &rivalry.rival_id)?;

        let confronto = if confrontos_disponiveis {
            head_to_head(conn, &driver.id, &rivalry.rival_id)?
        } else {
            HeadToHead::default()
        };
        let dupla = companheirismo(
            conn,
            &driver.id,
            &rivalry.rival_id,
            &confronto.por_temporada,
        )?;

        itens.push(DriverRivalInfo {
            driver_id: rivalry.rival_id,
            nome,
            tipo: rivalry.tipo.as_str().to_string(),
            nivel_chave: nivel_chave(rivalry.perceived_intensity),
            intensidade: rivalry.perceived_intensity.round().clamp(0.0, 100.0) as u8,
            intensidade_historica: rivalry.historical_intensity.round().clamp(0.0, 100.0) as u8,
            atividade_recente: rivalry.recent_activity.round().clamp(0.0, 100.0) as u8,
            confrontos: confronto.encontros.len() as i32,
            vitorias: confronto.vitorias,
            derrotas: confronto.derrotas,
            vitorias_quali: confronto.vitorias_quali,
            derrotas_quali: confronto.derrotas_quali,
            gap_medio: confronto.gap_medio,
            companheirismo: dupla,
            encontros: confronto.encontros,
            // A categoria do piloto guarda só a base ("endurance"), que é o
            // recorte certo aqui: dividir grid é o que importa, e a classe dentro
            // dele é detalhe de campeonato.
            mesma_categoria: match (
                driver.categoria_atual.as_deref(),
                categoria_rival.as_deref(),
            ) {
                (Some(minha), Some(dele)) => minha == dele,
                _ => false,
            },
            categoria_atual: categoria_rival.as_deref().map(rotulo_de_categoria),
            equipe_nome: equipe.as_ref().map(|(nome, _)| nome.clone()),
            equipe_cor: equipe.map(|(_, cor)| cor),
        });
    }

    Ok(DriverRivalsBlock { itens })
}

/// A faixa semântica da intensidade percebida, na chave que a tela traduz.
/// Deriva de `rivalry::intensity_level` para os limiares viverem num lugar só —
/// o mesmo que decide quando nasce notícia de escalada.
fn nivel_chave(perceived: f64) -> String {
    use crate::rivalry::RivalryIntensityLevel::*;
    match crate::rivalry::intensity_level(perceived) {
        AtritoLeve => "atrito_leve",
        Inicial => "inicial",
        Clara => "clara",
        Forte => "forte",
        Intensa => "intensa",
    }
    .to_string()
}

fn rotulo_de_categoria(categoria: &str) -> String {
    categories::get_category_config(categoria)
        .map(|config| config.nome_curto.to_string())
        .unwrap_or_else(|| categoria.to_string())
}

/// Equipe atual do rival — nome e cor. A cor é o que tira a aba do cinza: cada
/// card passa a ser uma pessoa de uma equipe, e não mais uma laje igual à do lado.
fn load_rival_team(conn: &Connection, rival_id: &str) -> Result<Option<(String, String)>, String> {
    conn.query_row(
        "SELECT nome, cor_primaria FROM teams
         WHERE piloto_1_id = ?1 OR piloto_2_id = ?1
         LIMIT 1",
        rusqlite::params![rival_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar equipe do rival: {e}"))
}

#[derive(Default)]
struct HeadToHead {
    encontros: Vec<DriverRivalMeeting>,
    vitorias: i32,
    derrotas: i32,
    vitorias_quali: i32,
    derrotas_quali: i32,
    gap_medio: Option<f64>,
    /// Placar por temporada, para o recorte de companheiros de equipe.
    por_temporada: HashMap<i32, (i32, i32)>,
}

/// Uma linha do confronto: as duas metades do mesmo domingo.
struct LinhaDeConfronto {
    season_number: i32,
    ano: i32,
    rodada: i32,
    pista: String,
    minha_posicao: i32,
    meu_dnf: bool,
    posicao_dele: i32,
    dnf_dele: bool,
    minha_largada: i32,
    largada_dele: i32,
    meu_gap_ms: f64,
    gap_dele_ms: f64,
}

/// Toda corrida em que os dois têm resultado, do mais antigo para o mais recente.
///
/// A ordem cronológica não é detalhe de consulta: é ela que deixa a tela desenhar
/// a faixa do confronto na sequência em que a briga aconteceu, e mostrar quando a
/// maré virou. Um placar diz quanto; só a ordem diz quando.
///
/// ABANDONO É ENCONTRO, MAS NÃO É DUELO. Um motor que quebrou na volta 3 não diz
/// nada sobre quem é mais rápido, e contá-lo como derrota transformaria azar em
/// hierarquia. Sai do placar e continua na faixa como marca neutra — o dia
/// aconteceu, só não decidiu nada.
fn head_to_head(conn: &Connection, driver_id: &str, rival_id: &str) -> Result<HeadToHead, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                COALESCE(s.numero, 0) AS season_number,
                COALESCE(s.ano, 0),
                c.rodada,
                COALESCE(NULLIF(c.track_name, ''), c.pista),
                meu.posicao_final,
                meu.dnf,
                dele.posicao_final,
                dele.dnf,
                meu.posicao_largada,
                dele.posicao_largada,
                meu.gap_to_winner_ms,
                dele.gap_to_winner_ms
             FROM race_results meu
             INNER JOIN race_results dele
                     ON dele.race_id = meu.race_id AND dele.piloto_id = ?2
             INNER JOIN calendar c ON c.id = meu.race_id
             LEFT JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
             WHERE meu.piloto_id = ?1
             ORDER BY season_number ASC, c.rodada ASC, meu.id ASC",
        )
        .map_err(|e| format!("Falha ao preparar confronto direto: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![driver_id, rival_id], |row| {
            Ok(LinhaDeConfronto {
                season_number: row.get(0)?,
                ano: row.get(1)?,
                rodada: row.get(2)?,
                pista: row.get(3)?,
                minha_posicao: row.get(4)?,
                meu_dnf: row.get::<_, i32>(5)? != 0,
                posicao_dele: row.get(6)?,
                dnf_dele: row.get::<_, i32>(7)? != 0,
                minha_largada: row.get(8)?,
                largada_dele: row.get(9)?,
                meu_gap_ms: row.get(10)?,
                gap_dele_ms: row.get(11)?,
            })
        })
        .map_err(|e| format!("Falha ao consultar confronto direto: {e}"))?;

    let mut placar = HeadToHead::default();
    let mut soma_gap = 0.0_f64;
    let mut corridas_com_gap = 0_i32;

    for row in rows {
        let linha = row.map_err(|e| format!("Falha ao ler confronto direto: {e}"))?;

        // A CLASSIFICAÇÃO independe do que aconteceu no domingo: o grid foi
        // formado, e quem largou à frente largou à frente mesmo que o motor
        // tenha quebrado na primeira volta. Por isso o abandono não filtra aqui
        // como filtra no placar de corrida. Largada zerada é corrida sem grid
        // registrado (save antigo), e essa fica de fora dos dois lados.
        if linha.minha_largada > 0 && linha.largada_dele > 0 {
            if linha.minha_largada < linha.largada_dele {
                placar.vitorias_quali += 1;
            } else if linha.largada_dele < linha.minha_largada {
                placar.derrotas_quali += 1;
            }
        }

        let ambos_terminaram = !linha.meu_dnf && !linha.dnf_dele;

        // O GAP só existe entre dois carros que cruzaram a linha. E a corrida em
        // que os dois marcam gap zero é save antigo sem o campo gravado, não um
        // empate perfeito — ela entraria na média puxando tudo para o zero.
        let gap = (ambos_terminaram && (linha.meu_gap_ms > 0.0 || linha.gap_dele_ms > 0.0))
            .then(|| (linha.meu_gap_ms - linha.gap_dele_ms) / 1000.0);
        if let Some(gap) = gap {
            soma_gap += gap;
            corridas_com_gap += 1;
        }

        let vencedor = if !ambos_terminaram {
            "nenhum"
        } else if linha.minha_posicao < linha.posicao_dele {
            placar.vitorias += 1;
            "piloto"
        } else if linha.posicao_dele < linha.minha_posicao {
            placar.derrotas += 1;
            "rival"
        } else {
            "nenhum"
        };

        if ambos_terminaram && vencedor != "nenhum" {
            let temporada = placar
                .por_temporada
                .entry(linha.season_number)
                .or_insert((0, 0));
            if vencedor == "piloto" {
                temporada.0 += 1;
            } else {
                temporada.1 += 1;
            }
        }

        placar.encontros.push(DriverRivalMeeting {
            ano: linha.ano,
            season_number: linha.season_number,
            rodada: linha.rodada,
            pista: linha.pista,
            vencedor: vencedor.to_string(),
            gap,
        });
    }

    if corridas_com_gap > 0 {
        placar.gap_medio = Some(soma_gap / f64::from(corridas_com_gap));
    }

    Ok(placar)
}

/// As temporadas em que os dois dividiram o MESMO box, e o confronto restrito a
/// elas.
///
/// É a única comparação da ficha sem o carro no meio: em qualquer outro recorte,
/// "ele terminou à frente" pode ser o pacote falando. Aqui não pode.
fn companheirismo(
    conn: &Connection,
    driver_id: &str,
    rival_id: &str,
    por_temporada: &HashMap<i32, (i32, i32)>,
) -> Result<Option<DriverRivalTeammateSpell>, String> {
    if !sqlite_table_exists(conn, "team_season_archive")? {
        return Ok(None);
    }

    let mut stmt = conn
        .prepare(
            "SELECT t.season_number, t.ano, COALESCE(e.nome, '')
             FROM team_season_archive t
             LEFT JOIN teams e ON e.id = t.team_id
             WHERE (t.piloto_1_id = ?1 AND t.piloto_2_id = ?2)
                OR (t.piloto_1_id = ?2 AND t.piloto_2_id = ?1)
             ORDER BY t.season_number ASC",
        )
        .map_err(|e| format!("Falha ao preparar temporadas de companheirismo: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![driver_id, rival_id], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar temporadas de companheirismo: {e}"))?;

    let mut anos = Vec::new();
    let mut equipe: Option<String> = None;
    let mut vitorias = 0;
    let mut derrotas = 0;
    for row in rows {
        let (season_number, ano, nome_equipe) =
            row.map_err(|e| format!("Falha ao ler temporada de companheirismo: {e}"))?;
        anos.push(ano);
        if equipe.is_none() && !nome_equipe.trim().is_empty() {
            equipe = Some(nome_equipe);
        }
        if let Some((v, d)) = por_temporada.get(&season_number) {
            vitorias += v;
            derrotas += d;
        }
    }

    if anos.is_empty() {
        return Ok(None);
    }
    Ok(Some(DriverRivalTeammateSpell {
        equipe,
        anos,
        vitorias,
        derrotas,
    }))
}
