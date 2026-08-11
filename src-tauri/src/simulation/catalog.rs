//! **Catálogo de incidentes** — o texto que dá cara ao que quebrou no carro.
//!
//! O texto de um incidente é resolvido POR ID no locale ativo (`breakdown.<id>.{dnf|warn|part}`
//! em `locales/*.yml`); o que o banco semeia em `dnf_template`/`non_dnf_template`/
//! `description_short` é só fallback para o id que ainda não tem linha no locale.
//!
//! ## O texto CONGELA no idioma em que a corrida foi disputada — e isso é decisão
//!
//! O render acontece uma vez, na hora do incidente, e o resultado vai para o save junto com a
//! corrida. Trocar o idioma do jogo depois **não retraduz o histórico**: as corridas antigas
//! seguem no idioma em que foram corridas, as novas saem no idioma novo.
//!
//! Isto é a **opção A**, a mesma já adotada nas lesões, e está escrito aqui porque sem o
//! registro o próximo leitor trata como bug. As três razões, na ordem em que pesam:
//!
//! 1. **O save guarda prosa, não chave.** O incidente é gravado como texto renderizado, com o
//!    nome do piloto já substituído. Retraduzir exigiria guardar `(id, parâmetros)` em vez da
//!    frase e recompor tudo na leitura — mudança de contrato de save, não de texto.
//! 2. **O histórico é um registro do que aconteceu.** Uma manchete de 2029 lida em 2031 é um
//!    documento da temporada de 2029; retraduzi-la seria reescrever o passado, não exibi-lo.
//! 3. **O id sobrevive.** `SelectedEntry::catalog_id` vai junto no save, então **a porta não
//!    está fechada**: quem um dia quiser retraduzir o histórico tem por onde — a decisão é de
//!    não fazer agora, não de impedir.
//!
//! ## Paridade — conferida em 11/08/2026
//!
//! Os 54 ids semeados por `db::migrations::seed_incidentes` existem nos DOIS locales, sem sobra
//! de nenhum lado. Isso importa mais do que parece por causa do fallback: um id que faltasse no
//! `en-US` não daria erro nem chave crua — cairia calado no `dnf_template` do banco, que está
//! **em português**, e o jogador em inglês veria uma frase em português no meio da corrida sem
//! nada indicar que algo falhou. É a guarda
//! `todo_id_do_catalogo_tem_texto_nos_dois_locales` que impede isso de voltar.

#![allow(dead_code)]

use rand::Rng;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::connection::DbError;

// ── Enums ─────────────────────────────────────────────────────────────────────

/// Classe de veículo — determina quais entries do catálogo são elegíveis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VehicleClass {
    StreetBased,
    RaceSpec,
    Prototype,
}

/// Filtro de formato de corrida no catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaceFormatFilter {
    Sprint,
    Endurance,
    Both,
}

/// Fonte do incidente no catálogo — ortogonal ao IncidentType do motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentSource {
    Mechanical,
    DriverError,
    PostCollision,
    Operational,
}

/// Quando a entry é elegível para seleção.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    /// Roll normal do motor (mecânico espontâneo, erro de pilotagem).
    Spontaneous,
    /// Só após colisão Minor/Major não-DNF.
    PostCollision,
    /// Só após DriverError Minor (rodada) — agravamento para stall.
    PostSpinStall,
}

/// Se a entry se aplica a DNF, non-DNF ou ambos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityContext {
    DnfOnly,
    NonDnfOnly,
    Both,
}

// ── Structs ───────────────────────────────────────────────────────────────────

pub struct CatalogEntry {
    pub id: String,
    pub vehicle_class: VehicleClass,
    pub race_format: RaceFormatFilter,
    pub incident_source: IncidentSource,
    pub trigger_type: TriggerType,
    pub severity_context: SeverityContext,
    pub weight_sprint: u32,
    pub weight_endurance: u32,
    pub dnf_template: String,
    pub non_dnf_template: Option<String>,
    pub description_short: String,
}

