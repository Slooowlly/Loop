//! A leitura da TABELA do save, para o rádio.
//!
//! Este é o único ponto do caminho do engenheiro que sabe o que é uma tabela SQL — o resto
//! (`engenheiro::campeonato`, `engenheiro::responder`) trabalha em cima de um valor já
//! lido, e por isso é testável sem banco nenhum.
//!
//! ## Por que não reusar o `Mundo` do feed de quebra
//!
//! [`commands::overlay::radio::contexto::Mundo`] carrega a mesma tabela e mais quatro
//! coisas — nêmesis, rivalidades, elenco das equipes, dono de cada assento. Ele paga isso
//! porque o feed dele redige falas que citam pilotos. Uma pergunta ao rádio não cita
//! ninguém: precisa de onde o JOGADOR está e de quanto o separa dos vizinhos na tabela.
//! Carregar as outras quatro consultas a cada pergunta seria pagar por informação que não
//! sai pela boca.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::engenheiro::campeonato::Contexto;

/// Pontos da temporada CORRENTE, por piloto, na categoria em que o jogador corre.
///
/// A mesma consulta do feed de quebra, e de propósito: duas definições de "pontos da
/// temporada" divergiriam em silêncio, e o sintoma seria o rádio dizer uma posição e a
/// tela do campeonato mostrar outra.
fn pontos_da_temporada(conn: &Connection, categoria: &str) -> Vec<(String, f64)> {
    let sql = "SELECT r.piloto_id, COALESCE(SUM(r.pontos), 0.0)
               FROM race_results r
               INNER JOIN calendar c ON c.id = r.race_id
               INNER JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
               WHERE s.status = 'EmAndamento' AND c.categoria = ?1
               GROUP BY r.piloto_id";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return Vec::new();
    };
    let Ok(linhas) = stmt.query_map([categoria], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    }) else {
        return Vec::new();
    };
    linhas.filter_map(Result::ok).collect()
}

/// Quantas corridas da temporada corrente ainda faltam na categoria do jogador.
///
/// A corrida que está sendo disputada AGORA conta como pendente: no Loop o resultado só entra
/// no save pela importação, depois. É isso que torna "esta é a última do ano" uma pergunta
/// respondível de dentro do carro — a resposta é `1`.
///
/// Mesma definição de temporada corrente de [`pontos_da_temporada`]; duas divergiriam em
/// silêncio, e o sintoma seria o rádio anunciar um título uma corrida cedo demais.
fn pendentes_da_temporada(conn: &Connection, categoria: &str) -> Option<i64> {
    conn.query_row(
        "SELECT COUNT(*)
         FROM calendar c
         INNER JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
         WHERE s.status = 'EmAndamento' AND c.categoria = ?1 AND c.status = 'Pendente'",
        [categoria],
        |row| row.get(0),
    )
    .ok()
}

/// O que o save sabe sobre a CARREIRA no instante da bandeirada. Ver
/// [`crate::engenheiro::marco`].
///
/// A projeção é a mesma de [`carregar`], e vem de fora justamente para não ser calculada duas
/// vezes: duas derivações do mesmo número divergiriam na borda, e a borda aqui é a diferença
/// entre anunciar um título e não anunciar.
pub fn marcos(conn: &Connection, projecao: Option<i32>) -> crate::engenheiro::marco::Contexto {
    use crate::engenheiro::marco::Contexto;

    let Ok(jogador) = crate::db::queries::drivers::get_player_driver(conn) else {
        return Contexto::default();
    };
    let ultima_da_temporada = jogador
        .categoria_atual
        .as_deref()
        .and_then(|cat| pendentes_da_temporada(conn, cat))
        == Some(1);
    Contexto {
        vitorias: jogador.stats_carreira.vitorias,
        corridas: jogador.stats_carreira.corridas,
        ultima_da_temporada,
        projecao,
    }
}

/// Onde o jogador está na tabela e por quanto.
///
/// Devolve o contexto ZERADO (`conhecido() == false`) em vez de erro quando não há o que
/// dizer: temporada não começada, jogador sem pontos, categoria desconhecida. O rádio trata
/// "não sei" calando sobre o assunto, que é o comportamento certo — um engenheiro não
/// anuncia que a tabela está vazia.
/// O mapa número do carro → piloto da carreira, escrito no export do roster.
///
/// Ausente é o caso normal de quem nunca exportou um grid, e devolve o mapa vazio — que
/// faz a projeção se recusar a sair, que é o comportamento certo.
pub fn por_numero(base_dir: &std::path::Path, career_id: &str) -> HashMap<i64, String> {
    let caminho = crate::commands::iracing::numbers_path(base_dir, career_id);
    let Ok(texto) = std::fs::read_to_string(caminho) else {
        return HashMap::new();
    };
    let porta: HashMap<String, i64> = serde_json::from_str(&texto).unwrap_or_default();
    porta.into_iter().map(|(id, n)| (n, id)).collect()
}

/// Onde o jogador está na tabela, por quanto, e onde ele TERMINARIA se a corrida acabasse
/// agora.
///
/// `ordem` e `minha_posicao` vêm da telemetria; sem corrida em andamento eles chegam vazios
/// e a projeção simplesmente não sai — o resto (posição e margens) continua valendo, porque
/// não depende do que está acontecendo na pista.
pub fn carregar(
    conn: &Connection,
    por_numero: &HashMap<i64, String>,
    ordem: &[(i32, i32)],
    minha_posicao: i32,
) -> Contexto {
    let Ok(jogador) = crate::db::queries::drivers::get_player_driver(conn) else {
        return Contexto::default();
    };
    let Some(categoria) = jogador.categoria_atual.as_deref() else {
        return Contexto::default();
    };

    let mut tabela = pontos_da_temporada(conn, categoria);
    // Ninguém pontuou: não há tabela, e não há o que dizer sobre ela.
    if tabela.iter().all(|(_, p)| *p <= 0.0) {
        return Contexto::default();
    }
    // Decrescente por pontos. O desempate por id é arbitrário e não importa: o que sai pela
    // boca é a posição do jogador e a diferença para os vizinhos DELE, e ambas continuam
    // verdadeiras qualquer que seja a ordem entre dois empatados.
    tabela.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let Some(i) = tabela.iter().position(|(id, _)| *id == jogador.id) else {
        return Contexto::default();
    };
    Contexto {
        posicao: i as i32 + 1,
        para_o_proximo: (i > 0).then(|| tabela[i - 1].1 - tabela[i].1),
        folga: tabela.get(i + 1).map(|(_, p)| tabela[i].1 - p),
        projecao: crate::engenheiro::campeonato::projetar(
            &tabela,
            ordem,
            por_numero,
            &jogador.id,
            minha_posicao,
            categoria == "endurance",
        ),
    }
}
