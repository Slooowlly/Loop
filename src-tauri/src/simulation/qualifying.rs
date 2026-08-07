use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::constants::scoring::QUALI_SCORE_TO_LAP_MS;

use super::context::{SimDriver, SimulationContext};
use super::math::{
    car_weight_scale, category_car_performance, normalize_car_performance, rain_intensity_for,
};
use super::track_profile::TrackCharacter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualifyingResult {
    pub pilot_id: String,
    pub pilot_name: String,
    pub team_id: String,
    pub team_name: String,
    pub position: i32,
    pub quali_score: f64,
    pub best_lap_time_ms: f64,
    pub gap_to_pole_ms: f64,
    pub is_pole: bool,
    pub is_jogador: bool,
    /// Este piloto perdeu a volta boa na sessão (trânsito, amarela na hora errada, erro que
    /// jogou a volta fora) e largou muito atrás do que o ritmo dele daria. Evento raro e
    /// legível — é o gancho de "ele perdeu a volta no trânsito" para a narrativa.
    /// `#[serde(default)]` porque resultados já salvos não têm o campo.
    #[serde(default)]
    pub volta_perdida: bool,
}

// ─────────────────────── Sábado ≠ domingo ───────────────────────
//
// A classificação entregava a corrida decidida: os mesmos atributos, o mesmo carro e o
// mesmo sorteio simétrico dos dois lados. Três coisas separam as duas disciplinas aqui, e
// nenhuma delas é "mais aleatoriedade":
//
// 1. **Quali é o melhor de N tentativas; a corrida é a média.** Uma sessão são várias
//    voltas e vale a melhor; uma corrida é desempenho sustentado. A consequência sai de
//    graça e é o arquétipo mais reconhecível do esporte: `consistencia` INVERTE de sinal.
//    O irregular tem cauda alta — dá uma volta fora da curva e crava a pole. O metrônomo
//    entrega sempre a mesma volta boa-mas-não-excepcional, e recupera no domingo.
// 2. **O carro tem dois trims** (ver `car_build`): aqui lemos `car_performance_quali`.
// 3. **A volta perdida.** O risco de sábado é ASSIMÉTRICO — ninguém ganha meio segundo
//    por sorte, mas todo mundo pode perder a volta no trânsito.
//
// Todas as constantes deste bloco são PROVISÓRIAS: quem fecha os valores é o harness.

/// Janela ÚTIL de uma sessão de classificação, em ms — o pedaço do tempo de sessão que de
/// fato vira volta rápida (fora out-lap, in-lap, espera por espaço na pista e vida de pneu).
pub const QUALI_JANELA_UTIL_MS: f64 = 315_000.0;

/// Piso e teto de tentativas. Ninguém sai da sessão com menos de duas voltas boas, e mais
/// que cinco não muda mais nada — o máximo de N tentativas satura rápido.
pub const QUALI_TENTATIVAS_MIN: u32 = 2;
pub const QUALI_TENTATIVAS_MAX: u32 = 5;

/// A partir deste tier a classificação é dividida em fases (Q1/Q2/Q3): mais uma passada de
/// pneu novo para quem chega lá, ou seja, uma tentativa a mais.
pub const QUALI_TIER_FASES: u8 = 4;

/// Compensação de amplitude do "melhor de N".
///
/// Sem isto o melhor de N sai pela culatra, e a medição mostra: tomar o MÁXIMO de N
/// sorteios ENCOLHE o espalhamento do resultado (o máximo de 4 uniformes tem ~57% do desvio
/// de um sorteio só). Uma sessão menos ruidosa é uma sessão que lê melhor o ritmo real do
/// piloto — ou seja, o mecanismo pensado para descolar sábado de domingo estava
/// ACOPLANDO os dois.
///
/// A causa é de calibração, não de modelo: o `8.0` da faixa de variância descreve o
/// resultado de uma SESSÃO, e ao passar a sortear VOLTAS é a volta que precisa daquela
/// amplitude. Este fator devolve à sessão o desvio que ela tinha, deixando o melhor de N
/// mudar só o FORMATO da distribuição — assimétrico à direita, premiando a cauda de quem é
/// irregular. É o que se queria: descorrelacionar por construção, sem inflar variância.
///
/// Fator = desvio(1 sorteio) / desvio(máximo de N sorteios), para uniformes.
pub fn compensacao_de_tentativas(tentativas: u32) -> f64 {
    let n = tentativas.max(1) as f64;
    // Desvio do máximo de N uniformes em [0,1]: √(N / ((N+1)² (N+2))).
    let desvio_do_maximo = (n / ((n + 1.0).powi(2) * (n + 2.0))).sqrt();
    // Desvio de um uniforme em [0,1]: 1/√12.
    let desvio_de_um = (1.0_f64 / 12.0).sqrt();
    desvio_de_um / desvio_do_maximo
}

/// Quantas voltas rápidas de verdade este piloto tem nesta sessão.
///
/// Sai do tempo de volta da pista, e a consequência é boa: em Spa dão duas tentativas e a
/// quali fica perto do ritmo real; em Tsukuba dão cinco, e o irregular tem cinco chances de
/// achar a volta da vida. Pista longa premia quem é rápido; pista curta premia quem tem
/// cauda.
pub fn tentativas_de_quali(base_lap_time_ms: f64, category_tier: u8) -> u32 {
    let voltas = if base_lap_time_ms > 0.0 {
        (QUALI_JANELA_UTIL_MS / base_lap_time_ms).floor() as u32
    } else {
        QUALI_TENTATIVAS_MIN
    };
    let bonus = u32::from(category_tier >= QUALI_TIER_FASES);
    (voltas + bonus).clamp(QUALI_TENTATIVAS_MIN, QUALI_TENTATIVAS_MAX)
}

// ── A volta perdida ──

/// Probabilidade-base de uma sessão perdida, antes dos modificadores. PROVISÓRIO.
pub const VOLTA_PERDIDA_PROB_BASE: f64 = 0.045;

/// Teto duro da probabilidade — nem no pior cenário a sessão vira loteria.
pub const VOLTA_PERDIDA_PROB_MAX: f64 = 0.30;

/// Custo da volta perdida, medido em POSIÇÕES de grid. PROVISÓRIO.
pub const VOLTA_PERDIDA_CUSTO_POSICOES: f64 = 7.0;

/// Piso do custo em pontos de score, só como guarda para grid degenerado (todo mundo
/// exatamente igual, um piloto só).
pub const VOLTA_PERDIDA_CUSTO_MINIMO: f64 = 1.0;

