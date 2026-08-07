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

/// Comprimento da pista em METROS, de `WeekendInfo.TrackLength` (que vem como
/// `1.472 km`). É o fator que converte `CarIdxLapDistPct` — a única medida de posição
/// que o SDK publica pros outros carros — em distância. Sem ele o spotter de obstáculo
/// não tem como dizer "a 150 m", só "a 6% da volta".
///
/// Varredura por linha, como o resto: o YAML do sim é raso. `None` quando ausente ou
/// não numérico — o consumidor trata como "sem pista conhecida" e não detecta nada, que
/// é melhor do que detectar com um comprimento chutado.
pub fn parse_track_length_m(yaml: &str) -> Option<f64> {
    yaml.lines()
        .find_map(|line| line.trim().strip_prefix("TrackLength:"))
        .and_then(|v| {
            let v = v.trim();
            let numero = v.split_whitespace().next()?;
            let km = numero.parse::<f64>().ok()?;
            // O sufixo é sempre `km` nas builds vistas, mas não custa não confiar.
            let metros = if v.ends_with("mi") { km * 1609.344 } else { km * 1000.0 };
            Some(metros)
        })
        .filter(|m| *m > 100.0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_comprimento_da_pista_como_o_iracing_escreve() {
        // A forma real, de uma captura de Lime Rock.
        let yaml = "WeekendInfo:\n TrackName: limerock\n TrackLength: 2.37 km\n";
        assert_eq!(parse_track_length_m(yaml), Some(2370.0));
    }

    #[test]
    fn milhas_nao_viram_metros_por_engano() {
        let yaml = " TrackLength: 2.50 mi\n";
        let m = parse_track_length_m(yaml).unwrap();
        assert!((m - 4023.36).abs() < 1.0, "{m}");
    }

    #[test]
    fn ausente_ou_absurdo_devolve_nada() {
        assert_eq!(parse_track_length_m("WeekendInfo:\n TrackName: x\n"), None);
        assert_eq!(parse_track_length_m(" TrackLength: 0.00 km\n"), None);
        assert_eq!(parse_track_length_m(" TrackLength: sei lá\n"), None);
    }
}
