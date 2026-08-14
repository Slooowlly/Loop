//! Evolução dos pilotos na virada: crescimento, declínio, motivação, retenção do
//! veterano insubstituível e registro das aposentadorias.

use super::*;

/// O grupo em que o piloto de fato disputou: a categoria e, nas multiclasse, a
/// CLASSE dentro dela (`endurance` + `lmp2`, `production_challenger` + `mazda`).
/// `None` de classe é a monoclasse, onde o grupo é a categoria inteira.
///
/// **É o grupo, não a categoria, que serve de régua da "superação da máquina".** A
/// posição de campeonato de Endurance e Production Challenger é apurada POR CLASSE
/// ([`build_special_category_standings`]), e comparar essa posição com um percentil
/// de carro medido na categoria inteira mede duas coisas diferentes: o campeão da
/// GT4 aparecia como 1º de um pelotão que inclui LMP2, e a expectativa dele saía do
/// carro mais rápido de uma classe que ele nunca disputou.
type GrupoCompetitivo = (String, Option<String>);

/// Classe sem lixo de borda: `""` e espaço em branco valem por ausência, como o
/// `NULLIF(TRIM(...))` que apura os standings especiais.
fn classe_do_grupo(classe: Option<&str>) -> Option<String> {
    classe
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_string)
}

fn grupo_competitivo(standing: &StandingEntry) -> GrupoCompetitivo {
    (
        standing.category.clone(),
        classe_do_grupo(standing.classe.as_deref()),
    )
}

/// A equipe disputa o mesmo grupo? Na MONOCLASSE (classe do grupo nula) o grupo é a
/// categoria inteira e a classe da equipe não é consultada — o comportamento ali
/// segue exatamente o de antes.
fn pertence_ao_grupo(team: &Team, grupo: &GrupoCompetitivo) -> bool {
    if team.categoria != grupo.0 {
        return false;
    }
    match &grupo.1 {
        Some(classe) => classe_do_grupo(team.classe.as_deref()).as_ref() == Some(classe),
        None => true,
    }
}

pub(super) fn process_driver_evolution(
    conn: &Connection,
    season: &Season,
    standings_by_driver: &HashMap<String, StandingEntry>,
    titles_by_driver: &HashMap<String, i32>,
    contracts_by_driver: &HashMap<String, Contract>,
    teams_by_id: &HashMap<String, Team>,
    rng: &mut impl Rng,
) -> Result<
    (
        Vec<GrowthReport>,
        Vec<MotivationReport>,
        Vec<RetirementInfo>,
        HashSet<String>,
    ),
    String,
