//! Dossiê de habilidade do JOGADOR — atributos inferidos do desempenho REAL na
//! pista. **Lógica pura** (sem DB, sem IO): recebe as amostras de corrida já
//! montadas e a `midia` atual, devolve o dossiê pronto pro front.
//!
//! Filosofia (ver `docs/superpowers/specs/2026-07-12-player-skill-tracking-design.md`):
//! é a INVERSÃO do fluxo da IA — lá o atributo alimenta a simulação; aqui o
//! resultado real INFERE o atributo, só pra exibir (o mercado NÃO consulta).
//!
//! Sacada do `skill`/`ritmo`: nunca usar o número absoluto da IA (a força efetiva
//! de um mesmo skill varia por pista). Usa-se a POSIÇÃO do jogador dentro do grid
//! daquela corrida — todas as IAs rodaram a mesma pista, então a ordenação delas
//! já embute a esquisitice da pista, e o efeito se cancela na interpolação.
//!
//! Fase 1 = "Grupo A": tudo computável do resultado oficial + o roster que nós
//! geramos, sem telemetria nova. `skill`, `ritmo_classificacao`, `aggression`,
//! `racecraft`, `fator_chuva`, `adaptabilidade`, `experiencia`, `midia`.
//!
//! ## As curvas de conversão são ILUSTRATIVAS, e isto é uma decisão registrada
//!
//! Três atributos daqui não saem de uma inferência contra o grid — saem de uma reta escolhida
//! à mão, escrita no comentário da própria função:
//!
//! | atributo | curva | o que ela SUPÕE |
//! |---|---|---|
//! | `aggression` | `15 + 16 × incidentes/corrida` | que a média percorra 0–5 incidentes |
//! | `adaptabilidade` | `50 + 5 × posições ganhas entre 1ª e última temporada` | que ±10 posições seja o extremo |
//! | `experiencia` | `100 · r / (r + 60)` | que 60 corridas seja "metade da carreira" |
//!
//! **A primeira já está medida, e está errada.** [`medicao::distribuicao_de_incidentes_por_corrida`]
//! mediu 4.800 piloto-corridas em 11/08/2026: a média real é 0,43–0,48 incidentes por corrida e
//! o percentil 90 é 1,0 — o que joga a população inteira entre as notas 22 e 31 de uma escala de
//! 84 pontos. O jogador mediano e o jogador do percentil 90 recebem leituras a 8 pontos um do
//! outro. Detalhe e conserto na própria função.
//!
//! As outras duas seguem sem medição.
//!
//! O que segura a gravidade disto: **o dossiê é só exibição.** O mercado, a evolução e a
//! simulação não leem nada daqui; o `SimDriver` do jogador usa os atributos de verdade. O risco
//! é o dossiê MENTIR PARA O JOGADOR (dizer "passivo" a quem bate toda corrida), não desbalancear
//! o jogo. É por isso que o conserto não foi aplicado nesta passagem: ele muda uma curva de
//! exibição que o jogador já viu, e a escolha entre "normalizar contra o grid" e "reconciliar as
//! duas fontes de contagem de incidente" é decisão de produto, não dedução do código.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::models::driver_tags::get_attribute_tag;

/// Uma IA no grid de uma corrida que o jogador disputou. `skill`/`quali_skill`/
/// `rain_skill` são os atributos da IA (skill, ritmo_classificacao, fator_chuva).
/// Usamos o valor ATUAL como proxy do valor à época — deriva lenta, aceitável pra
/// uma estimativa visual agregada (ver caveat no spec §6).
#[derive(Debug, Clone)]
pub struct GridDriver {
    pub skill: f64,
    pub quali_skill: f64,
    pub rain_skill: f64,
    pub finish: i32,
    pub start: i32,
    pub dnf: bool,
}

/// Sinais de telemetria de uma corrida do jogador (Fase 2) — só existem quando ele
/// DIRIGIU no iRacing e foi monitorado (tabela `player_race_telemetry`). Sentinela
/// `-1.0`/`start_valid=false` = não computável naquela corrida.
#[derive(Debug, Clone)]
pub struct RaceTelemetry {
    /// 0–100: quão parelhos foram os tempos de volta. < 0 = indisponível.
    pub consistency: f64,
    /// 0–1: fração das amostras em briga (carro a < 1s). < 0 = indisponível.
    pub battle_fraction: f64,
    /// Posições subidas/perdidas na pista (bruto, do fluxo de posição).
    pub on_track_gained: i32,
    pub on_track_lost: i32,
    /// Posições ganhas nos ~20s iniciais (positivo = subiu).
    pub start_delta: i32,
    pub start_valid: bool,
}

