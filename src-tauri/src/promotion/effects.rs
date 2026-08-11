use rand::Rng;
use rusqlite::Connection;

use crate::db::queries::teams as team_queries;
use crate::finance::events::parachute_payment_for_relegation;
use crate::models::team::Team;
use crate::promotion::{MovementType, TeamAttributeDelta};

pub fn calculate_promotion_effects(team: &Team, rng: &mut impl Rng) -> TeamAttributeDelta {
    TeamAttributeDelta {
        team_id: team.id.clone(),
        team_name: team.nome.clone(),
        movement_type: MovementType::Promocao,
        // Anti-snowball: a promoção NÃO dá mais carro de graça. O ganho fixo
        // (+5..10, independente de caixa) permitia que uma equipe rookie — carro
        // spec, que nunca investiu no carro — chegasse na categoria de cima já com
        // um carro forte, pulando o gating econômico e subindo a escada sem parar.
        // Agora a equipe MANTÉM o número do carro dela (sem reset): como a banda de
        // carro da categoria nova é mais alta, ela entra por baixo e tem que ganhar
        // evolução via a economia da própria categoria (offseason em cashflow.rs).
        // Prêmio da promoção segue nos demais atributos (orçamento, estrutura, etc.).
        car_performance_delta: 0.0,
        budget_delta: rng.gen_range(5.0..=15.0),
        facilities_delta: rng.gen_range(0.0..=5.0),
        engineering_delta: rng.gen_range(0.0..=3.0),
        morale_multiplier: 1.15,
        reputacao_delta: rng.gen_range(3.0..=8.0),
    }
}

pub fn calculate_relegation_effects(team: &Team, rng: &mut impl Rng) -> TeamAttributeDelta {
    TeamAttributeDelta {
        team_id: team.id.clone(),
        team_name: team.nome.clone(),
        movement_type: MovementType::Rebaixamento,
        car_performance_delta: rng.gen_range(-16.0..=-10.0),
        budget_delta: rng.gen_range(-30.0..=-20.0),
        facilities_delta: rng.gen_range(-15.0..=-8.0),
        engineering_delta: rng.gen_range(-15.0..=-8.0),
        morale_multiplier: 0.60,
        reputacao_delta: rng.gen_range(-25.0..=-15.0),
    }
}

/// Soft-landing da promoção, LIGADO por padrão (desligável via
/// `IRACER_PROMO_SOFT_LANDING=0` para A/B no Monte Carlo).
///
/// **Fala em NÍVEL DE PEÇA, não na coluna legada `car_performance`.** A versão
/// anterior calculava um pouso sofisticado e escrevia num número que o sim não lê:
/// desde o Sistema de Nível do Carro, [`crate::models::team::Team::effective_car_performance`]
/// deriva o ritmo de `team_car` e IGNORA a coluna sempre que o time tem carro
/// persistido — ou seja, o pouso inteiro era inerte, e o promovido caía na
/// categoria nova com o carro cru da de baixo (na Production, nível 2 contra um
/// campo no 4).
///
/// Alvo = o nível do **pior incumbente** que permanece na categoria de destino.
/// Intenção de design: o campeão de baixo deve entrar "um pouco melhor que a pior
/// da categoria de cima" — com chance real de brigar pela PERMANÊNCIA, longe de
/// brigar por título. O resto da escada até o teto ele constrói sozinho, na
/// cadência de desenvolvimento (3–4 upgrades por temporada), o que leva as 2–3
/// temporadas em que ele deveria estar aprendendo a categoria.
///
/// - `field_levels` = nível MÉDIO do carro de cada incumbente que fica (exclui o
///   próprio promovido e a rebaixada, que já saiu na troca).
///
/// `None` se o campo estiver vazio (nada a posicionar). Nunca REBAIXA o carro: o
/// pouso é piso, não teto — quem chega acima do pior incumbente fica onde está.
pub fn promotion_landing_level(field_levels: &[f64]) -> Option<u8> {
    if field_levels.is_empty() {
        return None;
    }
    let worst = field_levels.iter().copied().fold(f64::INFINITY, f64::min);
    Some(worst.floor().clamp(1.0, 10.0) as u8)
}

