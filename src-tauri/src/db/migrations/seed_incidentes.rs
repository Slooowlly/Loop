//! Catálogo de incidentes de corrida — dados de seed da tabela `incident_catalog`.
//!
//! Extraído da migração v14 original. É puro dado de **identidade**: 54 entradas com id,
//! filtros de elegibilidade e peso. Nenhuma frase mora aqui. O texto de cada entrada vive
//! em `locales/*.yml`, sob `breakdown.<id>.{dnf|warn|part}`, e é resolvido no locale ATIVO
//! na hora de apresentar (`simulation::catalog`).
//!
//! ## As chaves são derivadas, não digitadas
//!
//! Uma entrada não escolhe suas chaves: elas saem de `(id, severity_context)` por regra
//! fixa, em [`chaves_de_texto`]. A regra foi conferida entrada por entrada contra o seed
//! antigo, que guardava a prosa, e bate nos 54 casos sem exceção:
//!
//! - entrada `NonDnfOnly` era exatamente a que tinha `dnf_template` vazio;
//! - entrada `DnfOnly` era exatamente a que tinha `non_dnf_template` nulo.
//!
//! Isso não é coincidência e sim consequência do filtro: `severity_matches` nunca oferece
//! uma entrada `NonDnfOnly` a um desfecho de DNF, então o texto de DNF dela seria letra
//! morta. A guarda `a_presenca_de_chave_segue_o_severity_context` prende a regra.
//!
//! A chave é **escrita no banco** em vez de derivada na leitura de propósito: a tabela fica
//! legível sozinha, e a migração de save antigo tem para onde escrever.

use rusqlite::Connection;

use crate::db::connection::DbError;

/// Uma linha do catálogo: só identidade, filtro e peso.
///
/// Ordem: `(id, vehicle_class, race_format, incident_source, trigger_type,
/// severity_context, weight_sprint, weight_endurance)`.
pub(crate) type EntradaDoCatalogo = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    i64,
    i64,
);

/// As três chaves de texto de uma entrada, derivadas de `(id, severity_context)`.
///
/// Devolve `(dnf_key, non_dnf_key, description_key)`. A chave de DNF vem **vazia** quando a
/// entrada é `NonDnfOnly` e a de não-DNF vem `None` quando é `DnfOnly` — os dois casos em
/// que o filtro do catálogo nunca chega a pedir aquele texto. String vazia em vez de `NULL`
/// no primeiro caso porque a coluna nasceu `NOT NULL` na baseline v53, que é imutável.
pub(crate) fn chaves_de_texto(
    id: &str,
    severity_context: &str,
) -> (String, Option<String>, String) {
    let dnf = if severity_context == "NonDnfOnly" {
        String::new()
    } else {
        format!("breakdown.{id}.dnf")
    };
    let non_dnf = if severity_context == "DnfOnly" {
        None
    } else {
        Some(format!("breakdown.{id}.warn"))
    };
    (dnf, non_dnf, format!("breakdown.{id}.part"))
}

