//! Leitura de BOTÕES de volante (joystick do Windows).
//!
//! Existe porque a tecla de recentro do VR só entendia TECLADO: a captura no front
//! ouvia `keydown` e a layer OpenXR perguntava por `GetAsyncKeyState(vk)`. Botão de
//! volante é HID/DirectInput — não gera evento de teclado nem tem Virtual-Key, então
//! as duas pontas eram cegas pra ele.
//!
//! Usamos a API `joyGetPosEx` (winmm). É a mais antiga das três (winmm → DirectInput →
//! XInput) e por isso a mais compatível: qualquer volante que apareça em "Dispositivos
//! de Jogo" do Windows responde por ela, sem SDK de fabricante. O teto é 16 dispositivos
//! e 32 botões por dispositivo, de sobra pra um volante.
//!
//! Fora do Windows compila como stub inerte, igual ao resto da integração com o iRacing.

use serde::{Deserialize, Serialize};

/// Um botão específico de um volante. `dispositivo` é o índice do joystick no Windows
/// (0..15) e `botao` o número do botão (0..31) — os dois juntos são a "tecla".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotaoVolante {
    pub dispositivo: u32,
    pub botao: u32,
}

#[cfg(windows)]
mod imp {
    use super::BotaoVolante;

    // Binding manual do winmm, como já é feito com a shared memory do iRacing: o
    // winapi 0.3 não expõe a API de joystick (não tem a feature `joystickapi`), e
    // trocar de crate por duas funções sairia mais caro que declará-las aqui.
    #[repr(C)]
    #[derive(Default)]
    struct JoyInfoEx {
        dw_size: u32,
        dw_flags: u32,
        dw_xpos: u32,
        dw_ypos: u32,
        dw_zpos: u32,
        dw_rpos: u32,
        dw_upos: u32,
        dw_vpos: u32,
        dw_buttons: u32,
        dw_button_number: u32,
        dw_pov: u32,
        dw_reserved1: u32,
        dw_reserved2: u32,
    }

    #[link(name = "winmm")]
    extern "system" {
        fn joyGetNumDevs() -> u32;
        fn joyGetPosEx(u_joy_id: u32, pji: *mut JoyInfoEx) -> u32;
    }

    const JOYERR_NOERROR: u32 = 0;
    const JOY_RETURNBUTTONS: u32 = 0x0000_0080;

    /// Teto da API antiga: 16 dispositivos, 32 botões cada.
    const MAX_DISPOSITIVOS: u32 = 16;
    const MAX_BOTOES: u32 = 32;

    /// Máscara de botões pressionados do dispositivo, ou `None` se ele não existe /
    /// está desconectado. Um bit por botão.
    fn botoes_de(dispositivo: u32) -> Option<u32> {
        let mut info = JoyInfoEx {
            dw_size: std::mem::size_of::<JoyInfoEx>() as u32,
            // Só os botões: pedir eixos/POV faria o driver trabalhar à toa, e este
            // código roda em laço de vigia.
            dw_flags: JOY_RETURNBUTTONS,
            ..Default::default()
        };
        let r = unsafe { joyGetPosEx(dispositivo, &mut info) };
        (r == JOYERR_NOERROR).then_some(info.dw_buttons)
    }

    /// Índices dos dispositivos conectados AGORA. `joyGetNumDevs` devolve quantos o
    /// driver suporta (costuma ser 16), não quantos existem — quem responde de fato só
    /// se descobre consultando um a um.
    pub fn dispositivos_conectados() -> Vec<u32> {
        let teto = unsafe { joyGetNumDevs() }.min(MAX_DISPOSITIVOS);
        (0..teto).filter(|d| botoes_de(*d).is_some()).collect()
    }

    /// O primeiro botão pressionado agora, varrendo todos os dispositivos. É o que a
    /// captura da interface usa: o jogador aperta, isto responde qual foi.
    pub fn botao_pressionado() -> Option<BotaoVolante> {
        for dispositivo in dispositivos_conectados() {
            let Some(mascara) = botoes_de(dispositivo) else {
                continue;
            };
            if mascara == 0 {
                continue;
            }
            for botao in 0..MAX_BOTOES {
                if mascara & (1 << botao) != 0 {
                    return Some(BotaoVolante { dispositivo, botao });
                }
            }
        }
        None
    }

    /// Aquele botão está apertado agora? Dispositivo sumido conta como solto — se o
    /// volante for desligado no meio da corrida, o vigia não trava com o botão preso.
    pub fn esta_pressionado(b: BotaoVolante) -> bool {
        if b.botao >= MAX_BOTOES {
            return false;
        }
        botoes_de(b.dispositivo).is_some_and(|m| m & (1 << b.botao) != 0)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::BotaoVolante;

    pub fn dispositivos_conectados() -> Vec<u32> {
        Vec::new()
    }
    pub fn botao_pressionado() -> Option<BotaoVolante> {
        None
    }
    pub fn esta_pressionado(_b: BotaoVolante) -> bool {
        false
    }
}

pub use imp::{botao_pressionado, dispositivos_conectados, esta_pressionado};

#[cfg(test)]
mod tests {
    use super::*;

    /// Diagnóstico, não asserção: a máquina do CI não tem volante, e a do
    /// desenvolvedor pode ter. O que este teste garante é que a chamada à API não
    /// explode nem trava — o que ela ENCONTRA sai no `--nocapture`.
    #[test]
    fn enumerar_dispositivos_nao_estoura() {
        let ds = dispositivos_conectados();
        println!("dispositivos de jogo conectados: {ds:?}");
        for d in &ds {
            println!("  dispositivo {d}: pressionado agora = {:?}", botao_pressionado());
        }
        assert!(ds.len() <= 16);
    }

    #[test]
    fn botao_invalido_nunca_conta_como_pressionado() {
        // Botão acima do teto da API e dispositivo que não existe: os dois têm de
        // responder "solto", nunca entrar em pânico.
        assert!(!esta_pressionado(BotaoVolante { dispositivo: 0, botao: 99 }));
        assert!(!esta_pressionado(BotaoVolante { dispositivo: 15, botao: 0 }));
    }
}
