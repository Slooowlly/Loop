//! Leitura do YAML de sessão do iRacing. O YAML do sim é raso o suficiente para
//! uma varredura por linha bastar — sem precisar de um parser YAML completo.
//!
//! A varredura tem um risco conhecido: ela pega a PRIMEIRA ocorrência do prefixo no
//! arquivo inteiro. Um campo homônimo em outra seção passaria a ser lido no lugar do
//! certo, sem erro nenhum. Por isso os campos ambíguos são ancorados na seção esperada
//! com [`secao_do_yaml`] antes de varrer.

/// Recorta uma SEÇÃO de topo do YAML (`WeekendInfo:`, `SessionInfo:`, `DriverInfo:`).
///
/// O YAML do iRacing tem as seções em coluna zero e todo o resto indentado, então a
/// seção vai da linha `Nome:` até a próxima linha que começa sem recuo. Seção ausente
/// devolve o documento INTEIRO de propósito: fixture de teste costuma trazer só o campo
/// solto, e recusar tudo aí transformaria uma leitura que funciona numa que não lê nada.
/// A âncora existe para desambiguar campo homônimo, não para ficar mais exigente.
pub fn secao_do_yaml<'a>(yaml: &'a str, secao: &str) -> &'a str {
    let cabecalho = format!("{secao}:");
    let Some(inicio) = yaml.find(&cabecalho).filter(|&i| {
        // Só vale em coluna zero: `WeekendInfo:` no topo, não `SubWeekendInfo:` indentado.
        i == 0 || yaml.as_bytes().get(i - 1) == Some(&b'\n')
    }) else {
        return yaml;
    };
    let corpo = &yaml[inicio + cabecalho.len()..];
    // Fim = próxima linha em coluna zero que não é continuação da seção.
    let fim = corpo
        .match_indices('\n')
        .find(|(i, _)| {
            corpo[i + 1..]
                .chars()
                .next()
                .is_some_and(|c| !c.is_whitespace() && c != '-')
        })
        .map(|(i, _)| i)
        .unwrap_or(corpo.len());
    &corpo[..fim]
}

/// Extrai `TrackDisplayName: ...` da string YAML de sessão, se presente.
/// Ancorado em `WeekendInfo`, que é onde o campo mora.
pub(crate) fn parse_track_name(yaml: &str) -> Option<String> {
    secao_do_yaml(yaml, "WeekendInfo")
        .lines()
        .find_map(|line| line.trim().strip_prefix("TrackDisplayName:"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Extrai o `custid` (id de cliente iRacing) do JOGADOR do YAML de sessão:
/// `DriverInfo.DriverCarIdx` diz qual carro é o do jogador, e o `UserID` do
/// piloto com aquele `CarIdx` é o custid. Varredura por linha (sem parser YAML).
pub fn parse_player_custid(yaml: &str) -> Option<i64> {
    // `CarIdx` e `UserID` só existem dentro de `DriverInfo`, mas `DriverCarIdx` é
    // genérico o bastante para reaparecer noutra seção numa build futura do sim.
    let yaml = secao_do_yaml(yaml, "DriverInfo");
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
    secao_do_yaml(yaml, "WeekendInfo")
        .lines()
        .find_map(|line| line.trim().strip_prefix("TrackLength:"))
        .and_then(|v| {
            let v = v.trim();
            let numero = v.split_whitespace().next()?;
            let km = numero.parse::<f64>().ok()?;
            // O sufixo é sempre `km` nas builds vistas, mas não custa não confiar.
            let metros = if v.ends_with("mi") {
                km * 1609.344
            } else {
                km * 1000.0
            };
            Some(metros)
        })
        .filter(|m| *m > 100.0)
}

/// Redline do carro do jogador (RPM) do YAML de sessão (`DriverInfo.DriverCarRedLine`).
/// Referência pro estilo de pilotagem (colado no limitador / short-shift). Varredura por
/// linha. `None` se ausente (o consumidor trata como redline desconhecido → ignora rotação).
pub fn parse_car_redline(yaml: &str) -> Option<f64> {
    secao_do_yaml(yaml, "DriverInfo")
        .lines()
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

    /// Um YAML no formato do sim: seções em coluna zero, resto indentado.
    fn yaml_de_sessao() -> String {
        "---\n\
         WeekendInfo:\n\
         \x20TrackName: limerock\n\
         \x20TrackLength: 2.37 km\n\
         \x20TrackDisplayName: Lime Rock Park\n\
         \x20WeekendOptions:\n\
         \x20 NumStarters: 20\n\
         SessionInfo:\n\
         \x20Sessions:\n\
         \x20- SessionNum: 0\n\
         \x20  SessionType: Race\n\
         DriverInfo:\n\
         \x20DriverCarIdx: 1\n\
         \x20DriverCarRedLine: 7200.000\n\
         \x20Drivers:\n\
         \x20- CarIdx: 0\n\
         \x20  UserID: 111\n\
         \x20- CarIdx: 1\n\
         \x20  UserID: 222\n"
            .to_string()
    }

    #[test]
    fn a_secao_recorta_ate_a_proxima_de_topo() {
        let y = yaml_de_sessao();
        let weekend = secao_do_yaml(&y, "WeekendInfo");
        assert!(weekend.contains("TrackLength"));
        assert!(
            weekend.contains("NumStarters"),
            "subseção indentada faz parte"
        );
        assert!(
            !weekend.contains("DriverCarIdx"),
            "a seção não pode invadir a próxima"
        );
        let driver = secao_do_yaml(&y, "DriverInfo");
        assert!(driver.contains("UserID: 222"));
        assert!(!driver.contains("TrackLength"));
    }

    #[test]
    fn secao_ausente_devolve_o_documento_inteiro() {
        // Fixture solta continua funcionando: a âncora desambigua, não endurece.
        let y = " TrackLength: 2.37 km\n";
        assert_eq!(secao_do_yaml(y, "WeekendInfo"), y);
        assert_eq!(parse_track_length_m(y), Some(2370.0));
    }

    #[test]
    fn campo_homonimo_em_outra_secao_nao_rouba_a_leitura() {
        // O defeito que a âncora fecha: a varredura pegava a PRIMEIRA ocorrência do
        // prefixo no arquivo inteiro. Aqui a seção de cima traz um homônimo.
        let y = "CameraInfo:\n\
                 \x20TrackLength: 9.99 km\n\
                 \x20TrackDisplayName: Câmera\n\
                 WeekendInfo:\n\
                 \x20TrackLength: 2.37 km\n\
                 \x20TrackDisplayName: Lime Rock Park\n";
        assert_eq!(parse_track_length_m(y), Some(2370.0));
        assert_eq!(parse_track_name(y).as_deref(), Some("Lime Rock Park"));
    }

    #[test]
    fn o_custid_do_jogador_sai_do_carro_dele() {
        let y = yaml_de_sessao();
        assert_eq!(parse_player_custid(&y), Some(222));
    }

    #[test]
    fn o_redline_vem_do_driver_info() {
        let y = yaml_de_sessao();
        assert_eq!(parse_car_redline(&y), Some(7200.0));
    }
}
