//! Recordes históricos da categoria: líderes de vitórias, pódios, poles, jejuns e afins.

use rusqlite::{Connection, OptionalExtension};

use crate::db::connection::DbError;

/// Recordista (líder histórico) de uma métrica na categoria.
#[derive(Debug, Clone)]
pub struct CategoryRecord {
    pub pilot_id: String,
    pub pilot_name: String,
    pub value: i32,
}

/// Líderes históricos da categoria em vitórias, pódios e largadas (todas as temporadas).
#[derive(Debug, Clone, Default)]
pub struct CategoryRecords {
    pub most_wins: Option<CategoryRecord>,
    pub most_podiums: Option<CategoryRecord>,
    pub most_starts: Option<CategoryRecord>,
    pub most_poles: Option<CategoryRecord>,
    /// 2º maior número de vitórias (valor). Usado para detectar quando o recorde de
    /// vitórias acabou de ser SUPERADO (líder == 2º+1).
    pub second_most_wins: Option<i32>,
    /// 2º maior número de pódios (valor). Mesmo uso para o recorde de pódios.
    pub second_most_podiums: Option<i32>,
    /// 2º maior número de poles (valor). Mesmo uso para o recorde de poles.
    pub second_most_poles: Option<i32>,
}

/// Calcula, numa passada, quem lidera a categoria em vitórias, pódios e largadas.
pub fn get_category_records(
    conn: &Connection,
    categoria: &str,
) -> Result<CategoryRecords, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.piloto_id, d.nome,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS wins,
            SUM(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END) AS podiums,
            COUNT(*) AS starts,
            SUM(CASE WHEN r.posicao_largada = 1 THEN 1 ELSE 0 END) AS poles
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN drivers d ON r.piloto_id = d.id
         WHERE c.categoria = ?1
         GROUP BY r.piloto_id",
    )?;

    let mut records = CategoryRecords::default();
    // Top-2 de vitórias, pódios e poles (valores) para detectar recorde recém-superado.
    let mut top1_wins = i32::MIN;
    let mut top2_wins = i32::MIN;
    let mut top1_podiums = i32::MIN;
    let mut top2_podiums = i32::MIN;
    let mut top1_poles = i32::MIN;
    let mut top2_poles = i32::MIN;
    let mut rows = stmt.query(rusqlite::params![categoria])?;
    while let Some(row) = rows.next()? {
        let pilot_id: String = row.get(0)?;
        let pilot_name: String = row.get(1)?;
        let wins: i32 = row.get(2)?;
        let podiums: i32 = row.get(3)?;
        let starts: i32 = row.get(4)?;
        let poles: i32 = row.get(5)?;
        // Atualiza o recordista de uma métrica se este piloto a supera (>0).
        let beats = |cur: &Option<CategoryRecord>, val: i32| {
            val > 0 && cur.as_ref().map_or(true, |c| val > c.value)
        };
        if beats(&records.most_wins, wins) {
            records.most_wins = Some(CategoryRecord {
                pilot_id: pilot_id.clone(),
                pilot_name: pilot_name.clone(),
                value: wins,
            });
        }
        if beats(&records.most_podiums, podiums) {
            records.most_podiums = Some(CategoryRecord {
                pilot_id: pilot_id.clone(),
                pilot_name: pilot_name.clone(),
                value: podiums,
            });
        }
        if beats(&records.most_starts, starts) {
            records.most_starts = Some(CategoryRecord {
                pilot_id: pilot_id.clone(),
                pilot_name: pilot_name.clone(),
                value: starts,
            });
        }
        if beats(&records.most_poles, poles) {
            records.most_poles = Some(CategoryRecord {
                pilot_id,
                pilot_name,
                value: poles,
            });
        }
        // Top-2 de vitórias (mantém duplicatas: empate no topo → top2 == top1).
        if wins > top1_wins {
            top2_wins = top1_wins;
            top1_wins = wins;
        } else if wins > top2_wins {
            top2_wins = wins;
        }
        // Top-2 de pódios, mesma lógica.
        if podiums > top1_podiums {
            top2_podiums = top1_podiums;
            top1_podiums = podiums;
        } else if podiums > top2_podiums {
            top2_podiums = podiums;
        }
        // Top-2 de poles, mesma lógica.
        if poles > top1_poles {
            top2_poles = top1_poles;
            top1_poles = poles;
        } else if poles > top2_poles {
            top2_poles = poles;
        }
    }
    if top2_wins > i32::MIN {
        records.second_most_wins = Some(top2_wins);
    }
    if top2_podiums > i32::MIN {
        records.second_most_podiums = Some(top2_podiums);
    }
    if top2_poles > i32::MIN {
        records.second_most_poles = Some(top2_poles);
    }
    Ok(records)
}