pub fn apply_attribute_deltas(
    conn: &Connection,
    team_id: &str,
    delta: &TeamAttributeDelta,
) -> Result<(), String> {
    let mut team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao buscar equipe '{team_id}': {e}"))?
        .ok_or_else(|| format!("Equipe '{team_id}' nao encontrada"))?;

    // Sem teto superior (Pilar B): só piso em −5.
    team.car_performance = (team.car_performance + delta.car_performance_delta).max(-5.0);
    team.cash_balance += promotion_budget_delta_to_cash(&team, delta.budget_delta);
    team.facilities = (team.facilities + delta.facilities_delta).clamp(0.0, 100.0);
    team.engineering = (team.engineering + delta.engineering_delta).clamp(0.0, 100.0);
    team.morale = (team.morale * delta.morale_multiplier).clamp(0.5, 1.5);
    team.reputacao = (team.reputacao + delta.reputacao_delta).clamp(0.0, 100.0);
    if delta.movement_type == MovementType::Rebaixamento {
        team.parachute_payment_remaining += parachute_payment_for_relegation(&team);
    }

    team_queries::update_team(conn, &team)
        .map_err(|e| format!("Falha ao atualizar equipe '{}': {e}", team.nome))?;
    Ok(())
}

/// Quantos MESES de operação o pacote da promoção paga por ponto de `budget_delta`.
///
/// É o valor que a fórmula antiga já entregava, escrito na unidade em que ele existe. Ela
/// dizia `janela_de_caixa × delta/100 × 0,35`, e a janela é `cash_max − cash_min` = 11 − 1
/// = **dez meses** de operação por construção: o produto sempre foi `0,035 × delta` meses.
/// Nada mudou de magnitude aqui — mudou o que dá para ler no código.
const MESES_POR_PONTO_DE_PROMOCAO: f64 = 0.035;

/// Converte o `budget_delta` do pacote de promoção em dinheiro.
///
/// # O rótulo não bate mais com a escala
///
/// `budget_delta` é declarado em PONTOS de `budget_index` (+5 a +15 ao subir, −20 a −30 ao
/// cair). Depois que o índice foi re-derivado sobre a escada de estados, dez pontos perto da
/// banda `saudavel` valem ~7 meses de operação — e este pacote paga 0,35. **O pacote é ~20×
/// menor do que o próprio nome dele afirma.**
///
/// Isso não é regressão desta mudança: a fórmula antiga nunca leu a escala do índice, só a
/// janela de caixa, então o pacote sempre valeu 0,035 mês por ponto. O que a re-derivação
/// fez foi tornar a discrepância visível — antes o índice saturava em 100 e um "delta de 10
/// pontos" não tinha significado nenhum contra o qual conferir.
///
/// Fica na magnitude de hoje **de propósito**. Fazer o delta valer o que ele diz multiplicaria
/// por vinte a injeção de caixa de toda promoção, que é calibração do anti-snowball
/// ([`PromotionDiminishConfig`]) e não conserto de unidade.
fn promotion_budget_delta_to_cash(team: &Team, budget_delta: f64) -> f64 {
    let mensal =
        crate::finance::state::custo_operacional_mensal(&team.categoria, team.classe.as_deref());

    mensal * budget_delta * MESES_POR_PONTO_DE_PROMOCAO
}

/// Parâmetros do retorno decrescente do pacote ECONÔMICO da promoção.
pub struct PromotionDiminishConfig {
    /// Fator geométrico por promoção encadeada (0..1). Menor = freio mais forte.
    pub decay: f64,
    /// Janela móvel (em temporadas): promoções espaçadas além disso resetam a contagem.
    pub window: i32,
    /// Piso do fator — mesmo encadeando muitas promoções, o pacote nunca zera.
    pub floor: f64,
}

/// Retorno decrescente do pacote ECONÔMICO da promoção (anti-snowball do chain-promotion).
/// LIGADO por padrão; desligável via `IRACER_PROMO_DIMINISH=0` (A/B no Monte Carlo).
///
/// Motivo: a promoção injeta orçamento/estrutura/engenharia TODA vez que a equipe sobe, e
/// esse caixa vira carro no offseason (`finance::cashflow`) — então uma equipe que vence a
/// categoria uma vez tende a chegar MAIS FORTE na de cima e vencer de novo, encadeando
/// promoções (rookie→amador→...→production) sem freio. Aqui cada promoção DENTRO da janela
/// móvel rende um fator menor (`decay^(n-1)`, piso `floor`): a 1ª vem cheia (fator 1.0), a
/// 2ª encadeada vale `decay`, a 3ª `decay²`, etc. Promoções espaçadas (fora da janela)
/// resetam a contagem — punimos o FOGUETE, não a equipe que sobe, consolida e volta a subir
/// anos depois. Calibrável por `IRACER_PROMO_DIMINISH_{DECAY,WINDOW,FLOOR}`. `None` =
/// desligado (aplica pacote cheio, nada a persistir).
pub fn promotion_diminish_config() -> Option<PromotionDiminishConfig> {
    // As quatro estão declaradas em `constants::flags_experimentais` (padrão, dono e
    // efeito); aqui fica só o clamp de cada uma, que é regra deste cálculo.
    use crate::constants::flags_experimentais as flags;
    if !flags::booleana("IRACER_PROMO_DIMINISH") {
        return None;
    }
    let decay = flags::numerica("IRACER_PROMO_DIMINISH_DECAY").clamp(0.0, 1.0);
    let window = (flags::numerica("IRACER_PROMO_DIMINISH_WINDOW") as i32).max(1);
    let floor = flags::numerica("IRACER_PROMO_DIMINISH_FLOOR").clamp(0.0, 1.0);
    Some(PromotionDiminishConfig {
        decay,
        window,
        floor,
    })
}

