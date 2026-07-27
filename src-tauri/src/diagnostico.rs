//! Log de diagnóstico em ARQUIVO.
//!
//! O app é uma GUI sem console: todo `eprintln!` do backend morre no vazio na
//! máquina do jogador. Quando um problema só acontece no PC de outra pessoa — o
//! caso que motivou este módulo, um beta tester com o SDK do iRacing sem puxar
//! nada — não havia o que pedir a ele além de print de tela, e print de tela não
//! mostra código de erro do Windows.
//!
//! Aqui gravamos um log pequeno e rotativo em `<app_data>/logs/loop.log`: o
//! bastante para reconstruir o que houve (transições de conexão do sim, códigos
//! de erro do `OpenFileMappingW`, o diagnóstico cruzado) e pequeno o bastante
//! para o jogador mandar por mensagem.
//!
//! Regras de ouro deste módulo:
//!
//! - **Nunca falha.** Toda operação de disco é best-effort; um log que derruba o
//!   app é pior que não ter log.
//! - **Nunca no caminho quente.** O sampler roda a 60 Hz; quem loga de lá usa
//!   [`linha_unica`], que só escreve quando a mensagem MUDA.
//! - **Sem dado pessoal.** Caminhos do próprio app e códigos de erro, nada mais.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Tamanho máximo do log antes de rotacionar. Dois arquivos de 512 KiB é o
/// bastante para uma sessão inteira de corrida e ainda cabe num anexo.
const MAX_BYTES: u64 = 512 * 1024;

/// Caminho do arquivo ativo. `None` até o `init()` do boot.
static ARQUIVO: OnceLock<PathBuf> = OnceLock::new();

/// Serializa as escritas (o sampler roda numa thread, os comandos em outra).
static TRAVA: Mutex<()> = Mutex::new(());

/// Última mensagem vista por categoria, para o [`linha_unica`] só registrar
/// TRANSIÇÕES em vez de repetir a mesma linha a cada tick.
static ULTIMA: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn ultima() -> &'static Mutex<HashMap<String, String>> {
    ULTIMA.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Liga o log. Chamado UMA vez no `setup()` do boot, antes do sampler — ele roda
/// numa thread sem `AppHandle` e só enxerga o estático daqui.
///
/// Cria `<base_dir>/logs/` e carimba um cabeçalho com versão e SO, que é o
/// primeiro contexto que se quer ao abrir o arquivo de outra pessoa.
pub fn init(base_dir: &Path) {
    let dir = base_dir.join("logs");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = ARQUIVO.set(dir.join("loop.log"));
    linha(
        "boot",
        &format!(
            "Loop {} ({}) iniciado",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS
        ),
    );
}

/// Caminho do log ativo, para a UI mostrar/abrir. `None` se o `init()` não rodou
/// (ou falhou em criar a pasta).
pub fn caminho() -> Option<PathBuf> {
    ARQUIVO.get().cloned()
}

/// Registra uma linha. Best-effort: qualquer erro de disco é engolido de
/// propósito — o log existe para ajudar a depurar, nunca para atrapalhar.
pub fn linha(categoria: &str, mensagem: &str) {
    let Some(path) = ARQUIVO.get() else {
        // Antes do init (ou sem pasta gravável) ainda vale o console: em `npm run
        // tauri dev` há terminal, e é lá que o desenvolvedor está olhando.
        eprintln!("[{categoria}] {mensagem}");
        return;
    };
    let Ok(_guard) = TRAVA.lock() else {
        return;
    };
    rotacionar_se_grande(path);
    let carimbo = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let Ok(mut arquivo) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = writeln!(arquivo, "{carimbo} [{categoria}] {mensagem}");
}

/// Registra a linha SÓ SE ela for diferente da última daquela categoria.
///
/// É o que o sampler usa: a 60 Hz (ou 1 Hz ocioso) uma linha por tick encheria o
/// arquivo em minutos e afogaria o que importa. Com isto, um sim fechado a tarde
/// inteira gera UMA linha, e a próxima só aparece quando o estado realmente muda
/// — que é exatamente o evento que se quer ver ao abrir o log.
/// Devolve `true` quando de fato escreveu — ou seja, quando houve TRANSIÇÃO. O
/// chamador usa isso para pendurar no instante da mudança o que é caro demais
/// para rodar todo tick (no sampler, o diagnóstico cruzado que varre janelas).
pub fn linha_unica(categoria: &str, mensagem: &str) -> bool {
    {
        let Ok(mut mapa) = ultima().lock() else {
            return false;
        };
        if mapa.get(categoria).map(String::as_str) == Some(mensagem) {
            return false;
        }
        mapa.insert(categoria.to_string(), mensagem.to_string());
    }
    linha(categoria, mensagem);
    true
}

/// Devolve o FINAL do log (últimos `max_bytes`), para a UI exibir e o jogador
/// copiar. Pega o fim porque é onde está o que acabou de acontecer.
pub fn ler_final(max_bytes: usize) -> String {
    let Some(path) = ARQUIVO.get() else {
        return String::new();
    };
    let Ok(conteudo) = std::fs::read(path) else {
        return String::new();
    };
    let inicio = conteudo.len().saturating_sub(max_bytes);
    // `from_utf8_lossy` a partir de um corte por BYTES pode começar no meio de um
    // caractere acentuado; descartar a primeira linha (parcial de qualquer jeito)
    // resolve sem precisar procurar a fronteira do UTF-8.
    let texto = String::from_utf8_lossy(&conteudo[inicio..]).into_owned();
    if inicio == 0 {
        return texto;
    }
    match texto.find('\n') {
        Some(quebra) => texto[quebra + 1..].to_string(),
        None => texto,
    }
}

