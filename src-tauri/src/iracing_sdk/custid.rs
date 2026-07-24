//! Custid do jogador: capturado automaticamente do YAML de sessão e persistido
//! em disco, para o app saber quem é o jogador entre execuções.

use super::parse_player_custid;

static PLAYER_CUSTID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
static CUSTID_LOADED: std::sync::Once = std::sync::Once::new();

fn custid_file() -> std::path::PathBuf {
    std::env::temp_dir().join("loop_player_custid.txt")
}

fn ensure_custid_loaded() {
    CUSTID_LOADED.call_once(|| {
        if let Ok(s) = std::fs::read_to_string(custid_file()) {
            if let Ok(id) = s.trim().parse::<i64>() {
                if id > 0 {
                    PLAYER_CUSTID.store(id, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
    });
}

/// Custid do jogador já conhecido (capturado nesta sessão ou persistido de antes).
/// `None` enquanto o jogador não tiver entrado em nenhuma sessão.
pub fn cached_custid() -> Option<i64> {
    ensure_custid_loaded();
    let v = PLAYER_CUSTID.load(std::sync::atomic::Ordering::Relaxed);
    (v > 0).then_some(v)
}

/// Captura o custid do YAML de sessão e persiste — chamado pelo sampler de fundo
/// enquanto o jogador corre. Grava uma única vez; ignora chamadas seguintes.
pub fn note_session_custid(yaml: &str) {
    ensure_custid_loaded();
    if PLAYER_CUSTID.load(std::sync::atomic::Ordering::Relaxed) > 0 {
        return;
    }
    if let Some(id) = parse_player_custid(yaml) {
        PLAYER_CUSTID.store(id, std::sync::atomic::Ordering::Relaxed);
        let _ = std::fs::write(custid_file(), id.to_string());
    }
}
