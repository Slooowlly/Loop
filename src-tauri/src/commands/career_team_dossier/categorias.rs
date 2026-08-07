//! Escada de categorias da equipe: as passagens por categoria, o caminho
//! percorrido (promoção/rebaixamento) e o resumo de movimento.

use super::*;

/// Uma PASSAGEM contínua por uma categoria — não a categoria inteira.
///
/// A distinção importa em quem sobe e volta. Agrupar por categoria (um `MIN`/`MAX`
/// de temporada por id) colapsava gt4 → gt3 → gt4 num único "GT4 2020-2022" com o
/// GT3 pendurado no meio: a escada mostrava uma promoção, nenhum rebaixamento, e a
/// janela do GT4 mentia ao sugerir três anos seguidos lá. Aqui cada ida é uma
/// entrada própria, e a viagem de volta aparece.
pub(super) struct CategorySpan {
    pub(super) category: String,
    pub(super) start_season: i32,
    pub(super) start_year: i32,
    pub(super) end_year: i32,
    /// Temporadas efetivamente corridas nesta passagem.
    pub(super) seasons: i32,
    pub(super) races: i32,
    pub(super) wins: i32,
    pub(super) podiums: i32,
    /// Fim da passagem, em número de temporada. Interno: delimita a janela que
    /// recebe os fatos no segundo passo.
    end_season: i32,
}

/// Recorta os fatos em passagens contínuas, em ordem cronológica. Base
/// compartilhada da escada (`category_path`), do movimento e da pirâmide.
pub(super) fn category_spans(facts: &[TeamRaceFact]) -> Vec<CategorySpan> {
    // Uma temporada, uma categoria: a da MAIORIA das corridas daquele ano. A
    // equipe corre um campeonato por temporada, mas o bloco especial a leva como
    // convidada a outro — sem essa redução, a convocação viraria uma passagem e a
    // escada registraria uma subida ao tier 6 e a queda de volta no ano seguinte.
    let mut per_season: BTreeMap<i32, BTreeMap<String, i32>> = BTreeMap::new();
    let mut year_of_season: BTreeMap<i32, i32> = BTreeMap::new();
    for fact in facts {
        *per_season
            .entry(fact.season_number)
            .or_default()
            .entry(fact.category.clone())
            .or_insert(0) += 1;
        year_of_season.insert(fact.season_number, fact.season_year);
    }

    let mut spans: Vec<CategorySpan> = Vec::new();
    for (season, counts) in &per_season {
        // Empate fica com a primeira em ordem alfabética (o `BTreeMap` já vem
        // ordenado, e a comparação é estrita) — qualquer critério serve, desde que
        // o mesmo save produza sempre a mesma escada.
        let mut chosen: Option<(&String, i32)> = None;
        for (category, races) in counts {
            if chosen.map(|(_, best)| *races > best).unwrap_or(true) {
                chosen = Some((category, *races));
            }
        }
        let Some((category, _)) = chosen else {
            continue;
        };
        let year = year_of_season.get(season).copied().unwrap_or(0);

        // Buraco de temporada com a MESMA categoria dos dois lados continua uma
        // passagem só: daqui não dá para ver se a equipe sumiu do mundo ou foi
        // correr numa escada fora do recorte. Quem responde isso é a faixa ano a
        // ano, que recebe as temporadas fora de escopo à parte.
        match spans.last_mut() {
            Some(last) if last.category == *category => {
                last.end_season = *season;
                last.end_year = year;
                last.seasons += 1;
            }
            _ => spans.push(CategorySpan {
                category: category.clone(),
                start_season: *season,
                start_year: year,
                end_season: *season,
                end_year: year,
                seasons: 1,
                races: 0,
                wins: 0,
                podiums: 0,
            }),
        }
    }

    // Segundo passo: o saldo de cada passagem. Fato de categoria minoritária num
    // ano misto não cai em passagem nenhuma — é a mesma corrida que já foi
    // descartada na redução acima, e contá-la aqui inflaria a passagem vizinha.
    for fact in facts {
        let Some(span) = spans.iter_mut().find(|span| {
            span.category == fact.category
                && span.start_season <= fact.season_number
                && fact.season_number <= span.end_season
        }) else {
            continue;
        };
        span.races += 1;
        if fact.win {
            span.wins += 1;
        }
        if fact.podium {
            span.podiums += 1;
        }
    }

    spans
}

