//! A FILA DE REENVIO em disco: o que não entrou espera a próxima abertura do app.
//!
//! É a rede-de-segurança que a medição de 08/08/2026 exigiu (ver o cabeçalho do módulo
//! pai): sem ela, um Cloud Run frio come o evento e ninguém fica sabendo. Formato JSON
//! Lines, uma linha por evento, para uma escrita interrompida custar um evento em vez
//! do arquivo inteiro.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use super::{
    agora, entrega::entregar, is_enabled, FILA_FRACASSOS_ATE_DESISTIR, FILA_MAX_EVENTOS,
    FILA_MAX_IDADE_SECS,
};

/// Arquivo da fila de reenvio. `None` antes do `init()` do boot — sem ele a telemetria
/// continua funcionando, só sem rede-de-segurança.
static FILA: OnceLock<PathBuf> = OnceLock::new();

/// Serializa leitura e escrita da fila: cada envio tem sua própria thread, e a drenagem
/// do boot é mais uma.
static FILA_TRAVA: Mutex<()> = Mutex::new(());

/// Define o arquivo da fila. Uma vez só, no `init` do boot.
pub(super) fn definir_arquivo(caminho: PathBuf) {
    let _ = FILA.set(caminho);
}

/// Um evento que não entrou, guardado inteiro. O corpo já vem montado com o
/// contexto de carreira do instante em que nasceu — remontá-lo na drenagem
/// descreveria a carreira de hoje, e não a daquela corrida.
#[derive(Serialize, Deserialize, Clone)]
struct Pendente {
    criado_em: i64,
    corpo: serde_json::Value,
}

/// Guarda um evento que não entrou. `pub(super)` porque quem chama é o `send` da fachada.
pub(super) fn enfileirar(corpo: serde_json::Value, criado_em: i64) {
    let Some(path) = FILA.get() else {
        return;
    };
    let Ok(_trava) = FILA_TRAVA.lock() else {
        return;
    };
    let mut pendentes = ler_fila(path);
    pendentes.push(Pendente { criado_em, corpo });
    let descartados = podar(&mut pendentes, agora());
    let total = pendentes.len();
    gravar_fila(path, &pendentes);
    let sobre_descarte = if descartados > 0 {
        format!(", {descartados} descartado(s) por idade ou teto")
    } else {
        String::new()
    };
    crate::diagnostico::linha(
        "telemetria",
        &format!("enfileirado para reenvio ({total} na fila{sobre_descarte})"),
    );
}

/// Manda o que ficou para trás. Chamada UMA vez no boot, depois do `init()`.
///
/// Roda em thread própria porque a primeira tentativa pode pagar o cold start
/// inteiro, e o boot não espera por telemetria.
pub fn drenar_fila() {
    if !is_enabled() {
        return;
    }
    let Some(path) = FILA.get() else {
        return;
    };
    std::thread::spawn(move || {
        // Tira tudo do arquivo de uma vez: o que não for entregue volta no fim. Se
        // o app morrer no meio, perde-se a fila, e isso é melhor que reenviar em
        // duplicata a cada boot.
        let pendentes = {
            let Ok(_trava) = FILA_TRAVA.lock() else {
                return;
            };
            let lidos = ler_fila(path);
            let _ = std::fs::remove_file(path);
            lidos
        };
        if pendentes.is_empty() {
            return;
        }
        crate::diagnostico::linha(
            "telemetria",
            &format!("drenando a fila ({} pendente(s))", pendentes.len()),
        );

        let (mut entregues, mut fracassos_seguidos) = (0usize, 0u32);
        let mut sobraram: Vec<Pendente> = Vec::new();
        for (i, p) in pendentes.iter().enumerate() {
            // Uma tentativa por evento aqui. Quem insiste é o boot seguinte: com
            // 200 na fila, as esperas do envio normal virariam mais de uma hora de
            // thread para nada.
            if fracassos_seguidos >= FILA_FRACASSOS_ATE_DESISTIR || !is_enabled() {
                sobraram.extend_from_slice(&pendentes[i..]);
                break;
            }
            if entregar(&p.corpo, p.criado_em, &[]) {
                entregues += 1;
                fracassos_seguidos = 0;
            } else {
                fracassos_seguidos += 1;
                sobraram.push(p.clone());
            }
        }

        crate::diagnostico::linha(
            "telemetria",
            &format!(
                "fila drenada: {entregues} entregue(s), {} de volta na fila",
                sobraram.len()
            ),
        );
        if sobraram.is_empty() || !is_enabled() {
            return;
        }
        let Ok(_trava) = FILA_TRAVA.lock() else {
            return;
        };
        // Junta com o que chegou enquanto a drenagem rodava, em vez de sobrescrever.
        // Os que sobraram vêm primeiro: são os mais antigos.
        let mut atual = sobraram;
        atual.extend(ler_fila(path));
        podar(&mut atual, agora());
        gravar_fila(path, &atual);
    });
}

