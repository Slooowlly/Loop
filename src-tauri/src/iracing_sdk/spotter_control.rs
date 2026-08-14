//! Silencia o spotter nativo do iRacing enquanto o Loop está aberto, e o devolve
//! ao fechar.
//!
//! O Loop assume o papel de spotter/engenheiro com voz própria. Duas vozes falando
//! por cima uma da outra é pior que uma, então o nativo precisa calar. O interruptor
//! fica na seção `[SPCC]` do `app.ini`:
//!
//! ```ini
//! [SPCC]
//! voice=1   ; Does the spotter talk to you?
//! text=1    ; Does the spotter display text messages?
//! ```
//!
//! **Por que reverter ao fechar, diferente da macro de bandeira.** A macro de
//! amarela (ver [`super::race_control`]) é irrelevante fora do Loop, então lá a
//! reversão é manual e assumida. Aqui não: quem sai do Loop e vai correr online
//! PRECISA do spotter de volta, e fazer isso à mão toda vez é um imposto. Então a
//! reversão é automática — e, como fechamento limpo não é garantido, ela também
//! acontece no boot seguinte (ver [`sincronizar`]).
//!
//! **A janela de escrita.** O iRacing lê o `app.ini` ao abrir e o REESCREVE ao
//! fechar, com o que tinha em memória. Escrever com o sim aberto é jogar fora. Não
//! bloqueamos por isso — a escrita continua valendo para a próxima abertura do sim
//! —, mas é a razão de o estado ser reconciliado a cada boot em vez de confiado.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Seção do spotter no `app.ini`. Comparada sem diferenciar maiúsculas.
const SECAO: &str = "SPCC";
/// A voz do spotter. É a chave que importa: o Loop toma o áudio.
const CHAVE_VOZ: &str = "voice";
/// As mensagens de texto do spotter na tela.
const CHAVE_TEXTO: &str = "text";
/// Valor que o Loop escreve para calar.
const CALADO: &str = "0";

const ARQUIVO_ESTADO: &str = "loop_spotter.json";
/// Backup completo do `app.ini`. O MESMO nome que [`super::race_control`] usa —
/// de propósito: é um retrato de "antes de o Loop tocar em qualquer coisa", e dois
/// backups concorrentes do mesmo arquivo só criariam a dúvida de qual é o bom.
const ARQUIVO_BACKUP: &str = "app.ini.loop.bak";

/// Valores ORIGINAIS do jogador, guardados para a devolução.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OriginaisSpotter {
    pub voice: String,
    pub text: String,
}

/// Estado exposto à UI.
#[derive(Serialize, Clone, Debug)]
pub struct SpotterStatus {
    pub app_ini_found: bool,
    pub app_ini_path: Option<String>,
    /// O spotter nativo está calado pelo Loop agora?
    pub silenciado: bool,
    pub voice_atual: Option<String>,
    pub text_atual: Option<String>,
    /// O que será devolvido ao fechar. `None` = nada guardado.
    pub original: Option<OriginaisSpotter>,
}

// ---------------------------------------------------------------------------
// Leitura e escrita de chave dentro de seção, preservando o resto do arquivo
// ---------------------------------------------------------------------------

/// Nome da seção se a linha for um cabeçalho `[Nome]`.
fn cabecalho_de_secao(linha: &str) -> Option<&str> {
    let t = linha.trim();
    t.strip_prefix('[')?.strip_suffix(']')
}

/// A linha declara `chave` (tolera recuo e espaço antes do `=`)?
///
/// O casamento é por prefixo MAIS o `=` logo depois, e essa segunda metade não é
/// zelo: sem ela, procurar `voice` casaria com `voicePack` — que existe na mesma
/// seção, três linhas abaixo, e apagaria o pacote de voz do jogador.
fn eh_linha_da_chave(linha: &str, chave: &str) -> bool {
    linha
        .trim_start()
        .strip_prefix(chave)
        .map(|resto| resto.trim_start().starts_with('='))
        .unwrap_or(false)
}

