//! Põe o iRacing em modo janela — o pré-requisito do overlay do Loop.
//!
//! O overlay é uma janela sempre-no-topo. Em tela cheia EXCLUSIVA o Windows não
//! deixa nada por cima do simulador — é a mesma razão de `SetForegroundWindow`
//! devolver `false` ali (ver [`super::imp`]) —, então o overlay simplesmente não
//! aparece, sem erro nenhum para o jogador entender. O interruptor vive na seção
//! `[Display]` dos `rendererDX11*.ini`:
//!
//! ```ini
//! [Display]
//! fullScreen=0          ; janela
//! border=0              ; sem borda
//! windowedMaximized=1   ; ocupando a tela
//! ```
//!
//! Os três JUNTOS dão o "borderless fullscreen": para o jogador continua parecendo
//! tela cheia, e para o Windows é uma janela. Só `fullScreen=0` devolveria uma
//! janelinha com barra de título no meio da tela, que ninguém pediu.
//!
//! **Qual arquivo.** Não é o `rendererDX11.ini` sozinho. O sim mantém um arquivo por
//! modo de renderização (`Monitor`, `OpenXR`, `OpenVR`, `Oculus`) e usa o do modo
//! ativo: numa máquina real o `rendererDX11.ini` estava parado havia oito meses
//! enquanto o `Monitor` era reescrito a cada sessão. Como a escolha do modo não está
//! em lugar nenhum que dê para ler (vem da UI web do iRacing), escrevemos nos DOIS do
//! caminho de monitor. Os arquivos de VR ficam de fora de propósito: lá o overlay é
//! camada de VR e não depende da janela do desktop — mexer neles seria mudar o espelho
//! da tela do jogador sem nenhum ganho.
//!
//! **A janela de escrita.** Como o `app.ini` (ver [`super::spotter_control`]), o sim
//! lê ao abrir e REESCREVE ao fechar. Aqui, diferente de lá, isso é motivo de
//! BLOQUEAR: escrever com o simulador aberto não é "vale na próxima abertura", é
//! perder a escrita no fechamento dele. Com só a UI/lobby aberta não há problema —
//! quem reescreve esses arquivos é o simulador, não o launcher. É por isso que o
//! pedido aparece na exportação da etapa: o jogador está no lobby, prestes a entrar.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::spotter_control::{escrever_chave, ler_chave};

/// Seção dos `rendererDX11*.ini`. Comparada sem diferenciar maiúsculas.
const SECAO: &str = "Display";
const CHAVE_TELA_CHEIA: &str = "fullScreen";
const CHAVE_BORDA: &str = "border";
const CHAVE_MAXIMIZADA: &str = "windowedMaximized";

/// Os valores que o overlay precisa.
const EM_JANELA: &str = "0";
const SEM_BORDA: &str = "0";
const MAXIMIZADA: &str = "1";

/// Arquivos do caminho de monitor, na ordem de preferência. O `Monitor` é o que o
/// sim usa hoje; o base fica junto porque versões antigas leem dele.
const ARQUIVOS: [&str; 2] = ["rendererDX11Monitor.ini", "rendererDX11.ini"];

/// Um `renderer*.ini` encontrado e o que ele diz agora.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ArquivoModoJanela {
    pub caminho: String,
    /// `true` quando os três valores já estão como o overlay precisa.
    pub em_janela: bool,
}

/// Estado exposto à UI.
#[derive(Serialize, Clone, Debug)]
pub struct ModoJanelaStatus {
    /// Achou pelo menos um `rendererDX11*.ini` do caminho de monitor.
    pub encontrado: bool,
    /// Todos os arquivos achados já estão em janela. Sem arquivo nenhum isto é
    /// `false` querendo dizer "não sei" — quem decide se pergunta é
    /// [`ModoJanelaStatus::deve_perguntar`], não este campo.
    pub em_janela: bool,
    /// O SIMULADOR está aberto agora. A UI/lobby NÃO conta: ela não reescreve
    /// esses arquivos, então dá para ajustar com o jogador parado no lobby.
    pub simulador_aberto: bool,
    /// Vale a pena pedir permissão ao jogador agora? Falso quando não há o que
    /// ajustar, quando já está ajustado, ou quando a escrita seria perdida.
    pub deve_perguntar: bool,
    pub arquivos: Vec<ArquivoModoJanela>,
}

// ---------------------------------------------------------------------------
// Leitura e escrita, sem tocar em disco (é o que os testes exercitam)
// ---------------------------------------------------------------------------

/// Os três valores já estão como o overlay precisa? `None` quando o arquivo não
/// tem sequer a chave `fullScreen` na seção `[Display]` — aí não é "está em tela
/// cheia", é "este arquivo não é o que eu penso que é".
fn conteudo_em_janela(conteudo: &str) -> Option<bool> {
    let tela_cheia = ler_chave(conteudo, SECAO, CHAVE_TELA_CHEIA)?;
    let borda = ler_chave(conteudo, SECAO, CHAVE_BORDA);
    let maximizada = ler_chave(conteudo, SECAO, CHAVE_MAXIMIZADA);
    Some(
        tela_cheia == EM_JANELA
            && borda.as_deref() != Some("1")
            && maximizada.as_deref() != Some("0"),
    )
}