/// As 54 entradas do catálogo. Mexer aqui muda a incidência dos eventos na corrida.
pub(crate) const ENTRADAS: &[EntradaDoCatalogo] = &[
    // ═══ STREETBASED SPRINT MECHANICAL SPONTANEOUS ═══
    (
        "SB_S_MEC_01",
        "StreetBased",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        100,
        0,
    ),
    (
        "SB_S_MEC_02",
        "StreetBased",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        70,
        0,
    ),
    (
        "SB_S_MEC_03",
        "StreetBased",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        40,
        0,
    ),
    (
        "SB_S_MEC_04",
        "StreetBased",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        20,
        0,
    ),
    (
        "SB_S_MEC_05",
        "StreetBased",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        10,
        0,
    ),
    // ═══ STREETBASED ENDURANCE MECHANICAL SPONTANEOUS ═══
    (
        "SB_E_MEC_01",
        "StreetBased",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        100,
    ),
    (
        "SB_E_MEC_02",
        "StreetBased",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        70,
    ),
    (
        "SB_E_MEC_03",
        "StreetBased",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        70,
    ),
    (
        "SB_E_MEC_04",
        "StreetBased",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        50,
    ),
    (
        "SB_E_MEC_05",
        "StreetBased",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        40,
    ),
    (
        "SB_E_MEC_06",
        "StreetBased",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        40,
    ),
    (
        "SB_E_MEC_07",
        "StreetBased",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        20,
    ),
    (
        "SB_E_MEC_08",
        "StreetBased",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        10,
    ),
    // ═══ RACESPEC SPRINT MECHANICAL SPONTANEOUS ═══
    (
        "RS_S_MEC_01",
        "RaceSpec",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        100,
        0,
    ),
    (
        "RS_S_MEC_02",
        "RaceSpec",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        70,
        0,
    ),
    (
        "RS_S_MEC_03",
        "RaceSpec",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        40,
        0,
    ),
    (
        "RS_S_MEC_04",
        "RaceSpec",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        20,
        0,
    ),
    (
        "RS_S_MEC_05",
        "RaceSpec",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        10,
        0,
    ),
    // ═══ RACESPEC ENDURANCE MECHANICAL SPONTANEOUS ═══
    (
        "RS_E_MEC_01",
        "RaceSpec",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        100,
    ),
    (
        "RS_E_MEC_02",
        "RaceSpec",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        70,
    ),
    (
        "RS_E_MEC_03",
        "RaceSpec",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        60,
    ),
    (
        "RS_E_MEC_04",
        "RaceSpec",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        50,
    ),
    (
        "RS_E_MEC_05",
        "RaceSpec",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        40,
    ),
    (
        "RS_E_MEC_06",
        "RaceSpec",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        40,
    ),
    (
        "RS_E_MEC_07",
        "RaceSpec",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        20,
    ),
    (
        "RS_E_MEC_08",
        "RaceSpec",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        20,
    ),
    (
        "RS_E_MEC_09",
        "RaceSpec",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        10,
    ),
    // ═══ ERRO DE COMBUSTÍVEL ENDURANCE (Mechanical/Spontaneous) ═══
    // Resolução 1: usa Mechanical para ser selecionado pelo roll_mechanical existente.
    (
        "SB_E_PIT_02",
        "StreetBased",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "DnfOnly",
        0,
        30,
    ),
    (
        "RS_E_PIT_02",
        "RaceSpec",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "DnfOnly",
        0,
        30,
    ),
    // ═══ STREETBASED POST-COLLISION (Both formats) ═══
    (
        "SB_COL_01",
        "StreetBased",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        100,
        100,
    ),
    (
        "SB_COL_02",
        "StreetBased",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        70,
        70,
    ),
    (
        "SB_COL_03",
        "StreetBased",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        40,
        70,
    ),
    (
        "SB_COL_04",
        "StreetBased",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        40,
        40,
    ),
    (
        "SB_COL_05",
        "StreetBased",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        10,
        40,
    ),
    // ═══ RACESPEC POST-COLLISION (Both formats) ═══
    (
        "RS_COL_01",
        "RaceSpec",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        100,
        100,
    ),
    (
        "RS_COL_02",
        "RaceSpec",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        100,
        100,
    ),
    (
        "RS_COL_03",
        "RaceSpec",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        70,
        70,
    ),
    (
        "RS_COL_04",
        "RaceSpec",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        70,
        100,
    ),
    (
        "RS_COL_05",
        "RaceSpec",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        40,
        70,
    ),
    (
        "RS_COL_06",
        "RaceSpec",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        40,
        40,
    ),
    (
        "RS_COL_07",
        "RaceSpec",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        20,
        40,
    ),
    (
        "RS_COL_08",
        "RaceSpec",
        "Endurance",
        "PostCollision",
        "PostCollision",
        "Both",
        0,
        40,
    ),
    // ═══ STREETBASED DRIVER ERROR SPONTANEOUS ═══
    (
        "SB_S_ERR_01",
        "StreetBased",
        "Both",
        "DriverError",
        "Spontaneous",
        "NonDnfOnly",
        80,
        60,
    ),
    (
        "SB_S_ERR_02",
        "StreetBased",
        "Both",
        "DriverError",
        "Spontaneous",
        "NonDnfOnly",
        70,
        50,
    ),
    (
        "SB_S_ERR_03",
        "StreetBased",
        "Both",
        "DriverError",
        "Spontaneous",
        "DnfOnly",
        40,
        30,
    ),
    // ═══ RACESPEC DRIVER ERROR SPONTANEOUS ═══
    (
        "RS_S_ERR_01",
        "RaceSpec",
        "Both",
        "DriverError",
        "Spontaneous",
        "NonDnfOnly",
        80,
        60,
    ),
    (
        "RS_S_ERR_02",
        "RaceSpec",
        "Both",
        "DriverError",
        "Spontaneous",
        "NonDnfOnly",
        70,
        50,
    ),
    (
        "RS_S_ERR_03",
        "RaceSpec",
        "Both",
        "DriverError",
        "Spontaneous",
        "DnfOnly",
        40,
        30,
    ),
    // ═══ OPERATIONAL POST SPIN STALL ═══
    (
        "SB_S_PIT_01",
        "StreetBased",
        "Sprint",
        "Operational",
        "PostSpinStall",
        "DnfOnly",
        40,
        0,
    ),
    (
        "SB_S_PIT_02",
        "StreetBased",
        "Sprint",
        "Operational",
        "PostSpinStall",
        "DnfOnly",
        60,
        0,
    ),
    (
        "SB_E_PIT_01",
        "StreetBased",
        "Endurance",
        "Operational",
        "PostSpinStall",
        "DnfOnly",
        0,
        40,
    ),
    (
        "RS_S_PIT_01",
        "RaceSpec",
        "Sprint",
        "Operational",
        "PostSpinStall",
        "DnfOnly",
        40,
        0,
    ),
    (
        "RS_S_PIT_02",
        "RaceSpec",
        "Sprint",
        "Operational",
        "PostSpinStall",
        "DnfOnly",
        60,
        0,
    ),
    (
        "RS_E_PIT_01",
        "RaceSpec",
        "Endurance",
        "Operational",
        "PostSpinStall",
        "DnfOnly",
        0,
        40,
    ),
];

