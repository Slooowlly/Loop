//! Custid do jogador: capturado automaticamente do YAML de sessão e persistido
//! em disco, para o app saber quem é o jogador entre execuções.
//!
//! Mora na pasta de dados do app ([`super::prefs`]), e não em `%TEMP%`: este é o
//! número que casa o jogador no pós-corrida, e a Limpeza de Disco do Windows apagava
//! o arquivo sem deixar rastro — o sintoma (dificuldade adaptativa que não reconhece
//! ninguém) aparecia semanas depois, longe da causa.

use super::parse_player_custid;
use super::prefs;

static PLAYER_CUSTID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
static CUSTID_LOADED: std::sync::Once = std::sync::Once::new();

/// Nome do arquivo. Mantido igual ao antigo para o [`prefs::ler`] achar e migrar o
/// valor de quem já jogava.
const ARQUIVO: &str = "loop_player_custid.txt";

fn ensure_custid_loaded() {
    CUSTID_LOADED.call_once(|| {
        if let Some(s) = prefs::ler(ARQUIVO) {
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
        // A falha vai pro log em vez de sumir: sem este arquivo a identidade do jogador
        // se perde entre execuções, e o sintoma aparece longe daqui.
        if let Err(e) = prefs::gravar(ARQUIVO, &id.to_string()) {
            crate::diagnostico::linha(
                "iracing",
                &format!("Falha ao gravar o custid do jogador: {e}"),
            );
        }
    }
}
