//! AVISO PESSOAL (peça do jogador na zona de risco) + banner de chat bloqueado.

use super::demo::demo_player_warnings;
use super::tipos::BreakdownMessage;
use crate::iracing_sdk::race_monitor;

// ───────────────────── AVISO PESSOAL (peça do jogador na zona de risco) ─────────────────────

/// O detalhe do card — a linha de baixo, que o áudio NÃO fala.
///
/// A fala é a de cima; esta é a explicação escrita, e por isso pode ter travessão e ser mais
/// longa. Separá-las é o que permite ao rádio ser curto sem o card ficar seco.
fn detalhe(variante: usize) -> &'static str {
    [
        "pode dar problema a qualquer momento",
        "fica de olho — pode falhar a qualquer hora",
        "risco de pane a qualquer momento",
    ][variante % 3]
}

/// Monta o AVISO pessoal na VOZ DO ENGENHEIRO (rádio, 1ª pessoa dele → você). Sem número:
/// ele "ouve algo estranho" e alerta que a peça pode dar problema a qualquer momento. 3
/// variações; severidade "warn" → card distinto no front.
pub(crate) fn player_warning_msg(part_key: &str, id: usize) -> BreakdownMessage {
    use crate::engenheiro::peca_propria as pp;
    BreakdownMessage {
        id,
        severity: "warn".to_string(),
        text: pp::aviso_frase(part_key, id),
        detail: detalhe(id).to_string(),
        // Frase INTEIRA, uma peça só. A montagem por fragmentos existe na família de quebra
        // porque lá o sujeito é um de 355 sobrenomes; aqui o sujeito é sempre "você".
        pecas: vec![pp::chave_aviso(part_key, id)],
    }
}

/// Monta o DESFECHO da quebra do NOSSO carro, em 2ª pessoa.
///
/// Severidade real (`light`/`heavy`/`dnf`), e não `"warn"`: é ela que escolhe entre "dá pra
/// seguir" e "acabou por hoje", e é ela que o card usa para decidir a cor. Um abandono pintado
/// como aviso seria a mesma diluição que a fala evita.
pub(crate) fn player_breakdown_msg(
    part_key: &str,
    severidade: &str,
    id: usize,
) -> BreakdownMessage {
    use crate::engenheiro::peca_propria as pp;
    BreakdownMessage {
        id,
        severity: severidade.to_string(),
        text: pp::desfecho_frase(part_key, severidade, id),
        // O detalhe escrito nomeia a peça de novo, com o artigo da 3ª pessoa, porque a linha de
        // baixo do card é legenda e não fala.
        detail: format!(
            "problema {}",
            crate::engenheiro::quebra::part_com_artigo(part_key)
        ),
        // Frase INTEIRA, uma peça só — mesmo argumento do aviso: aqui o sujeito é sempre você.
        pecas: vec![pp::chave_desfecho(part_key, severidade, id)],
    }
}

/// Monta o motivo do castigo por CARRO DESTRUÍDO NA CLASSIFICAÇÃO, em 2ª pessoa e antes do
/// verde. O jogador tem de saber por que largou do fundo (ou por que não largou) no momento
/// em que acontece: descobrir a punição pelo efeito é o que faz parecer bug.
///
/// Card SEM áudio (`pecas` vazio, caso já previsto pelo overlay): é a única fala do rádio que
/// nasce fora da corrida, e o acervo de voz do engenheiro não tem peça gravada para ela.
pub(crate) fn quali_destruida_msg(severidade: &str, id: usize) -> BreakdownMessage {
    use crate::engenheiro::peca_propria as pp;
    // Dois momentos falam por aqui: o LOCKOUT dentro da própria quali (quali_*) — o instante
    // em que o fim de semana muda — e o despacho na largada (eol/dq), que reafirma. A
    // severidade do card gradua a cor: o que ainda corre é "heavy"; o que acabou é "dnf".
    let (sev, detail) = match severidade {
        "quali_grave" => ("heavy", "danos graves encerraram a classificação"),
        "quali_destruido" => ("dnf", "carro destruído na classificação"),
        "quali_catastrofico" => ("dnf", "batida catastrófica na classificação"),
        "dq" => ("dnf", "destruído na classificação"),
        // A legenda do card diz a REGRA que o áudio só insinua: sem ela, "largamos em último"
        // depois de um bom tempo de quali continua parecendo defeito do jogo.
        "eol" => ("heavy", "parque fechado quebrado — largada no fim do grid"),
        _ => ("heavy", "castigo pelo carro destruído na classificação"),
    };
    // O TEXTO vem do acervo, não daqui: é a mesma frase que a peça de voz grava, e duas
    // redações da mesma fala divergiriam no primeiro ajuste. `pecas` fica com a chave —
    // vazio até a gravação existir, e o overlay já trata card sem áudio.
    let chave = format!("meu_quali_{}", severidade.trim_start_matches("quali_"));
    let text = pp::quali_frase(&chave).unwrap_or("O carro não aguenta seguir assim.");
    BreakdownMessage {
        id,
        severity: sev.to_string(),
        text: text.to_string(),
        detail: detail.to_string(),
        pecas: vec![chave],
    }
}

