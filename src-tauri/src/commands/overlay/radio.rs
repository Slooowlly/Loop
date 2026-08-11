//! RÁDIO DA EQUIPE: as quebras da corrida viram frases do engenheiro.
//!
//! A REDAÇÃO não mora aqui. Ela mora em [`crate::engenheiro::quebra`], junto com a decisão de
//! como o piloto é nomeado e com o catálogo de peças a gravar — porque a mesma frase tem que
//! sair idêntica no card e no áudio, e manter as duas em arquivos diferentes é garantir que
//! elas divirjam. Aqui ficam só a resolução do carro para o nosso elenco e a leitura do log.

use std::collections::HashMap;

use super::demo::{demo_breakdown_feed, session_driver_names};
use super::tipos::BreakdownMessage;
use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::drivers as dq;
use crate::engenheiro::quebra;
use crate::iracing_sdk::race_monitor;

// O `#[path]` explícito é herdado: `overlay.rs` declara este arquivo com `#[path]`, e aí os
// submódulos daqui resolvem relativos à pasta DELE, não à de `radio.rs`.
#[path = "radio/contexto.rs"]
mod contexto;

// A fonte única das redações mudou de casa; estes nomes seguem valendo para quem já os usava
// (o tour do demo), sem uma segunda cópia do texto.
pub(crate) use quebra::{breakdown_frases, part_com_artigo};

/// Feed do RÁDIO DE RITMO: a volta mais rápida da corrida e a nossa aproximação dela.
///
/// Canal SEPARADO do feed de quebras, e não uma mistura, porque os dois crescem em ritmos
/// próprios: intercalá-los num só stream faria os ids de um embaralharem os do outro, e o
/// overlay — que mostra o de maior id — passaria a engolir mensagens conforme a ordem de
/// chegada. Dois streams, cada um com o próprio cursor; quem arbitra a tela é o front.
#[tauri::command]
pub fn get_pace_feed(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Vec<BreakdownMessage>, String> {
    use crate::engenheiro::{nomes, tempo_volta as tv};
    use crate::iracing_sdk::race_monitor::FalaDeRitmo;
    use tauri::Manager;

    let log = race_monitor::peek_ritmo_log();
    if log.is_empty() {
        return Ok(Vec::new());
    }

    // O nome só é preciso na fala que nomeia alguém. Abrir o banco para "Essa foi a volta mais
    // rápida da corrida!" seria pagar I/O por nada, num caminho de polling.
    let precisa_nome = log
        .iter()
        .any(|f| matches!(f, FalaDeRitmo::MelhorDeOutro { .. }));
    let elenco = precisa_nome
        .then(|| {
            let base_dir = app.path().app_data_dir().ok()?;
            let config = AppConfig::load_or_default(&base_dir);
            let db = Database::open_existing(
                &config.saves_dir().join(&career_id).join("career.db"),
            )
            .ok()?;
            let numbers: HashMap<String, i64> = std::fs::read_to_string(
                crate::commands::iracing::numbers_path(&base_dir, &career_id),
            )
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
            let by_number: HashMap<i64, String> =
                numbers.into_iter().map(|(id, n)| (n, id)).collect();
            Some((db, by_number))
        })
        .flatten();

    let out = log
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let (text, pecas) = match f {
                FalaDeRitmo::Tomamos(chave) => (
                    tv::tomamos_frase(i).1.to_string(),
                    vec![chave.clone()],
                ),
                FalaDeRitmo::Aproximando(chave) => {
                    // O texto do card sai da MESMA função que gerou a chave, pelo número que a
                    // chave carrega — reescrevê-lo aqui seria a segunda redação da mesma frase.
                    let decimos = chave
                        .rsplit('_')
                        .next()
                        .and_then(|n| n.parse::<i32>().ok())
                        .unwrap_or(0);
                    let texto = tv::aproximacao_frase(decimos)
                        .map(|(_, t)| t)
                        .unwrap_or_else(|| "Estamos perto da melhor volta.".to_string());
                    (texto, vec![chave.clone()])
                }
                FalaDeRitmo::MelhorDeOutro {
                    lead,
                    car_number,
                    tempo,
                    decimos,
                } => {
                    let nome = elenco
                        .as_ref()
                        .and_then(|(db, by_number)| {
                            by_number
                                .get(&(*car_number as i64))
                                .and_then(|id| dq::get_driver(&db.conn, id).ok())
                        })
                        .map(|d| nomes::sobrenome_de(&d.nome).to_string());
                    let tempo_txt = tv::tempo_falado(*decimos).unwrap_or_default();
                    match nome.as_deref().and_then(|s| {
                        nomes::chave_sobrenome(s).map(|chave| (s.to_string(), chave))
                    }) {
                        Some((sobrenome, chave_nome)) => (
                            format!("{} {sobrenome}, {tempo_txt}.", tv::LEAD_MELHOR.1),
                            vec![lead.clone(), chave_nome, tempo.clone()],
                        ),
                        // Sem gravação do sobrenome (o jogador, ou piloto de save antigo): o
                        // card conta com o número do carro e o áudio cala. Anunciar sem sujeito
                        // — "A volta mais rápida da corrida é do, um trinta e dois" — seria pior
                        // que o silêncio.
                        None => (
                            format!("Volta mais rápida da corrida: carro {car_number}, {tempo_txt}."),
                            Vec::new(),
                        ),
                    }
                }
            };
            BreakdownMessage {
                id: race_monitor::radio_epoch() + i,
                severity: "pace".to_string(),
                text,
                detail: String::new(),
                pecas,
            }
        })
        .collect();

    Ok(out)
}