fn span_years(span: &CategorySpan) -> String {
    if span.start_year == span.end_year {
        span.start_year.to_string()
    } else {
        format!("{}-{}", span.start_year, span.end_year)
    }
}

fn tier_of(category: &str) -> Option<u8> {
    categories::get_category(category).map(|config| config.tier)
}

pub(super) fn build_real_category_path(facts: &[TeamRaceFact]) -> Vec<TeamHistoryCategoryStep> {
    let spans = category_spans(facts);
    let mut steps = Vec::new();
    let mut prev_tier: Option<u8> = None;
    for (index, span) in spans.iter().enumerate() {
        let tier = tier_of(&span.category);
        let movement = match (prev_tier, tier) {
            (None, _) => "start",
            (Some(prev), Some(current)) if current > prev => "promotion",
            (Some(prev), Some(current)) if current < prev => "relegation",
            _ => "same",
        };
        if tier.is_some() {
            prev_tier = tier;
        }
        let detail = rust_i18n::t!(match movement {
            "promotion" => "team_dossier.ladder.promotion",
            "relegation" => "team_dossier.ladder.relegation",
            "start" => "team_dossier.ladder.start",
            _ => "team_dossier.ladder.same",
        })
        .to_string();
        steps.push(TeamHistoryCategoryStep {
            category: team_history_category_label(&span.category),
            category_id: span.category.clone(),
            years: span_years(span),
            start_year: span.start_year,
            end_year: span.end_year,
            detail,
            color: history_palette(index),
            movement: movement.to_string(),
            tier: tier.unwrap_or(0) as i32,
        });
    }
    steps
}

/// Resumo real de movimento entre categorias para a aba Categorias.
pub(super) fn build_team_movement(
    facts: &[TeamRaceFact],
    category_ids: &[String],
) -> TeamHistoryMovement {
    let spans = category_spans(facts);

    // Promoções / rebaixamentos pelas transições entre passagens CONSECUTIVAS.
    let mut promotions = 0;
    let mut relegations = 0;
    let mut prev_tier: Option<u8> = None;
    for span in &spans {
        if let Some(tier) = tier_of(&span.category) {
            if let Some(prev) = prev_tier {
                if tier > prev {
                    promotions += 1;
                } else if tier < prev {
                    relegations += 1;
                }
            }
            prev_tier = Some(tier);
        }
    }

    // Tempo e saldo por categoria: somando as passagens, na ordem em que a equipe
    // as estreou. Duas idas ao GT4 são o tempo dela no GT4, não dois degraus.
    let mut ordem: Vec<String> = Vec::new();
    let mut por_categoria: BTreeMap<String, (i32, i32, i32, i32)> = BTreeMap::new();
    for span in &spans {
        if !ordem.iter().any(|id| id == &span.category) {
            ordem.push(span.category.clone());
        }
        let entrada = por_categoria
            .entry(span.category.clone())
            .or_insert((0, 0, 0, 0));
        entrada.0 += span.seasons;
        entrada.1 += span.races;
        entrada.2 += span.wins;
        entrada.3 += span.podiums;
    }

    let mut time_lines: Vec<TeamHistoryCategoryTime> = ordem
        .iter()
        .map(|id| {
            let (seasons, races, wins, podiums) =
                por_categoria.get(id).copied().unwrap_or((0, 0, 0, 0));
            TeamHistoryCategoryTime {
                category: team_history_category_label(id),
                category_id: id.clone(),
                tier: tier_of(id).unwrap_or(0) as i32,
                seasons,
                races,
                wins,
                podiums,
            }
        })
        .collect();
    // Do topo para a base, na mesma direção da pirâmide. Ordenar por estreia
    // punha a categoria mais alta no rodapé da lista, e os dois blocos — que
    // desenham a MESMA escada, um em cima do outro — discordavam sobre onde fica
    // o alto. Empate de degrau mantém a ordem de estreia (a ordenação é estável).
    time_lines.sort_by_key(|linha| std::cmp::Reverse(linha.tier));

    // A string continua para o v1, que desenha uma linha só.
    let time_by_category = time_lines
        .iter()
        .map(|linha| {
            format!(
                "{}: {} {}",
                linha.category,
                linha.seasons,
                if linha.seasons == 1 { "ano" } else { "anos" }
            )
        })
        .collect::<Vec<_>>()
        .join(" · ");

    // Teto e degrau de casa no lugar de "melhor / mais difícil categoria".
    //
    // As duas antigas saíam da taxa de vitória com mínimo de três corridas, e a
    // conta era frágil dos dois lados: numa equipe de uma categoria só as duas
    // respondiam a MESMA coisa (parecia defeito), e num histórico longo três
    // corridas ruins de um ano derrubavam uma categoria à frente de sessenta
    // corridas noutra. Teto e casa são fatos — não índices — e não empatam.
    let peak_category = spans
        .iter()
        .filter_map(|span| tier_of(&span.category).map(|tier| (tier, span.category.clone())))
        .max_by_key(|(tier, _)| *tier)
        .map(|(_, id)| team_history_category_label(&id))
        .unwrap_or_else(|| "—".to_string());
    let home_category = time_lines
        .iter()
        .max_by_key(|linha| (linha.seasons, linha.races))
        .map(|linha| linha.category.clone())
        .unwrap_or_else(|| "—".to_string());

    TeamHistoryMovement {
        promotions,
        relegations,
        time_by_category,
        peak_category,
        home_category,
        time_lines,
        ladder: build_team_ladder(&spans, &ladder_categories(facts, category_ids)),
    }
}