pub struct SelectedEntry {
    pub catalog_id: String,
    pub rendered_text: String,
    pub description_short: String,
}

pub struct IncidentCatalog {
    entries: Vec<CatalogEntry>,
}

// ── Parsing helpers ───────────────────────────────────────────────────────────

fn parse_vehicle_class(s: &str) -> VehicleClass {
    match s {
        "RaceSpec" => VehicleClass::RaceSpec,
        "Prototype" => VehicleClass::Prototype,
        _ => VehicleClass::StreetBased,
    }
}

fn parse_race_format(s: &str) -> RaceFormatFilter {
    match s {
        "Sprint" => RaceFormatFilter::Sprint,
        "Endurance" => RaceFormatFilter::Endurance,
        _ => RaceFormatFilter::Both,
    }
}

fn parse_incident_source(s: &str) -> IncidentSource {
    match s {
        "DriverError" => IncidentSource::DriverError,
        "PostCollision" => IncidentSource::PostCollision,
        "Operational" => IncidentSource::Operational,
        _ => IncidentSource::Mechanical,
    }
}

fn parse_trigger_type(s: &str) -> TriggerType {
    match s {
        "PostCollision" => TriggerType::PostCollision,
        "PostSpinStall" => TriggerType::PostSpinStall,
        _ => TriggerType::Spontaneous,
    }
}

fn parse_severity_context(s: &str) -> SeverityContext {
    match s {
        "DnfOnly" => SeverityContext::DnfOnly,
        "NonDnfOnly" => SeverityContext::NonDnfOnly,
        _ => SeverityContext::Both,
    }
}

// ── Filter helpers ────────────────────────────────────────────────────────────

fn format_matches(filter: RaceFormatFilter, is_endurance: bool) -> bool {
    match filter {
        RaceFormatFilter::Both => true,
        RaceFormatFilter::Sprint => !is_endurance,
        RaceFormatFilter::Endurance => is_endurance,
    }
}

fn severity_matches(ctx: SeverityContext, is_dnf: bool) -> bool {
    match ctx {
        SeverityContext::Both => true,
        SeverityContext::DnfOnly => is_dnf,
        SeverityContext::NonDnfOnly => !is_dnf,
    }
}

// ── vehicle_class_from_category ───────────────────────────────────────────────

/// Resolve vehicle class a partir do category_id.
/// Categorias desconhecidas → StreetBased (fallback conservador).
pub fn vehicle_class_from_category(category_id: &str) -> VehicleClass {
    match category_id {
        "mazda_rookie"
        | "toyota_rookie"
        | "mazda_amador"
        | "toyota_amador"
        | "bmw_m2"
        | "production_challenger" => VehicleClass::StreetBased,
        "gt4" | "gt3" => VehicleClass::RaceSpec,
        _ => VehicleClass::StreetBased,
    }
}

// ── IncidentCatalog ───────────────────────────────────────────────────────────

