//! Leitura da sessão: parsers do YAML do iRacing e os campos do monitor que
//! guardam a identidade da sessão (pista, subsessão, classes, nomes, quali).

use super::*;

/// Lê `CarIsAI`/`CarIsPaceCar`/`CarClassID` por `CarIdx` do `DriverInfo` no YAML.
/// Retorna `[(is_ai, is_pace, class_id); 64]`. Varredura por linha (sem parser YAML).
/// Lê `WeekendInfo:TrackID` do YAML de sessão (a pista da corrida). 0 se ausente.
pub(crate) fn parse_track_id(yaml: &str) -> i64 {
    for line in yaml.lines() {
        if let Some(rest) = line.trim().strip_prefix("TrackID:") {
            if let Ok(id) = rest.trim().parse::<i64>() {
                return id;
            }
        }
    }
    0
}

/// Lê `WeekendInfo:SubSessionID` do YAML. 0 se ausente ou inválido.
pub(crate) fn parse_subsession_id(yaml: &str) -> i64 {
    for line in yaml.lines() {
        if let Some(rest) = line.trim().strip_prefix("SubSessionID:") {
            if let Ok(id) = rest.trim().parse::<i64>() {
                return id;
            }
        }
    }
    0
}

/// `SessionNum` da sessão de QUALIFY no YAML (-1 se não houver). Varre
/// `SessionInfo:Sessions` e casa o `SessionType` que contém "qualify".
pub(crate) fn parse_qualy_session_num(yaml: &str) -> i32 {
    let mut cur_num: i32 = -1;
    for line in yaml.lines() {
        // Idem `parse_race_session_num`: sem tirar o "- " da lista o `SessionNum` nunca
        // casa, a função devolve -1 sempre e o gate da quali fica desarmado — voltas e
        // paradas da classificatória entravam no histórico da CORRIDA.
        let t = line.trim();
        let t = t.strip_prefix("- ").unwrap_or(t);
        if let Some(rest) = t.strip_prefix("SessionNum:") {
            cur_num = rest.trim().parse::<i32>().unwrap_or(-1);
        } else if let Some(rest) = t.strip_prefix("SessionType:") {
            if rest.to_lowercase().contains("qualify") {
                return cur_num;
            }
        }
    }
    -1
}

/// `SessionNum` da sessão de CORRIDA no YAML (-1 se não houver). Mesmo formato do
/// parser de quali, mas casando `SessionType: Race` EXATO — "Open Qualify" e
/// "Lone Qualify" também contêm palavras genéricas, e um `contains` aqui pegaria
/// qualquer sessão cujo tipo mencionasse corrida.
///
/// Existe porque o snapshot do grid (posição na classe no verde) só pode ser tirado
/// na corrida. Treino livre e classificatória também passam por `SessionState =
/// Racing`, então sem este número a primeira sessão do fim de semana consumia o
/// gate e o grid da corrida vinha do treino.
pub(crate) fn parse_race_session_num(yaml: &str) -> i32 {
    let mut cur_num: i32 = -1;
    for line in yaml.lines() {
        // `Sessions:` é uma LISTA no YAML do iRacing, então a primeira chave de cada
        // item vem com "- " na frente (`- SessionNum: 0`). Sem tirar o traço, o número
        // nunca casa e todo fim de semana pareceria não ter corrida.
        let t = line.trim();
        let t = t.strip_prefix("- ").unwrap_or(t);
        if let Some(rest) = t.strip_prefix("SessionNum:") {
            cur_num = rest.trim().parse::<i32>().unwrap_or(-1);
        } else if let Some(rest) = t.strip_prefix("SessionType:") {
            if rest
                .trim()
                .trim_matches('"')
                .trim()
                .eq_ignore_ascii_case("race")
            {
                return cur_num;
            }
        }
    }
    -1
}

