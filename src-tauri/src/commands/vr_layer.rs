//! Instalação e detecção da API layer do OpenXR (o LEITOR do overlay de VR).
//!
//! A layer é uma DLL C++ (`vr-overlay/`) que o loader do OpenXR carrega DENTRO do
//! processo do iRacing quando ele abre em VR. Ela lê a memória compartilhada que o app
//! escreve e sobe os pixels pro quad. Sem ela registrada, o app escreve frames que
//! ninguém lê — o overlay de VR simplesmente não existe naquela máquina.
//!
//! ── Instalação (`instalar`) ──
//! Registrar uma layer implícita é: um manifesto JSON apontando pra DLL, e um valor em
//! `HKCU\Software\Khronos\OpenXR\1\ApiLayers\Implicit` cujo NOME é o caminho do
//! manifesto e o dado é DWORD 0 (habilitada). Por usuário, sem admin.
//!
//! Rodamos isso a CADA BOOT, de propósito: o valor guarda um caminho ABSOLUTO, e se o
//! diretório de instalação mudar (update, reinstalação em outro lugar, perfil movido) o
//! manifesto antigo vira caminho morto e a layer para de carregar SEM ERRO NENHUM —
//! o overlay só não aparece. Reescrever a cada boot faz isso se autocorrigir, e a
//! varredura de valores órfãos evita acumular entradas mortas de instalações passadas.
//!
//! O manifesto vai pro app data (sempre gravável), não ao lado da DLL: a DLL mora no
//! diretório de instalação, que pode ser somente-leitura numa instalação por-máquina.
//!
//! ── Detecção (`sim_em_vr`) ──
//! A layer mantém aberto um evento nomeado `Local\iRacerVrActive` enquanto o iRacing tem
//! uma instância OpenXR viva (ver `SignalVrActive` no overlay_layer.cpp). Aqui a gente só
//! tenta abrir: existir = o sim está em VR agora. O tempo de vida é do SO, então crash do
//! sim apaga o sinal sozinho.
//!
//! A desinstalação do valor de registro é do hook do NSIS (`installer/hooks.nsh`) — é o
//! único momento em que o app não tem como agir por conta própria.

const MANIFEST_FILE: &str = "XR_APILAYER_NOVA_iracer_overlay.json";
const LAYER_NAME: &str = "XR_APILAYER_NOVA_iracer_overlay";
const REG_PATH: &str = r"Software\Khronos\OpenXR\1\ApiLayers\Implicit";

#[cfg(windows)]
mod imp {
    use std::path::{Path, PathBuf};

