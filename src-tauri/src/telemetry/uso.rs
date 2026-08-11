//! O USO do app por fim de semana: quanto tempo nas telas que custam IA e quantos
//! apertos de push-to-talk, acumulado por rodada num arquivo próprio.
//!
//! Vive fora da fachada porque tem ciclo de vida próprio — o bloco atravessa o
//! fechamento do app e só fecha quando a RODADA vira, e não quando a corrida acaba.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{is_enabled, send};

/// Quanto tempo o jogador passou nas telas que CUSTAM geração de IA, mais o uso do
/// push-to-talk, acumulado por rodada.
///
/// Por que só três telas: notícias, briefing e debriefing são as que consomem o
/// servidor de IA. Calendário, tabela e ficha de piloto não custam nada, e medir a
/// permanência nelas seria vigiar por vigiar.
///
/// Por que por RODADA e não por sessão: o número sozinho ("12 minutos de notícias")
/// não decide nada. Cruzado com a corrida, ele responde se a matéria que foi gerada
/// para aquela etapa chegou a ser lida — que é a pergunta que paga a conta.
///
/// O bloco fecha quando a rodada VIRA. O que se mede é a janela entre uma corrida e a
/// seguinte: o debriefing da rodada N, as notícias da semana e o briefing da N+1 caem
/// todos no bloco rotulado N. Fechar no `race_end` perderia justamente o debriefing,
/// que só é lido depois.
#[derive(Serialize, Deserialize, Clone, Default)]
struct UsoRodada {
    /// `None` até a primeira corrida da instalação. O tempo lido antes disso não
    /// pertence a rodada nenhuma.
    temporada: Option<i32>,
    rodada: Option<i32>,
    categoria: Option<String>,
    #[serde(default)]
    segundos_noticias: i64,
    #[serde(default)]
    segundos_briefing: i64,
    #[serde(default)]
    segundos_debriefing: i64,
    /// Apertos que abriram o microfone.
    #[serde(default)]
    ptt_apertos: i64,
    /// Apertos que viraram pergunta de verdade. A diferença para os apertos é o que
    /// foi descartado como toque acidental ou cortado pelo freio do rádio, e é
    /// exatamente o aperto que NÃO custou transcrição nem geração.
    #[serde(default)]
    ptt_perguntas: i64,
}

impl UsoRodada {
    /// Só o rótulo da rodada, sem um segundo lido nem um aperto. É o estado de quem
    /// abriu o app e fechou sem ler nada.
    fn vazio(&self) -> bool {
        self.segundos_noticias == 0
            && self.segundos_briefing == 0
            && self.segundos_debriefing == 0
            && self.ptt_apertos == 0
    }
}

/// Arquivo do acumulado corrente. Em disco, e não só em memória, porque o bloco
/// atravessa o fechamento do app: quem lê o debriefing e fecha o Loop não pode levar
/// esse tempo embora.
static USO: OnceLock<PathBuf> = OnceLock::new();

/// Define o arquivo do acumulado. Uma vez só, no `init` do boot.
pub(super) fn definir_arquivo(caminho: PathBuf) {
    let _ = USO.set(caminho);
}
static USO_TRAVA: Mutex<()> = Mutex::new(());

/// Somou tempo numa das três telas. `tela` é `noticias` | `briefing` | `debriefing`;
/// qualquer outra coisa é ignorada em silêncio, para o front não conseguir inventar
/// uma métrica nova sem passar por aqui.
pub fn uso_tela(tela: &str, segundos: i64) {
    if !is_enabled() || segundos <= 0 {
        return;
    }
    com_uso(|uso| match tela {
        "noticias" => uso.segundos_noticias += segundos,
        "briefing" => uso.segundos_briefing += segundos,
        "debriefing" => uso.segundos_debriefing += segundos,
        // O descarte é intencional (o front não inventa métrica), e passou a ser
        // VISÍVEL: uma tela nova com o nome errado sumia sem erro e sem linha de log,
        // e o buraco na medição só aparecia semanas depois, no painel.
        outra => crate::diagnostico::linha(
            "telemetria",
            &format!(
                "tela '{outra}' não está entre as medidas ({}) — {segundos}s descartados",
                TELAS_MEDIDAS.join(", ")
            ),
        ),
    });
}

/// As três telas que custam geração de IA. Fora desta lista, nada é medido.
pub(crate) const TELAS_MEDIDAS: [&str; 3] = ["noticias", "briefing", "debriefing"];

/// O jogador apertou o push-to-talk. Contado na borda física do botão, então inclui
/// o toque acidental e o aperto que o freio do rádio vai cortar.
pub fn uso_ptt_aperto() {
    if !is_enabled() {
        return;
    }
    com_uso(|uso| uso.ptt_apertos += 1);
}

/// O aperto virou pergunta de verdade e foi ao servidor. É este que custa dinheiro:
/// transcrição mais geração da resposta.
pub fn uso_ptt_pergunta() {
    if !is_enabled() {
        return;
    }
    com_uso(|uso| uso.ptt_perguntas += 1);
}

/// Uma corrida terminou (dirigida ou simulada). Fecha o bloco anterior, manda, e abre
/// um novo rotulado com esta rodada.
///
/// Chamada DEPOIS do `race_end`/`race_sim` de propósito: o evento de uso é sobre a
/// leitura em volta da corrida, e não sobre a corrida.
pub fn uso_virar_rodada(temporada: i32, rodada: i32, categoria: &str) {
    if !is_enabled() {
        return;
    }
    let anterior = {
        let Ok(_trava) = USO_TRAVA.lock() else {
            return;
        };
        let atual = ler_uso();
        // Mesma rodada de novo (reimportação, corrida refeita): não fecha bloco, senão
        // a mesma janela de leitura viraria dois eventos com metade do tempo cada.
        if atual.temporada == Some(temporada) && atual.rodada == Some(rodada) {
            return;
        }
        gravar_uso(&UsoRodada {
            temporada: Some(temporada),
            rodada: Some(rodada),
            categoria: Some(categoria.to_string()),
            ..Default::default()
        });
        atual
    };
    enviar_uso(anterior);
}

