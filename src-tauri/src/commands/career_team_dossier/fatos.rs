//! Fatos brutos do historico da equipe: leitura de corridas, titulos de construtores,
//! posicoes finais arquivadas e as agregacoes derivadas deles.

use super::*;

#[derive(Debug, Clone)]
pub(super) struct TeamRaceFact {
    pub(super) team_id: String,
    pub(super) season_number: i32,
    pub(super) season_year: i32,
    pub(super) category: String,
    pub(super) round: i32,
    pub(super) points: f64,
    pub(super) win: bool,
    pub(super) podium: bool,
    /// Melhor colocação da equipe naquela corrida (o menor `posicao_final` entre
    /// os carros dela). É o que separa um pódio de prata de um de bronze — com
    /// dois carros, a equipe leva o degrau mais alto que conseguiu.
    pub(super) best_position: Option<i32>,
    /// Semana do ano da corrida (1–47). Junto com `season_year` forma o relógio do
    /// mundo: a distância entre dois fatos sai de uma subtração, sem parsear data.
    pub(super) week_of_year: i32,
    /// Carros da equipe que ABANDONARAM aquela corrida (0, 1 ou 2 — a grade tem
    /// dois carros por equipe).
    ///
    /// Conta carro, e não corrida, de propósito: um fim de semana em que um dos
    /// dois quebrou é meio prejuízo, não um prejuízo inteiro, e somar por corrida
    /// apagaria a diferença entre a equipe que perde um carro e a que perde os
    /// dois. Ele NÃO entra na conta do top 5 — a colocação continua saindo do
    /// melhor carro, e um abandono do carro reserva não tira o pódio do outro.
    pub(super) dnfs: i32,
    /// Classe do carro naquela temporada ("mazda", "toyota", "bmw"), vazia nas
    /// categorias monomarca.
    ///
    /// A Production e a Endurance são multiclasse: três marcas disputam a mesma
    /// categoria em campeonatos separados. Sem a classe, o Grupo Mazda arrastaria
    /// para dentro as equipes de Toyota e BMW que correm na Production — que
    /// nunca dividiram a pista com uma Mazda.
    pub(super) class: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct TeamHistoryAggregate {
    pub(super) races: i32,
    pub(super) wins: i32,
    pub(super) podiums: i32,
    pub(super) points: f64,
}

#[derive(Debug, Clone)]
pub(super) struct TeamTitleFact {
    pub(super) season_id: String,
    pub(super) season_year: i32,
    pub(super) category: String,
    /// Pontos e vitórias da equipe na temporada do título. Já eram calculados
    /// para decidir quem ficou em primeiro; guardá-los é o que permite a galeria
    /// dizer COMO o título foi ganho em vez de só que foi.
    pub(super) points: f64,
    pub(super) wins: i32,
    /// Classe do campeonato em que o título foi ganho. Vazia nas monomarca.
    pub(super) class: String,
}

/// Campeão de pilotos de uma temporada numa categoria.
#[derive(Debug, Clone)]
pub(super) struct DriversChampion {
    pub(super) driver: String,
    pub(super) team_id: String,
    pub(super) team: String,
}

pub(super) fn load_team_race_facts(
    conn: &rusqlite::Connection,
    category_ids: &[String],
) -> Result<Vec<TeamRaceFact>, String> {
    if category_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            r.equipe_id,
            s.numero,
            s.ano,
            c.categoria,
            c.rodada,
            r.race_id,
            SUM(r.pontos) AS team_points,
            MAX(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS has_win,
            MAX(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END) AS has_podium,
            MIN(r.posicao_final) AS best_position,
            -- Classe do carro naquela temporada. O arquivo é a fonte histórica; a
            -- coluna da equipe cobre a temporada corrente, que ainda não foi
            -- arquivada. Sem o fallback, o ano em curso sairia sem classe e
            -- escaparia do recorte por marca justamente na tela que fala do agora.
            COALESCE(
                NULLIF(TRIM(a.classe), ''),
                NULLIF(TRIM(t.classe), ''),
                ''
            ) AS classe,
            c.week_of_year,
            SUM(CASE WHEN r.dnf <> 0 THEN 1 ELSE 0 END) AS dnfs
         FROM race_results r
         JOIN calendar c ON c.id = r.race_id
         JOIN seasons s ON s.id = c.temporada_id
         LEFT JOIN team_season_archive a
                ON a.team_id = r.equipe_id
               AND a.season_number = s.numero
               AND a.categoria = c.categoria
         LEFT JOIN teams t ON t.id = r.equipe_id
         WHERE c.categoria IN ({placeholders})
         GROUP BY r.equipe_id, s.numero, c.categoria, c.rodada, r.race_id
         ORDER BY s.numero ASC, c.rodada ASC, r.race_id ASC, r.equipe_id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar histórico real da equipe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(category_ids.iter()), |row| {
            Ok(TeamRaceFact {
                team_id: row.get(0)?,
                season_number: row.get(1)?,
                season_year: row.get(2)?,
                category: row.get(3)?,
                round: row.get(4)?,
                points: row.get(6)?,
                win: row.get::<_, i32>(7)? > 0,
                podium: row.get::<_, i32>(8)? > 0,
                best_position: row.get::<_, Option<i32>>(9)?,
                class: row.get::<_, String>(10)?.to_lowercase(),
                week_of_year: row.get::<_, Option<i32>>(11)?.unwrap_or(0),
                dnfs: row.get::<_, Option<i32>>(12)?.unwrap_or(0),
            })
        })
        .map_err(|e| format!("Falha ao consultar histórico real da equipe: {e}"))?;

    let mut facts = Vec::new();
    for row in rows {
        facts.push(row.map_err(|e| format!("Falha ao ler histórico real da equipe: {e}"))?);
    }
    Ok(facts)
}

pub(super) fn aggregate_team_history(
    facts: &[TeamRaceFact],
) -> HashMap<String, TeamHistoryAggregate> {
    let mut aggregates: HashMap<String, TeamHistoryAggregate> = HashMap::new();
    for fact in facts {
        let entry = aggregates.entry(fact.team_id.clone()).or_default();
        entry.races += 1;
        entry.points += fact.points;
        if fact.win {
            entry.wins += 1;
        }
        if fact.podium {
            entry.podiums += 1;
        }
    }
    aggregates
}

pub(super) fn load_constructor_titles_by_team(
    conn: &rusqlite::Connection,
    category_ids: &[String],
) -> Result<HashMap<String, Vec<TeamTitleFact>>, String> {
    if category_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            st.temporada_id,
            s.numero,
            s.ano,
            st.equipe_id,
            st.categoria,
            SUM(st.pontos) AS team_points,
            SUM(st.vitorias) AS team_wins,
            COALESCE(NULLIF(TRIM(st.classe), ''), '') AS classe
         FROM standings st
         JOIN seasons s ON s.id = st.temporada_id
         WHERE st.equipe_id IS NOT NULL
           AND st.categoria IN ({placeholders})
         GROUP BY st.temporada_id, s.numero, s.ano, st.equipe_id, st.categoria, classe
         ORDER BY s.numero ASC, team_points DESC, team_wins DESC, st.equipe_id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar títulos reais de equipes: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(category_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, i32>(6)?,
                row.get::<_, String>(7)?.to_lowercase(),
            ))
        })
        .map_err(|e| format!("Falha ao consultar títulos reais de equipes: {e}"))?;

    let mut best_by_season_category: BTreeMap<
        String,
        (i32, i32, String, String, f64, i32, String),
    > = BTreeMap::new();
    for row in rows {
        let (season_id, season_number, season_year, team_id, category, points, wins, class) =
            row.map_err(|e| format!("Falha ao ler títulos reais de equipes: {e}"))?;
        // A chave inclui a CLASSE: numa Production há três campeões, um por
        // marca. Sem ela o `LIMIT 1` por categoria dava o título da Production
        // inteira a quem fez mais pontos entre as três classes, e as outras duas
        // taças simplesmente não existiam.
        let key = format!("{season_id}:{category}:{class}");
        let replace = best_by_season_category
            .get(&key)
            .map(|(_, _, current_team, _, current_points, current_wins, _)| {
                points > *current_points
                    || ((points - *current_points).abs() < f64::EPSILON
                        && (wins > *current_wins
                            || (wins == *current_wins && team_id < *current_team)))
            })
            .unwrap_or(true);
        if replace {
            best_by_season_category.insert(
                key,
                (
                    season_number,
                    season_year,
                    team_id,
                    category,
                    points,
                    wins,
                    class,
                ),
            );
        }
    }

    let mut titles: HashMap<String, Vec<TeamTitleFact>> = HashMap::new();
    for (key, (_season_number, season_year, team_id, category, points, wins, class)) in
        best_by_season_category
    {
        let season_id = key.split(':').next().unwrap_or_default().to_string();
        titles.entry(team_id).or_default().push(TeamTitleFact {
            season_id,
            season_year,
            category,
            points,
            wins,
            class,
        });
    }
    Ok(titles)
}

