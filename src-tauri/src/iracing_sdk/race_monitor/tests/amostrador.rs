//! Testes do AMOSTRADOR (ver [`super::super::amostrador`]).
//!
//! Só há um caso aqui, e ele é sobre o lock. O resto do amostrador é o laço de 60 Hz, que não
//! se testa sem SDK.

use serial_test::serial;

use super::super::amostrador::despachar_um_comando;
use super::super::{lock, MONITOR};

/// O comando de quebra tem de sair com o monitor SOLTO, e a linha de falha tem de conseguir
/// pegá-lo de volta depois.
///
/// O defeito era um autodeadlock. O guard de um `if let` vive até o fim do corpo do `if`, então
/// `if let Some(cmd) = lock().take_one_breakdown_cmd() { … }` mantinha o monitor na mão durante
/// o envio inteiro — e o `lock()` da linha de falha, dentro daquele mesmo corpo, pedia um mutex
/// que a própria thread já segurava. A thread do amostrador parava de vez, e parava exatamente
/// no caso que o aviso âmbar existe para cobrir: sim em fullscreen exclusivo, `SendInput`
/// recusado.
///
/// A conferência é por `try_lock` e não por `lock`: pedir o lock aqui responderia à pergunta
/// travando o teste em vez de reprovando-o, e teste que trava não diz o que quebrou.
#[test]
#[serial]
fn o_comando_de_quebra_sai_com_o_lock_solto() {
    {
        let mut m = lock();
        m.pending_breakdown_cmds = vec!["!black #42 1".to_string()];
        m.chat_send_warned = false;
    }

    let mut enviado = None;
    let mut livre_durante_o_envio = false;
    despachar_um_comando(|cmd| {
        enviado = Some(cmd.to_string());
        livre_durante_o_envio = MONITOR.try_lock().is_ok();
        Err::<(), _>("janela do simulador não encontrada")
    });

    assert_eq!(enviado.as_deref(), Some("!black #42 1"));
    assert!(
        livre_durante_o_envio,
        "o envio rodou com o monitor na mão — o `send_chat_text` foca a janela e digita, e nesse \
         intervalo todo `invoke` do overlay fica esperando"
    );

    let mut m = lock();
    assert!(
        m.chat_send_warned,
        "a falha não foi registrada: o reacquire depois do envio não aconteceu"
    );
    assert!(
        m.pending_breakdown_cmds.is_empty(),
        "o comando saiu da fila uma vez só"
    );
    // O monitor é global e a suíte inteira o compartilha: o latch fica como estava.
    m.chat_send_warned = false;
}

/// Envio que dá certo não arma o aviso âmbar. É o caminho normal, e ele também roda com o lock
/// solto — o defeito ficava invisível justamente porque ninguém pedia o lock de volta aqui.
#[test]
#[serial]
fn envio_bem_sucedido_nao_arma_o_aviso() {
    {
        let mut m = lock();
        m.pending_breakdown_cmds = vec!["!black #7 1".to_string()];
        m.chat_send_warned = false;
    }

    let mut livre_durante_o_envio = false;
    despachar_um_comando(|_cmd| {
        livre_durante_o_envio = MONITOR.try_lock().is_ok();
        Ok::<(), &str>(())
    });

    assert!(livre_durante_o_envio);
    assert!(!lock().chat_send_warned);
}

/// Fila vazia não chama o envio. Sem isso o amostrador roubaria o foco do sim a cada 1,5 s.
#[test]
#[serial]
fn fila_vazia_nao_chama_o_envio() {
    lock().pending_breakdown_cmds.clear();

    let mut chamou = false;
    despachar_um_comando(|_cmd| {
        chamou = true;
        Ok::<(), &str>(())
    });

    assert!(!chamou);
}
