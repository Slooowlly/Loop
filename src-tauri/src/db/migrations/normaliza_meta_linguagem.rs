//! Normalização dos textos de IA já gravados no save: some com a meta-linguagem de
//! quarta parede que os fatos antigos usavam para marcar o piloto do jogador.
//!
//! ## Por que isto existe, e por que existe AQUI
//!
//! Os fatos curados marcavam o piloto do jogador com expressões de quarta parede
//! ("Fulano, piloto acompanhado pelo leitor, ..."), e o modelo copiava a marcação para
//! dentro da matéria. A origem foi corrigida na redação dos fatos (rótulos em linguagem
//! de universo, ver `narrative.context` nos locales), mas o texto velho fica PERSISTIDO
//! em dois papéis diferentes:
//!
//! - `ai_race_story.facts` é ENTRADA: um save antigo continua mandando a redação velha ao
//!   servidor a cada boletim que ainda não foi gerado, então a origem volta a vazar;
//! - as demais colunas são SAÍDA já em cache, redigida antes de existir filtro nenhum.
//!
//! Enquanto isso não fosse normalizado, `narrative::client` precisava filtrar o texto que
//! VOLTAVA do servidor a cada chamada — um filtro no caminho quente para consertar dado
//! que dava para consertar uma vez só. Esta migração é essa vez só. Com ela, o filtro sai
//! da saída de rede e a transformação passa a viver aqui, onde tem um único consumidor.
//!
//! ## Por que é seguro
//!
//! A transformação é a MESMA que o filtro aplicava, byte por byte: nada é reescrito, só
//! removido ou trocado pelo substituto neutro da tabela. Nenhuma redação nova é escolhida
//! aqui. Linha que não contém nenhuma das frases não é regravada, e as duas colunas que
//! guardam JSON só são regravadas se o resultado continuar sendo JSON válido.

use rusqlite::Connection;

use crate::db::connection::DbError;

/// Expressões de quarta parede e o substituto neutro de cada uma.
///
/// Ordem: da mais longa pra mais curta, senão a curta come um pedaço da longa e deixa
/// palavra pendurada — invariante preso pelo teste
/// `a_lista_de_meta_linguagem_vai_da_mais_longa_para_a_mais_curta`, e não pela mão de quem
/// acrescentar a próxima.
const FRASES_META: [(&str, &str); 9] = [
    ("piloto acompanhado pelo leitor", "piloto"),
    ("piloto acompanhada pelo leitor", "piloto"),
    ("acompanhado pelo leitor", ""),
    ("acompanhada pelo leitor", ""),
    ("piloto do leitor", "piloto"),
    ("driver followed by the reader", "driver"),
    ("followed by the reader", ""),
    ("the reader's driver", "the driver"),
    ("reader's driver", "driver"),
];

/// As colunas de texto de IA que podem carregar a redação velha.
///
/// `facts` e `notes_json` guardam JSON; as outras, prosa solta. O papel de cada uma está
/// no doc do módulo.
const COLUNAS_DE_TEXTO: &[(&str, &str, &str, bool)] = &[
    // (tabela, chave primária, coluna de texto, é JSON)
    ("ai_race_story", "news_id", "facts", true),
    ("ai_race_story", "news_id", "story", false),
    ("ai_pre_race_briefing", "race_id", "headline", false),
    ("ai_pre_race_briefing", "race_id", "narrative", false),
    ("ai_pre_race_briefing", "race_id", "team_voice", false),
    ("ai_post_race_debrief", "race_id", "headline", false),
    ("ai_post_race_debrief", "race_id", "body", false),
    ("ai_world_notes", "cache_key", "notes_json", true),
];

/// Busca ASCII case-insensitive. As frases são 100% ASCII, então comparar bytes é
/// seguro: byte ASCII nunca aparece no meio de um caractere multibyte de UTF-8, logo
/// todo match cai em fronteira de caractere.
fn achar_ascii_ci(texto: &str, frase: &str, a_partir_de: usize) -> Option<usize> {
    let t = texto.as_bytes();
    let f = frase.as_bytes();
    if f.is_empty() || t.len() < f.len() || a_partir_de > t.len() - f.len() {
        return None;
    }
    (a_partir_de..=t.len() - f.len()).find(|&i| t[i..i + f.len()].eq_ignore_ascii_case(f))
}