// ── Envio ao desenvolvedor ──────────────────────────────────────────────────

/// Rota de recebimento de logs, na MESMA porta do resto (prévia de temporada,
/// telemetria). Um host só, um segredo só, um lugar só para trocar.
const ENDPOINT_LOG: &str = "https://iracer-news-124606451488.southamerica-east1.run.app/log";

/// 45s como o cliente de notícias: o Cloud Run faz scale-to-zero e a primeira
/// chamada depois de um tempo ocioso paga o cold start. Aqui é um envio pedido
/// pelo jogador, com spinner na tela — vale esperar em vez de falhar à toa.
const ENVIO_TIMEOUT_SECS: u64 = 45;

/// Teto do que vai no corpo. O final do log é o que interessa, e um corpo
/// gigante só aumenta a chance do envio falhar.
const ENVIO_MAX_BYTES: usize = 60 * 1024;

/// Envia o final do log ao servidor e devolve o TICKET (código curto) que o
/// jogador informa no relato.
///
/// **Nunca é automático.** Só roda quando o jogador aperta o botão: um log traz
/// caminhos de arquivo e o histórico da sessão dele, e isso sai da máquina por
/// decisão dele, não nossa.
///
/// O ticket é gerado no CLIENTE de propósito: assim ele aparece na tela mesmo se
/// a resposta do servidor vier num formato inesperado, e já vai junto no corpo
/// para casar os dois lados.
pub fn enviar(nota: Option<String>, diagnostico: serde_json::Value) -> Result<String, String> {
    let conteudo = redigir(&ler_final(ENVIO_MAX_BYTES));
    if conteudo.trim().is_empty() {
        return Err("Não há log para enviar ainda.".to_string());
    }
    let ticket = uuid::Uuid::new_v4()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_uppercase();

    // As últimas corridas viajam junto: quase todo relato ("o carro quebrou do nada", "a
    // posição ficou errada") só se resolve olhando a corrida em que aconteceu, e pedir o
    // arquivo depois custa uma ida e volta que muita gente não responde. Resumo, não frames
    // crus — ver `race_capture::resumo_para_log`.
    let corridas = crate::iracing_sdk::race_capture::resumo_para_log();

    let corpo = serde_json::json!({
        "ticket": ticket,
        "install_id": crate::telemetry::install_id().unwrap_or_default(),
        "app_version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "nota": nota.unwrap_or_default(),
        "diagnostico": diagnostico,
        "log": conteudo,
        "corridas": corridas,
    });

    let cliente = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(ENVIO_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Falha ao preparar o envio: {e}"))?;

    let resposta = cliente
        .post(ENDPOINT_LOG)
        .header("x-app-secret", crate::narrative::client::APP_SECRET)
        .json(&corpo)
        .send()
        .map_err(|e| format!("Falha ao enviar: {e}"))?;

    let status = resposta.status();
    if !status.is_success() {
        return Err(format!("O servidor recusou o envio (HTTP {status})."));
    }
    linha("diagnostico", &format!("log enviado ao servidor, ticket {ticket}"));
    Ok(ticket)
}

/// Troca o caminho do perfil do usuário por `%USERPROFILE%` antes de o log sair
/// da máquina. O nome de usuário do Windows aparece em todo caminho absoluto e
/// não ajuda em nada a depurar — o que importa é a estrutura do caminho.
pub fn redigir(texto: &str) -> String {
    let Ok(perfil) = std::env::var("USERPROFILE") else {
        return texto.to_string();
    };
    if perfil.is_empty() {
        return texto.to_string();
    }
    // Os caminhos aparecem tanto com `\` quanto com `/` conforme quem escreveu.
    let barra_normal = perfil.replace('\\', "/");
    texto
        .replace(&perfil, "%USERPROFILE%")
        .replace(&barra_normal, "%USERPROFILE%")
}

/// Rotaciona quando passa de [`MAX_BYTES`]: o atual vira `loop.log.1` (o `.1`
/// anterior é descartado) e a gravação recomeça num arquivo limpo. Dois arquivos
/// bastam — queremos a sessão atual e a anterior, não um histórico.
fn rotacionar_se_grande(path: &Path) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if meta.len() < MAX_BYTES {
        return;
    }
    let anterior = path.with_extension("log.1");
    let _ = std::fs::remove_file(&anterior);
    let _ = std::fs::rename(path, &anterior);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sem `init()` (o caso dos testes e de uma pasta não gravável) nada pode
    /// explodir — as funções têm de virar no-op silencioso.
    #[test]
    fn sem_init_nao_quebra() {
        linha("teste", "não deve panicar");
        linha_unica("teste", "nem esta");
        assert_eq!(ler_final(1024), "");
    }

    /// O corte por bytes do `ler_final` descarta a primeira linha (que pode estar
    /// partida no meio de um acento) e devolve o resto íntegro.
    #[test]
    fn ler_final_corta_na_quebra_de_linha() {
        let conteudo = "linha antiga\nlinha nova\n";
        let inicio = conteudo.len().saturating_sub(14);
        let texto = &conteudo[inicio..];
        let cortado = texto.find('\n').map(|q| &texto[q + 1..]).unwrap_or(texto);
        assert_eq!(cortado, "linha nova\n");
    }
}