/// Reescreve os três valores preservando o resto do arquivo.
///
/// `fullScreen` é obrigatória — sem ela não há ajuste nenhum a fazer e devolvemos
/// `None`. As outras duas são o melhor esforço: um `renderer.ini` de versão antiga
/// que não tenha `windowedMaximized` ainda fica utilizável em janela.
fn conteudo_ajustado(conteudo: &str) -> Option<String> {
    let mut saida = escrever_chave(conteudo, SECAO, CHAVE_TELA_CHEIA, EM_JANELA)?;
    for (chave, valor) in [(CHAVE_BORDA, SEM_BORDA), (CHAVE_MAXIMIZADA, MAXIMIZADA)] {
        if let Some(novo) = escrever_chave(&saida, SECAO, chave, valor) {
            saida = novo;
        }
    }
    Some(saida)
}

// ---------------------------------------------------------------------------
// Arquivos
// ---------------------------------------------------------------------------

/// Backup completo do arquivo, com o mesmo sufixo que o resto do módulo usa no
/// `app.ini`. É um retrato de antes de o Loop tocar em qualquer coisa, então só
/// é criado uma vez.
fn caminho_backup(ini: &Path) -> PathBuf {
    let nome = ini
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "rendererDX11.ini".to_string());
    ini.with_file_name(format!("{nome}.loop.bak"))
}