/// Remove do texto a família de expressões que quebram a quarta parede. Quando a frase é
/// um aposto (", piloto acompanhado pelo leitor,"), cai o aposto inteiro; solta no meio da
/// frase, entra o substituto neutro.
pub(super) fn remover_vazamento_meta(texto: &str) -> String {
    let mut t = texto.to_string();
    for (frase, neutro) in FRASES_META {
        let mut de = 0;
        while let Some(i) = achar_ascii_ci(&t, frase, de) {
            let j = i + frase.len();
            let antes = t[..i].trim_end();
            let abre_virgula = antes.ends_with(',');
            let abre_parentese = antes.ends_with('(');
            // Byte onde começa a remoção de um aposto/parentético: a vírgula ou o
            // '(' que o abre (ambos ASCII, 1 byte).
            let corte = antes.len().saturating_sub(1);
            let depois = t[j..].trim_start();
            let proximo = depois.chars().next();
            let fim_sem_espaco = t.len() - depois.len();
            if abre_virgula
                && matches!(
                    proximo,
                    Some(',' | '.' | ';' | ':' | ')' | '!' | '?') | None
                )
            {
                // Aposto entre vírgulas: some com a vírgula de abertura e a frase,
                // a pontuação seguinte fecha a oração anterior.
                t.replace_range(corte..fim_sem_espaco, "");
                de = corte;
            } else if abre_parentese && proximo == Some(')') {
                // Parentético: "(piloto acompanhado pelo leitor)" cai inteiro.
                let fim = fim_sem_espaco + ')'.len_utf8();
                t.replace_range(corte..fim, "");
                de = corte;
            } else {
                t.replace_range(i..j, neutro);
                de = i + neutro.len();
            }
        }
    }
    // Cicatrizes da remoção: espaço duplo e espaço antes de pontuação.
    while t.contains("  ") {
        t = t.replace("  ", " ");
    }
    t.replace(" ,", ",").replace(" .", ".").trim().to_string()
}

/// Verdadeiro se o texto contém alguma das frases — o filtro barato que evita reescrever
/// linha que não precisa (e evita mexer no espaçamento de JSON íntegro).
fn tem_meta_linguagem(texto: &str) -> bool {
    FRASES_META
        .iter()
        .any(|(frase, _)| achar_ascii_ci(texto, frase, 0).is_some())
}

