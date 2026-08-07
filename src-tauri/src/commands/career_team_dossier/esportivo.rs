//! Recorte esportivo do dossie: marcos, superlativos, resultados temporada a
//! temporada, linha do tempo e sequencias (streaks).

use super::*;

/// Marcos cronológicos (primeira vitória, primeiro pódio, primeiro título).
pub(super) fn build_team_milestones(
    facts: &[TeamRaceFact],
    titles: &[TeamTitleFact],
) -> Vec<TeamHistoryMilestone> {
    let mut milestones = Vec::new();
    if let Some(year) = facts
        .iter()
        .filter(|f| f.podium)
        .map(|f| f.season_year)
        .min()
    {
        milestones.push(TeamHistoryMilestone {
            label: rust_i18n::t!("team_dossier.first_milestone.podium").to_string(),
            year: year.to_string(),
            kind: "first_podium".to_string(),
        });
    }
    if let Some(year) = facts.iter().filter(|f| f.win).map(|f| f.season_year).min() {
        milestones.push(TeamHistoryMilestone {
            label: rust_i18n::t!("team_dossier.first_milestone.win").to_string(),
            year: year.to_string(),
            kind: "first_win".to_string(),
        });
    }
    if let Some(year) = titles.iter().map(|t| t.season_year).min() {
        milestones.push(TeamHistoryMilestone {
            label: rust_i18n::t!("team_dossier.first_milestone.title").to_string(),
            year: year.to_string(),
            kind: "first_title".to_string(),
        });
    }
    milestones
}

/// Quantas corridas a fita de forma recente carrega. Dez cabe numa linha sem
/// virar quadrado de 8px, e cobre pouco mais de uma temporada — o bastante para
/// uma troca de categoria recente aparecer inteira.
const FORM_RACES: usize = 10;

/// As últimas corridas, da mais antiga para a mais nova. `facts` já vem ordenado
/// por temporada e rodada.
pub(super) fn build_team_recent_form(facts: &[TeamRaceFact]) -> Vec<TeamHistoryFormRace> {
    facts
        .iter()
        .rev()
        .take(FORM_RACES)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|fact| TeamHistoryFormRace {
            year: fact.season_year.to_string(),
            round: fact.round,
            category: team_history_category_label(&fact.category),
            category_id: fact.category.clone(),
            position: fact.best_position,
        })
        .collect()
}

/// Distribuição das corridas por faixa de colocação. Corrida sem posição
/// registrada cai em `outside` junto com o resto de fora do top 10: para quem lê
/// a barra, "não pontuou" e "não terminou" são o mesmo fim de semana.
pub(super) fn build_team_result_spread(facts: &[TeamRaceFact]) -> TeamHistoryResultSpread {
    let mut spread = TeamHistoryResultSpread::default();
    for fact in facts {
        spread.races += 1;
        match fact.best_position {
            Some(1) => spread.first += 1,
            Some(2..=3) => spread.podium += 1,
            Some(4..=5) => spread.near_miss += 1,
            Some(6..=10) => spread.top_ten += 1,
            _ => spread.outside += 1,
        }
    }
    spread
}

