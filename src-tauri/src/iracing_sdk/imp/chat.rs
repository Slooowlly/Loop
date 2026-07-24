//! Envio de comandos de chat ao iRacing: macro (broadcast message) e texto livre
//! (foco da janela + `SendInput`).

use super::janela::{bring_iracing_to_front, find_iracing_hwnd};
use super::util::wide_null;
use crate::iracing_sdk::IracingError;

/// Dispara um macro de chat do iRacing via a broadcast message documentada
/// do SDK (`IRSDK_BROADCASTMSG`). Não usa a shared memory — é uma mensagem
/// de janela. O iRacing então "digita" o texto do macro (ex.: `!yellow`).
pub fn send_chat_macro(macro_num: i32) -> Result<(), IracingError> {
    use winapi::um::winuser::{RegisterWindowMessageW, SendNotifyMessageW, HWND_BROADCAST};

    // irsdk_BroadcastChatComand = 8; irsdk_ChatCommand_Macro = 0.
    const BROADCAST_CHAT_COMMAND: i32 = 8;
    const CHAT_COMMAND_MACRO: i32 = 0;

    let name = wide_null("IRSDK_BROADCASTMSG");
    unsafe {
        let msg_id = RegisterWindowMessageW(name.as_ptr());
        if msg_id == 0 {
            return Err(IracingError::MapFailed(
                winapi::um::errhandlingapi::GetLastError(),
            ));
        }
        // wParam = MAKELONG(msg, var1); lParam = MAKELONG(var2, var3).
        let wparam = (BROADCAST_CHAT_COMMAND as usize & 0xffff)
            | ((CHAT_COMMAND_MACRO as usize & 0xffff) << 16);
        let lparam = (macro_num & 0xffff) as isize;
        SendNotifyMessageW(HWND_BROADCAST, msg_id, wparam, lparam);
    }
    Ok(())
}

/// Abre a linha de chat do iRacing via broadcast `ChatCommand_BeginChat`
/// (subcomando 1 do `BroadcastChatCommand`). Diferente do macro, isto só
/// ABRE a caixa de digitação — o texto vem depois, por [`type_unicode`].
fn begin_chat() -> Result<(), IracingError> {
    use winapi::um::winuser::{RegisterWindowMessageW, SendNotifyMessageW, HWND_BROADCAST};

    // irsdk_BroadcastChatComand = 8; irsdk_ChatCommand_BeginChat = 1.
    const BROADCAST_CHAT_COMMAND: i32 = 8;
    const CHAT_COMMAND_BEGIN: i32 = 1;

    let name = wide_null("IRSDK_BROADCASTMSG");
    unsafe {
        let msg_id = RegisterWindowMessageW(name.as_ptr());
        if msg_id == 0 {
            return Err(IracingError::MapFailed(
                winapi::um::errhandlingapi::GetLastError(),
            ));
        }
        let wparam = (BROADCAST_CHAT_COMMAND as usize & 0xffff)
            | ((CHAT_COMMAND_BEGIN as usize & 0xffff) << 16);
        SendNotifyMessageW(HWND_BROADCAST, msg_id, wparam, 0);
    }
    Ok(())
}

/// Digita `text` (e Enter no fim) via `SendInput` a nível de SO — cada char
/// vai como evento Unicode, então funciona com qualquer caractere sem depender
/// de layout de teclado. Requer que a janela ALVO esteja em foreground (é o
/// papel do `focus_iracing_window` antes desta chamada). Injeta tudo num único
/// `SendInput` para preservar a ordem.
fn type_unicode(text: &str) -> Result<(), IracingError> {
    use winapi::um::winuser::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_RETURN,
    };

    unsafe fn key_event(scan: u16, vk: u16, flags: u32) -> INPUT {
        let mut input: INPUT = std::mem::zeroed();
        input.type_ = INPUT_KEYBOARD;
        let ki = input.u.ki_mut();
        ki.wVk = vk;
        ki.wScan = scan;
        ki.dwFlags = flags;
        input
    }

    let mut inputs: Vec<INPUT> = Vec::new();
    unsafe {
        // Texto: cada unidade UTF-16 como key-down/key-up Unicode.
        for unit in text.encode_utf16() {
            inputs.push(key_event(unit, 0, KEYEVENTF_UNICODE));
            inputs.push(key_event(unit, 0, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
        }
        // Enter: por virtual-key (mais confiável que '\n' num jogo).
        inputs.push(key_event(0, VK_RETURN as u16, 0));
        inputs.push(key_event(0, VK_RETURN as u16, KEYEVENTF_KEYUP));

        let sent = SendInput(
            inputs.len() as u32,
            inputs.as_mut_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
        if sent as usize != inputs.len() {
            return Err(IracingError::MapFailed(
                winapi::um::errhandlingapi::GetLastError(),
            ));
        }
    }
    Ok(())
}

/// Envia um comando de chat de TEXTO LIVRE ao iRacing (ex.: `!black #1 20`).
/// Foca a janela do sim → abre o chat (`begin_chat`) → digita o texto + Enter
/// (`type_unicode`). É o caminho para comandos PARAMETRIZADOS, que o macro
/// (texto fixo em `app.ini`, cacheado pelo sim) não cobre. Os `sleep` dão
/// tempo do foco assentar e da caixa de chat abrir antes de digitar.
pub fn send_chat_text(text: &str) -> Result<(), IracingError> {
    use std::thread::sleep;
    use std::time::Duration;

    let Some(hwnd) = find_iracing_hwnd() else {
        return Err(IracingError::NotRunning("iRacing".to_string()));
    };
    // Traz o sim ao foreground E VERIFICA que o SO aceitou. Sem essa checagem, o
    // `SendInput` seguinte seria disparado contra uma janela que nunca virou foco
    // (fullscreen exclusivo / trava de foco) e o comando (`!black`, `!dq`) sumiria
    // sem rastro. Reportando `ForegroundBlocked`, o chamador ao vivo consegue avisar
    // o jogador em vez de o sistema de quebra falhar em silêncio.
    //
    // LIMITE conhecido: se o sim JÁ é o foreground (jogador dirigindo) mas está em
    // fullscreen EXCLUSIVO, o `SetForegroundWindow` devolve sucesso e mesmo assim o
    // `SendInput` pode não chegar — isso não é detectável daqui. A checagem cobre o
    // caso comum (nosso app em segundo plano e o SO recusando o roubo de foco).
    if !bring_iracing_to_front(hwnd) {
        return Err(IracingError::ForegroundBlocked);
    }
    sleep(Duration::from_millis(150));
    begin_chat()?;
    sleep(Duration::from_millis(150));
    type_unicode(text)
}