/// Separa o que vem depois do `=` em (valor, resto). O resto guarda o espaçamento
/// e o comentário alinhado, que precisam voltar intactos.
fn partir_valor(depois_do_igual: &str) -> (&str, &str) {
    let corte = depois_do_igual.find(';').unwrap_or(depois_do_igual.len());
    let bruto = depois_do_igual[..corte].trim_end_matches(['\r', '\n']);
    let fim = bruto.trim_end().len();
    (&depois_do_igual[..fim], &depois_do_igual[fim..])
}

/// Valor de `chave` dentro de `secao`.
pub fn ler_chave(conteudo: &str, secao: &str, chave: &str) -> Option<String> {
    let mut dentro = false;
    for linha in conteudo.lines() {
        if let Some(nome) = cabecalho_de_secao(linha) {
            dentro = nome.eq_ignore_ascii_case(secao);
            continue;
        }
        if dentro && eh_linha_da_chave(linha, chave) {
            let (_, depois) = linha.split_once('=')?;
            return Some(partir_valor(depois).0.trim().to_string());
        }
    }
    None
}

/// Reescreve `chave` dentro de `secao` preservando recuo, espaçamento, comentário
/// alinhado e a quebra de linha (CRLF).
///
/// Devolve `None` se a chave não existir na seção. Não INSERIMOS a chave nesse caso:
/// o `app.ini` é do iRacing, e adicionar linha num arquivo que o sim reescreve
/// sozinho é assumir um contrato que não temos.
pub fn escrever_chave(conteudo: &str, secao: &str, chave: &str, novo: &str) -> Option<String> {
    let mut saida = String::with_capacity(conteudo.len() + 8);
    let mut dentro = false;
    let mut trocou = false;

    for seg in conteudo.split_inclusive('\n') {
        if let Some(nome) = cabecalho_de_secao(seg) {
            dentro = nome.eq_ignore_ascii_case(secao);
            saida.push_str(seg);
            continue;
        }
        if dentro && !trocou && eh_linha_da_chave(seg, chave) {
            let (antes, depois) = match seg.split_once('=') {
                Some(par) => par,
                None => {
                    saida.push_str(seg);
                    continue;
                }
            };
            let (_, resto) = partir_valor(depois);
            saida.push_str(antes);
            saida.push('=');
            saida.push_str(novo);
            saida.push_str(resto);
            trocou = true;
            continue;
        }
        saida.push_str(seg);
    }
    trocou.then_some(saida)
}

// ---------------------------------------------------------------------------
// Arquivos ao lado do app.ini
// ---------------------------------------------------------------------------

fn caminho_estado(app_ini: &Path) -> PathBuf {
    app_ini.with_file_name(ARQUIVO_ESTADO)
}

fn caminho_backup(app_ini: &Path) -> PathBuf {
    app_ini.with_file_name(ARQUIVO_BACKUP)
}

fn ler_estado(app_ini: &Path) -> Option<OriginaisSpotter> {
    let bruto = fs::read_to_string(caminho_estado(app_ini)).ok()?;
    serde_json::from_str(&bruto).ok()
}

fn gravar_estado(app_ini: &Path, orig: &OriginaisSpotter) -> Result<(), String> {
    fs::write(
        caminho_estado(app_ini),
        serde_json::to_string_pretty(orig).unwrap_or_default(),
    )
    .map_err(|e| format!("Falha ao salvar o estado do spotter: {e}"))
}

/// Decide o que guardar como ORIGINAL, que é a única parte com armadilha.
///
/// Quatro situações, e cada uma tem um jeito certo de errar:
///
/// - **Sem estado, valores normais** — primeira aplicação. Guarda o que está lá.
/// - **Sem estado, já calado** — uma execução anterior aplicou e morreu sem
///   devolver, levando o arquivo de estado junto. Guardar `0/0` como "original"
///   condenaria o jogador ao silêncio permanente, então caímos no BACKUP, que é o
///   retrato de antes de o Loop existir naquele arquivo.
/// - **Com estado e ainda calado** — fechamento sujo (travamento, kill). O que está
///   guardado continua sendo a verdade; não sobrescrever.
/// - **Com estado e valores diferentes** — o jogador mexeu à mão no iRacing depois
///   da última aplicação. A escolha dele vence: vira o novo original.
fn decidir_originais(
    estado: Option<OriginaisSpotter>,
    atual: &OriginaisSpotter,
    backup: Option<OriginaisSpotter>,
) -> OriginaisSpotter {
    let calado = atual.voice == CALADO && atual.text == CALADO;
    match estado {
        Some(guardado) if calado => guardado,
        Some(_) => atual.clone(),
        None if calado => backup.unwrap_or_else(|| atual.clone()),
        None => atual.clone(),
    }
}

