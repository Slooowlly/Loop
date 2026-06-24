//! Parser dos RESULTADOS OFICIAIS do iRacing (a partir do YAML de sessão).
//!
//! O iRacing roda a própria simulação da corrida E da classificação até o fim,
//! independente do que o jogador faça — se ele bate na volta 2 e sai, o resultado
//! final ainda existe. Esses resultados ficam no YAML de sessão em
//! `SessionInfo:Sessions:[].ResultsPositions` (um por sessão: QUALIFY, RACE).
//!
//! É a fonte AUTORITATIVA do resultado: posições finais, melhor volta, voltas
//! lideradas, incidentes e motivo de abandono — e o grid sai do ResultsPositions
//! da quali. Muito mais robusto que reconstruir amostrando a telemetria ao vivo
//! (que falha quando o jogador sai cedo).
//!
//! Sem dependência de lib YAML (o projeto evita) — varredura por linha, como o
//! resto do módulo (ver `parse_player_custid`). A estrutura é regular o bastante.

use std::collections::HashMap;

/// Uma posição no resultado de uma sessão (um carro).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResultPos {
    pub car_idx: i32,
    /// Posição GERAL (1-based).
    pub position: i32,
    /// Posição na CLASSE (0-based no iRacing — somar 1 para a nossa convenção).
    pub class_position: i32,
    /// Melhor volta em segundos; <= 0 = sem volta válida.
    pub fastest_time: f64,
    /// Última volta em segundos; <= 0 = sem volta válida.
    pub last_time: f64,
    pub laps_complete: i32,
    pub laps_led: i32,
    pub incidents: i32,
    /// "Running" = classificado/terminou; qualquer outra coisa = abandono.
    pub reason_out: String,
}

impl ResultPos {
    pub fn is_dnf(&self) -> bool {
        !self.reason_out.eq_ignore_ascii_case("Running")
    }
}

/// Resultados oficiais extraídos do YAML de sessão.
#[derive(Debug, Clone, Default)]
pub struct SessionResults {
    /// CarIdx do jogador (`DriverInfo:DriverCarIdx`). -1 se não achado.
    pub player_car_idx: i32,
    /// Incidentes do jogador (`DriverInfo:DriverIncidentCount`).
    pub player_incidents: i32,
    /// CarIdx → número do carro (`CarNumberRaw`), de `DriverInfo:Drivers`.
    pub car_number_by_idx: HashMap<i32, i32>,
    /// Resultado da QUALI (o grid). Vazio se não houve quali.
    pub qualify: Vec<ResultPos>,
    /// Resultado da CORRIDA (final). Vazio se ainda não foi rodada/finalizada.
    pub race: Vec<ResultPos>,
}

impl SessionResults {
    /// A corrida tem resultado FINAL utilizável? (ao menos um carro com volta
    /// completa — o iRacing preenche `ResultsPositions` com a ordem de grid e
    /// `LapsComplete=0` antes de a corrida acontecer).
    pub fn race_is_final(&self) -> bool {
        self.race.iter().any(|p| p.laps_complete > 0)
    }

    /// Posição de largada (grid) por CarIdx, da quali — `class_position`+1.
    /// Vazio se não houve quali.
    pub fn grid_by_idx(&self) -> HashMap<i32, i32> {
        self.qualify
            .iter()
            .map(|p| (p.car_idx, p.class_position + 1))
            .collect()
    }
}

fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Extrai o valor após "Chave:" de uma linha já trimada (ou `None`).
fn field<'a>(trimmed: &'a str, key: &str) -> Option<&'a str> {
    trimmed.strip_prefix(key).map(|v| v.trim())
}

