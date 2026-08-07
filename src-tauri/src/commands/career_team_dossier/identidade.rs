//! Identidade da equipe: origem, heranca, perfil de desempenho, rival historico e
//! piloto simbolo.

use super::*;

#[derive(Debug, Clone, Default)]
pub(super) struct DriverSymbolAggregate {
    pub(super) name: String,
    pub(super) races: i32,
    pub(super) wins: i32,
    pub(super) podiums: i32,
}

pub(super) fn build_real_team_identity(
    conn: &rusqlite::Connection,
    team_id: &str,
    category: &str,
    record_scope: &str,
    selected_facts: &[TeamRaceFact],
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    titles: i32,
) -> Result<TeamHistoryIdentity, String> {
    let origin_category = selected_facts
        .first()
        .map(|fact| fact.category.as_str())
        .unwrap_or(category);
    let current = current_team_category_label(conn, team_id)
        .unwrap_or_else(|| team_history_category_label(category));
    let races = selected_facts.len() as i32;
    let wins = selected_facts.iter().filter(|fact| fact.win).count() as i32;
    let podiums = selected_facts.iter().filter(|fact| fact.podium).count() as i32;
    let profile = real_team_profile(races, wins, podiums, titles);
    let team_name = conn
        .query_row(
            "SELECT nome FROM teams WHERE id = ?1",
            rusqlite::params![team_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao buscar equipe para identidade real: {e}"))?
        .unwrap_or_else(|| rust_i18n::t!("team_dossier.team_fallback").to_string());
    let rival = real_team_rival(conn, team_id, selected_facts, aggregates, record_scope)?;
    let symbol = real_symbol_driver(conn, team_id, selected_facts)?;
    let (best_track, worst_track) = real_track_affinity(conn, team_id, selected_facts)?;
    let recruitment = real_recruitment_dna(conn, team_id, selected_facts)?;

    let heritage = team_heritage_label(distinct_seasons(selected_facts).len() as i32);

    Ok(TeamHistoryIdentity {
        origin: team_history_category_label(origin_category),
        current,
        heritage,
        profile: team_profile_label(profile),
        summary: real_identity_summary(&team_name, profile, races, titles),
        profile_races: races,
        profile_wins: wins,
        profile_podiums: podiums,
        rival,
        symbol_driver: symbol.name,
        symbol_driver_detail: symbol.detail,
        symbol_driver_years: symbol.years,
        symbol_driver_active: symbol.active,
        symbol_driver_nationality: symbol.nationality,
        symbol_driver_races: symbol.races,
        symbol_driver_wins: symbol.wins,
        symbol_driver_podiums: symbol.podiums,
        best_track,
        worst_track,
        recruitment,
    })
}

fn current_team_category_label(conn: &rusqlite::Connection, team_id: &str) -> Option<String> {
    let category = conn
        .query_row(
            "SELECT categoria FROM teams WHERE id = ?1",
            rusqlite::params![team_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()?;
    Some(team_history_category_label(&category))
}

/// Herança da equipe por EXPERIÊNCIA real (temporadas competidas), substituindo
/// o antigo corte de ano de fundação (1970) que rotulava quase todo time igual.
fn team_heritage_label(seasons: i32) -> String {
    let key = match seasons {
        0 => "debutant",
        1..=2 => "new",
        3..=6 => "rising",
        7..=14 => "established",
        _ => "traditional",
    };
    let full = format!("team_dossier.heritage.{key}");
    rust_i18n::t!(&full).to_string()
}

/// Perfil de DESEMPENHO por histórico real, numa escada coerente do topo ao
/// fundo do grid. Fonte única de verdade (o fallback do frontend só vale durante
/// o carregamento). Taxas calculadas no nível de corrida.
/// Perfil de DESEMPENHO por histórico real. Retorna uma CHAVE estável (não o texto)
/// — o display resolve por `team_profile_label` e a lógica de resumo casa a chave,
/// evitando o antipattern de comparar prosa traduzível.
fn real_team_profile(races: i32, wins: i32, podiums: i32, titles: i32) -> &'static str {
    if races < 4 {
        return "forming";
    }
    let win_rate = wins as f64 / races as f64;
    let podium_rate = podiums as f64 / races as f64;

    // "Dominante" por TAXA precisa de amostra: com o piso único de 4 corridas, uma
    // equipe de duas temporadas com 6 pódios em 10 provas cruzava o corte e o card
    // dizia "Dominante" ao lado de um cabeçalho com 0 títulos. Título conquistado
    // dispensa amostra — é fato, não taxa; a taxa só vale a partir de MIN_SAMPLE.
    const MIN_SAMPLE_FOR_RATE_TOP: i32 = 12;
    let rate_sample_ok = races >= MIN_SAMPLE_FOR_RATE_TOP;

    if titles > 0 || (rate_sample_ok && (win_rate >= 0.30 || podium_rate >= 0.60)) {
        "dominant"
    } else if win_rate >= 0.10 {
        "winning"
    } else if podium_rate >= 0.30 {
        "competitive"
    } else if podium_rate >= 0.10 {
        "midfield"
    } else {
        "support"
    }
}

/// Rótulo de display do perfil (i18n) a partir da chave estável.
fn team_profile_label(key: &str) -> String {
    let full = format!("team_dossier.profile.{key}");
    rust_i18n::t!(&full).to_string()
}

fn real_identity_summary(team_name: &str, profile_key: &str, races: i32, _titles: i32) -> String {
    match profile_key {
        "dominant" => {
            rust_i18n::t!("team_dossier.identity_summary.dominant", team = team_name).to_string()
        }
        "winning" => {
            rust_i18n::t!("team_dossier.identity_summary.winning", team = team_name).to_string()
        }
        "competitive" => rust_i18n::t!(
            "team_dossier.identity_summary.competitive",
            team = team_name
        )
        .to_string(),
        "midfield" => {
            rust_i18n::t!("team_dossier.identity_summary.midfield", team = team_name).to_string()
        }
        "support" => {
            rust_i18n::t!("team_dossier.identity_summary.support", team = team_name).to_string()
        }
        _ => rust_i18n::t!(
            "team_dossier.identity_summary.forming",
            team = team_name,
            races = races
        )
        .to_string(),
    }
}

/// Maior rival histórico da equipe.
///
/// A fonte de verdade é o motor de `rivalry::team`: ele guarda de ONDE a rivalidade
/// nasceu (tabela, mercado, pista, transbordamento dos pilotos) e os dois eixos de
/// intensidade. A heurística de confronto compartilhado só entra quando o mundo
/// ainda não registrou rivalidade nenhuma — sozinha ela elege "quem dividiu mais
/// corridas comigo", que num grid fixo é toda a concorrência.
fn real_team_rival(
    conn: &rusqlite::Connection,
    team_id: &str,
    selected_facts: &[TeamRaceFact],
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    record_scope: &str,
) -> Result<TeamHistoryRival, String> {
    let shared_races = head_to_head_by_team(conn, team_id, selected_facts)?;

    if let Some(rival) = engine_team_rival(conn, team_id, &shared_races, record_scope)? {
        return Ok(rival);
    }

    heuristic_team_rival(conn, &shared_races, aggregates, team_id, record_scope)
}

/// O retrospecto da equipe do dossiê contra UM adversário no recorte.
#[derive(Debug, Clone, Default)]
pub(super) struct HeadToHead {
    pub(super) shared: i32,
    pub(super) wins: i32,
    pub(super) losses: i32,
    /// O encontro mais recente, ordenado por (temporada, rodada).
    pub(super) last: Option<TeamHistoryRivalMeeting>,
}

/// Confronto direto contra cada outra equipe do recorte: quantas vezes se
/// cruzaram, quem terminou à frente e como foi a última vez.
///
/// A comparação é entre as MELHORES colocações de cada equipe na corrida (a que
/// `TeamRaceFact` já carrega), então não há empate — com dois carros, vale o
/// degrau mais alto que cada uma alcançou.
fn head_to_head_by_team(
    conn: &rusqlite::Connection,
    team_id: &str,
    selected_facts: &[TeamRaceFact],
) -> Result<HashMap<String, HeadToHead>, String> {
    let mine: HashMap<(i32, i32, String), &TeamRaceFact> = selected_facts
        .iter()
        .map(|fact| {
            (
                (fact.season_number, fact.round, fact.category.clone()),
                fact,
            )
        })
        .collect();
    let mut records: HashMap<String, HeadToHead> = HashMap::new();
    let mut latest: HashMap<String, (i32, i32)> = HashMap::new();

    let all_facts = load_team_race_facts(conn, &fact_category_ids(selected_facts))?;
    // O "agora" do mundo é a corrida mais recente que existe no recorte — a
    // carreira não tem relógio de parede, tem calendário. Com ele a idade de um
    // encontro sai de uma subtração de semanas, sem parsear data nenhuma.
    let now = all_facts
        .iter()
        .map(|fact| (fact.season_year, fact.week_of_year))
        .max()
        .unwrap_or((0, 0));

    for fact in all_facts {
        if fact.team_id == team_id {
            continue;
        }
        let key = (fact.season_number, fact.round, fact.category.clone());
        let Some(ours) = mine.get(&key) else {
            continue;
        };
        let entry = records.entry(fact.team_id.clone()).or_default();
        entry.shared += 1;

        // Corrida em que uma das duas não teve colocação registrada conta como
        // encontro, mas não como confronto: não há o que comparar.
        let (Some(ours_position), Some(theirs_position)) = (ours.best_position, fact.best_position)
        else {
            continue;
        };
        if ours_position < theirs_position {
            entry.wins += 1;
        } else if theirs_position < ours_position {
            entry.losses += 1;
        }

        let stamp = (fact.season_number, fact.round);
        let is_latest = latest
            .get(&fact.team_id)
            .is_none_or(|current| stamp > *current);
        if is_latest {
            latest.insert(fact.team_id.clone(), stamp);
            entry.last = Some(TeamHistoryRivalMeeting {
                year: ours.season_year,
                round: fact.round,
                position: ours_position,
                rival_position: theirs_position,
                weeks_ago: weeks_between((fact.season_year, fact.week_of_year), now),
            });
        }
    }
    Ok(records)
}

/// Distância em semanas entre dois pontos do calendário do mundo, cada um dado
/// como `(ano, semana do ano)`. Nunca negativa: ponto no futuro vira zero.
fn weeks_between(from: (i32, i32), to: (i32, i32)) -> i32 {
    const WEEKS_PER_YEAR: i32 = 52;
    ((to.0 - from.0) * WEEKS_PER_YEAR + (to.1 - from.1)).max(0)
}

/// Rival vindo do motor de rivalidade de equipes, escolhido pela intensidade
/// percebida (0.4·histórico + 0.6·recente). Rivalidade é conceito VITALÍCIO, então
/// a escolha ignora o recorte de categoria — o recorte entra só na nota de confronto.
fn engine_team_rival(
    conn: &rusqlite::Connection,
    team_id: &str,
    shared_races: &HashMap<String, HeadToHead>,
    record_scope: &str,
) -> Result<Option<TeamHistoryRival>, String> {
    let mut rivalries = crate::rivalry::team::get_team_rivalries(conn, team_id)
        .map_err(|e| format!("Falha ao ler rivalidades de equipe: {e}"))?;
    rivalries.sort_by(|left, right| {
        right
            .perceived_intensity
            .total_cmp(&left.perceived_intensity)
            .then_with(|| left.rival_id.cmp(&right.rival_id))
    });

    for rivalry in rivalries {
        // Equipe pode ter sido arquivada/removida; nesse caso segue para a próxima.
        let Some((name, category, color)) = conn
            .query_row(
                "SELECT nome, categoria, cor_primaria FROM teams WHERE id = ?1",
                rusqlite::params![&rivalry.rival_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| format!("Falha ao buscar rival histórico real: {e}"))?
        else {
            continue;
        };
        let record = shared_races
            .get(&rivalry.rival_id)
            .cloned()
            .unwrap_or_default();
        return Ok(Some(TeamHistoryRival {
            name,
            current_category: team_history_category_label(&category),
            note: shared_races_note(record.shared, record_scope),
            color,
            origin_kind: Some(rival_origin_label(&rivalry.tipo)),
            historical_intensity: Some(rivalry.historical_intensity),
            recent_activity: Some(rivalry.recent_activity),
            perceived_intensity: Some(rivalry.perceived_intensity),
            head_to_head_wins: record.wins,
            head_to_head_losses: record.losses,
            last_meeting: record.last,
        }));
    }

    Ok(None)
}

fn rival_origin_label(tipo: &crate::models::team_rivalry::TeamRivalryType) -> String {
    let key = format!("team_dossier.rival_origin.{}", tipo.as_str().to_lowercase());
    rust_i18n::t!(&key).to_string()
}

fn shared_races_note(shared: i32, record_scope: &str) -> String {
    if shared == 0 {
        return rust_i18n::t!("team_dossier.rival_note_no_shared", scope = record_scope)
            .to_string();
    }
    let key = if shared == 1 {
        "team_dossier.rival_note_shared_one"
    } else {
        "team_dossier.rival_note_shared_other"
    };
    rust_i18n::t!(key, count = shared.to_string(), scope = record_scope).to_string()
}

fn heuristic_team_rival(
    conn: &rusqlite::Connection,
    shared_races: &HashMap<String, HeadToHead>,
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    team_id: &str,
    record_scope: &str,
) -> Result<TeamHistoryRival, String> {
    let selected_points = aggregates
        .get(team_id)
        .map(|entry| entry.points)
        .unwrap_or(0.0);
    let rival_id = shared_races
        .iter()
        .max_by(|(left_id, left), (right_id, right)| {
            left.shared.cmp(&right.shared).then_with(|| {
                let left_gap = aggregates
                    .get(left_id.as_str())
                    .map(|entry| (entry.points - selected_points).abs())
                    .unwrap_or(f64::MAX);
                let right_gap = aggregates
                    .get(right_id.as_str())
                    .map(|entry| (entry.points - selected_points).abs())
                    .unwrap_or(f64::MAX);
                right_gap.total_cmp(&left_gap)
            })
        })
        .map(|(id, _)| id.clone());

    let Some(rival_id) = rival_id else {
        return Ok(TeamHistoryRival {
            name: rust_i18n::t!("team_dossier.rival_none").to_string(),
            current_category: record_scope.to_string(),
            note: rust_i18n::t!("team_dossier.rival_none_note").to_string(),
            color: String::new(),
            origin_kind: None,
            historical_intensity: None,
            recent_activity: None,
            perceived_intensity: None,
            head_to_head_wins: 0,
            head_to_head_losses: 0,
            last_meeting: None,
        });
    };

    let (name, category, color): (String, String, String) = conn
        .query_row(
            "SELECT nome, categoria, cor_primaria FROM teams WHERE id = ?1",
            rusqlite::params![&rival_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| format!("Falha ao buscar rival histórico real: {e}"))?;
    let record = shared_races.get(&rival_id).cloned().unwrap_or_default();
    Ok(TeamHistoryRival {
        name,
        current_category: team_history_category_label(&category),
        note: shared_races_note(record.shared, record_scope),
        color,
        origin_kind: None,
        historical_intensity: None,
        recent_activity: None,
        perceived_intensity: None,
        head_to_head_wins: record.wins,
        head_to_head_losses: record.losses,
        last_meeting: record.last,
    })
}

/// O piloto símbolo já resolvido para a tela: nome, prosa de números, o intervalo
/// de anos pela equipe e se ele ainda está lá. Os dois últimos são o que separa
/// "o cara que construiu isso aqui" de "o cara que ganhou duas e foi embora".
pub(super) struct SymbolDriverView {
    pub(super) name: String,
    pub(super) detail: String,
    pub(super) years: String,
    pub(super) active: bool,
    pub(super) nationality: String,
    pub(super) races: i32,
    pub(super) wins: i32,
    pub(super) podiums: i32,
}

impl SymbolDriverView {
    fn none(detail_key: &str) -> Self {
        Self {
            name: rust_i18n::t!("team_dossier.symbol_none").to_string(),
            detail: rust_i18n::t!(detail_key).to_string(),
            years: String::new(),
            active: false,
            nationality: String::new(),
            races: 0,
            wins: 0,
            podiums: 0,
        }
    }
}

fn real_symbol_driver(
    conn: &rusqlite::Connection,
    team_id: &str,
    selected_facts: &[TeamRaceFact],
) -> Result<SymbolDriverView, String> {
    if selected_facts.is_empty() {
        return Ok(SymbolDriverView::none("team_dossier.symbol_none_no_races"));
    }
    let category_ids = fact_category_ids(selected_facts);
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            r.piloto_id,
            d.nome,
            COUNT(DISTINCT r.race_id) AS races,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS wins,
            SUM(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END) AS podiums,
            MIN(s.ano) AS first_year,
            MAX(s.ano) AS last_year,
            d.nacionalidade
         FROM race_results r
         JOIN calendar c ON c.id = r.race_id
         JOIN seasons s ON s.id = c.temporada_id
         JOIN drivers d ON d.id = r.piloto_id
         WHERE r.equipe_id = ?1
           AND c.categoria IN ({placeholders})
         GROUP BY r.piloto_id, d.nome, d.nacionalidade
         ORDER BY wins DESC, podiums DESC, races DESC, d.nome ASC
         LIMIT 1"
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&team_id];
    for category in &category_ids {
        params.push(category);
    }
    let symbol = conn
        .query_row(&sql, params.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                DriverSymbolAggregate {
                    name: row.get(1)?,
                    races: row.get(2)?,
                    wins: row.get(3)?,
                    podiums: row.get(4)?,
                },
                row.get::<_, i32>(5)?,
                row.get::<_, i32>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .optional()
        .map_err(|e| format!("Falha ao buscar piloto símbolo real: {e}"))?;

    let Some((driver_id, symbol, first_year, last_year, nationality)) = symbol else {
        return Ok(SymbolDriverView::none(
            "team_dossier.symbol_none_no_results",
        ));
    };

    // Contrato regular ativo é o que define "ainda está aqui" no mundo do Loop —
    // a tabela `drivers` não guarda equipe.
    let active = conn
        .query_row(
            "SELECT 1 FROM contracts
              WHERE piloto_id = ?1 AND equipe_id = ?2
                AND status = 'Ativo' AND tipo = 'Regular'
              LIMIT 1",
            rusqlite::params![&driver_id, team_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| format!("Falha ao checar contrato do piloto símbolo: {e}"))?
        .is_some();

    let years = if first_year == last_year {
        first_year.to_string()
    } else {
        format!("{first_year}–{last_year}")
    };

    Ok(SymbolDriverView {
        name: symbol.name,
        detail: format!(
            "{}, {}, {} pela equipe.",
            count_label(symbol.races, "corrida", "corridas"),
            count_label(symbol.wins, "vitória", "vitórias"),
            count_label(symbol.podiums, "pódio", "pódios")
        ),
        years,
        active,
        nationality,
        races: symbol.races,
        wins: symbol.wins,
        podiums: symbol.podiums,
    })
}

/// Os dois extremos da leitura de pista: (favorita, indigesta).
type TrackAffinityPair = (
    Option<TeamHistoryTrackAffinity>,
    Option<TeamHistoryTrackAffinity>,
);

/// Circuito-fetiche e circuito-carrasco da equipe, pela média da melhor colocação
/// em cada corrida. Só entra pista com corrida repetida e só quando há mais de uma
/// pista qualificada — com um circuito só não existe "melhor", existe o único.
fn real_track_affinity(
    conn: &rusqlite::Connection,
    team_id: &str,
    selected_facts: &[TeamRaceFact],
) -> Result<TrackAffinityPair, String> {
    const MIN_RACES_PER_TRACK: i32 = 2;

    if selected_facts.is_empty() {
        return Ok((None, None));
    }
    let category_ids = fact_category_ids(selected_facts);
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    // Dois níveis: primeiro a melhor colocação da equipe em CADA corrida (ela tem
    // dois carros), depois a média por circuito. Sem o nível de dentro, o carro
    // reserva puxaria a média para baixo em toda pista.
    let sql = format!(
        "SELECT track, COUNT(*) AS races, AVG(best) AS media, MIN(best) AS melhor
         FROM (
            SELECT COALESCE(NULLIF(TRIM(c.track_name), ''), c.pista) AS track,
                   r.race_id,
                   MIN(r.posicao_final) AS best
              FROM race_results r
              JOIN calendar c ON c.id = r.race_id
             WHERE r.equipe_id = ?1
               AND c.categoria IN ({placeholders})
               AND r.posicao_final > 0
             GROUP BY track, r.race_id
         )
         GROUP BY track
         HAVING races >= {MIN_RACES_PER_TRACK}
         ORDER BY media ASC, races DESC, track ASC"
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&team_id];
    for category in &category_ids {
        params.push(category);
    }
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar afinidade de pista: {e}"))?;
    let rows = stmt
        .query_map(params.as_slice(), |row| {
            Ok(TeamHistoryTrackAffinity {
                track: row.get(0)?,
                races: row.get(1)?,
                average_position: row.get(2)?,
                best_position: row.get(3)?,
            })
        })
        .map_err(|e| format!("Falha ao ler afinidade de pista: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Falha ao montar afinidade de pista: {e}"))?;

    if rows.len() < 2 {
        return Ok((None, None));
    }
    let best = rows.first().cloned();
    let worst = rows.last().cloned();
    Ok((best, worst))
}

/// Como a equipe monta o grid: forma gente ou compra pronto.
///
/// A experiência é medida em ANOS DE CARREIRA na chegada (`primeiro ano na equipe
/// − ano de início de carreira`), e não em idade. Idade dependeria do ano corrente
/// e traria um erro de arredondamento a cada temporada; a carreira é uma subtração
/// entre dois fatos já gravados.
fn real_recruitment_dna(
    conn: &rusqlite::Connection,
    team_id: &str,
    selected_facts: &[TeamRaceFact],
) -> Result<Option<TeamHistoryRecruitment>, String> {
    // Abaixo disso não há padrão, há anedota: duas contratações não formam DNA.
    const MIN_DRIVERS: usize = 3;

    if selected_facts.is_empty() {
        return Ok(None);
    }
    let category_ids = fact_category_ids(selected_facts);
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    // A leitura é do GRID inteiro, não só da equipe: numa categoria de entrada
    // todo mundo estreia, e um corte absoluto rotulava quase todas as equipes de
    // "Escola". A régua tem de ser o vizinho.
    let sql = format!(
        "SELECT r.equipe_id, MIN(s.ano) - d.ano_inicio_carreira AS experiencia
           FROM race_results r
           JOIN calendar c ON c.id = r.race_id
           JOIN seasons s ON s.id = c.temporada_id
           JOIN drivers d ON d.id = r.piloto_id
          WHERE c.categoria IN ({placeholders})
          GROUP BY r.equipe_id, r.piloto_id, d.ano_inicio_carreira"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar DNA de recrutamento: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(category_ids.iter()), |row| {
            // Carreira negativa é dado sujo (save antigo com ano de início à
            // frente da temporada); trata como estreante em vez de envenenar a
            // média.
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?.max(0)))
        })
        .map_err(|e| format!("Falha ao ler DNA de recrutamento: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Falha ao montar DNA de recrutamento: {e}"))?;

    let experiences: Vec<i32> = rows
        .iter()
        .filter(|(team, _)| team == team_id)
        .map(|(_, years)| *years)
        .collect();
    let field: Vec<i32> = rows
        .iter()
        .filter(|(team, _)| team != team_id)
        .map(|(_, years)| *years)
        .collect();

    // Sem amostra própria não há DNA, e sem grid não há régua — nos dois casos a
    // aba prefere omitir o bloco a publicar um rótulo que não significa nada.
    if experiences.len() < MIN_DRIVERS || field.len() < MIN_DRIVERS {
        return Ok(None);
    }

    let drivers = experiences.len() as i32;
    let rookies = experiences.iter().filter(|years| **years <= 1).count() as i32;
    let average = experiences.iter().sum::<i32>() as f64 / drivers as f64;
    let rookie_share = rookies as f64 * 100.0 / drivers as f64;
    let field_rookie_share =
        field.iter().filter(|years| **years <= 1).count() as f64 * 100.0 / field.len() as f64;

    let profile_key = format!(
        "team_dossier.recruitment.{}",
        recruitment_profile_key(rookie_share, field_rookie_share)
    );
    Ok(Some(TeamHistoryRecruitment {
        profile: rust_i18n::t!(&profile_key).to_string(),
        drivers,
        rookies,
        average_experience: average,
        rookie_share,
        field_rookie_share,
    }))
}

/// Classificação do DNA: a equipe contra o resto do grid do recorte, em pontos
/// percentuais de estreantes.
///
/// O corte absoluto anterior ("metade do elenco estreou aqui") dizia "Escola"
/// para quase todo mundo, porque na base da pirâmide TODO piloto está estreando —
/// o rótulo media a categoria, não a equipe. Relativo ao grid, "forma gente" volta
/// a significar formar mais que os vizinhos.
fn recruitment_profile_key(rookie_share: f64, field_rookie_share: f64) -> &'static str {
    // Margem em pontos percentuais. Abaixo disso a diferença é ruído de amostra
    // pequena, não escolha de gestão.
    const MARGIN: f64 = 12.0;

    if rookie_share >= field_rookie_share + MARGIN {
        "school"
    } else if rookie_share <= field_rookie_share - MARGIN {
        "market"
    } else {
        "mixed"
    }
}

/// Categorias distintas presentes no recorte, na ordem estável do `BTreeSet`.
fn fact_category_ids(selected_facts: &[TeamRaceFact]) -> Vec<String> {
    selected_facts
        .iter()
        .map(|fact| fact.category.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn count_label(count: i32, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

#[cfg(test)]
mod tests {
    use super::{real_team_profile, recruitment_profile_key};

    #[test]
    fn topo_por_taxa_exige_amostra() {
        // 60% de pódio em 10 corridas é bom, mas não é dinastia — e "Dominante"
        // ao lado de 0 títulos no cabeçalho lia como bug.
        assert_eq!(real_team_profile(10, 2, 6, 0), "winning");
        // A mesma taxa com amostra que a sustenta já vale o topo.
        assert_eq!(real_team_profile(20, 4, 12, 0), "dominant");
    }

    #[test]
    fn titulo_dispensa_amostra() {
        // Título é fato, não taxa: quem ganhou não precisa de N corridas para
        // provar. Uma temporada curta e campeã continua "Dominante".
        assert_eq!(real_team_profile(5, 1, 2, 1), "dominant");
    }

    #[test]
    fn historia_curta_demais_nao_recebe_perfil() {
        assert_eq!(real_team_profile(3, 3, 3, 0), "forming");
    }

    #[test]
    fn dna_de_recrutamento_mede_contra_o_grid() {
        // Formar 70% num grid que forma 40% é escola de verdade.
        assert_eq!(recruitment_profile_key(70.0, 40.0), "school");
        // Os MESMOS 70% num grid que forma 75% não são escola nenhuma: é a
        // categoria de entrada inteira estreando, e era isto que rotulava quase
        // todas as equipes de "Escola".
        assert_eq!(recruitment_profile_key(70.0, 75.0), "mixed");
        // Bem abaixo do grid: a equipe compra pronto.
        assert_eq!(recruitment_profile_key(20.0, 60.0), "market");
        // Diferença dentro da margem é ruído de amostra, não gestão.
        assert_eq!(recruitment_profile_key(48.0, 40.0), "mixed");
    }
}