/// Passa o normalizador nas colunas de texto de IA do save.
///
/// Devolve quantas linhas foram regravadas — útil no teste e no diagnóstico, não no fluxo
/// do jogo. Coluna que guarda JSON só é regravada se o resultado continuar parseável: se
/// alguma redação futura na tabela quebrar o parse, a linha fica como estava em vez de
/// virar cache corrompido.
pub(super) fn normaliza_textos_de_ia(conn: &Connection) -> Result<usize, DbError> {
    let mut regravadas = 0usize;
    for (tabela, chave, coluna, e_json) in COLUNAS_DE_TEXTO {
        let mut stmt = conn.prepare(&format!(
            "SELECT {chave}, {coluna} FROM {tabela} WHERE {coluna} IS NOT NULL"
        ))?;
        let linhas: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        drop(stmt);

        for (id, texto) in linhas {
            if !tem_meta_linguagem(&texto) {
                continue;
            }
            let limpo = remover_vazamento_meta(&texto);
            if limpo == texto {
                continue;
            }
            if *e_json && serde_json::from_str::<serde_json::Value>(&limpo).is_err() {
                continue;
            }
            conn.execute(
                &format!("UPDATE {tabela} SET {coluna} = ?1 WHERE {chave} = ?2"),
                rusqlite::params![limpo, id],
            )?;
            regravadas += 1;
        }
    }
    Ok(regravadas)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn banco() -> Connection {
        let conn = Connection::open_in_memory().expect("banco em memória");
        crate::db::migrations::run_all(&conn).expect("migrações");
        conn
    }

    /// O caso real que motivou o filtro: o modelo colou o rótulo interno como aposto.
    #[test]
    fn vazamento_em_aposto_cai_com_o_aposto_inteiro() {
        assert_eq!(
            remover_vazamento_meta(
                "Para Carlos Magno, piloto acompanhado pelo leitor, a etapa terminou em P8."
            ),
            "Para Carlos Magno, a etapa terminou em P8."
        );
    }

    #[test]
    fn vazamento_no_meio_da_frase_vira_termo_neutro() {
        assert_eq!(
            remover_vazamento_meta("O piloto acompanhado pelo leitor cruzou em oitavo."),
            "O piloto cruzou em oitavo."
        );
        assert_eq!(
            remover_vazamento_meta("A batida do piloto do leitor custou posições."),
            "A batida do piloto custou posições."
        );
    }

    #[test]
    fn vazamento_entre_parenteses_cai_com_os_parenteses() {
        assert_eq!(
            remover_vazamento_meta("Carlos Magno (piloto acompanhado pelo leitor) venceu."),
            "Carlos Magno venceu."
        );
    }

    #[test]
    fn vazamento_em_ingles_tambem_cai() {
        let limpo = remover_vazamento_meta(
            "For Carlos Magno, the driver followed by the reader, it ended early.",
        );
        assert!(!limpo.to_lowercase().contains("reader"), "{limpo}");
    }

    /// A ORDEM da lista é o que faz a limpeza funcionar, e mantê-la à mão é frágil: quem
    /// acrescentar uma frase curta no topo faz a longa nunca casar, e o vazamento volta
    /// com meia frase pendurada ("piloto acompanhado pelo leitor" vira "piloto pelo
    /// leitor"). O invariante é este: nenhuma entrada pode estar CONTIDA numa entrada
    /// posterior.
    #[test]
    fn a_lista_de_meta_linguagem_vai_da_mais_longa_para_a_mais_curta() {
        for (i, (curta, _)) in FRASES_META.iter().enumerate() {
            for (longa, _) in FRASES_META.iter().skip(i + 1) {
                assert!(
                    !longa.contains(curta),
                    "'{curta}' aparece antes de '{longa}', que a contém: a curta casaria \
                     primeiro e deixaria o resto da longa no texto. Suba a mais longa."
                );
            }
        }
    }

    /// Texto limpo (com acento e pontuação normais) atravessa a normalização intacto.
    #[test]
    fn texto_sem_meta_passa_intacto() {
        let texto = "Miguel Sanz venceu em Interlagos com autoridade, e o pelotão sentiu.";
        assert_eq!(remover_vazamento_meta(texto), texto);
    }

    /// O caso que a migração existe para resolver: fatos gravados na redação velha.
    #[test]
    fn a_migracao_limpa_fatos_e_boletim_ja_gravados() {
        let conn = banco();
        let facts = serde_json::json!({
            "jogador": "Carlos Magno, piloto acompanhado pelo leitor, largou quarto.",
            "resumo": "A batida do piloto do leitor custou posições."
        })
        .to_string();
        crate::db::queries::ai_story::store_race_facts(&conn, "N1", &facts, "{}")
            .expect("grava fatos");
        crate::db::queries::ai_story::set_story(
            &conn,
            "N1",
            "O piloto acompanhado pelo leitor cruzou em oitavo.",
        )
        .expect("grava boletim");

        assert_eq!(normaliza_textos_de_ia(&conn).expect("normaliza"), 2);

        let linha = crate::db::queries::ai_story::get_story(&conn, "N1")
            .expect("lê")
            .expect("linha");
        assert!(
            !tem_meta_linguagem(&linha.facts),
            "os fatos ainda carregam meta-linguagem: {}",
            linha.facts
        );
        assert!(
            serde_json::from_str::<serde_json::Value>(&linha.facts).is_ok(),
            "os fatos deixaram de ser JSON válido: {}",
            linha.facts
        );
        assert_eq!(
            linha.story.expect("boletim"),
            "O piloto cruzou em oitavo.",
            "o boletim em cache não foi normalizado"
        );
    }

    /// Roda duas vezes: a segunda não regrava nada. Migração de dado tem de ser
    /// idempotente porque `run_all` e `run_pending` batem no mesmo código.
    #[test]
    fn normalizar_duas_vezes_nao_muda_mais_nada() {
        let conn = banco();
        crate::db::queries::ai_post_race::set_post_race(
            &conn,
            "R1",
            "Fim de linha",
            "Você, piloto acompanhado pelo leitor, perdeu o eixo na segunda volta.",
        )
        .expect("grava debrief");

        assert_eq!(normaliza_textos_de_ia(&conn).expect("1ª"), 1);
        assert_eq!(
            normaliza_textos_de_ia(&conn).expect("2ª"),
            0,
            "a segunda passada regravou linha — a transformação não é idempotente"
        );
    }

    /// Save sem nenhum texto de IA (o caso comum de banco novo) passa pela migração sem
    /// erro e sem tocar em nada.
    #[test]
    fn banco_sem_texto_de_ia_passa_limpo() {
        let conn = banco();
        assert_eq!(normaliza_textos_de_ia(&conn).expect("normaliza"), 0);
    }
}