/// Campeão de PILOTOS por temporada e categoria, indexado por `"{season_id}:{categoria}"`.
///
/// O título da galeria é de construtores; o de pilotos é outro campeonato, que
/// pode ter ido para outra equipe no mesmo ano. Sem isso, o card não respondia a
/// primeira pergunta que alguém faz ao ver um título: quem pilotava.
pub(super) fn load_drivers_champions(
    conn: &rusqlite::Connection,
    category_ids: &[String],
) -> HashMap<String, DriversChampion> {
    let mut champions = HashMap::new();
    if category_ids.is_empty() {
        return champions;
    }
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT st.temporada_id, st.categoria, d.nome, COALESCE(st.equipe_id, ''),
                COALESCE(t.nome, '')
         FROM standings st
         JOIN drivers d ON d.id = st.piloto_id
         LEFT JOIN teams t ON t.id = st.equipe_id
         WHERE st.categoria IN ({placeholders})
           AND st.posicao = 1
         ORDER BY st.temporada_id ASC, st.categoria ASC,
                  (st.classe IS NULL) DESC, st.pontos DESC, d.nome ASC"
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(stmt) => stmt,
        Err(_) => return champions,
    };
    let rows = match stmt.query_map(rusqlite::params_from_iter(category_ids.iter()), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return champions,
    };
    // Categoria multiclasse tem um campeão por classe. A ordenação põe o campeão
    // geral (`classe IS NULL`) na frente, e o primeiro de cada chave vence — numa
    // categoria só de classes, sobra o de mais pontos, que é o melhor palpite
    // possível para "o campeão do ano".
    for (season_id, categoria, driver, team_id, team) in rows.flatten() {
        champions
            .entry(format!("{season_id}:{categoria}"))
            .or_insert(DriversChampion {
                driver,
                team_id,
                team,
            });
    }
    champions
}

