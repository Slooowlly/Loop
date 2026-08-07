//! **A voz própria**: o nome do jogador, sintetizado uma vez e guardado na máquina dele.
//!
//! O acervo do engenheiro tem 355 sobrenomes gravados, e todos vêm dos pools de IA — nenhum
//! primeiro nome, e nenhuma chance de o do jogador estar lá. Este módulo fecha esse buraco com
//! a única coisa que funciona para todo mundo: uma síntese por carreira, guardada em disco.
//!
//! ## Uma ida, e nunca mais
//!
//! O arquivo mora em `saves/<carreira>/voz/<chave>.mp3` e é a **fonte da verdade**: existindo
//! ele, nada mais toca a rede. A carreira inteira paga uma síntese, e as vinte, cinquenta ou
//! duzentas corridas seguintes tocam do disco como qualquer peça do instalador.
//!
//! Fica **no save**, e não numa pasta global de cache, porque o nome é da carreira: dois saves
//! do mesmo jogador podem ter pilotos com nomes diferentes, e apagar uma carreira tem de levar
//! junto o que era só dela.
//!
//! ## Por que o áudio volta em base64 e não um caminho
//!
//! Quem toca é o `engenheiroVoz.js`, e ele já sabe transformar áudio do servidor em peça —
//! decodifica, roda a cadeia de rádio no `OfflineAudioContext` e nivela ao mesmo alvo do
//! acervo (`processarComoPeca`). É exatamente o caminho da resposta do modelo, e reusá-lo é o
//! que garante que a peça própria saia no MESMO volume e com a MESMA cor de som das outras.
//!
//! Devolver um caminho de arquivo obrigaria a expor a pasta do save ao protocolo de assets do
//! webview e a escrever um segundo caminho de decodificação — dois riscos por nenhuma
//! vantagem, já que o áudio tem alguns KB.
//!
//! ## Nunca derruba nada
//!
//! Tudo devolve `None`: sem carreira, sem save, sem nome, sem rede, sem permissão de escrita.
//! Um `None` custa o vocativo — o engenheiro fala a mesma frase sem o nome na frente — e é
//! esse o preço certo. A alternativa seria um erro na cara do jogador por causa de uma palavra.

use crate::engenheiro::tratamento;

/// O que o app recebe para tocar. `chave` é o nome da peça (`voc_magno`), e é por ela
/// que o renderizador vai pedi-la depois.
#[derive(serde::Serialize)]
pub struct VozPropria {
    pub chave: String,
    pub audio_b64: String,
    pub mime: String,
    /// `true` quando esta chamada foi ao servidor. Só para o rastro do painel — o app trata
    /// os dois casos igual.
    pub gerada_agora: bool,
}

/// A pasta das peças próprias de uma carreira.
pub fn pasta(base_dir: &std::path::Path, career_id: &str) -> std::path::PathBuf {
    crate::config::app_config::AppConfig::load_or_default(base_dir)
        .saves_dir()
        .join(career_id)
        .join("voz")
}

/// O arquivo de uma peça própria. `.mp3` porque é o que a Cloud TTS devolve — e um arquivo
/// com a extensão certa é um arquivo que o jogador pode abrir e ouvir.
pub fn arquivo(base_dir: &std::path::Path, career_id: &str, chave: &str) -> std::path::PathBuf {
    pasta(base_dir, career_id).join(format!("{chave}.mp3"))
}

/// **A peça própria desta carreira**, do disco ou do servidor.
///
/// `None` em toda situação em que ela não existe nem pode existir agora — ver o cabeçalho.
/// Não é erro: é o engenheiro falando sem o vocativo até a próxima vez.
#[tauri::command]
pub async fn engenheiro_voz_propria(
    app: tauri::AppHandle,
    career_id: String,
) -> Option<VozPropria> {
    tauri::async_runtime::spawn_blocking(move || resolver(&app, &career_id))
        .await
        .ok()
        .flatten()
}

