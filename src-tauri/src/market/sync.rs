//! Sincronismo entre os CONTRATOS ativos e os assentos das equipes.
//!
//! É a função chamada em quase todo passo do mercado, e a fonte de verdade é sempre a
//! mesma: os contratos regulares ativos mandam, os campos `piloto_1_id`/`piloto_2_id` da
//! equipe e o `categoria_atual` do piloto obedecem. Tudo o que não couber nessa regra
//! (contrato de aposentado, de categoria que não é a da equipe, terceiro contrato na
//! mesma equipe, contrato de equipe que não existe mais) é RESCINDIDO aqui.
//!
//! Erro de sincronismo de vaga só aparece três passos adiante, no lugar errado — daí os
//! testes no fim do arquivo cobrirem cada uma dessas regras direto.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::db::queries::contracts as contract_queries;
use crate::db::queries::teams as team_queries;
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, DriverStatus};

pub(crate) fn sync_team_slots_from_active_regular_contracts(
    conn: &Connection,
    teams: &[crate::models::team::Team],
    drivers_by_id: &HashMap<String, Driver>,
) -> Result<(), String> {
    let active_contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao carregar contratos ativos: {e}"))?;
    let mut valid_contracts: HashMap<String, Vec<Contract>> = HashMap::new();

    for contract in active_contracts {
        let driver = drivers_by_id.get(&contract.piloto_id).ok_or_else(|| {
            format!(
                "Contrato ativo '{}' referencia piloto inexistente '{}'",
                contract.id, contract.piloto_id
            )
        })?;
        if driver.status == DriverStatus::Aposentado {
            contract_queries::update_contract_status(
                conn,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato invalido '{}': {e}",
                    contract.id
                )
            })?;
            continue;
        }
        valid_contracts
            .entry(contract.equipe_id.clone())
            .or_default()
            .push(contract);
    }

    for team in teams {
        let raw_contracts = valid_contracts.remove(&team.id).unwrap_or_default();
        let mut contracts = Vec::new();
        for contract in raw_contracts {
            if contract.categoria != team.categoria || contract.classe != team.classe {
                contract_queries::update_contract_status(
                    conn,
                    &contract.id,
                    &ContractStatus::Rescindido,
                )
                .map_err(|e| {
                    format!(
                        "Falha ao rescindir contrato fora da categoria/classe da equipe '{}': {e}",
                        contract.id
                    )
                })?;
                continue;
            }
            contracts.push(contract);
        }
        contracts.sort_by(|a, b| {
            let skill_a = drivers_by_id
                .get(&a.piloto_id)
                .expect("piloto validado deve existir para ordenacao")
                .atributos
                .skill;
            let skill_b = drivers_by_id
                .get(&b.piloto_id)
                .expect("piloto validado deve existir para ordenacao")
                .atributos
                .skill;
            skill_b.total_cmp(&skill_a)
        });
        let piloto_1 = contracts
            .first()
            .map(|contract| contract.piloto_id.as_str());
        let piloto_2 = contracts.get(1).map(|contract| contract.piloto_id.as_str());

        for contract in contracts.iter().take(2) {
            conn.execute(
                "UPDATE drivers SET categoria_atual = ?1 WHERE id = ?2",
                rusqlite::params![&contract.categoria, &contract.piloto_id],
            )
            .map_err(|e| {
                format!(
                    "Falha ao sincronizar categoria atual do piloto '{}': {e}",
                    contract.piloto_id
                )
            })?;
        }

        for contract in contracts.iter().skip(2) {
            contract_queries::update_contract_status(
                conn,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato excedente '{}': {e}",
                    contract.id
                )
            })?;
        }

        team_queries::update_team_pilots(conn, &team.id, piloto_1, piloto_2).map_err(|e| {
            format!(
                "Falha ao sincronizar pilotos da equipe '{}': {e}",
                team.nome
            )
        })?;
    }

    for contracts in valid_contracts.values() {
        for contract in contracts {
            contract_queries::update_contract_status(
                conn,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato fora da grade sincronizada '{}': {e}",
                    contract.id
                )
            })?;
        }
    }

    conn.execute(
        "UPDATE drivers SET categoria_atual = NULL
         WHERE categoria_atual IS NOT NULL
         AND id NOT IN (
             SELECT piloto_id FROM contracts
             WHERE status = 'Ativo' AND tipo = 'Regular'
         )",
        [],
    )
    .map_err(|e| format!("Falha ao limpar categoria_atual de pilotos sem contrato: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use crate::constants::teams::get_reference_team_template;
    use crate::db::migrations;
    use crate::db::queries::drivers as driver_queries;
    use crate::models::enums::TeamRole;
    use crate::models::team::Team;

    fn banco() -> Connection {
        let conn = Connection::open_in_memory().expect("banco em memoria");
        migrations::run_all(&conn).expect("schema");
        conn
    }

    fn equipe(categoria: &str, id: &str, semente: u64) -> Team {
        let template = get_reference_team_template(categoria, None).expect("template de equipe");
        let mut rng = StdRng::seed_from_u64(semente);
        Team::from_template_with_rng(template, categoria, id.to_string(), 2025, &mut rng)
    }

    fn piloto(id: &str, skill: f64) -> Driver {
        let mut driver = Driver::new(
            id.to_string(),
            format!("Piloto {id}"),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        driver.atributos.skill = skill;
        driver
    }

    fn contrato(id: &str, driver: &Driver, team: &Team, papel: TeamRole) -> Contract {
        let mut contract = Contract::new(
            id.to_string(),
            driver.id.clone(),
            driver.nome.clone(),
            team.id.clone(),
            team.nome.clone(),
            2025,
            1,
            100_000.0,
            papel,
            team.categoria.clone(),
        );
        contract.classe = team.classe.clone();
        contract
    }

    /// Grava pilotos e contratos e devolve o mapa por id que a sincronização consome.
    fn semear(
        conn: &Connection,
        drivers: &[Driver],
        contracts: &[Contract],
    ) -> HashMap<String, Driver> {
        for driver in drivers {
            driver_queries::insert_driver(conn, driver).expect("insere piloto");
        }
        for contract in contracts {
            contract_queries::insert_contract(conn, contract).expect("insere contrato");
        }
        drivers
            .iter()
            .cloned()
            .map(|driver| (driver.id.clone(), driver))
            .collect()
    }

    fn status_do_contrato(conn: &Connection, id: &str) -> String {
        conn.query_row("SELECT status FROM contracts WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .expect("status do contrato")
    }

    fn categoria_atual(conn: &Connection, piloto_id: &str) -> Option<String> {
        conn.query_row(
            "SELECT categoria_atual FROM drivers WHERE id = ?1",
            [piloto_id],
            |row| row.get(0),
        )
        .expect("categoria atual")
    }

    #[test]
    fn o_mais_habilidoso_vira_n1_e_os_dois_recebem_a_categoria() {
        let conn = banco();
        let team = equipe("gt3", "T1", 1);
        team_queries::insert_team(&conn, &team).expect("insere equipe");

        // Ordem de INSERÇÃO invertida de propósito: quem manda é a skill, não a ordem.
        let fraco = piloto("P_FRACO", 55.0);
        let forte = piloto("P_FORTE", 82.0);
        let drivers = semear(
            &conn,
            &[fraco.clone(), forte.clone()],
            &[
                contrato("C1", &fraco, &team, TeamRole::Numero1),
                contrato("C2", &forte, &team, TeamRole::Numero2),
            ],
        );

        sync_team_slots_from_active_regular_contracts(&conn, &[team.clone()], &drivers)
            .expect("sincroniza");

        let atualizada = team_queries::get_team_by_id(&conn, &team.id)
            .expect("le equipe")
            .expect("equipe existe");
        assert_eq!(atualizada.piloto_1_id.as_deref(), Some("P_FORTE"));
        assert_eq!(atualizada.piloto_2_id.as_deref(), Some("P_FRACO"));
        assert_eq!(categoria_atual(&conn, "P_FORTE").as_deref(), Some("gt3"));
        assert_eq!(categoria_atual(&conn, "P_FRACO").as_deref(), Some("gt3"));
    }

    #[test]
    fn contrato_de_aposentado_e_rescindido_e_o_assento_fica_aberto() {
        let conn = banco();
        let team = equipe("gt3", "T1", 2);
        team_queries::insert_team(&conn, &team).expect("insere equipe");

        let mut aposentado = piloto("P_APO", 90.0);
        aposentado.status = DriverStatus::Aposentado;
        let ativo = piloto("P_ATIVO", 60.0);
        let drivers = semear(
            &conn,
            &[aposentado.clone(), ativo.clone()],
            &[
                contrato("C1", &aposentado, &team, TeamRole::Numero1),
                contrato("C2", &ativo, &team, TeamRole::Numero2),
            ],
        );

        sync_team_slots_from_active_regular_contracts(&conn, &[team.clone()], &drivers)
            .expect("sincroniza");

        assert_eq!(status_do_contrato(&conn, "C1"), "Rescindido");
        let atualizada = team_queries::get_team_by_id(&conn, &team.id)
            .expect("le equipe")
            .expect("equipe existe");
        // O aposentado tinha a MAIOR skill: sem a rescisão ele seria o N1.
        assert_eq!(atualizada.piloto_1_id.as_deref(), Some("P_ATIVO"));
        assert_eq!(atualizada.piloto_2_id, None, "sobra vaga de N2 aberta");
    }

    #[test]
    fn contrato_de_outra_categoria_nao_ocupa_o_assento() {
        let conn = banco();
        let team = equipe("gt3", "T1", 3);
        team_queries::insert_team(&conn, &team).expect("insere equipe");

        let intruso = piloto("P_INTRUSO", 88.0);
        let legitimo = piloto("P_LEGIT", 60.0);
        let mut fora_da_categoria = contrato("C1", &intruso, &team, TeamRole::Numero1);
        fora_da_categoria.categoria = "gt4".to_string();
        let drivers = semear(
            &conn,
            &[intruso.clone(), legitimo.clone()],
            &[
                fora_da_categoria,
                contrato("C2", &legitimo, &team, TeamRole::Numero2),
            ],
        );

        sync_team_slots_from_active_regular_contracts(&conn, &[team.clone()], &drivers)
            .expect("sincroniza");

        assert_eq!(status_do_contrato(&conn, "C1"), "Rescindido");
        let atualizada = team_queries::get_team_by_id(&conn, &team.id)
            .expect("le equipe")
            .expect("equipe existe");
        assert_eq!(atualizada.piloto_1_id.as_deref(), Some("P_LEGIT"));
        assert_eq!(atualizada.piloto_2_id, None);
    }

    #[test]
    fn terceiro_contrato_na_mesma_equipe_e_rescindido_pelo_pior() {
        let conn = banco();
        let team = equipe("gt3", "T1", 4);
        team_queries::insert_team(&conn, &team).expect("insere equipe");

        let a = piloto("P_A", 80.0);
        let b = piloto("P_B", 70.0);
        let c = piloto("P_C", 60.0);
        let drivers = semear(
            &conn,
            &[a.clone(), b.clone(), c.clone()],
            &[
                contrato("C1", &a, &team, TeamRole::Numero1),
                contrato("C2", &b, &team, TeamRole::Numero2),
                contrato("C3", &c, &team, TeamRole::Numero2),
            ],
        );

        sync_team_slots_from_active_regular_contracts(&conn, &[team.clone()], &drivers)
            .expect("sincroniza");

        assert_eq!(status_do_contrato(&conn, "C1"), "Ativo");
        assert_eq!(status_do_contrato(&conn, "C2"), "Ativo");
        assert_eq!(
            status_do_contrato(&conn, "C3"),
            "Rescindido",
            "o excedente é o de menor skill"
        );
        assert_eq!(categoria_atual(&conn, "P_C"), None);
    }

    #[test]
    fn contrato_de_equipe_fora_da_grade_sincronizada_e_rescindido() {
        let conn = banco();
        let team = equipe("gt3", "T1", 5);
        let fantasma = equipe("gt3", "T_FANTASMA", 6);
        team_queries::insert_team(&conn, &team).expect("insere equipe");
        team_queries::insert_team(&conn, &fantasma).expect("insere equipe fantasma");

        let orfao = piloto("P_ORFAO", 70.0);
        let drivers = semear(
            &conn,
            &[orfao.clone()],
            &[contrato("C1", &orfao, &fantasma, TeamRole::Numero1)],
        );

        // A equipe fantasma NÃO entra na lista sincronizada.
        sync_team_slots_from_active_regular_contracts(&conn, &[team], &drivers)
            .expect("sincroniza");

        assert_eq!(status_do_contrato(&conn, "C1"), "Rescindido");
        assert_eq!(categoria_atual(&conn, "P_ORFAO"), None);
    }

    #[test]
    fn piloto_sem_contrato_ativo_perde_a_categoria_atual() {
        let conn = banco();
        let team = equipe("gt3", "T1", 7);
        team_queries::insert_team(&conn, &team).expect("insere equipe");

        let mut livre = piloto("P_LIVRE", 70.0);
        livre.categoria_atual = Some("gt3".to_string()); // resquício da temporada passada
        let drivers = semear(&conn, &[livre.clone()], &[]);

        sync_team_slots_from_active_regular_contracts(&conn, &[team], &drivers)
            .expect("sincroniza");

        assert_eq!(categoria_atual(&conn, "P_LIVRE"), None);
    }

    #[test]
    fn contrato_apontando_para_piloto_inexistente_falha_alto() {
        let conn = banco();
        let team = equipe("gt3", "T1", 8);
        team_queries::insert_team(&conn, &team).expect("insere equipe");

        let sumido = piloto("P_SUMIDO", 70.0);
        semear(
            &conn,
            &[sumido.clone()],
            &[contrato("C1", &sumido, &team, TeamRole::Numero1)],
        );

        // O piloto EXISTE no banco mas ficou de fora do mapa que o chamador montou (foto
        // defasada). Seguir em frente escreveria um assento apontando para ninguém, e o
        // erro só apareceria muitos passos adiante — daí a falha ser aqui e alta.
        let erro = sync_team_slots_from_active_regular_contracts(&conn, &[team], &HashMap::new())
            .expect_err("contrato orfao deve falhar");
        assert!(erro.contains("P_SUMIDO"), "{erro}");
    }
}
