//! O **gatilho** do push-to-talk: o botão que o piloto segura para falar com o engenheiro.
//!
//! Toda a vigilância mora aqui, no Rust, e não no webview, por um motivo que não é
//! preferência: quando o jogador está falando com o engenheiro ele está **dentro do
//! iRacing**, com o Loop atrás. Um `keydown` no front só chega à janela em foco, e a
//! janela em foco é o simulador. `GetAsyncKeyState` e `joyGetPosEx` respondem
//! independentemente de foco — são as duas únicas portas que funcionam no caso real.
//!
//! Duas diferenças em relação ao vigia do recentro de VR, que mora ao lado em
//! [`crate::commands::volante`]:
//!
//! * O recentro dispara na borda de **descida** (apertou, aconteceu). O push-to-talk
//!   precisa das **duas**: apertou abre o microfone, soltou fecha e envia.
//! * O recentro só entende botão de volante. Aqui a tecla também vale — nem todo mundo
//!   tem um botão sobrando no volante, e sem isso não dá nem para testar na mesa.
//!
//! O vigia emite dois eventos para o app (`ptt-apertou`, `ptt-soltou`) e não sabe mais
//! nada: quem grava, quem transcreve e quem responde é o orquestrador no front.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::volante::{self, BotaoVolante};

/// De onde vem o toque. Etiquetado (`tipo`) para o front montar sem ambiguidade e para
/// uma origem nova — um pedal, um botão do teclado numérico — não quebrar o que existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum GatilhoPtt {
    /// Botão de volante: par (dispositivo, botão) da API antiga do Windows.
    Volante { dispositivo: u32, botao: u32 },
    /// Tecla, pelo código Virtual-Key do Windows.
    Tecla { codigo: u32 },
}

/// Intervalo do vigia.
///
/// Mais apertado que os 30 ms do recentro, e a razão está na ponta de SOLTAR: o instante
/// em que o piloto solta é o instante em que paramos de gravar, e todo milissegundo de
/// atraso ali entra como silêncio no fim do áudio que sobe para o Scribe. 15 ms custa
/// uma consulta a mais por segundo e some no ruído.
const VIGIA_MS: u64 = 15;

fn gatilho() -> &'static Mutex<Option<GatilhoPtt>> {
    static G: OnceLock<Mutex<Option<GatilhoPtt>>> = OnceLock::new();
    G.get_or_init(|| Mutex::new(None))
}

fn app() -> &'static OnceLock<AppHandle> {
    static A: OnceLock<AppHandle> = OnceLock::new();
    &A
}

#[cfg(windows)]
fn tecla_pressionada(vk: u32) -> bool {
    // O bit ALTO diz "está apertada agora". O bit baixo diz "foi apertada desde a última
    // consulta" e LIMPA-SE ao ser lido — usá-lo aqui faria o botão parecer solto na
    // segunda volta do laço, com a mão ainda em cima dele.
    unsafe { (winapi::um::winuser::GetAsyncKeyState(vk as i32) as u16) & 0x8000 != 0 }
}

#[cfg(not(windows))]
fn tecla_pressionada(_vk: u32) -> bool {
    false
}

/// O gatilho está apertado agora?
fn esta_apertado(g: GatilhoPtt) -> bool {
    match g {
        GatilhoPtt::Volante { dispositivo, botao } => {
            volante::esta_pressionado(BotaoVolante { dispositivo, botao })
        }
        // O código 0 não é Virtual-Key nenhum: é o que chega quando a captura no front
        // falha em ler a tecla. Perguntar por ele ao Windows é comportamento indefinido,
        // e um "apertado" aqui deixaria o microfone aberto para sempre.
        GatilhoPtt::Tecla { codigo } => codigo != 0 && tecla_pressionada(codigo),
    }
}

/// Sobe o vigia na primeira vez que alguém associa um gatilho. Fica vivo até o app
/// fechar — com `None` guardado ele só dorme, igual ao vigia do recentro.
fn garantir_vigia() {
    static LIGADO: AtomicBool = AtomicBool::new(false);
    if LIGADO.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        let mut antes = false;
        loop {
            let agora = gatilho()
                .lock()
                .map(|g| g.is_some_and(esta_apertado))
                .unwrap_or(false);
            if agora != antes {
                // O evento vai para TODAS as janelas. Quem escuta é a principal, onde o
                // áudio pode tocar (a política de autoplay exige um gesto, e é lá que o
                // jogador clica) — mas emitir para uma janela por nome amarraria este
                // módulo à topologia de janelas, que já mudou uma vez neste projeto.
                if let Some(h) = app().get() {
                    let _ = h.emit(if agora { "ptt-apertou" } else { "ptt-soltou" }, ());
                }
                // Telemetria de produto: o aperto é contado AQUI, na borda física, e a
                // pergunta que de fato foi ao servidor é contada em `ptt_voz`. A
                // diferença entre os dois é o aperto que não custou nada.
                if agora {
                    crate::telemetry::uso_ptt_aperto();
                }
                antes = agora;
            }
            std::thread::sleep(std::time::Duration::from_millis(VIGIA_MS));
        }
    });
}

/// Associa (ou desassocia, com `gatilho: None`) o botão do push-to-talk.
///
/// A ligação NÃO é persistida aqui: quem guarda é o front, no mesmo padrão do botão de
/// recentro, e reempurra no boot. Guardar dos dois lados renderia duas fontes de verdade
/// para a mesma tecla.
#[tauri::command]
pub fn ptt_set_gatilho(app_handle: AppHandle, gatilho: Option<GatilhoPtt>) -> Result<(), String> {
    let _ = app().set(app_handle);
    *self::gatilho().lock().map_err(|e| e.to_string())? = gatilho;
    garantir_vigia();
    Ok(())
}

/// O gatilho associado agora — para a interface mostrar o que está valendo.
#[tauri::command]
pub fn ptt_gatilho_atual() -> Option<GatilhoPtt> {
    gatilho().lock().ok().and_then(|g| *g)
}

/// Está apertado neste instante? Só diagnóstico: o painel de configuração acende um
/// ponto enquanto o jogador segura, que é como ele confirma que associou o botão certo.
#[tauri::command]
pub fn ptt_esta_apertado() -> bool {
    gatilho()
        .lock()
        .map(|g| g.is_some_and(esta_apertado))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_gatilho_associado_nada_esta_apertado() {
        *gatilho().lock().unwrap() = None;
        assert!(!ptt_esta_apertado());
    }

    #[test]
    fn dispositivo_inexistente_conta_como_solto() {
        // O volante desligado no meio da corrida não pode deixar o microfone preso
        // aberto — o teto da API é 16 dispositivos, então 15 nunca responde.
        assert!(!esta_apertado(GatilhoPtt::Volante {
            dispositivo: 15,
            botao: 31
        }));
    }

    #[test]
    fn tecla_zero_nao_e_tecla() {
        assert!(!esta_apertado(GatilhoPtt::Tecla { codigo: 0 }));
    }

    #[test]
    fn o_gatilho_guardado_e_o_que_volta() {
        let g = GatilhoPtt::Volante {
            dispositivo: 2,
            botao: 9,
        };
        *gatilho().lock().unwrap() = Some(g);
        assert_eq!(ptt_gatilho_atual(), Some(g));
        *gatilho().lock().unwrap() = None;
        assert_eq!(ptt_gatilho_atual(), None);
    }
}
