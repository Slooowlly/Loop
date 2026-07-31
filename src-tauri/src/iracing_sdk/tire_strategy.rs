//! Reconstrução da estratégia de pneu de cada carro a partir das PARADAS de box.
//! **Lógica pura, testável** (não fala com o SDK — recebe os dados já capturados).
//!
//! Contexto (decisão do user): o iRacing **não expõe** o serviço de pit dos carros
//! da IA (trocou pneu? qual composto?). Só dá pra saber, ao vivo, que um carro
//! ENTROU/PAROU/SAIU do box (`CarIdxOnPitRoad` + `CarIdxTrackSurface == InPitStall`).
//! Então a estratégia é INFERIDA. Como o iRacing só tem dois compostos — **seco** e
//! **chuva** — o clima + as paradas reconstroem a estratégia inteira:
//!
//! 1. **Trocou pneu?** Parada longa = troca (o serviço de pneu adiciona tempo fixo,
//!    bem maior que um "splash" de combustível). REFORÇO por clima: parar com a pista
//!    molhada estando de seco = troca pra wet quase certa (ninguém para na chuva só
//!    pra abastecer) — o clima destrava o veredito quando o dwell fica ambíguo.
//! 2. **Pra qual pneu?** Molhado se a pista está molhada no instante; seco caso contrário.
//! 3. **Pneu de largada** (o pulo do gato): numa corrida molhada, quem **nunca parou**
//!    rodou tudo no pneu de largada ⇒ **largou de wet** (apostou na chuva); quem parou
//!    quando a chuva chegou ⇒ largou de seco e trocou. Corrida seca o tempo todo ⇒
//!    todos largam de seco.

use serde::{Deserialize, Serialize};

/// Composto de pneu — o iRacing só tem estes dois.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Compound {
    Dry,
    Wet,
    /// Não foi possível inferir (sem dados de clima/paradas suficientes).
    Unknown,
}

impl Compound {
    fn label(self) -> &'static str {
        match self {
            Compound::Dry => "seco",
            Compound::Wet => "chuva",
            Compound::Unknown => "indefinido",
        }
    }
}

/// Uma parada de box detectada (capturada ao vivo, por carro). Entrada do motor.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PitStop {
    /// Carro que parou (`CarIdx`).
    pub car_idx: i32,
    /// Volta em que entrou no box.
    pub lap: i32,
    /// Tempo PARADO na caixa, em segundos (dwell) — a métrica de troca-vs-abastecimento.
    pub stationary_secs: f64,
    /// Pista estava molhada NO INSTANTE da parada (do `TrackWetness`).
    pub track_wet_at_stop: bool,
}

// ── Modelo de tempo de serviço no box (medido pelo user, esta categoria) ──────
// CRÍTICO: abastecer e trocar pneus NÃO acontecem ao mesmo tempo — são SEQUENCIAIS,
// então um pit com os dois soma os tempos. Dados medidos:
//   • só pneus ............ 21–22 s  (bloco fixo)
//   • só 11.4 L ........... ~19 s
//   • só 3.8 L ............ 9.2 s
// Combustível ≈ afim: ~4.3 s base + ~1.29 s/L. Pneus = ~21.5 s POR CIMA do combustível.
/// Tempo de serviço de uma troca de pneus (s) — bloco fixo, medido.
pub const TIRE_SERVICE_SECS: f64 = 21.5;
/// Base do abastecimento (s) — parte fixa do serviço de combustível.
pub const FUEL_BASE_SECS: f64 = 4.3;
/// Tempo de abastecimento por litro (s/L).
pub const FUEL_SECS_PER_L: f64 = 1.29;

/// Limiar de dwell (s) acima do qual consideramos que houve TROCA de pneus (modo
/// ABSOLUTO, usado por carro isolado e como fallback). Fica ENTRE o maior
/// abastecimento-sozinho observado (~19 s, 11.4 L) e o serviço mínimo de pneus
/// (~21 s). O dwell MEDIDO (`InPitStall`) inclui alguns segundos de entrada/saída
/// além do serviço puro, então pode subir um pouco após uma medição real.
pub const TIRE_CHANGE_MIN_SECS: f64 = 20.0;