/// Monta o conselho de POUPAR O CARRO. Ver [`crate::engenheiro::peca_propria`] para o gatilho.
pub(crate) fn poupar_msg(id: usize) -> BreakdownMessage {
    let (chave, texto) = crate::engenheiro::peca_propria::poupar_frase(id);
    BreakdownMessage {
        id,
        severity: "warn".to_string(),
        text: texto.to_string(),
        detail: "a pista está cobrando caro hoje".to_string(),
        pecas: vec![chave.to_string()],
    }
}

/// Registra, UMA vez por execução, que alguém está de fato consultando este canal.
///
/// Sem ela o silêncio do rádio era indistinguível de "a janela nunca perguntou" — que foi o
/// caso medido em 2026-08-11: o castigo da classificação apareceu no log do Rust e o canal
/// não tinha um único leitor, porque a janela do rádio só liga quando o app reconhece uma
/// sessão ao vivo (carreira carregada + dados de overlay). Uma linha por execução é barata e
/// separa "não produzi" de "ninguém pediu".
fn registrar_primeira_consulta() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static PRIMEIRA: AtomicBool = AtomicBool::new(true);
    if PRIMEIRA.swap(false, Ordering::Relaxed) {
        crate::diagnostico::linha(
            "overlay",
            "o rádio começou a consultar os avisos do nosso carro",
        );
    }
}

/// Registra no log de arquivo que o aviso CHEGOU ao rádio, uma vez por fala.
///
/// Existe porque o silêncio do overlay é indistinguível de três causas muito diferentes: o
/// backend não produziu a fala, a janela do rádio não está consultando, ou a fala chegou e o
/// áudio não saiu. As duas primeiras o log já separava; esta linha separa a terceira — se ela
/// aparece e o piloto não ouviu nada, o defeito está no front, não aqui.
///
/// Estrangulado pelo ÚLTIMO id entregue, e não por tempo: o overlay consulta a cada 800 ms, e
/// uma linha por consulta afogaria o log com a mesma fala repetida uma dezena de vezes.
fn registrar_entrega(out: &[BreakdownMessage]) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static ULTIMO: AtomicUsize = AtomicUsize::new(usize::MAX);
    registrar_primeira_consulta();
    let Some(ultimo) = out.last() else {
        return;
    };
    if ULTIMO.swap(ultimo.id, Ordering::Relaxed) == ultimo.id {
        return;
    }
    crate::diagnostico::linha(
        "overlay",
        &format!(
            "aviso entregue ao rádio: id={} sev={} pecas={} — \"{}\"",
            ultimo.id,
            ultimo.severity,
            ultimo.pecas.len(),
            ultimo.text
        ),
    );
}

/// AVISOS pessoais do jogador (peça DELE entrou na zona de risco) — o overlay mostra num card
/// DISTINTO (2ª pessoa). Lê o log vivo do monitor. No demo, cicla o tour pelo tempo.
#[tauri::command]
pub fn get_player_warnings() -> Result<Vec<BreakdownMessage>, String> {
    if crate::commands::overlay_window::demo_enabled() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let tour = demo_player_warnings();
        if tour.is_empty() {
            return Ok(Vec::new());
        }
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let step = (secs / 6) as usize; // troca a cada 6 s
        let mut m = tour[step % tour.len()].clone();
        m.id = step;
        return Ok(vec![m]);
    }
    // Os dois tipos vêm no MESMO fluxo, na ordem em que o monitor os produziu — e é a ordem
    // que arbitra: o overlay mostra o de maior `id`, então o conselho de poupar aparece quando
    // ele chegou, sem competir com um canal paralelo por espaço de tela.
    let warns = race_monitor::peek_player_warnings();
    // `id` = base + índice. A base cobre o que já foi descartado por reinícios: o overlay só
    // mostra id INÉDITO, e sem ela o log esvaziado devolvia ids repetidos e o canal emudecia
    // para o resto da sessão. Ver `race_monitor::radio_epoch`.
    let base = race_monitor::radio_epoch();
    let out: Vec<BreakdownMessage> = warns
        .iter()
        .enumerate()
        .map(|(i, w)| (base + i, w))
        .map(|(i, w)| match w.tipo {
            race_monitor::TipoAvisoProprio::Peca => player_warning_msg(w.part, i),
            race_monitor::TipoAvisoProprio::Poupar => poupar_msg(i),
            race_monitor::TipoAvisoProprio::Quebra => player_breakdown_msg(w.part, w.severidade, i),
            race_monitor::TipoAvisoProprio::QualiDestruida => quali_destruida_msg(w.severidade, i),
        })
        .collect();
    registrar_entrega(&out);
    Ok(out)
}

/// `true` se algum comando de quebra (`!black`/`!dq`) falhou em chegar ao iRacing nesta
/// corrida (janela não encontrada / foreground recusado). É estado LATCH por corrida — não
/// um stream de eventos —, então o overlay o consome como banner booleano persistente
/// (canal separado dos avisos de peça, que são stream por id) para não mascará-los.
#[tauri::command]
pub fn iracing_chat_blocked() -> bool {
    race_monitor::chat_send_blocked()
}