/// O corpo bloqueante: lê o save, procura o arquivo e, se precisar, sintetiza.
fn resolver(app: &tauri::AppHandle, career_id: &str) -> Option<VozPropria> {
    let (base_dir, db) = super::abrir_save(app, career_id)?;
    let piloto = crate::db::queries::drivers::get_player_driver(&db.conn).ok()?;
    let chave = tratamento::chave_do_nome(&piloto.nome)?;
    let caminho = arquivo(&base_dir, career_id, &chave);

    // O DISCO primeiro, e sem olhar a contagem de corridas. O gatilho das três corridas
    // decide quando GRAVAR; uma peça que já existe é para tocar sempre, e reconferir o
    // limiar aqui faria um save restaurado perder o vocativo que já tinha.
    if let Ok(bytes) = std::fs::read(&caminho) {
        if !bytes.is_empty() {
            return Some(VozPropria {
                chave,
                audio_b64: base64(&bytes),
                mime: "audio/mpeg".into(),
                gerada_agora: false,
            });
        }
    }

    if !tratamento::pode_gravar(
        piloto.stats_carreira.corridas,
        piloto.stats_carreira.vitorias,
    ) {
        return None;
    }

    let texto = tratamento::texto_da_peca(&piloto.nome)?;
    let (audio_b64, mime) = super::super::ptt_voz::sintetizar_peca(app, &texto).ok()?;

    // A escrita é o que pode falhar sem custar a fala: se o disco recusar, esta sessão toca
    // a peça mesmo assim e a próxima abertura do app tenta gravar de novo. O erro que
    // importaria seria o oposto — devolver `None` porque não deu para salvar.
    let _ = std::fs::create_dir_all(pasta(&base_dir, career_id));
    if let Ok(bytes) = decodificar_base64(&audio_b64) {
        let _ = std::fs::write(&caminho, bytes);
    }

    Some(VozPropria {
        chave,
        audio_b64,
        mime,
        gerada_agora: true,
    })
}

const ALFABETO: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 padrão, com o preenchimento. Escrito à mão para não trazer uma dependência por
/// causa de um arquivo de alguns KB que atravessa a ponte uma vez por carreira.
fn base64(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for pedaco in bytes.chunks(3) {
        let b = [
            pedaco[0],
            *pedaco.get(1).unwrap_or(&0),
            *pedaco.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= pedaco.len() {
                s.push(ALFABETO[((n >> (18 - 6 * i)) & 0x3F) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// O caminho de volta, para gravar em disco o que o servidor mandou. Ignora tudo o que não
/// pertence ao alfabeto — inclusive o preenchimento e quebras de linha.
fn decodificar_base64(texto: &str) -> Result<Vec<u8>, ()> {
    let mut bytes = Vec::with_capacity(texto.len() / 4 * 3);
    let mut acumulado: u32 = 0;
    let mut bits = 0u32;
    for c in texto.bytes() {
        let Some(v) = ALFABETO.iter().position(|a| *a == c) else {
            continue;
        };
        acumulado = (acumulado << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push(((acumulado >> bits) & 0xFF) as u8);
        }
    }
    if bytes.is_empty() {
        return Err(());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_ida_e_volta_em_todo_resto_de_tres() {
        // Os três restos, porque o preenchimento é onde um base64 escrito à mão erra. Um erro
        // aqui grava um MP3 corrompido no save do jogador — e ele fica lá, porque o disco é a
        // fonte da verdade e ninguém tenta de novo.
        for n in 1..=32usize {
            let dados: Vec<u8> = (0..n).map(|i| (i * 37 % 256) as u8).collect();
            let volta = decodificar_base64(&base64(&dados)).expect("decodifica");
            assert_eq!(volta, dados, "n = {n}");
        }
    }

    #[test]
    fn o_base64_bate_com_o_de_referencia() {
        // Contra um valor conhecido, e não só contra si mesmo: uma ida e volta consistente
        // com um alfabeto trocado passaria no teste acima e produziria lixo para o navegador.
        assert_eq!(base64(b"Loop"), "TG9vcA==");
        assert_eq!(base64(b"iRacing engenheiro"), "aVJhY2luZyBlbmdlbmhlaXJv");
    }

    #[test]
    fn base64_vazio_nao_vira_arquivo() {
        assert!(decodificar_base64("").is_err());
        assert!(decodificar_base64("!!!!").is_err());
    }
}
