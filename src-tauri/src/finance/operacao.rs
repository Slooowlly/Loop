//! Fatura de OPERAÇÃO do fim de semana: a decomposição do `event_operations_cost` em
//! linhas de verdade (carro, logística, equipe).
//!
//! Antes deste módulo o custo de operação era uma constante achatada
//! (`base * 0.42 + facilities * base * 0.004`) e a tela do pós-corrida mostrava uma
//! fatura PARALELA, calculada com uma fração própria que não conversava com o dinheiro
//! debitado. Agora existe uma conta só: as linhas daqui somam o `event_operations_cost`,
//! e é esse total que sai do caixa em `apply_round_cashflow`.
//!
//! Os pesos nominais somam ~0.42 sob fatores médios, então a escala financeira herdada
//! continua de pé — o que muda é que a rodada deixa de custar igual toda vez: etapa em
//! casa sai mais barata que etapa intercontinental, corrida encerrada cedo queima menos
//! combustível, pneu castigado cobra mais.

use crate::constants::tracks::get_track;

/// Bloco da fatura ao qual uma linha pertence. Vira agrupamento visual na tela do
/// pós-corrida — token, não texto de UI.
pub const GROUP_CAR: &str = "carro";
pub const GROUP_LOGISTICS: &str = "logistica";
pub const GROUP_CREW: &str = "equipe";

/// Uma linha da fatura de operação da rodada.
#[derive(Debug, Clone)]
pub struct OperationLine {
    /// Token da linha (`gasolina`, `frete`, ...). Resolve o rótulo em `maintenance.<key>`.
    pub key: &'static str,
    /// Bloco ao qual a linha pertence (`carro` / `logistica` / `equipe`).
    pub group: &'static str,
    pub cost: f64,
}

/// Tudo que dimensiona a fatura de uma equipe numa rodada. Montado por
/// [`OperationInputs::for_round`] a partir do time + pista + o que os carros dele
/// fizeram na corrida.
#[derive(Debug, Clone, Copy)]
pub struct OperationInputs {
    /// Custo operacional médio da rodada (escala da categoria / nº de rodadas).
    pub round_operating_base: f64,
    /// Instalações do time (0–100): motorhome/estrutura móvel maior custa mais no evento.
    pub facilities: f64,
    /// Qualidade do pit crew (0–100), proxy do tamanho da operação levada à etapa.
    pub pit_crew_quality: f64,
    /// Modulador de custo da economia global.
    pub cost_modifier: f64,
    /// Distância logística da etapa: casa / mesmo continente / intercontinental.
    pub travel_factor: f64,
    /// Fração da corrida que os carros do time completaram (0..1). Abandono cedo
    /// queima menos combustível.
    pub laps_ratio: f64,
    /// Desgaste final médio dos pneus dos carros do time (0..1).
    pub tire_wear: f64,
}

/// Fator de logística por distância da etapa. Média ~1.0 num calendário regional,
/// então trocar a constante achatada por isto não desloca a escala financeira.
pub const TRAVEL_HOME: f64 = 0.70;
pub const TRAVEL_CONTINENTAL: f64 = 1.00;
pub const TRAVEL_INTERCONTINENTAL: f64 = 1.45;

/// Pesos nominais de cada linha como fração do custo operacional da rodada. Calibrados
/// para que, num time mediano (`facilities` 50) e sob fatores médios (voltas ~0.95,
/// desgaste ~0.65, viagem 1.0, equipe 1.0), a soma dê ~0.62 — exatamente o que a fórmula
/// achatada cobrava (`0.42 + 0.004 × 50`).
const W_GASOLINA: f64 = 0.070;
const W_PNEUS: f64 = 0.073;
const W_PECAS: f64 = 0.041;
const W_FRETE: f64 = 0.125;
const W_VIAGEM: f64 = 0.095;
const W_ESTADIA: f64 = 0.080;
const W_DIARIAS: f64 = 0.045;
const W_ESTRUTURA: f64 = 0.051;
/// Inscrição é a única linha que ignora TODOS os fatores: é a mesma taxa para todo mundo,
/// em qualquer etapa. Serve de âncora — o jogador reconhece o número de corrida em corrida.
const W_INSCRICAO: f64 = 0.045;

