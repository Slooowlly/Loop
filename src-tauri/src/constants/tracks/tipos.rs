#![allow(dead_code)]

use crate::models::enums::{RainGroup, TrackType};

pub struct TrackInfo {
    pub track_id: u32,
    pub nome: &'static str,
    pub nome_curto: &'static str,
    pub pais: &'static str,
    pub comprimento_km: f64,
    pub rain_group: RainGroup,
    pub gratuita: bool,
    pub tipo: TrackType,
}

pub type TrackDefinition = TrackInfo;