/// Apaga a fila. Chamado quando o consentimento cai: evento pendente é fala já
/// engatilhada, e guardá-la seria continuar falando pelas costas de quem pediu silêncio.
pub(super) fn apagar() {
    let Some(path) = FILA.get() else {
        return;
    };
    let Ok(_trava) = FILA_TRAVA.lock() else {
        return;
    };
    let _ = std::fs::remove_file(path);
}

/// JSON Lines, e não um array: uma linha corrompida (queda de energia no meio da
/// escrita) custa aquele evento, em vez do arquivo inteiro.
fn ler_fila(path: &Path) -> Vec<Pendente> {
    let Ok(texto) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    texto
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Pendente>(l).ok())
        .collect()
}

fn gravar_fila(path: &Path, pendentes: &[Pendente]) {
    if pendentes.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }
    let mut texto = String::new();
    for p in pendentes {
        if let Ok(linha) = serde_json::to_string(p) {
            texto.push_str(&linha);
            texto.push('\n');
        }
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, texto);
}

/// Tira o que envelheceu e corta pelo teto, mantendo os MAIS ANTIGOS: o começo da
/// corrida explica o fim, e a ordem do reenvio importa para o upsert do servidor.
/// Devolve quantos saíram.
fn podar(pendentes: &mut Vec<Pendente>, agora: i64) -> usize {
    let antes = pendentes.len();
    pendentes.retain(|p| agora - p.criado_em <= FILA_MAX_IDADE_SECS);
    pendentes.truncate(FILA_MAX_EVENTOS);
    antes - pendentes.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Só a parte de disco e de poda tem teste. O envio em si depende do servidor,
    /// e o que se pode afirmar sem rede é que o evento que não entrou sobrevive ao
    /// fechamento do app — que é exatamente o buraco que a fila veio tapar.
    fn dir_temp(rotulo: &str) -> PathBuf {
        let unico = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("relógio antes da época")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("loop_telemetria_{rotulo}_{unico}"));
        std::fs::create_dir_all(&dir).expect("criar dir temporário");
        dir
    }

    fn pendente(evento: &str, criado_em: i64) -> Pendente {
        Pendente {
            criado_em,
            corpo: json!({ "event": evento, "install_id": "abc" }),
        }
    }

    #[test]
    fn fila_sobrevive_a_ida_e_volta_do_disco() {
        let path = dir_temp("ida_volta").join("fila.jsonl");
        let pendentes = vec![pendente("race_start", 100), pendente("race_end", 200)];
        gravar_fila(&path, &pendentes);

        let lidos = ler_fila(&path);
        assert_eq!(lidos.len(), 2);
        assert_eq!(lidos[0].criado_em, 100);
        assert_eq!(lidos[1].corpo["event"], "race_end");
    }

    #[test]
    fn linha_corrompida_custa_so_aquele_evento() {
        let path = dir_temp("corrompida").join("fila.jsonl");
        let bom = serde_json::to_string(&pendente("race_start", 100)).unwrap();
        let outro = serde_json::to_string(&pendente("race_end", 200)).unwrap();
        std::fs::write(&path, format!("{bom}\n{{isto não é json\n{outro}\n")).unwrap();

        let lidos = ler_fila(&path);
        assert_eq!(lidos.len(), 2, "a linha quebrada levou as vizinhas junto");
    }

    #[test]
    fn fila_vazia_apaga_o_arquivo() {
        let path = dir_temp("vazia").join("fila.jsonl");
        gravar_fila(&path, &[pendente("race_start", 100)]);
        assert!(path.exists());

        gravar_fila(&path, &[]);
        assert!(!path.exists(), "arquivo vazio ficou para trás");
        assert!(ler_fila(&path).is_empty());
    }

    #[test]
    fn poda_tira_o_que_envelheceu() {
        let agora = 1_000_000;
        let mut pendentes = vec![
            pendente("velho", agora - FILA_MAX_IDADE_SECS - 1),
            pendente("no_limite", agora - FILA_MAX_IDADE_SECS),
            pendente("novo", agora - 60),
        ];

        assert_eq!(podar(&mut pendentes, agora), 1);
        assert_eq!(pendentes.len(), 2);
        assert_eq!(pendentes[0].corpo["event"], "no_limite");
    }

    #[test]
    fn poda_corta_pelo_teto_mantendo_os_mais_antigos() {
        let agora = 1_000_000;
        let mut pendentes: Vec<Pendente> = (0..FILA_MAX_EVENTOS + 10)
            .map(|i| pendente(&format!("e{i}"), agora - 60))
            .collect();

        assert_eq!(podar(&mut pendentes, agora), 10);
        assert_eq!(pendentes.len(), FILA_MAX_EVENTOS);
        assert_eq!(
            pendentes[0].corpo["event"], "e0",
            "o corte comeu pelo começo da fila"
        );
    }
}
