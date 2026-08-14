//! Assédio entre equipes da IA (poaching) e a rivalidade de transferência comum.
//!
//! Etapa que roda ANTES da janela aberta: as equipes fortes vão buscar quem já tem
//! contrato, disputam o alvo em leilão de lances e pagam a multa rescisória. O
//! `PoachAudit` é a trilha auditável de cada disputa, consumida pelo painel de debug.

use super::*;

/// DEBUG (Fase 2b): raio-x de um assédio — tudo que decidiu o leilão. O leilão só
/// roda entre IAs e não tem tela até o 2b.3; isto é o que o comando de debug
/// (dry-run) devolve pra dar pra ver acontecendo.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct PoachAuditBid {
    pub team_name: String,
    pub salary: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct PoachAudit {
    pub categoria: String,
    pub poacher_name: String,
    pub poacher_cash: f64,
    pub seller_name: String,
    pub target_name: String,
    pub target_skill: f64,
    pub target_fama: f64,
    pub incumbent_name: String,
    pub buyout: f64,
    pub salario_atual: f64,
    /// Vínculo do alvo com o assediante e com a casa (0–100).
    pub bond_poacher: f64,
    pub bond_holder: f64,
    pub ceiling_poacher: f64,
    pub ceiling_holder: f64,
    pub bids: Vec<PoachAuditBid>,
    pub poacher_wins: bool,
    pub salario_final: f64,
}

/// Valor de um piloto (por id) para o assediante: `poach_target_value` (skill +
/// apelo comercial × necessidade). 0 se o piloto não existe.
pub(super) fn poach_value_of(drivers_by_id: &HashMap<String, Driver>, id: &str, need: f64) -> f64 {
    drivers_by_id
        .get(id)
        .map(|d| {
            crate::market::poaching::poach_target_value(d.atributos.skill, d.atributos.midia, need)
        })
        .unwrap_or(0.0)
}

/// Executa uma quebra de contrato: o `poacher` arranca `target` (contratado no
/// `seller`) pagando a `multa`; o piloto dispensado (`incumbent`) vira agente livre
/// limpo (categoria zerada → a escada o repesca). A vaga aberta no vendedor é
/// preenchida depois pela escada. (Fase 2b.1, só IA.)
#[allow(clippy::too_many_arguments)]
pub(super) fn execute_poach(
    conn: &Connection,
    poacher: &crate::models::team::Team,
    seller_team_id: &str,
    target: &Driver,
    target_contract: &Contract,
    incumbent_contract: &Contract,
    buyout: f64,
    salary: f64,
    new_season_number: i32,
    report: &mut MarketReport,
) -> Result<(), String> {
    // Rescinde o alvo (no vendedor) e o dispensado (no poacher).
    for cid in [&target_contract.id, &incumbent_contract.id] {
        contract_queries::update_contract_status(conn, cid, &ContractStatus::Rescindido)
            .map_err(|e| format!("Falha ao rescindir contrato no poaching: {e}"))?;
    }

    // O dispensado volta ao pool como agente livre LIMPO (categoria None) — senão
    // vira órfão que a escada não repesca.
    if let Some(mut incumbent) =
        driver_queries::get_driver(conn, &incumbent_contract.piloto_id).ok()
    {
        incumbent.categoria_atual = None;
        // Perdeu a vaga AQUI, e no sentido mais literal que o jogo tem: o assento dele
        // foi dado a outro piloto no meio do contrato. O efeito vai no mesmo write.
        if !incumbent.is_jogador {
            crate::evolution::motivation::adjust_lost_seat_motivation(&mut incumbent);
        }
        driver_queries::update_driver(conn, &incumbent)
            .map_err(|e| format!("Falha ao liberar dispensado no poaching: {e}"))?;
    }

    // Novo contrato: alvo → poacher, no papel do assento liberado.
    let mut contract = Contract::new(
        next_id(conn, IdType::Contract)
            .map_err(|e| format!("Falha ao gerar ID de contrato no poaching: {e}"))?,
        target.id.clone(),
        target.nome.clone(),
        poacher.id.clone(),
        poacher.nome.clone(),
        new_season_number,
        1,
        salary,
        incumbent_contract.papel.clone(),
        poacher.categoria.clone(),
    );
    contract.classe = poacher.classe.clone();
    contract_queries::insert_contract(conn, &contract)
        .map_err(|e| format!("Falha ao inserir contrato no poaching: {e}"))?;

    let mut moved = target.clone();
    moved.mover_para_categoria(Some(poacher.categoria.clone()));
    driver_queries::update_driver(conn, &moved)
        .map_err(|e| format!("Falha ao atualizar piloto arrancado: {e}"))?;

    // A multa: dinheiro do assediante → vendedor.
    transfer_between_teams(conn, &poacher.id, seller_team_id, buyout)?;

    // Rivalidade entre EQUIPES — Fonte 2 (roubo de talento, o Elo 2): arrancar um astro
    // contratado do rival deixa marca duradoura no par de times. É o assédio mid-contrato
    // (rancor máximo). Best-effort — não desfaz o poaching se a semeadura falhar.
    if let Err(e) = crate::rivalry::team::seed_team_rivalry_from_transfer(
        conn,
        seller_team_id,
        &poacher.id,
        target,
        true, // is_poaching
        new_season_number,
    ) {
        eprintln!("Aviso: falha ao semear rivalidade de equipe no poaching: {e}");
    }

    report.new_signings.push(SigningInfo {
        driver_id: target.id.clone(),
        driver_name: target.nome.clone(),
        team_id: poacher.id.clone(),
        team_name: poacher.nome.clone(),
        categoria: poacher.categoria.clone(),
        papel: incumbent_contract.papel.as_str().to_string(),
        tipo: "poaching".to_string(),
    });
    Ok(())
}

