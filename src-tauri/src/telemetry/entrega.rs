//! A ENTREGA de um evento: o POST, as tentativas e o veredito de cada desfecho.
//!
//! Separado da fachada porque é a única parte que fala com a rede, e a única cujas
//! constantes foram calibradas contra uma medição (08/08/2026, ver o cabeçalho do
//! módulo pai). Quem monta o evento não precisa saber nada disto; quem mexe nos
//! números de espera e timeout não precisa ler o resto.

use std::time::Duration;

use serde_json::json;

use crate::narrative::client::APP_SECRET;

use super::{agora, ATRASO_MINIMO_SECS, ENDPOINT, TIMEOUT_SECS};

/// Desfecho de um POST, na granularidade que decide o que fazer em seguida.
enum Resultado {
    Entregue(u16),
    /// O servidor entendeu e recusou (corpo malformado, segredo errado). Repetir
    /// não conserta, e enfileirar só encheria o disco com um evento natimorto.
    Recusado(u16),
    /// Rede, timeout ou 5xx. Vale tentar de novo.
    Falhou(String),
}

/// Entrega um evento, com até `esperas.len() + 1` tentativas. Devolve `true`
/// quando o assunto está resolvido — entregue OU recusado em definitivo.
///
/// `criado_em` é o carimbo de quando o evento NASCEU. Ele não vai no payload como
/// data (quem carimba a hora é o servidor, porque o relógio da máquina do jogador
/// não é confiável); vira o campo `atraso_s`, que é a informação de que o servidor
/// precisa para não tratar um evento drenado de ontem como se fosse de agora.
pub(super) fn entregar(corpo: &serde_json::Value, criado_em: i64, esperas: &[u64]) -> bool {
    let evento = corpo
        .get("event")
        .and_then(|e| e.as_str())
        .unwrap_or("?")
        .to_string();

    let Ok(cliente) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
    else {
        crate::diagnostico::linha("telemetria", &format!("{evento}: sem cliente HTTP"));
        return false;
    };

    let mut corpo = corpo.clone();
    for tentativa in 0..=esperas.len() {
        // Recalculado a cada tentativa: entre a primeira e a última passam ~35s, e
        // na drenagem podem ter passado dias.
        let atraso = (agora() - criado_em).max(0);
        if let Some(map) = corpo.as_object_mut() {
            if atraso >= ATRASO_MINIMO_SECS {
                map.insert("atraso_s".into(), json!(atraso));
            }
        }

        match postar(&cliente, &corpo) {
            Resultado::Entregue(status) => {
                let sufixo = if tentativa > 0 {
                    format!(" na tentativa {}", tentativa + 1)
                } else {
                    String::new()
                };
                crate::diagnostico::linha(
                    "telemetria",
                    &format!("{evento}: entregue ({status}){sufixo}, atraso {atraso}s"),
                );
                return true;
            }
            Resultado::Recusado(status) => {
                crate::diagnostico::linha(
                    "telemetria",
                    &format!("{evento}: recusado pelo servidor ({status}), descartado"),
                );
                return true;
            }
            Resultado::Falhou(motivo) => {
                crate::diagnostico::linha(
                    "telemetria",
                    &format!("{evento}: falhou ({motivo}) na tentativa {}", tentativa + 1),
                );
                if let Some(espera) = esperas.get(tentativa) {
                    std::thread::sleep(Duration::from_secs(*espera));
                }
            }
        }
    }
    false
}

fn postar(cliente: &reqwest::blocking::Client, corpo: &serde_json::Value) -> Resultado {
    match cliente
        .post(ENDPOINT)
        .header("x-app-secret", APP_SECRET)
        .json(corpo)
        .send()
    {
        Ok(resposta) => {
            let status = resposta.status();
            let codigo = status.as_u16();
            if status.is_success() {
                // 202 com `dropped: daily_cap` cai aqui de propósito: o servidor
                // recebeu e decidiu não guardar. Reenviar não muda a decisão dele.
                Resultado::Entregue(codigo)
            } else if status.is_client_error() && codigo != 408 && codigo != 429 {
                Resultado::Recusado(codigo)
            } else {
                Resultado::Falhou(format!("HTTP {codigo}"))
            }
        }
        Err(e) if e.is_timeout() => Resultado::Falhou("timeout".to_string()),
        Err(e) if e.is_connect() => Resultado::Falhou("sem conexão".to_string()),
        Err(e) => Resultado::Falhou(e.to_string()),
    }
}