/// Instalações do time escalam a operação levada à etapa (frete, comitiva, hospedagem,
/// hospitalidade). É o antigo termo `facilities × base × 0.004`, que sozinho valia quase
/// metade do custo operacional — como linha única ele dominava a fatura, então virou o
/// multiplicador do bloco que ele de fato representa. Calibrado em 1.0 para `facilities` 50,
/// preservando a escala herdada em toda a faixa (0 → 0.49, 100 → 1.51).
fn facilities_factor(facilities: f64) -> f64 {
    let f = facilities.clamp(0.0, 100.0);
    (0.196 + 0.004 * f) / 0.396
}

impl OperationInputs {
    /// Monta os fatores de uma equipe nesta etapa. `laps_ratio`/`tire_wear` vêm do que
    /// os carros DELA fizeram na corrida — vale igual para IA e para o jogador, então
    /// não existe caminho especial.
    pub fn for_round(
        round_operating_base: f64,
        facilities: f64,
        pit_crew_quality: f64,
        cost_modifier: f64,
        team_pais_sede: &str,
        track_id: u32,
        laps_ratio: f64,
        tire_wear: f64,
    ) -> Self {
        Self {
            round_operating_base,
            facilities,
            pit_crew_quality,
            cost_modifier,
            travel_factor: travel_factor(team_pais_sede, track_id),
            laps_ratio: laps_ratio.clamp(0.0, 1.0),
            tire_wear: tire_wear.clamp(0.0, 1.0),
        }
    }

    /// Fator de operação da equipe (0.80–1.20 pelo pit crew) — proxy do tamanho da
    /// comitiva que come e dorme na etapa.
    fn crew_factor(&self) -> f64 {
        0.80 + self.pit_crew_quality.clamp(0.0, 100.0) / 250.0
    }
}

/// Continente do país (formato "🇧🇷 Brasil", com ou sem bandeira). `None` quando o país
/// não está no mapa — nesse caso a etapa é tratada como continental (fator neutro).
fn continent(pais: &str) -> Option<&'static str> {
    const EUROPA: &[&str] = &[
        "Áustria",
        "Austria",
        "Bélgica",
        "Belgica",
        "Alemanha",
        "Espanha",
        "França",
        "Franca",
        "Reino Unido",
        "Hungria",
        "Itália",
        "Italia",
        "Holanda",
        "Noruega",
        "Portugal",
        "Suíça",
        "Suica",
        "Finlândia",
        "Finlandia",
        "Suécia",
        "Suecia",
        "Dinamarca",
        "Polônia",
        "Polonia",
        "Irlanda",
        "República Tcheca",
        "Republica Tcheca",
    ];
    const AMERICA_NORTE: &[&str] = &["EUA", "Canadá", "Canada", "México", "Mexico"];
    const AMERICA_SUL: &[&str] = &["Brasil", "Argentina"];
    const ASIA: &[&str] = &[
        "Japão",
        "Japao",
        "Taiwan",
        "China",
        "Coreia do Sul",
        "Emirados",
        "Bahrein",
        "Catar",
        "Índia",
        "India",
    ];
    const OCEANIA: &[&str] = &["Austrália", "Australia", "Nova Zelândia", "Nova Zelandia"];
    const AFRICA: &[&str] = &["África do Sul", "Africa do Sul"];

    for (nomes, nome_continente) in [
        (OCEANIA, "oceania"),
        (EUROPA, "europa"),
        (AMERICA_NORTE, "america_norte"),
        (AMERICA_SUL, "america_sul"),
        (ASIA, "asia"),
        (AFRICA, "africa"),
    ] {
        if nomes.iter().any(|n| pais.contains(n)) {
            return Some(nome_continente);
        }
    }
    None
}