/// Os `renderer*.ini` do caminho de monitor que existem de verdade.
fn arquivos_existentes() -> Vec<PathBuf> {
    let dir = match super::paths::iracing_dir() {
        Some(d) => d,
        None => return Vec::new(),
    };
    ARQUIVOS
        .iter()
        .map(|nome| dir.join(nome))
        .filter(|p| p.is_file())
        .collect()
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

/// Estado atual, sem tocar em nada.
pub fn status() -> ModoJanelaStatus {
    let simulador_aberto = super::imp::simulador_aberto();
    let arquivos: Vec<ArquivoModoJanela> = arquivos_existentes()
        .into_iter()
        .filter_map(|p| {
            let conteudo = fs::read_to_string(&p).ok()?;
            Some(ArquivoModoJanela {
                caminho: p.display().to_string(),
                em_janela: conteudo_em_janela(&conteudo)?,
            })
        })
        .collect();

    let encontrado = !arquivos.is_empty();
    let em_janela = encontrado && arquivos.iter().all(|a| a.em_janela);
    ModoJanelaStatus {
        encontrado,
        em_janela,
        simulador_aberto,
        deve_perguntar: encontrado && !em_janela && !simulador_aberto,
        arquivos,
    }
}

/// Põe os arquivos em modo janela. Idempotente.
///
/// Erra em vez de fingir sucesso quando o simulador está aberto: ali a escrita
/// seria desfeita no fechamento dele, e um "pronto!" que não sobrevive ao próximo
/// clique é pior do que a tela cheia que o jogador já tinha.
pub fn aplicar() -> Result<ModoJanelaStatus, String> {
    if super::imp::simulador_aberto() {
        return Err(
            "O simulador do iRacing está aberto. Feche-o antes — com ele aberto, o ajuste \
             seria desfeito quando você sair."
                .to_string(),
        );
    }

    let alvos = arquivos_existentes();
    if alvos.is_empty() {
        return Err(
            "Nenhum rendererDX11.ini encontrado em Documentos/iRacing. Abra o iRacing uma vez \
             para ele criar a configuração gráfica."
                .to_string(),
        );
    }

    let mut ajustados = 0usize;
    for ini in &alvos {
        let conteudo = match fs::read_to_string(ini) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let novo = match conteudo_ajustado(&conteudo) {
            Some(n) => n,
            // Sem `fullScreen` na seção: não é arquivo para nós. Segue para o próximo.
            None => continue,
        };
        if novo == conteudo {
            ajustados += 1; // já estava certo
            continue;
        }
        let bak = caminho_backup(ini);
        if !bak.exists() {
            fs::write(&bak, &conteudo).map_err(|e| format!("Falha ao criar o backup: {e}"))?;
        }
        fs::write(ini, novo)
            .map_err(|e| format!("Falha ao escrever {}: {e}", ini.display()))?;
        ajustados += 1;
    }

    if ajustados == 0 {
        return Err(
            "A seção [Display] não tem a chave 'fullScreen'. Abra as opções gráficas do iRacing \
             uma vez e tente de novo."
                .to_string(),
        );
    }
    Ok(status())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recorte fiel da seção `[Display]` de um `rendererDX11.ini` real, com o
    /// espaçamento, as tabulações e os comentários alinhados como o sim escreve.
    /// Inclui as vizinhas `fullScreenWidth/Height/Depth` de propósito: são a
    /// armadilha real de casamento por prefixo.
    const DISPLAY_REAL: &str = "[Display]\n\
windowedYPos=0                                  \t; Window top left corner in windowed mode\n\
windowedWidth=1920                              \t; windowed mode width\n\
windowedMaximized=0                             \t; Window is maximized in windowed mode\n\
windowedHeight=1080                             \t; windowed mode height\n\
pixelRatioWindowed=1.000                        \t; Windowed mode\n\
fullScreenWidth=1920                            \t; full screen Window's width\n\
fullScreenHeight=1080                           \t; full screen Window's height\n\
fullScreenDepth=32                              \t; Color depth\n\
fullScreen=1                                    \t; fullscreen?\n\
deviceIdx=0                                     \t; which adapter\n\
border=1                                        \t; window border?\n\
\n\
[MonitorSetup]\n\
fullScreen=1                                    \t; nao e a nossa secao\n";

    #[test]
    fn le_tela_cheia_como_fora_de_janela() {
        assert_eq!(conteudo_em_janela(DISPLAY_REAL), Some(false));
    }

    #[test]
    fn ajuste_poe_os_tres_valores() {
        let novo = conteudo_ajustado(DISPLAY_REAL).expect("tem fullScreen");
        assert_eq!(ler_chave(&novo, SECAO, CHAVE_TELA_CHEIA).as_deref(), Some("0"));
        assert_eq!(ler_chave(&novo, SECAO, CHAVE_BORDA).as_deref(), Some("0"));
        assert_eq!(ler_chave(&novo, SECAO, CHAVE_MAXIMIZADA).as_deref(), Some("1"));
        assert_eq!(conteudo_em_janela(&novo), Some(true));
    }

    /// A armadilha: `fullScreen` não pode casar com `fullScreenWidth`, que vem
    /// ANTES no arquivo. Errar aqui zeraria a resolução do jogador.
    #[test]
    fn nao_encosta_nas_chaves_vizinhas() {
        let novo = conteudo_ajustado(DISPLAY_REAL).expect("tem fullScreen");
        assert!(novo.contains("fullScreenWidth=1920"));
        assert!(novo.contains("fullScreenHeight=1080"));
        assert!(novo.contains("fullScreenDepth=32"));
        assert!(novo.contains("pixelRatioWindowed=1.000"));
    }

    /// Comentário alinhado e tabulação voltam intactos — o sim reescreve o arquivo
    /// inteiro depois, mas um diff sujo esconde regressão de verdade.
    #[test]
    fn preserva_o_alinhamento_do_comentario() {
        let novo = conteudo_ajustado(DISPLAY_REAL).expect("tem fullScreen");
        assert!(novo.contains("fullScreen=0                                    \t; fullscreen?"));
        assert!(novo.contains("border=0                                        \t; window border?"));
    }

    /// Só a seção `[Display]` é tocada. O `[MonitorSetup]` tem uma chave de mesmo
    /// nome logo abaixo.
    #[test]
    fn nao_vaza_para_outra_secao() {
        let novo = conteudo_ajustado(DISPLAY_REAL).expect("tem fullScreen");
        let depois = novo.split("[MonitorSetup]").nth(1).expect("tem a seção");
        assert!(depois.contains("fullScreen=1"));
    }

    #[test]
    fn idempotente() {
        let uma = conteudo_ajustado(DISPLAY_REAL).expect("tem fullScreen");
        let duas = conteudo_ajustado(&uma).expect("tem fullScreen");
        assert_eq!(uma, duas);
    }

    #[test]
    fn sem_a_chave_nao_ha_ajuste() {
        let sem = "[Display]\nwindowedWidth=1920\n";
        assert_eq!(conteudo_em_janela(sem), None);
        assert_eq!(conteudo_ajustado(sem), None);
    }

    /// Arquivo antigo sem `windowedMaximized`: o ajuste ainda vale pelo que existe.
    #[test]
    fn melhor_esforco_nas_chaves_opcionais() {
        let antigo = "[Display]\nfullScreen=1\nborder=1\n";
        let novo = conteudo_ajustado(antigo).expect("tem fullScreen");
        assert_eq!(ler_chave(&novo, SECAO, CHAVE_TELA_CHEIA).as_deref(), Some("0"));
        assert_eq!(ler_chave(&novo, SECAO, CHAVE_BORDA).as_deref(), Some("0"));
        assert_eq!(conteudo_em_janela(&novo), Some(true));
    }

    /// Prova contra os arquivos DE VERDADE da máquina, quando existem: as chaves
    /// que a gente procura estão mesmo lá, com esse nome e nessa seção.
    #[test]
    fn arquivos_reais_tem_as_chaves() {
        for ini in arquivos_existentes() {
            let conteudo = fs::read_to_string(&ini).expect("arquivo legível");
            assert!(
                conteudo_em_janela(&conteudo).is_some(),
                "{} não tem [Display] fullScreen",
                ini.display()
            );
        }
    }
}
