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

        let template = if is_dnf {
            &chosen.dnf_template
        } else {
            chosen
                .non_dnf_template
                .as_deref()
                .unwrap_or(&chosen.dnf_template)
        };

        let rendered_text = template.replace("{driver}", driver_name);

        Some(SelectedEntry {
            catalog_id: chosen.id.clone(),
            rendered_text,
            description_short: chosen.description_short.clone(),
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
