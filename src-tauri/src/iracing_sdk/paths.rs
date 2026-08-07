//! Resolução robusta dos caminhos do iRacing na pasta Documentos do usuário.
//!
//! O iRacing guarda `app.ini`, `airosters/`, `aiseasons/` etc. dentro de
//! `Documentos/iRacing`. Mas "Documentos" varia: pode estar redirecionado ao
//! OneDrive (pessoal ou empresarial), localizado por idioma ("Documentos",
//! "Dokumente", "文档") ou movido. Em vez de adivinhar nomes, perguntamos ao
//! Windows o caminho REAL via Known Folder API (`FOLDERID_Documents`), com uma
//! heurística por variável de ambiente como rede de segurança.

use std::path::PathBuf;

/// Pasta "Documentos" real do usuário. `None` se não der para resolver.
pub fn documents_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(p) = known_documents() {
            return Some(p);
        }
    }
    fallback_documents()
}

/// `Documentos/iRacing`.
pub fn iracing_dir() -> Option<PathBuf> {
    documents_dir().map(|d| d.join("iRacing"))
}

/// Caminhos candidatos do `app.ini`, em ordem de preferência: primeiro o resolvido
/// pela Known Folder API (robusto a OneDrive/idioma/relocação), depois heurísticas
/// por variável de ambiente como rede de segurança.
///
/// Devolve candidatos, não um caminho pronto, de propósito: [`documents_dir`] pode
/// resolver uma pasta Documentos que existe mas NÃO é a que tem o `iRacing/` dentro
/// (perfil com OneDrive parcialmente migrado). Quem escolhe é [`app_ini_path`], que
/// fica com o primeiro candidato que é arquivo de verdade.
pub fn app_ini_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(iracing) = iracing_dir() {
        out.push(iracing.join("app.ini"));
    }
    if let Ok(up) = std::env::var("USERPROFILE") {
        let base = PathBuf::from(&up);
        for docs in [
            "Documents",
            "OneDrive/Documents",
            "OneDrive/Documentos",
            "Documentos",
        ] {
            out.push(base.join(docs).join("iRacing").join("app.ini"));
        }
    }
    out
}

/// O `app.ini` do iRacing, se existir em algum dos candidatos.
pub fn app_ini_path() -> Option<PathBuf> {
    app_ini_candidates().into_iter().find(|p| p.is_file())
}

/// `Documentos/iRacing/airosters` (rosters de IA).
#[allow(dead_code)] // consumido pela futura geração de roster
pub fn airosters_dir() -> Option<PathBuf> {
    iracing_dir().map(|d| d.join("airosters"))
}

/// `Documentos/iRacing/aiseasons` (temporadas de IA).
#[allow(dead_code)] // consumido pela futura geração de temporada
pub fn aiseasons_dir() -> Option<PathBuf> {
    iracing_dir().map(|d| d.join("aiseasons"))
}

/// Pasta de pintura de um carro. O nome da pasta é o `carPath` com `\` virando
/// espaço (ex.: `mx5\mx52016` → `mx5 mx52016`, confirmado nos arquivos reais).
pub fn car_paint_dir(car_path: &str) -> Option<PathBuf> {
    let folder = car_path.replace('\\', " ");
    iracing_dir().map(|d| d.join("paint").join(folder))
}

/// Pergunta ao Windows a pasta Documentos via `SHGetKnownFolderPath`. Resolve
/// OneDrive, idioma e relocação de forma canônica.
#[cfg(windows)]
fn known_documents() -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::ptr;
    use winapi::shared::ntdef::PWSTR;
    use winapi::um::combaseapi::CoTaskMemFree;
    use winapi::um::knownfolders::FOLDERID_Documents;
    use winapi::um::shlobj::SHGetKnownFolderPath;

    unsafe {
        let mut path_ptr: PWSTR = ptr::null_mut();
        let hr = SHGetKnownFolderPath(&FOLDERID_Documents, 0, ptr::null_mut(), &mut path_ptr);
        if hr < 0 || path_ptr.is_null() {
            return None;
        }
        // A string é terminada em NUL (UTF-16); mede até ele.
        let mut len = 0usize;
        while *path_ptr.add(len) != 0 {
            len += 1;
        }
        let wide = std::slice::from_raw_parts(path_ptr, len);
        let os = OsString::from_wide(wide);
        CoTaskMemFree(path_ptr as *mut _);
        let path = PathBuf::from(os);
        if path.as_os_str().is_empty() {
            None
        } else {
            Some(path)
        }
    }
}

/// Rede de segurança quando a API falha (ou em build não-Windows): tenta nomes
/// comuns de "Documentos" sob o perfil do usuário.
fn fallback_documents() -> Option<PathBuf> {
    let base = PathBuf::from(std::env::var("USERPROFILE").ok()?);
    for docs in [
        "Documents",
        "OneDrive/Documents",
        "OneDrive/Documentos",
        "Documentos",
    ] {
        let candidate = base.join(docs);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}