/// Quanto vale, em pontos de score, perder a volta nesta sessão.
///
/// Ancorado em POSIÇÕES, não em pontos nem em fração do espalhamento: o custo é o preço de
/// [`VOLTA_PERDIDA_CUSTO_POSICOES`] lugares no grid desta sessão. Isso o torna justo em
/// qualquer categoria sem constante por categoria — e usar a MEDIANA dos intervalos entre
/// pilotos, em vez do espalhamento total, o deixa imune a um grid com um carro fora da
/// curva (que inflaria o intervalo médio e faria a punição virar exagero para todo mundo).
pub fn custo_volta_perdida(scores_limpos: &[f64]) -> f64 {
    let mut ordenados: Vec<f64> = scores_limpos.to_vec();
    ordenados.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let mut intervalos: Vec<f64> = ordenados.windows(2).map(|par| par[0] - par[1]).collect();
    if intervalos.is_empty() {
        return VOLTA_PERDIDA_CUSTO_MINIMO;
    }
    intervalos.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mediana = intervalos[intervalos.len() / 2];
    (mediana * VOLTA_PERDIDA_CUSTO_POSICOES).max(VOLTA_PERDIDA_CUSTO_MINIMO)
}

/// Molhado da pista em [0, 1], a partir da intensidade categórica que o resto da simulação
/// já usa. Só a volta perdida precisa disto como número.
pub fn molhado_da_pista(intensidade: crate::iracing_sdk::weather::RainIntensity) -> f64 {
    use crate::iracing_sdk::weather::RainIntensity;
    match intensidade {
        RainIntensity::None => 0.0,
        RainIntensity::Light => 0.35,
        RainIntensity::Decent => 0.60,
        RainIntensity::Heavy => 0.85,
        RainIntensity::VeryHeavy => 1.0,
    }
}

/// Chance de este piloto perder a volta boa nesta sessão.
///
/// Sai do que já existe, não de uma constante solta: quem anda no limite (`aggression`) se
/// mete em encrenca, quem é irregular (`consistencia` baixa) erra a volta, pelotão compacto
/// (`pack_density_factor`, já calculado pela extensão da pista) é trânsito na cara, e pista
/// molhada multiplica tudo.
pub fn prob_volta_perdida(
    aggression: f64,
    consistencia: f64,
    pack_density_factor: f64,
    rain_intensity: f64,
) -> f64 {
    let agressivo = 0.70 + 0.60 * (aggression.clamp(0.0, 100.0) / 100.0);
    let irregular = 0.70 + 0.60 * (1.0 - consistencia.clamp(0.0, 100.0) / 100.0);
    let transito = pack_density_factor.max(0.0);
    let molhado = 1.0 + 1.2 * rain_intensity.clamp(0.0, 1.0);

    (VOLTA_PERDIDA_PROB_BASE * agressivo * irregular * transito * molhado)
        .clamp(0.0, VOLTA_PERDIDA_PROB_MAX)
}

/// O que separa uma sessão de classificação de outra. `simulate_qualifying` usa
/// [`ConfigQuali::para`]; o construtor [`ConfigQuali::legada`] reproduz o comportamento
/// anterior ao pacote e existe para o harness (e para os testes) medirem o antes/depois
/// sem manter duas implementações vivas.
#[derive(Debug, Clone, Copy)]
pub struct ConfigQuali {
    /// Quantas voltas rápidas valem — o N do "melhor de N". 1 = a média de sempre.
    pub tentativas: u32,
    /// Se a sessão pode ser perdida.
    pub volta_perdida: bool,
    /// Se o carro usa o trim de volta única em vez do de corrida.
    pub trim_de_quali: bool,
    /// Se usa a mistura de atributos ANTERIOR ao pacote (`skill` dominante em vez de
    /// `ritmo_classificacao`). Só a linha de base liga isto.
    pub mix_legado: bool,
}

impl ConfigQuali {
    /// A sessão como o jogo a disputa.
    pub fn para(ctx: &SimulationContext) -> Self {
        Self {
            tentativas: tentativas_de_quali(ctx.base_lap_time_ms, ctx.category_tier),
            volta_perdida: true,
            trim_de_quali: true,
            mix_legado: false,
        }
    }

    /// A classificação como era antes do pacote: uma tentativa só, sem volta perdida, com o
    /// carro de corrida e com `skill` dominando a mistura de atributos. Serve de linha de
    /// base medível — sem ela, a métrica do pacote se compara consigo mesma.
    pub fn legada() -> Self {
        Self {
            tentativas: 1,
            volta_perdida: false,
            trim_de_quali: false,
            mix_legado: true,
        }
    }
}

/// Retorna os pesos (skill, ritmo_classificacao, car_performance, adaptabilidade)
/// adaptados ao caráter da pista. Somam 1.0.
///
/// **`ritmo_classificacao` domina, `skill` acompanha** — e essa ordem é o conserto da
/// segunda metade do problema. `skill` é o atributo que a CORRIDA também lê (é o maior peso
/// de `race/pontuacao.rs`); `ritmo_classificacao` a corrida não lê em lugar nenhum. Enquanto
/// o genérico pesasse mais que o específico, sábado era domingo por construção, por mais que
/// se mexesse na variância. Na geração de pilotos o ritmo de classificação sai de
/// `skill ± 12`, então há espaço real de piloto: existe o rápido de uma volta que não é o
/// mais rápido do grid, e agora isso aparece na pista.
///
/// Pesos PROVISÓRIOS — quem fecha é o harness.
fn qual_weights(character: TrackCharacter) -> (f64, f64, f64, f64) {
    match character {
        TrackCharacter::Flowing => (0.27, 0.43, 0.27, 0.03), // velocidade/carro dominam
        TrackCharacter::Technical => (0.25, 0.40, 0.25, 0.10), // baseline equilibrado
        TrackCharacter::Tight => (0.22, 0.35, 0.18, 0.25),   // adaptabilidade dominante
        TrackCharacter::Roval => (0.25, 0.45, 0.25, 0.05),   // volta única pura
    }
}

/// A mistura ANTERIOR ao pacote, com `skill` dominando. Preservada só como linha de base
/// para [`ConfigQuali::legada`] — não é usada pelo jogo.
fn qual_weights_legados(character: TrackCharacter) -> (f64, f64, f64, f64) {
    let (skill, ritmo, car, adapt) = qual_weights(character);
    (ritmo, skill, car, adapt)
}

pub fn simulate_qualifying(
    drivers: &[SimDriver],
    ctx: &SimulationContext,
    rng: &mut impl Rng,
) -> Vec<QualifyingResult> {
    simulate_qualifying_cfg(drivers, ctx, rng, ConfigQuali::para(ctx))
}