/// Fator de logística da etapa para uma equipe: correr na porta de casa é barato,
/// atravessar o oceano com carro e comitiva não é. Pista desconhecida ou país fora do
/// mapa → neutro (1.0), nunca penaliza por falta de dado.
pub fn travel_factor(team_pais_sede: &str, track_id: u32) -> f64 {
    let Some(track) = get_track(track_id) else {
        return TRAVEL_CONTINENTAL;
    };
    if pais_igual(team_pais_sede, track.pais) {
        return TRAVEL_HOME;
    }
    match (continent(team_pais_sede), continent(track.pais)) {
        (Some(a), Some(b)) if a != b => TRAVEL_INTERCONTINENTAL,
        _ => TRAVEL_CONTINENTAL,
    }
}

/// Compara dois países tolerando a bandeira emoji na frente (o dado de `teams.rs` e o de
/// `tracks.rs` usam o mesmo formato, mas variantes legadas vêm sem emoji).
fn pais_igual(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphabetic() || c.is_whitespace())
            .collect::<String>()
            .trim()
            .to_lowercase()
    };
    let (a, b) = (norm(a), norm(b));
    !a.is_empty() && a == b
}

/// As linhas da fatura de operação da rodada, já em dinheiro. A soma é o
/// `event_operations_cost` que sai do caixa — não existe outro número.
pub fn compute_operation_lines(input: &OperationInputs) -> Vec<OperationLine> {
    let base = input.round_operating_base.max(0.0) * input.cost_modifier;
    let travel = input.travel_factor;
    let crew = input.crew_factor();
    let infra = facilities_factor(input.facilities);
    // Combustível segue as voltas rodadas; pneu segue o desgaste com que o carro voltou.
    let fuel = 0.55 + 0.45 * input.laps_ratio;
    let tire = 0.60 + 0.55 * input.tire_wear;
    // Estadia sente a viagem, mas de forma amortecida: dormir fora custa parecido, o que
    // muda é quantas noites a logística obriga.
    let stay = 0.55 + 0.45 * travel;

    let bruto = [
        (GROUP_CAR, "gasolina", W_GASOLINA * fuel),
        (GROUP_CAR, "pneus", W_PNEUS * tire),
        (GROUP_CAR, "pecas", W_PECAS),
        (GROUP_LOGISTICS, "frete", W_FRETE * travel * infra),
        (GROUP_LOGISTICS, "viagem", W_VIAGEM * travel * infra),
        (GROUP_LOGISTICS, "estadia", W_ESTADIA * stay * infra),
        (GROUP_LOGISTICS, "inscricao", W_INSCRICAO),
        (GROUP_CREW, "diarias", W_DIARIAS * crew * infra),
        (GROUP_CREW, "estrutura", W_ESTRUTURA * infra),
    ];

    bruto
        .into_iter()
        .map(|(group, key, peso)| OperationLine {
            key,
            group,
            cost: base * peso,
        })
        .filter(|l| l.cost > 0.0)
        .collect()
}