/// Redação de ABANDONO com o nome embutido, para o tour do demo — que mostra a frase pronta e
/// não monta peça nenhuma. O caminho de corrida usa [`quebra::montar`], que separa o nome do
/// trecho porque o áudio é colado.
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

    // O carro do JOGADOR fora do feed da grade. A linha dele continua no log (é ela que manda o
    // comando ao sim e alimenta o resultado), mas aqui ela sairia em 3ª pessoa: como o nome do
    // jogador não sai de pool nenhum, a fala caía na forma pela equipe — "o piloto um da tal
    // equipe abandonou" — sobre ele mesmo, 1,3 vez por corrida (medido em `radio-carga.md`). O
    // desfecho dele vive em `get_player_warnings`, na 2ª pessoa.
    let eu = race_monitor::player_car_number();
    let log: Vec<_> = race_monitor::peek_breakdown_log()
        .into_iter()
        .filter(|o| Some(o.car_number) != eu)
        .collect();
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

    let numbers: HashMap<String, i64> = std::fs::read_to_string(
        crate::commands::iracing::numbers_path(&base_dir, &career_id),
    )
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default();
    let by_number: HashMap<i64, String> = numbers.into_iter().map(|(id, n)| (n, id)).collect();

    // Uma leitura do save por chamada do feed, não por quebra: rivalidade, nêmesis, elenco das
    // equipes e tabela do campeonato saem todos daqui.
    let mundo = contexto::Mundo::carregar(&db.conn);

    // Abandonos ACUMULADOS até cada linha, contando ela. O comentário de atrito sai no
    // cruzamento exato do limiar, então a conta tem que ser a do momento da quebra — e não a
    // do fim da corrida, que faria a fala aparecer retroativamente na primeira leitura.
    let mut acumulado = 0u32;
    let abandonos_ate: Vec<u32> = log
        .iter()
        .map(|o| {
            if o.severity.e_abandono() {
                acumulado += 1;
            }
            acumulado
        })
        .collect();

    // O contexto de cada linha, resolvido uma vez. A fusão precisa olhar o PAR, então não dá
    // para montar a fala no mesmo passo que resolve o piloto.
    let contextos: Vec<quebra::Contexto> = log
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let piloto_id = by_number.get(&(o.car_number as i64)).cloned();
            let name = piloto_id
                .as_deref()
                .and_then(|id| dq::get_driver(&db.conn, id).ok())
                .map(|d| d.nome)
                .unwrap_or_else(|| format!("Carro #{}", o.car_number));
            // Variante ESTÁVEL por carro+ordem: a mesma quebra dá sempre a mesma frase, e a
            // grade varia.
            let variant = (o.car_number as usize).wrapping_add(i) % 3;
            mundo.contexto(
                piloto_id.as_deref(),
                &name,
                &o.part,
                o.severity.key(),
                variant,
                abandonos_ate[i],
            )
        })
        .collect();

    // FUSÃO de quebras simultâneas.
    //
    // Duas na mesma VOLTA são o mesmo instante para o modelo: o tick da grade avalia cada carro
    // no cruzamento da linha, e o `on_lap_at` resolve a volta inteira de uma vez. Sem juntar,
    // elas viram duas mensagens — e o overlay, que mostra a de maior `id`, engoliria a primeira
    // sem card e sem áudio. A fusão conserta isso pela raiz, em vez de enfileirar duas falas de
    // sete segundos.
    //
    // O `id` de uma fala fundida é o da ÚLTIMA linha que ela cobre. Assim ele continua
    // crescendo monotonicamente e — porque o agrupamento é determinístico sobre um log que só
    // cresce no fim — continua o mesmo a cada leitura, que é do que o cursor do front depende.
    let mut out: Vec<BreakdownMessage> = Vec::new();
    let mut i = 0usize;
    while i < log.len() {
        let par = (i + 1 < log.len() && log[i].lap == log[i + 1].lap)
            .then(|| quebra::montar_duplo(&contextos[i], &contextos[i + 1]))
            .flatten();
        match par {
            Some(fala) => {
                out.push(BreakdownMessage {
                    id: i + 1,
                    severity: log[i].severity.key().to_string(),
                    // O detalhe some na fusão: são duas causas concretas, e escolher uma seria
                    // dizer que a outra não houve.
                    detail: String::new(),
                    text: fala.texto,
                    pecas: fala.pecas,
                });
                i += 2;
            }
            None => {
                let fala = quebra::montar(&contextos[i]);
                out.push(BreakdownMessage {
                    id: race_monitor::radio_epoch() + i,
                    severity: log[i].severity.key().to_string(),
                    text: fala.texto,
                    detail: log[i].label.clone(),
                    pecas: fala.pecas,
                });
                i += 1;
            }
        }
    }

    Ok(out)
}
