//! RÁDIO DA EQUIPE: as quebras da corrida viram frases do engenheiro.

use std::collections::HashMap;

use super::demo::{demo_breakdown_feed, session_driver_names};
use super::tipos::BreakdownMessage;
use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::drivers as dq;
use crate::iracing_sdk::race_monitor;

/// Peça com preposição/artigo pra frase do engenheiro ("no motor", "na suspensão").
pub(crate) fn part_com_artigo(part_key: &str) -> &'static str {
    match part_key {
        "engine" => "no motor",
        "gearbox" => "no câmbio",
        "brakes" => "nos freios",
        "suspension" => "na suspensão",
        "cooling" => "no arrefecimento",
        "front_wing" => "na asa dianteira",
        "rear_wing" => "na asa traseira",
        "sidepods" => "nas laterais",
        "underbody" => "no assoalho",
        "chassis" => "no chassi",
        "electronics" => "na parte elétrica",
        _ => "no carro",
    }
}

/// FONTE ÚNICA das redações do rádio: 3 opções por (peça, severidade), voz do engenheiro,
/// 3ª pessoa. Devolve o TRECHO depois do nome (a peça já vem escrita no texto). A causa
/// concreta (`detail`) é anexada pelo card na mesma linha. Peça/severidade desconhecida cai
/// num genérico. O DNF é tratado à parte (`dnf_frase`), pois a redação é diferente.
pub(crate) fn breakdown_frases(part_key: &str, severity: &str) -> [&'static str; 3] {
    match (severity, part_key) {
        // ── MOTOR ──
        ("light", "engine") => [
            "sente o motor perdendo fôlego",
            "relata o motor engasgando",
            "avisa que o motor não está redondo",
        ],
        ("heavy", "engine") => [
            "está com o motor em pane",
            "perdeu potência e o motor pode não aguentar",
            "relata o motor no limite — situação séria",
        ],
        // ── CÂMBIO ──
        ("light", "gearbox") => [
            "está com o câmbio arisco nas trocas",
            "relata engates falhando no câmbio",
            "sente o câmbio embolando as marchas",
        ],
        ("heavy", "gearbox") => [
            "está com o câmbio travando",
            "perdeu marchas — o câmbio está indo embora",
            "relata o câmbio prestes a parar",
        ],
        // ── FREIOS ──
        ("light", "brakes") => [
            "sente o pedal de freio amolecendo",
            "relata os freios pedindo água",
            "avisa que os freios estão longos",
        ],
        ("heavy", "brakes") => [
            "está praticamente sem freio",
            "relata os freios cozinhando",
            "perdeu o pedal — freios em pane",
        ],
        // ── SUSPENSÃO ──
        ("light", "suspension") => [
            "sente a suspensão reclamando nas zebras",
            "relata o carro batendo demais atrás",
            "avisa de uma folga na suspensão",
        ],
        ("heavy", "suspension") => [
            "está com a suspensão comprometida",
            "relata algo quebrado na suspensão",
            "perdeu firmeza — suspensão em pane",
        ],
        // ── ARREFECIMENTO ──
        ("light", "cooling") => [
            "vê a temperatura subindo aos poucos",
            "relata o arrefecimento no limite",
            "avisa que a água está esquentando",
        ],
        ("heavy", "cooling") => [
            "está com o arrefecimento estourando",
            "relata superaquecimento crítico",
            "perdeu o arrefecimento — temperatura no vermelho",
        ],
        // ── ASA DIANTEIRA ──
        ("light", "front_wing") => [
            "relata a asa dianteira leve",
            "sente o bico perdendo apoio",
            "avisa de dano na asa dianteira",
        ],
        ("heavy", "front_wing") => [
            "está com a asa dianteira danificada",
            "perdeu apoio na frente — asa comprometida",
            "relata a asa dianteira se soltando",
        ],
        // ── ASA TRASEIRA ──
        ("light", "rear_wing") => [
            "sente a traseira solta na reta",
            "relata a asa traseira leve",
            "avisa de dano na asa traseira",
        ],
        ("heavy", "rear_wing") => [
            "está com a asa traseira danificada",
            "perdeu apoio atrás — traseira nervosa",
            "relata a asa traseira cedendo",
        ],
        // ── LATERAIS ──
        ("light", "sidepods") => [
            "relata dano nas laterais",
            "sente o carro puxando de lado",
            "avisa de um amassado nas laterais",
        ],
        ("heavy", "sidepods") => [
            "está com as laterais abertas",
            "perdeu parte da lateral — carro ferido",
            "relata dano sério nas laterais",
        ],
        // ── ASSOALHO ──
        ("light", "underbody") => [
            "sente o assoalho raspando",
            "relata perda de apoio no assoalho",
            "avisa de dano no assoalho",
        ],
        ("heavy", "underbody") => [
            "está com o assoalho comprometido",
            "perdeu downforce — assoalho ferido",
            "relata o assoalho arrastando forte",
        ],
        // ── CHASSI ──
        ("light", "chassis") => [
            "sente o chassi estranho",
            "relata o carro desalinhado",
            "avisa de algo torto no chassi",
        ],
        ("heavy", "chassis") => [
            "está com o chassi comprometido",
            "relata dano estrutural no chassi",
            "perdeu rigidez — chassi ferido",
        ],
        // ── PARTE ELÉTRICA ──
        ("light", "electronics") => [
            "relata oscilações na parte elétrica",
            "sente o painel piscando",
            "avisa de falha elétrica intermitente",
        ],
        ("heavy", "electronics") => [
            "está com pane elétrica",
            "perdeu comandos — parte elétrica em pane",
            "relata o carro cortando — falha elétrica",
        ],
        // ── genérico ──
        ("heavy", _) => [
            "está com um problema grave no carro",
            "relata um problema sério no carro",
            "perdeu desempenho — algo grave no carro",
        ],
        _ => [
            "apresenta um problema no carro",
            "relata algo estranho no carro",
            "avisa de um problema no carro",
        ],
    }
}