/// Vão MÍNIMO (s) no histograma de dwell do pelotão para ser considerado o "salto"
/// do bloco de pneus (~21.5 s). 0.65×bloco — robusto a ruído de entrada/saída.
pub const TIRE_GAP_MIN_SECS: f64 = 14.0;

/// MODO RELATIVO: acha o limiar olhando o pelotão todo. Como trocar pneus soma um
/// bloco fixo (~21.5 s) por cima do combustível, as paradas formam DUAS faixas
/// (só-combustível embaixo, com-pneus ~21 s acima) com um VÃO entre elas. Corta no
/// meio do maior vão ≥ `TIRE_GAP_MIN_SECS`. Sem vão claro (poucas paradas, ou todas
/// na mesma faixa) → cai no limiar absoluto. Mais robusto que o absoluto quando o
/// combustível varia (um abastecimento grande de 19 s não é confundido com troca).
fn relative_threshold(dwells: &[f64]) -> f64 {
    if dwells.len() < 2 {
        return TIRE_CHANGE_MIN_SECS;
    }
    let mut v: Vec<f64> = dwells.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut best_gap = 0.0;
    let mut split: Option<f64> = None;
    for w in v.windows(2) {
        let gap = w[1] - w[0];
        if gap >= TIRE_GAP_MIN_SECS && gap > best_gap {
            best_gap = gap;
            split = Some((w[0] + w[1]) / 2.0);
        }
    }
    split.unwrap_or(TIRE_CHANGE_MIN_SECS)
}

/// Um stint: a partir de qual volta, com qual composto, e se esse composto VEIO de uma
/// troca no box (`changed=false` só para o stint inicial, da largada).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TireStint {
    /// Volta de início do stint (1 = largada).
    pub from_lap: i32,
    pub compound: Compound,
    /// Se este stint começou com uma TROCA no box (false = pneu de largada).
    pub changed: bool,
    /// Confiança da inferência deste stint (0–1) — clima destrava → alta; só dwell → média.
    pub confidence: f64,
}

/// Detalhe de UMA parada de box — base do tracking de TEMPO DE PIT (só a caixa).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PitStopDetail {
    pub lap: i32,
    /// Tempo PARADO na caixa, em segundos (dwell no `InPitStall`) = o "tempo de pit".
    pub box_secs: f64,
    /// Se essa parada incluiu troca de pneu (classificação dwell + clima).
    pub tire_change: bool,
    /// Pista molhada no instante da parada.
    pub track_wet: bool,
}

/// A estratégia de pneu reconstruída de um carro na corrida.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarTireStrategy {
    pub car_idx: i32,
    /// Nome do piloto — preenchido fora (no `telemetry_analysis`, via `name_by_idx`).
    /// Vazio aqui no módulo puro. Default "Carro {idx}" cabe na UI.
    #[serde(default)]
    pub pilot_name: String,
    /// Nome da equipe — preenchido fora (via `team_by_idx`, contrato ativo). Vazio aqui.
    #[serde(default)]
    pub team_name: String,
    /// Composto com que largou (reconstruído).
    pub start_compound: Compound,
    /// Stints na ordem da corrida (o primeiro é a largada).
    pub stints: Vec<TireStint>,
    /// Nº de paradas em que houve troca de pneu (não conta abastecimento puro).
    pub tire_changes: i32,
    /// Carro ficou no pneu ERRADO (estava de seco com a pista molhada e nunca trocou),
    /// ou vice-versa — sinalizador de aposta furada.
    pub wrong_tire: bool,
    /// Todas as paradas de box do carro (tempo na caixa por parada) — o tracking de
    /// tempo de pit. Inclui as só-de-combustível (que não criam stint novo).
    #[serde(default)]
    pub stops: Vec<PitStopDetail>,
    /// Resumo curto em PT para a UI.
    pub summary: String,
}

/// Contexto de clima da corrida (capturado ao vivo): se estava molhado na largada,
/// se em algum momento molhou, e se estava molhado na bandeirada.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct RaceWeatherContext {
    /// Pista molhada na LARGADA (lights out).
    pub wet_at_start: bool,
    /// Pista ficou molhada em ALGUM momento da corrida.
    pub ever_wet: bool,
    /// Pista molhada na BANDEIRADA (fim).
    pub wet_at_finish: bool,
}

