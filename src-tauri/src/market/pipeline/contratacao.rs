//! Escrituração das contratações: encerra o contrato antigo, abre o novo, mexe nos
//! slots da equipe e concede a licença quando necessário.
//!
//! É o "cartório" do mercado — todas as etapas (assédio, janela, consolidação)
//! terminam aqui.

use super::*;

pub(crate) fn sign_driver_to_team(
    conn: &Connection,
    driver: &Driver,
    vacancy: &Vacancy,
    new_season_number: i32,
    salary: f64,
    duration: i32,
    role: TeamRole,
) -> Result<(), String> {
    with_savepoint(conn, "market_sign_driver", || {
        let team = team_queries::get_team_by_id(conn, &vacancy.team_id)
            .map_err(|e| format!("Falha ao buscar equipe da assinatura: {e}"))?
            .ok_or_else(|| format!("Equipe '{}' nao encontrada", vacancy.team_id))?;
        ensure_driver_can_join_division(
            conn,
            &driver.id,
            &driver.nome,
            &vacancy.categoria,
            vacancy.classe.as_deref(),
        )?;
        let mut new_contract = Contract::new(
            next_id(conn, IdType::Contract)
                .map_err(|e| format!("Falha ao gerar ID de contrato: {e}"))?,
            driver.id.clone(),
            driver.nome.clone(),
            vacancy.team_id.clone(),
            team.nome.clone(),
            new_season_number,
            duration,
            salary,
            role,
            vacancy.categoria.clone(),
        );
        new_contract.classe = team.classe.clone();
        contract_queries::insert_contract(conn, &new_contract)
            .map_err(|e| format!("Falha ao inserir contratacao: {e}"))?;

        let mut updated_driver = driver.clone();
        updated_driver.mover_para_categoria(Some(vacancy.categoria.clone()));
        driver_queries::update_driver(conn, &updated_driver).map_err(|e| {
            format!(
                "Falha ao atualizar piloto contratado '{}': {e}",
                driver.nome
            )
        })?;
        Ok(())
    })?;
    // Rivalidade entre EQUIPES — Fonte 2 (Elo 2) na TRANSFERÊNCIA NORMAL: assinar um piloto
    // que largou o rival na temporada passada deixa marca no par de times. Fora do savepoint
    // (best-effort — nunca desfaz a assinatura) e DEPOIS do commit, pra o histórico já incluir
    // o contrato novo. O poaching tem seu próprio site (rancor máximo), então não duplica.
    seed_ordinary_transfer_rivalry(conn, driver, &vacancy.team_id, new_season_number);
    Ok(())
}

/// Transfere `amount` do caixa do time `from` para o `to` — a 1ª mecânica de dinheiro
/// time→time (a multa de rescisão do poaching, Fase 2b). No-op se `amount ≤ 0` ou
/// mesma equipe. Debita o assediante e credita o vendedor.
pub(crate) fn transfer_between_teams(
    conn: &Connection,
    from_team: &str,
    to_team: &str,
    amount: f64,
) -> Result<(), String> {
    if amount <= 0.0 || from_team == to_team {
        return Ok(());
    }
    team_queries::adjust_team_cash(conn, from_team, -amount)
        .map_err(|e| format!("Falha ao debitar multa do assediante: {e}"))?;
    team_queries::adjust_team_cash(conn, to_team, amount)
        .map_err(|e| format!("Falha ao creditar multa ao vendedor: {e}"))?;
    Ok(())
}

/// Executa a troca de assentos: `riser` (de baixo) assume a vaga de `weak` (de cima)
/// e `weak` assume a vaga de `riser`. Rescinde os dois contratos e cria os novos
/// trocados; ambos já têm a licença das divisões de destino (ver chamador).
pub(super) fn swap_contract_seats(
    conn: &Connection,
    riser: &Contract,
    weak: &Contract,
    new_season_number: i32,
    report: &mut MarketReport,
) -> Result<(), String> {
    for contract_id in [&riser.id, &weak.id] {
        contract_queries::update_contract_status(conn, contract_id, &ContractStatus::Rescindido)
            .map_err(|e| format!("Falha ao rescindir contrato na troca de mérito: {e}"))?;
    }

    let mut move_driver = |conn: &Connection,
                           piloto_id: &str,
                           piloto_nome: &str,
                           destino: &Contract,
                           tipo: &str|
     -> Result<(), String> {
        let mut contract = Contract::new(
            next_id(conn, IdType::Contract)
                .map_err(|e| format!("Falha ao gerar ID de contrato na troca: {e}"))?,
            piloto_id.to_string(),
            piloto_nome.to_string(),
            destino.equipe_id.clone(),
            destino.equipe_nome.clone(),
            new_season_number,
            1,
            destino.salario_anual,
            destino.papel.clone(),
            destino.categoria.clone(),
        );
        contract.classe = destino.classe.clone();
        contract_queries::insert_contract(conn, &contract)
            .map_err(|e| format!("Falha ao inserir contrato na troca de mérito: {e}"))?;

        if let Some(mut driver) = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao carregar piloto na troca: {e}"))?
            .into_iter()
            .find(|driver| driver.id == piloto_id)
        {
            driver.mover_para_categoria(Some(destino.categoria.clone()));
            driver_queries::update_driver(conn, &driver)
                .map_err(|e| format!("Falha ao atualizar categoria na troca: {e}"))?;
        }

        report.new_signings.push(SigningInfo {
            driver_id: piloto_id.to_string(),
            driver_name: piloto_nome.to_string(),
            team_id: destino.equipe_id.clone(),
            team_name: destino.equipe_nome.clone(),
            categoria: destino.categoria.clone(),
            papel: destino.papel.as_str().to_string(),
            tipo: tipo.to_string(),
        });
        Ok(())
    };

    // riser sobe para a vaga de weak; weak desce para a vaga do riser.
    move_driver(
        conn,
        &riser.piloto_id,
        &riser.piloto_nome,
        weak,
        "promocao_merito",
    )?;
    move_driver(
        conn,
        &weak.piloto_id,
        &weak.piloto_nome,
        riser,
        "rebaixamento",
    )?;
    Ok(())
}

pub(super) fn generate_and_sign_rookie_for_vacancy(
    conn: &Connection,
    vacancy: &Vacancy,
    new_season_number: i32,
    debut_year: i32,
    rng: &mut impl Rng,
) -> Result<Driver, String> {
    let mut existing_names: HashSet<String> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar nomes existentes para rookie: {e}"))?
        .into_iter()
        .map(|driver| driver.nome)
        .collect();
    let mut rookie = generate_rookies(1, debut_year, &mut existing_names, rng)
        .into_iter()
        .next()
        .ok_or_else(|| "Falha ao gerar rookie para vaga final.".to_string())?;
    rookie.id =
        next_id(conn, IdType::Driver).map_err(|e| format!("Falha ao gerar ID de rookie: {e}"))?;
    rookie.categoria_atual = None;

    driver_queries::insert_driver(conn, &rookie)
        .map_err(|e| format!("Falha ao inserir rookie '{}': {e}", rookie.nome))?;
    grant_driver_license_for_division_if_needed(
        conn,
        &rookie.id,
        &vacancy.categoria,
        vacancy.classe.as_deref(),
    )?;
    sign_driver_to_team(
        conn,
        &rookie,
        vacancy,
        new_season_number,
        calculate_offer_salary(vacancy, &rookie, rng),
        1,
        vacancy.papel_necessario.clone(),
    )?;

    Ok(rookie)
}
