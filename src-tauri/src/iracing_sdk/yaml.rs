//! Leitura do YAML de sessão do iRacing. O YAML do sim é raso o suficiente para
//! uma varredura por linha bastar — sem precisar de um parser YAML completo.

/// Extrai `TrackDisplayName: ...` da string YAML de sessão, se presente.
/// O YAML do iRacing é raso o suficiente para uma varredura por linha bastar
/// neste teste — sem precisar de um parser YAML completo.
pub(crate) fn parse_track_name(yaml: &str) -> Option<String> {
    yaml.lines()
        .find_map(|line| line.trim().strip_prefix("TrackDisplayName:"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Extrai o `custid` (id de cliente iRacing) do JOGADOR do YAML de sessão:
/// `DriverInfo.DriverCarIdx` diz qual carro é o do jogador, e o `UserID` do
/// piloto com aquele `CarIdx` é o custid. Varredura por linha (sem parser YAML).
pub fn parse_player_custid(yaml: &str) -> Option<i64> {
    let target = yaml
        .lines()
        .find_map(|line| line.trim().strip_prefix("DriverCarIdx:"))
        .and_then(|v| v.trim().parse::<i64>().ok())?;

    let mut current: Option<i64> = None;
    for line in yaml.lines() {
        let t = line.trim();
        let t = t.strip_prefix("- ").unwrap_or(t);
        if let Some(rest) = t.strip_prefix("CarIdx:") {
            current = rest.trim().parse::<i64>().ok();
        } else if let Some(rest) = t.strip_prefix("UserID:") {
            if current == Some(target) {
                return rest.trim().parse::<i64>().ok().filter(|id| *id > 0);
            }
        }
    }
    None
}

/// Redline do carro do jogador (RPM) do YAML de sessão (`DriverInfo.DriverCarRedLine`).
/// Referência pro estilo de pilotagem (colado no limitador / short-shift). Varredura por
/// linha. `None` se ausente (o consumidor trata como redline desconhecido → ignora rotação).
pub fn parse_car_redline(yaml: &str) -> Option<f64> {
    yaml.lines()
        .find_map(|line| line.trim().strip_prefix("DriverCarRedLine:"))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|rpm| *rpm > 0.0)
}