/// Carro do JOGADOR (`CarScreenName`) no `DriverInfo`. Acha o `DriverCarIdx` (o índice
/// do próprio jogador, que vem antes da lista) e devolve o `CarScreenName` da entrada
/// com aquele `CarIdx`. `None` se o YAML não trouxer os dois.
///
/// Existe só para a telemetria de produto: sem o carro, o tempo de volta não é
/// comparável entre jogadores — a chave de comparação é (pista, carro).
pub(crate) fn parse_player_car_name(yaml: &str) -> Option<String> {
    let mut driver_car_idx: Option<usize> = None;
    for line in yaml.lines() {
        if let Some(rest) = line.trim().strip_prefix("DriverCarIdx:") {
            driver_car_idx = rest.trim().parse::<usize>().ok();
            break;
        }
    }
    let target = driver_car_idx?;

    let mut current: Option<usize> = None;
    for line in yaml.lines() {
        let t = line.trim();
        let t = t.strip_prefix("- ").unwrap_or(t);
        if let Some(rest) = t.strip_prefix("CarIdx:") {
            current = rest.trim().parse::<usize>().ok();
        } else if let Some(rest) = t.strip_prefix("CarScreenName:") {
            if current == Some(target) {
                let name = rest.trim().trim_matches('"').trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

/// Número de cada carro (`CarNumberRaw`) por `CarIdx` do `DriverInfo`. A ponte para
/// o nosso `driver_id` (nós exportamos o roster, então o número é o que demos).
pub(crate) fn parse_car_numbers(yaml: &str) -> [i32; 64] {
    let mut out = [0i32; 64];
    let mut current: Option<usize> = None;
    for line in yaml.lines() {
        let t = line.trim();
        let t = t.strip_prefix("- ").unwrap_or(t);
        if let Some(rest) = t.strip_prefix("CarIdx:") {
            current = rest.trim().parse::<usize>().ok().filter(|n| *n < 64);
        } else if let Some(rest) = t.strip_prefix("CarNumberRaw:") {
            if let Some(i) = current {
                out[i] = rest.trim().parse::<i32>().unwrap_or(0);
            }
        }
    }
    out
}

pub(crate) fn parse_driver_classes(yaml: &str) -> [(bool, bool, i64); 64] {
    let mut out = [(true, false, 0i64); 64]; // padrão: IA, não pace, sem classe
    let mut current: Option<usize> = None;
    for line in yaml.lines() {
        let t = line.trim();
        let t = t.strip_prefix("- ").unwrap_or(t);
        if let Some(rest) = t.strip_prefix("CarIdx:") {
            current = rest.trim().parse::<usize>().ok().filter(|n| *n < 64);
        } else if let Some(rest) = t.strip_prefix("CarIsAI:") {
            if let Some(i) = current {
                out[i].0 = rest.trim() == "1";
            }
        } else if let Some(rest) = t.strip_prefix("CarIsPaceCar:") {
            if let Some(i) = current {
                out[i].1 = rest.trim() == "1";
            }
        } else if let Some(rest) = t.strip_prefix("CarClassID:") {
            if let Some(i) = current {
                out[i].2 = rest.trim().parse::<i64>().unwrap_or(0);
            }
        }
    }
    out
}

/// Mapeia `CarClassID -> CarClassShortName` (do `DriverInfo` do YAML), para
/// rotular as abas por categoria no overlay multiclasse. O `CarClassID` aparece
/// antes do `CarClassShortName` dentro do bloco de cada piloto.
pub(crate) fn parse_class_names(yaml: &str) -> Vec<(i64, String)> {
    let mut out: Vec<(i64, String)> = Vec::new();
    let mut cur_class: Option<i64> = None;
    for line in yaml.lines() {
        let t = line.trim();
        let t = t.strip_prefix("- ").unwrap_or(t);
        if let Some(rest) = t.strip_prefix("CarClassID:") {
            cur_class = rest.trim().parse::<i64>().ok();
        } else if let Some(rest) = t.strip_prefix("CarClassShortName:") {
            if let Some(id) = cur_class.filter(|c| *c != 0) {
                let name = rest.trim().trim_matches('"').trim();
                if !name.is_empty() && name != "null" && !out.iter().any(|(i, _)| *i == id) {
                    out.push((id, name.to_string()));
                }
            }
        }
    }
    out
}

/// Mapeia `CarIdx -> UserName` (nome do piloto) do `DriverInfo` do YAML. O
/// `UserName` aparece logo após o `CarIdx` no bloco de cada piloto.
pub(crate) fn parse_driver_names(yaml: &str) -> Vec<(i32, String)> {
    let mut out: Vec<(i32, String)> = Vec::new();
    let mut current: Option<i32> = None;
    for line in yaml.lines() {
        let t = line.trim();
        let t = t.strip_prefix("- ").unwrap_or(t);
        if let Some(rest) = t.strip_prefix("CarIdx:") {
            current = rest.trim().parse::<i32>().ok();
        } else if let Some(rest) = t.strip_prefix("UserName:") {
            if let Some(idx) = current {
                let name = rest.trim().trim_matches('"').trim();
                if !name.is_empty() && name != "null" && !out.iter().any(|(i, _)| *i == idx) {
                    out.push((idx, name.to_string()));
                }
            }
        }
    }
    out
}

impl RaceMonitor {
    /// Atualiza a classificação dos carros a partir do `DriverInfo` do YAML.
    pub(super) fn set_car_classes(&mut self, classes: &[(bool, bool, i64); 64]) {
        for i in 0..64 {
            self.car_is_ai[i] = classes[i].0;
            self.car_is_pace[i] = classes[i].1;
            self.car_class_id[i] = classes[i].2;
        }
    }

    /// Guarda os nomes curtos das classes (do `DriverInfo` do YAML).
    pub(super) fn set_class_names(&mut self, names: Vec<(i64, String)>) {
        if !names.is_empty() {
            self.class_names = names;
        }
    }

    /// Guarda os nomes dos pilotos por car_idx (do `DriverInfo` do YAML).
    pub(super) fn set_driver_names(&mut self, names: Vec<(i32, String)>) {
        if !names.is_empty() {
            self.driver_names = names;
        }
    }

    /// Guarda a pista da sessão (do `WeekendInfo:TrackID`).
    pub(super) fn set_session_track_id(&mut self, track_id: i64) {
        if track_id > 0 {
            self.session_track_id = track_id;
        }
    }

    /// Guarda o carro do jogador (do `CarScreenName` do YAML).
    pub(super) fn set_session_car_name(&mut self, name: Option<String>) {
        if let Some(n) = name {
            if !n.is_empty() {
                self.session_car_name = Some(n);
            }
        }
    }

    /// Guarda o redline do carro (do `DriverInfo:DriverCarRedLine`) pro estilo de pilotagem.
    pub(super) fn set_car_redline(&mut self, redline: Option<f64>) {
        if let Some(rpm) = redline {
            if rpm > 0.0 {
                self.car_redline = Some(rpm);
            }
        }
    }

    /// Número do carro do JOGADOR nesta sessão (do `CarNumberRaw`), se conhecido.
    pub(super) fn player_car_number(&self) -> Option<u32> {
        let idx = self.history.player_car_idx;
        if idx < 0 || idx as usize >= self.car_number.len() {
            return None;
        }
        let n = self.car_number[idx as usize];
        if n > 0 {
            Some(n as u32)
        } else {
            None
        }
    }

    pub(super) fn reset_qualy_state(&mut self) {
        self.prev_in_qualy = false;
        self.qualy_laps.clear();
        self.qualy_car_lap_completed = [0; 64];
    }

    pub(super) fn set_session_subsession_id(&mut self, id: i64) {
        if id <= 0 {
            return;
        }
        if self.session_subsession_id != id {
            self.reset_qualy_state();
            self.session_subsession_id = id;
        }
    }

    /// Guarda o número da sessão de qualify (do YAML).
    pub(super) fn set_qualy_session_num(&mut self, num: i32) {
        if self.qualy_session_num != num {
            self.reset_qualy_state();
        }
        self.qualy_session_num = num;
    }

    /// Guarda o número da sessão de corrida (do YAML). Sem reset associado: ao
    /// contrário da quali, este número não guarda estado derivado — é só a régua
    /// que diz "a sessão atual é a corrida".
    pub(super) fn set_race_session_num(&mut self, num: i32) {
        self.race_session_num = num;
    }

    /// A sessão que está rolando AGORA é a corrida? Quando o YAML não traz sessão de
    /// corrida (-1) responde `false`: melhor não capturar grid nenhum — e mostrar
    /// delta zerado — do que capturar o do treino e mostrar número errado.
    pub(super) fn in_race_session(&self, t: &IracingTelemetry) -> bool {
        self.race_session_num >= 0 && t.session_num == self.race_session_num
    }

    /// Guarda o número de cada carro (do `CarNumberRaw`).
    pub(super) fn set_car_numbers(&mut self, numbers: &[i32; 64]) {
        self.car_number = *numbers;
    }

    /// Captura as voltas da sessão de QUALI (fora do gate de corrida). Roda todo
    /// tick; só age quando a sessão atual é a de qualify. Zera ao entrar numa quali
    /// nova (novo fim de semana). As voltas servem de amostra de ritmo LIMPO.
    pub(super) fn capture_qualy(&mut self, t: &IracingTelemetry) {
        let in_qualy = self.qualy_session_num >= 0 && t.session_num == self.qualy_session_num;
        if in_qualy && !self.prev_in_qualy {
            self.reset_qualy_state();
        }
        self.prev_in_qualy = in_qualy;
        if !in_qualy {
            return;
        }
        for car in &t.cars {
            let i = car.idx;
            if i < 0 || i as usize >= 64 || self.car_is_pace[i as usize] {
                continue;
            }
            if car.lap_completed > self.qualy_car_lap_completed[i as usize] {
                self.qualy_car_lap_completed[i as usize] = car.lap_completed;
                if car.last_lap_time > 0.0 && car.lap_completed >= 1 {
                    self.qualy_laps.push(CarLap {
                        car_idx: i,
                        lap: car.lap_completed,
                        time: car.last_lap_time,
                    });
                    if self.qualy_laps.len() > MAX_CAR_LAPS {
                        self.qualy_laps.remove(0);
                    }
                }
            }
        }
    }

    pub(super) fn qualy_laps_snapshot(&self) -> Vec<CarLap> {
        self.qualy_laps.clone()
    }
}