/// Uma corrida disputada pelo jogador, com o grid de IAs reconstruído.
#[derive(Debug, Clone)]
pub struct RaceSample {
    pub season: i32,
    pub round: i32,
    pub is_wet: bool,
    pub player_finish: i32,
    pub player_start: i32,
    pub player_dnf: bool,
    pub player_incidents: i32,
    /// IAs que estavam no grid (sem o jogador).
    pub grid: Vec<GridDriver>,
    /// Telemetria da corrida (Fase 2). `None` em corridas simuladas no app.
    pub telemetry: Option<RaceTelemetry>,
}

/// Regra de desbloqueio de um atributo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlock {
    /// Sempre disponível (basta ≥1 amostra pra ter valor).
    Immediate,
    /// N corridas com dado válido.
    Races(u32),
    /// N corridas em pista molhada.
    WetRaces(u32),
    /// N temporadas distintas com dado válido.
    Seasons(u32),
    /// N corridas com telemetria (dirigidas no iRacing).
    TelemetryRaces(u32),
}

impl Unlock {
    fn threshold(self) -> u32 {
        match self {
            Unlock::Immediate => 1,
            Unlock::Races(n)
            | Unlock::WetRaces(n)
            | Unlock::Seasons(n)
            | Unlock::TelemetryRaces(n) => n,
        }
    }
    fn kind(self) -> &'static str {
        match self {
            Unlock::Immediate => "immediate",
            Unlock::Races(_) => "races",
            Unlock::WetRaces(_) => "wet_races",
            Unlock::Seasons(_) => "seasons",
            Unlock::TelemetryRaces(_) => "telemetry_races",
        }
    }
}

/// Um atributo do dossiê, pronto pro front.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DossierAttribute {
    pub key: String,
    pub unlocked: bool,
    /// 0–100 quando desbloqueado; `None` enquanto bloqueado.
    pub value: Option<u8>,
    /// 0.0–1.0: firma conforme acumula amostra.
    pub confidence: f64,
    /// Amostras que contribuíram (corridas, corridas de chuva ou temporadas).
    pub sample_count: u32,
    pub unlock_threshold: u32,
    /// "races" | "wet_races" | "seasons" | "immediate".
    pub unlock_kind: String,
    /// Quantas amostras ainda faltam pra desbloquear (0 se já aberto).
    pub remaining: u32,
    /// Texto da tag quando o valor é extremo (reusa o mapa da IA); senão `None`.
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerDossier {
    pub total_races: u32,
    pub total_seasons: u32,
    pub attributes: Vec<DossierAttribute>,
}

// ── Constantes de calibração (tunáveis) ──────────────────────────────────────

/// Margem aplicada quando o jogador vence (ou perde para) o grid inteiro — ele
/// fica um degrau acima do melhor que venceu / abaixo do pior que o venceu.
const BOUNDARY_MARGIN: f64 = 3.0;

/// Desbloqueios por atributo.
const UNLOCK_SKILL: Unlock = Unlock::Races(5);
const UNLOCK_QUALI: Unlock = Unlock::Races(5);
const UNLOCK_RACECRAFT: Unlock = Unlock::Races(5);
const UNLOCK_AGGRESSION: Unlock = Unlock::Races(5);
const UNLOCK_RAIN: Unlock = Unlock::WetRaces(3);
const UNLOCK_ADAPT: Unlock = Unlock::Seasons(2);
const UNLOCK_CONSISTENCY: Unlock = Unlock::TelemetryRaces(4);
const UNLOCK_START: Unlock = Unlock::TelemetryRaces(5);

/// Confiança plena com este múltiplo do threshold de amostras.
const CONFIDENCE_FULL_MULT: f64 = 3.0;

// ── Interpolação no grid ─────────────────────────────────────────────────────

/// Estima o valor do jogador a partir da POSIÇÃO dele no grid: `ahead_count` é
/// quantas IAs terminaram à frente (rank−1), e `attrs` são os atributos das IAs
/// consideradas. Interpola a fronteira que o jogador cruzou.
fn interpolate_in_grid(ahead_count: usize, mut attrs: Vec<f64>) -> Option<f64> {
    if attrs.is_empty() {
        return None;
    }
    attrs.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)); // desc
    let hi = if ahead_count == 0 {
        None
    } else {
        attrs.get(ahead_count - 1).copied()
    };
    let lo = attrs.get(ahead_count).copied();
    let v = match (hi, lo) {
        (Some(h), Some(l)) => (h + l) / 2.0, // fronteira entre os dois
        (None, Some(l)) => l + BOUNDARY_MARGIN, // venceu todos
        (Some(h), None) => h - BOUNDARY_MARGIN, // perdeu de todos
        (None, None) => return None,
    };
    Some(v.clamp(1.0, 99.0))
}

/// Média simples de uma lista de estimativas por corrida (vazio → `None`).
fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

// ── Estimadores por atributo (cada um devolve (valor, nº de amostras)) ────────