/// Passe de POACHING / quebra de contrato (Fase 2b, só IA): na janela, times com
/// caixa arrancam astros CONTRATADOS de outros times da mesma categoria, pagando a
/// multa. Gatilho = fama + mérito (`poach_target_value`); conservador (poucos por
/// janela). NUNCA mexe no jogador nem no time dele (isso é 2b.3).
///
/// O time atual **reage** (2b.2): cada assédio vira um leilão de salário
/// (`resolve_salary_auction`) em que o vínculo do piloto com a casa é defesa. Se o
/// time segura, o aumento fica no contrato; se perde, a multa paga o consolo.
///
/// `audit` recebe o raio-x de cada assédio (só pro debug/dry-run; o jogo real
/// passa um vetor que descarta).
pub(crate) fn run_poaching_pass(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    new_season_number: i32,
    rng: &mut impl Rng,
    report: &mut MarketReport,
    audit: &mut Vec<PoachAudit>,
) -> Result<(), String> {
    let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos p/ poaching: {e}"))?
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    let active = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao carregar contratos p/ poaching: {e}"))?;
    let contract_by_driver: HashMap<String, Contract> = active
        .iter()
        .cloned()
        .map(|c| (c.piloto_id.clone(), c))
        .collect();
    let player_seller_teams: HashSet<String> = teams
        .iter()
        .filter(|t| t.is_player_team)
        .map(|t| t.id.clone())
        .collect();

    let is_active_non_player = |id: &str| {
        drivers_by_id
            .get(id)
            .is_some_and(|d| !d.is_jogador && d.status == DriverStatus::Ativo)
    };

    // Poachers: times regulares, não do jogador, com AS DUAS vagas preenchidas (só
    // substituem quem já têm). Embaralha (determinístico via rng).
    let mut poachers: Vec<crate::models::team::Team> = teams
        .iter()
        .filter(|t| uses_regular_contracts(&t.categoria) && !t.is_player_team)
        .filter(|t| t.piloto_1_id.is_some() && t.piloto_2_id.is_some())
        .cloned()
        .collect();
    for i in (1..poachers.len()).rev() {
        let j = rng.gen_range(0..=i);
        poachers.swap(i, j);
    }

    let max_poaches = (teams.len() / 8).clamp(1, 3);
    let mut done = 0usize;
    let mut moved: HashSet<String> = HashSet::new();

    for poacher_stale in poachers {
        if done >= max_poaches {
            break;
        }
        let Some(poacher) = team_queries::get_team_by_id(conn, &poacher_stale.id)
            .ok()
            .flatten()
        else {
            continue;
        };
        let (Some(p1), Some(p2)) = (poacher.piloto_1_id.clone(), poacher.piloto_2_id.clone())
        else {
            continue; // perdeu uma vaga no meio do passe
        };
        let need = crate::fame::team_need_factor(
            crate::finance::planning::derive_budget_index_from_money(&poacher),
            poacher.reputacao,
        );

        // Incumbente = o mais fraco dos dois (por valor), não-jogador, ainda não movido.
        let mut inc_candidates: Vec<String> = [p1, p2]
            .into_iter()
            .filter(|id| is_active_non_player(id) && !moved.contains(id))
            .collect();
        inc_candidates.sort_by(|a, b| {
            poach_value_of(&drivers_by_id, a, need).total_cmp(&poach_value_of(
                &drivers_by_id,
                b,
                need,
            ))
        });
        let Some(incumbent_id) = inc_candidates.first().cloned() else {
            continue;
        };
        let Some(incumbent_contract) = contract_by_driver.get(&incumbent_id) else {
            continue;
        };
        let incumbent_value = poach_value_of(&drivers_by_id, &incumbent_id, need);

        // Melhor alvo: contratado, mesma categoria/classe, outro time (não do jogador),
        // não-jogador, upgrade claro, multa que cabe no caixa.
        let mut best: Option<(Contract, Driver, f64)> = None;
        for c in &active {
            if c.categoria != poacher.categoria
                || c.classe != poacher.classe
                || c.equipe_id == poacher.id
                || moved.contains(&c.piloto_id)
                || player_seller_teams.contains(&c.equipe_id)
                || !is_active_non_player(&c.piloto_id)
            {
                continue;
            }
            let Some(target) = drivers_by_id.get(&c.piloto_id) else {
                continue;
            };
            let tv = crate::market::poaching::poach_target_value(
                target.atributos.skill,
                target.atributos.midia,
                need,
            );
            if !crate::market::poaching::is_clear_upgrade(tv, incumbent_value) {
                continue;
            }
            let years = (c.temporada_fim - new_season_number + 1).max(1);
            let buyout = crate::market::poaching::buyout_fee(
                c.salario_anual,
                years,
                target.atributos.skill,
                target.atributos.midia,
            );
            if !crate::market::poaching::can_afford_buyout(poacher.cash_balance, buyout) {
                continue;
            }
            let better = best.as_ref().is_none_or(|(_, bd, _)| {
                tv > crate::market::poaching::poach_target_value(
                    bd.atributos.skill,
                    bd.atributos.midia,
                    need,
                )
            });
            if better {
                best = Some((c.clone(), target.clone(), buyout));
            }
        }

        if let Some((target_contract, target, buyout)) = best {
            // Leilão de salário (Fase 2b.2): o time atual briga pra segurar. O status
            // quo é o lance de abertura — vínculo alto já é defesa, sem gastar nada.
            let Some(seller) = team_queries::get_team_by_id(conn, &target_contract.equipe_id)
                .ok()
                .flatten()
            else {
                continue;
            };
            let reference = target_contract.salario_anual;
            let poacher_side = crate::market::poaching::AuctionSide {
                team_id: poacher.id.clone(),
                team_quality: team_quality(&poacher),
                bond: crate::market::bond::get_bond(conn, &target.id, &poacher.id)?,
                // O caixa do assediante já conta a multa que ele vai desembolsar.
                ceiling: crate::market::poaching::salary_ceiling(
                    reference,
                    crate::market::poaching::poach_target_value(
                        target.atributos.skill,
                        target.atributos.midia,
                        need,
                    ),
                    poacher.cash_balance - buyout,
                ),
            };
            let seller_need = crate::fame::team_need_factor(
                crate::finance::planning::derive_budget_index_from_money(&seller),
                seller.reputacao,
            );
            let holder_side = crate::market::poaching::AuctionSide {
                team_id: seller.id.clone(),
                team_quality: team_quality(&seller),
                bond: crate::market::bond::get_bond(conn, &target.id, &seller.id)?,
                ceiling: crate::market::poaching::salary_ceiling(
                    reference,
                    crate::market::poaching::poach_target_value(
                        target.atributos.skill,
                        target.atributos.midia,
                        seller_need,
                    ),
                    seller.cash_balance,
                ),
            };
            let auction = crate::market::poaching::resolve_salary_auction(
                reference,
                &poacher_side,
                &holder_side,
            );

            audit.push(PoachAudit {
                categoria: poacher.categoria.clone(),
                poacher_name: poacher.nome.clone(),
                poacher_cash: poacher.cash_balance,
                seller_name: seller.nome.clone(),
                target_name: target.nome.clone(),
                target_skill: target.atributos.skill,
                target_fama: target.atributos.midia,
                incumbent_name: drivers_by_id
                    .get(&incumbent_id)
                    .map(|d| d.nome.clone())
                    .unwrap_or_else(|| incumbent_id.clone()),
                buyout,
                salario_atual: reference,
                bond_poacher: poacher_side.bond,
                bond_holder: holder_side.bond,
                ceiling_poacher: poacher_side.ceiling,
                ceiling_holder: holder_side.ceiling,
                bids: auction
                    .bids
                    .iter()
                    .map(|b| PoachAuditBid {
                        team_name: if b.team_id == poacher.id {
                            poacher.nome.clone()
                        } else {
                            seller.nome.clone()
                        },
                        salary: b.salary,
                    })
                    .collect(),
                poacher_wins: auction.poacher_wins,
                salario_final: auction.salary,
            });

            if auction.poacher_wins {
                execute_poach(
                    conn,
                    &poacher,
                    &target_contract.equipe_id,
                    &target,
                    &target_contract,
                    incumbent_contract,
                    buyout,
                    auction.salary,
                    new_season_number,
                    report,
                )?;
                moved.insert(target.id.clone());
                moved.insert(incumbent_id);
                done += 1;
            } else if auction.bids.len() > 1 {
                // Houve assédio de verdade e o time atual cobriu: o aumento fica no
                // contrato (segurar um astro custa). Se ninguém chegou a dar lance,
                // não houve notícia — e o piloto segue livre pra outro assediante.
                if auction.salary > reference {
                    contract_queries::update_contract_salary(
                        conn,
                        &target_contract.id,
                        auction.salary,
                    )
                    .map_err(|e| format!("Falha ao reajustar salario na retencao: {e}"))?;
                }
                report.new_signings.push(SigningInfo {
                    driver_id: target.id.clone(),
                    driver_name: target.nome.clone(),
                    team_id: seller.id.clone(),
                    team_name: seller.nome.clone(),
                    categoria: seller.categoria.clone(),
                    papel: target_contract.papel.as_str().to_string(),
                    tipo: "retencao".to_string(),
                });
                moved.insert(target.id.clone());
            }
        }
    }

    if done > 0 {
        let refreshed: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao recarregar pilotos apos poaching: {e}"))?
            .into_iter()
            .map(|d| (d.id.clone(), d))
            .collect();
        sync_team_slots_from_active_regular_contracts(conn, teams, &refreshed)?;
    }
    Ok(())
}