fn originais_do_arquivo(conteudo: &str) -> Option<OriginaisSpotter> {
    Some(OriginaisSpotter {
        voice: ler_chave(conteudo, SECAO, CHAVE_VOZ)?,
        text: ler_chave(conteudo, SECAO, CHAVE_TEXTO)?,
    })
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

/// Estado atual, sem tocar em nada.
pub fn status() -> SpotterStatus {
    let app_ini = super::paths::app_ini_path();
    let mut st = SpotterStatus {
        app_ini_found: app_ini.is_some(),
        app_ini_path: app_ini.as_ref().map(|p| p.display().to_string()),
        silenciado: false,
        voice_atual: None,
        text_atual: None,
        original: None,
    };
    if let Some(path) = &app_ini {
        st.original = ler_estado(path);
        if let Ok(conteudo) = fs::read_to_string(path) {
            let voz = ler_chave(&conteudo, SECAO, CHAVE_VOZ);
            let texto = ler_chave(&conteudo, SECAO, CHAVE_TEXTO);
            st.silenciado = st.original.is_some()
                && voz.as_deref() == Some(CALADO)
                && texto.as_deref() == Some(CALADO);
            st.voice_atual = voz;
            st.text_atual = texto;
        }
    }
    st
}

/// Cala o spotter nativo, guardando o original. Idempotente.
pub fn silenciar() -> Result<SpotterStatus, String> {
    let app_ini = super::paths::app_ini_path()
        .ok_or("app.ini do iRacing não encontrado (Documentos/iRacing/app.ini).")?;
    let conteudo =
        fs::read_to_string(&app_ini).map_err(|e| format!("Falha ao ler o app.ini: {e}"))?;

    let atual = originais_do_arquivo(&conteudo).ok_or(
        "Seção [SPCC] sem as chaves 'voice'/'text'. Abra as opções de som do iRacing uma vez.",
    )?;

    // Backup completo, uma vez. É também a rede da recuperação em `decidir_originais`.
    let bak = caminho_backup(&app_ini);
    if !bak.exists() {
        fs::write(&bak, &conteudo).map_err(|e| format!("Falha ao criar o backup: {e}"))?;
    }
    let do_backup = fs::read_to_string(&bak)
        .ok()
        .and_then(|c| originais_do_arquivo(&c));

    let originais = decidir_originais(ler_estado(&app_ini), &atual, do_backup);
    gravar_estado(&app_ini, &originais)?;

    let novo = escrever_chave(&conteudo, SECAO, CHAVE_VOZ, CALADO)
        .and_then(|c| escrever_chave(&c, SECAO, CHAVE_TEXTO, CALADO))
        .ok_or("Não consegui reescrever [SPCC] no app.ini.")?;
    fs::write(&app_ini, novo).map_err(|e| format!("Falha ao escrever o app.ini: {e}"))?;

    Ok(status())
}

/// Devolve o spotter nativo ao que era. Idempotente: sem estado guardado, no-op.
pub fn devolver() -> Result<SpotterStatus, String> {
    let app_ini = match super::paths::app_ini_path() {
        Some(p) => p,
        None => return Ok(status()),
    };
    let originais = match ler_estado(&app_ini) {
        Some(o) => o,
        None => return Ok(status()),
    };
    let conteudo =
        fs::read_to_string(&app_ini).map_err(|e| format!("Falha ao ler o app.ini: {e}"))?;

    let novo = escrever_chave(&conteudo, SECAO, CHAVE_VOZ, &originais.voice)
        .and_then(|c| escrever_chave(&c, SECAO, CHAVE_TEXTO, &originais.text))
        .ok_or("Não consegui reescrever [SPCC] no app.ini.")?;
    fs::write(&app_ini, novo).map_err(|e| format!("Falha ao escrever o app.ini: {e}"))?;

    // O estado só some DEPOIS da escrita dar certo. Removê-lo antes trocaria uma
    // falha de escrita pela perda do original.
    let _ = fs::remove_file(caminho_estado(&app_ini));
    Ok(status())
}

/// Põe o `app.ini` de acordo com a preferência do jogador. É o único ponto que o
/// boot precisa chamar: cobre a primeira aplicação, a reaplicação e — quando o
/// jogador desliga a opção — a devolução do que ficou pendurado por um fechamento
/// sujo da execução anterior.
pub fn sincronizar(assumir: bool) -> Result<SpotterStatus, String> {
    if assumir {
        silenciar()
    } else {
        devolver()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recorte fiel do `app.ini` real, com o espaçamento e os comentários alinhados
    /// por TAB — que é justamente o que a escrita não pode destruir.
    const AMOSTRA: &str = concat!(
        "[Audio]\r\n",
        "loudnessSPCC=-15.100000                 \t; Volume adjustment for spotter noise in dB\r\n",
        "\r\n",
        "[SPCC]\r\n",
        "enabled=1                               \t; Is the spotter enabled at all?\r\n",
        "text=1                                  \t; Does the spotter display text messages?\r\n",
        "verbosity=2                             \t; How chatty is the spotter?\r\n",
        "voice=1                                 \t; Does the spotter talk to you?\r\n",
        "voicePack=Portuguese-Guto_Colvara       \t; Voice pack for spotter\r\n",
        "\r\n",
        "[spectator]\r\n",
        "text=9                                  \t; nao e a nossa secao\r\n",
    );

    #[test]
    fn le_chave_apenas_da_secao_certa() {
        assert_eq!(ler_chave(AMOSTRA, "SPCC", "voice").as_deref(), Some("1"));
        assert_eq!(ler_chave(AMOSTRA, "SPCC", "text").as_deref(), Some("1"));
        // `text` também existe em [spectator]; a busca não pode vazar para lá.
        assert_eq!(
            ler_chave(AMOSTRA, "spectator", "text").as_deref(),
            Some("9")
        );
        assert_eq!(ler_chave(AMOSTRA, "SPCC", "inexistente"), None);
    }

    #[test]
    fn secao_casa_sem_diferenciar_caixa() {
        assert_eq!(ler_chave(AMOSTRA, "spcc", "voice").as_deref(), Some("1"));
    }

    #[test]
    fn voice_nao_casa_com_voice_pack() {
        // A regressão que este teste existe para pegar: apagar o pacote de voz do
        // jogador ao procurar a chave `voice`.
        let novo = escrever_chave(AMOSTRA, "SPCC", "voice", "0").unwrap();
        assert!(novo.contains("voicePack=Portuguese-Guto_Colvara"));
        assert_eq!(ler_chave(&novo, "SPCC", "voice").as_deref(), Some("0"));
    }

    #[test]
    fn escrita_preserva_comentario_alinhado_e_crlf() {
        let novo = escrever_chave(AMOSTRA, "SPCC", "voice", "0").unwrap();
        assert!(novo.contains(
            "voice=0                                 \t; Does the spotter talk to you?\r\n"
        ));
        // Nenhuma outra linha muda.
        assert_eq!(novo.lines().count(), AMOSTRA.lines().count());
        assert!(
            novo.contains("verbosity=2                             \t; How chatty is the spotter?")
        );
    }

    #[test]
    fn escrita_nao_vaza_para_outra_secao() {
        let novo = escrever_chave(AMOSTRA, "SPCC", "text", "0").unwrap();
        assert_eq!(ler_chave(&novo, "SPCC", "text").as_deref(), Some("0"));
        assert_eq!(ler_chave(&novo, "spectator", "text").as_deref(), Some("9"));
    }

    #[test]
    fn escrita_de_chave_ausente_devolve_none() {
        assert!(escrever_chave(AMOSTRA, "SPCC", "naoexiste", "0").is_none());
        assert!(escrever_chave(AMOSTRA, "SecaoQueNaoExiste", "voice", "0").is_none());
    }

    #[test]
    fn ida_e_volta_devolve_o_arquivo_identico() {
        let calado = escrever_chave(AMOSTRA, "SPCC", "voice", "0")
            .and_then(|c| escrever_chave(&c, "SPCC", "text", "0"))
            .unwrap();
        let voltou = escrever_chave(&calado, "SPCC", "voice", "1")
            .and_then(|c| escrever_chave(&c, "SPCC", "text", "1"))
            .unwrap();
        assert_eq!(voltou, AMOSTRA);
    }

    fn orig(voice: &str, text: &str) -> OriginaisSpotter {
        OriginaisSpotter {
            voice: voice.to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn primeira_aplicacao_guarda_o_que_esta_la() {
        assert_eq!(
            decidir_originais(None, &orig("1", "1"), None),
            orig("1", "1")
        );
    }

    #[test]
    fn fechamento_sujo_mantem_o_original_guardado() {
        // Estado existe e o arquivo continua calado: a execução anterior morreu
        // sem devolver. Sobrescrever aqui perderia o original para sempre.
        assert_eq!(
            decidir_originais(Some(orig("1", "1")), &orig("0", "0"), None),
            orig("1", "1")
        );
    }

    #[test]
    fn mudanca_manual_do_jogador_vence() {
        // O jogador religou o spotter dentro do iRacing. A escolha dele passa a ser
        // o original, senão a devolução o calaria de novo no fim da sessão.
        assert_eq!(
            decidir_originais(Some(orig("1", "1")), &orig("1", "0"), None),
            orig("1", "0")
        );
    }

    #[test]
    fn estado_perdido_com_arquivo_calado_cai_no_backup() {
        // Sem o backup, guardaria "0/0" como original e condenaria o jogador ao
        // silêncio permanente.
        assert_eq!(
            decidir_originais(None, &orig("0", "0"), Some(orig("1", "1"))),
            orig("1", "1")
        );
    }

    /// Prova contra o `app.ini` DE VERDADE da máquina, quando existe.
    ///
    /// A amostra acima é fiel, mas foi escrita por quem escreveu o parser — o que é
    /// exatamente o viés que um teste de parser não pega. Este roda sobre o arquivo
    /// real (~500 linhas, dezenas de seções, valores com espaço e parênteses) e exige
    /// que a ida e volta devolva o arquivo BYTE A BYTE, o que só acontece se nenhuma
    /// outra linha tiver sido tocada.
    ///
    /// Não escreve nada em disco. Pula em silêncio quando o iRacing não está
    /// instalado — é o caso do CI.
    #[test]
    fn ida_e_volta_no_app_ini_real_nao_toca_em_mais_nada() {
        let Some(caminho) = super::super::paths::app_ini_path() else {
            return;
        };
        let Ok(real) = fs::read_to_string(&caminho) else {
            return;
        };
        let Some(antes) = originais_do_arquivo(&real) else {
            return;
        };

        let calado = escrever_chave(&real, SECAO, CHAVE_VOZ, CALADO)
            .and_then(|c| escrever_chave(&c, SECAO, CHAVE_TEXTO, CALADO))
            .expect("[SPCC] com voice/text deveria ser reescrevível");
        assert_eq!(
            ler_chave(&calado, SECAO, CHAVE_VOZ).as_deref(),
            Some(CALADO)
        );
        assert_eq!(
            ler_chave(&calado, SECAO, CHAVE_TEXTO).as_deref(),
            Some(CALADO)
        );

        let voltou = escrever_chave(&calado, SECAO, CHAVE_VOZ, &antes.voice)
            .and_then(|c| escrever_chave(&c, SECAO, CHAVE_TEXTO, &antes.text))
            .expect("a devolução deveria achar as mesmas chaves");
        assert_eq!(
            voltou,
            real,
            "a ida e volta mexeu em algo além de [SPCC] voice/text em {}",
            caminho.display()
        );
    }

    #[test]
    fn estado_e_backup_perdidos_nao_inventam_valor() {
        // Sem nenhuma fonte confiável, o que está no arquivo é tudo que se tem.
        assert_eq!(
            decidir_originais(None, &orig("0", "0"), None),
            orig("0", "0")
        );
    }
}
