//! v65 — o catálogo de incidentes deixa de guardar frase e passa a guardar chave.
//!
//! ## O que estava errado
//!
//! `incident_catalog` nasceu com três colunas de prosa em português — `dnf_template`,
//! `non_dnf_template` e `description_short` — semeadas pela migração. Elas viravam a
//! descrição do incidente na tela e o corpo da notícia. Trocar o idioma do jogo não mudava
//! nada: o texto estava no save, e não havia caminho de tradução sem mexer na tabela.
//!
//! ## O que esta migração faz
//!
//! As três colunas são **renomeadas** para `dnf_key`, `non_dnf_key` e `description_key`, e o
//! conteúdo passa a ser a chave de i18n derivada de `(id, severity_context)` — a mesma regra
//! que o seed usa, em [`super::seed_incidentes::chaves_de_texto`]. Renomear em vez de
//! reconstruir a tabela é decisão: índices, chave primária e o `dnf_catalog_id` que outras
//! tabelas guardam ficam intocados, e a operação é uma só no arquivo do jogador.
//!
//! Nada de comportamento muda. Peso, filtro e incidência dos 54 eventos são exatamente os
//! mesmos, e a identidade de cada entrada é o `id`, que não é tocado.
//!
//! ## O que acontece com id que a migração não reconhece
//!
//! Uma linha cujo `id` não está entre os 54 do seed não tem chave de locale para onde
//! apontar. Traduzir na marra seria inventar texto que ninguém escreveu, então o caminho é
//! outro: a prosa original vai inteira para `incident_catalog_texto_legado` **antes** do
//! rename, a migração conta quantas foram e registra a contagem no `loop.log`, e
//! `simulation::catalog` continua exibindo aquele texto como estava. O jogador não perde
//! nada e ninguém finge que aquilo foi traduzido.
//!
//! Na prática a tabela nasce vazia — o catálogo é semeado pela migração e não tem porta de
//! entrada pelo jogo. Ela existe para o caso em que não está vazia.

use std::collections::HashSet;

use rusqlite::Connection;

use super::seed_incidentes::{chaves_de_texto, ENTRADAS};
use crate::db::connection::DbError;

/// Quarentena do texto que a v65 não soube converter em chave.
pub const DDL_TEXTO_LEGADO: &str = "
    CREATE TABLE IF NOT EXISTS incident_catalog_texto_legado (
        id                TEXT PRIMARY KEY,
        dnf_template      TEXT,
        non_dnf_template  TEXT,
        description_short TEXT,
        migrado_em        TEXT NOT NULL
    );
";

/// Converte o catálogo de prosa para chave. Devolve quantas linhas foram para a quarentena.
///
/// Idempotente: se `dnf_key` já existe, a migração já rodou e não há o que fazer.
pub fn catalogo_guarda_chave(conn: &Connection) -> Result<usize, DbError> {
    conn.execute_batch(DDL_TEXTO_LEGADO)?;

    if coluna_existe(conn, "incident_catalog", "dnf_key")? {
        return Ok(0);
    }

    // ── 1. O que não é nosso vai inteiro para a quarentena, antes de qualquer rename ──
    let conhecidos: HashSet<&str> = ENTRADAS.iter().map(|e| e.0).collect();

    let linhas: Vec<(String, Option<String>, Option<String>, Option<String>)> = {
        let mut stmt = conn.prepare(
            "SELECT id, dnf_template, non_dnf_template, description_short
             FROM incident_catalog",
        )?;
        let it = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        it.collect::<Result<Vec<_>, _>>()?
    };

    let agora = chrono::Local::now().to_rfc3339();
    let mut preservadas = 0usize;
    for (id, dnf, non_dnf, curta) in &linhas {
        if conhecidos.contains(id.as_str()) {
            continue;
        }
        conn.execute(
            "INSERT OR REPLACE INTO incident_catalog_texto_legado
             (id, dnf_template, non_dnf_template, description_short, migrado_em)
             VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![id, dnf, non_dnf, curta, agora],
        )?;
        preservadas += 1;
    }

    // ── 2. As três colunas de prosa viram três colunas de chave ──────────────────────
    conn.execute_batch(
        "ALTER TABLE incident_catalog RENAME COLUMN dnf_template      TO dnf_key;
         ALTER TABLE incident_catalog RENAME COLUMN non_dnf_template  TO non_dnf_key;
         ALTER TABLE incident_catalog RENAME COLUMN description_short TO description_key;",
    )?;

    // ── 3. O conteúdo delas vira chave, derivada de (id, severity_context) ───────────
    //
    // O valor ANTIGO é ignorado de propósito. Num save de verdade ele é a prosa em PT; num
    // banco recém-criado pela baseline ele já é a chave. Derivar dos dois lados faz os dois
    // caminhos terminarem idênticos, sem precisar adivinhar qual dos dois se está lendo.
    let severidades: Vec<(String, String)> = {
        let mut stmt = conn.prepare("SELECT id, severity_context FROM incident_catalog")?;
        let it = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        it.collect::<Result<Vec<_>, _>>()?
    };
    for (id, severidade) in &severidades {
        let (dnf_key, non_dnf_key, description_key) = chaves_de_texto(id, severidade);
        conn.execute(
            "UPDATE incident_catalog
             SET dnf_key = ?1, non_dnf_key = ?2, description_key = ?3
             WHERE id = ?4",
            rusqlite::params![dnf_key, non_dnf_key, description_key, id],
        )?;
    }

    if preservadas > 0 {
        crate::diagnostico::linha(
            "migracao",
            &format!(
                "v65: {preservadas} entrada(s) de incident_catalog fora do catálogo de 54. \
                 O texto original foi preservado em incident_catalog_texto_legado e continua \
                 sendo exibido como estava; nenhuma tradução foi inventada."
            ),
        );
    }

    Ok(preservadas)
}

