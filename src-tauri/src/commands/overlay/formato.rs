//! Diagnóstico, formatação e chaves de ordenação da torre de tempos.

use std::collections::HashMap;

use crate::iracing_sdk::{race_monitor, tire_strategy, CarSnapshot};

/// Diagnóstico do overlay: grava o ESTADO atual em `%TEMP%\iracer_overlay_data.log`,
/// só quando o estado MUDA (o front chama ~1×/s; sem flood). Diz exatamente em qual
/// porta o `get_overlay_data` está saindo (telemetria/cars/cruzamento/sucesso).
pub(crate) fn dbg_state(state: &str) {
    use std::io::Write;
    use std::sync::Mutex;
    static LAST: Mutex<Option<String>> = Mutex::new(None);
    {
        let mut last = match LAST.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if last.as_deref() == Some(state) {
            return;
        }
        *last = Some(state.to_string());
    }
    let mut path = std::env::temp_dir();
    path.push("iracer_overlay_data.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{secs}] {state}");
    }
}

/// Quantas voltas recentes do líder entram na mediana do ritmo de referência.
///
/// Cinco cobre uma janela curta o bastante para acompanhar a evolução da prova (pista
/// borrachando, combustível queimando) e longa o bastante para uma parada de box ou uma
/// volta sob amarela não arrastarem a mediana — que é justamente o que a mediana resolve
/// e a média não resolveria.
const VOLTAS_DO_RITMO: usize = 5;

/// Ritmo de referência para estimar quantas voltas ainda cabem numa prova por TEMPO.
///
/// `voltas_do_lider` vem em ordem cronológica; `melhor_do_campo` é a reserva.
///
/// A conta antiga dividia o tempo restante pela MELHOR volta absoluta do campo, e
/// subestimava o total de forma sistemática: a melhor volta é mais rápida que o ritmo
/// real de corrida (tráfego, combustível, pneu), então cabiam menos voltas do que a
/// divisão dizia. A mediana das últimas voltas do líder é o ritmo que a prova está de
/// fato andando.
///
/// A reserva continua sendo a melhor volta: no começo da corrida ninguém tem cinco voltas
/// fechadas ainda, e um número aproximado no cabeçalho é melhor que nenhum.
pub(crate) fn ritmo_de_referencia(
    voltas_do_lider: &[f64],
    melhor_do_campo: Option<f64>,
) -> Option<f64> {
    let mut recentes: Vec<f64> = voltas_do_lider
        .iter()
        .copied()
        .filter(|t| *t > 0.0)
        .rev()
        .take(VOLTAS_DO_RITMO)
        .collect();
    // Menos de três voltas não formam mediana que valha: um in-lap sozinho mandaria.
    if recentes.len() < 3 {
        return melhor_do_campo.filter(|t| *t > 0.0);
    }
    recentes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(recentes[recentes.len() / 2])
}

pub(crate) fn fmt_lap(secs: f64) -> String {
    if secs <= 0.0 {
        return String::new();
    }
    let total_ms = (secs * 1000.0).round() as i64;
    let m = total_ms / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}.{:03}", m, s, ms)
}

pub(crate) fn wetness_to_condition(w: i32) -> &'static str {
    match w {
        n if n >= 4 => "rain", // LightlyWet+
        1 => "clear",          // Dry
        _ => "clouds",         // MostlyDry / desconhecido
    }
}

pub(crate) fn compound_str(c: tire_strategy::Compound) -> &'static str {
    match c {
        tire_strategy::Compound::Wet => "wet",
        _ => "dry",
    }
}

/// Mapa `SessionNum -> SessionType` (ex.: "Race", "Open Qualify", "Practice") do
/// YAML. Em cada bloco de sessão o `SessionNum` vem antes do `SessionType`.
///
/// `SessionInfo:Sessions` é uma LISTA, então a primeira chave de cada item vem com
/// "- " na frente (`- SessionNum: 0`). Sem tirar o traço o número nunca casa, o mapa
/// sai VAZIO e [`session_kind`] cai no `_ => "R"`: toda sessão vira corrida. O efeito
/// visível era na torre da classificatória, que passava a ordenar por progresso na
/// pista (`OrderMode::Progresso`) em vez de por tempo de volta. É o mesmo tropeço que
/// os parsers de `race_monitor::sessao` já pagaram uma vez.
pub(crate) fn parse_session_types(yaml: &str) -> HashMap<i32, String> {
    let mut map = HashMap::new();
    let mut cur: Option<i32> = None;
    for line in yaml.lines() {
        let l = line.trim();
        let l = l.strip_prefix("- ").unwrap_or(l);
        if let Some(v) = l.strip_prefix("SessionNum:") {
            cur = v.trim().parse::<i32>().ok();
        } else if let Some(v) = l.strip_prefix("SessionType:") {
            if let Some(n) = cur {
                map.insert(n, v.trim().to_string());
            }
        }
    }
    map
}