/// Manda o que sobrou da sessão anterior. Chamada no boot, junto da drenagem da fila.
///
/// É isto que fecha o buraco do fechamento do app: o bloco fica no disco e vira evento
/// na próxima abertura, com o `atraso_s` dizendo quanto tempo ele esperou.
pub fn uso_enviar_pendente() {
    if !is_enabled() {
        return;
    }
    let pendente = {
        let Ok(_trava) = USO_TRAVA.lock() else {
            return;
        };
        let atual = ler_uso();
        // Preserva o rótulo da rodada: o jogador reabre o app na mesma etapa em que
        // parou, e o tempo que ele ainda vai gastar lendo pertence a ela.
        gravar_uso(&UsoRodada {
            temporada: atual.temporada,
            rodada: atual.rodada,
            categoria: atual.categoria.clone(),
            ..Default::default()
        });
        atual
    };
    enviar_uso(pendente);
}

fn enviar_uso(uso: UsoRodada) {
    // Bloco sem nenhum segundo e sem nenhum aperto não descreve nada. Acontece toda
    // vez que alguém abre o app e fecha sem ler, e mandar isso seria ruído puro.
    if uso.vazio() {
        return;
    }
    let mut payload = json!({
        "segundos_noticias": uso.segundos_noticias,
        "segundos_briefing": uso.segundos_briefing,
        "segundos_debriefing": uso.segundos_debriefing,
        "ptt_apertos": uso.ptt_apertos,
        "ptt_perguntas": uso.ptt_perguntas,
    });
    if let Some(map) = payload.as_object_mut() {
        if let Some(t) = uso.temporada {
            map.insert("temporada".into(), json!(t));
        }
        if let Some(r) = uso.rodada {
            map.insert("rodada".into(), json!(r));
        }
        if let Some(c) = uso.categoria {
            map.insert("categoria_rodada".into(), json!(c));
        }
    }
    send("weekend_usage", payload);
}

/// Lê, deixa o chamador mexer, grava. Toda escrita do acumulado passa por aqui.
fn com_uso(f: impl FnOnce(&mut UsoRodada)) {
    let Ok(_trava) = USO_TRAVA.lock() else {
        return;
    };
    let mut uso = ler_uso();
    f(&mut uso);
    gravar_uso(&uso);
}

fn ler_uso() -> UsoRodada {
    USO.get()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn gravar_uso(uso: &UsoRodada) {
    let Some(path) = USO.get() else {
        return;
    };
    if let (Some(dir), Ok(texto)) = (path.parent(), serde_json::to_string(uso)) {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(path, texto);
    }
}

/// Apaga o acumulado. Chamado quando o consentimento cai.
pub(super) fn apagar() {
    let Some(path) = USO.get() else {
        return;
    };
    let Ok(_trava) = USO_TRAVA.lock() else {
        return;
    };
    let _ = std::fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O acumulado de uso vive num estático de processo (`USO`), então os testes que o
    /// tocariam de verdade seriam serializados entre si e frágeis. O que dá para testar
    /// sem isso é a regra que decide o que vira evento, e é ela que erra na prática:
    /// bloco vazio nunca deve virar `weekend_usage`.
    #[test]
    fn bloco_sem_leitura_nao_vira_evento() {
        let so_rotulo = UsoRodada {
            temporada: Some(1),
            rodada: Some(3),
            categoria: Some("mazda_rookie".into()),
            ..Default::default()
        };
        assert!(so_rotulo.vazio(), "abriu o app e fechou sem ler: nada a dizer");

        let leu = UsoRodada {
            segundos_debriefing: 4,
            ..so_rotulo.clone()
        };
        assert!(!leu.vazio());

        let so_apertou = UsoRodada {
            ptt_apertos: 1,
            ..so_rotulo
        };
        assert!(
            !so_apertou.vazio(),
            "aperto de PTT sem leitura nenhuma ainda é uso"
        );
    }

    /// Campo novo no meio da vida do arquivo não pode zerar o que já estava lá: o
    /// acumulado sobrevive a um update do app com o jogador no meio de uma rodada.
    #[test]
    fn acumulado_antigo_sobrevive_a_campo_novo() {
        let antigo = r#"{"temporada":2,"rodada":5,"segundos_noticias":90}"#;
        let uso: UsoRodada = serde_json::from_str(antigo).expect("ler acumulado antigo");

        assert_eq!(uso.segundos_noticias, 90);
        assert_eq!(uso.rodada, Some(5));
        assert_eq!(uso.ptt_perguntas, 0);
        assert_eq!(uso.categoria, None);
    }

    /// A tela desconhecida é descartada de propósito, para o front não inventar métrica
    /// nova sem passar por aqui. O descarte não pode, porém, ser INVISÍVEL: uma tela
    /// nova com o nome errado sumia sem erro e sem linha de log, e ninguém descobria.
    #[test]
    fn tela_desconhecida_e_reconhecivel_pela_lista() {
        assert!(TELAS_MEDIDAS.contains(&"noticias"));
        assert!(TELAS_MEDIDAS.contains(&"briefing"));
        assert!(TELAS_MEDIDAS.contains(&"debriefing"));
        assert!(!TELAS_MEDIDAS.contains(&"calendario"));
    }
}
