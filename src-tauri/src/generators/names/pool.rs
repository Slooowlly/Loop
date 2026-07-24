//! A tabela de pools de nomes por nacionalidade e seus acessores.

use super::dados::*;

#[derive(Debug, Clone)]
pub struct NamePool {
    pub nationality_id: &'static str,
    pub nomes_masculinos: &'static [&'static str],
    pub nomes_femininos: &'static [&'static str],
    pub sobrenomes: &'static [&'static str],
}

pub(crate) static NAME_POOLS: [NamePool; 23] = [
    NamePool {
        nationality_id: "gb",
        nomes_masculinos: GB_MALE,
        nomes_femininos: GB_FEMALE,
        sobrenomes: GB_LAST,
    },
    NamePool {
        nationality_id: "de",
        nomes_masculinos: DE_MALE,
        nomes_femininos: DE_FEMALE,
        sobrenomes: DE_LAST,
    },
    NamePool {
        nationality_id: "fr",
        nomes_masculinos: FR_MALE,
        nomes_femininos: FR_FEMALE,
        sobrenomes: FR_LAST,
    },
    NamePool {
        nationality_id: "it",
        nomes_masculinos: IT_MALE,
        nomes_femininos: IT_FEMALE,
        sobrenomes: IT_LAST,
    },
    NamePool {
        nationality_id: "es",
        nomes_masculinos: ES_MALE,
        nomes_femininos: ES_FEMALE,
        sobrenomes: ES_LAST,
    },
    NamePool {
        nationality_id: "br",
        nomes_masculinos: BR_MALE,
        nomes_femininos: BR_FEMALE,
        sobrenomes: BR_LAST,
    },
    NamePool {
        nationality_id: "nl",
        nomes_masculinos: NL_MALE,
        nomes_femininos: NL_FEMALE,
        sobrenomes: NL_LAST,
    },
    NamePool {
        nationality_id: "au",
        nomes_masculinos: AU_MALE,
        nomes_femininos: AU_FEMALE,
        sobrenomes: AU_LAST,
    },
    NamePool {
        nationality_id: "jp",
        nomes_masculinos: JP_MALE,
        nomes_femininos: JP_FEMALE,
        sobrenomes: JP_LAST,
    },
    NamePool {
        nationality_id: "us",
        nomes_masculinos: US_MALE,
        nomes_femininos: US_FEMALE,
        sobrenomes: US_LAST,
    },
    NamePool {
        nationality_id: "mx",
        nomes_masculinos: MX_MALE,
        nomes_femininos: MX_FEMALE,
        sobrenomes: MX_LAST,
    },
    NamePool {
        nationality_id: "ar",
        nomes_masculinos: AR_MALE,
        nomes_femininos: AR_FEMALE,
        sobrenomes: AR_LAST,
    },
    NamePool {
        nationality_id: "fi",
        nomes_masculinos: FI_MALE,
        nomes_femininos: FI_FEMALE,
        sobrenomes: FI_LAST,
    },
    NamePool {
        nationality_id: "be",
        nomes_masculinos: BE_MALE,
        nomes_femininos: BE_FEMALE,
        sobrenomes: BE_LAST,
    },
    NamePool {
        nationality_id: "pt",
        nomes_masculinos: PT_MALE,
        nomes_femininos: PT_FEMALE,
        sobrenomes: PT_LAST,
    },
    NamePool {
        nationality_id: "ca",
        nomes_masculinos: CA_MALE,
        nomes_femininos: CA_FEMALE,
        sobrenomes: CA_LAST,
    },
    NamePool {
        nationality_id: "at",
        nomes_masculinos: AT_MALE,
        nomes_femininos: AT_FEMALE,
        sobrenomes: AT_LAST,
    },
    NamePool {
        nationality_id: "ch",
        nomes_masculinos: CH_MALE,
        nomes_femininos: CH_FEMALE,
        sobrenomes: CH_LAST,
    },
    NamePool {
        nationality_id: "dk",
        nomes_masculinos: DK_MALE,
        nomes_femininos: DK_FEMALE,
        sobrenomes: DK_LAST,
    },
    NamePool {
        nationality_id: "se",
        nomes_masculinos: SE_MALE,
        nomes_femininos: SE_FEMALE,
        sobrenomes: SE_LAST,
    },
    NamePool {
        nationality_id: "no",
        nomes_masculinos: NO_MALE,
        nomes_femininos: NO_FEMALE,
        sobrenomes: NO_LAST,
    },
    NamePool {
        nationality_id: "pl",
        nomes_masculinos: PL_MALE,
        nomes_femininos: PL_FEMALE,
        sobrenomes: PL_LAST,
    },
    NamePool {
        nationality_id: "cn",
        nomes_masculinos: CN_MALE,
        nomes_femininos: CN_FEMALE,
        sobrenomes: CN_LAST,
    },
];

pub fn get_all_name_pools() -> &'static [NamePool] {
    &NAME_POOLS
}

pub fn get_name_pool(nationality_id: &str) -> Option<&'static NamePool> {
    NAME_POOLS
        .iter()
        .find(|pool| pool.nationality_id == nationality_id)
}
