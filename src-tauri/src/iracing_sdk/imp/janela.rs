//! Manipulação das janelas: achar a do iRacing, trazer ao primeiro plano (a nossa
//! ou a do sim) e saber quem está em foco agora.

use crate::iracing_sdk::IracingError;

/// Acha a janela do iRacing por título (varrendo as janelas de topo). Prefere o
/// simulador (score 2) sobre a UI/launcher (score 1). Devolve o `HWND` como
/// `isize` (bruto, `Copy`) ou `None` quando nada casa (sim fechado).
pub(super) fn find_iracing_hwnd() -> Option<isize> {
    find_iracing_janela().map(|(hwnd, _)| hwnd)
}

/// Igual à [`find_iracing_hwnd`], mas devolve também o SCORE do que foi achado:
/// 2 = simulador, 1 = só a UI/launcher. Essa distinção é o segundo sinal do
/// diagnóstico cruzado: só o simulador publica a memória do SDK, então "UI aberta
/// e memória ausente" é estado normal, enquanto "simulador aberto e memória
/// ausente" é problema de verdade. Vem de `EnumWindows`, que não depende de
/// permissão sobre o objeto de memória — por isso é um sinal independente.
pub(super) fn find_iracing_janela() -> Option<(isize, i32)> {
    use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
    use winapi::shared::windef::HWND;
    use winapi::um::winuser::{EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible};

    // Melhor candidato encontrado: o simulador (score 2) ganha da UI (score 1).
    struct Found {
        hwnd: HWND,
        score: i32,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let found = &mut *(lparam as *mut Option<Found>);
        if IsWindowVisible(hwnd) == 0 {
            return TRUE;
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return TRUE;
        }
        let mut buf: Vec<u16> = vec![0; (len + 1) as usize];
        let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
        if got <= 0 {
            return TRUE;
        }
        buf.truncate(got as usize);
        let title = String::from_utf16_lossy(&buf).to_lowercase();
        let score = if title.contains("iracing.com simulator") {
            2
        } else if title.contains("iracing") {
            1
        } else {
            0
        };
        if score > 0 && found.as_ref().map_or(true, |f| score > f.score) {
            *found = Some(Found { hwnd, score });
        }
        TRUE
    }

    let mut found: Option<Found> = None;
    unsafe {
        EnumWindows(Some(enum_proc), &mut found as *mut _ as LPARAM);
    }
    found.map(|f| (f.hwnd as isize, f.score))
}

/// Restaura (se minimizada) e traz a janela `hwnd_raw` ao primeiro plano. Devolve
/// `true` se o SO aceitou a troca de foreground (`SetForegroundWindow` != 0); em
/// fullscreen EXCLUSIVO ou com trava de foco o Windows recusa e devolve `false`.
pub(super) fn bring_iracing_to_front(hwnd_raw: isize) -> bool {
    use winapi::shared::windef::HWND;
    use winapi::um::winuser::{
        BringWindowToTop, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    let hwnd = hwnd_raw as HWND;
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd) != 0
    }
}

/// Acha a janela do iRacing (simulador ou launcher) e a traz para frente, para o
/// jogador "cair" no iRacing logo após exportar os dados. Como o app é a janela em
/// foco quando o usuário clica no toast, o Windows permite passar o foreground.
/// `Ok(false)` = iRacing fechado (janela não encontrada).
pub fn focus_iracing_window() -> Result<bool, IracingError> {
    match find_iracing_hwnd() {
        Some(hwnd) => {
            bring_iracing_to_front(hwnd);
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Força a janela `hwnd_raw` para o primeiro plano. O Windows só deixa o
/// processo em foco trocar o foreground; quando o app está em segundo plano
/// (caso do iRacing fechando), anexamos o input da nossa thread à thread da
/// janela em foco (`AttachThreadInput`) para o `SetForegroundWindow` valer.
pub fn force_foreground_window(hwnd_raw: isize) {
    use winapi::shared::windef::HWND;
    use winapi::um::processthreadsapi::GetCurrentThreadId;
    use winapi::um::winuser::{
        AttachThreadInput, BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId,
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };

    let hwnd = hwnd_raw as HWND;
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let foreground = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
        let our_thread = GetCurrentThreadId();
        let attached = fg_thread != 0
            && fg_thread != our_thread
            && AttachThreadInput(our_thread, fg_thread, 1) != 0;

        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
        }
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);

        if attached {
            AttachThreadInput(our_thread, fg_thread, 0);
        }
    }
}

/// `true` se a janela em foco agora tem "iracing" no título.
pub fn foreground_is_iracing() -> bool {
    use winapi::um::winuser::{GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return false;
        }
        let mut buf: Vec<u16> = vec![0; (len + 1) as usize];
        let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
        if got <= 0 {
            return false;
        }
        buf.truncate(got as usize);
        String::from_utf16_lossy(&buf)
            .to_lowercase()
            .contains("iracing")
    }
}