/// As entradas de **PROTÓTIPO** (o grid de LMP2), acrescentadas em 12/08/2026.
///
/// Lista SEPARADA de [`ENTRADAS`], e não mais 16 linhas dentro dela, por uma razão prática: a
/// baseline v53 semeia `ENTRADAS` num banco novo, mas um save que já passou pela v53 nunca
/// mais roda aquele seed. Quem leva estas linhas ao save antigo é a migração v67, e migração
/// lançada é imutável — ela precisa de uma lista que não cresça debaixo dela. Entrada de
/// protótipo nova entra numa migração nova, como qualquer outra.
///
/// ## Por que elas não existiam
///
/// O catálogo nasceu com duas classes, `StreetBased` e `RaceSpec`, e `vehicle_class_from_category`
/// tinha um braço final `_ => StreetBased` que fazia o LMP2 receber flavor text de MX-5. Em
/// 12/08/2026 esse braço virou recusa e o LMP2 passou a resolver como `Prototype` — correto, e
/// sem uma única linha no catálogo para casar. O incidente passou a sair com o texto genérico do
/// motor ("{driver} abandona por problema mecânico"), que é honesto e mudo. Estas entradas são a
/// outra metade do conserto.
///
/// A cobertura acompanha as combinações que o motor de fato consulta (ver
/// `simulation::incidents::segmento`): mecânica espontânea em sprint e em enduro, erro de piloto,
/// pós-colisão e o stall depois da rodada.
pub(crate) const ENTRADAS_PROTOTIPO: &[EntradaDoCatalogo] = &[
    // ═══ PROTOTYPE SPRINT MECHANICAL SPONTANEOUS ═══
    (
        "PR_S_MEC_01",
        "Prototype",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        100,
        0,
    ),
    (
        "PR_S_MEC_02",
        "Prototype",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        70,
        0,
    ),
    (
        "PR_S_MEC_03",
        "Prototype",
        "Sprint",
        "Mechanical",
        "Spontaneous",
        "Both",
        40,
        0,
    ),
    // ═══ PROTOTYPE ENDURANCE MECHANICAL SPONTANEOUS ═══
    (
        "PR_E_MEC_01",
        "Prototype",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        100,
    ),
    (
        "PR_E_MEC_02",
        "Prototype",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        70,
    ),
    (
        "PR_E_MEC_03",
        "Prototype",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        50,
    ),
    (
        "PR_E_MEC_04",
        "Prototype",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "Both",
        0,
        30,
    ),
    // ═══ ERRO DE COMBUSTÍVEL NO ENDURO (Mechanical/Spontaneous, DnfOnly) ═══
    (
        "PR_E_PIT_02",
        "Prototype",
        "Endurance",
        "Mechanical",
        "Spontaneous",
        "DnfOnly",
        0,
        30,
    ),
    // ═══ PROTOTYPE POST-COLLISION (os dois formatos) ═══
    (
        "PR_COL_01",
        "Prototype",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        100,
        100,
    ),
    (
        "PR_COL_02",
        "Prototype",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        70,
        70,
    ),
    (
        "PR_COL_03",
        "Prototype",
        "Both",
        "PostCollision",
        "PostCollision",
        "Both",
        40,
        70,
    ),
    // ═══ PROTOTYPE DRIVER ERROR SPONTANEOUS ═══
    (
        "PR_S_ERR_01",
        "Prototype",
        "Both",
        "DriverError",
        "Spontaneous",
        "NonDnfOnly",
        80,
        60,
    ),
    (
        "PR_S_ERR_02",
        "Prototype",
        "Both",
        "DriverError",
        "Spontaneous",
        "NonDnfOnly",
        70,
        50,
    ),
    (
        "PR_S_ERR_03",
        "Prototype",
        "Both",
        "DriverError",
        "Spontaneous",
        "DnfOnly",
        40,
        30,
    ),
    // ═══ OPERATIONAL POST SPIN STALL ═══
    (
        "PR_S_PIT_01",
        "Prototype",
        "Sprint",
        "Operational",
        "PostSpinStall",
        "DnfOnly",
        40,
        0,
    ),
    (
        "PR_E_PIT_01",
        "Prototype",
        "Endurance",
        "Operational",
        "PostSpinStall",
        "DnfOnly",
        0,
        40,
    ),
];