/// Posição final no campeonato por temporada (melhor posição se multiclasse).
/// Degrada para vazio se a tabela de arquivo ainda não existe.
pub(super) fn load_team_season_positions(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> HashMap<i32, i32> {
    let mut positions = HashMap::new();
    let mut stmt = match conn.prepare(
        "SELECT season_number, MIN(posicao_campeonato)
         FROM team_season_archive
         WHERE team_id = ?1 AND posicao_campeonato IS NOT NULL
         GROUP BY season_number",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return positions,
    };
    if let Ok(rows) = stmt.query_map(rusqlite::params![team_id], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?))
    }) {
        for row in rows.flatten() {
            positions.insert(row.0, row.1);
        }
    }
    positions
}

/// Identidade de exibição de uma equipe, para telas que listam o campo inteiro.
#[derive(Debug, Clone, Default)]
pub(super) struct TeamCard {
    pub(super) name: String,
    pub(super) color: String,
    pub(super) category_id: String,
    pub(super) active: bool,
}

/// Nome, cor e categoria atual de cada equipe. A tabela de recordes lista todas
/// as equipes do grupo, e sem isso cada linha seria um id.
pub(super) fn load_team_cards(conn: &rusqlite::Connection) -> HashMap<String, TeamCard> {
    let mut cards = HashMap::new();
    let mut stmt = match conn.prepare("SELECT id, nome, cor_primaria, categoria, ativa FROM teams")
    {
        Ok(stmt) => stmt,
        Err(_) => return cards,
    };
    if let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            TeamCard {
                name: row.get::<_, String>(1)?,
                color: row.get::<_, String>(2)?,
                category_id: row.get::<_, String>(3)?,
                active: row.get::<_, i32>(4)? != 0,
            },
        ))
    }) {
        for (id, card) in rows.flatten() {
            cards.insert(id, card);
        }
    }
    cards
}