/// Duração DECLARADA de cada sessão no YAML (`SessionTime: 480.0000 sec`), em segundos.
///
/// É a fonte de tempo que sobrevive quando o canal de telemetria não ajuda: sessão sem
/// relógio traz `unlimited`, que não faz número e fica de fora do mapa, e o mesmo vale
/// para o sentinela de "ilimitado". O bloco é o MESMO que [`parse_session_types`]
/// percorre — em cada item da lista o `SessionNum` vem antes, e o "- " do primeiro campo
/// precisa cair fora para o número casar.
pub(crate) fn parse_session_times(yaml: &str) -> HashMap<i32, f64> {
    let mut map = HashMap::new();
    let mut cur: Option<i32> = None;
    for line in yaml.lines() {
        let l = line.trim();
        let l = l.strip_prefix("- ").unwrap_or(l);
        if let Some(v) = l.strip_prefix("SessionNum:") {
            cur = v.trim().parse::<i32>().ok();
        } else if let Some(v) = l.strip_prefix("SessionTime:") {
            let secs = v
                .trim()
                .trim_end_matches("sec")
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|s| tempo_util(*s));
            if let (Some(n), Some(s)) = (cur, secs) {
                map.insert(n, s);
            }
        }
    }
    map
}

/// "Race"/"Qualify"/"Practice" -> "R"/"Q"/"P".
pub(crate) fn session_kind(session_types: &HashMap<i32, String>, session_num: i32) -> &'static str {
    match session_types.get(&session_num).map(|s| s.as_str()) {
        Some(s) if s.contains("Qualify") => "Q",
        Some(s) if s.contains("Practice") || s.contains("Warmup") => "P",
        _ => "R",
    }
}

pub(crate) fn best_positive_lap(live: Option<f64>, recorded: Option<f64>) -> f64 {
    match (
        live.filter(|secs| *secs > 0.0),
        recorded.filter(|secs| *secs > 0.0),
    ) {
        (Some(live), Some(recorded)) => live.min(recorded),
        (Some(live), None) => live,
        (None, Some(recorded)) => recorded,
        (None, None) => 0.0,
    }
}

/// O histórico capturado é da MESMA sessão que está rolando agora?
///
/// Igualdade simples — cobre os dois mundos:
///   • ONLINE/hosted: `WeekendInfo:SubSessionID` > 0, único por evento.
///   • OFFLINE (aiseason de IA — o caso do jogo): o iRacing manda SubSessionID = **0**.
///
/// Antes exigíamos `> 0`, então offline caía em `0 == 0` → **false**, e o overlay descartava
/// TODO o histórico ao vivo: sem `grid_by_idx` (delta sempre "— 0") e sem `tire_by_idx`
/// (a coluna de pneu nunca acumulava as paradas). O histórico já é resetado por
/// tentativa/`session_num` (`record_history`), então confiar no id igual — inclusive 0 —
/// é seguro contra dados de uma corrida anterior.
pub(crate) fn history_matches_subsession(history_id: i64, current_id: i64) -> bool {
    history_id == current_id
}

/// Acima disto, o número de voltas que o iRacing manda é o sentinela de "ilimitado"
/// (`SessionLapsTotal`/`SessionLapsRemainEx` valem 32767 em prova por TEMPO), e não uma
/// contagem. Ver `docs/iracing-dados-disponiveis.md`.
pub(crate) const SENTINELA_VOLTAS: i32 = 10_000;

/// Acima disto, o tempo que o iRacing manda é o sentinela de "ilimitado" (`SessionTimeTotal`
/// vale 604800 s — uma semana — em sessão sem relógio), e não uma duração. Um dia de sessão
/// já é mais do que qualquer prova do Loop, então o corte serve para os dois canais de tempo.
pub(crate) const SENTINELA_TEMPO_S: f64 = 86_400.0;

/// O valor de tempo é uma duração de verdade, e não o sentinela de "ilimitado"?
pub(crate) fn tempo_util(secs: f64) -> bool {
    secs > 0.0 && secs < SENTINELA_TEMPO_S
}