/// Fator a multiplicar no pacote econômico e a nova contagem a persistir para a promoção
/// corrente. `last_promotion_season`/`recent_promotions` vêm do histórico da equipe (0/0 se
/// nunca subiu). Ver `promotion_diminish_config` para o design. Função PURA (testável sem DB).
pub fn promotion_diminish_factor(
    last_promotion_season: i32,
    recent_promotions: i32,
    current_season: i32,
    config: &PromotionDiminishConfig,
) -> (f64, i32) {
    let chained = last_promotion_season > 0
        && current_season > last_promotion_season
        && current_season - last_promotion_season <= config.window;
    let next_recent = if chained { recent_promotions + 1 } else { 1 };
    let exp = (next_recent - 1).max(0);
    let factor = config.decay.powi(exp).max(config.floor);
    (factor, next_recent)
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::constants::teams::get_team_templates;
    use crate::db::migrations;
    use crate::db::queries::teams as team_queries;
    use crate::models::team::Team;
    use crate::promotion::MovementType;

    #[test]
    fn test_promotion_effects_positive() {
        let mut rng = StdRng::seed_from_u64(40);
        let team = sample_team("gt4", "T001");

        let delta = calculate_promotion_effects(&team, &mut rng);

        assert_eq!(delta.movement_type, MovementType::Promocao);
        // Anti-snowball: promoção não dá mais carro de graça (mantém o número atual).
        assert_eq!(delta.car_performance_delta, 0.0);
        assert!(delta.budget_delta > 0.0);
        assert!(delta.facilities_delta >= 0.0);
        assert!(delta.engineering_delta >= 0.0);
        assert!(delta.morale_multiplier > 1.0);
        assert!(delta.reputacao_delta > 0.0);
    }

    #[test]
    fn test_relegation_effects_negative() {
        let mut rng = StdRng::seed_from_u64(41);
        let team = sample_team("gt4", "T001");

        let delta = calculate_relegation_effects(&team, &mut rng);

        assert_eq!(delta.movement_type, MovementType::Rebaixamento);
        assert!(delta.car_performance_delta < 0.0);
        assert!(delta.budget_delta < 0.0);
        assert!(delta.facilities_delta <= 0.0);
        assert!(delta.engineering_delta < 0.0);
        assert!(delta.morale_multiplier < 1.0);
        assert!(delta.reputacao_delta < 0.0);
    }

    #[test]
    fn test_relegation_effects_are_heavy_enough_to_limit_immediate_bounce_back() {
        let mut rng = StdRng::seed_from_u64(42);
        let team = sample_team("mazda_rookie", "T001");

        let delta = calculate_relegation_effects(&team, &mut rng);

        assert!(delta.car_performance_delta <= -10.0);
        assert!(delta.budget_delta <= -20.0);
        assert!(delta.facilities_delta <= -8.0);
        assert!(delta.engineering_delta <= -8.0);
        assert!(delta.morale_multiplier <= 0.65);
        assert!(delta.reputacao_delta <= -15.0);
    }

    #[test]
    fn test_effects_clamped() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let mut team = sample_team("gt4", "T001");
        team.car_performance = 15.5;
        team.budget = 99.0;
        team.facilities = 99.0;
        team.engineering = 99.0;
        team.morale = 1.45;
        team.reputacao = 99.0;
        let before_cash = team.cash_balance;
        team_queries::insert_team(&conn, &team).expect("insert team");

        let delta = TeamAttributeDelta {
            team_id: team.id.clone(),
            team_name: team.nome.clone(),
            movement_type: MovementType::Promocao,
            car_performance_delta: 5.0,
            budget_delta: 10.0,
            facilities_delta: 10.0,
            engineering_delta: 10.0,
            morale_multiplier: 1.15,
            reputacao_delta: 10.0,
        };

        apply_attribute_deltas(&conn, &team.id, &delta).expect("apply deltas");
        let updated = team_queries::get_team_by_id(&conn, &team.id)
            .expect("team query")
            .expect("team exists");

        // Pilar B: car_performance não tem mais teto — o delta aplica integral
        // (15.5 + 5.0). Os demais atributos seguem clampados (100 / 1.5).
        assert_eq!(updated.car_performance, 20.5);
        assert!(updated.cash_balance > before_cash);
        let expected_budget = crate::finance::planning::derive_budget_index_from_money(&updated);
        assert!((updated.budget - expected_budget).abs() < 0.0001);
        assert_eq!(updated.facilities, 100.0);
        assert_eq!(updated.engineering, 100.0);
        assert_eq!(updated.morale, 1.5);
        assert_eq!(updated.reputacao, 100.0);
    }

    #[test]
    fn test_relegation_delta_initializes_parachute_payment() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let team = sample_team("gt4", "T001");
        team_queries::insert_team(&conn, &team).expect("insert team");

        let delta = TeamAttributeDelta {
            team_id: team.id.clone(),
            team_name: team.nome.clone(),
            movement_type: MovementType::Rebaixamento,
            car_performance_delta: -4.0,
            budget_delta: -8.0,
            facilities_delta: -1.0,
            engineering_delta: -2.0,
            morale_multiplier: 0.75,
            reputacao_delta: -6.0,
        };

        apply_attribute_deltas(&conn, &team.id, &delta).expect("apply deltas");
        let updated = team_queries::get_team_by_id(&conn, &team.id)
            .expect("team query")
            .expect("team exists");

        assert!(updated.parachute_payment_remaining > 0.0);
    }

    fn sample_team(category: &str, id: &str) -> Team {
        let template = get_team_templates(category)[0];
        let mut rng = StdRng::seed_from_u64(404);
        Team::from_template_with_rng(template, category, id.to_string(), 2025, &mut rng)
    }

    #[test]
    fn test_landing_aterrissa_no_nivel_do_pior_incumbente() {
        // Campo da Production com um lanterna no 2 e o resto no teto 4: o promovido
        // aterrissa no 2 — dentro do campo, mas no fundo dele.
        let field = [2.0, 4.0, 4.0, 4.0];
        assert_eq!(promotion_landing_level(&field), Some(2));
    }

    #[test]
    fn test_landing_nunca_aterrissa_no_meio_do_peloton() {
        // Anti-título: mesmo num campo todo no teto, o alvo é o PIOR — nunca a média.
        let field = [3.0, 4.0, 4.0, 4.0];
        assert_eq!(promotion_landing_level(&field), Some(3));
    }

    #[test]
    fn test_landing_arredonda_o_nivel_medio_para_baixo() {
        // Nível médio fracionário (carro desparelho) não pode virar um nível a mais.
        let field = [2.9, 4.0];
        assert_eq!(promotion_landing_level(&field), Some(2));
    }

    #[test]
    fn test_landing_none_when_field_empty() {
        assert!(promotion_landing_level(&[]).is_none());
    }

    fn diminish_cfg() -> PromotionDiminishConfig {
        PromotionDiminishConfig {
            decay: 0.5,
            window: 3,
            floor: 0.1,
        }
    }

    #[test]
    fn test_diminish_first_promotion_is_full() {
        // Nunca subiu (0,0): fator cheio (1.0), contagem começa em 1.
        let (factor, next) = promotion_diminish_factor(0, 0, 5, &diminish_cfg());
        assert!((factor - 1.0).abs() < 1e-9);
        assert_eq!(next, 1);
    }

    #[test]
    fn test_diminish_chained_promotions_decay_geometrically() {
        let cfg = diminish_cfg();
        // Subiu na 5 (recent=1); sobe de novo na 6 (dentro da janela): fator = decay^1.
        let (f2, n2) = promotion_diminish_factor(5, 1, 6, &cfg);
        assert!((f2 - 0.5).abs() < 1e-9);
        assert_eq!(n2, 2);
        // 3ª encadeada (recent=2), temporada 7: decay^2.
        let (f3, n3) = promotion_diminish_factor(6, 2, 7, &cfg);
        assert!((f3 - 0.25).abs() < 1e-9);
        assert_eq!(n3, 3);
    }

    #[test]
    fn test_diminish_respects_floor() {
        let cfg = diminish_cfg(); // decay 0.5, floor 0.1
                                  // recent alto → decay^5 = 0.03125, mas o piso segura em 0.1.
        let (factor, next) = promotion_diminish_factor(10, 5, 11, &cfg);
        assert!((factor - 0.1).abs() < 1e-9, "piso deve segurar o fator");
        assert_eq!(next, 6);
    }

    #[test]
    fn test_diminish_resets_when_promotion_is_outside_window() {
        let cfg = diminish_cfg(); // janela = 3
                                  // Subiu na 5 (recent=3), mas só volta a subir na 10 (gap 5 > 3): reseta a contagem.
        let (factor, next) = promotion_diminish_factor(5, 3, 10, &cfg);
        assert!(
            (factor - 1.0).abs() < 1e-9,
            "promoção espaçada volta ao cheio"
        );
        assert_eq!(next, 1);
    }
}
