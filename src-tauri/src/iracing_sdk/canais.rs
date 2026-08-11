//! Os canais de telemetria que o Loop CURA do SDK, e a conferência contra o que o
//! iRacing de fato publica nesta build.
//!
//! O motivo de existir: `imp::leitura::extract_telemetry` casa cada canal por NOME num
//! `match`, e nome que não existe cai no `_ => {}` calado. O campo fica zerado para
//! sempre, e de fora um canal ausente e um canal que vale zero são exatamente a mesma
//! coisa. Já aconteceu em produção com `PitRepairNeeded` (o nome real é `PitRepairLeft`),
//! e o sintoma foi o dano do carro simplesmente não aparecer.
//!
//! O inventário (`read_var_inventory`) sempre existiu e ia para a captura de corrida.
//! Aqui ele passa a ser CRUZADO com esta lista na borda de conexão: canal curado que o
//! sim não publica vira uma linha no `loop.log`, em vez de silêncio.

use crate::iracing_sdk::VarDoSdk;

/// Nomes que [`crate::iracing_sdk::imp`] lê do buffer de telemetria.
///
/// A lista é conferida contra o `match` real pelo teste
/// `a_lista_curada_bate_com_o_match_da_leitura` — acrescentar um canal na leitura sem
/// acrescentar aqui quebra a suíte, que é o ponto: uma lista que envelhece calada não
/// serviria de guarda para nada.
pub const CANAIS_CURADOS: [&str; 86] = [
    "AirTemp",
    "Brake",
    "CamCameraNumber",
    "CamCameraState",
    "CamCarIdx",
    "CamGroupNumber",
    "CarIdxBestLapNum",
    "CarIdxBestLapTime",
    "CarIdxClassPosition",
    "CarIdxEstTime",
    "CarIdxF2Time",
    "CarIdxFastRepairsUsed",
    "CarIdxGear",
    "CarIdxLap",
    "CarIdxLapCompleted",
    "CarIdxLapDistPct",
    "CarIdxLastLapTime",
    "CarIdxOnPitRoad",
    "CarIdxPaceFlags",
    "CarIdxPaceLine",
    "CarIdxPaceRow",
    "CarIdxPosition",
    "CarIdxRPM",
    "CarIdxSessionFlags",
    "CarIdxSteer",
    "CarIdxTireCompound",
    "CarIdxTrackSurface",
    "CarIdxTrackSurfaceMaterial",
    "CarLeftRight",
    "Clutch",
    "FogLevel",
    "FuelLevel",
    "Gear",
    "IsInGarage",
    "IsOnTrack",
    "IsOnTrackCar",
    "IsReplayPlaying",
    "Lap",
    "LapCompleted",
    "LapCurrentLapTime",
    "LapDistPct",
    "LapLastLapTime",
    "LatAccel",
    "LongAccel",
    "OnPitRoad",
    "PaceMode",
    "PitOptRepairLeft",
    "PitRepairLeft",
    "PitchRate",
    "PitsOpen",
    "PlayerCarDriverIncidentCount",
    "PlayerCarIdx",
    "PlayerCarMyIncidentCount",
    "PlayerCarPosition",
    "PlayerCarTeamIncidentCount",
    "PlayerCarTowTime",
    "PlayerTrackSurface",
    "Precipitation",
    "RPM",
    "RelativeHumidity",
    "ReplayFrameNum",
    "ReplaySessionNum",
    "ReplaySessionTime",
    "RollRate",
    "SessionFlags",
    "SessionLapsRemain",
    "SessionLapsRemainEx",
    "SessionLapsTotal",
    "SessionNum",
    "SessionState",
    "SessionTick",
    "SessionTime",
    "SessionTimeRemain",
    "SessionTimeTotal",
    "Skies",
    "Speed",
    "SteeringWheelAngle",
    "Throttle",
    "TrackTemp",
    "TrackTempCrew",
    "TrackWetness",
    "VertAccel",
    "WeatherDeclaredWet",
    "WindDir",
    "WindVel",
    "YawRate",
];

/// Canais curados que o inventário do sim NÃO traz nesta build/sessão.
///
/// Puro, para ser testável sem o sim aberto.
pub fn canais_ausentes<'a>(inventario: &[VarDoSdk]) -> Vec<&'a str> {
    let publicados: std::collections::HashSet<&str> =
        inventario.iter().map(|v| v.nome.as_str()).collect();
    CANAIS_CURADOS
        .iter()
        .copied()
        .filter(|nome| !publicados.contains(nome))
        .collect()
}