/// Resultados temporada a temporada (ano, categoria dominante, vitórias, pódios,
/// pontos) — base da aba Esportivo, em ordem cronológica.
pub(super) fn build_team_season_results(
    facts: &[TeamRaceFact],
    positions: &HashMap<i32, i32>,
) -> Vec<TeamHistorySeasonResult> {
    use std::collections::BTreeMap;

    // Acumulador por temporada. Virou struct quando os degraus do pódio entraram:
    // uma tupla de oito campos posicionais é convite a erro na primeira mudança.
    #[derive(Default)]
    struct Acumulado {
        year: i32,
        wins: i32,
        seconds: i32,
        thirds: i32,
        fourths: i32,
        fifths: i32,
        dnfs: i32,
        podiums: i32,
        points: f64,
        races: i32,
        categories: HashMap<String, i32>,
    }

    let mut by_season: BTreeMap<i32, Acumulado> = BTreeMap::new();
    for fact in facts {
        let entry = by_season.entry(fact.season_number).or_default();
        entry.year = fact.season_year;
        if fact.podium {
            entry.podiums += 1;
        }
        // As colocações são EXCLUSIVAS entre si: cada corrida entra uma única
        // vez, pelo melhor carro da equipe. Sem isso, uma dobradinha 1º-3º
        // contaria duas colocações na mesma corrida. O contador de pódios acima
        // segue independente — ele é o total de top 3, usado em outras telas.
        match fact.best_position {
            Some(1) => entry.wins += 1,
            Some(2) => entry.seconds += 1,
            Some(3) => entry.thirds += 1,
            Some(4) => entry.fourths += 1,
            Some(5) => entry.fifths += 1,
            _ => {}
        }
        // Fora do `match` das colocações porque não é uma colocação: o abandono
        // de um carro convive com o pódio do outro na mesma corrida.
        entry.dnfs += fact.dnfs;
        entry.points += fact.points;
        entry.races += 1;
        *entry.categories.entry(fact.category.clone()).or_insert(0) += 1;
    }

    by_season
        .into_iter()
        .map(|(season_number, season)| {
            let category_id = season
                .categories
                .iter()
                .max_by_key(|(_, races)| **races)
                .map(|(cat, _)| cat.clone())
                .unwrap_or_default();
            let category = if category_id.is_empty() {
                String::new()
            } else {
                team_history_category_label(&category_id)
            };
            let position = positions
                .get(&season_number)
                .map(|pos| format!("P{pos}"))
                .unwrap_or_else(|| "—".to_string());
            TeamHistorySeasonResult {
                year: season.year.to_string(),
                category,
                category_id,
                position,
                wins: season.wins,
                podiums: season.podiums,
                points: format!("{}", season.points.round() as i64),
                races: season.races,
                seconds: season.seconds,
                thirds: season.thirds,
                fourths: season.fourths,
                fifths: season.fifths,
                dnfs: season.dnfs,
            }
        })
        .collect()
}