> {
    let mut all_drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao buscar pilotos: {e}"))?;
    let existing_names: HashSet<String> = all_drivers
        .iter()
        .map(|driver| driver.nome.clone())
        .collect();
    let mut growth_reports = Vec::new();
    let mut motivation_reports = Vec::new();
    let mut retirements = Vec::new();

    // Distribuição de força de carro por GRUPO COMPETITIVO (times DISTINTOS que
    // competem nele), pra medir o percentil do carro de cada piloto — base da
    // "superação de expectativa" no crescimento (talento = render acima da própria
    // máquina).
    let mut cat_car_perfs: HashMap<GrupoCompetitivo, Vec<f64>> = HashMap::new();
    {
        let mut seen: HashSet<(GrupoCompetitivo, String)> = HashSet::new();
        for (driver_id, standing) in standings_by_driver {
            if let Some(contract) = contracts_by_driver.get(driver_id) {
                if let Some(team) = teams_by_id.get(&contract.equipe_id) {
                    let grupo = grupo_competitivo(standing);
                    if seen.insert((grupo.clone(), contract.equipe_id.clone())) {
                        cat_car_perfs
                            .entry(grupo)
                            .or_default()
                            .push(team.car_strength());
                    }
                }
            }
        }
        for perfs in cat_car_perfs.values_mut() {
            perfs.sort_by(|a, b| a.total_cmp(b));
        }
    }
    // Percentil do carro no grupo (0 = pior, 1 = melhor); midrank pra empates.
    // Grupo trivial/desconhecido → 0.5 (neutro, sem bônus nem penalidade).
    let car_percentile = |grupo: &GrupoCompetitivo, perf: f64| -> f64 {
        match cat_car_perfs.get(grupo) {
            Some(perfs) if perfs.len() > 1 => {
                let below = perfs.iter().filter(|&&p| p < perf).count() as f64;
                let equal = perfs
                    .iter()
                    .filter(|&&p| (p - perf).abs() < f64::EPSILON)
                    .count() as f64;
                (below + equal / 2.0) / perfs.len() as f64
            }
            _ => 0.5,
        }
    };

    for driver in &mut all_drivers {
        // O TÍTULO é creditado ANTES do filtro de status; o resto da evolução,
        // não. Envelhecer, crescer e perder motivação são coisas de quem está na
        // ativa. Ter vencido o campeonato é um fato da tabela final: se o piloto
        // chegou em dezembro lesionado ou suspenso, o campeonato ainda foi dele.
        //
        // Com o crédito lá embaixo, depois do `continue`, o arquivo
        // (`archive_driver_season`, que grava a linha de TODO MUNDO) registrava o
        // P1 e o contador de carreira não subia. O piloto ficava com um título a
        // menos do que a própria ficha lista ano a ano.
        let titles_won = titles_by_driver.get(&driver.id).copied().unwrap_or(0);
        if titles_won > 0 {
            driver.stats_carreira.titulos += titles_won as u32;
        }

        if driver.status != DriverStatus::Ativo {
            // O `update_driver` do fim do laço não é alcançado daqui, então o
            // crédito precisa ser gravado na saída — e só ele, para não escrever
            // linha de piloto que não mudou em nada.
            if titles_won > 0 {
                driver_queries::update_driver(conn, driver)
                    .map_err(|e| format!("Falha ao creditar titulo de '{}': {e}", driver.nome))?;
            }
            continue;
        }

        let standing = standings_by_driver.get(&driver.id).cloned();
        if let Some(standing) = standing {
            let team = contracts_by_driver
                .get(&driver.id)
                .and_then(|contract| teams_by_id.get(&contract.equipe_id));
            let team_car_strength = team.map(|team| team.car_strength()).unwrap_or(0.0);
            let grupo = grupo_competitivo(&standing);
            let car_field_percentile = team
                .map(|team| car_percentile(&grupo, team.car_strength()))
                .unwrap_or(0.5);

            let category_tier = get_category_config(&standing.category)
                .map(|config| config.tier)
                .unwrap_or(0);
            let growth_report = calculate_growth(
                driver,
                &standing.stats,
                team_car_strength,
                category_tier,
                car_field_percentile,
                rng,
            );
            if !growth_report.changes.is_empty() {
                growth_reports.push(growth_report);
            }

            let _decline_changes = apply_age_decline(driver, rng);
            let seasons_in_category = driver.temporadas_na_categoria as i32 + 1;
            // Superou a maquina: terminou bem acima do que a forca do carro previa.
            // Expectativa ~ (carros melhores NO GRUPO) * 2 pilotos + 1.
            let cars_ahead = teams_by_id
                .values()
                .filter(|team| {
                    pertence_ao_grupo(team, &grupo) && team.car_strength() > team_car_strength
                })
                .count() as i32;
            let expected_position = cars_ahead * 2 + 1;
            let outperformed_machinery = standing.stats.posicao_campeonato + 3 <= expected_position;
            // Só os sinais da TEMPORADA entram aqui. Os do offseason (promovido,
            // rebaixado, renovado, dispensado) não existem nesta altura do
            // pipeline e chegam no segundo passe — `apply_offseason_motivation`,
            // depois da promoção e do mercado.
            let motivation_ctx = MotivationContext {
                was_champion: standing.position == 1,
                seasons_in_category,
                outperformed_machinery,
            };
            let motivation_report =
                adjust_end_of_season_motivation(driver, &standing.stats, &motivation_ctx, rng);
            motivation_reports.push(motivation_report);

            driver.temporadas_na_categoria += 1;
            driver.corridas_na_categoria += standing.stats.corridas.max(0) as u32;
        }

        driver.idade += 1;
        if driver.motivacao < 20.0 {
            driver.temporadas_motivacao_baixa += 1;
        } else {
            driver.temporadas_motivacao_baixa = 0;
        }

        driver.close_career_season();

        let mut retirement =
            check_retirement(driver, driver.temporadas_motivacao_baixa as i32, false, rng);
        // Item 6: órfão ocioso (IA sem assento que não correu nesta temporada nem tem
        // categoria) — o mercado o preteriu. Se o check padrão não o aposentou, aplica
        // a chance de aposentadoria por falta de equipe (evita agente livre eterno).
        if !retirement.should_retire && !driver.is_jogador {
            let idle_orphan = !standings_by_driver.contains_key(&driver.id)
                && !contracts_by_driver.contains_key(&driver.id)
                && driver.categoria_atual.is_none();
            if idle_orphan
                && rng.gen::<f64>()
                    < idle_orphan_retirement_chance(driver.idade, driver.atributos.skill)
            {
                retirement.should_retire = true;
                retirement.reason = Some("Aposentou-se sem encontrar equipe".to_string());
            }
        }
        // Retenção de lenda: se o piloto quer se aposentar mas é INSUBSTITUÍVEL (nenhum
        // piloto licenciado da categoria de baixo o supera), o time o segura por +1 ano
        // com salário maior. Adia a aposentadoria até surgir um substituto à altura
        // (acontece naturalmente pela queda de idade). É decisão do time — vale inclusive
        // para o time do jogador (só não retém o próprio piloto-jogador).
        if retirement.should_retire
            && !try_retain_irreplaceable_veteran(conn, driver, contracts_by_driver, season)?
        {
            let reason = retirement
                .reason
                .clone()
                .unwrap_or_else(|| "Aposentadoria".to_string());
            let final_category = driver.categoria_atual.clone().or_else(|| {
                standings_by_driver
                    .get(&driver.id)
                    .map(|standing| standing.category.clone())
            });
            let retired_category = final_category
                .clone()
                .unwrap_or_else(|| "SemCategoria".to_string());
            persist_retired_driver(conn, driver, season, &retired_category, &reason)
                .map_err(|e| format!("Falha ao registrar aposentadoria: {e}"))?;
            process_retirement(driver);
            driver.categoria_atual = None;
            retirements.push(RetirementInfo {
                driver_id: driver.id.clone(),
                driver_name: driver.nome.clone(),
                age: driver.idade as i32,
                reason,
                categoria: final_category,
            });
        }
        driver_queries::update_driver(conn, driver)
            .map_err(|e| format!("Falha ao salvar piloto '{}': {e}", driver.nome))?;
    }

    Ok((
        growth_reports,
        motivation_reports,
        retirements,
        existing_names,
    ))
}