/// Nome de cada equipe, por id. A campanha do campeonato desenha o campo
/// inteiro, e uma linha sem nome não pode nem ser identificada no tooltip.
/// Equipe ausente da tabela (dissolvida e removida) degrada para id vazio no
/// mapa — quem consome cai no rótulo genérico.
pub(super) fn load_team_names(conn: &rusqlite::Connection) -> HashMap<String, String> {
    let mut names = HashMap::new();
    let mut stmt = match conn.prepare("SELECT id, nome FROM teams") {
        Ok(stmt) => stmt,
        Err(_) => return names,
    };
    if let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        for (id, nome) in rows.flatten() {
            names.insert(id, nome);
        }
    }
    names
}

/// Número da temporada mais recente do save. A campanha só se anuncia "em
/// andamento" quando é ESTA — em qualquer temporada anterior o campeonato já
/// fechou, e a ponta da linha é resultado, não parcial.
pub(super) fn load_current_season_number(conn: &rusqlite::Connection) -> i32 {
    conn.query_row("SELECT COALESCE(MAX(numero), 0) FROM seasons", [], |row| {
        row.get::<_, i32>(0)
    })
    .unwrap_or(0)
}

/// Primeiro e último ano com temporada no save. É o eixo do mundo, não o da
/// equipe: a faixa do dossiê precisa dele para mostrar os anos em que a equipe
/// NÃO correu. Sem temporada nenhuma, devolve (0, 0).
pub(super) fn load_world_year_span(conn: &rusqlite::Connection) -> (i32, i32) {
    conn.query_row(
        "SELECT COALESCE(MIN(ano), 0), COALESCE(MAX(ano), 0) FROM seasons",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .unwrap_or((0, 0))
}

/// Anos em que a equipe correu FORA do recorte de categorias do dossiê, com a
/// categoria dominante de cada um.
///
/// O dossiê compara records dentro de um grupo ("Grupo GT3"), então as corridas
/// da equipe em outra escada não entram nos fatos. Sem esta consulta, a faixa
/// desenhava um "×" nesses anos — dizendo que a equipe não disputou nada, quando
/// ela disputou outro campeonato. É a diferença entre um buraco e uma mudança de
/// endereço.
pub(super) fn load_team_seasons_outside_scope(
    conn: &rusqlite::Connection,
    team_id: &str,
    scope_categories: &[String],
) -> Vec<(i32, String)> {
    let placeholders = if scope_categories.is_empty() {
        "''".to_string()
    } else {
        vec!["?"; scope_categories.len()].join(", ")
    };
    let sql = format!(
        "SELECT s.ano, c.categoria, COUNT(*) AS corridas
         FROM race_results r
         JOIN calendar c ON c.id = r.race_id
         JOIN seasons s ON s.id = c.temporada_id
         WHERE r.equipe_id = ?1
           AND c.categoria NOT IN ({placeholders})
         GROUP BY s.ano, c.categoria
         ORDER BY s.ano ASC, corridas DESC, c.categoria ASC"
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&team_id];
    for categoria in scope_categories {
        params.push(categoria);
    }
    let rows = match stmt.query_map(params.as_slice(), |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    // A consulta já vem ordenada por corridas dentro do ano, então o primeiro
    // registro de cada ano é a categoria dominante.
    let mut por_ano: BTreeMap<i32, String> = BTreeMap::new();
    for (ano, categoria) in rows.flatten() {
        por_ano.entry(ano).or_insert(categoria);
    }
    por_ano.into_iter().collect()
}

pub(super) fn distinct_seasons(facts: &[TeamRaceFact]) -> Vec<i32> {
    facts
        .iter()
        .map(|fact| fact.season_number)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