impl IncidentCatalog {
    /// Carrega todas as entries da tabela incident_catalog.
    pub fn load(conn: &Connection) -> Result<Self, DbError> {
        let mut stmt = conn.prepare(
            "SELECT id, vehicle_class, race_format, incident_source, trigger_type,
                    severity_context, weight_sprint, weight_endurance,
                    dnf_template, non_dnf_template, description_short
             FROM incident_catalog",
        )?;

        let entries = stmt
            .query_map([], |row| {
                let vehicle_class_str: String = row.get(1)?;
                let race_format_str: String = row.get(2)?;
                let incident_source_str: String = row.get(3)?;
                let trigger_type_str: String = row.get(4)?;
                let severity_context_str: String = row.get(5)?;

                Ok(CatalogEntry {
                    id: row.get(0)?,
                    vehicle_class: parse_vehicle_class(&vehicle_class_str),
                    race_format: parse_race_format(&race_format_str),
                    incident_source: parse_incident_source(&incident_source_str),
                    trigger_type: parse_trigger_type(&trigger_type_str),
                    severity_context: parse_severity_context(&severity_context_str),
                    weight_sprint: row.get::<_, i64>(6)? as u32,
                    weight_endurance: row.get::<_, i64>(7)? as u32,
                    dnf_template: row.get(8)?,
                    non_dnf_template: row.get(9)?,
                    description_short: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(Self { entries })
    }

    /// Catálogo vazio — para testes que não se importam com flavor text.
    /// Com catálogo vazio, `select_and_render` retorna `None`,
    /// `catalog_id` fica `None`, e o comportamento existente é preservado.
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Retorna entries que satisfazem todos os critérios.
    pub fn filter(
        &self,
        vehicle_class: VehicleClass,
        is_endurance: bool,
        incident_source: IncidentSource,
        trigger_type: TriggerType,
        is_dnf: bool,
    ) -> Vec<&CatalogEntry> {
        self.entries
            .iter()
            .filter(|e| {
                e.vehicle_class == vehicle_class
                    && format_matches(e.race_format, is_endurance)
                    && e.incident_source == incident_source
                    && e.trigger_type == trigger_type
                    && severity_matches(e.severity_context, is_dnf)
                    && weight_for(e, is_endurance) > 0
            })
            .collect()
    }

    /// Seleciona uma entry por peso ponderado e renderiza o template.
    /// Retorna `None` se nenhuma entry elegível existe (catálogo vazio ou sem match).
    pub fn select_and_render(
        &self,
        vehicle_class: VehicleClass,
        is_endurance: bool,
        incident_source: IncidentSource,
        trigger_type: TriggerType,
        is_dnf: bool,
        driver_name: &str,
        rng: &mut impl Rng,
    ) -> Option<SelectedEntry> {
        let candidates = self.filter(
            vehicle_class,
            is_endurance,
            incident_source,
            trigger_type,
            is_dnf,
        );
        if candidates.is_empty() {
            return None;
        }

        let total_weight: u32 = candidates.iter().map(|e| weight_for(e, is_endurance)).sum();
        if total_weight == 0 {
            return None;
        }

        let mut pick = rng.gen_range(0..total_weight);
        let chosen = candidates.iter().find(|e| {
            let w = weight_for(e, is_endurance);
            if pick < w {
                true
            } else {
                pick -= w;
                false
            }
        })?;

        // Texto no locale ATIVO, resolvido por id (i18n `breakdown.<id>.{dnf|warn|part}`).
        // O texto semeado no banco (dnf_template/non_dnf_template/description_short) vira
        // só fallback: rust-i18n devolve a própria chave quando ela não existe em nenhum
        // locale (ex.: cenários sem .dnf/.warn). `{driver}` é substituído aqui.
        let sub = if is_dnf { "dnf" } else { "warn" };
        let key = format!("breakdown.{}.{}", chosen.id, sub);
        let translated = rust_i18n::t!(&key).to_string();
        let template = if translated == key {
            if is_dnf {
                chosen.dnf_template.clone()
            } else {
                chosen
                    .non_dnf_template
                    .clone()
                    .unwrap_or_else(|| chosen.dnf_template.clone())
            }
        } else {
            translated
        };
        let rendered_text = template.replace("{driver}", driver_name);

        let part_key = format!("breakdown.{}.part", chosen.id);
        let part = rust_i18n::t!(&part_key).to_string();
        let description_short = if part == part_key {
            chosen.description_short.clone()
        } else {
            part
        };

        Some(SelectedEntry {
            catalog_id: chosen.id.clone(),
            rendered_text,
            description_short,
        })
    }
}

fn weight_for(entry: &CatalogEntry, is_endurance: bool) -> u32 {
    if is_endurance {
        entry.weight_endurance
    } else {
        entry.weight_sprint
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    /// Cenário de quebra real (`breakdown.<id>`) resolve nos dois locales, mantém o
    /// placeholder `{driver}` (substituído no render) e não vaza `%{...}`. `#[serial]`
    /// porque troca o locale global.
    #[test]
    #[serial_test::serial]
    fn breakdown_scenario_resolve_nos_dois_locales() {
        rust_i18n::set_locale("pt-BR");
        let pt = rust_i18n::t!("breakdown.SB_S_MEC_01.dnf").to_string();
        assert!(
            pt.contains("{driver}") && pt.contains("câmbio") && !pt.contains("%{"),
            "{pt}"
        );

        rust_i18n::set_locale("en-US");
        let en = rust_i18n::t!("breakdown.SB_S_MEC_01.dnf").to_string();
        assert!(
            en.contains("{driver}") && en.contains("gearbox") && !en.contains("%{"),
            "{en}"
        );
        assert_ne!(pt, en);

        rust_i18n::set_locale("pt-BR"); // restaura
    }

    /// **TODO id semeado tem texto nos dois locales.** O teste acima confere UM id; este
    /// confere os 54, e é o que fecha a decisão de congelamento documentada no topo do módulo.
    ///
    /// O modo de falha que ele existe para pegar é silencioso por construção: id sem linha no
    /// locale não estoura nem devolve a chave crua — cai no `dnf_template` do banco, que está
    /// em português. O jogador em inglês leria português achando que é o jogo.
    ///
    /// A lista de ids sai do próprio seed, lido como TEXTO — a mesma técnica de
    /// `calibracao::consumo`. Ler o fonte evita acoplar esta guarda ao banco (migração inteira
    /// só para descobrir 54 strings) e garante que um cenário novo entre na guarda no mesmo
    /// commit em que entra no seed. `#[serial]` porque o locale é global do processo.
    #[test]
    #[serial_test::serial]
    fn todo_id_do_catalogo_tem_texto_nos_dois_locales() {
        let fonte = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/db/migrations/seed_incidentes.rs"),
        )
        .expect("fonte do seed do catálogo");

        // Os ids são a PRIMEIRA string de cada tupla e têm forma fixa (`SB_S_MEC_01`,
        // `RS_COL_03`): duas letras, blocos maiúsculos separados por `_`, número no fim.
        let mut ids: Vec<String> = fonte
            .split('"')
            .skip(1)
            .step_by(2)
            .filter(|s| {
                let partes: Vec<&str> = s.split('_').collect();
                partes.len() >= 3
                    && partes[0].len() == 2
                    && partes.iter().all(|p| !p.is_empty())
                    && partes[..partes.len() - 1]
                        .iter()
                        .all(|p| p.chars().all(|c| c.is_ascii_uppercase()))
                    && partes[partes.len() - 1].chars().all(|c| c.is_ascii_digit())
            })
            .map(str::to_string)
            .collect();
        ids.sort();
        ids.dedup();
        assert_eq!(
            ids.len(),
            54,
            "o seed mudou de tamanho — confira a guarda antes de mudar o número: {ids:?}"
        );

        let anterior = rust_i18n::locale().to_string();
        let mut faltando = Vec::new();
        for locale in ["pt-BR", "en-US"] {
            rust_i18n::set_locale(locale);
            for id in &ids {
                // `.part` é o único sufixo que TODO cenário tem: `.dnf`/`.warn` dependem do
                // `severity_context` da entry, e cobrar os três daria falso positivo.
                let chave = format!("breakdown.{id}.part");
                if rust_i18n::t!(&chave) == chave {
                    faltando.push(format!("{locale}: {chave}"));
                }
            }
        }
        rust_i18n::set_locale(&anterior);

        assert!(
            faltando.is_empty(),
            "id do catálogo sem texto no locale (cairia no template PT do banco em silêncio):\n{}",
            faltando.join("\n")
        );
    }

    fn make_entry(
        id: &str,
        vehicle_class: VehicleClass,
        race_format: RaceFormatFilter,
        incident_source: IncidentSource,
        trigger_type: TriggerType,
        severity_context: SeverityContext,
        weight_sprint: u32,
        weight_endurance: u32,
        dnf_template: &str,
        non_dnf_template: Option<&str>,
    ) -> CatalogEntry {
        CatalogEntry {
            id: id.to_string(),
            vehicle_class,
            race_format,
            incident_source,
            trigger_type,
            severity_context,
            weight_sprint,
            weight_endurance,
            dnf_template: dnf_template.to_string(),
            non_dnf_template: non_dnf_template.map(|s| s.to_string()),
            description_short: format!("{} short", id),
        }
    }

    #[test]
    fn test_filter_returns_matching_entries() {
        let catalog = IncidentCatalog {
            entries: vec![
                make_entry(
                    "SB_S",
                    VehicleClass::StreetBased,
                    RaceFormatFilter::Sprint,
                    IncidentSource::Mechanical,
                    TriggerType::Spontaneous,
                    SeverityContext::Both,
                    100,
                    0,
                    "dnf {driver}",
                    Some("non {driver}"),
                ),
                make_entry(
                    "RS_S",
                    VehicleClass::RaceSpec,
                    RaceFormatFilter::Sprint,
                    IncidentSource::Mechanical,
                    TriggerType::Spontaneous,
                    SeverityContext::Both,
                    100,
                    0,
                    "dnf {driver}",
                    Some("non {driver}"),
                ),
                make_entry(
                    "SB_E",
                    VehicleClass::StreetBased,
                    RaceFormatFilter::Endurance,
                    IncidentSource::Mechanical,
                    TriggerType::Spontaneous,
                    SeverityContext::Both,
                    0,
                    100,
                    "dnf {driver}",
                    Some("non {driver}"),
                ),
            ],
        };

        let result = catalog.filter(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Mechanical,
            TriggerType::Spontaneous,
            true,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "SB_S");
    }

    #[test]
    fn test_filter_both_format_matches_sprint_and_endurance() {
        let catalog = IncidentCatalog {
            entries: vec![make_entry(
                "BOTH",
                VehicleClass::StreetBased,
                RaceFormatFilter::Both,
                IncidentSource::PostCollision,
                TriggerType::PostCollision,
                SeverityContext::Both,
                100,
                100,
                "{driver} dnf",
                None,
            )],
        };

        let sprint = catalog.filter(
            VehicleClass::StreetBased,
            false,
            IncidentSource::PostCollision,
            TriggerType::PostCollision,
            true,
        );
        let endurance = catalog.filter(
            VehicleClass::StreetBased,
            true,
            IncidentSource::PostCollision,
            TriggerType::PostCollision,
            true,
        );
        assert_eq!(sprint.len(), 1);
        assert_eq!(endurance.len(), 1);
    }

    #[test]
    fn test_weighted_selection_excludes_zero_weight() {
        let catalog = IncidentCatalog {
            entries: vec![
                make_entry(
                    "GOOD",
                    VehicleClass::StreetBased,
                    RaceFormatFilter::Sprint,
                    IncidentSource::Mechanical,
                    TriggerType::Spontaneous,
                    SeverityContext::Both,
                    100,
                    0,
                    "{driver} dnf",
                    None,
                ),
                make_entry(
                    "ZERO",
                    VehicleClass::StreetBased,
                    RaceFormatFilter::Sprint,
                    IncidentSource::Mechanical,
                    TriggerType::Spontaneous,
                    SeverityContext::Both,
                    0,
                    0,
                    "{driver} dnf",
                    None,
                ),
            ],
        };
        let mut rng = StdRng::seed_from_u64(42);

        for _ in 0..20 {
            let result = catalog.select_and_render(
                VehicleClass::StreetBased,
                false,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                true,
                "Piloto",
                &mut rng,
            );
            assert!(result.is_some());
            assert_eq!(result.unwrap().catalog_id, "GOOD");
        }
    }

    #[test]
    fn test_select_and_render_substitutes_driver_name() {
        let catalog = IncidentCatalog {
            entries: vec![make_entry(
                "E1",
                VehicleClass::StreetBased,
                RaceFormatFilter::Sprint,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                SeverityContext::Both,
                100,
                0,
                "{driver} abandona com problema no câmbio",
                None,
            )],
        };
        let mut rng = StdRng::seed_from_u64(1);

        let result = catalog.select_and_render(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Mechanical,
            TriggerType::Spontaneous,
            true,
            "Senna",
            &mut rng,
        );
        assert!(result.is_some());
        let sel = result.unwrap();
        assert_eq!(sel.rendered_text, "Senna abandona com problema no câmbio");
        assert_eq!(sel.catalog_id, "E1");
    }

    #[test]
    fn test_select_and_render_non_dnf_uses_non_dnf_template() {
        let catalog = IncidentCatalog {
            entries: vec![make_entry(
                "E1",
                VehicleClass::StreetBased,
                RaceFormatFilter::Sprint,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                SeverityContext::Both,
                100,
                0,
                "{driver} abandona",
                Some("{driver} perdeu ritmo"),
            )],
        };
        let mut rng = StdRng::seed_from_u64(1);

        let result = catalog.select_and_render(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Mechanical,
            TriggerType::Spontaneous,
            false,
            "Prost",
            &mut rng,
        );
        assert_eq!(result.unwrap().rendered_text, "Prost perdeu ritmo");
    }

    #[test]
    fn test_empty_catalog_returns_none() {
        let catalog = IncidentCatalog::empty();
        let mut rng = StdRng::seed_from_u64(1);

        let result = catalog.select_and_render(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Mechanical,
            TriggerType::Spontaneous,
            true,
            "Piloto",
            &mut rng,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_vehicle_class_from_category_known() {
        assert_eq!(vehicle_class_from_category("gt3"), VehicleClass::RaceSpec);
        assert_eq!(vehicle_class_from_category("gt4"), VehicleClass::RaceSpec);
        assert_eq!(
            vehicle_class_from_category("mazda_rookie"),
            VehicleClass::StreetBased
        );
        assert_eq!(
            vehicle_class_from_category("bmw_m2"),
            VehicleClass::StreetBased
        );
        assert_eq!(
            vehicle_class_from_category("production_challenger"),
            VehicleClass::StreetBased
        );
    }

    #[test]
    fn test_vehicle_class_from_category_unknown_fallback() {
        assert_eq!(
            vehicle_class_from_category("formula_x"),
            VehicleClass::StreetBased
        );
        assert_eq!(vehicle_class_from_category(""), VehicleClass::StreetBased);
        assert_eq!(
            vehicle_class_from_category("endurance"),
            VehicleClass::StreetBased
        );
    }

    #[test]
    fn test_severity_context_dnf_only_excludes_non_dnf() {
        let catalog = IncidentCatalog {
            entries: vec![make_entry(
                "DNF_ONLY",
                VehicleClass::StreetBased,
                RaceFormatFilter::Sprint,
                IncidentSource::Operational,
                TriggerType::PostSpinStall,
                SeverityContext::DnfOnly,
                100,
                0,
                "{driver} rodou e não religou",
                None,
            )],
        };

        let dnf_result = catalog.filter(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Operational,
            TriggerType::PostSpinStall,
            true,
        );
        let non_dnf_result = catalog.filter(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Operational,
            TriggerType::PostSpinStall,
            false,
        );

        assert_eq!(dnf_result.len(), 1);
        assert_eq!(non_dnf_result.len(), 0);
    }
}