/// `skill`: interpolação no grid da CORRIDA (só finalizadores; corridas em que o
/// jogador abandonou não contam — a chegada não reflete o ritmo).
fn estimate_skill(samples: &[RaceSample]) -> (Option<f64>, u32) {
    let mut per_race = Vec::new();
    for s in samples {
        if s.player_dnf {
            continue;
        }
        let attrs: Vec<f64> = s.grid.iter().filter(|g| !g.dnf).map(|g| g.skill).collect();
        let ahead = s
            .grid
            .iter()
            .filter(|g| !g.dnf && g.finish < s.player_finish)
            .count();
        if let Some(v) = interpolate_in_grid(ahead, attrs) {
            per_race.push(v);
        }
    }
    let n = per_race.len() as u32;
    (mean(&per_race), n)
}

/// `ritmo_classificacao`: interpolação no grid da QUALI (posição de largada).
fn estimate_quali(samples: &[RaceSample]) -> (Option<f64>, u32) {
    let mut per_race = Vec::new();
    for s in samples {
        if s.player_start <= 0 {
            continue;
        }
        let attrs: Vec<f64> = s
            .grid
            .iter()
            .filter(|g| g.start > 0)
            .map(|g| g.quali_skill)
            .collect();
        let ahead = s
            .grid
            .iter()
            .filter(|g| g.start > 0 && g.start < s.player_start)
            .count();
        if let Some(v) = interpolate_in_grid(ahead, attrs) {
            per_race.push(v);
        }
    }
    let n = per_race.len() as u32;
    (mean(&per_race), n)
}

/// `fator_chuva`: só corridas molhadas; interpola a chegada contra o `fator_chuva`
/// das IAs. Se foi tão bem quanto as IAs de chuva alta, o jogador também é bom.
fn estimate_rain(samples: &[RaceSample]) -> (Option<f64>, u32) {
    let mut per_race = Vec::new();
    for s in samples {
        if !s.is_wet || s.player_dnf {
            continue;
        }
        let attrs: Vec<f64> = s
            .grid
            .iter()
            .filter(|g| !g.dnf)
            .map(|g| g.rain_skill)
            .collect();
        let ahead = s
            .grid
            .iter()
            .filter(|g| !g.dnf && g.finish < s.player_finish)
            .count();
        if let Some(v) = interpolate_in_grid(ahead, attrs) {
            per_race.push(v);
        }
    }
    let n = per_race.len() as u32;
    (mean(&per_race), n)
}

/// `racecraft`: por corrida, quão bem o jogador subiu na disputa.
/// - Com telemetria (iRacing): usa o SALDO de posições NA PISTA (`on_track_gained −
///   lost`, que exclui heranças de DNF) e premia/pune brigar muito conforme o saldo
///   — é o "racecraft avançado" (tempo em briga).
/// - Sem telemetria (simulado): cai no saldo oficial largada − chegada.
/// Corridas com abandono não contam.
fn estimate_racecraft(samples: &[RaceSample]) -> (Option<f64>, u32) {
    let mut per_race = Vec::new();
    for s in samples {
        if s.player_dnf {
            continue;
        }
        let value = match racecraft_from_telemetry(s) {
            Some(v) => Some(v),
            None => {
                if s.player_start > 0 && s.player_finish > 0 {
                    let gain = (s.player_start - s.player_finish) as f64;
                    Some((50.0 + gain * 4.0).clamp(1.0, 99.0))
                } else {
                    None
                }
            }
        };
        if let Some(v) = value {
            per_race.push(v);
        }
    }
    let n = per_race.len() as u32;
    (mean(&per_race), n)
}

/// Racecraft de UMA corrida a partir da telemetria (só quando há movimento real na
/// pista). Base = saldo na pista; a fração de briga amplia o efeito (brigar muito e
/// sair ganhando = elite; brigar muito e perder = ruim).
fn racecraft_from_telemetry(s: &RaceSample) -> Option<f64> {
    let t = s.telemetry.as_ref()?;
    if t.on_track_gained <= 0 && t.on_track_lost <= 0 {
        return None; // sem sinal de posição na pista
    }
    let net = (t.on_track_gained - t.on_track_lost) as f64;
    let battle = if t.battle_fraction >= 0.0 {
        t.battle_fraction
    } else {
        0.0
    };
    let battle_term = 10.0 * battle * net.signum();
    Some((50.0 + net * 4.0 + battle_term).clamp(1.0, 99.0))
}

/// `consistencia` (telemetria): média da nota de consistência das corridas com o
/// dado (voltas parelhas = alto). Só corridas dirigidas no iRacing contribuem.
fn estimate_consistency(samples: &[RaceSample]) -> (Option<f64>, u32) {
    let values: Vec<f64> = samples
        .iter()
        .filter_map(|s| s.telemetry.as_ref())
        .map(|t| t.consistency)
        .filter(|c| *c >= 0.0)
        .collect();
    let n = values.len() as u32;
    (mean(&values), n)
}

