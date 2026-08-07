//! AVISO PESSOAL (peça do jogador na zona de risco) + banner de chat bloqueado.

use super::demo::demo_player_warnings;
use super::tipos::BreakdownMessage;
use crate::iracing_sdk::race_monitor;

// ───────────────────── AVISO PESSOAL (peça do jogador na zona de risco) ─────────────────────

/// Peça com preposição na 2ª pessoa ("no seu motor", "na sua suspensão") — pro engenheiro.
///
/// A redação (e a gravação) vivem em [`crate::engenheiro::peca_propria`], junto com o resto do
/// acervo de voz. Aqui só se resolve o card.
pub(crate) use crate::engenheiro::peca_propria::part_com_seu;

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
pub(crate) fn player_breakdown_msg(part_key: &str, severidade: &str, id: usize) -> BreakdownMessage {
    use crate::engenheiro::peca_propria as pp;
    BreakdownMessage {
        id,
        severity: severidade.to_string(),
        text: pp::desfecho_frase(part_key, severidade, id),
        // O detalhe escrito nomeia a peça de novo, com o artigo da 3ª pessoa, porque a linha de
        // baixo do card é legenda e não fala.
        detail: format!("problema {}", crate::engenheiro::quebra::part_com_artigo(part_key)),
        // Frase INTEIRA, uma peça só — mesmo argumento do aviso: aqui o sujeito é sempre você.
        pecas: vec![pp::chave_desfecho(part_key, severidade, id)],
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
    Ok(warns
        .iter()
        .enumerate()
        .map(|(i, w)| match w.tipo {
            race_monitor::TipoAvisoProprio::Peca => player_warning_msg(w.part, i),
            race_monitor::TipoAvisoProprio::Poupar => poupar_msg(i),
            race_monitor::TipoAvisoProprio::Quebra => {
                player_breakdown_msg(w.part, w.severidade, i)
            }
        })
        .collect())
}

/// `true` se algum comando de quebra (`!black`/`!dq`) falhou em chegar ao iRacing nesta
/// corrida (janela não encontrada / foreground recusado). É estado LATCH por corrida — não
/// um stream de eventos —, então o overlay o consome como banner booleano persistente
/// (canal separado dos avisos de peça, que são stream por id) para não mascará-los.
#[tauri::command]
pub fn iracing_chat_blocked() -> bool {
    race_monitor::chat_send_blocked()
}