/// A campanha da temporada mais recente da equipe, rodada a rodada, com TODAS as
/// equipes que correram o mesmo campeonato.
///
/// A curva de posição final por temporada responde onde a equipe terminou; esta
/// responde COMO. São perguntas diferentes e a segunda é a que conversa com a
/// forma recente: as mesmas corridas, agora somadas contra os adversários que
/// realmente estavam na pista.
///
/// O recorte é uma temporada e UMA categoria. O dossiê compara dentro de um
/// grupo ("Grupo Mazda"), que pode conter mais de um campeonato — somar pontos de
/// campeonatos distintos daria um total que não existe em lugar nenhum.
pub(super) fn build_team_championship_run(
    all_facts: &[TeamRaceFact],
    team_id: &str,
    names: &HashMap<String, String>,
    current_season: i32,
) -> Option<TeamHistoryChampionshipRun> {
    use std::collections::{BTreeMap, BTreeSet};

    // As temporadas da EQUIPE, da mais recente para a mais antiga — e não a
    // última do save: numa equipe dissolvida há três anos, a temporada corrente
    // não tem linha nenhuma dela e o gráfico ficaria sem o único fio que importa.
    //
    // Anda para trás porque uma rodada só não é campanha: no comeco de temporada
    // a mais recente tem uma corrida ou nenhuma, e parar ali fazia o bloco
    // inteiro sumir — junto com o seletor entre as duas vistas, que so aparece
    // quando as duas tem dado. Recuar mostra a ultima disputa que existiu, e ela
    // se identifica sozinha: o cabecalho do grafico traz o ano e a categoria, e
    // `live` fica falso.
    let temporadas: Vec<i32> = all_facts
        .iter()
        .filter(|fact| fact.team_id == team_id)
        .map(|fact| fact.season_number)
        .collect::<BTreeSet<i32>>()
        .into_iter()
        .rev()
        .collect();

    let mut escolhido: Option<(i32, String, Vec<&TeamRaceFact>, Vec<i32>)> = None;
    for season in temporadas {
        // Dentro da temporada, a categoria em que a equipe mais correu. Uma
        // equipe promovida no meio do ano aparece nas duas; a que vale é onde ela
        // disputou o campeonato de fato.
        let mut por_categoria: HashMap<&str, i32> = HashMap::new();
        for fact in all_facts
            .iter()
            .filter(|fact| fact.season_number == season && fact.team_id == team_id)
        {
            *por_categoria.entry(fact.category.as_str()).or_insert(0) += 1;
        }
        // Desempate por nome da categoria para o resultado não depender da ordem
        // do HashMap — dois campeonatos com o mesmo número de corridas existem.
        let Some(category_id) = por_categoria
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
            .map(|(cat, _)| cat.to_string())
        else {
            continue;
        };

        let recorte: Vec<&TeamRaceFact> = all_facts
            .iter()
            .filter(|fact| fact.season_number == season && fact.category == category_id)
            .collect();
        if recorte.is_empty() {
            continue;
        }

        let rounds: Vec<i32> = recorte
            .iter()
            .map(|fact| fact.round)
            .collect::<BTreeSet<i32>>()
            .into_iter()
            .collect();
        if rounds.len() < 2 {
            continue;
        }

        escolhido = Some((season, category_id, recorte, rounds));
        break;
    }

    let (season, category_id, recorte, rounds) = escolhido?;

    // Pontos por equipe e rodada. BTreeMap para a ordem das equipes ser estável
    // entre execuções — o desenho de vinte linhas cinzas não pode piscar de
    // ordem a cada abertura.
    let mut por_equipe: BTreeMap<&str, HashMap<i32, f64>> = BTreeMap::new();
    for fact in &recorte {
        *por_equipe
            .entry(fact.team_id.as_str())
            .or_default()
            .entry(fact.round)
            .or_insert(0.0) += fact.points;
    }

    let year = recorte
        .first()
        .map(|fact| fact.season_year)
        .unwrap_or_default();

    let mut lines: Vec<TeamHistoryChampionshipLine> = por_equipe
        .into_iter()
        .map(|(id, por_rodada)| {
            // O acumulado carrega adiante em rodada sem pontuação: a linha da
            // equipe que abandonou anda reta, não cai. Cair significaria perder
            // pontos, o que não acontece em campeonato nenhum.
            let mut acumulado = 0.0;
            let points: Vec<f64> = rounds
                .iter()
                .map(|round| {
                    acumulado += por_rodada.get(round).copied().unwrap_or(0.0);
                    (acumulado * 100.0).round() / 100.0
                })
                .collect();
            TeamHistoryChampionshipLine {
                team_id: id.to_string(),
                team: names.get(id).cloned().unwrap_or_default(),
                selected: id == team_id,
                position: 0,
                total: format!("{}", acumulado.round() as i64),
                points,
            }
        })
        .collect();

    // Ordena pela pontuação final e numera. O empate cai para o id, que é
    // arbitrário mas estável — a alternativa (deixar a ordem do mapa decidir)
    // muda a colocação exibida entre aberturas da mesma tela.
    lines.sort_by(|a, b| {
        let fim = |line: &TeamHistoryChampionshipLine| line.points.last().copied().unwrap_or(0.0);
        fim(b)
            .partial_cmp(&fim(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.team_id.cmp(&b.team_id))
    });
    for (index, line) in lines.iter_mut().enumerate() {
        line.position = index as i32 + 1;
    }

    Some(TeamHistoryChampionshipRun {
        year: year.to_string(),
        category: team_history_category_label(&category_id),
        category_id,
        rounds,
        lines,
        live: season == current_season && current_season > 0,
    })
}

/// Superlativos da equipe a partir do histórico real: melhor temporada (vitórias),
/// pico de pódios numa temporada e maior sequência de títulos consecutivos.
pub(super) fn build_team_highlights(
    facts: &[TeamRaceFact],
    titles: &[TeamTitleFact],
    positions: &HashMap<i32, i32>,
) -> Vec<TeamHistoryHighlight> {
    use std::collections::BTreeMap;

    // Agrega por temporada: (ano, vitórias, pódios, categoria→corridas).
    let mut by_season: BTreeMap<i32, (i32, i32, i32, HashMap<String, i32>)> = BTreeMap::new();
    for fact in facts {
        let entry = by_season
            .entry(fact.season_number)
            .or_insert_with(|| (fact.season_year, 0, 0, HashMap::new()));
        entry.0 = fact.season_year;
        if fact.win {
            entry.1 += 1;
        }
        if fact.podium {
            entry.2 += 1;
        }
        *entry.3.entry(fact.category.clone()).or_insert(0) += 1;
    }

    let dominant_category = |cats: &HashMap<String, i32>| -> String {
        cats.iter()
            .max_by_key(|(_, races)| **races)
            .map(|(cat, _)| team_history_category_label(cat))
            .unwrap_or_default()
    };

    // Os candidatos entram em ordem de IMPORTÂNCIA, não de conveniência de
    // cálculo: o corte lá embaixo é por cima da lista, e quem fica de fora é o
    // superlativo mais fraco.
    let mut highlights = Vec::new();

    // Maior sequência de títulos consecutivos. Abre a fileira: é o único aqui que
    // fala de domínio sustentado, e não de um ano bom isolado.
    let mut years: Vec<i32> = titles.iter().map(|title| title.season_year).collect();
    years.sort_unstable();
    years.dedup();
    let mut best_run = 0;
    let mut best_run_end = 0;
    let mut run = 0;
    let mut prev: Option<i32> = None;
    for year in &years {
        run = if prev == Some(year - 1) { run + 1 } else { 1 };
        if run > best_run {
            best_run = run;
            best_run_end = *year;
        }
        prev = Some(*year);
    }
    if best_run >= 2 {
        highlights.push(TeamHistoryHighlight {
            label: rust_i18n::t!("team_dossier.highlight.biggest_dynasty").to_string(),
            value: rust_i18n::t!(
                "team_dossier.highlight.biggest_dynasty_value",
                count = best_run
            )
            .to_string(),
            // O intervalo INTEIRO, não só o fim. "Até 2019" obrigava a subtrair de
            // cabeça a sequência do card ao lado para saber quando começou.
            detail: rust_i18n::t!(
                "team_dossier.highlight.detail_span",
                first = best_run_end - best_run + 1,
                last = best_run_end
            )
            .to_string(),
        });
    }

    // Melhor temporada por vitórias.
    if let Some((_, (year, wins, _, cats))) = by_season.iter().max_by_key(|(_, v)| v.1) {
        if *wins > 0 {
            highlights.push(TeamHistoryHighlight {
                label: rust_i18n::t!("team_dossier.highlight.best_season").to_string(),
                value: rust_i18n::t!("team_dossier.highlight.best_season_value", count = wins)
                    .to_string(),
                detail: rust_i18n::t!(
                    "team_dossier.highlight.detail_year_category",
                    year = year,
                    category = dominant_category(cats)
                )
                .to_string(),
            });
        }
    }

    // Pico de pódios numa temporada.
    if let Some((_, (year, _, podiums, cats))) = by_season.iter().max_by_key(|(_, v)| v.2) {
        if *podiums > 0 {
            highlights.push(TeamHistoryHighlight {
                label: rust_i18n::t!("team_dossier.highlight.most_podiums").to_string(),
                value: rust_i18n::t!("team_dossier.highlight.most_podiums_value", count = podiums)
                    .to_string(),
                detail: rust_i18n::t!(
                    "team_dossier.highlight.detail_year_category",
                    year = year,
                    category = dominant_category(cats)
                )
                .to_string(),
            });
        }
    }

    // Melhor campanha (menor posição final no campeonato). Fecha a fila: numa
    // equipe que já foi campeã nove vezes, "Campeão, em 2008" repete um número
    // que o cabeçalho do dossiê já mostra. Só sobrevive ao corte em quem tem
    // pouco mais a contar — e aí é o teto dela, que não aparece em outro lugar.
    if let Some((season, position)) = positions.iter().min_by_key(|(_, pos)| **pos) {
        let year = by_season.get(season).map(|entry| entry.0).unwrap_or(0);
        let value = if *position == 1 {
            rust_i18n::t!("team_dossier.highlight.champion_value").to_string()
        } else {
            format!("P{position}")
        };
        highlights.push(TeamHistoryHighlight {
            label: rust_i18n::t!("team_dossier.highlight.best_campaign").to_string(),
            value,
            detail: rust_i18n::t!("team_dossier.highlight.detail_year", year = year).to_string(),
        });
    }

    // A fileira é de TRÊS colunas: quatro cards deixam um órfão sozinho na linha
    // de baixo, e cinco deixam um buraco. Só 3 ou 6 fecham a grade — e como o
    // caldo aqui nunca chega a seis, na prática o corte é em três.
    highlights.truncate(if highlights.len() >= 6 { 6 } else { 3 });
    highlights
}

pub(super) fn build_real_team_timeline(facts: &[TeamRaceFact]) -> Vec<TeamHistoryTimelineItem> {
    let Some(first) = facts.first() else {
        return vec![TeamHistoryTimelineItem {
            year: "-".to_string(),
            text: "Sem corridas registradas neste recorte.".to_string(),
            kind: "empty".to_string(),
        }];
    };
    let mut items = vec![TeamHistoryTimelineItem {
        year: first.season_year.to_string(),
        text: format!(
            "Primeira corrida registrada em {}, rodada {}.",
            team_history_category_label(&first.category),
            first.round
        ),
        kind: "first_race".to_string(),
    }];

    if let Some(first_win) = facts.iter().find(|fact| fact.win) {
        items.push(TeamHistoryTimelineItem {
            year: first_win.season_year.to_string(),
            text: format!(
                "Primeira vitória real em {}, rodada {}.",
                team_history_category_label(&first_win.category),
                first_win.round
            ),
            kind: "first_win".to_string(),
        });
    }

    // A "melhor temporada registrada: N pts" saiu daqui. O card de destaque em
    // Records já se chama "Melhor temporada" e mede outra coisa (vitórias), então
    // os dois apareciam com o mesmo nome, anos possivelmente diferentes e números
    // que não se conversam — leitura de contradição, não de informação a mais.

    if let Some(latest) = facts.last() {
        items.push(TeamHistoryTimelineItem {
            year: latest.season_year.to_string(),
            text: format!(
                "Último registro em {}, rodada {}.",
                team_history_category_label(&latest.category),
                latest.round
            ),
            kind: "last_record".to_string(),
        });
    }

    items
}

pub(super) fn season_count_label(total: i32) -> String {
    match total {
        0 => rust_i18n::t!("team_dossier.season_count.none").to_string(),
        1 => rust_i18n::t!("team_dossier.season_count.one").to_string(),
        value => rust_i18n::t!("team_dossier.season_count.other", count = value).to_string(),
    }
}

/// Sequência atual por NÍVEL (rookie, amador, pro, ...) — quantas temporadas
/// consecutivas a equipe está no nível atual. Diferente do "grupo" (que a equipe
/// nunca troca), o nível muda com promoções/rebaixamentos, então o streak importa.
pub(super) fn current_level_streak_label(facts: &[TeamRaceFact]) -> String {
    if facts.is_empty() {
        return rust_i18n::t!("team_dossier.streak.none").to_string();
    }

    // season → categoria dominante → nível.
    let mut by_season: BTreeMap<i32, HashMap<String, i32>> = BTreeMap::new();
    for fact in facts {
        *by_season
            .entry(fact.season_number)
            .or_default()
            .entry(fact.category.clone())
            .or_insert(0) += 1;
    }
    let mut season_levels: Vec<(i32, String)> = by_season
        .into_iter()
        .map(|(season, cats)| {
            let category = cats
                .iter()
                .max_by_key(|(_, races)| **races)
                .map(|(cat, _)| cat.clone())
                .unwrap_or_default();
            let level = categories::get_category(&category)
                .map(|config| crate::constants::category_tier_label(config.nivel))
                .unwrap_or_else(|| "—".to_string());
            (season, level)
        })
        .collect();
    season_levels.sort_by_key(|(season, _)| *season);

    let current_level = match season_levels.last() {
        Some((_, level)) => level.clone(),
        None => return rust_i18n::t!("team_dossier.streak.none").to_string(),
    };

    // Conta temporadas consecutivas (e contíguas) no nível atual, do fim para trás.
    let mut streak = 0;
    let mut prev_season: Option<i32> = None;
    for (season, level) in season_levels.iter().rev() {
        if *level != current_level {
            break;
        }
        if let Some(prev) = prev_season {
            if prev - season != 1 {
                break;
            }
        }
        streak += 1;
        prev_season = Some(*season);
    }

    if streak <= 1 {
        rust_i18n::t!(
            "team_dossier.streak.level_one",
            level = current_level.as_str()
        )
        .to_string()
    } else {
        rust_i18n::t!(
            "team_dossier.streak.level_other",
            count = streak,
            level = current_level.as_str()
        )
        .to_string()
    }
}

pub(super) fn best_real_streak_label(facts: &[TeamRaceFact]) -> String {
    if facts.is_empty() {
        return rust_i18n::t!("team_dossier.streak.none").to_string();
    }
    let mut best_podium = 0;
    let mut current_podium = 0;
    let mut best_points = 0;
    let mut current_points = 0;
    for fact in facts {
        if fact.podium {
            current_podium += 1;
            best_podium = best_podium.max(current_podium);
        } else {
            current_podium = 0;
        }
        if fact.points > 0.0 {
            current_points += 1;
            best_points = best_points.max(current_points);
        } else {
            current_points = 0;
        }
    }
    if best_podium > 0 {
        if best_podium == 1 {
            rust_i18n::t!("team_dossier.streak.podium_one").to_string()
        } else {
            rust_i18n::t!("team_dossier.streak.podium_other", count = best_podium).to_string()
        }
    } else if best_points > 0 {
        if best_points == 1 {
            rust_i18n::t!("team_dossier.streak.points_one").to_string()
        } else {
            rust_i18n::t!("team_dossier.streak.points_other", count = best_points).to_string()
        }
    } else {
        rust_i18n::t!("team_dossier.streak.none").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fato(season: i32, year: i32, win: bool, podium: bool) -> TeamRaceFact {
        TeamRaceFact {
            team_id: "t1".to_string(),
            season_number: season,
            season_year: year,
            category: "gt3".to_string(),
            round: 1,
            points: 10.0,
            win,
            podium,
            best_position: Some(if win { 1 } else { 3 }),
            week_of_year: 1,
            dnfs: 0,
            class: String::new(),
        }
    }

    fn titulo(season_year: i32) -> TeamTitleFact {
        TeamTitleFact {
            season_id: format!("s{season_year}"),
            season_year,
            category: "gt3".to_string(),
            points: 300.0,
            wins: 4,
            class: String::new(),
        }
    }

    #[test]
    fn a_fileira_de_destaques_nunca_fecha_em_quatro() {
        // Equipe com tudo: dinastia, temporada boa, pico de pódios E uma campanha
        // de campeã. São quatro candidatos, e quatro deixam um órfão sozinho na
        // linha de baixo de uma grade de três colunas.
        let facts = vec![
            fato(1, 2020, true, true),
            fato(2, 2021, true, true),
            fato(3, 2022, true, true),
        ];
        let titles = vec![titulo(2020), titulo(2021), titulo(2022)];
        let positions: HashMap<i32, i32> = [(1, 1), (2, 1), (3, 1)].into_iter().collect();

        let highlights = build_team_highlights(&facts, &titles, &positions);
        assert_eq!(highlights.len(), 3);
        // O que cai é a campanha: numa equipe que já foi campeã, ela repete um
        // número que o cabeçalho do dossiê mostra.
        assert_eq!(
            highlights[0].label,
            rust_i18n::t!("team_dossier.highlight.biggest_dynasty").to_string(),
            "a dinastia abre a fileira"
        );
        assert!(!highlights
            .iter()
            .any(|item| item.label
                == rust_i18n::t!("team_dossier.highlight.best_campaign").to_string()));
    }

    #[test]
    fn a_dinastia_mostra_o_intervalo_inteiro() {
        let facts = vec![fato(1, 2017, true, true)];
        let titles = vec![titulo(2017), titulo(2018), titulo(2019)];
        let highlights = build_team_highlights(&facts, &titles, &HashMap::new());
        let dinastia = highlights
            .iter()
            .find(|item| {
                item.label == rust_i18n::t!("team_dossier.highlight.biggest_dynasty").to_string()
            })
            .expect("a dinastia entra na fileira");
        // "Até 2019" obrigava a subtrair a sequência de cabeça para saber o começo.
        assert!(
            dinastia.detail.contains("2017") && dinastia.detail.contains("2019"),
            "o detalhe traz o intervalo: {}",
            dinastia.detail
        );
    }

    #[test]
    fn quem_nunca_foi_campea_mantem_a_melhor_campanha() {
        let facts = vec![fato(1, 2020, true, true), fato(2, 2021, false, true)];
        let positions: HashMap<i32, i32> = [(1, 4), (2, 2)].into_iter().collect();
        let highlights = build_team_highlights(&facts, &[], &positions);
        assert_eq!(highlights.len(), 3);
        assert!(
            highlights.iter().any(|item| item.value == "P2"),
            "o teto dela não aparece em outro lugar"
        );
    }
    fn fato_rodada(season: i32, year: i32, round: i32, team: &str) -> TeamRaceFact {
        TeamRaceFact {
            team_id: team.to_string(),
            season_number: season,
            season_year: year,
            category: "gt3".to_string(),
            round,
            points: 10.0,
            win: false,
            podium: false,
            best_position: Some(4),
            week_of_year: round,
            dnfs: 0,
            class: String::new(),
        }
    }

    #[test]
    fn campanha_recua_para_a_ultima_temporada_que_teve_disputa() {
        // A temporada corrente mal comecou: uma rodada. Parar nela fazia a
        // campanha sumir — e, com ela, o seletor entre as duas vistas de "como a
        // equipe evolui", que so aparece quando as duas tem dado.
        let mut facts = vec![
            fato_rodada(1, 2025, 1, "t1"),
            fato_rodada(1, 2025, 2, "t1"),
            fato_rodada(1, 2025, 3, "t1"),
            fato_rodada(1, 2025, 1, "t2"),
            fato_rodada(1, 2025, 2, "t2"),
            fato_rodada(1, 2025, 3, "t2"),
        ];
        facts.push(fato_rodada(2, 2026, 1, "t1"));
        facts.push(fato_rodada(2, 2026, 1, "t2"));

        let mut names = HashMap::new();
        names.insert("t1".to_string(), "Equipe Um".to_string());
        names.insert("t2".to_string(), "Equipe Dois".to_string());

        let run = build_team_championship_run(&facts, "t1", &names, 2)
            .expect("a campanha de 2025 continua desenhavel");
        assert_eq!(run.year, "2025");
        assert_eq!(run.rounds.len(), 3);
        // Recuada, ela nao e o campeonato em andamento — e o grafico diz isso.
        assert!(!run.live);
    }

    #[test]
    fn sem_nenhuma_temporada_com_duas_rodadas_nao_ha_campanha() {
        let facts = vec![fato_rodada(2, 2026, 1, "t1"), fato_rodada(2, 2026, 1, "t2")];
        let names = HashMap::new();
        assert!(build_team_championship_run(&facts, "t1", &names, 2).is_none());
    }
}