/// TODA entrada do catálogo, das duas listas. É por aqui que as guardas caminham — uma lista
/// esquecida é exatamente o buraco que o protótipo teve.
pub(crate) fn todas_as_entradas() -> impl Iterator<Item = &'static EntradaDoCatalogo> {
    ENTRADAS.iter().chain(ENTRADAS_PROTOTIPO.iter())
}

/// Semeia as entradas de PROTÓTIPO no schema **pós-v65**, em que as colunas de texto já se
/// chamam `dnf_key`/`non_dnf_key`/`description_key`. É o que a migração v67 executa num save
/// que já existia; banco novo recebe as mesmas linhas por [`seed_incident_catalog`].
pub(crate) fn seed_prototipos(conn: &Connection) -> Result<(), DbError> {
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO incident_catalog
         (id, vehicle_class, race_format, incident_source, trigger_type,
          severity_context, weight_sprint, weight_endurance,
          dnf_key, non_dnf_key, description_key)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
    )?;
    for e in ENTRADAS_PROTOTIPO {
        let (dnf_key, non_dnf_key, description_key) = chaves_de_texto(e.0, e.5);
        stmt.execute(rusqlite::params![
            e.0,
            e.1,
            e.2,
            e.3,
            e.4,
            e.5,
            e.6,
            e.7,
            dnf_key,
            non_dnf_key,
            description_key
        ])?;
    }
    Ok(())
}

