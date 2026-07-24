//! Tipos compartilhados da geracao de mundo: os pacotes de saida, o alocador
//! local de ids e o ajuste de classe nos contratos.

use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::team::Team;

#[derive(Debug, Clone)]
pub struct WorldData {
    pub drivers: Vec<Driver>,
    pub teams: Vec<Team>,
    pub contracts: Vec<Contract>,
    pub player: Driver,
    pub player_team_id: String,
    pub player_contract: Contract,
}

#[derive(Debug, Clone)]
pub struct HistoricalWorldData {
    pub drivers: Vec<Driver>,
    pub teams: Vec<Team>,
    pub contracts: Vec<Contract>,
}

#[derive(Debug, Default)]
pub(crate) struct LocalIdAllocator {
    next_driver: u32,
    next_team: u32,
    next_contract: u32,
}

impl LocalIdAllocator {
    pub(crate) fn new() -> Self {
        Self {
            next_driver: 1,
            next_team: 1,
            next_contract: 1,
        }
    }

    pub(crate) fn next_driver_id(&mut self) -> String {
        let id = format!("P{:03}", self.next_driver);
        self.next_driver += 1;
        id
    }

    pub(crate) fn next_team_id(&mut self) -> String {
        let id = format!("T{:03}", self.next_team);
        self.next_team += 1;
        id
    }

    pub(crate) fn next_contract_id(&mut self) -> String {
        let id = format!("C{:03}", self.next_contract);
        self.next_contract += 1;
        id
    }
}

pub(crate) fn contract_with_team_class(mut contract: Contract, team: &Team) -> Contract {
    contract.classe = team.classe.clone();
    contract
}