/// A marca desta equipe, lida dos fatos DELA.
///
/// A primeira categoria monomarca que ela correu manda; num histórico que começa
/// direto na Production, a classe do carro responde o mesmo.
fn team_ladder_family(facts: &[TeamRaceFact]) -> Option<&'static str> {
    for fact in facts {
        if let Some(family) = team_history_group_family(&fact.category) {
            return Some(family);
        }
        match fact.class.as_str() {
            "mazda" => return Some("mazda"),
            "toyota" => return Some("toyota"),
            "bmw" => return Some("bmw"),
            _ => {}
        }
    }
    None
}

/// A escada que ESTA equipe pode subir — não o grupo do ranking.
///
/// Os dois recortes divergem na Production: o grupo dela é toda a base da
/// pirâmide (as duas marcas de entrada mais a BMW), porque é contra todas essas
/// equipes que o rank da Production se mede. Como escada isso mente — uma equipe
/// Mazda nunca correu na Toyota Cup nem na BMW M2, e a pirâmide oferecia esses
/// degraus como se faltasse subir neles.
fn ladder_categories(facts: &[TeamRaceFact], category_ids: &[String]) -> Vec<String> {
    let raiz = match team_ladder_family(facts) {
        Some("mazda") => "mazda_rookie",
        Some("toyota") => "toyota_rookie",
        Some("bmw") => "bmw_m2",
        // Fora das escadas de marca (GT4, GT3, LMP2, Endurance) o grupo já É a
        // escada: uma categoria só, sem irmã de outra marca para confundir.
        _ => return category_ids.to_vec(),
    };
    let mut escada = team_history_group_categories(raiz);
    // Rede de segurança: um degrau que a equipe pisou nunca some da pirâmide,
    // mesmo que a marca dela diga que ele não faz parte do caminho.
    for fact in facts {
        if !escada.iter().any(|id| id == &fact.category) {
            escada.push(fact.category.clone());
        }
    }
    escada
}