pub fn seed_incident_catalog(conn: &Connection) -> Result<(), DbError> {
    // INSERT OR IGNORE para idempotência.
    //
    // As colunas ainda têm o NOME antigo (`dnf_template` e companhia) porque quem cria a
    // tabela é a baseline v53, e migração lançada é imutável. O que entra nelas já é
    // CHAVE, e a v65 renomeia as três para `dnf_key`/`non_dnf_key`/`description_key`. Os
    // dois caminhos — banco novo pela baseline e save antigo pela v65 — terminam no mesmo
    // schema e no mesmo conteúdo.
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO incident_catalog
         (id, vehicle_class, race_format, incident_source, trigger_type,
          severity_context, weight_sprint, weight_endurance,
          dnf_template, non_dnf_template, description_short)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
    )?;

    // Só [`ENTRADAS`]. As de protótipo NÃO entram por aqui, e é de propósito: elas nascem na
    // v67, depois da v65 — que renomeia as colunas e trata como "texto legado" todo id que não
    // conhece. Semeadas na baseline, elas passariam pela v65 antes de existir para ela, e a
    // própria chave seria arquivada como prosa preservada. Banco novo recebe as 16 pela v67,
    // igualzinho a um save antigo.
    for e in ENTRADAS {
        let (dnf_key, non_dnf_key, description_key) = chaves_de_texto(e.0, e.5);
        stmt.execute(rusqlite::params![
            e.0,
            e.1,
            e.2,
            e.3,
            e.4,
            e.5,
            e.6,
            e.7,
            dnf_key,
            non_dnf_key,
            description_key
        ])?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O catálogo tem 54 entradas na lista original, 16 de protótipo, e nenhum id repetido
    /// entre as duas. O número está em prosa em vários lugares do projeto; aqui ele é medido.
    #[test]
    fn o_catalogo_tem_os_ids_unicos_das_duas_listas() {
        assert_eq!(ENTRADAS.len(), 54, "a lista da baseline mudou de tamanho");
        assert_eq!(
            ENTRADAS_PROTOTIPO.len(),
            16,
            "a lista da v67 mudou de tamanho"
        );
        let mut ids: Vec<&str> = todas_as_entradas().map(|e| e.0).collect();
        ids.sort_unstable();
        let antes = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), antes, "id repetido no catálogo");
    }

    /// **Toda classe de veículo tem entrada.** O protótipo passou a existir em
    /// `vehicle_class_from_category` antes de existir no catálogo, e o sintoma era mudo: o
    /// incidente caía no texto genérico do motor. Uma classe nova sem linha aqui repete isso.
    #[test]
    fn toda_classe_de_veiculo_tem_entrada_no_catalogo() {
        for classe in ["StreetBased", "RaceSpec", "Prototype"] {
            assert!(
                todas_as_entradas().any(|e| e.1 == classe),
                "a classe '{classe}' resolve em vehicle_class_from_category e não tem nenhuma \
                 entrada no catálogo — o incidente sairia com o texto genérico do motor"
            );
        }
    }

    /// **O protótipo cobre as combinações que o motor consulta.** Faltando uma, aquele caminho
    /// volta ao texto genérico só para o LMP2, e nada acusa.
    #[test]
    fn o_prototipo_cobre_as_combinacoes_que_o_motor_pede() {
        // (incident_source, trigger_type, precisa de texto de DNF, precisa de texto de não-DNF)
        let pedidos: &[(&str, &str, bool, bool)] = &[
            ("Mechanical", "Spontaneous", true, true),
            ("DriverError", "Spontaneous", true, true),
            ("Operational", "PostSpinStall", true, false),
            ("PostCollision", "PostCollision", false, true),
        ];
        for (fonte, gatilho, quer_dnf, quer_nao_dnf) in pedidos {
            let candidatas: Vec<_> = ENTRADAS_PROTOTIPO
                .iter()
                .filter(|e| e.3 == *fonte && e.4 == *gatilho)
                .collect();
            assert!(
                !candidatas.is_empty(),
                "protótipo sem entrada para {fonte}/{gatilho}"
            );
            if *quer_dnf {
                assert!(
                    candidatas.iter().any(|e| e.5 != "NonDnfOnly"),
                    "protótipo sem desfecho de DNF em {fonte}/{gatilho}"
                );
            }
            if *quer_nao_dnf {
                assert!(
                    candidatas.iter().any(|e| e.5 != "DnfOnly"),
                    "protótipo sem desfecho de não-DNF em {fonte}/{gatilho}"
                );
            }
            // E os DOIS formatos: peso zero em todos os candidatos de um deles deixa o
            // sprint (ou o enduro) sem candidato nenhum, e o texto genérico volta só ali.
            assert!(
                candidatas.iter().any(|e| e.6 > 0),
                "protótipo sem peso de SPRINT em {fonte}/{gatilho}"
            );
            assert!(
                candidatas.iter().any(|e| e.7 > 0),
                "protótipo sem peso de ENDURO em {fonte}/{gatilho}"
            );
        }
    }

    /// **A presença de cada chave segue o `severity_context`.** É a regra que substituiu a
    /// prosa no banco, e ela só é segura porque o filtro do catálogo nunca pede o texto que
    /// falta: `NonDnfOnly` não é oferecida a um desfecho de DNF, `DnfOnly` não é oferecida a
    /// um desfecho de não-DNF.
    #[test]
    fn a_presenca_de_chave_segue_o_severity_context() {
        for e in todas_as_entradas() {
            let (dnf, non_dnf, curta) = chaves_de_texto(e.0, e.5);
            assert_eq!(
                dnf.is_empty(),
                e.5 == "NonDnfOnly",
                "chave de DNF em {} não bate com severity {}",
                e.0,
                e.5
            );
            assert_eq!(
                non_dnf.is_none(),
                e.5 == "DnfOnly",
                "chave de não-DNF em {} não bate com severity {}",
                e.0,
                e.5
            );
            assert_eq!(curta, format!("breakdown.{}.part", e.0));
        }
    }

    /// Nenhuma frase de apresentação sobrou no seed. A busca é por texto do próprio arquivo
    /// porque o que se quer impedir é a REGRESSÃO: alguém acrescentar uma entrada nova
    /// colando a frase junto, como era antes da v65.
    #[test]
    fn o_seed_nao_carrega_prosa() {
        let fonte = include_str!("seed_incidentes.rs");
        // As DUAS listas: o corpo de cada array vai da declaração até o `];`.
        for decl in [
            "pub(crate) const ENTRADAS: ",
            "pub(crate) const ENTRADAS_PROTOTIPO: ",
        ] {
            let inicio = fonte
                .find(decl)
                .unwrap_or_else(|| panic!("declaração {decl}"));
            let corpo = &fonte[inicio..];
            let fim = corpo.find("\n];").expect("fim do array");
            let corpo = &corpo[..fim];

            for literal in corpo.split('"').skip(1).step_by(2) {
                assert!(
                    !literal.contains(' ') && !literal.contains('{'),
                    "literal com cara de frase dentro de {decl}: {literal:?}. \
                     O texto do catálogo vive em locales/*.yml sob breakdown.<id>."
                );
            }
        }
    }
}
