//! Onde o módulo do iRacing guarda o punhado de coisas que precisam sobreviver ao
//! fechamento do app: a preferência de bandeira automática e o `custid` do jogador.
//!
//! Antes as duas moravam em `std::env::temp_dir()`. A Limpeza de Disco do Windows apaga
//! `%TEMP%` sem avisar, e o `custid` é a IDENTIDADE que casa o jogador no pós-corrida —
//! perdê-lo não dá erro nenhum, só faz a dificuldade adaptativa parar de reconhecer quem
//! está correndo, semanas depois, longe da causa. Agora as duas vivem no diretório de
//! dados do app, ao lado do `config.json`, dos saves e do `logs/loop.log`.
//!
//! O módulo roda em thread sem `AppHandle` (o amostrador), então a pasta é fixada uma vez
//! no boot num estático — mesmo padrão de [`crate::diagnostico`] e
//! [`crate::iracing_sdk::race_capture`].

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Subpasta dentro do diretório de dados do app. Junta o que é do iRacing num lugar só,
/// em vez de espalhar arquivos soltos na raiz do perfil.
const SUBPASTA: &str = "iracing";

static PASTA: OnceLock<PathBuf> = OnceLock::new();

/// Fixa a pasta de preferências. Chamado UMA vez no `setup()` do boot, antes do sampler.
pub fn init(base_dir: &Path) {
    let dir = base_dir.join(SUBPASTA);
    // A criação pode falhar (disco cheio, permissão); nesse caso a escrita falha depois e
    // o chamador registra. Fixar o caminho mesmo assim é o certo: sem isto cairíamos em
    // `%TEMP%`, que é justamente o problema que este módulo existe para resolver.
    let _ = std::fs::create_dir_all(&dir);
    let _ = PASTA.set(dir);
}

/// Caminho de um arquivo de preferência. Antes do [`init`] (testes, build sem Tauri) cai
/// no diretório temporário, que é onde estes arquivos moravam.
pub(crate) fn arquivo(nome: &str) -> PathBuf {
    match PASTA.get() {
        Some(dir) => dir.join(nome),
        None => std::env::temp_dir().join(nome),
    }
}

/// Onde o arquivo morava antes desta mudança. Só para a migração de quem já jogava.
fn arquivo_legado(nome: &str) -> PathBuf {
    std::env::temp_dir().join(nome)
}

/// Lê uma preferência, migrando de `%TEMP%` na primeira vez.
///
/// A migração é uma cópia, não um `rename`: o arquivo velho fica onde está (é `%TEMP%`, o
/// Windows limpa sozinho) e o novo passa a mandar. Assim uma falha de escrita no destino
/// não apaga o valor antigo junto.
pub(crate) fn ler(nome: &str) -> Option<String> {
    let novo = arquivo(nome);
    if let Ok(s) = std::fs::read_to_string(&novo) {
        return Some(s);
    }
    let velho = arquivo_legado(nome);
    if velho == novo {
        return None;
    }
    let s = std::fs::read_to_string(&velho).ok()?;
    if let Err(e) = gravar(nome, &s) {
        crate::diagnostico::linha(
            "iracing",
            &format!("Não consegui migrar {nome} de %TEMP% para a pasta do app: {e}"),
        );
    }
    Some(s)
}

/// Grava uma preferência. Devolve o erro em vez de engoli-lo: quem chama decide se
/// registra — perder estes valores em silêncio foi o defeito original.
pub(crate) fn gravar(nome: &str, conteudo: &str) -> std::io::Result<()> {
    let destino = arquivo(nome);
    if let Some(pai) = destino.parent() {
        std::fs::create_dir_all(pai)?;
    }
    std::fs::write(destino, conteudo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_init_o_caminho_cai_no_temporario() {
        // O binário de teste não passa pelo `setup()` do Tauri; a pasta fica sem fixar e
        // o módulo não pode entrar em pânico nem gravar na raiz do disco.
        let p = arquivo("loop_teste_prefs.txt");
        assert!(p.is_absolute());
        assert_eq!(p.file_name().unwrap(), "loop_teste_prefs.txt");
    }

    #[test]
    fn grava_e_le_o_que_gravou() {
        let nome = "loop_teste_prefs_ida_e_volta.txt";
        gravar(nome, "42").expect("gravação em pasta temporária deve funcionar");
        assert_eq!(ler(nome).as_deref(), Some("42"));
        let _ = std::fs::remove_file(arquivo(nome));
    }

    #[test]
    fn preferencia_ausente_devolve_nada() {
        let nome = "loop_teste_prefs_que_nunca_existiu.txt";
        let _ = std::fs::remove_file(arquivo(nome));
        assert_eq!(ler(nome), None);
    }
}