/// `habilidade_largada` (telemetria): média das posições ganhas nos ~20s iniciais,
/// mapeada para 0–100. +3 posições médias ≈ 68; −3 ≈ 32.
fn estimate_start(samples: &[RaceSample]) -> (Option<f64>, u32) {
    let deltas: Vec<f64> = samples
        .iter()
        .filter_map(|s| s.telemetry.as_ref())
        .filter(|t| t.start_valid)
        .map(|t| t.start_delta as f64)
        .collect();
    let n = deltas.len() as u32;
    let value = mean(&deltas).map(|d| (50.0 + d * 6.0).clamp(1.0, 99.0));
    (value, n)
}

/// `aggression`: média de incidentes por corrida. Muitos incidentes = agressivo.
///
/// **CURVA ILUSTRATIVA, e agora MEDIDA como mal escalada.** O mapa `0 inc → 15` e `5 inc → 95`
/// supõe que a média de incidentes por corrida percorra a faixa 0–5. Ela não percorre.
/// [`medicao::distribuicao_de_incidentes_por_corrida`], 4.800 piloto-corridas por categoria em
/// 11/08/2026:
///
/// | categoria | média | → nota | p90 | → nota | máximo | → nota |
/// |---|---|---|---|---|---|---|
/// | `mazda_rookie` | 0,484 | **23** | 1,000 | **31** | 5,000 | 95 |
/// | `gt3` | 0,426 | **22** | 1,000 | **31** | 5,000 | 95 |
///
/// Ou seja: **do piloto mediano ao percentil 90 vão 8 pontos de uma escala de 84.** Toda a
/// população cai entre 22 e 31, e a leitura que o jogador recebe é "passivo" quase sempre,
/// independentemente de como ele corre. O extremo superior existe (5 incidentes → 95), mas é
/// cauda rara, não faixa de trabalho.
///
/// O defeito é de ESCALA, não de deslocamento: mudar o `15` só move a faixa inteira, não a
/// abre. O conserto certo é o que [`estimate_skill`] já faz — normalizar contra a distribuição
/// do próprio grid em vez de usar o valor absoluto —, e ele não está feito aqui.
///
/// **Um caveat que impede fechar o número só com isto:** a medição acima é do MOTOR. Quando a
/// corrida vem importada do iRacing (`commands::iracing::resultado`), `player_incidents` é a
/// contagem do iRacing — o `x` de "4x" —, que vive numa escala bem mais alta. Os dois caminhos
/// alimentam o mesmo campo com grandezas diferentes, e essa é a pergunta a resolver ANTES de
/// escolher a curva: ou a normalização é por fonte, ou as duas contagens têm que ser
/// reconciliadas na entrada.
fn estimate_aggression(samples: &[RaceSample]) -> (Option<f64>, u32) {
    let inc: Vec<f64> = samples.iter().map(|s| s.player_incidents as f64).collect();
    let n = inc.len() as u32;
    // 0 inc ≈ 15 (passivo); ~5 inc ≈ 95 (kamikaze). 16 pontos por incidente.
    let value = mean(&inc).map(|i| (15.0 + i * 16.0).clamp(1.0, 99.0));
    (value, n)
}

/// `adaptabilidade`: melhora ao longo das temporadas. Compara a chegada média da
/// primeira temporada com a da última — melhorou = mais adaptável. Nº de amostras
/// = temporadas distintas com corrida finalizada.
fn estimate_adaptability(samples: &[RaceSample]) -> (Option<f64>, u32) {
    // chegada média por temporada (só finalizadores).
    let mut by_season: BTreeMap<i32, (f64, u32)> = BTreeMap::new();
    for s in samples {
        if s.player_dnf || s.player_finish <= 0 {
            continue;
        }
        let e = by_season.entry(s.season).or_insert((0.0, 0));
        e.0 += s.player_finish as f64;
        e.1 += 1;
    }
    let seasons: Vec<(i32, f64)> = by_season
        .into_iter()
        .map(|(season, (sum, count))| (season, sum / count as f64))
        .collect();
    let n = seasons.len() as u32;
    if n < 2 {
        return (None, n);
    }
    let first_avg = seasons.first().unwrap().1;
    let last_avg = seasons.last().unwrap().1;
    // melhora = chegada média caiu (posição menor é melhor). 1 posição ≈ 5 pontos.
    let improvement = first_avg - last_avg;
    let value = (50.0 + improvement * 5.0).clamp(1.0, 99.0);
    (Some(value), n)
}

/// `experiencia`: cresce com o nº de corridas, saturando. 60 corridas ≈ 50.
fn estimate_experience(total_races: u32) -> (Option<f64>, u32) {
    if total_races == 0 {
        return (None, 0);
    }
    let r = total_races as f64;
    let value = (100.0 * r / (r + 60.0)).clamp(1.0, 99.0);
    (Some(value), total_races)
}

// ── Montagem do dossiê ───────────────────────────────────────────────────────