/// O par volta/total do cabeçalho da torre.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ContagemVoltas {
    /// Volta em curso do líder. Nunca passa do total quando o total é conhecido.
    pub lap: i32,
    /// Total da prova. 0 = desconhecido (o front esconde o "/total").
    pub total: i32,
}

/// Volta em curso e total da prova, a partir do que o SDK entrega.
///
/// A conta ingênua — "volta em curso = maior volta completa + 1" — inventa uma volta a
/// mais no instante em que o líder cruza a linha final: numa prova de 8, ele completa a
/// oitava, `lap_completed` vira 8 e o cabeçalho anuncia a volta 9, que ninguém vai correr.
/// Duas regras tiram isso da raiz.
///
/// A primeira é o TOTAL não sair das voltas restantes. `SessionLapsRemainEx` chega a zero
/// quando o líder termina, então derivar o total dele fazia o "/8" desaparecer do
/// cabeçalho na bandeirada, justo quando ele seria o teto. `SessionLapsTotal` é fixo pela
/// prova inteira e por isso vem primeiro; as restantes ficam como reserva para quando o
/// canal do total não chega.
///
/// A segunda é que cruzar a linha sob a quadriculada NÃO abre volta nova. Depois da
/// bandeirada (`encerrada`) o líder já completou a última e o resto do pelotão está
/// terminando essa mesma volta, então a volta em curso é a que ele completou. Isso é o
/// que segura o número em prova por TEMPO, onde não existe total previsto para servir de
/// teto — e no cool down, em que o iRacing continua contando voltas de quem segue na
/// pista.
///
/// `por_voltas` distingue os dois regimes: só numa prova por VOLTAS os canais de
/// contagem do iRacing valem, e é só nela que existe um total previsto para servir de
/// teto. `estimativa_restantes` é o palpite de voltas que ainda cabem em prova por tempo
/// (`None` em prova por voltas ou quando não há ritmo de referência).
pub(crate) fn contagem_de_voltas(
    max_completed: i32,
    por_voltas: bool,
    laps_total: i32,
    laps_remain_ex: i32,
    encerrada: bool,
    estimativa_restantes: Option<i32>,
) -> ContagemVoltas {
    let max_completed = max_completed.max(0);

    // Depois da bandeirada a volta em curso é a última COMPLETA, não a seguinte. O piso
    // de 1 evita "Volta 0" numa bandeirada disparada antes de qualquer volta fechar.
    let lap = if encerrada {
        max_completed.max(1)
    } else {
        max_completed + 1
    };

    let declarado =
        (por_voltas && laps_total > 0 && laps_total < SENTINELA_VOLTAS).then_some(laps_total);
    let por_restantes = (por_voltas && laps_remain_ex > 0 && laps_remain_ex < SENTINELA_VOLTAS)
        .then(|| max_completed + laps_remain_ex);

    match declarado.or(por_restantes) {
        // Prova por VOLTAS: o total é a autoridade e a volta em curso não passa dele.
        Some(total) => ContagemVoltas {
            lap: lap.min(total),
            total,
        },
        // Prova por TEMPO: não há total fixo, só estimativa — e ela nunca pode ficar
        // atrás da volta em curso, senão o "/total" some do cabeçalho na reta final.
        None => ContagemVoltas {
            lap,
            total: estimativa_restantes
                .map(|restantes| (max_completed + restantes.max(0)).max(lap))
                .unwrap_or(0),
        },
    }
}

/// Cor usada quando o time do piloto não foi resolvido (sem contrato / sem carreira).
pub(crate) const NEUTRAL_TEAM_COLOR: &str = "#7d8590";

/// Chave de comparação de nome para o join de RESERVA entre o `UserName` que o SDK
/// devolve e o nosso elenco. Como somos NÓS que exportamos o roster da IA, o nome do
/// piloto no iRacing é literalmente o `nome` do nosso piloto — só normalizamos pontas
/// e caixa para o casamento não depender de formatação.
pub(crate) fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

pub(crate) fn roster_with_telemetry<'a>(
    roster: &'a [race_monitor::YamlCarMeta],
    telemetry: &'a [CarSnapshot],
) -> Vec<(&'a race_monitor::YamlCarMeta, Option<&'a CarSnapshot>)> {
    let telemetry_by_idx: HashMap<i32, &CarSnapshot> =
        telemetry.iter().map(|car| (car.idx, car)).collect();

    roster
        .iter()
        .filter(|meta| !meta.is_pace)
        .map(|meta| (meta, telemetry_by_idx.get(&meta.idx).copied()))
        .collect()
}