/// O assento de um piloto num retrato do banco.
///
/// Guarda só o DEGRAU. A equipe também morava aqui, para separar "renovou" de "só
/// continua no meio de um plurianual", e saiu junto com o sinal de renovação — ver
/// [`crate::evolution::motivation::adjust_contract_renewal_motivation`], que passou a
/// aplicar o bônus no instante em que o contrato novo é gravado.
#[derive(Debug, Clone)]
pub(super) struct Assento {
    /// `None` quando a categoria do contrato não bate com nenhuma config conhecida —
    /// comparar degraus com um palpite inventaria promoções e rebaixamentos que não
    /// houve.
    pub(super) tier: Option<u8>,
}

/// O assento de cada piloto: `piloto_id -> Assento`.
pub(super) fn seat_map(conn: &Connection) -> Result<HashMap<String, Assento>, String> {
    let contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao buscar contratos ativos: {e}"))?;
    Ok(contracts
        .into_iter()
        .map(|contract| {
            // Categoria multiclasse chega como "endurance:lmp2"; a config mora na base.
            let base = contract
                .categoria
                .split_once(':')
                .map_or(contract.categoria.as_str(), |(base, _)| base);
            let tier = get_category_config(base).map(|config| config.tier);
            (contract.piloto_id, Assento { tier })
        })
        .collect())
}