/// Redação de ABANDONO (DNF) — 3 opções. Usa a peça com preposição (`part_com_artigo`).
pub(crate) fn dnf_frase(name: &str, part: &str, variant: usize) -> String {
    match variant % 3 {
        0 => format!("{name} está fora — problemas {part}"),
        1 => format!("{name} abandona a corrida com problemas {part}"),
        _ => format!("{name} foi retirado da corrida — problemas {part}"),
    }
}

/// Feed do RÁDIO DA EQUIPE: as quebras da corrida em andamento viram frases do engenheiro
/// (piloto resolvido pelo número → nosso elenco). O front mostra as NOVAS (por `id`). Não
/// depende de telemetria — lê o log vivo do monitor + o banco da carreira.
#[tauri::command]
pub fn get_breakdown_feed(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Vec<BreakdownMessage>, String> {
    use tauri::Manager;

    // Modo demo: mostra templates ciclando (pra posicionar/ver o overlay do rádio), com
    // os pilotos REAIS do grid. Ligado pelo botão das Configurações OU pela env.
    if crate::commands::overlay_window::demo_enabled() {
        return Ok(demo_breakdown_feed(&session_driver_names(&app, &career_id)));
    }

    let log = race_monitor::peek_breakdown_log();
    if log.is_empty() {
        return Ok(Vec::new());
    }

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let numbers: HashMap<String, i64> =
        std::fs::read_to_string(crate::commands::iracing::numbers_path(&base_dir, &career_id))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    let by_number: HashMap<i64, String> = numbers.into_iter().map(|(id, n)| (n, id)).collect();

    let out = log
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let name = by_number
                .get(&(o.car_number as i64))
                .and_then(|id| dq::get_driver(&db.conn, id).ok())
                .map(|d| d.nome)
                .unwrap_or_else(|| format!("Carro #{}", o.car_number));
            // Redação DIRETA a partir da fonte única (`breakdown_frases`): 3 opções por
            // (peça, severidade). Variante ESTÁVEL por carro+ordem (mesma quebra → mesma
            // frase; a grade varia). A causa (`detail`) segue na mesma linha no card.
            let variant = (o.car_number as usize).wrapping_add(i) % 3;
            let text = if o.severity == "dnf" {
                dnf_frase(&name, part_com_artigo(&o.part), variant)
            } else {
                format!("{name} {}", breakdown_frases(&o.part, &o.severity)[variant])
            };
            BreakdownMessage {
                id: i,
                severity: o.severity.clone(),
                text,
                detail: o.label.clone(),
            }
        })
        .collect();

    Ok(out)
}