impl RaceWeatherContext {
    /// Corrida 100% seca, sem chuva em momento algum.
    pub const DRY: Self = Self {
        wet_at_start: false,
        ever_wet: false,
        wet_at_finish: false,
    };
}

/// Decide se uma parada foi troca de pneu. Combina dwell + reforço por clima:
/// se o carro estava de SECO e a pista está MOLHADA, parar ali é troca quase certa
/// (mesmo com dwell curto). Devolve `(trocou, confiança)`.
fn stop_is_tire_change(stop: &PitStop, current: Compound, threshold: f64) -> (bool, f64) {
    // Descasamento pneu↔pista: de seco com pista molhada, OU de wet com pista seca.
    // Parar com o pneu errado pra condição = troca quase certa (pra qualquer dos lados).
    let weather_forces = (stop.track_wet_at_stop && current == Compound::Dry)
        || (!stop.track_wet_at_stop && current == Compound::Wet);
    let long_dwell = stop.stationary_secs >= threshold;
    match (weather_forces, long_dwell) {
        // Clima + dwell concordam → troca, confiança alta.
        (true, true) => (true, 0.95),
        // Clima manda (pista molhou e estava de seco) mesmo com dwell curto → troca.
        (true, false) => (true, 0.85),
        // Só o dwell (sem mudança de condição) → troca, confiança média.
        (false, true) => (true, 0.65),
        // Nem clima nem dwell → só abastecimento.
        (false, false) => (false, 0.7),
    }
}

/// Reconstrói a estratégia de pneu de UM carro a partir das suas paradas e do clima,
/// usando o limiar ABSOLUTO. `stops` deve conter só as paradas DESTE carro, em ordem
/// de volta. (O `infer_all` usa a variante com limiar relativo do pelotão.)
pub fn infer_car_strategy(
    car_idx: i32,
    stops: &[PitStop],
    ctx: RaceWeatherContext,
) -> CarTireStrategy {
    infer_car_strategy_thr(car_idx, stops, ctx, TIRE_CHANGE_MIN_SECS)
}

/// Núcleo da reconstrução, com o limiar de troca explícito (absoluto ou relativo).
fn infer_car_strategy_thr(
    car_idx: i32,
    stops: &[PitStop],
    ctx: RaceWeatherContext,
    threshold: f64,
) -> CarTireStrategy {
    // ── Pneu de largada ──────────────────────────────────────────────────────
    // Seca o tempo todo → seco. Molhada na largada → wet. Caso a corrida tenha
    // molhado SÓ DEPOIS: largou de seco SE parou pra trocar quando a chuva chegou;
    // se NUNCA parou numa corrida que molhou, rodou tudo no pneu de largada — e a
    // única forma de aguentar a chuva sem parar é ter largado de WET (apostou).
    let changed_to_wet_later = stops
        .iter()
        .any(|s| s.track_wet_at_stop && s.stationary_secs >= 0.0);
    let start_compound = if !ctx.ever_wet {
        Compound::Dry
    } else if ctx.wet_at_start {
        Compound::Wet
    } else if changed_to_wet_later {
        // Molhou no meio e o carro parou (pra montar wet) → largou de seco.
        Compound::Dry
    } else if stops.is_empty() {
        // Molhou em algum momento e o carro NUNCA parou → largou de wet (aposta).
        Compound::Wet
    } else {
        // Parou, mas nunca com a pista molhada (só abasteceu/trocou seco→seco) numa
        // corrida que molhou: provavelmente largou de seco e a janela molhada não o pegou.
        Compound::Dry
    };

    // ── Stints, percorrendo as paradas ───────────────────────────────────────
    let start_conf = if !ctx.ever_wet {
        0.9
    } else if ctx.wet_at_start {
        0.85
    } else if stops.is_empty() {
        0.8 // nunca-parou-na-chuva = largou-de-wet (inferência forte do user)
    } else {
        0.7
    };
    let mut stints = vec![TireStint {
        from_lap: 1,
        compound: start_compound,
        changed: false,
        confidence: start_conf,
    }];
    let mut current = start_compound;
    let mut tire_changes = 0;
    let mut stops_detail = Vec::with_capacity(stops.len());

    for stop in stops {
        let (changed, conf) = stop_is_tire_change(stop, current, threshold);
        // Tracking de tempo de pit: registra TODA parada (com ou sem troca).
        stops_detail.push(PitStopDetail {
            lap: stop.lap.max(1),
            box_secs: stop.stationary_secs,
            tire_change: changed,
            track_wet: stop.track_wet_at_stop,
        });
        if changed {
            let new_compound = if stop.track_wet_at_stop {
                Compound::Wet
            } else {
                Compound::Dry
            };
            tire_changes += 1;
            current = new_compound;
            stints.push(TireStint {
                from_lap: stop.lap.max(1),
                compound: new_compound,
                changed: true,
                confidence: conf,
            });
        }
    }

    // ── Pneu errado? (estava de seco com a pista molhada no fim, e não trocou) ──
    let wrong_tire = ctx.wet_at_finish && current == Compound::Dry
        || (!ctx.ever_wet && current == Compound::Wet);

    let summary = build_summary(&stints, tire_changes, wrong_tire);

    CarTireStrategy {
        car_idx,
        pilot_name: String::new(),
        team_name: String::new(),
        start_compound,
        stints,
        tire_changes,
        wrong_tire,
        stops: stops_detail,
        summary,
    }
}