fn confidence_for(sample_count: u32, unlock: Unlock) -> f64 {
    let full = (unlock.threshold() as f64 * CONFIDENCE_FULL_MULT).max(1.0);
    (sample_count as f64 / full).clamp(0.0, 1.0)
}

fn make_attribute(
    key: &'static str,
    value: Option<f64>,
    sample_count: u32,
    unlock: Unlock,
) -> DossierAttribute {
    let threshold = unlock.threshold();
    let unlocked = sample_count >= threshold && value.is_some();
    let rounded = value.map(|v| v.round().clamp(0.0, 100.0) as u8);
    let tag = if unlocked {
        rounded
            .and_then(|v| get_attribute_tag(key, v as f64))
            .map(|t| t.tag_text.to_string())
    } else {
        None
    };
    DossierAttribute {
        key: key.to_string(),
        unlocked,
        value: if unlocked { rounded } else { None },
        confidence: confidence_for(sample_count, unlock),
        sample_count,
        unlock_threshold: threshold,
        unlock_kind: unlock.kind().to_string(),
        remaining: threshold.saturating_sub(sample_count),
        tag,
    }
}

/// Constrói o dossiê completo do jogador a partir das amostras de corrida e da
/// `midia` atual (que é a própria fama do sistema — leitura direta, sem estimar).
pub fn build_dossier(samples: &[RaceSample], current_midia: f64) -> PlayerDossier {
    let total_races = samples.len() as u32;
    let total_seasons = samples
        .iter()
        .map(|s| s.season)
        .collect::<std::collections::BTreeSet<_>>()
        .len() as u32;

    let (skill, skill_n) = estimate_skill(samples);
    let (quali, quali_n) = estimate_quali(samples);
    let (rain, rain_n) = estimate_rain(samples);
    let (racecraft, racecraft_n) = estimate_racecraft(samples);
    let (aggression, aggression_n) = estimate_aggression(samples);
    let (adapt, adapt_n) = estimate_adaptability(samples);
    let (exp, exp_n) = estimate_experience(total_races);
    let (consistency, consistency_n) = estimate_consistency(samples);
    let (start, start_n) = estimate_start(samples);

    let attributes = vec![
        make_attribute("skill", skill, skill_n, UNLOCK_SKILL),
        make_attribute("ritmo_classificacao", quali, quali_n, UNLOCK_QUALI),
        make_attribute("racecraft", racecraft, racecraft_n, UNLOCK_RACECRAFT),
        make_attribute(
            "consistencia",
            consistency,
            consistency_n,
            UNLOCK_CONSISTENCY,
        ),
        make_attribute("habilidade_largada", start, start_n, UNLOCK_START),
        make_attribute("aggression", aggression, aggression_n, UNLOCK_AGGRESSION),
        make_attribute("fator_chuva", rain, rain_n, UNLOCK_RAIN),
        make_attribute("adaptabilidade", adapt, adapt_n, UNLOCK_ADAPT),
        make_attribute("experiencia", exp, exp_n, Unlock::Immediate),
        // midia é leitura direta da fama do sistema — sempre aberta.
        make_attribute(
            "midia",
            Some(current_midia),
            total_races.max(1),
            Unlock::Immediate,
        ),
    ];

    PlayerDossier {
        total_races,
        total_seasons,
        attributes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ai(skill: f64, finish: i32, start: i32) -> GridDriver {
        GridDriver {
            skill,
            quali_skill: skill,
            rain_skill: skill,
            finish,
            start,
            dnf: false,
        }
    }

    /// Grid do exemplo do spec: jogador passa 82/80/78, perde pra 88/85 → ~83.5.
    #[test]
    fn skill_interpolates_boundary_from_spec_example() {
        // Jogador terminou em 3º (2 IAs à frente: 88, 85).
        let grid = vec![
            ai(88.0, 1, 1),
            ai(85.0, 2, 2),
            ai(82.0, 4, 4),
            ai(80.0, 5, 5),
            ai(78.0, 6, 6),
        ];
        let sample = RaceSample {
            season: 1,
            round: 1,
            is_wet: false,
            player_finish: 3,
            player_start: 3,
            player_dnf: false,
            player_incidents: 0,
            grid,
            telemetry: None,
        };
        let (v, n) = estimate_skill(&[sample]);
        assert_eq!(n, 1);
        assert!((v.unwrap() - 83.5).abs() < 0.01, "got {v:?}");
    }

    #[test]
    fn skill_winner_sits_above_the_grid() {
        let grid = vec![ai(80.0, 2, 1), ai(75.0, 3, 2), ai(70.0, 4, 3)];
        let sample = RaceSample {
            season: 1,
            round: 1,
            is_wet: false,
            player_finish: 1,
            player_start: 4,
            player_dnf: false,
            player_incidents: 0,
            grid,
            telemetry: None,
        };
        let (v, _) = estimate_skill(&[sample]);
        assert!(
            (v.unwrap() - 83.0).abs() < 0.01,
            "venceu todos → 80+margem, got {v:?}"
        );
    }

    #[test]
    fn skill_last_sits_below_the_grid() {
        let grid = vec![ai(80.0, 1, 1), ai(75.0, 2, 2), ai(70.0, 3, 3)];
        let sample = RaceSample {
            season: 1,
            round: 1,
            is_wet: false,
            player_finish: 4,
            player_start: 4,
            player_dnf: false,
            player_incidents: 0,
            grid,
            telemetry: None,
        };
        let (v, _) = estimate_skill(&[sample]);
        assert!(
            (v.unwrap() - 67.0).abs() < 0.01,
            "perdeu de todos → 70−margem, got {v:?}"
        );
    }

    #[test]
    fn skill_skips_player_dnf_races() {
        let grid = vec![ai(80.0, 1, 1), ai(75.0, 2, 2)];
        let sample = RaceSample {
            season: 1,
            round: 1,
            is_wet: false,
            player_finish: 20,
            player_start: 3,
            player_dnf: true,
            player_incidents: 0,
            grid,
            telemetry: None,
        };
        let (v, n) = estimate_skill(&[sample]);
        assert_eq!(n, 0);
        assert!(v.is_none());
    }

    #[test]
    fn skill_ignores_dnf_ai_in_the_grid() {
        // A IA de 90 abandonou (finish alto) — não deve contar como "vencida".
        let mut strong = ai(90.0, 15, 1);
        strong.dnf = true;
        let grid = vec![strong, ai(70.0, 2, 2), ai(65.0, 3, 3)];
        let sample = RaceSample {
            season: 1,
            round: 1,
            is_wet: false,
            player_finish: 1,
            player_start: 4,
            player_dnf: false,
            player_incidents: 0,
            grid,
            telemetry: None,
        };
        let (v, _) = estimate_skill(&[sample]);
        // Só 70 e 65 contam → venceu ambos → 70+margem.
        assert!((v.unwrap() - 73.0).abs() < 0.01, "got {v:?}");
    }

    #[test]
    fn dossier_locks_until_threshold_then_unlocks() {
        // 4 corridas: skill (threshold 5) fica bloqueado.
        let four: Vec<RaceSample> = (0..4)
            .map(|i| RaceSample {
                season: 1,
                round: i + 1,
                is_wet: false,
                player_finish: 2,
                player_start: 2,
                player_dnf: false,
                player_incidents: 1,
                grid: vec![ai(80.0, 1, 1), ai(70.0, 3, 3)],
                telemetry: None,
            })
            .collect();
        let dossier = build_dossier(&four, 40.0);
        let skill = dossier
            .attributes
            .iter()
            .find(|a| a.key == "skill")
            .unwrap();
        assert!(!skill.unlocked);
        assert_eq!(skill.remaining, 1);
        assert!(skill.value.is_none());

        // 5ª corrida abre.
        let mut five = four.clone();
        five.push(RaceSample {
            season: 1,
            round: 5,
            is_wet: false,
            player_finish: 2,
            player_start: 2,
            player_dnf: false,
            player_incidents: 1,
            grid: vec![ai(80.0, 1, 1), ai(70.0, 3, 3)],
            telemetry: None,
        });
        let dossier = build_dossier(&five, 40.0);
        let skill = dossier
            .attributes
            .iter()
            .find(|a| a.key == "skill")
            .unwrap();
        assert!(skill.unlocked);
        assert_eq!(skill.remaining, 0);
        assert!(skill.value.is_some());
    }

    #[test]
    fn experience_and_midia_are_immediately_available() {
        let one = vec![RaceSample {
            season: 1,
            round: 1,
            is_wet: false,
            player_finish: 5,
            player_start: 5,
            player_dnf: false,
            player_incidents: 2,
            grid: vec![ai(80.0, 1, 1)],
            telemetry: None,
        }];
        let dossier = build_dossier(&one, 62.0);
        let exp = dossier
            .attributes
            .iter()
            .find(|a| a.key == "experiencia")
            .unwrap();
        let midia = dossier
            .attributes
            .iter()
            .find(|a| a.key == "midia")
            .unwrap();
        assert!(exp.unlocked && exp.value.is_some());
        assert!(midia.unlocked);
        assert_eq!(midia.value, Some(62));
    }

    #[test]
    fn rain_needs_three_wet_races() {
        let wet = |round| RaceSample {
            season: 1,
            round,
            is_wet: true,
            player_finish: 2,
            player_start: 4,
            player_dnf: false,
            player_incidents: 0,
            grid: vec![ai(80.0, 1, 1), ai(60.0, 3, 3)],
            telemetry: None,
        };
        let two = vec![wet(1), wet(2)];
        let dossier = build_dossier(&two, 30.0);
        let rain = dossier
            .attributes
            .iter()
            .find(|a| a.key == "fator_chuva")
            .unwrap();
        assert!(!rain.unlocked);
        assert_eq!(rain.unlock_kind, "wet_races");
        assert_eq!(rain.remaining, 1);

        let three = vec![wet(1), wet(2), wet(3)];
        let dossier = build_dossier(&three, 30.0);
        let rain = dossier
            .attributes
            .iter()
            .find(|a| a.key == "fator_chuva")
            .unwrap();
        assert!(rain.unlocked);
    }

    #[test]
    fn adaptability_needs_two_seasons_and_rewards_improvement() {
        // Temporada 1 chega em média 8º; temporada 2 em 3º → melhorou muito.
        let mk = |season, finish| RaceSample {
            season,
            round: 1,
            is_wet: false,
            player_finish: finish,
            player_start: finish,
            player_dnf: false,
            player_incidents: 0,
            grid: vec![ai(70.0, 1, 1)],
            telemetry: None,
        };
        let one_season = vec![mk(1, 8), mk(1, 8)];
        let dossier = build_dossier(&one_season, 30.0);
        let adapt = dossier
            .attributes
            .iter()
            .find(|a| a.key == "adaptabilidade")
            .unwrap();
        assert!(!adapt.unlocked, "uma temporada não basta");

        let two_seasons = vec![mk(1, 8), mk(2, 3)];
        let dossier = build_dossier(&two_seasons, 30.0);
        let adapt = dossier
            .attributes
            .iter()
            .find(|a| a.key == "adaptabilidade")
            .unwrap();
        assert!(adapt.unlocked);
        // melhora de 5 posições → 50 + 25 = 75.
        assert_eq!(adapt.value, Some(75));
    }

    #[test]
    fn confidence_grows_with_samples() {
        let mk = |round| RaceSample {
            season: 1,
            round,
            is_wet: false,
            player_finish: 2,
            player_start: 2,
            player_dnf: false,
            player_incidents: 1,
            grid: vec![ai(80.0, 1, 1), ai(70.0, 3, 3)],
            telemetry: None,
        };
        let five: Vec<_> = (0..5).map(mk).collect();
        let fifteen: Vec<_> = (0..15).map(mk).collect();
        let c5 = build_dossier(&five, 30.0)
            .attributes
            .iter()
            .find(|a| a.key == "skill")
            .unwrap()
            .confidence;
        let c15 = build_dossier(&fifteen, 30.0)
            .attributes
            .iter()
            .find(|a| a.key == "skill")
            .unwrap()
            .confidence;
        assert!(c15 > c5);
        assert!((c15 - 1.0).abs() < 1e-9, "15 amostras = confiança plena");
    }

    fn telemetry(
        consistency: f64,
        battle: f64,
        gained: i32,
        lost: i32,
        start_delta: i32,
        start_valid: bool,
    ) -> RaceTelemetry {
        RaceTelemetry {
            consistency,
            battle_fraction: battle,
            on_track_gained: gained,
            on_track_lost: lost,
            start_delta,
            start_valid,
        }
    }

    fn tele_sample(round: i32, tel: RaceTelemetry) -> RaceSample {
        RaceSample {
            season: 1,
            round,
            is_wet: false,
            player_finish: 5,
            player_start: 5,
            player_dnf: false,
            player_incidents: 0,
            grid: vec![ai(80.0, 1, 1)],
            telemetry: Some(tel),
        }
    }

    #[test]
    fn consistencia_unlocks_with_four_telemetry_races() {
        let three: Vec<_> = (0..3)
            .map(|i| tele_sample(i, telemetry(85.0, -1.0, 0, 0, 0, false)))
            .collect();
        let c = build_dossier(&three, 30.0)
            .attributes
            .into_iter()
            .find(|a| a.key == "consistencia")
            .unwrap();
        assert!(!c.unlocked);
        assert_eq!(c.unlock_kind, "telemetry_races");
        assert_eq!(c.remaining, 1);

        let four: Vec<_> = (0..4)
            .map(|i| tele_sample(i, telemetry(85.0, -1.0, 0, 0, 0, false)))
            .collect();
        let c = build_dossier(&four, 30.0)
            .attributes
            .into_iter()
            .find(|a| a.key == "consistencia")
            .unwrap();
        assert!(c.unlocked);
        assert_eq!(c.value, Some(85));
    }

    #[test]
    fn largada_maps_start_delta() {
        // start_delta médio +3 → 50 + 18 = 68; destrava com 5 corridas de telemetria.
        let five: Vec<_> = (0..5)
            .map(|i| tele_sample(i, telemetry(-1.0, -1.0, 0, 0, 3, true)))
            .collect();
        let l = build_dossier(&five, 30.0)
            .attributes
            .into_iter()
            .find(|a| a.key == "habilidade_largada")
            .unwrap();
        assert!(l.unlocked);
        assert_eq!(l.value, Some(68));
    }

    #[test]
    fn racecraft_refined_by_on_track_net_and_battle() {
        // Saldo oficial 0 (largou 5, chegou 5), mas subiu 5 na pista brigando muito:
        // 50 + 5*4 + 10*0.5 = 75.
        let tel = telemetry(-1.0, 0.5, 6, 1, 0, false);
        let races: Vec<_> = (0..5).map(|i| tele_sample(i, tel.clone())).collect();
        let rc = build_dossier(&races, 30.0)
            .attributes
            .into_iter()
            .find(|a| a.key == "racecraft")
            .unwrap();
        assert!(rc.unlocked);
        assert_eq!(rc.value, Some(75));
    }

    #[test]
    fn racecraft_falls_back_to_grid_without_telemetry() {
        // Sem telemetria: largou 8, chegou 4 → ganho 4 → 50 + 16 = 66.
        let races: Vec<RaceSample> = (0..5)
            .map(|i| RaceSample {
                season: 1,
                round: i,
                is_wet: false,
                player_finish: 4,
                player_start: 8,
                player_dnf: false,
                player_incidents: 0,
                grid: vec![ai(80.0, 1, 1)],
                telemetry: None,
            })
            .collect();
        let rc = build_dossier(&races, 30.0)
            .attributes
            .into_iter()
            .find(|a| a.key == "racecraft")
            .unwrap();
        assert_eq!(rc.value, Some(66));
    }
}

// ── Medição das curvas ilustrativas ──────────────────────────────────────────
//
// Ver o aviso no topo do módulo. Isto não é teste de invariante: é o instrumento que transforma
// "a curva parece errada" em "a curva está errada nesta faixa e por esta razão".

#[cfg(test)]
mod medicao {
    use crate::simulation::calibracao::arena::{self, ConfigTemporada};

    /// **Onde a curva de `aggression` realmente cai.**
    ///
    /// A curva supõe uma média de 0 a 5 incidentes por corrida. Este harness roda temporadas
    /// sintéticas pelo mesmo caminho do jogo, conta os incidentes por piloto-corrida e imprime
    /// a média, o percentil 90 e o máximo — junto com o valor de `aggression` que o dossiê
    /// atribuiria a cada um desses pontos.
    ///
    /// A leitura que interessa é a AMPLITUDE: se o piloto mediano e o piloto do topo do
    /// percentil 90 recebem notas coladas, a curva não está errada por deslocamento (que se
    /// conserta mudando o 15), está errada por ESCALA — e aí o conserto é trocar o coeficiente
    /// linear por uma normalização contra a distribuição do grid, do mesmo jeito que
    /// `estimate_skill` já faz com a posição em vez de usar o valor absoluto.
    ///
    /// **Por que a saída daqui não fecha o número sozinha:** o jogador NÃO é um piloto de IA.
    /// Ele corre no iRacing de verdade, com um contador de incidentes que é do iRacing (o `x`
    /// de "4x"), não o do nosso motor. Fechar a curva exige a distribuição de incidentes de
    /// corrida REAL do jogador, que só existe depois de haver histórico de corrida importada.
    /// O que este harness entrega é o LIMITE INFERIOR — a faixa do lado simulado — e a
    /// demonstração de que a suposição de 0–5 não vem dele.
    #[test]
    #[ignore = "pesado; harness de calibração da curva de aggression do dossiê"]
    fn distribuicao_de_incidentes_por_corrida() {
        for (rotulo, config) in [
            ("mazda_rookie", ConfigTemporada::rookie()),
            ("gt3", ConfigTemporada::gt3()),
        ] {
            let config = ConfigTemporada {
                pilotos: 20,
                etapas: 12,
                ..config
            }
            .com_incidentes(true);

            let mut por_piloto_corrida: Vec<f64> = Vec::new();
            for (_, corridas) in arena::rodar_campanha_crua(&config, 20, 7_311) {
                for corrida in &corridas {
                    for r in &corrida.race_results {
                        por_piloto_corrida.push(r.incidents_count as f64);
                    }
                }
            }
            por_piloto_corrida.sort_by(f64::total_cmp);

            let n = por_piloto_corrida.len();
            assert!(n > 0, "a campanha não produziu corrida nenhuma");
            let media = por_piloto_corrida.iter().sum::<f64>() / n as f64;
            let p90 = por_piloto_corrida[((n as f64 * 0.90) as usize).min(n - 1)];
            let maximo = por_piloto_corrida[n - 1];
            let nota = |i: f64| (15.0 + i * 16.0).clamp(1.0, 99.0);

            println!(
                "\n== {rotulo} — incidentes por piloto-corrida ({n} amostras) ==\n\
                 média  {media:.3} → aggression {:.0}\n\
                 p90    {p90:.3} → aggression {:.0}\n\
                 máximo {maximo:.3} → aggression {:.0}\n\
                 amplitude média→p90: {:.0} pontos de 84 possíveis",
                nota(media),
                nota(p90),
                nota(maximo),
                nota(p90) - nota(media),
            );
        }
    }
}
