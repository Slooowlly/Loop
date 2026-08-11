use std::collections::HashMap;

use crate::event_interest::models::{InterestTier, RealizedEventInterest};
use crate::models::injury::Injury;

// ── Tipos públicos ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum MediaImpactReason {
    Win,
    Pole,
    Podium,
    /// P4–P5: não vira manchete, mas aparece. É o que permite a um piloto regular
    /// construir nome sem nunca subir no pódio.
    Top5,
    /// P6–P10: o mínimo para não sumir. O ganho é menor que o decaimento de quem
    /// não pontua, então é presença, não escada.
    Top10,
    MainIncident,
    Injury,
}

/// Impacto consolidado de mídia pública para um driver após uma corrida.
/// `reasons` preserva todos os papéis que contribuíram, em ordem de processamento,
/// com deduplicação (sem repetição do mesmo reason por piloto).
#[derive(Debug, Clone)]
pub struct DriverMediaImpact {
    pub driver_id: String,
    pub delta: f64,
    pub reasons: Vec<MediaImpactReason>,
}

/// Contexto mínimo e explícito da corrida para cálculo de impacto público.
///
/// `winner_id` e `pole_sitter_id` são strings canônicas (sempre válidas em corrida concluída).
/// `podium_ids`: apenas P2 e P3 elegíveis (!dnf). `winner_id` deve ser excluído do slice
/// pelo call site (garantia de mutuidade Win/Podium).
/// `top5_ids`: P4–P5 elegíveis (!dnf); `top10_ids`: P6–P10 elegíveis (!dnf). O call site
/// garante que cada piloto aparece em UM papel só — o mais alto que conquistou.
/// `main_incident_pilot_id`: curadoria editorial v1 — apenas o piloto central do incidente
/// narrativamente principal. Escolha de um único piloto é deliberada, não perda acidental.
/// `excluded_driver_id`: excluído por dupla aplicação (já recebe tratamento player-facing),
/// não por ausência do mundo simulado.
/// `category_tier`: degrau da escada (0 Rookie → 6 Endurance); escala todo o ganho por
/// [`crate::fame::fame_category_tier_mult`].
#[derive(Debug, Clone)]
pub struct RaceEventContext<'a> {
    pub winner_id: &'a str,
    pub pole_sitter_id: &'a str,
    pub podium_ids: &'a [&'a str],
    pub top5_ids: &'a [&'a str],
    pub top10_ids: &'a [&'a str],
    pub main_incident_pilot_id: Option<&'a str>,
    pub excluded_driver_id: &'a str,
    pub category_tier: u8,
}

// ── Cálculo de domínio puro ───────────────────────────────────────────────────

/// Quanto o INTERESSE realizado do evento escala o ganho de fama de quem correu bem.
///
/// **É a única modulação de importância que existe.** O bloco player-facing usava
/// `RealizedEventInterest::media_delta_modifier` (0,75–1,60, praticamente sempre ~1,0)
/// para o mesmo papel, sobre o mesmo evento e o mesmo `final_score`. Como as duas curvas
/// só se cruzam em interesse alto, o jogador recebia mais que a IA justamente nas
/// corridas comuns, que são a maioria do calendário:
///
/// | interesse do evento | mundo (aqui) | jogador (antes) | vantagem do jogador |
/// |---|---:|---:|---:|
/// | Baixo (score < 25) | 0,30 | 0,75–0,96 | 2,5–3,2× |
/// | Moderado (25–45) | 0,70 | 0,96–1,13 | 1,4–1,6× |
/// | Alto (45–65) | 1,00 | 1,13–1,30 | 1,1–1,3× |
/// | Muito alto (65–85) | 1,50 | 1,30–1,47 | 0,9–1,0× |
/// | Evento principal (≥85) | 2,50 | 1,47–1,60 | 0,6× |
///
/// `media_delta_modifier` continua no DTO — ele descreve a repercussão do evento e a
/// tela de pós-corrida o mostra —, mas deixou de decidir fama.
pub fn fame_event_interest_mult(tier: &InterestTier) -> f64 {
    match tier {
        InterestTier::Baixo => 0.3,
        InterestTier::Moderado => 0.7,
        InterestTier::Alto => 1.0,
        InterestTier::MuitoAlto => 1.5,
        InterestTier::EventoPrincipal => 2.5,
    }
}

