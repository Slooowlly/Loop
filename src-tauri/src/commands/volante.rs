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

/// O vigia já foi ligado neste processo? Fica no escopo do módulo (e não dentro de
/// `garantir_vigia`) pra que o teste possa afirmar que a suíte NÃO subiu a thread.
static VIGIA_LIGADO: AtomicBool = AtomicBool::new(false);

/// Liga o vigia na primeira vez que alguém associa um botão. Fica vivo até o app
/// fechar — parar e recriar a thread a cada troca de ligação renderia mais estado
/// pra sincronizar do que o laço custa parado (com tudo `None` ele só dorme).
fn garantir_vigia() {
    if VIGIA_LIGADO.swap(true, Ordering::SeqCst) {
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

/// Resolve o alvo e guarda a ligação. É a parte SEM efeito de mundo da associação:
/// não sobe o vigia, não fala com DirectInput. Separada do comando justamente pra que
/// o teste exercite a regra (alvo válido, alvo certo, desassociar) sem acordar a thread
/// eterna de polling — que, uma vez ligada, viveria pelo resto do processo de teste.
fn definir_ligacao(alvo: &str, botao: Option<BotaoVolante>) -> Result<(), String> {
    let i = indice(alvo)?;
    ligacoes().lock().unwrap()[i] = botao;
    Ok(())
}

/// Associa (ou desassocia, com `botao: None`) um botão ao recentro do alvo.
#[tauri::command]
pub fn volante_set_recenter_button(
    alvo: String,
    botao: Option<BotaoVolante>,
) -> Result<(), String> {
    definir_ligacao(&alvo, botao)?;
    garantir_vigia();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// As ligações são estado GLOBAL do processo. Cada caso zera o array antes de
    /// medir e `#[serial]` impede que dois casos disputem o mesmo `Mutex` — sem isso
    /// a asserção "o outro alvo continua vazio" quebraria por contaminação, não por bug.
    fn zerar_ligacoes() {
        *ligacoes().lock().unwrap() = [None, None];
    }

    #[test]
    #[serial]
    fn alvo_desconhecido_e_erro_e_nao_mexe_em_nada() {
        zerar_ligacoes();
        assert!(definir_ligacao("cozinha", None).is_err());
        assert_eq!(*ligacoes().lock().unwrap(), [None, None]);
    }

    #[test]
    #[serial]
    fn associar_e_desassociar_guarda_o_botao_do_alvo_certo() {
        zerar_ligacoes();
        let b = BotaoVolante {
            dispositivo: 1,
            botao: 7,
        };
        definir_ligacao("engineer", Some(b)).unwrap();
        assert_eq!(ligacoes().lock().unwrap()[1], Some(b));
        // O outro alvo não pode ter sido tocado.
        assert_eq!(ligacoes().lock().unwrap()[0], None);

        definir_ligacao("engineer", None).unwrap();
        assert_eq!(ligacoes().lock().unwrap()[1], None);
    }

    /// O vigia é o único ponto que conversa com DirectInput. Nenhum caso desta suíte
    /// pode chamá-lo — o guard existe pra que religar `volante_set_recenter_button`
    /// aqui dentro apareça como falha, e não como uma thread silenciosa vazando.
    #[test]
    #[serial]
    fn a_suite_nao_liga_o_vigia() {
        zerar_ligacoes();
        definir_ligacao("overlay", None).unwrap();
        assert!(
            !VIGIA_LIGADO.load(Ordering::SeqCst),
            "algum caso chamou garantir_vigia e deixou a thread de polling viva"
        );
    }
}