pub fn simulate_qualifying_cfg(
    drivers: &[SimDriver],
    ctx: &SimulationContext,
    rng: &mut impl Rng,
    cfg: ConfigQuali,
) -> Vec<QualifyingResult> {
    // Peso do carro escalado por categoria (rookie baixo, topo alto), com o delta
    // redistribuído nos pesos de piloto (soma = 1.0). Pilar A do redesign.
    let (base_skill, base_ritmo, base_car, base_adapt) = if cfg.mix_legado {
        qual_weights_legados(ctx.track_character)
    } else {
        qual_weights(ctx.track_character)
    };
    let car_scale = car_weight_scale(&ctx.category_id);
    let new_car = base_car * car_scale;
    let non_car = base_skill + base_ritmo + base_adapt;
    let factor = if non_car > 0.0 {
        (non_car + (base_car - new_car)) / non_car
    } else {
        1.0
    };
    let (w_skill, w_ritmo, w_car, w_adapt) = (
        base_skill * factor,
        base_ritmo * factor,
        new_car,
        base_adapt * factor,
    );

    let rain = rain_intensity_for(ctx.weather);

    // Quem perdeu a volta é decidido ANTES de saber o espalhamento do grid, mas o custo só
    // pode ser cobrado depois — por isso a sessão sai em duas passadas.
    let mut perdeu_a_volta: Vec<bool> = Vec::with_capacity(drivers.len());

    let mut results: Vec<QualifyingResult> = drivers
        .iter()
        .map(|driver| {
            // O carro da VOLTA ÚNICA, não o de corrida (ver `car_build`).
            let car_bruto = if cfg.trim_de_quali {
                driver.car_performance_quali
            } else {
                driver.car_performance
            };
            let car_norm =
                normalize_car_performance(category_car_performance(&ctx.category_id, car_bruto));
            let mut base = driver.skill as f64 * w_skill
                + driver.ritmo_classificacao as f64 * w_ritmo
                + car_norm * w_car
                + driver.adaptabilidade as f64 * w_adapt;

            // Chuva: MESMA penalidade por piloto do export iRacing (curva validada
            // `rain_skill_penalty`) — rain-good perde menos pontos. Seco = 0. A partir do pacote
            // G, escalada pela `rain_sensitivity` da pista/categoria, que até então era calculada
            // no perfil e nunca lida.
            base -= crate::simulation::math::rain_penalty_escalada(
                ctx.weather,
                driver.fator_chuva as f64,
                ctx.rain_sensitivity,
            );

            if driver.corridas_na_categoria < 10 {
                let experience_penalty = (10 - driver.corridas_na_categoria) as f64 * 0.005;
                base *= 1.0 - experience_penalty;
            }

            // Variância escalada pelo perfil da categoria. É a AMPLITUDE de UMA VOLTA deste
            // piloto: o irregular tem faixa larga, o metrônomo tem faixa estreita. A
            // compensação mantém o espalhamento da SESSÃO no que o perfil calibrou — ver
            // `compensacao_de_tentativas`.
            let variance_range = (100.0 - driver.consistencia as f64) / 100.0
                * 8.0
                * ctx.qualifying_variance_multiplier
                * compensacao_de_tentativas(cfg.tentativas);

            // MELHOR DE N. É aqui que sábado deixa de ser domingo: a corrida soma
            // segmentos e portanto premia a MÉDIA; a sessão vale a melhor volta e portanto
            // premia a CAUDA. Com faixa larga, o máximo de N sorteios sobe muito mais do
            // que a média subiria — o irregular crava a pole e apanha no domingo.
            let mut score = f64::NEG_INFINITY;
            for _ in 0..cfg.tentativas.max(1) {
                let tentativa = base + rng.gen_range(-variance_range..=variance_range);
                if tentativa > score {
                    score = tentativa;
                }
            }

            // A VOLTA PERDIDA: trânsito na volta rápida, amarela na hora errada, um erro
            // que joga a volta fora. Assimétrico de propósito — não existe o simétrico
            // disso, ninguém ganha meio segundo por sorte. O custo é cobrado abaixo.
            let perdeu = cfg.volta_perdida
                && rng.gen::<f64>()
                    < prob_volta_perdida(
                        driver.aggression as f64,
                        driver.consistencia as f64,
                        ctx.pack_density_factor,
                        molhado_da_pista(rain),
                    );
            perdeu_a_volta.push(perdeu);

            QualifyingResult {
                pilot_id: driver.id.clone(),
                pilot_name: driver.nome.clone(),
                team_id: driver.team_id.clone(),
                team_name: driver.team_name.clone(),
                position: 0,
                quali_score: score.max(10.0),
                best_lap_time_ms: 0.0,
                gap_to_pole_ms: 0.0,
                is_pole: false,
                is_jogador: driver.is_jogador,
                volta_perdida: perdeu,
            }
        })
        .collect();

    // Custo da volta perdida: o preço de N posições no grid desta sessão. O espalhamento é
    // lido do grid INTEIRO (antes de qualquer punição) para o custo não depender de quantos
    // pilotos perderam a volta na mesma sessão.
    if perdeu_a_volta.iter().any(|p| *p) {
        let scores: Vec<f64> = results.iter().map(|r| r.quali_score).collect();
        let custo = custo_volta_perdida(&scores);
        for (result, perdeu) in results.iter_mut().zip(&perdeu_a_volta) {
            if *perdeu {
                result.quali_score = (result.quali_score - custo).max(1.0);
            }
        }
    }

    results.sort_by(|a, b| {
        b.quali_score
            .partial_cmp(&a.quali_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let pole_score = results
        .first()
        .map(|result| result.quali_score)
        .unwrap_or(0.0);
    let pole_time = ctx.base_lap_time_ms;

    for (index, result) in results.iter_mut().enumerate() {
        result.position = index as i32 + 1;
        result.is_pole = index == 0;
        result.best_lap_time_ms = ctx.base_lap_time_ms
            + (pole_score - result.quali_score).max(0.0) * QUALI_SCORE_TO_LAP_MS;
        result.gap_to_pole_ms = (result.best_lap_time_ms - pole_time).max(0.0);
    }

    results
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use crate::models::driver::Driver;
    use crate::models::enums::WeatherCondition;
    use crate::models::team::placeholder_team_from_db;
    use crate::simulation::context::SimulationContext;
    use crate::simulation::track_profile::TrackCharacter;

    use super::*;

    fn sample_context(weather: WeatherCondition) -> SimulationContext {
        SimulationContext {
            weather,
            ..SimulationContext::test_default()
        }
    }

    fn build_driver(id: &str, skill: f64, quali: f64, rain: f64, consistency: f64) -> SimDriver {
        let mut driver = Driver::create_player(
            id.to_string(),
            format!("Driver {}", id),
            "🇧🇷 Brasileiro".to_string(),
            20,
        );
        driver.is_jogador = false;
        driver.atributos.skill = skill;
        driver.atributos.ritmo_classificacao = quali;
        driver.atributos.fator_chuva = rain;
        driver.atributos.consistencia = consistency;
        driver.atributos.adaptabilidade = 60.0;

        let mut team = placeholder_team_from_db(
            format!("T{}", id),
            format!("Team {}", id),
            "mazda_rookie".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = 8.0;

        SimDriver::from_driver_and_team(&driver, &team)
    }

    fn sample_grid() -> Vec<SimDriver> {
        (0..12)
            .map(|index| {
                build_driver(
                    &format!("{:03}", index + 1),
                    50.0 + index as f64,
                    48.0 + index as f64,
                    50.0,
                    85.0,
                )
            })
            .collect()
    }

    #[test]
    fn test_qualifying_returns_all_drivers() {
        let mut rng = StdRng::seed_from_u64(11);
        let results = simulate_qualifying(
            &sample_grid(),
            &sample_context(WeatherCondition::Dry),
            &mut rng,
        );
        assert_eq!(results.len(), 12);
    }

    #[test]
    fn test_qualifying_positions_sequential() {
        let mut rng = StdRng::seed_from_u64(12);
        let results = simulate_qualifying(
            &sample_grid(),
            &sample_context(WeatherCondition::Dry),
            &mut rng,
        );
        let positions: Vec<i32> = results.iter().map(|result| result.position).collect();
        assert_eq!(positions, (1..=12).collect::<Vec<_>>());
    }

    #[test]
    fn test_qualifying_higher_skill_tends_to_pole() {
        let elite = build_driver("A", 95.0, 95.0, 50.0, 90.0);
        let grid: Vec<SimDriver> = std::iter::once(elite.clone())
            .chain((0..11).map(|index| build_driver(&format!("B{index}"), 55.0, 55.0, 50.0, 75.0)))
            .collect();

        let mut top3_finishes = 0;
        for seed in 0..100 {
            let mut rng = StdRng::seed_from_u64(seed);
            let results =
                simulate_qualifying(&grid, &sample_context(WeatherCondition::Dry), &mut rng);
            let pos = results
                .iter()
                .find(|result| result.pilot_id == elite.id)
                .expect("elite result")
                .position;
            if pos <= 3 {
                top3_finishes += 1;
            }
        }

        assert!(
            top3_finishes >= 80,
            "elite driver only reached top3 {} times",
            top3_finishes
        );
    }

    #[test]
    fn test_qualifying_rain_penalty_applied() {
        let wet_specialist = build_driver("RAIN", 75.0, 75.0, 95.0, 90.0);
        let wet_weak = build_driver("DRY", 75.0, 75.0, 20.0, 90.0);
        let grid = vec![wet_specialist.clone(), wet_weak.clone()];

        let mut better_in_rain = 0;
        for seed in 0..30 {
            let mut rng = StdRng::seed_from_u64(seed);
            let results = simulate_qualifying(
                &grid,
                &sample_context(WeatherCondition::HeavyRain),
                &mut rng,
            );
            if results[0].pilot_id == wet_specialist.id {
                better_in_rain += 1;
            }
        }

        assert!(better_in_rain >= 20);
    }

    #[test]
    fn test_qualifying_gap_to_pole() {
        let mut rng = StdRng::seed_from_u64(14);
        let results = simulate_qualifying(
            &sample_grid(),
            &sample_context(WeatherCondition::Dry),
            &mut rng,
        );
        assert_eq!(results[0].gap_to_pole_ms, 0.0);
        assert!(results
            .iter()
            .skip(1)
            .all(|result| result.gap_to_pole_ms > 0.0));
    }

    #[test]
    fn test_qualifying_lap_times_ordered() {
        let mut rng = StdRng::seed_from_u64(15);
        let results = simulate_qualifying(
            &sample_grid(),
            &sample_context(WeatherCondition::Dry),
            &mut rng,
        );
        assert!(results
            .windows(2)
            .all(|window| window[0].best_lap_time_ms <= window[1].best_lap_time_ms));
    }

    #[test]
    fn test_tight_track_favors_adaptability_over_flowing() {
        use super::qual_weights;
        let (_, _, _, tight_adapt) = qual_weights(TrackCharacter::Tight);
        let (_, _, _, flowing_adapt) = qual_weights(TrackCharacter::Flowing);
        assert!(
            tight_adapt > flowing_adapt,
            "Tight adapt weight={} should > Flowing={}",
            tight_adapt,
            flowing_adapt
        );
    }

    #[test]
    fn test_roval_favors_skill_over_tight() {
        use super::qual_weights;
        let (roval_skill, _, _, _) = qual_weights(TrackCharacter::Roval);
        let (tight_skill, _, _, _) = qual_weights(TrackCharacter::Tight);
        assert!(
            roval_skill > tight_skill,
            "Roval skill weight={} should > Tight={}",
            roval_skill,
            tight_skill
        );
    }

    #[test]
    fn test_gt3_has_less_variance_than_rookie() {
        use crate::simulation::profile::resolve_simulation_profile;

        let rookie_profile =
            resolve_simulation_profile("mazda_rookie", 47, 22.0, WeatherCondition::Dry, 15, 12);
        let gt3_profile =
            resolve_simulation_profile("gt3", 47, 22.0, WeatherCondition::Dry, 50, 20);

        let rookie_ctx = SimulationContext {
            category_id: "mazda_rookie".to_string(),
            category_tier: 0,
            qualifying_variance_multiplier: rookie_profile.qualifying_variance_multiplier,
            base_lap_time_ms: rookie_profile.base_lap_time_ms,
            ..SimulationContext::test_default()
        };
        let gt3_ctx = SimulationContext {
            category_id: "gt3".to_string(),
            category_tier: 4,
            qualifying_variance_multiplier: gt3_profile.qualifying_variance_multiplier,
            base_lap_time_ms: gt3_profile.base_lap_time_ms,
            ..SimulationContext::test_default()
        };

        // Grid uniforme para isolar o efeito da variância
        let grid: Vec<SimDriver> = (0..10)
            .map(|i| build_driver(&format!("D{i}"), 70.0, 70.0, 50.0, 80.0))
            .collect();

        let mut rookie_spread_sum = 0.0_f64;
        let mut gt3_spread_sum = 0.0_f64;
        let runs = 50;

        for seed in 0..runs {
            let mut rng = StdRng::seed_from_u64(seed);
            let rookie_results = simulate_qualifying(&grid, &rookie_ctx, &mut rng);
            let rookie_scores: Vec<f64> = rookie_results.iter().map(|r| r.quali_score).collect();
            let rookie_max = rookie_scores
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            let rookie_min = rookie_scores.iter().cloned().fold(f64::INFINITY, f64::min);
            rookie_spread_sum += rookie_max - rookie_min;

            let mut rng2 = StdRng::seed_from_u64(seed + 1000);
            let gt3_results = simulate_qualifying(&grid, &gt3_ctx, &mut rng2);
            let gt3_scores: Vec<f64> = gt3_results.iter().map(|r| r.quali_score).collect();
            let gt3_max = gt3_scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let gt3_min = gt3_scores.iter().cloned().fold(f64::INFINITY, f64::min);
            gt3_spread_sum += gt3_max - gt3_min;
        }

        let rookie_avg_spread = rookie_spread_sum / runs as f64;
        let gt3_avg_spread = gt3_spread_sum / runs as f64;

        assert!(
            rookie_avg_spread > gt3_avg_spread,
            "rookie avg spread={:.2} should > gt3 avg spread={:.2}",
            rookie_avg_spread,
            gt3_avg_spread
        );
    }

    // O casamento shape↔pista na classificação segue o mesmo `car_performance` efetivo
    // do sim (via `effective_car_performance_from_shape`), testado deterministicamente em
    // `car_build` e `context`. O antigo teste por perfil discreto foi aposentado com o
    // `CarBuildProfile`.

    // ─────────────────────── Melhor de N ───────────────────────

    #[test]
    fn tentativas_caem_com_o_tamanho_da_pista() {
        // Spa (~2:20) dá poucas voltas boas; Tsukuba (~1:00) dá muitas.
        let spa = tentativas_de_quali(140_000.0, 3);
        let tsukuba = tentativas_de_quali(60_000.0, 3);
        assert!(spa < tsukuba, "spa={spa} tsukuba={tsukuba}");
        assert!((QUALI_TENTATIVAS_MIN..=QUALI_TENTATIVAS_MAX).contains(&spa));
        assert!((QUALI_TENTATIVAS_MIN..=QUALI_TENTATIVAS_MAX).contains(&tsukuba));
    }

    #[test]
    fn tier_com_fases_ganha_uma_tentativa() {
        // Mesma pista, categoria com Q1/Q2/Q3 tem mais uma passada de pneu novo.
        let media = tentativas_de_quali(100_000.0, 1);
        let topo = tentativas_de_quali(100_000.0, QUALI_TIER_FASES);
        assert!(topo > media, "topo={topo} media={media}");
    }

    #[test]
    fn tentativas_respeitam_piso_e_teto() {
        assert_eq!(tentativas_de_quali(10_000_000.0, 0), QUALI_TENTATIVAS_MIN);
        assert_eq!(tentativas_de_quali(1_000.0, 6), QUALI_TENTATIVAS_MAX);
        // Pista sem tempo-base resolvido não pode explodir nem zerar.
        assert_eq!(tentativas_de_quali(0.0, 0), QUALI_TENTATIVAS_MIN);
    }

    #[test]
    fn compensacao_preserva_o_espalhamento_da_sessao() {
        // Uma tentativa não compensa nada.
        assert!((compensacao_de_tentativas(1) - 1.0).abs() < 1e-9);
        // Mais tentativas → faixa por volta mais larga, porque o máximo estreita.
        assert!(compensacao_de_tentativas(4) > compensacao_de_tentativas(2));

        // O que ela promete: o desvio do RESULTADO da sessão fica igual ao de um sorteio
        // só. Sem isso, o melhor de N deixaria a quali menos ruidosa e, portanto, MAIS
        // preditiva da corrida — o oposto do objetivo.
        let desvio_da_sessao = |n: u32| {
            let faixa = 8.0 * compensacao_de_tentativas(n);
            let mut amostras = Vec::new();
            for seed in 0..4000 {
                let mut rng = StdRng::seed_from_u64(seed);
                let mut melhor = f64::NEG_INFINITY;
                for _ in 0..n {
                    melhor = melhor.max(rng.gen_range(-faixa..=faixa));
                }
                amostras.push(melhor);
            }
            let media: f64 = amostras.iter().sum::<f64>() / amostras.len() as f64;
            (amostras.iter().map(|v| (v - media).powi(2)).sum::<f64>() / amostras.len() as f64)
                .sqrt()
        };

        let uma = desvio_da_sessao(1);
        for n in [2, 3, 4, 5] {
            let atual = desvio_da_sessao(n);
            assert!(
                (atual - uma).abs() < uma * 0.08,
                "N={n}: desvio da sessão {atual:.3} deveria bater com {uma:.3}"
            );
        }
    }

    /// A peça central do pacote: `consistencia` INVERTE de sinal entre sábado e domingo.
    /// Dois pilotos de mesmo ritmo, um metrônomo e um irregular — na média (1 tentativa)
    /// eles empatam; no melhor de N o irregular leva, porque o máximo premia a cauda.
    #[test]
    fn irregular_ganha_da_media_na_quali_mas_nao_no_ritmo() {
        let irregular = build_driver("WILD", 70.0, 70.0, 50.0, 30.0);
        let metronomo = build_driver("METRO", 70.0, 70.0, 50.0, 95.0);
        let grid = vec![irregular.clone(), metronomo.clone()];
        let ctx = sample_context(WeatherCondition::Dry);

        let poles_do_irregular = |cfg: ConfigQuali| {
            let mut poles = 0;
            for seed in 0..400 {
                let mut rng = StdRng::seed_from_u64(seed);
                let r = simulate_qualifying_cfg(&grid, &ctx, &mut rng, cfg);
                if r[0].pilot_id == irregular.id {
                    poles += 1;
                }
            }
            poles
        };

        let sem_ruido = ConfigQuali {
            tentativas: 1,
            volta_perdida: false,
            trim_de_quali: false,
            mix_legado: false,
        };
        // Uma tentativa só (a "média"): moeda ao ar entre os dois.
        let na_media = poles_do_irregular(sem_ruido);
        // Melhor de 4: o irregular tem cauda e ela aparece.
        let no_melhor_de_n = poles_do_irregular(ConfigQuali {
            tentativas: 4,
            ..sem_ruido
        });

        assert!(
            (150..=250).contains(&na_media),
            "com 1 tentativa deveria dar quase 50/50, deu {na_media}/400"
        );
        assert!(
            no_melhor_de_n > na_media + 60,
            "o melhor de N tem que premiar a cauda: {no_melhor_de_n} vs {na_media} em 400"
        );
    }

    // ─────────────────────── A volta perdida ───────────────────────

    #[test]
    fn prob_volta_perdida_cresce_com_risco_transito_e_chuva() {
        let calmo = prob_volta_perdida(20.0, 95.0, 1.0, 0.0);
        let agressivo = prob_volta_perdida(95.0, 95.0, 1.0, 0.0);
        let irregular = prob_volta_perdida(20.0, 20.0, 1.0, 0.0);
        let apertado = prob_volta_perdida(20.0, 95.0, 1.4, 0.0);
        let molhado = prob_volta_perdida(20.0, 95.0, 1.0, 1.0);

        assert!(agressivo > calmo, "{agressivo} vs {calmo}");
        assert!(irregular > calmo, "{irregular} vs {calmo}");
        assert!(apertado > calmo, "{apertado} vs {calmo}");
        assert!(molhado > calmo, "{molhado} vs {calmo}");
        assert!(prob_volta_perdida(100.0, 0.0, 1.4, 1.0) <= VOLTA_PERDIDA_PROB_MAX);
    }

    #[test]
    fn volta_perdida_e_rara() {
        // Frequência: um piloto médio, em pelotão médio e pista seca, perde a sessão em
        // torno de uma vez a cada vinte — raro o bastante para ser notícia.
        let p = prob_volta_perdida(50.0, 50.0, 1.1, 0.0);
        assert!(
            (0.02..=0.09).contains(&p),
            "frequência fora do razoável: {p}"
        );
    }

    #[test]
    fn custo_da_volta_perdida_vale_o_numero_de_posicoes_prometido() {
        // Grid espaçado de exatamente 1 ponto por posição: o custo tem que sair igual ao
        // número de posições da constante.
        let escada: Vec<f64> = (0..12).map(|i| 100.0 - i as f64).collect();
        let custo = custo_volta_perdida(&escada);
        assert!(
            (custo - VOLTA_PERDIDA_CUSTO_POSICOES).abs() < 0.01,
            "custo={custo}, esperado {VOLTA_PERDIDA_CUSTO_POSICOES}"
        );

        // Grid colado: em pontos o custo é menor, mas continua valendo as mesmas posições.
        let colado: Vec<f64> = (0..12).map(|i| 100.0 - i as f64 * 0.2).collect();
        let custo_colado = custo_volta_perdida(&colado);
        assert!(custo_colado < custo, "{custo_colado} vs {custo}");
        assert!(
            (custo_colado - 0.2 * VOLTA_PERDIDA_CUSTO_POSICOES).abs() < 0.01,
            "custo_colado={custo_colado}"
        );
    }

    #[test]
    fn custo_da_volta_perdida_ignora_o_carro_fora_da_curva() {
        // Um piloto 30 pontos à frente não pode inflar a punição do resto do grid — é por
        // isso que a âncora é a MEDIANA dos intervalos, não o espalhamento total.
        let mut com_outlier: Vec<f64> = (0..12).map(|i| 100.0 - i as f64).collect();
        com_outlier[0] = 130.0;
        let custo = custo_volta_perdida(&com_outlier);
        assert!(
            (custo - VOLTA_PERDIDA_CUSTO_POSICOES).abs() < 0.01,
            "o outlier vazou pro custo: {custo}"
        );
    }

    #[test]
    fn volta_perdida_derruba_o_piloto_no_grid() {
        // Num grid realista, quem perde a volta larga MUITO atrás de onde o ritmo o poria.
        // Medido comparando a MESMA sessão com e sem o evento ligado.
        let grid: Vec<SimDriver> = (0..12)
            .map(|i| {
                build_driver(
                    &format!("D{i:02}"),
                    72.0 - i as f64,
                    72.0 - i as f64,
                    50.0,
                    70.0,
                )
            })
            .collect();
        let ctx = sample_context(WeatherCondition::Dry);
        let com = ConfigQuali::para(&ctx);
        let sem = ConfigQuali {
            volta_perdida: false,
            ..com
        };

        let mut quedas: Vec<i32> = Vec::new();
        for seed in 0..400 {
            let mut rng_a = StdRng::seed_from_u64(seed);
            let com_evento = simulate_qualifying_cfg(&grid, &ctx, &mut rng_a, com);
            let mut rng_b = StdRng::seed_from_u64(seed);
            let sem_evento = simulate_qualifying_cfg(&grid, &ctx, &mut rng_b, sem);
            for driver in &grid {
                let a = com_evento
                    .iter()
                    .find(|x| x.pilot_id == driver.id)
                    .expect("com")
                    .position;
                let b = sem_evento
                    .iter()
                    .find(|x| x.pilot_id == driver.id)
                    .expect("sem")
                    .position;
                if a - b >= 3 {
                    quedas.push(a - b);
                }
            }
        }

        assert!(
            !quedas.is_empty(),
            "ninguém perdeu a volta em 400 sessões de 12 pilotos"
        );
        let media = quedas.iter().sum::<i32>() as f64 / quedas.len() as f64;
        println!(
            "volta perdida: {} quedas grandes em 4800 largadas | queda média {media:.1} posições",
            quedas.len()
        );
        assert!(
            media >= 4.0,
            "a queda média foi de só {media:.1} posições — barato demais para 'perdeu a volta'"
        );
    }

    // ─────────────────────── Trim de quali ───────────────────────

    #[test]
    fn quali_le_o_trim_de_quali_e_a_corrida_nao_ve_diferenca() {
        // Dois pilotos idênticos; um tem carro melhor SÓ na volta única. A quali enxerga.
        let mut apoio = build_driver("APOIO", 70.0, 70.0, 50.0, 95.0);
        let ponta = build_driver("PONTA", 70.0, 70.0, 50.0, 95.0);
        apoio.car_performance_quali = ponta.car_performance_quali + 4.0;
        // …e o trim de CORRIDA dos dois continua igual: o pacote não mexeu nisso.
        assert_eq!(apoio.car_performance, ponta.car_performance);

        let grid = vec![apoio.clone(), ponta.clone()];
        let ctx = sample_context(WeatherCondition::Dry);
        let mut poles = 0;
        for seed in 0..200 {
            let mut rng = StdRng::seed_from_u64(seed);
            let r = simulate_qualifying_cfg(
                &grid,
                &ctx,
                &mut rng,
                ConfigQuali {
                    tentativas: 3,
                    volta_perdida: false,
                    trim_de_quali: true,
                    mix_legado: false,
                },
            );
            if r[0].pilot_id == apoio.id {
                poles += 1;
            }
        }
        assert!(poles > 130, "o trim de quali tem que pesar: {poles}/200");
    }

    // ─────────────────────── A métrica do pacote ───────────────────────
    //
    // O que define se o pacote funcionou: a correlação (Spearman) entre a ordem de
    // classificação e a ordem de chegada. Rode com:
    //
    //   cargo test --manifest-path src-tauri/Cargo.toml correlacao_quali -- --nocapture
    #[cfg(test)]
    mod correlacao {
        use super::*;
        use crate::simulation::catalog::IncidentCatalog;
        use crate::simulation::race::{
            simulate_race_com_modo, simulate_race_with_breakdowns, ModoDoMotor,
        };
        use crate::simulation::track_profile::get_track_simulation_data;

        /// As mesmas cinco etapas da temporada sintética de `simulation::forma`.
        const ETAPAS: [(u32, &str); 5] = [
            (523, "Spa"),
            (166, "Okayama"),
            (413, "Hungaroring"),
            (586, "Laguna Seca"),
            (239, "Monza"),
        ];

        /// Sorteio determinístico e reprodutível em `[min, max]`, a partir de (piloto,
        /// atributo). Faz o papel do `rng` de `models::driver_generation` sem depender de
        /// ordem de chamada.
        fn sorteio(piloto: usize, atributo: u64, min: i32, max: i32) -> i32 {
            let mut h = 0xcbf2_9ce4_8422_2325_u64;
            for b in (piloto as u64)
                .to_le_bytes()
                .iter()
                .chain(atributo.to_le_bytes().iter())
            {
                h ^= *b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01B3);
            }
            h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            h ^= h >> 27;
            min + (h % ((max - min + 1) as u64)) as i32
        }

        /// Grid sintético de 12 pilotos em 6 equipes de 2, montado com a MESMA estrutura de
        /// correlação de `models::driver_generation` — e não com tudo colado no nível do
        /// piloto, como na primeira demo. Isso importa para a métrica: lá, `consistencia`,
        /// `ritmo_classificacao` e `gestao_pneus` andavam junto com o `skill`, o que inflava
        /// artificialmente o acoplamento entre sábado e domingo.
        ///
        /// Fiel à geração real: `consistencia = skill ± 10`, `racecraft/defesa = skill ± 8`,
        /// `ritmo_classificacao = skill ± 12` (é aqui que mora o especialista em volta
        /// única), e pneu/largada/adaptabilidade/agressividade sorteados INDEPENDENTES do
        /// nível, em 40–70. `smoothness` é o inverso da agressividade.
        fn grid() -> Vec<SimDriver> {
            (0..12)
                .map(|i| {
                    let equipe = i / 2;
                    let skill = 72 - i as i32;
                    let com_desvio = |atributo: u64, desvio: i32| -> u8 {
                        (skill + sorteio(i, atributo, -desvio, desvio)).clamp(5, 99) as u8
                    };
                    let independente = |atributo: u64, min: i32, max: i32| -> u8 {
                        sorteio(i, atributo, min, max) as u8
                    };
                    let aggression = independente(9, 30, 70);
                    // Metade da garagem monta carro de apoio (crava a volta), metade monta
                    // carro de ponta (rende no stint) — o viés que `car_build` deriva do
                    // shape, aqui posto à mão para o grid não depender de um `Car` semeado.
                    let vies = if equipe % 2 == 0 {
                        crate::simulation::car_build::GANHO_TRIM_QUALI
                    } else {
                        -crate::simulation::car_build::GANHO_TRIM_QUALI
                    };
                    let corrida = 78.0 - equipe as f64 * 1.2;
                    SimDriver {
                        id: format!("DRV-{:02}", i + 1),
                        nome: format!("Piloto {:02}", i + 1),
                        is_jogador: false,
                        skill: skill as u8,
                        consistencia: com_desvio(1, 10),
                        racecraft: com_desvio(2, 8),
                        defesa: com_desvio(3, 8),
                        ritmo_classificacao: com_desvio(4, 12),
                        gestao_pneus: independente(5, 40, 70),
                        habilidade_largada: independente(6, 40, 70),
                        adaptabilidade: independente(7, 40, 70),
                        fator_chuva: independente(8, 30, 70),
                        fitness: independente(10, 40, 70),
                        experiencia: independente(11, 40, 70),
                        aggression,
                        smoothness: 100u8.saturating_sub(aggression),
                        mentalidade: independente(12, 40, 70),
                        confianca: independente(13, 40, 70),
                        motivacao: 70.0,
                        car_performance: corrida,
                        car_performance_quali: corrida + vies,
                        // Campo novo do pacote D (ar sujo). Este grid de teste não modela
                        // shape de carro, então fica neutro.
                        vies_de_pico: 0.0,
                        qualidade_de_estrategia: 50.0,
                        car_reliability: 90.0,
                        team_id: format!("TEAM-{}", equipe + 1),
                        team_name: format!("Equipe {}", equipe + 1),
                        corridas_na_categoria: 40,
                        pressure_error_mult: 1.0,
                        duelo_de_pista: None,
                    }
                })
                .collect()
        }

        fn contexto(track_id: u32, nome: &str) -> SimulationContext {
            SimulationContext {
                track_id,
                track_name: nome.to_string(),
                track_character: get_track_simulation_data(track_id).track_character,
                ..SimulationContext::test_default()
            }
        }

        /// Spearman entre duas ordens dadas como posição por piloto (mesma indexação).
        fn spearman(a: &[f64], b: &[f64]) -> f64 {
            let n = a.len() as f64;
            let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
            let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
            let va: f64 = a.iter().map(|x| (x - ma).powi(2)).sum::<f64>().sqrt();
            let vb: f64 = b.iter().map(|y| (y - mb).powi(2)).sum::<f64>().sqrt();
            if va == 0.0 || vb == 0.0 {
                return 0.0;
            }
            cov / (va * vb)
        }

        /// Correlação média entre grid e chegada, sobre as 5 etapas × `repeticoes` semeadas.
        fn correlacao_media(cfg: ConfigQuali, repeticoes: u64) -> f64 {
            let grid = grid();
            let mut soma = 0.0;
            let mut n = 0;
            for (i, (track_id, nome)) in ETAPAS.iter().enumerate() {
                let ctx = contexto(*track_id, nome);
                for r in 0..repeticoes {
                    let semente = 7_000 + (i as u64) * 1_000 + r;
                    // Repete o que `engine::run_full_race_with_breakdowns` faz — quali e
                    // depois corrida, no MESMO rng — só que com a config de quali sob
                    // teste. Chamar o engine direto não serviria: ele roda a quali dele
                    // internamente, e aí a linha de base mediria a quali nova de qualquer
                    // jeito. A corrida recebe a grade que esta sessão produziu, que é
                    // exatamente o acoplamento que queremos medir.
                    let mut rng = StdRng::seed_from_u64(semente);
                    let quali = simulate_qualifying_cfg(&grid, &ctx, &mut rng, cfg);
                    // Corrida com o TRÁFEGO DESLIGADO (`LegadoDePontos`), de propósito. Este
                    // teste afere o mecanismo DESTE pacote, e medi-lo no mundo com posição na
                    // pista o diluía: o pacote D levantou o piso de 0,232 para ~0,47, e o
                    // limiar teve de cair de 0,04 para 0,02 só por causa disso. Cada pacote
                    // futuro diluiria de novo, e o limiar cairia de novo. Medindo no mundo em
                    // que o F foi calibrado, nenhum pacote posterior mexe nesta régua.
                    let corrida = simulate_race_com_modo(
                        &grid,
                        &quali,
                        &ctx,
                        &IncidentCatalog::empty(),
                        false,
                        None,
                        ModoDoMotor::LegadoDePontos,
                        &mut rng,
                    );
                    let mut posicao_quali = Vec::with_capacity(grid.len());
                    let mut posicao_chegada = Vec::with_capacity(grid.len());
                    for driver in &grid {
                        let q = quali
                            .iter()
                            .find(|x| x.pilot_id == driver.id)
                            .map(|x| x.position as f64);
                        let c = corrida
                            .race_results
                            .iter()
                            .find(|x| x.pilot_id == driver.id)
                            .map(|x| x.finish_position as f64);
                        if let (Some(q), Some(c)) = (q, c) {
                            posicao_quali.push(q);
                            posicao_chegada.push(c);
                        }
                    }
                    soma += spearman(&posicao_quali, &posicao_chegada);
                    n += 1;
                }
            }
            soma / n as f64
        }

        /// Diagnóstico: quanto da correlação grid × chegada vem PURAMENTE de largar na
        /// frente, e não de o rápido ser rápido nos dois dias.
        ///
        /// Mede embaralhando a grade — os pilotos largam em ordem SORTEADA, sem relação
        /// nenhuma com o ritmo deles. Tudo o que sobrar de correlação aí é causado pela
        /// posição de largada em si, ou seja, pelo bônus de grid de `race/motor.rs`. Esse
        /// número é o TETO do que qualquer trabalho feito na classificação pode derrubar:
        /// enquanto ele existir, mexer em quem faz a pole só mexe na outra metade.
        fn correlacao_com_grade_sorteada(repeticoes: u64) -> f64 {
            let grid = grid();
            let mut soma = 0.0;
            let mut n = 0;
            for (i, (track_id, nome)) in ETAPAS.iter().enumerate() {
                let ctx = contexto(*track_id, nome);
                for r in 0..repeticoes {
                    let semente = 31_000 + (i as u64) * 1_000 + r;
                    let mut rng = StdRng::seed_from_u64(semente);
                    let mut quali =
                        simulate_qualifying_cfg(&grid, &ctx, &mut rng, ConfigQuali::legada());
                    // Embaralha a GRADE mantendo os mesmos pilotos: quem larga onde passa a
                    // ser sorteio puro. Fisher-Yates com o mesmo rng.
                    for k in (1..quali.len()).rev() {
                        let j = (rng.gen::<u64>() % (k as u64 + 1)) as usize;
                        let (pa, pb) = (quali[k].position, quali[j].position);
                        quali[k].position = pb;
                        quali[j].position = pa;
                        quali.swap(k, j);
                    }
                    quali.sort_by_key(|x| x.position);
                    let corrida = simulate_race_with_breakdowns(
                        &grid,
                        &quali,
                        &ctx,
                        &IncidentCatalog::empty(),
                        false,
                        None,
                        &mut rng,
                    );
                    let mut pq = Vec::new();
                    let mut pc = Vec::new();
                    for driver in &grid {
                        let q = quali
                            .iter()
                            .find(|x| x.pilot_id == driver.id)
                            .map(|x| x.position as f64);
                        let c = corrida
                            .race_results
                            .iter()
                            .find(|x| x.pilot_id == driver.id)
                            .map(|x| x.finish_position as f64);
                        if let (Some(q), Some(c)) = (q, c) {
                            pq.push(q);
                            pc.push(c);
                        }
                    }
                    soma += spearman(&pq, &pc);
                    n += 1;
                }
            }
            soma / n as f64
        }

        #[test]
        fn correlacao_quali_chegada_cai_com_o_pacote() {
            let antes = correlacao_media(ConfigQuali::legada(), 40);
            let ctx = contexto(523, "Spa");
            let depois = correlacao_media(ConfigQuali::para(&ctx), 40);

            let piso = correlacao_com_grade_sorteada(40);
            let legada = ConfigQuali::legada();
            let completa = ConfigQuali::para(&ctx);

            // Contribuição de cada mecanismo SOZINHO, para o harness saber onde mexer.
            let so_mix = correlacao_media(
                ConfigQuali {
                    mix_legado: false,
                    ..legada
                },
                40,
            );
            let so_melhor_de_n = correlacao_media(
                ConfigQuali {
                    tentativas: completa.tentativas,
                    ..legada
                },
                40,
            );
            let so_trim = correlacao_media(
                ConfigQuali {
                    trim_de_quali: true,
                    ..legada
                },
                40,
            );
            let so_volta_perdida = correlacao_media(
                ConfigQuali {
                    volta_perdida: true,
                    ..legada
                },
                40,
            );

            println!("\n=== Correlação (Spearman) grid × chegada, 5 etapas × 40 semeadas ===");
            println!("ANTES  (quali legada)                     : {antes:.3}");
            println!("DEPOIS (mix + melhor de N + trim + volta) : {depois:.3}");
            println!(
                "queda                                     : {:.3}",
                antes - depois
            );
            println!("\n-- contribuição de cada mecanismo sozinho (queda contra o ANTES) --");
            println!(
                "mistura de atributos (ritmo > skill) : {so_mix:.3}  ({:+.3})",
                so_mix - antes
            );
            println!(
                "melhor de N                          : {so_melhor_de_n:.3}  ({:+.3})",
                so_melhor_de_n - antes
            );
            println!(
                "trim de quali do carro               : {so_trim:.3}  ({:+.3})",
                so_trim - antes
            );
            println!(
                "volta perdida                        : {so_volta_perdida:.3}  ({:+.3})",
                so_volta_perdida - antes
            );
            println!("\nPISO (grade SORTEADA — só o bônus de largada de race/motor.rs): {piso:.3}");
            println!(
                "→ largar na frente, sozinho, explica {piso:.3} da correlação; o resto é o \
                 rápido ser rápido nos dois dias. O pacote comeu {:.0}% do acoplamento \
                 que está acima desse piso.",
                (antes - depois) / (antes - piso).max(1e-9) * 100.0
            );

            assert!(
                antes > 0.80,
                "a linha de base tem que mostrar o sintoma: {antes:.3}"
            );
            // Limiar de volta em 0,04, agora que a medição roda com o tráfego desligado (ver o
            // comentário em `correlacao_media`). O 0,02 provisório do pacote D era sintoma de
            // medir o mecanismo num mundo que muda debaixo dele; medindo no mundo em que o F
            // foi calibrado, nenhum pacote posterior mexe nesta régua.
            assert!(
                depois < antes - 0.04,
                "o pacote tem que descorrelacionar: {antes:.3} -> {depois:.3}"
            );
            // O piso não é meu: enquanto largar na frente valer o que vale hoje, ele é o
            // limite do que qualquer trabalho na classificação consegue derrubar.
            assert!(
                depois > piso - 0.05,
                "medida suspeita: ficamos ABAIXO do piso de grade sorteada ({depois:.3} < {piso:.3})"
            );
        }

        #[test]
        fn os_dois_trims_do_carro_se_descolam_no_grid() {
            // Quanto o carro de sábado deixou de prever o de domingo.
            let grid = grid();
            let corrida: Vec<f64> = grid.iter().map(|d| d.car_performance).collect();
            let quali: Vec<f64> = grid.iter().map(|d| d.car_performance_quali).collect();
            let r = spearman(&corrida, &quali);
            println!("correlação entre os dois trims do carro: {r:.3}");
            assert!(
                (0.2..0.95).contains(&r),
                "os trims têm que andar juntos SEM serem o mesmo número: {r:.3}"
            );
        }
    }
}