/// A pirâmide do recorte: TODOS os degraus da escada do grupo, pisados ou não.
///
/// Mostrar só os degraus pisados escondia o que a aba existe para dizer. Uma
/// equipe de estreia virava um card solitário, e o dado mais alto dela — que há
/// dois degraus acima e ela está no primeiro — não aparecia em lugar nenhum.
fn build_team_ladder(
    spans: &[CategorySpan],
    category_ids: &[String],
) -> Vec<TeamHistoryLadderRung> {
    let mut vistos: Vec<String> = Vec::new();
    let mut degraus: Vec<(u8, String)> = Vec::new();
    for id in category_ids {
        if vistos.iter().any(|visto| visto == id) {
            continue;
        }
        vistos.push(id.clone());
        degraus.push((tier_of(id).unwrap_or(0), id.clone()));
    }
    degraus.sort_by_key(|(tier, _)| *tier);

    let peak_tier = spans
        .iter()
        .filter_map(|span| tier_of(&span.category))
        .max();
    let current = spans.last().map(|span| span.category.clone());

    degraus
        .into_iter()
        .map(|(tier, id)| {
            let passagens: Vec<&CategorySpan> =
                spans.iter().filter(|span| span.category == id).collect();
            let seasons: i32 = passagens.iter().map(|span| span.seasons).sum();
            let years = match (
                passagens.iter().map(|span| span.start_year).min(),
                passagens.iter().map(|span| span.end_year).max(),
            ) {
                (Some(first), Some(last)) if first == last => first.to_string(),
                (Some(first), Some(last)) => format!("{first}-{last}"),
                _ => String::new(),
            };
            TeamHistoryLadderRung {
                category: team_history_category_label(&id),
                tier: tier as i32,
                visited: !passagens.is_empty(),
                is_peak: !passagens.is_empty() && peak_tier == Some(tier),
                is_current: current.as_deref() == Some(id.as_str()),
                seasons,
                years,
                category_id: id,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fato(season: i32, year: i32, category: &str, round: i32) -> TeamRaceFact {
        TeamRaceFact {
            team_id: "t1".to_string(),
            season_number: season,
            season_year: year,
            category: category.to_string(),
            round,
            points: 10.0,
            win: false,
            podium: false,
            best_position: Some(5),
            week_of_year: round,
            dnfs: 0,
            class: String::new(),
        }
    }

    #[test]
    fn volta_para_a_mesma_categoria_vira_duas_passagens() {
        let facts = vec![
            fato(1, 2030, "gt4", 1),
            fato(2, 2031, "gt3", 1),
            fato(3, 2032, "gt4", 1),
        ];
        let spans = category_spans(&facts);
        assert_eq!(
            spans.len(),
            3,
            "a ida e a volta ao GT4 são passagens distintas"
        );
        assert_eq!(spans[0].category, "gt4");
        assert_eq!(
            spans[0].end_year, 2030,
            "a primeira passagem não engole a segunda"
        );
        assert_eq!(spans[1].category, "gt3");
        assert_eq!(spans[2].category, "gt4");

        let movement = build_team_movement(&facts, &["gt4".to_string(), "gt3".to_string()]);
        assert_eq!(movement.promotions, 1);
        assert_eq!(
            movement.relegations, 1,
            "a queda de volta ao GT4 era invisível"
        );
    }

    #[test]
    fn convocacao_nao_vira_degrau_da_escada() {
        // Temporada de GT3 com uma única corrida do bloco especial no meio.
        let facts = vec![
            fato(1, 2030, "gt3", 1),
            fato(1, 2030, "gt3", 2),
            fato(1, 2030, "endurance", 3),
        ];
        let spans = category_spans(&facts);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].category, "gt3");
        assert_eq!(
            spans[0].races, 2,
            "a corrida de convidada não entra no saldo do degrau"
        );
    }

    #[test]
    fn o_saldo_da_categoria_soma_as_duas_idas() {
        // A equipe volta ao GT4: a linha por categoria é UMA, com o saldo somado.
        // A escada mostra a viagem; esta responde "o que ela fez em cada degrau".
        let mut facts = vec![
            fato(1, 2030, "gt4", 1),
            fato(2, 2031, "gt3", 1),
            fato(3, 2032, "gt4", 1),
        ];
        facts[0].win = true;
        facts[0].podium = true;
        facts[2].podium = true;

        let movement = build_team_movement(&facts, &["gt4".to_string(), "gt3".to_string()]);
        let gt4 = movement
            .time_lines
            .iter()
            .find(|linha| linha.category_id == "gt4")
            .expect("o GT4 tem linha própria");
        assert_eq!(gt4.seasons, 2);
        assert_eq!(gt4.races, 2);
        assert_eq!(gt4.wins, 1);
        assert_eq!(gt4.podiums, 2);
        assert_eq!(
            movement.time_lines.len(),
            2,
            "as duas idas não viram duas linhas"
        );
        // Do topo para a base, como a pirâmide logo acima: o GT3 vem primeiro
        // mesmo tendo sido estreado depois.
        assert_eq!(movement.time_lines[0].category_id, "gt3");
        assert_eq!(movement.time_lines[1].category_id, "gt4");
    }

    #[test]
    fn a_piramide_traz_os_degraus_nao_pisados() {
        let facts = vec![fato(1, 2030, "mazda_rookie", 1)];
        let grupo = vec![
            "mazda_rookie".to_string(),
            "mazda_amador".to_string(),
            "production_challenger".to_string(),
        ];
        let movement = build_team_movement(&facts, &grupo);
        assert_eq!(movement.ladder.len(), 3);
        assert_eq!(movement.ladder[0].category_id, "mazda_rookie");
        assert!(movement.ladder[0].visited);
        assert!(movement.ladder[0].is_peak);
        assert!(movement.ladder[0].is_current);
        assert!(
            !movement.ladder[1].visited,
            "o degrau nunca pisado aparece apagado"
        );
        assert!(
            movement.ladder[2].tier >= movement.ladder[1].tier,
            "a pirâmide vem de baixo para cima"
        );
    }

    #[test]
    fn a_escada_de_uma_equipe_mazda_nao_oferece_degrau_de_outra_marca() {
        // O recorte da Production é a base inteira da pirâmide, porque é contra
        // todas essas equipes que o RANKING dela se mede. Como escada, mente.
        let mut na_production = fato(4, 2033, "production_challenger", 1);
        na_production.class = "mazda".to_string();
        let facts = vec![fato(1, 2030, "mazda_rookie", 1), na_production];
        let grupo = vec![
            "mazda_rookie".to_string(),
            "mazda_amador".to_string(),
            "toyota_rookie".to_string(),
            "toyota_amador".to_string(),
            "bmw_m2".to_string(),
            "production_challenger".to_string(),
        ];

        let movement = build_team_movement(&facts, &grupo);
        let degraus: Vec<&str> = movement
            .ladder
            .iter()
            .map(|rung| rung.category_id.as_str())
            .collect();
        assert_eq!(
            degraus,
            vec!["mazda_rookie", "mazda_amador", "production_challenger"],
            "a escada é a da marca da equipe, não a do ranking"
        );
    }

    #[test]
    fn equipe_que_estreia_na_production_pega_a_marca_pela_classe() {
        let mut estreia = fato(1, 2030, "production_challenger", 1);
        estreia.class = "toyota".to_string();
        let movement = build_team_movement(&[estreia], &["production_challenger".to_string()]);
        let degraus: Vec<&str> = movement
            .ladder
            .iter()
            .map(|rung| rung.category_id.as_str())
            .collect();
        assert_eq!(
            degraus,
            vec!["toyota_rookie", "toyota_amador", "production_challenger"]
        );
    }

    #[test]
    fn fora_das_escadas_de_marca_o_grupo_continua_sendo_a_escada() {
        let movement = build_team_movement(&[fato(1, 2030, "gt3", 1)], &["gt3".to_string()]);
        assert_eq!(movement.ladder.len(), 1);
        assert_eq!(movement.ladder[0].category_id, "gt3");
    }

    #[test]
    fn teto_e_casa_nao_empatam_numa_categoria_so() {
        // A régua antiga (taxa de vitória) devolvia a MESMA categoria nos dois
        // cards quando havia uma só; teto e casa continuam iguais aqui, mas por
        // serem fatos distintos deixam de parecer um defeito num histórico longo.
        let facts = vec![
            fato(1, 2030, "gt4", 1),
            fato(2, 2031, "gt4", 1),
            fato(3, 2032, "gt3", 1),
        ];
        let movement = build_team_movement(&facts, &["gt4".to_string(), "gt3".to_string()]);
        assert_eq!(movement.peak_category, team_history_category_label("gt3"));
        assert_eq!(movement.home_category, team_history_category_label("gt4"));
    }
}
