//! Botão de volante como atalho de RECENTRO do VR.
//!
//! A tecla de recentro que já existia só entendia teclado, dos dois lados: a captura
//! no front ouvia `keydown` e a layer OpenXR perguntava por `GetAsyncKeyState(vk)`.
//! Botão de volante não passa por nenhum dos dois.
//!
//! Em vez de ensinar a layer C++ a ler joystick, o vigia mora AQUI e reaproveita o
//! caminho que o botão "recentrar" da interface já usa (`bump_recenter`, via
//! `recenterSeq` na memória compartilhada). Sai mais barato, não toca no C++, e
//! funciona com o app em segundo plano — que é o caso de uso inteiro: o jogador está
//! de óculos, dentro do iRacing.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::volante::{self, BotaoVolante};

/// Alvos possíveis do recentro, na mesma ordem do array de ligações.
const ALVOS: [&str; 2] = ["overlay", "engineer"];

fn ligacoes() -> &'static Mutex<[Option<BotaoVolante>; 2]> {
    static L: OnceLock<Mutex<[Option<BotaoVolante>; 2]>> = OnceLock::new();
    L.get_or_init(|| Mutex::new([None, None]))
}

/// Intervalo do vigia. 30 ms é rápido o bastante pra não perder um toque curto e
/// devagar o bastante pra não pesar: cada volta é uma consulta por dispositivo ligado.
const VIGIA_MS: u64 = 30;

/// Liga o vigia na primeira vez que alguém associa um botão. Fica vivo até o app
/// fechar — parar e recriar a thread a cada troca de ligação renderia mais estado
/// pra sincronizar do que o laço custa parado (com tudo `None` ele só dorme).
fn garantir_vigia() {
    static LIGADO: AtomicBool = AtomicBool::new(false);
    if LIGADO.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        // Estado do quadro anterior, pra disparar só na BORDA de descida. Sem isso um
        // botão segurado viraria uma enxurrada de recentros.
        let mut antes = [false; ALVOS.len()];
        loop {
            let atual = *ligacoes().lock().unwrap();
            for (i, ligacao) in atual.iter().enumerate() {
                let agora = ligacao.is_some_and(volante::esta_pressionado);
                if agora && !antes[i] {
                    let r = match i {
                        0 => crate::commands::vr_overlay::vr_overlay_recenter(),
                        _ => crate::commands::vr_overlay::vr_engineer_recenter(),
                    };
                    // Sem overlay de VR no ar o recentro falha — é o caso normal de
                    // quem joga em monitor, não erro que mereça barulho.
                    let _ = r;
                }
                antes[i] = agora;
            }
            std::thread::sleep(std::time::Duration::from_millis(VIGIA_MS));
        }
    });
}

fn indice(alvo: &str) -> Result<usize, String> {
    ALVOS
        .iter()
        .position(|a| *a == alvo)
        .ok_or_else(|| format!("alvo desconhecido: {alvo}"))
}

/// Qual botão está pressionado AGORA, em qualquer volante. A interface chama isto em
/// laço enquanto está "capturando": o jogador aperta e o painel descobre qual foi.
#[tauri::command]
pub fn volante_botao_pressionado() -> Option<BotaoVolante> {
    volante::botao_pressionado()
}

/// Dispositivos de jogo vistos pelo Windows. Só diagnóstico — lista vazia é a
/// diferença entre "nenhum volante ligado" e "o botão não foi reconhecido".
#[tauri::command]
pub fn volante_dispositivos() -> Vec<u32> {
    volante::dispositivos_conectados()
}

/// Associa (ou desassocia, com `botao: None`) um botão ao recentro do alvo.
#[tauri::command]
pub fn volante_set_recenter_button(
    alvo: String,
    botao: Option<BotaoVolante>,
) -> Result<(), String> {
    let i = indice(&alvo)?;
    ligacoes().lock().unwrap()[i] = botao;
    garantir_vigia();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alvo_desconhecido_e_erro_e_nao_mexe_em_nada() {
        assert!(volante_set_recenter_button("cozinha".into(), None).is_err());
    }

    #[test]
    fn associar_e_desassociar_guarda_o_botao_do_alvo_certo() {
        let b = BotaoVolante {
            dispositivo: 1,
            botao: 7,
        };
        volante_set_recenter_button("engineer".into(), Some(b)).unwrap();
        assert_eq!(ligacoes().lock().unwrap()[1], Some(b));
        // O outro alvo não pode ter sido tocado.
        assert_eq!(ligacoes().lock().unwrap()[0], None);

        volante_set_recenter_button("engineer".into(), None).unwrap();
        assert_eq!(ligacoes().lock().unwrap()[1], None);
    }
}