/// Parseia os resultados oficiais do YAML de sessão do iRacing.
pub fn parse_session_results(yaml: &str) -> SessionResults {
    let mut out = SessionResults {
        player_car_idx: -1,
        ..Default::default()
    };

    let mut session_kind: Option<&'static str> = None; // "qualify" | "race" | "other"
    let mut in_results = false;
    let mut cur: Option<ResultPos> = None;

    let mut in_drivers = false;
    let mut cur_driver_idx: Option<i32> = None;

    // Empurra a entrada de resultado corrente para o vetor da sessão atual.
    fn flush(
        cur: &mut Option<ResultPos>,
        kind: Option<&str>,
        qualify: &mut Vec<ResultPos>,
        race: &mut Vec<ResultPos>,
    ) {
        if let Some(entry) = cur.take() {
            match kind {
                Some("qualify") => qualify.push(entry),
                Some("race") => race.push(entry),
                _ => {}
            }
        }
    }

    for line in yaml.lines() {
        let t = line.trim();
        let indent = leading_spaces(line);

        // ── Marcadores de seção (encerram blocos abertos) ───────────────────
        if let Some(v) = field(t, "DriverCarIdx:") {
            out.player_car_idx = v.parse().unwrap_or(-1);
            continue;
        }
        if let Some(v) = field(t, "DriverIncidentCount:") {
            out.player_incidents = v.parse().unwrap_or(0);
            continue;
        }
        if t == "Drivers:" {
            flush(&mut cur, session_kind, &mut out.qualify, &mut out.race);
            in_results = false;
            in_drivers = true;
            cur_driver_idx = None;
            continue;
        }
        if let Some(v) = field(t, "SessionType:") {
            flush(&mut cur, session_kind, &mut out.qualify, &mut out.race);
            in_results = false;
            in_drivers = false;
            session_kind = Some(if v.contains("Qualify") {
                "qualify"
            } else if v == "Race" {
                "race"
            } else {
                "other"
            });
            continue;
        }
        if t == "ResultsPositions:" {
            flush(&mut cur, session_kind, &mut out.qualify, &mut out.race);
            in_results = true;
            cur = None;
            continue;
        }

        // ── Lista de pilotos (CarIdx → número) ──────────────────────────────
        if in_drivers {
            if indent == 0 && t.ends_with(':') {
                in_drivers = false; // dedent para um bloco de topo
            } else if let Some(v) = field(t, "- CarIdx:") {
                cur_driver_idx = v.parse().ok();
            } else if let Some(v) = field(t, "CarNumberRaw:") {
                if let (Some(idx), Ok(num)) = (cur_driver_idx, v.parse::<i32>()) {
                    out.car_number_by_idx.insert(idx, num);
                }
            }
            continue;
        }

        // ── ResultsPositions ────────────────────────────────────────────────
        if in_results {
            if let Some(v) = field(t, "- Position:") {
                flush(&mut cur, session_kind, &mut out.qualify, &mut out.race);
                let mut entry = ResultPos::default();
                entry.position = v.parse().unwrap_or(0);
                cur = Some(entry);
            } else if indent >= 5 {
                if let Some(entry) = cur.as_mut() {
                    if let Some(v) = field(t, "ClassPosition:") {
                        entry.class_position = v.parse().unwrap_or(0);
                    } else if let Some(v) = field(t, "CarIdx:") {
                        entry.car_idx = v.parse().unwrap_or(-1);
                    } else if let Some(v) = field(t, "FastestTime:") {
                        entry.fastest_time = v.parse().unwrap_or(-1.0);
                    } else if let Some(v) = field(t, "LastTime:") {
                        entry.last_time = v.parse().unwrap_or(-1.0);
                    } else if let Some(v) = field(t, "LapsComplete:") {
                        entry.laps_complete = v.parse().unwrap_or(0);
                    } else if let Some(v) = field(t, "LapsLed:") {
                        entry.laps_led = v.parse().unwrap_or(0);
                    } else if let Some(v) = field(t, "Incidents:") {
                        entry.incidents = v.parse().unwrap_or(0);
                    } else if let Some(v) = field(t, "ReasonOutStr:") {
                        entry.reason_out = v.to_string();
                    }
                }
            } else {
                // Dedent (3 espaços, ex.: ResultsFastestLap:) encerra o bloco.
                flush(&mut cur, session_kind, &mut out.qualify, &mut out.race);
                in_results = false;
            }
            continue;
        }
    }
    flush(&mut cur, session_kind, &mut out.qualify, &mut out.race);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Recorte real do YAML de sessão do iRacing (quali completa, race ainda em
    // grid + DriverInfo). Indentação preservada do dump.
    const SAMPLE: &str = "\
Sessions:
 - SessionNum: 0
   SessionType: Open Qualify
   SessionName: QUALIFY
   ResultsPositions:
   - Position: 1
     ClassPosition: 0
     CarIdx: 4
     FastestTime: 98.8641
     LastTime: 98.8641
     LapsLed: 0
     LapsComplete: 1
     Incidents: 0
     ReasonOutId: 0
     ReasonOutStr: Running
   - Position: 2
     ClassPosition: 1
     CarIdx: 10
     FastestTime: 100.3085
     LastTime: 100.3085
     LapsLed: 0
     LapsComplete: 1
     Incidents: 0
     ReasonOutId: 0
     ReasonOutStr: Running
   ResultsFastestLap:
   - CarIdx: 4
 - SessionNum: 1
   SessionType: Race
   SessionName: RACE
   ResultsPositions:
   - Position: 1
     ClassPosition: 0
     CarIdx: 4
     FastestTime: 136.4990
     LastTime: 137.0
     LapsLed: 7
     LapsComplete: 7
     Incidents: 0
     ReasonOutId: 0
     ReasonOutStr: Running
   - Position: 12
     ClassPosition: 11
     CarIdx: 0
     FastestTime: -1.0000
     LastTime: -1.0000
     LapsLed: 0
     LapsComplete: 1
     Incidents: 4
     ReasonOutId: 32
     ReasonOutStr: Disconnected
   ResultsFastestLap:
   - CarIdx: 4
DriverInfo:
 DriverCarIdx: 0
 DriverIncidentCount: 4
 Drivers:
 - CarIdx: 0
   UserName: Rodrigo Carvalho Silva
   CarNumber: \"64\"
   CarNumberRaw: 64
 - CarIdx: 4
   UserName: Cooper Hall
   CarNumber: \"4\"
   CarNumberRaw: 4
SplitTimeInfo:
 Sectors:
";

    #[test]
    fn parses_player_and_numbers() {
        let r = parse_session_results(SAMPLE);
        assert_eq!(r.player_car_idx, 0);
        assert_eq!(r.player_incidents, 4);
        assert_eq!(r.car_number_by_idx.get(&0), Some(&64));
        assert_eq!(r.car_number_by_idx.get(&4), Some(&4));
    }

    #[test]
    fn parses_qualify_grid() {
        let r = parse_session_results(SAMPLE);
        assert_eq!(r.qualify.len(), 2);
        let grid = r.grid_by_idx();
        // CarIdx 4 foi pole (ClassPosition 0 -> grid 1).
        assert_eq!(grid.get(&4), Some(&1));
        assert_eq!(grid.get(&10), Some(&2));
    }

    #[test]
    fn parses_race_result_with_dnf() {
        let r = parse_session_results(SAMPLE);
        assert!(r.race_is_final(), "corrida com voltas completas é final");
        assert_eq!(r.race.len(), 2);
        let winner = &r.race[0];
        assert_eq!(winner.car_idx, 4);
        assert_eq!(winner.position, 1);
        assert_eq!(winner.laps_led, 7);
        assert!((winner.fastest_time - 136.499).abs() < 0.001);
        assert!(!winner.is_dnf());
        // O jogador (CarIdx 0) saiu: Disconnected -> DNF, 4 incidentes.
        let player = r.race.iter().find(|p| p.car_idx == 0).unwrap();
        assert!(player.is_dnf());
        assert_eq!(player.incidents, 4);
        assert_eq!(player.class_position + 1, 12);
    }

    #[test]
    fn race_not_final_when_all_zero_laps() {
        let pre = "\
Sessions:
 - SessionNum: 1
   SessionType: Race
   ResultsPositions:
   - Position: 1
     ClassPosition: 0
     CarIdx: 4
     LapsComplete: 0
     ReasonOutStr: Running
DriverInfo:
 DriverCarIdx: 0
";
        let r = parse_session_results(pre);
        assert!(!r.race_is_final(), "grid-only não é resultado final");
    }
}