/// Maior recuperação numa única corrida da categoria (posições ganhas do grid à
/// chegada) ANTES da corrida atual, identificada por (temporada, rodada) para excluí-la.
/// Só corridas terminadas (`dnf = 0`) com grid válido. Devolve `(pilot_id, nome, posicoes)`.
pub fn get_category_comeback_record(
    conn: &Connection,
    categoria: &str,
    exclude_temporada_id: &str,
    exclude_round: i32,
) -> Result<Option<CategoryRecord>, DbError> {
    let row = conn
        .query_row(
            "SELECT r.piloto_id, d.nome, (r.posicao_largada - r.posicao_final) AS gained
             FROM race_results r
             JOIN calendar c ON r.race_id = c.id
             JOIN drivers d ON r.piloto_id = d.id
             WHERE c.categoria = ?1 AND r.dnf = 0 AND r.posicao_largada > 0
               AND NOT (c.temporada_id = ?2 AND c.rodada = ?3)
             ORDER BY gained DESC
             LIMIT 1",
            rusqlite::params![categoria, exclude_temporada_id, exclude_round],
            |r| {
                Ok(CategoryRecord {
                    pilot_id: r.get(0)?,
                    pilot_name: r.get(1)?,
                    value: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Maior número de títulos na categoria entre TODOS os pilotos EXCETO um (para saber se
/// o campeão da temporada passou a ser dono isolado do recorde). 0 se ninguém mais tem.
///
/// `classe` restringe o recorde a uma classe da categoria — nas multiclasse cada classe
/// tem seu campeonato, então o campeão da Mazda não disputa recorde com o da BMW. Tem que
/// casar com o filtro de [`super::get_pilot_category_titles`], senão a comparação mistura
/// um lado por classe com o outro por categoria. `None` nas categorias de classe única.
pub fn get_category_titles_leader_excluding(
    conn: &Connection,
    categoria: &str,
    exclude_pilot: &str,
    classe: Option<&str>,
) -> Result<i32, DbError> {
    let n: i32 = conn.query_row(
        "SELECT COALESCE(MAX(cnt), 0) FROM (
            SELECT piloto_id, COUNT(*) AS cnt FROM driver_season_archive
            WHERE categoria = ?1 AND posicao_campeonato = 1 AND piloto_id <> ?2
              AND (?3 IS NULL
                   OR COALESCE(NULLIF(TRIM(json_extract(snapshot_json, '$.classe')), ''), '') = ?3)
            GROUP BY piloto_id
         )",
        rusqlite::params![categoria, exclude_pilot, classe],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Piloto com MAIS LARGADAS SEM NUNCA VENCER na categoria (o "eterno azarão").
pub fn get_category_most_starts_no_win(
    conn: &Connection,
    categoria: &str,
) -> Result<Option<CategoryRecord>, DbError> {
    let row = conn
        .query_row(
            "SELECT r.piloto_id, d.nome, COUNT(*) AS starts
             FROM race_results r JOIN calendar c ON r.race_id = c.id
             JOIN drivers d ON r.piloto_id = d.id
             WHERE c.categoria = ?1
             GROUP BY r.piloto_id
             HAVING SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) = 0
             ORDER BY starts DESC LIMIT 1",
            rusqlite::params![categoria],
            |r| {
                Ok(CategoryRecord {
                    pilot_id: r.get(0)?,
                    pilot_name: r.get(1)?,
                    value: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Piloto com MAIS ABANDONOS (DNFs) na carreira da categoria.
pub fn get_category_most_career_dnfs(
    conn: &Connection,
    categoria: &str,
) -> Result<Option<CategoryRecord>, DbError> {
    let row = conn
        .query_row(
            "SELECT r.piloto_id, d.nome, SUM(r.dnf) AS dnfs
             FROM race_results r JOIN calendar c ON r.race_id = c.id
             JOIN drivers d ON r.piloto_id = d.id
             WHERE c.categoria = ?1
             GROUP BY r.piloto_id
             HAVING dnfs > 0
             ORDER BY dnfs DESC LIMIT 1",
            rusqlite::params![categoria],
            |r| {
                Ok(CategoryRecord {
                    pilot_id: r.get(0)?,
                    pilot_name: r.get(1)?,
                    value: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Piloto com MAIS PONTOS na carreira da categoria.
pub fn get_category_most_career_points(
    conn: &Connection,
    categoria: &str,
) -> Result<Option<CategoryRecord>, DbError> {
    let row = conn
        .query_row(
            "SELECT r.piloto_id, d.nome, CAST(SUM(r.pontos) AS INTEGER) AS pts
             FROM race_results r JOIN calendar c ON r.race_id = c.id
             JOIN drivers d ON r.piloto_id = d.id
             WHERE c.categoria = ?1
             GROUP BY r.piloto_id
             ORDER BY pts DESC LIMIT 1",
            rusqlite::params![categoria],
            |r| {
                Ok(CategoryRecord {
                    pilot_id: r.get(0)?,
                    pilot_name: r.get(1)?,
                    value: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Piloto com MAIS POLES SEM NUNCA TER SIDO CAMPEÃO na categoria.
pub fn get_category_most_poles_no_title(
    conn: &Connection,
    categoria: &str,
) -> Result<Option<CategoryRecord>, DbError> {
    let row = conn
        .query_row(
            "SELECT r.piloto_id, d.nome,
                    SUM(CASE WHEN r.posicao_largada = 1 THEN 1 ELSE 0 END) AS poles
             FROM race_results r JOIN calendar c ON r.race_id = c.id
             JOIN drivers d ON r.piloto_id = d.id
             WHERE c.categoria = ?1
             GROUP BY r.piloto_id
             HAVING poles > 0 AND (
                SELECT COUNT(*) FROM driver_season_archive a
                WHERE a.piloto_id = r.piloto_id AND a.categoria = ?1 AND a.posicao_campeonato = 1
             ) = 0
             ORDER BY poles DESC LIMIT 1",
            rusqlite::params![categoria],
            |r| {
                Ok(CategoryRecord {
                    pilot_id: r.get(0)?,
                    pilot_name: r.get(1)?,
                    value: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Dupla piloto-equipe mais longeva da categoria: `(pilot_id, "Piloto @ Equipe", temporadas)`.
pub fn get_category_longest_pairing(
    conn: &Connection,
    categoria: &str,
) -> Result<Option<CategoryRecord>, DbError> {
    let row = conn
        .query_row(
            "SELECT b.piloto_id, d.nome || ' / ' || t.nome, b.temporadas
             FROM driver_team_bond b
             JOIN teams t ON b.equipe_id = t.id
             JOIN drivers d ON b.piloto_id = d.id
             WHERE t.categoria = ?1
             ORDER BY b.temporadas DESC LIMIT 1",
            rusqlite::params![categoria],
            |r| {
                Ok(CategoryRecord {
                    pilot_id: r.get(0)?,
                    pilot_name: r.get(1)?,
                    value: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Recorde de vitórias numa ÚNICA temporada na categoria (lido de `standings`, que é
/// gravado no fim de cada temporada — logo, NÃO inclui a temporada corrente).
pub fn get_category_single_season_win_record(
    conn: &Connection,
    categoria: &str,
) -> Result<Option<CategoryRecord>, DbError> {
    let row = conn
        .query_row(
            "SELECT s.piloto_id, d.nome, s.vitorias
             FROM standings s JOIN drivers d ON s.piloto_id = d.id
             WHERE s.categoria = ?1
             ORDER BY s.vitorias DESC
             LIMIT 1",
            rusqlite::params![categoria],
            |r| {
                Ok(CategoryRecord {
                    pilot_id: r.get(0)?,
                    pilot_name: r.get(1)?,
                    value: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Piloto que mais venceu por uma equipe na categoria (todas as temporadas). `None`
/// se a equipe nunca venceu na categoria.
pub fn get_team_top_winner_in_category(
    conn: &Connection,
    team_id: &str,
    categoria: &str,
) -> Result<Option<CategoryRecord>, DbError> {
    let res = conn
        .query_row(
            "SELECT r.piloto_id, d.nome,
                SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS wins
             FROM race_results r
             JOIN calendar c ON r.race_id = c.id
             JOIN drivers d ON r.piloto_id = d.id
             WHERE r.equipe_id = ?1 AND c.categoria = ?2
             GROUP BY r.piloto_id
             HAVING wins > 0
             ORDER BY wins DESC, d.nome ASC
             LIMIT 1",
            rusqlite::params![team_id, categoria],
            |r| {
                Ok(CategoryRecord {
                    pilot_id: r.get(0)?,
                    pilot_name: r.get(1)?,
                    value: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(res)
}

/// Vitórias na categoria de cada piloto que AINDA corre nela (ativo + categoria_atual).
/// Serve para comparar o vencedor com rivais que ainda estão no grid. Só `wins > 0`.
pub fn get_active_category_win_counts(
    conn: &Connection,
    categoria: &str,
) -> Result<Vec<CategoryRecord>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT r.piloto_id, d.nome,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS wins
         FROM race_results r
         JOIN calendar c ON r.race_id = c.id
         JOIN drivers d ON r.piloto_id = d.id
         WHERE c.categoria = ?1
           AND d.status != 'Aposentado'
           AND d.categoria_atual = ?1
         GROUP BY r.piloto_id
         HAVING wins > 0",
    )?;
    let mut out = Vec::new();
    let mut rows = stmt.query(rusqlite::params![categoria])?;
    while let Some(row) = rows.next()? {
        out.push(CategoryRecord {
            pilot_id: row.get(0)?,
            pilot_name: row.get(1)?,
            value: row.get(2)?,
        });
    }
    Ok(out)
}

/// Ano (calendário) em que a marca de `target_wins` vitórias acumuladas foi atingida
/// pela PRIMEIRA vez na categoria, por qualquer piloto. Usado para dizer há quanto
/// tempo um recorde resistia. `None` se ninguém jamais chegou a essa marca.
pub fn first_year_reaching_wins(
    conn: &Connection,
    categoria: &str,
    target_wins: i32,
) -> Result<Option<i32>, DbError> {
    if target_wins <= 0 {
        return Ok(None);
    }
    // MIN(ano) sempre retorna uma linha (NULL se ninguém atingiu a marca).
    let year: Option<i32> = conn.query_row(
        "WITH wins AS (
            SELECT s.ano AS ano,
                   ROW_NUMBER() OVER (
                       PARTITION BY r.piloto_id
                       ORDER BY s.numero ASC, c.rodada ASC
                   ) AS rn
            FROM race_results r
            JOIN calendar c ON r.race_id = c.id
            JOIN seasons s ON c.temporada_id = s.id
            WHERE c.categoria = ?1 AND r.posicao_final = 1
         )
         SELECT MIN(ano) FROM wins WHERE rn = ?2",
        rusqlite::params![categoria, target_wins],
        |row| row.get::<_, Option<i32>>(0),
    )?;
    Ok(year)
}