    use winapi::shared::minwindef::{DWORD, HKEY};
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::synchapi::OpenEventW;
    use winapi::um::winnt::{KEY_READ, KEY_SET_VALUE, KEY_WRITE, REG_DWORD, SYNCHRONIZE};
    use winapi::um::winreg::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegEnumValueW, RegSetValueExW,
        HKEY_CURRENT_USER,
    };

    use super::{LAYER_NAME, MANIFEST_FILE, REG_PATH};

    // Strings do Windows são UTF-16 terminadas em zero.
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// O iRacing está rodando em VR AGORA? Responde por presença do evento que a layer
    /// mantém aberto. `false` também é a resposta quando a layer não está instalada — daí
    /// o modo manual das Configurações continuar existindo como escape.
    pub fn sim_em_vr() -> bool {
        let nome = wide(r"Local\iRacerVrActive");
        // SYNCHRONIZE é o direito mínimo: não vamos esperar nem sinalizar, só perguntar
        // se o objeto existe.
        let h = unsafe { OpenEventW(SYNCHRONIZE, 0, nome.as_ptr()) };
        if h.is_null() {
            return false;
        }
        unsafe { CloseHandle(h) };
        true
    }

    fn escreve_manifesto(destino: &Path, dll: &Path) -> Result<(), String> {
        // JSON exige as barras escapadas.
        let dll_json = dll.to_string_lossy().replace('\\', "\\\\");
        let manifesto = format!(
            r#"{{
  "file_format_version": "1.0.0",
  "api_layer": {{
    "name": "{LAYER_NAME}",
    "library_path": "{dll_json}",
    "api_version": "1.0",
    "implementation_version": "1",
    "description": "Loop — torre de tempos e radio da equipe em VR",
    "disable_environment": "IRACER_OVERLAY_DISABLE"
  }}
}}
"#
        );
        if let Some(pai) = destino.parent() {
            std::fs::create_dir_all(pai).map_err(|e| format!("criar {}: {e}", pai.display()))?;
        }
        // Sem BOM: o parser de manifesto do OpenXR engasga com BOM.
        std::fs::write(destino, manifesto.as_bytes())
            .map_err(|e| format!("escrever {}: {e}", destino.display()))
    }

    struct Chave(HKEY);

    impl Drop for Chave {
        fn drop(&mut self) {
            unsafe { RegCloseKey(self.0) };
        }
    }

    fn abre_chave(reg_path: &str) -> Result<Chave, String> {
        let sub = wide(reg_path);
        let mut h: HKEY = std::ptr::null_mut();
        let rc = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                sub.as_ptr(),
                0,
                std::ptr::null_mut(),
                0,
                KEY_READ | KEY_WRITE | KEY_SET_VALUE,
                std::ptr::null_mut(),
                &mut h,
                std::ptr::null_mut(),
            )
        };
        if rc != 0 {
            return Err(format!("RegCreateKeyExW falhou ({rc})"));
        }
        Ok(Chave(h))
    }

    /// Valores da chave cujo nome termina no nosso manifesto — são de instalações
    /// anteriores (caminho antigo). Colhidos antes de escrever o novo.
    fn orfaos(chave: &Chave, atual: &str) -> Vec<Vec<u16>> {
        let mut fora = Vec::new();
        let mut i: DWORD = 0;
        loop {
            // 32768 = teto de nome de valor no registro (em WCHARs).
            let mut nome = vec![0u16; 32768];
            let mut tam: DWORD = nome.len() as DWORD;
            let rc = unsafe {
                RegEnumValueW(
                    chave.0,
                    i,
                    nome.as_mut_ptr(),
                    &mut tam,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            if rc != 0 {
                break; // ERROR_NO_MORE_ITEMS (ou falha: paramos de qualquer jeito)
            }
            nome.truncate(tam as usize);
            let texto = String::from_utf16_lossy(&nome);
            if texto.ends_with(MANIFEST_FILE) && !texto.eq_ignore_ascii_case(atual) {
                fora.push(wide(&texto));
            }
            i += 1;
        }
        fora
    }

    /// Registra a layer: manifesto no app data + valor em HKCU. Idempotente.
    pub fn instalar(base_dir: &Path, dll: &Path) -> Result<PathBuf, String> {
        instalar_em(base_dir, dll, REG_PATH)
    }

    /// Igual, com a chave de registro EXPLÍCITA. O parâmetro existe pelos testes: sem ele
    /// o único jeito de cobrir a gravação e a limpeza de órfãos seria mexendo na chave
    /// real do OpenXR na máquina de quem roda a suíte.
    pub fn instalar_em(base_dir: &Path, dll: &Path, reg_path: &str) -> Result<PathBuf, String> {
        if !dll.exists() {
            return Err(format!("DLL da layer não encontrada em {}", dll.display()));
        }
        let manifesto = base_dir.join(MANIFEST_FILE);
        escreve_manifesto(&manifesto, dll)?;

        let chave = abre_chave(reg_path)?;
        let atual = manifesto.to_string_lossy().to_string();

        // Limpa entradas de instalações passadas ANTES de gravar a nova, pra a chave nunca
        // ter dois manifestos nossos (o loader carregaria a layer duas vezes).
        //
        // Falhar aqui não aborta a instalação: sem o valor NOVO o overlay não existe, e com
        // o órfão ele existe duas vezes — desenhar dois quads sobrepostos é ruim, não ter
        // overlay nenhum é pior. Mas o retorno não pode ser descartado: um órfão que resiste
        // é exatamente o estado que produz o sintoma "o painel está borrado/duplicado", e
        // sem esta linha no log ninguém liga uma coisa à outra.
        for velho in orfaos(&chave, &atual) {
            let rc = unsafe { RegDeleteValueW(chave.0, velho.as_ptr()) };
            if rc != 0 {
                // `velho` é UTF-16 terminado em zero; tira o terminador antes de imprimir.
                let nome = String::from_utf16_lossy(&velho[..velho.len().saturating_sub(1)]);
                eprintln!(
                    "[vr-layer] AVISO: RegDeleteValueW falhou ({rc}) no valor órfão {nome} — \
                     a layer pode carregar duas vezes"
                );
            }
        }

        let nome = wide(&atual);
        let habilitada: DWORD = 0; // 0 = habilitada (contrato do loader do OpenXR)
        let rc = unsafe {
            RegSetValueExW(
                chave.0,
                nome.as_ptr(),
                0,
                REG_DWORD,
                &habilitada as *const DWORD as *const u8,
                std::mem::size_of::<DWORD>() as DWORD,
            )
        };
        if rc != 0 {
            return Err(format!("RegSetValueExW falhou ({rc})"));
        }
        Ok(manifesto)
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::{Path, PathBuf};

    // Fora do Windows não há iRacing nem OpenXR/DX11 — a integração é stub inerte.
    pub fn sim_em_vr() -> bool {
        false
    }

    pub fn instalar(_base_dir: &Path, _dll: &Path) -> Result<PathBuf, String> {
        Err("API layer do OpenXR só existe no Windows".into())
    }
}

pub use imp::sim_em_vr;

#[cfg(all(test, windows))]
mod tests {
    use super::{imp, LAYER_NAME, MANIFEST_FILE};
    use serial_test::serial;
    use std::path::PathBuf;

    // Chave de TESTE (nunca a real do OpenXR): a suíte roda na máquina de quem
    // desenvolve, e registrar uma layer de verdade num `cargo test` seria efeito
    // colateral em software de terceiro.
    //
    // Os testes que mexem nela são `#[serial]`: a chave é um recurso GLOBAL do processo
    // (como o locale do rust-i18n), e em paralelo a instalação de um teste enxergava o
    // valor do outro como órfão e o apagava.
    const REG_TESTE: &str = r"Software\Loop\TestApiLayers";

    fn temp_dir(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("loop-vr-layer-{nome}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("criar temp");
        d
    }

    fn dll_falsa(dir: &PathBuf, nome: &str) -> PathBuf {
        let p = dir.join(nome);
        std::fs::write(&p, b"MZ").expect("escrever dll falsa");
        p
    }

    fn valores(reg_path: &str) -> Vec<String> {
        // Lê os nomes de valor pelo `reg query` — de propósito por FORA do código sob
        // teste, pra o teste não validar a gravação com a mesma leitura que ele exercita.
        let saida = std::process::Command::new("reg")
            .args(["query", &format!("HKCU\\{reg_path}")])
            .output()
            .expect("reg query");
        String::from_utf8_lossy(&saida.stdout)
            .lines()
            .filter(|l| l.contains(MANIFEST_FILE))
            .map(|l| l.trim().to_string())
            .collect()
    }

    fn limpa_chave() {
        let _ = std::process::Command::new("reg")
            .args(["delete", &format!("HKCU\\{REG_TESTE}"), "/f"])
            .output();
    }

    #[test]
    #[serial]
    fn instalar_escreve_manifesto_e_registra_a_layer() {
        let dir = temp_dir("instala");
        let dll = dll_falsa(&dir, "iracer_overlay_layer.dll");
        limpa_chave();

        let manifesto = imp::instalar_em(&dir, &dll, REG_TESTE).expect("instalar");

        // Manifesto: aponta pra DLL, com as barras escapadas, e sem BOM (o parser de
        // manifesto do OpenXR engasga com BOM).
        let bytes = std::fs::read(&manifesto).expect("ler manifesto");
        assert_ne!(
            &bytes[..3.min(bytes.len())],
            b"\xEF\xBB\xBF",
            "manifesto não deve ter BOM"
        );
        let texto = String::from_utf8(bytes).expect("utf-8");
        assert!(
            texto.contains(LAYER_NAME),
            "manifesto deve declarar o nome da layer"
        );
        assert!(
            texto.contains(&dll.to_string_lossy().replace('\\', "\\\\")),
            "library_path deve apontar pra DLL com as barras escapadas"
        );

        // Registro: um valor, com o caminho do manifesto como NOME.
        let vs = valores(REG_TESTE);
        assert_eq!(vs.len(), 1, "esperado exatamente 1 valor, veio {vs:?}");
        assert!(vs[0].contains(&manifesto.to_string_lossy().to_string()));
        // 0x0 = habilitada, no contrato do loader do OpenXR.
        assert!(
            vs[0].contains("0x0"),
            "a layer deve ficar HABILITADA (DWORD 0): {}",
            vs[0]
        );

        limpa_chave();
    }

    #[test]
    #[serial]
    fn instalar_remove_o_valor_de_uma_instalacao_anterior() {
        // O valor guarda um caminho ABSOLUTO. Se o diretório do app mudar (update,
        // reinstalação em outro lugar), o antigo viraria caminho morto e o loader
        // carregaria a layer duas vezes. O órfão tem que sair.
        let antigo = temp_dir("orfao-antigo");
        let novo = temp_dir("orfao-novo");
        let dll_antiga = dll_falsa(&antigo, "iracer_overlay_layer.dll");
        let dll_nova = dll_falsa(&novo, "iracer_overlay_layer.dll");
        limpa_chave();

        imp::instalar_em(&antigo, &dll_antiga, REG_TESTE).expect("instalar antigo");
        assert_eq!(valores(REG_TESTE).len(), 1);

        imp::instalar_em(&novo, &dll_nova, REG_TESTE).expect("instalar novo");
        let vs = valores(REG_TESTE);
        assert_eq!(
            vs.len(),
            1,
            "o órfão da instalação anterior devia ter saído: {vs:?}"
        );
        assert!(
            vs[0].contains(&novo.join(MANIFEST_FILE).to_string_lossy().to_string()),
            "o valor que sobrou deve ser o NOVO: {}",
            vs[0]
        );

        limpa_chave();
    }

    #[test]
    #[serial]
    fn instalar_falha_quando_a_dll_nao_existe() {
        // Falhar aqui é o certo: melhor logar "não registrei" do que registrar um
        // manifesto apontando pra uma DLL inexistente e o overlay não aparecer sem motivo.
        let dir = temp_dir("sem-dll");
        let erro = imp::instalar_em(&dir, &dir.join("nao-existe.dll"), REG_TESTE)
            .expect_err("devia falhar sem a DLL");
        assert!(erro.contains("não encontrada"), "erro inesperado: {erro}");
    }

    #[test]
    fn sem_o_iracing_em_vr_a_deteccao_e_falsa() {
        // A suíte não roda com o iRacing aberto em VR, então o evento nomeado não existe.
        // Guarda o contrato do "auto": sem sinal, não desenhamos os painéis de VR.
        assert!(!imp::sim_em_vr());
    }
}

/// Registra a layer a partir do resource empacotado. Chamado no boot; falha só loga.
/// O caminho do resource é resolvido pelo Tauri (difere entre dev e instalado).
pub fn instalar_do_bundle(app: &tauri::AppHandle) {
    use tauri::Manager;

    let base_dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[vr-layer] sem app_data_dir: {e}");
            return;
        }
    };
    let dll = match app.path().resolve(
        "resources/iracer_overlay_layer.dll",
        tauri::path::BaseDirectory::Resource,
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[vr-layer] resource da layer não resolvido: {e}");
            return;
        }
    };

    match imp::instalar(&base_dir, &dll) {
        Ok(manifesto) => {
            println!("[vr-layer] registrada: {}", manifesto.display());
        }
        Err(e) => {
            // Não é fatal: o app roda inteiro sem VR. Só o overlay de VR não vai aparecer.
            eprintln!("[vr-layer] falha ao registrar: {e}");
        }
    }
}
