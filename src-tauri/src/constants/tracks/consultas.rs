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

/// Substituta da pista pedida quando ela não está no conjunto POSSUÍDO, dado esse
/// conjunto explicitamente. Pista possuída (e a própria escolha) passa intacta.
///
/// A escolha é determinística e INDEPENDENTE de ordem (`track_id % pool`): a mesma
/// pista de fora sempre mapeia para a mesma substituta em todo export — o import compara
/// o resultado do iRacing contra a pista que foi de fato exportada, então o mapa precisa
/// ser estável entre exports. Prefere substituta do mesmo tipo (road/roval).
///
/// `possuidas` vazio devolve a original: sem dado de posse, o certo é não substituir
/// nada, e não substituir tudo.
pub fn substituta_para_posse(
    track_id: u32,
    possuidas: &[&'static TrackInfo],
) -> Option<&'static TrackInfo> {
    let original = get_track(track_id);
    if let Some(possuida) = possuidas.iter().find(|t| t.track_id == track_id) {
        return Some(possuida);
    }
    if possuidas.is_empty() {
        return original;
    }

    // Prefere manter o tipo (road/roval) da pista original; se não houver possuída do
    // mesmo tipo, cai para o conjunto inteiro.
    let pool: Vec<&'static TrackInfo> = match original.map(|t| t.tipo) {
        Some(tipo) => {
            let same: Vec<&'static TrackInfo> = possuidas
                .iter()
                .copied()
                .filter(|t| t.tipo == tipo)
                .collect();
            if same.is_empty() {
                possuidas.to_vec()
            } else {
                same
            }
        }
        None => possuidas.to_vec(),
    };

    Some(pool[(track_id as usize) % pool.len()])
}

/// Fallback do EXPORT: substitui todo conteúdo PAGO por uma pista grátis.
///
/// **Verdito do D-06 (11/08/2026): aproximação assumida, bloqueada por falta de dado.**
/// O correto é substituir pelo que o jogador REALMENTE possui, e a regra já está escrita
/// para isso em [`substituta_para_posse`] — o que falta é o conjunto. Não existe, em
/// lugar nenhum do save nem da configuração, uma lista de pistas possuídas: o app não
/// consulta o entitlement do iRacing, não há tela para o jogador marcar o que tem, e o
/// próprio catálogo de `dados.rs` já é a posse cravada de um usuário específico, de
/// julho de 2026 (é o que o cabeçalho de lá diz: "cada entrada é um evento que o
/// jogador possui"). Enquanto isso, `gratuita` é o único recorte defensável: conteúdo
/// grátis todo mundo tem.
///
/// O que falta para fechar: a lista de posse persistida por save (ou por instalação) e a
/// UI que a preenche. No dia em que existir, esta função vira uma chamada a
/// [`substituta_para_posse`] passando essa lista, e o call site do export
/// (`commands/iracing/temporada.rs`) é o único lugar a mudar.
pub fn free_or_substitute(track_id: u32) -> Option<&'static TrackInfo> {
    match get_track(track_id) {
        Some(track) if track.gratuita => Some(track),
        _ => substituta_para_posse(track_id, &get_free_tracks()),
    }
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

/// Duração da quali da pista do catálogo. Delega para `duracao_classificacao_para`:
/// a regra mora num lugar só, senão mudar o corte de 5 km numa e esquecer a outra
/// desalinha o export do iRacing e a simulação.
pub fn get_qualifying_duration(track_id: u32) -> u8 {
    let Some(track) = get_track(track_id) else {
        return 15;
    };

    duracao_classificacao_para(track.comprimento_km, track.track_id) as u8
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