/// Reconstrói a estratégia de TODOS os carros. Agrupa as paradas por carro (mantendo
/// a ordem de volta esperada) e infere cada um. Usa o limiar RELATIVO calculado do
/// pelotão inteiro (modo gap) — mais robusto que o absoluto.
pub fn infer_all(stops: &[PitStop], ctx: RaceWeatherContext) -> Vec<CarTireStrategy> {
    // Limiar do pelotão: olha TODAS as paradas pra achar o vão do bloco de pneus.
    let dwells: Vec<f64> = stops.iter().map(|s| s.stationary_secs).collect();
    let threshold = relative_threshold(&dwells);

    let mut idxs: Vec<i32> = stops.iter().map(|s| s.car_idx).collect();
    idxs.sort_unstable();
    idxs.dedup();
    idxs.into_iter()
        .map(|idx| {
            let mut mine: Vec<PitStop> =
                stops.iter().copied().filter(|s| s.car_idx == idx).collect();
            mine.sort_by_key(|s| s.lap);
            infer_car_strategy_thr(idx, &mine, ctx, threshold)
        })
        .collect()
}

/// Frase curta em PT descrevendo a estratégia (para a UI pós-corrida).
fn build_summary(stints: &[TireStint], tire_changes: i32, wrong_tire: bool) -> String {
    let start = stints
        .first()
        .map(|s| s.compound)
        .unwrap_or(Compound::Unknown);
    let base = match tire_changes {
        0 => format!("Largou de {} e não trocou pneus", start.label()),
        1 => {
            let to = stints
                .last()
                .map(|s| s.compound.label())
                .unwrap_or("indefinido");
            format!("Largou de {} e trocou pra {} (1 parada)", start.label(), to)
        }
        n => format!("Largou de {} · {} trocas de pneu", start.label(), n),
    };
    if wrong_tire {
        format!("{base} — ficou no pneu errado")
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(car: i32, lap: i32, secs: f64, wet: bool) -> PitStop {
        PitStop {
            car_idx: car,
            lap,
            stationary_secs: secs,
            track_wet_at_stop: wet,
        }
    }

    #[test]
    fn corrida_seca_largou_de_seco_sem_troca() {
        let s = infer_car_strategy(5, &[], RaceWeatherContext::DRY);
        assert_eq!(s.start_compound, Compound::Dry);
        assert_eq!(s.tire_changes, 0);
        assert!(!s.wrong_tire);
        assert_eq!(s.stints.len(), 1);
    }

    #[test]
    fn parada_curta_e_so_abastecimento() {
        // Seca, parada de 6s no meio → splash de combustível, NÃO troca.
        let stops = [stop(5, 10, 6.0, false)];
        let s = infer_car_strategy(5, &stops, RaceWeatherContext::DRY);
        assert_eq!(s.tire_changes, 0);
        assert_eq!(s.stints.len(), 1, "abastecimento não cria stint novo");
    }

    #[test]
    fn parada_longa_e_troca_de_pneu_seco() {
        // Seca, parada de 22s (acima do bloco de pneus de ~21s) → trocou pneus (seco→seco).
        let stops = [stop(5, 12, 22.0, false)];
        let s = infer_car_strategy(5, &stops, RaceWeatherContext::DRY);
        assert_eq!(s.tire_changes, 1);
        assert_eq!(s.stints.len(), 2);
        assert_eq!(s.stints[1].compound, Compound::Dry);
    }

    #[test]
    fn abastecimento_grande_nao_e_troca_mas_pneus_sim() {
        // Números medidos pelo user: abastecer 11.4 L = ~19 s (NÃO é troca);
        // trocar pneus = 21–22 s (É troca). O limiar de 20 s separa os dois.
        let so_combustivel =
            infer_car_strategy(1, &[stop(1, 10, 19.0, false)], RaceWeatherContext::DRY);
        assert_eq!(
            so_combustivel.tire_changes, 0,
            "abastecer 11.4L não é troca"
        );
        let so_pneus = infer_car_strategy(1, &[stop(1, 10, 21.0, false)], RaceWeatherContext::DRY);
        assert_eq!(so_pneus.tire_changes, 1, "21s = troca de pneus");
    }

    #[test]
    fn chuva_desde_a_largada_largou_de_wet() {
        let ctx = RaceWeatherContext {
            wet_at_start: true,
            ever_wet: true,
            wet_at_finish: true,
        };
        let s = infer_car_strategy(3, &[], ctx);
        assert_eq!(s.start_compound, Compound::Wet);
        assert!(!s.wrong_tire);
    }

    #[test]
    fn molhou_no_meio_e_carro_parou_largou_de_seco_trocou_wet() {
        // Cenário clássico: largou de seco, choveu, parou pra montar wet.
        let ctx = RaceWeatherContext {
            wet_at_start: false,
            ever_wet: true,
            wet_at_finish: true,
        };
        // Parada curta (5s), mas com pista MOLHADA → clima destrava a troca.
        let stops = [stop(7, 8, 5.0, true)];
        let s = infer_car_strategy(7, &stops, ctx);
        assert_eq!(s.start_compound, Compound::Dry);
        assert_eq!(s.tire_changes, 1);
        assert_eq!(s.stints[1].compound, Compound::Wet);
        assert!(!s.wrong_tire, "trocou pra wet a tempo");
        // Clima destrava → confiança alta mesmo com dwell curto.
        assert!(s.stints[1].confidence >= 0.85);
    }

    #[test]
    fn corrida_molhada_nunca_parou_largou_de_wet() {
        // O EXEMPLO do user: corrida de chuva, o carro NÃO entrou no pit → largou de wet.
        let ctx = RaceWeatherContext {
            wet_at_start: true,
            ever_wet: true,
            wet_at_finish: true,
        };
        let s = infer_car_strategy(9, &[], ctx);
        assert_eq!(s.start_compound, Compound::Wet);
        assert_eq!(s.tire_changes, 0);
    }

    #[test]
    fn preso_no_seco_na_chuva_sinaliza_pneu_errado() {
        // Largou seco, parou cedo (seco→seco, antes da chuva), molhou e terminou molhado
        // sem trocar → ficou no pneu errado.
        let ctx = RaceWeatherContext {
            wet_at_start: false,
            ever_wet: true,
            wet_at_finish: true,
        };
        let stops = [stop(4, 5, 24.0, false)]; // troca seco→seco antes da chuva
        let s = infer_car_strategy(4, &stops, ctx);
        assert_eq!(s.start_compound, Compound::Dry);
        // Terminou de seco com pista molhada → pneu errado.
        assert!(s.wrong_tire, "deveria sinalizar pneu errado");
    }

    #[test]
    fn dry_para_wet_e_de_volta_pra_dry_duas_trocas() {
        // Pista molhou e secou: seco → wet → seco.
        let ctx = RaceWeatherContext {
            wet_at_start: false,
            ever_wet: true,
            wet_at_finish: false,
        };
        let stops = [stop(2, 6, 6.0, true), stop(2, 14, 6.0, false)];
        let s = infer_car_strategy(2, &stops, ctx);
        assert_eq!(s.start_compound, Compound::Dry);
        assert_eq!(s.tire_changes, 2);
        assert_eq!(s.stints[1].compound, Compound::Wet);
        assert_eq!(s.stints[2].compound, Compound::Dry);
        assert!(!s.wrong_tire, "terminou de seco com pista seca");
    }

    #[test]
    fn infer_all_agrupa_e_ordena_por_carro() {
        let ctx = RaceWeatherContext::DRY;
        let stops = [
            stop(7, 10, 22.0, false),
            stop(3, 8, 5.0, false),
            stop(7, 4, 24.0, false),
        ];
        let all = infer_all(&stops, ctx);
        assert_eq!(all.len(), 2, "dois carros distintos");
        let car7 = all.iter().find(|s| s.car_idx == 7).unwrap();
        // Carro 7: duas paradas longas → duas trocas, na ordem de volta (4 antes de 10).
        assert_eq!(car7.tire_changes, 2);
        assert_eq!(car7.stints[1].from_lap, 4);
        assert_eq!(car7.stints[2].from_lap, 10);
        let car3 = all.iter().find(|s| s.car_idx == 3).unwrap();
        assert_eq!(car3.tire_changes, 0, "parada curta = só abasteceu");
    }

    #[test]
    fn modo_relativo_separa_combustivel_grande_de_pneus() {
        // Pelotão seco: uns só abastecem (16–19 s), outros trocam pneus (37–40 s = 19 + 21).
        // O VÃO entre 19 e 37 é o bloco de pneus → corta no meio (~28 s), e os de 19 s
        // (que o limiar absoluto de 20 quase pegaria) ficam como SÓ abastecimento.
        let ctx = RaceWeatherContext::DRY;
        let stops = [
            stop(1, 10, 16.0, false), // só combustível
            stop(2, 10, 19.0, false), // só combustível (grande)
            stop(3, 10, 37.0, false), // combustível + pneus
            stop(4, 10, 40.0, false), // combustível + pneus
        ];
        let all = infer_all(&stops, ctx);
        let by = |idx: i32| all.iter().find(|s| s.car_idx == idx).unwrap().tire_changes;
        assert_eq!(by(1), 0, "16s = só abasteceu");
        assert_eq!(by(2), 0, "19s = só abasteceu (não confunde com troca)");
        assert_eq!(by(3), 1, "37s = trocou pneus");
        assert_eq!(by(4), 1, "40s = trocou pneus");
    }

    #[test]
    fn modo_relativo_sem_vao_cai_no_absoluto() {
        // Todas as paradas na mesma faixa (sem vão ≥14s) → usa o limiar absoluto (20).
        let ctx = RaceWeatherContext::DRY;
        let stops = [
            stop(1, 10, 9.0, false),  // splash pequeno
            stop(2, 10, 19.0, false), // abastecimento grande
        ];
        let all = infer_all(&stops, ctx);
        // 9 e 19 ambos < 20 e o vão (10) < 14 → nenhum vira troca.
        assert!(
            all.iter().all(|s| s.tire_changes == 0),
            "sem vão, nada de troca"
        );
    }

    #[test]
    fn tracking_tempo_de_pit_registra_toda_parada() {
        // Duas paradas: uma só-combustível (12s) e uma com troca (35s). Ambas entram no
        // tracking de tempo de pit, com o box_secs e o flag de troca.
        let ctx = RaceWeatherContext::DRY;
        let stops = [stop(8, 6, 12.0, false), stop(8, 14, 35.0, false)];
        let s = infer_car_strategy(8, &stops, ctx);
        assert_eq!(s.stops.len(), 2, "toda parada entra no tracking");
        assert_eq!(s.stops[0].box_secs, 12.0);
        assert!(!s.stops[0].tire_change, "12s = só abasteceu");
        assert_eq!(s.stops[1].box_secs, 35.0);
        assert!(s.stops[1].tire_change, "35s = trocou pneu");
        // O tracking inclui a só-combustível, que NÃO cria stint novo.
        assert_eq!(s.tire_changes, 1);
    }

    #[test]
    fn summary_descreve_estrategia() {
        let ctx = RaceWeatherContext {
            wet_at_start: false,
            ever_wet: true,
            wet_at_finish: true,
        };
        let stops = [stop(1, 8, 5.0, true)];
        let s = infer_car_strategy(1, &stops, ctx);
        assert!(s.summary.contains("seco"));
        assert!(s.summary.contains("chuva"));
    }
}