/// SEGUNDO PASSE da motivação: o DEGRAU em que o offseason deixou o piloto.
///
/// Compara o assento de antes da virada com o de depois do mercado de
/// pré-temporada. Derivar do antes/depois — em vez de consumir o
/// `PromotionResult` — captura o desfecho como o piloto o VIVEU: terminar um
/// degrau abaixo é rebaixamento para ele, tenha a equipe caído ou tenha ele sido
/// negociado para baixo.
///
/// **Nem a RENOVAÇÃO nem a PERDA DA VAGA são julgadas aqui.** Nenhuma das duas é um
/// degrau: são eventos, e o único instante em que são um fato é aquele em que o
/// contrato novo é gravado ou o assento se esvazia — que no modo jogável acontece
/// depois desta função, semanas depois. Ver
/// [`crate::evolution::motivation::adjust_contract_renewal_motivation`] e
/// [`crate::evolution::motivation::adjust_lost_seat_motivation`].
pub(super) fn apply_offseason_motivation(
    conn: &Connection,
    seats_before: &HashMap<String, Assento>,
) -> Result<Vec<MotivationReport>, String> {
    let seats_after = seat_map(conn)?;
    let mut drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao buscar pilotos para o passe de offseason: {e}"))?;
    let mut reports = Vec::new();

    for driver in &mut drivers {
        if driver.is_jogador || driver.status == DriverStatus::Aposentado {
            continue;
        }
        // Sem assento ANTES não há degrau a comparar: quem entrou no offseason como
        // agente livre não subiu nem desceu de categoria.
        let Some(antes) = seats_before.get(&driver.id) else {
            continue;
        };
        // Sem assento DEPOIS também não: ficar sem vaga é um evento, e ele já foi
        // aplicado no instante em que aconteceu (ver o doc acima). Aqui ele não vira
        // "degrau nenhum" nem arrasta o piloto para dentro do passe.
        let degrau = match (antes.tier, seats_after.get(&driver.id).and_then(|a| a.tier)) {
            (Some(antes), Some(agora)) => Some(agora.cmp(&antes)),
            _ => None,
        };
        let ctx = OffseasonContext {
            was_promoted: degrau == Some(std::cmp::Ordering::Greater),
            was_relegated: degrau == Some(std::cmp::Ordering::Less),
        };
        // Trocar de equipe no mesmo degrau não é nenhum dos sinais; sem isso o
        // ambicioso frustrado dispararia sozinho e o passe viraria um imposto anual
        // em cima de quem só mudou de endereço.
        if !ctx.was_promoted && !ctx.was_relegated {
            continue;
        }
        reports.push(adjust_offseason_motivation(driver, &ctx));
        driver_queries::update_driver_motivation(conn, &driver.id, driver.motivacao)
            .map_err(|e| format!("Falha ao gravar motivacao de '{}': {e}", driver.nome))?;
    }

    Ok(reports)
}

/// Funde os relatórios do segundo passe nos do primeiro, um por piloto: a tela de
/// fim de temporada mostra uma linha por piloto, e duas entradas para o mesmo
/// nome leriam como bug.
pub(super) fn merge_motivation_reports(
    base: &mut Vec<MotivationReport>,
    extra: Vec<MotivationReport>,
) {
    for novo in extra {
        match base.iter_mut().find(|r| r.driver_id == novo.driver_id) {
            Some(existente) => {
                existente.reasons.extend(novo.reasons);
                existente.new_motivation = novo.new_motivation;
                existente.delta = existente.new_motivation as i8 - existente.old_motivation as i8;
            }
            None => base.push(novo),
        }
    }
}