fn coluna_existe(conn: &Connection, tabela: &str, coluna: &str) -> Result<bool, DbError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info(\"{tabela}\")"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let nome: String = row.get(1)?;
        if nome == coluna {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recria a tabela na forma ANTERIOR à v65 — três colunas de prosa — e a semeia com o
    /// texto em português que os saves de verdade carregam.
    fn save_antes_da_v65() -> Connection {
        let conn = Connection::open_in_memory().expect("banco em memória");
        conn.execute_batch(
            "CREATE TABLE incident_catalog (
                id                TEXT PRIMARY KEY,
                vehicle_class     TEXT NOT NULL,
                race_format       TEXT NOT NULL,
                incident_source   TEXT NOT NULL,
                trigger_type      TEXT NOT NULL,
                severity_context  TEXT NOT NULL,
                weight_sprint     INTEGER NOT NULL DEFAULT 0,
                weight_endurance  INTEGER NOT NULL DEFAULT 0,
                dnf_template      TEXT NOT NULL,
                non_dnf_template  TEXT,
                description_short TEXT NOT NULL
            );
            CREATE INDEX idx_incident_catalog_source ON incident_catalog(incident_source);

            INSERT INTO incident_catalog VALUES
              ('SB_S_MEC_01','StreetBased','Sprint','Mechanical','Spontaneous','Both',100,0,
               '{driver} abandona com problema no câmbio – sincronizador da 3ª marcha falhou',
               '{driver} com dificuldade no câmbio – perdeu ritmo',
               'Câmbio – sincronizador'),
              ('SB_S_ERR_01','StreetBased','Both','DriverError','Spontaneous','NonDnfOnly',80,60,
               '',
               '{driver} escapou na saída da curva e perdeu posições',
               'Saída de pista'),
              ('SB_S_PIT_01','StreetBased','Sprint','Operational','PostSpinStall','DnfOnly',40,0,
               '{driver} rodou e não conseguiu religar o motor',
               NULL,
               'Rodada com stall');",
        )
        .expect("catálogo na forma antiga");
        conn
    }

    /// **Save antigo migra mecanicamente.** As três colunas trocam de nome e de conteúdo, e
    /// o que sobra é chave — nenhuma frase em português permanece na tabela.
    #[test]
    fn save_antigo_troca_prosa_por_chave() {
        let conn = save_antes_da_v65();
        let preservadas = catalogo_guarda_chave(&conn).expect("v65");
        assert_eq!(preservadas, 0, "os três ids são do catálogo semeado");

        assert!(coluna_existe(&conn, "incident_catalog", "dnf_key").expect("pragma"));
        assert!(coluna_existe(&conn, "incident_catalog", "non_dnf_key").expect("pragma"));
        assert!(coluna_existe(&conn, "incident_catalog", "description_key").expect("pragma"));
        assert!(!coluna_existe(&conn, "incident_catalog", "dnf_template").expect("pragma"));

        let (dnf, non_dnf, curta): (String, Option<String>, String) = conn
            .query_row(
                "SELECT dnf_key, non_dnf_key, description_key
                 FROM incident_catalog WHERE id = 'SB_S_MEC_01'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("linha migrada");
        assert_eq!(dnf, "breakdown.SB_S_MEC_01.dnf");
        assert_eq!(non_dnf.as_deref(), Some("breakdown.SB_S_MEC_01.warn"));
        assert_eq!(curta, "breakdown.SB_S_MEC_01.part");

        // A ausência de texto continua ausente: NonDnfOnly sem chave de DNF, DnfOnly sem
        // chave de não-DNF. É o mesmo par de buracos que a prosa tinha.
        let dnf_vazio: String = conn
            .query_row(
                "SELECT dnf_key FROM incident_catalog WHERE id = 'SB_S_ERR_01'",
                [],
                |r| r.get(0),
            )
            .expect("SB_S_ERR_01");
        assert_eq!(dnf_vazio, "");
        let non_dnf_nulo: Option<String> = conn
            .query_row(
                "SELECT non_dnf_key FROM incident_catalog WHERE id = 'SB_S_PIT_01'",
                [],
                |r| r.get(0),
            )
            .expect("SB_S_PIT_01");
        assert_eq!(non_dnf_nulo, None);

        // Nenhuma prosa sobrou: toda célula de texto ou é vazia ou começa por `breakdown.`.
        let fora_do_padrao: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM incident_catalog
                 WHERE (dnf_key <> '' AND dnf_key NOT LIKE 'breakdown.%')
                    OR (non_dnf_key IS NOT NULL AND non_dnf_key NOT LIKE 'breakdown.%')
                    OR description_key NOT LIKE 'breakdown.%'",
                [],
                |r| r.get(0),
            )
            .expect("varredura");
        assert_eq!(fora_do_padrao, 0);
    }

    /// **Id desconhecido é preservado, não traduzido.** A prosa vai inteira para a
    /// quarentena, com os parâmetros intactos, e a migração devolve a contagem para quem
    /// quiser reportar.
    #[test]
    fn id_fora_do_catalogo_vai_para_a_quarentena_com_o_texto_intacto() {
        let conn = save_antes_da_v65();
        conn.execute_batch(
            "INSERT INTO incident_catalog VALUES
              ('XX_CASEIRO_99','StreetBased','Sprint','Mechanical','Spontaneous','Both',50,0,
               '{driver} abandona com a bomba d''água estourada',
               '{driver} perdeu pressão de água',
               'Bomba d''água');",
        )
        .expect("linha caseira");

        let preservadas = catalogo_guarda_chave(&conn).expect("v65");
        assert_eq!(preservadas, 1);

        let (dnf, non_dnf, curta): (String, String, String) = conn
            .query_row(
                "SELECT dnf_template, non_dnf_template, description_short
                 FROM incident_catalog_texto_legado WHERE id = 'XX_CASEIRO_99'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("linha na quarentena");
        assert_eq!(dnf, "{driver} abandona com a bomba d'água estourada");
        assert_eq!(non_dnf, "{driver} perdeu pressão de água");
        assert_eq!(curta, "Bomba d'água");

        // A linha continua no catálogo, com peso e filtro intocados.
        let peso: i64 = conn
            .query_row(
                "SELECT weight_sprint FROM incident_catalog WHERE id = 'XX_CASEIRO_99'",
                [],
                |r| r.get(0),
            )
            .expect("peso preservado");
        assert_eq!(peso, 50);
    }

    /// Rodar duas vezes não quebra nem duplica: a segunda passada vê `dnf_key` e sai.
    #[test]
    fn rodar_de_novo_e_inofensivo() {
        let conn = save_antes_da_v65();
        catalogo_guarda_chave(&conn).expect("primeira");
        catalogo_guarda_chave(&conn).expect("segunda");

        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM incident_catalog", [], |r| r.get(0))
            .expect("contagem");
        assert_eq!(n, 3);
    }

    /// O índice sobrevive ao rename. É o motivo de a migração renomear em vez de
    /// reconstruir a tabela.
    #[test]
    fn o_indice_sobrevive() {
        let conn = save_antes_da_v65();
        catalogo_guarda_chave(&conn).expect("v65");
        let existe: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='index' AND name='idx_incident_catalog_source'",
                [],
                |r| r.get(0),
            )
            .expect("sqlite_master");
        assert_eq!(existe, 1);
    }
}
