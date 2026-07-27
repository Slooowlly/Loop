//! Diagnóstico cruzado da conexão com o iRacing (Windows).
//!
//! A pergunta que este arquivo responde é "por que não está vindo telemetria?",
//! e ela não tem resposta olhando um sinal só. `OpenFileMappingW` devolvendo nulo
//! é ambíguo por natureza: pode ser sim fechado (normal), sim aberto sem publicar
//! o SDK, ou permissão negada. Cruzando com a JANELA do simulador — que vem de
//! `EnumWindows` e não depende de permissão sobre o objeto de memória — os casos
//! se separam e viram um [`Veredito`] acionável.

use winapi::um::handleapi::CloseHandle;
use winapi::um::memoryapi::{MapViewOfFile, UnmapViewOfFile, FILE_MAP_READ};

use super::janela::find_iracing_janela;
use super::leitura::{abrir_mapeamento, ERRO_ACESSO_NEGADO};
use super::util::read_i32;
use crate::iracing_sdk::{header, ticks_observados, DiagnosticoIracing, Veredito, STATUS_CONNECTED};

/// Score de janela que corresponde ao SIMULADOR (a UI/launcher é 1).
const SCORE_SIMULADOR: i32 = 2;

pub fn diagnosticar() -> DiagnosticoIracing {
    let (janela_encontrada, janela_simulador) = match find_iracing_janela() {
        Some((_, score)) => (true, score >= SCORE_SIMULADOR),
        None => (false, false),
    };

    let mut memoria_ok = false;
    let mut memoria_nome = None;
    let mut memoria_erro = 0u32;
    let mut status = None;
    let mut num_vars = None;
    let mut session_info_len = None;

    unsafe {
        match abrir_mapeamento() {
            Ok((mapping, nome)) => {
                memoria_nome = Some(nome.to_string());
                let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
                if view.is_null() {
                    memoria_erro = winapi::um::errhandlingapi::GetLastError();
                } else {
                    memoria_ok = true;
                    let base = view as *const u8;
                    status = Some(read_i32(base, header::STATUS));
                    num_vars = Some(read_i32(base, header::NUM_VARS));
                    session_info_len = Some(read_i32(base, header::SESSION_INFO_LEN));
                    UnmapViewOfFile(view);
                }
                CloseHandle(mapping);
            }
            Err(crate::iracing_sdk::IracingError::AccessDenied(codigo)) => memoria_erro = codigo,
            // Qualquer outra falha aqui é o "não encontrado" do `abrir_mapeamento`.
            Err(_) => memoria_erro = super::leitura::ERRO_NAO_ENCONTRADO,
        }
    }

    let conectado = status.map(|s| s & STATUS_CONNECTED != 0).unwrap_or(false);
    let veredito = if memoria_ok && conectado {
        Veredito::Ok
    } else if memoria_ok {
        // Abriu mas o bit de conectado não está de pé: o sim está subindo ou
        // saindo de uma sessão. Transitório, não é falha de instalação.
        Veredito::SessaoNaoPronta
    } else if memoria_erro == ERRO_ACESSO_NEGADO {
        Veredito::AcessoNegado
    } else if janela_simulador {
        Veredito::SimSemSdk
    } else if janela_encontrada {
        Veredito::SoLauncher
    } else {
        Veredito::SimFechado
    };

    DiagnosticoIracing {
        veredito,
        memoria_ok,
        memoria_nome,
        memoria_erro,
        janela_encontrada,
        janela_simulador,
        status,
        num_vars,
        session_info_len,
        elevado: elevado(),
        ticks_observados: ticks_observados(),
        log_caminho: crate::diagnostico::caminho().map(|p| p.to_string_lossy().to_string()),
    }
}

/// O processo está rodando elevado (administrador). Só informativo: o que importa
/// é a DIFERENÇA entre nós e o sim, e essa a gente infere do acesso negado — mas
/// ver o valor no relato do jogador economiza uma pergunta.
fn elevado() -> bool {
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
    use winapi::um::securitybaseapi::GetTokenInformation;
    use winapi::um::winnt::{TokenElevation, HANDLE, TOKEN_ELEVATION, TOKEN_QUERY};

    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut info = TOKEN_ELEVATION {
            TokenIsElevated: 0,
        };
        let mut tamanho = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut info as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut tamanho,
        );
        CloseHandle(token);
        ok != 0 && info.TokenIsElevated != 0
    }
}