/// Tenta reter um veterano que quer se aposentar mas é INSUBSTITUÍVEL: nenhum piloto
/// licenciado da categoria de baixo tem skill >= a dele. Nesse caso estende o contrato
/// por +1 ano com salário +40% e o mantém ativo (retorna `true`), adiando a aposentadoria
/// até surgir um substituto à altura (acontece naturalmente pela queda de idade — sem teto
/// de idade). É uma decisão DO TIME — vale também para o time do jogador (o jogador não
/// controla o que o time decide). Só IA (o piloto-jogador nunca é retido contra a vontade)
/// e só categorias regulares (não-estreia, não-especiais; rookies têm reposição abundante
/// e especiais usam convocação).
pub(super) fn try_retain_irreplaceable_veteran(
    conn: &Connection,
    driver: &Driver,
    contracts_by_driver: &HashMap<String, Contract>,
    season: &Season,
) -> Result<bool, String> {
    const RETENTION_RAISE: f64 = 1.40;

    if driver.is_jogador || driver.stats_carreira.corridas == 0 {
        return Ok(false);
    }
    let Some(contract) = contracts_by_driver.get(&driver.id) else {
        return Ok(false);
    };
    if runs_in_special_phase(&contract.categoria) {
        return Ok(false);
    }
    let Some(required_license) =
        required_license_for_division(&contract.categoria, contract.classe.as_deref())
    else {
        return Ok(false);
    };

    let feeders = get_feeder_categories(&contract.categoria);
    if feeders.is_empty() {
        return Ok(false);
    }
    let placeholders = feeders.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT COUNT(*) FROM drivers d
         WHERE d.status = 'Ativo' AND d.is_jogador = 0
           AND d.categoria_atual IN ({placeholders})
           AND d.skill >= ?
           AND EXISTS (
               SELECT 1 FROM licenses l
               WHERE l.piloto_id = d.id AND CAST(l.nivel AS INTEGER) >= ?
           )"
    );
    let mut params: Vec<rusqlite::types::Value> = feeders
        .iter()
        .map(|feeder| rusqlite::types::Value::Text((*feeder).to_string()))
        .collect();
    params.push(rusqlite::types::Value::Real(driver.atributos.skill));
    params.push(rusqlite::types::Value::Integer(required_license as i64));
    let better_replacements: i64 = conn
        .query_row(&sql, rusqlite::params_from_iter(params), |row| row.get(0))
        .map_err(|e| format!("Falha ao avaliar substitutos do veterano: {e}"))?;
    if better_replacements > 0 {
        return Ok(false);
    }

    // Insubstituível: rescinde o contrato atual e cria um de +1 ano com salário +40%.
    contract_queries::update_contract_status(conn, &contract.id, &ContractStatus::Rescindido)
        .map_err(|e| format!("Falha ao rescindir contrato do veterano retido: {e}"))?;
    let mut renewed = Contract::new(
        next_id(conn, IdType::Contract)
            .map_err(|e| format!("Falha ao gerar ID de contrato de retenção: {e}"))?,
        driver.id.clone(),
        driver.nome.clone(),
        contract.equipe_id.clone(),
        contract.equipe_nome.clone(),
        season.numero + 1,
        1,
        contract.salario_anual * RETENTION_RAISE,
        contract.papel.clone(),
        contract.categoria.clone(),
    );
    renewed.classe = contract.classe.clone();
    contract_queries::insert_contract(conn, &renewed)
        .map_err(|e| format!("Falha ao inserir contrato de retenção: {e}"))?;
    Ok(true)
}

pub(crate) fn persist_retired_driver(
    conn: &Connection,
    driver: &Driver,
    season: &Season,
    final_category: &str,
    reason: &str,
) -> Result<(), String> {
    let stats_json = serde_json::to_string(&driver.stats_carreira).map_err(|e| {
        format!(
            "Falha ao serializar estatisticas do piloto aposentado '{}': {e}",
            driver.nome
        )
    })?;
    conn.execute(
        "INSERT OR REPLACE INTO retired (
            piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            &driver.id,
            &driver.nome,
            season.ano.to_string(),
            final_category,
            stats_json,
            reason,
        ],
    )
    .map_err(|e| format!("Falha ao salvar piloto aposentado '{}': {e}", driver.nome))?;
    Ok(())
}