/// Fonte 2 (Elo 2) para TRANSFERÊNCIAS NORMAIS (não-poaching): se o piloto correu na
/// temporada imediatamente anterior por um time DIFERENTE do novo, semeia rivalidade de
/// mercado LEVE (`is_poaching=false` — o peso final vem do calibre do piloto, dentro da
/// própria `seed_team_rivalry_from_transfer`). No-op para rookie (sem histórico), renovação
/// (mesmo time) ou quem voltou de um período parado (saída não-fresca, evita semear com um
/// time de anos atrás). Best-effort: um erro aqui nunca falha a assinatura.
pub(super) fn seed_ordinary_transfer_rivalry(
    conn: &Connection,
    driver: &Driver,
    new_team_id: &str,
    new_season_number: i32,
) {
    let Ok(history) = contract_queries::get_contracts_for_pilot(conn, &driver.id) else {
        return;
    };
    // Histórico já vem por temporada DESC → o 1º contrato num time diferente do novo é o
    // último time real do piloto (o contrato recém-inserido é no time novo, então é ignorado).
    let Some(prev) = history.iter().find(|c| c.equipe_id != new_team_id) else {
        return;
    };
    // Só saída FRESCA: terminou na temporada imediatamente anterior à da assinatura.
    if prev.temporada_fim != new_season_number - 1 {
        return;
    }
    if let Err(e) = crate::rivalry::team::seed_team_rivalry_from_transfer(
        conn,
        &prev.equipe_id,
        new_team_id,
        driver,
        false, // transferência normal (não é assédio mid-contrato)
        new_season_number,
    ) {
        eprintln!("Aviso: falha ao semear rivalidade de equipe na transferência: {e}");
    }
}