/// Cruza a lista curada com o que o sim publica e registra o que faltou.
///
/// Chamado uma vez por conexão, junto da leitura do inventário. Não devolve erro nem
/// impede nada: canal ausente degrada uma funcionalidade, não a corrida. O que muda é
/// que o silêncio vira diagnóstico — `loop.log` passa a dizer exatamente qual canal não
/// existe nesta build, em vez de deixar um campo zerado sem explicação.
pub fn conferir_inventario(inventario: &[VarDoSdk]) {
    if inventario.is_empty() {
        return; // sem inventário não há o que concluir (build stub, leitura falhou)
    }
    let ausentes = canais_ausentes(inventario);
    if ausentes.is_empty() {
        crate::diagnostico::linha_unica(
            "sdk",
            &format!(
                "inventário conferido: os {} canais curados existem nesta build",
                CANAIS_CURADOS.len()
            ),
        );
        return;
    }
    crate::diagnostico::linha_unica(
        "sdk",
        &format!(
            "canais curados AUSENTES no SDK ({} de {}): {} — o campo correspondente fica \
             zerado em silêncio; confira o nome contra o inventário da captura",
            ausentes.len(),
            CANAIS_CURADOS.len(),
            ausentes.join(", ")
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(nome: &str) -> VarDoSdk {
        VarDoSdk {
            nome: nome.to_string(),
            tipo: 4,
            quantidade: 1,
            unidade: String::new(),
            descricao: String::new(),
        }
    }

    #[test]
    fn a_lista_curada_bate_com_o_match_da_leitura() {
        // O guard de verdade: a lista é só útil enquanto espelha o `match`. Um canal novo
        // lido em `extract_telemetry` sem entrada aqui deixaria o buraco que este módulo
        // existe para fechar, e um nome que sobra aqui daria alarme falso para sempre.
        let fonte = include_str!("imp/leitura.rs");
        let mut faltando_na_lista: Vec<String> = Vec::new();
        for linha in fonte.lines() {
            let t = linha.trim();
            // As entradas do match têm a forma `"NomeDoCanal" => {`.
            let Some(resto) = t.strip_prefix('"') else {
                continue;
            };
            let Some((nome, cauda)) = resto.split_once('"') else {
                continue;
            };
            if !cauda.trim_start().starts_with("=>") {
                continue;
            }
            if !CANAIS_CURADOS.contains(&nome) && !faltando_na_lista.iter().any(|n| n == nome) {
                faltando_na_lista.push(nome.to_string());
            }
        }
        assert!(
            faltando_na_lista.is_empty(),
            "canais lidos em imp/leitura.rs e ausentes de CANAIS_CURADOS: {faltando_na_lista:?}"
        );

        let sobrando: Vec<&str> = CANAIS_CURADOS
            .iter()
            .copied()
            .filter(|nome| !fonte.contains(&format!("\"{nome}\" =>")))
            .collect();
        assert!(
            sobrando.is_empty(),
            "canais em CANAIS_CURADOS que a leitura não usa mais: {sobrando:?}"
        );
    }

    #[test]
    fn a_lista_nao_tem_nome_repetido() {
        let distintos: std::collections::HashSet<&str> = CANAIS_CURADOS.iter().copied().collect();
        assert_eq!(distintos.len(), CANAIS_CURADOS.len());
    }

    #[test]
    fn inventario_completo_nao_acusa_nada() {
        let inventario: Vec<VarDoSdk> = CANAIS_CURADOS.iter().map(|n| var(n)).collect();
        assert!(canais_ausentes(&inventario).is_empty());
    }

    #[test]
    fn o_canal_que_o_sim_nao_publica_e_apontado_pelo_nome() {
        // O caso real: o nome estava errado no `match` e o campo lia zero para sempre.
        let inventario: Vec<VarDoSdk> = CANAIS_CURADOS
            .iter()
            .filter(|n| **n != "PitRepairLeft")
            .map(|n| var(n))
            .collect();
        assert_eq!(canais_ausentes(&inventario), vec!["PitRepairLeft"]);
    }

    #[test]
    fn inventario_vazio_nao_acusa_a_build_inteira() {
        // Sem inventário (leitura falhou, build stub) não há conclusão a tirar, e uma
        // lista de 86 "ausentes" no log seria só ruído.
        conferir_inventario(&[]);
    }
}