/// Calcula impactos de mídia pública para pilotos AI relevantes de uma corrida.
///
/// Sem `RealizedEventInterest`, este bloco não deve ser chamado — a dependência
/// semântica é explícita: sem importância pública calculada, não há impacto público persistente.
///
/// O `excluded_driver_id` (jogador) é omitido de todos os papéis para evitar dupla aplicação
/// com o pipeline player-facing de media/motivação já existente.
///
/// Base deltas (antes dos multiplicadores de interesse e de categoria):
/// - Win: +3.0
/// - Pole (somente se polesitter ≠ vencedor): +1.5
/// - Podium P2/P3: +1.0
/// - Top5 P4/P5: +0.5
/// - Top10 P6–P10: +0.2
/// - MainIncident: +1.5
/// - Injury: +1.0
///
/// Os papéis de resultado são exclusivos entre si (o call site classifica cada piloto no
/// mais alto que conquistou), mas somam com incidente e lesão. A faixa Top5/Top10 existe
/// porque o decaimento é universal: sem ganho para quem termina bem sem subir no pódio,
/// 23 dos 28 carros de um grid só perdiam fama e a população inteira colapsava no piso.
///
/// Todo o ganho é escalado pelo TIER da categoria — pódio no Endurance vira nome, pódio
/// na Rookie quase não sai do paddock.
pub fn compute_public_media_impacts(
    ctx: &RaceEventContext<'_>,
    injuries: &[Injury],
    realized: &RealizedEventInterest,
) -> Vec<DriverMediaImpact> {
    let mult = fame_event_interest_mult(&realized.final_tier)
        * crate::fame::fame_category_tier_mult(ctx.category_tier);
    let mut accum: HashMap<String, (f64, Vec<MediaImpactReason>)> = HashMap::new();

    // Acumula delta e reason apenas se o reason ainda não foi contabilizado para este piloto.
    // Isso evita dupla contagem quando o mesmo reason ocorre múltiplas vezes (ex.: dois
    // registros de Injury para o mesmo pilot_id): o primeiro conta, os demais são ignorados.
    let mut add = |id: &str, base: f64, reason: MediaImpactReason| {
        if id.is_empty() || id == ctx.excluded_driver_id {
            return;
        }
        let entry = accum.entry(id.to_string()).or_insert((0.0, Vec::new()));
        if !entry.1.contains(&reason) {
            entry.0 += base * mult;
            entry.1.push(reason);
        }
    };

    // Papéis de POSIÇÃO DE CHEGADA — família exclusiva: cada piloto conta no papel mais
    // alto que conquistou, do topo para baixo. A exclusividade é imposta aqui e não
    // delegada ao call site, senão um vencedor listado também no top10 somaria as faixas.
    let mut ja_classificado: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut add_chegada =
        |id: &str, base: f64, reason: MediaImpactReason, vistos: &mut std::collections::HashSet<String>| {
            if id.is_empty() || vistos.contains(id) {
                return;
            }
            vistos.insert(id.to_string());
            add(id, base, reason);
        };

    // Win
    add_chegada(ctx.winner_id, crate::fame::FAME_FINISH_WIN, MediaImpactReason::Win, &mut ja_classificado);

    // Podium P2 e P3
    for &id in ctx.podium_ids {
        add_chegada(id, crate::fame::FAME_FINISH_PODIUM, MediaImpactReason::Podium, &mut ja_classificado);
    }

    // P4–P5: aparece no resumo, não na manchete
    for &id in ctx.top5_ids {
        add_chegada(id, crate::fame::FAME_FINISH_TOP5, MediaImpactReason::Top5, &mut ja_classificado);
    }

    // P6–P10: presença mínima — o suficiente para não sumir do mapa
    for &id in ctx.top10_ids {
        add_chegada(id, crate::fame::FAME_FINISH_TOP10, MediaImpactReason::Top10, &mut ja_classificado);
    }

    // Pole — papel de CLASSIFICAÇÃO, soma com o de chegada (o polesitter que termina em
    // P2 leva os dois). Só não soma com a vitória: aí a vitória já é o ápice do evento.
    if ctx.pole_sitter_id != ctx.winner_id {
        add(ctx.pole_sitter_id, 1.5, MediaImpactReason::Pole);
    }

    // Incidente principal — piloto central, curadoria editorial v1
    if let Some(id) = ctx.main_incident_pilot_id {
        add(id, 1.5, MediaImpactReason::MainIncident);
    }

    // Lesões novas da corrida
    for injury in injuries {
        add(&injury.pilot_id, 1.0, MediaImpactReason::Injury);
    }

    // Converter em Vec ordenado deterministicamente por driver_id
    let mut result: Vec<DriverMediaImpact> = accum
        .into_iter()
        .map(|(driver_id, (delta, reasons))| DriverMediaImpact {
            driver_id,
            delta,
            reasons,
        })
        .collect();
    result.sort_by(|a, b| a.driver_id.cmp(&b.driver_id));
    result
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_interest::models::{HeadlineStrength, InterestTier, RealizedEventInterest};
    use crate::models::enums::InjuryType;
    use crate::models::injury::Injury;

    fn make_realized(tier: InterestTier) -> RealizedEventInterest {
        RealizedEventInterest {
            expected_display_value: 0,
            expected_tier: InterestTier::Baixo,
            final_score: 0.0,
            final_display_value: 0,
            final_tier: tier,
            delta_vs_expected: 0.0,
            media_delta_modifier: 1.0,
            motivation_delta_modifier: 1.0,
            news_importance_bias: 0,
            headline_strength: HeadlineStrength::Normal,
        }
    }

    fn make_injury(pilot_id: &str) -> Injury {
        Injury {
            id: format!("INJ_{pilot_id}"),
            pilot_id: pilot_id.to_string(),
            injury_type: InjuryType::Leve,
            injury_name: "Dor no braço".to_string(),
            modifier: 0.95,
            races_total: 2,
            races_remaining: 2,
            skill_penalty: 0.05,
            season: 1,
            race_occurred: "R01".to_string(),
            active: true,
        }
    }

    /// Tier de referência dos fixtures (gt3). Fixo para os deltas esperados serem
    /// aritmética visível em vez de número mágico.
    const TIER_REF: u8 = 4;

    /// Multiplicador total aplicado a um delta-base nos fixtures: interesse × categoria.
    fn mult_ref(tier: &InterestTier) -> f64 {
        fame_event_interest_mult(tier) * crate::fame::fame_category_tier_mult(TIER_REF)
    }

    fn ctx_simple<'a>(
        winner: &'a str,
        pole: &'a str,
        podium: &'a [&'a str],
        incident: Option<&'a str>,
        excluded: &'a str,
    ) -> RaceEventContext<'a> {
        RaceEventContext {
            winner_id: winner,
            pole_sitter_id: pole,
            podium_ids: podium,
            top5_ids: &[],
            top10_ids: &[],
            main_incident_pilot_id: incident,
            excluded_driver_id: excluded,
            category_tier: TIER_REF,
        }
    }

    #[test]
    fn test_win_low_vs_high_interest_different_delta() {
        let ctx = ctx_simple("P001", "P002", &["P003", "P004"], None, "PLAYER");
        let low = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Baixo));
        let high =
            compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::EventoPrincipal));

        let winner_low = low.iter().find(|d| d.driver_id == "P001").unwrap();
        let winner_high = high.iter().find(|d| d.driver_id == "P001").unwrap();
        assert!(winner_high.delta > winner_low.delta);
    }

    #[test]
    fn test_polesitter_winner_only_gets_win() {
        // Mesmo piloto ganhou a pole e a corrida → recebe apenas Win, sem Pole
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], None, "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        let winner = impacts.iter().find(|d| d.driver_id == "P001").unwrap();
        assert!(winner.reasons.contains(&MediaImpactReason::Win));
        assert!(!winner.reasons.contains(&MediaImpactReason::Pole));
    }

    #[test]
    fn test_pole_different_pilot_separate_impact() {
        let ctx = ctx_simple("P001", "P002", &["P003", "P004"], None, "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        let winner = impacts.iter().find(|d| d.driver_id == "P001").unwrap();
        let poler = impacts.iter().find(|d| d.driver_id == "P002").unwrap();
        assert!(winner.reasons.contains(&MediaImpactReason::Win));
        assert!(poler.reasons.contains(&MediaImpactReason::Pole));
        // Pole delta < Win delta ao mesmo tier
        assert!(winner.delta > poler.delta);
    }

    #[test]
    fn test_winner_not_in_podium_role() {
        // call site garante que winner não está em podium_ids, mas verificamos que
        // mesmo que estivesse, Win e Podium não se duplicam incorretamente
        let ctx = ctx_simple("P001", "P099", &["P002", "P003"], None, "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        let winner = impacts.iter().find(|d| d.driver_id == "P001").unwrap();
        assert!(winner.reasons.contains(&MediaImpactReason::Win));
        assert!(!winner.reasons.contains(&MediaImpactReason::Podium));
    }

    #[test]
    fn test_main_incident_pilot_receives_impact() {
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], Some("P005"), "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        let inc_pilot = impacts.iter().find(|d| d.driver_id == "P005").unwrap();
        assert!(inc_pilot.reasons.contains(&MediaImpactReason::MainIncident));
        assert!(inc_pilot.delta > 0.0);
    }

    #[test]
    fn test_injury_generates_impact() {
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], None, "PLAYER");
        let injury = make_injury("P006");
        let impacts =
            compute_public_media_impacts(&ctx, &[injury], &make_realized(InterestTier::Alto));

        let injured = impacts.iter().find(|d| d.driver_id == "P006").unwrap();
        assert!(injured.reasons.contains(&MediaImpactReason::Injury));
        assert!(injured.delta > 0.0);
    }

    #[test]
    fn test_excluded_driver_absent_from_all_roles() {
        // O jogador é o vencedor — não deve aparecer no Vec
        let ctx = ctx_simple(
            "PLAYER",
            "PLAYER",
            &["P002", "P003"],
            Some("PLAYER"),
            "PLAYER",
        );
        let injury = make_injury("PLAYER");
        let impacts =
            compute_public_media_impacts(&ctx, &[injury], &make_realized(InterestTier::Alto));

        assert!(impacts.iter().all(|d| d.driver_id != "PLAYER"));
    }

    #[test]
    fn test_excluded_main_incident_absent() {
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], Some("PLAYER"), "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        assert!(impacts.iter().all(|d| d.driver_id != "PLAYER"));
    }

    #[test]
    fn test_multiple_reasons_preserved() {
        // Mesmo piloto: vence a corrida e está lesionado
        let ctx = ctx_simple("P001", "P099", &["P002", "P003"], None, "PLAYER");
        let injury = make_injury("P001");
        let impacts =
            compute_public_media_impacts(&ctx, &[injury], &make_realized(InterestTier::Alto));

        let pilot = impacts.iter().find(|d| d.driver_id == "P001").unwrap();
        assert!(pilot.reasons.contains(&MediaImpactReason::Win));
        assert!(pilot.reasons.contains(&MediaImpactReason::Injury));
        // Delta acumulado de Win + Injury
        let win_delta = 3.0 * mult_ref(&InterestTier::Alto);
        let inj_delta = 1.0 * mult_ref(&InterestTier::Alto);
        assert!((pilot.delta - (win_delta + inj_delta)).abs() < 1e-9);
    }

    #[test]
    fn test_duplicate_injury_counts_once() {
        // Se injuries contiver dois registros para o mesmo pilot_id,
        // o Injury deve ser contabilizado apenas uma vez (delta e reason).
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], None, "PLAYER");
        let injuries = vec![make_injury("P006"), make_injury("P006")];
        let impacts =
            compute_public_media_impacts(&ctx, &injuries, &make_realized(InterestTier::Alto));

        let injured = impacts.iter().find(|d| d.driver_id == "P006").unwrap();
        let expected_delta = 1.0 * mult_ref(&InterestTier::Alto);
        assert!(
            (injured.delta - expected_delta).abs() < 1e-9,
            "delta duplicado: esperado {expected_delta}, obtido {}",
            injured.delta
        );
        assert_eq!(
            injured
                .reasons
                .iter()
                .filter(|r| **r == MediaImpactReason::Injury)
                .count(),
            1,
            "Injury deve aparecer apenas uma vez em reasons"
        );
    }

    #[test]
    fn test_main_incident_and_injury_same_pilot() {
        // MainIncident e Injury são papéis distintos e independentes.
        // Um piloto que foi o incidente principal E ficou lesionado deve ter ambos
        // os papéis preservados em reasons e os deltas acumulados corretamente.
        let ctx = ctx_simple("P001", "P001", &["P002", "P003"], Some("P006"), "PLAYER");
        let injury = make_injury("P006");
        let impacts =
            compute_public_media_impacts(&ctx, &[injury], &make_realized(InterestTier::Alto));

        let pilot = impacts.iter().find(|d| d.driver_id == "P006").unwrap();
        assert!(pilot.reasons.contains(&MediaImpactReason::MainIncident));
        assert!(pilot.reasons.contains(&MediaImpactReason::Injury));
        // Delta = MainIncident (1.5) + Injury (1.0) × interesse(Alto) × categoria
        let expected_delta = (1.5 + 1.0) * mult_ref(&InterestTier::Alto);
        assert!(
            (pilot.delta - expected_delta).abs() < 1e-9,
            "delta esperado {expected_delta}, obtido {}",
            pilot.delta
        );
    }

    #[test]
    fn test_top5_e_top10_ganham_menos_que_o_podio() {
        // A hierarquia do ganho tem que sobreviver ao alargamento: vitória > pódio >
        // top5 > top10, todos positivos.
        let ctx = RaceEventContext {
            winner_id: "P001",
            pole_sitter_id: "P099",
            podium_ids: &["P002"],
            top5_ids: &["P004"],
            top10_ids: &["P008"],
            main_incident_pilot_id: None,
            excluded_driver_id: "PLAYER",
            category_tier: TIER_REF,
        };
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));
        let delta = |id: &str| impacts.iter().find(|d| d.driver_id == id).unwrap().delta;
        assert!(delta("P001") > delta("P002"));
        assert!(delta("P002") > delta("P004"));
        assert!(delta("P004") > delta("P008"));
        assert!(delta("P008") > 0.0, "top10 ainda constrói alguma coisa");

        let top5 = impacts.iter().find(|d| d.driver_id == "P004").unwrap();
        assert!(top5.reasons.contains(&MediaImpactReason::Top5));
        let top10 = impacts.iter().find(|d| d.driver_id == "P008").unwrap();
        assert!(top10.reasons.contains(&MediaImpactReason::Top10));
    }

    #[test]
    fn test_top5_consistente_supera_o_decaimento() {
        // O critério de projeto do alargamento: quem termina em P4/P5 toda corrida numa
        // categoria de topo SOBE de fama em vez de sangrar até o piso.
        let ctx = RaceEventContext {
            winner_id: "P001",
            pole_sitter_id: "P001",
            podium_ids: &["P002", "P003"],
            top5_ids: &["P004"],
            top10_ids: &[],
            main_incident_pilot_id: None,
            excluded_driver_id: "PLAYER",
            category_tier: 6, // Endurance
        };
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));
        let ganho_bruto = impacts.iter().find(|d| d.driver_id == "P004").unwrap().delta;
        let ganho = crate::fame::apply_carisma_to_fame_delta(ganho_bruto, 50.0);
        // Contra o decaimento de um piloto já bem acima do piso pessoal de estreante.
        let fama = 45.0;
        let perda = fama
            - crate::fame::decay_fame_toward(
                fama,
                crate::fame::FAME_DECAY_FLOOR,
                crate::fame::FAME_DECAY_BASE_RATE,
                50.0,
            );
        assert!(ganho > perda, "ganho={ganho}, perda={perda}");
    }

    #[test]
    fn test_categoria_de_topo_rende_mais_fama_que_a_base() {
        // Mesma vitória, mesmo interesse: Endurance constrói nome, Rookie quase não.
        let mk = |tier: u8| RaceEventContext {
            winner_id: "P001",
            pole_sitter_id: "P002",
            podium_ids: &[],
            top5_ids: &[],
            top10_ids: &[],
            main_incident_pilot_id: None,
            excluded_driver_id: "PLAYER",
            category_tier: tier,
        };
        let rookie =
            compute_public_media_impacts(&mk(0), &[], &make_realized(InterestTier::Alto));
        let endurance =
            compute_public_media_impacts(&mk(6), &[], &make_realized(InterestTier::Alto));
        let d = |v: &[DriverMediaImpact]| v.iter().find(|x| x.driver_id == "P001").unwrap().delta;
        assert!(d(&endurance) > d(&rookie) * 1.4, "a pirâmide tem inclinação");
    }

    #[test]
    fn test_papel_superior_ganha_do_inferior_no_mesmo_piloto() {
        // Se o call site errar e mandar o mesmo piloto em dois papéis de resultado, o
        // primeiro processado (o mais alto) é o que conta — sem soma de faixas.
        let ctx = RaceEventContext {
            winner_id: "P001",
            pole_sitter_id: "P001",
            podium_ids: &["P002"],
            top5_ids: &["P002"],
            top10_ids: &["P002"],
            main_incident_pilot_id: None,
            excluded_driver_id: "PLAYER",
            category_tier: TIER_REF,
        };
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));
        let p2 = impacts.iter().find(|d| d.driver_id == "P002").unwrap();
        let esperado = (1.0 + 0.5 + 0.2) * mult_ref(&InterestTier::Alto);
        assert!(
            p2.delta < esperado,
            "não é para somar as três faixas: {}",
            p2.delta
        );
    }

    #[test]
    fn test_only_eligible_roles_impacted() {
        // Fixture controlado: 4 pilotos com papéis conhecidos, nenhum extra
        let ctx = ctx_simple("P001", "P002", &["P003", "P004"], Some("P005"), "PLAYER");
        let impacts = compute_public_media_impacts(&ctx, &[], &make_realized(InterestTier::Alto));

        let ids: Vec<&str> = impacts.iter().map(|d| d.driver_id.as_str()).collect();
        assert!(ids.contains(&"P001")); // Win
        assert!(ids.contains(&"P002")); // Pole
        assert!(ids.contains(&"P003")); // Podium P2
        assert!(ids.contains(&"P004")); // Podium P3
        assert!(ids.contains(&"P005")); // MainIncident
                                        // Nenhum piloto fora dos papéis definidos
        assert_eq!(impacts.len(), 5);
    }
}
