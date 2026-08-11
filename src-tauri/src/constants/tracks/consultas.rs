#![allow(dead_code)]

use super::dados::TRACKS;
use super::tipos::TrackInfo;
use crate::constants::categories::get_category_config;
use crate::models::enums::RainGroup;

pub fn get_track(track_id: u32) -> Option<&'static TrackInfo> {
    TRACKS.iter().find(|track| track.track_id == track_id)
}

pub fn get_all_tracks() -> &'static [TrackInfo] {
    TRACKS
}

pub fn get_free_tracks() -> Vec<&'static TrackInfo> {
    TRACKS.iter().filter(|track| track.gratuita).collect()
}

/// Fallback de TESTE: quando a pista pedida é conteúdo PAGO (que o jogador pode
/// não possuir), devolve uma pista GRÁTIS determinística no lugar, para o export
/// conseguir rodar mesmo sem a pista real. Pistas já grátis (e a própria escolha)
/// passam intactas.
///
/// A escolha é determinística e INDEPENDENTE de ordem (`track_id % pool`): a mesma
/// pista paga sempre mapeia para a mesma free em todo export — o import compara o
/// resultado do iRacing contra a pista que foi de fato exportada, então o mapa
/// precisa ser estável entre exports. Prefere free do mesmo tipo (road/roval).
///
/// TODO(design final): trocar por "pistas que o jogador realmente possui" em vez
/// de substituir todo conteúdo pago — ver backlog de posse de pistas.
pub fn free_or_substitute(track_id: u32) -> Option<&'static TrackInfo> {
    let original = get_track(track_id);
    if let Some(track) = original {
        if track.gratuita {
            return Some(track);
        }
    }

    let free = get_free_tracks();
    if free.is_empty() {
        return original;
    }

    // Prefere manter o tipo (road/roval) da pista original; se não houver free do
    // mesmo tipo, cai para o pool free inteiro.
    let pool: Vec<&'static TrackInfo> = match original.map(|t| t.tipo) {
        Some(tipo) => {
            let same: Vec<&'static TrackInfo> =
                free.iter().copied().filter(|t| t.tipo == tipo).collect();
            if same.is_empty() {
                free
            } else {
                same
            }
        }
        None => free,
    };

    Some(pool[(track_id as usize) % pool.len()])
}

pub fn get_tracks_for_tier(tier: u8) -> Vec<&'static TrackInfo> {
    if tier <= 2 {
        get_free_tracks()
    } else {
        TRACKS.iter().collect()
    }
}

pub fn get_tracks_for_category(category_id: &str) -> Vec<&'static TrackInfo> {
    let Some(category) = get_category_config(category_id) else {
        return Vec::new();
    };

    get_tracks_for_tier(category.tier)
}

pub fn get_rain_chance(track_id: u32) -> f64 {
    match get_track(track_id).map(|track| track.rain_group) {
        Some(RainGroup::Dry) => 0.02,
        Some(RainGroup::Normal) => 0.08,
        Some(RainGroup::Rainy) => 0.18,
        None => 0.08,
    }
}

// Ids longos/endurance que merecem quali estendida: Nordschleife (249),
// Le Mans (268), Nürburgring 24h (252), Spa Endurance (525).
const LONG_QUALI_TRACK_IDS: &[u32] = &[249, 268, 252, 525];

pub fn get_qualifying_duration(track_id: u32) -> u8 {
    let Some(track) = get_track(track_id) else {
        return 15;
    };

    if LONG_QUALI_TRACK_IDS.contains(&track.track_id) {
        20
    } else if track.comprimento_km > 5.0 {
        18
    } else {
        15
    }
}

pub fn duracao_classificacao_para(comprimento_km: f64, track_id: u32) -> u32 {
    if LONG_QUALI_TRACK_IDS.contains(&track_id) {
        20
    } else if comprimento_km > 5.0 {
        18
    } else {
        15
    }
}