/// Custo de operação da rodada = soma da fatura. É este valor que
/// `calculate_team_round_finance_context` debita.
pub fn compute_operation_cost(input: &OperationInputs) -> f64 {
    compute_operation_lines(input).iter().map(|l| l.cost).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> OperationInputs {
        OperationInputs {
            round_operating_base: 100_000.0,
            facilities: 50.0,
            pit_crew_quality: 50.0,
            cost_modifier: 1.0,
            travel_factor: TRAVEL_CONTINENTAL,
            laps_ratio: 0.95,
            tire_wear: 0.65,
        }
    }

    /// A troca da constante achatada (0.42 + 0.004·facilities) pela fatura itemizada não
    /// pode deslocar a escala financeira: sob fatores médios os dois têm que bater dentro
    /// de uma folga estreita, senão a temporada inteira rebalanceia sem querer.
    #[test]
    fn total_medio_fica_perto_da_formula_achatada() {
        let i = inputs();
        let antigo = i.round_operating_base * 0.42 + i.facilities * i.round_operating_base * 0.004;
        let novo = compute_operation_cost(&i);
        let desvio = (novo - antigo).abs() / antigo;
        assert!(
            desvio < 0.05,
            "fatura itemizada desviou {:.1}% do custo achatado (antigo {antigo:.0}, novo {novo:.0})",
            desvio * 100.0
        );
    }

    /// O ponto do redesenho: nenhuma linha pode concentrar a fatura como gasolina e pneus
    /// concentravam (metade cada). No ponto de calibração e nos extremos de `facilities`,
    /// nenhuma passa de 25% do total. (Etapa intercontinental de um time com estrutura
    /// máxima é a exceção esperada — aí o frete DEVE pesar.)
    #[test]
    fn nenhuma_linha_domina_a_fatura() {
        for facilities in [0.0, 50.0, 100.0] {
            let i = OperationInputs {
                facilities,
                ..inputs()
            };
            let total = compute_operation_cost(&i);
            for linha in compute_operation_lines(&i) {
                let fatia = linha.cost / total;
                assert!(
                    fatia < 0.25,
                    "linha '{}' ficou com {:.0}% da fatura (facilities {facilities})",
                    linha.key,
                    fatia * 100.0
                );
            }
        }
    }

    /// A escala herdada tinha o termo de instalações linear (`0.004 × facilities`). Ele
    /// virou multiplicador do bloco de logística, e a equivalência precisa valer na faixa
    /// inteira — não só no time mediano.
    #[test]
    fn escala_por_instalacoes_bate_com_a_formula_antiga() {
        for facilities in [0.0, 25.0, 50.0, 75.0, 100.0] {
            let i = OperationInputs {
                facilities,
                ..inputs()
            };
            let antigo = i.round_operating_base * (0.42 + 0.004 * facilities);
            let novo = compute_operation_cost(&i);
            let desvio = (novo - antigo).abs() / antigo;
            assert!(
                desvio < 0.05,
                "facilities {facilities}: desvio de {:.1}% (antigo {antigo:.0}, novo {novo:.0})",
                desvio * 100.0
            );
        }
    }

    /// A rodada deixa de custar igual toda vez: casa < continental < intercontinental.
    #[test]
    fn distancia_da_etapa_move_o_custo() {
        let casa = compute_operation_cost(&OperationInputs {
            travel_factor: TRAVEL_HOME,
            ..inputs()
        });
        let cont = compute_operation_cost(&inputs());
        let longe = compute_operation_cost(&OperationInputs {
            travel_factor: TRAVEL_INTERCONTINENTAL,
            ..inputs()
        });
        assert!(
            casa < cont && cont < longe,
            "casa {casa:.0} / cont {cont:.0} / longe {longe:.0}"
        );
    }

    /// Abandonar cedo com pneu novo custa menos combustível e menos borracha.
    #[test]
    fn corrida_curta_queima_menos() {
        let inteira = compute_operation_cost(&OperationInputs {
            laps_ratio: 1.0,
            tire_wear: 0.9,
            ..inputs()
        });
        let abandono = compute_operation_cost(&OperationInputs {
            laps_ratio: 0.1,
            tire_wear: 0.1,
            ..inputs()
        });
        assert!(abandono < inteira);
    }

    /// Etapa no país-sede é "em casa" mesmo com o dado legado sem bandeira, e a Austrália
    /// não pode cair na Europa por conter "Austria" sem acento.
    #[test]
    fn geografia_resolve_casa_e_continente() {
        assert_eq!(continent("🇦🇺 Austrália"), Some("oceania"));
        assert_eq!(continent("Australia"), Some("oceania"));
        assert_eq!(continent("🇦🇹 Áustria"), Some("europa"));
        assert!(pais_igual("🇧🇷 Brasil", "Brasil"));
        assert!(!pais_igual("🇧🇷 Brasil", "🇵🇹 Portugal"));
    }
}
