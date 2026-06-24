//! O "cérebro" do pós-corrida: traduz um resultado bruto em SIGNIFICADO de
//! carreira — "você não terminou só em P8; terminou em P8 considerando sua
//! largada, seu carro, seu nível e seus rivais".
//!
//! Duas expectativas (decisão do usuário):
//! - **Potencial do conjunto:** onde você DEVERIA estar pelo mérito (carro +
//!   skill + forma + pista) vs a força do grid. Responde "leitura de carreira".
//! - **Meta da corrida:** depois da largada, uma meta justa. Mistura
//!   `0.60×largada + 0.40×potencial`. É o que a TELA PRINCIPAL compara.
//!
//! Posições são RANKING (menor = melhor). Trabalhamos com FAIXAS, não posição
//! exata, para a avaliação ser humana e não "burra" por 1 posição de diferença.
//!
//! Lógica pura e testável; o chamador injeta o mérito de cada piloto (do banco).

use serde::{Deserialize, Serialize};

/// Mérito de um piloto no grid — quanto maior, melhor o "conjunto" dele.
#[derive(Debug, Clone)]
pub struct DriverMerit {
    pub pilot_id: String,
    pub merit: f64,
}

/// Combina os fatores de mérito num score. `skill` e `car` são os pilares; forma
/// recente e familiaridade com a pista são ajustes menores. Calibrável.
/// `recent_avg_finish`: média das últimas chegadas (menor = melhor); `field_size`
/// normaliza essa média. `track_races`: quantas corridas o piloto já fez na pista.
pub fn compute_merit(
    skill: f64,
    car_norm: f64,
    recent_avg_finish: Option<f64>,
    field_size: i32,
    track_races: i32,
) -> f64 {
    // Pilares: skill e carro pesam igual (carro decide muito em corrida).
    let mut m = 0.5 * skill + 0.5 * car_norm;
    // Forma recente: chegar bem ultimamente sobe um pouco o mérito. Converte a
    // média de chegada para 0–100 (1º = 100, último = 0) e mistura leve (15%).
    if let (Some(avg), true) = (recent_avg_finish, field_size > 1) {
        let form = ((field_size as f64 - avg) / (field_size as f64 - 1.0) * 100.0).clamp(0.0, 100.0);
        m = 0.85 * m + 0.15 * form;
    }
    // Pista conhecida: pequeno bônus por experiência no circuito (satura rápido).
    m += (track_races.min(5) as f64) * 0.6;
    m
}

#[derive(Debug, Clone)]
pub struct RaceEvalInput {
    pub player_id: String,
    pub grid_position: i32,
    pub finish_position: i32,
    pub is_dnf: bool,
    /// Incidentes do jogador na corrida (off-track/contato).
    pub incidents: i32,
    /// Mérito de TODOS os carros (inclui o jogador) — para rankear o potencial.
    pub field: Vec<DriverMerit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Assessment {
    MuitoAcima,
    Acima,
    Dentro,
    Abaixo,
    MuitoAbaixo,
}

impl Assessment {
    pub fn label(self) -> &'static str {
        match self {
            Assessment::MuitoAcima => "muito acima do esperado",
            Assessment::Acima => "acima do esperado",
            Assessment::Dentro => "dentro do esperado",
            Assessment::Abaixo => "abaixo do esperado",
            Assessment::MuitoAbaixo => "muito abaixo do esperado",
        }
    }
    /// Componente da nota (0–10) que esta avaliação vale. Extremos suavizados
    /// (decisão de calibração): "muito acima" não é 10 perfeito, "abaixo"/"muito
    /// abaixo" não punem tão fundo.
    fn grade_points(self) -> f64 {
        match self {
            Assessment::MuitoAcima => 9.5,
            Assessment::Acima => 8.0,
            Assessment::Dentro => 6.0,
            Assessment::Abaixo => 4.0,
            Assessment::MuitoAbaixo => 3.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceEvaluation {
    /// Faixa do POTENCIAL do conjunto (leitura de carreira).
    pub potential_low: i32,
    pub potential_high: i32,
    /// Faixa da META da corrida (o que a tela principal compara).
    pub target_low: i32,
    pub target_high: i32,
    pub assessment: Assessment,
    /// Nota da corrida, 0–10 (uma casa).
    pub grade: f64,
    /// Frase principal (manchete) da corrida.
    pub headline: String,
    /// Leitura da equipe/conjunto (baseada no POTENCIAL).
    pub team_read: String,
}

fn clampi(v: i32, lo: i32, hi: i32) -> i32 {
    v.max(lo).min(hi)
}

/// Quão BOA foi a posição final em termos absolutos (0–10), NORMALIZADA pelo
/// tamanho do grid e ponderada pela ZONA DE PONTOS (top ~10). Assim P12 num grid
/// de 30 (bateu metade) vale mais que P12 num grid de 14 (quase último). DNF ≈ 0.
fn absolute_points(finish: i32, field_size: i32, is_dnf: bool) -> f64 {
    if is_dnf || field_size <= 1 {
        return 0.0;
    }
    let cutoff = 10.min(field_size); // zona de pontos
    // Posição relativa no campo: 1º = 1.0, último = 0.0.
    let field_norm = (field_size - finish) as f64 / (field_size - 1) as f64;
    // Profundidade nos pontos: 1º = 1.0, último ponto ≈ 0.1, fora dos pontos = 0.
    let points_norm = if finish <= cutoff {
        (cutoff - finish + 1) as f64 / cutoff as f64
    } else {
        0.0
    };
    (10.0 * (0.6 * field_norm + 0.4 * points_norm)).clamp(0.0, 10.0)
}

/// Avalia o resultado: potencial, meta, avaliação, nota e frases.
pub fn evaluate(input: &RaceEvalInput) -> RaceEvaluation {
    let field_size = input.field.len().max(1) as i32;

    // 1) POTENCIAL = rank do jogador por mérito (1 = melhor conjunto do grid).
    let player_merit = input
        .field
        .iter()
        .find(|d| d.pilot_id == input.player_id)
        .map(|d| d.merit)
        .unwrap_or(0.0);
    let better = input
        .field
        .iter()
        .filter(|d| d.pilot_id != input.player_id && d.merit > player_merit)
        .count() as i32;
    let potential = better + 1;
    let potential_low = clampi(potential - 1, 1, field_size);
    let potential_high = clampi(potential + 1, 1, field_size);

    // 2) META da corrida = 60% largada + 40% potencial (posição é ranking).
    let expected = 0.60 * input.grid_position as f64 + 0.40 * potential as f64;
    let target_low = clampi(expected.floor() as i32 - 1, 1, field_size);
    let target_high = clampi(expected.ceil() as i32 + 1, 1, field_size);

    // 3) AVALIAÇÃO relativa à faixa da meta. Ficar atrás da meta SÓ POR POSIÇÃO é
    // "abaixo"; "muito abaixo" exige AGRAVANTE (DNF, muitos incidentes, ou
    // terminar no fundo REAL do grid). Assim a tela não pune duro por 1-2 posições.
    let real_back = (input.finish_position as f64) > field_size as f64 * 0.85;
    let aggravante = input.incidents >= 3 || real_back;
    let assessment = if input.is_dnf {
        Assessment::MuitoAbaixo
    } else {
        let f = input.finish_position;
        if f <= target_low - 4 {
            Assessment::MuitoAcima
        } else if f <= target_low - 1 {
            Assessment::Acima
        } else if f <= target_high {
            Assessment::Dentro
        } else if aggravante {
            Assessment::MuitoAbaixo
        } else {
            Assessment::Abaixo
        }
    };

    // 4) NOTA (Fase 1): nota BASE = 52% expectativa + 30% grid→chegada +
    //    18% incidentes/DNF. A POSIÇÃO ABSOLUTA é um TETO (não um peso): impede
    //    a nota máxima quando você termina mal no campo, mas NÃO derruba uma
    //    recuperação limpa para o meio. (Decisão do usuário.)
    let assess_pts = assessment.grade_points();
    let gained = (input.grid_position - input.finish_position) as f64; // + = ganhou
    let grid_pts = (5.0 + gained * 0.5).clamp(0.0, 10.0);
    let incident_pts = if input.is_dnf {
        2.0
    } else {
        (10.0 - input.incidents as f64 * 2.0).clamp(0.0, 10.0)
    };
    let base = 0.52 * assess_pts + 0.30 * grid_pts + 0.18 * incident_pts;
    // Teto pela posição absoluta: pior do campo ≈ 7.0; vencedor = 10.
    let abs_pts = absolute_points(input.finish_position, field_size, input.is_dnf);
    let abs_cap = 7.0 + 0.3 * abs_pts;
    let grade = base.min(abs_cap).clamp(0.0, 10.0);
    let grade = (grade * 10.0).round() / 10.0;

    // 5) Frases.
    let headline = build_headline(input, assessment, gained as i32);
    let team_read = build_team_read(input, potential);

    RaceEvaluation {
        potential_low,
        potential_high,
        target_low,
        target_high,
        assessment,
        grade,
        headline,
        team_read,
    }
}

fn build_headline(input: &RaceEvalInput, a: Assessment, gained: i32) -> String {
    if input.is_dnf {
        return format!(
            "Abandono em {}. Fim de semana para esquecer — o importante é trazer o carro inteiro na próxima.",
            ord(input.finish_position)
        );
    }
    let recovery = gained >= 4;
    match a {
        Assessment::MuitoAcima | Assessment::Acima if recovery => format!(
            "Corrida forte de recuperação: largou {} e terminou {}, ganhando {gained} posições — {}.",
            ord(input.grid_position),
            ord(input.finish_position),
            a.label()
        ),
        Assessment::MuitoAcima | Assessment::Acima => format!(
            "Resultado sólido {}: {} → {}, melhor do que a meta da corrida pedia.",
            a.label(),
            ord(input.grid_position),
            ord(input.finish_position)
        ),
        Assessment::Dentro => format!(
            "Corrida dentro do esperado: {} → {}, entregando a meta realista do dia.",
            ord(input.grid_position),
            ord(input.finish_position)
        ),
        Assessment::Abaixo | Assessment::MuitoAbaixo if gained < 0 => format!(
            "Resultado {}: largou {} e perdeu posições até {}. Havia mais ali.",
            a.label(),
            ord(input.grid_position),
            ord(input.finish_position)
        ),
        Assessment::Abaixo | Assessment::MuitoAbaixo => format!(
            "Resultado {}: {} → {} ficou aquém da meta da corrida.",
            a.label(),
            ord(input.grid_position),
            ord(input.finish_position)
        ),
    }
}

/// Leitura QUALITATIVA do conjunto — compara a chegada com o potencial (que é
/// interno, NÃO revelado ao jogador): só diz se entregou acima/no limite/abaixo
/// do que o carro prometia, sem citar a posição-potencial.
fn build_team_read(input: &RaceEvalInput, potential: i32) -> String {
    if input.is_dnf {
        return "O abandono mascarou o que o conjunto tinha a oferecer neste fim de semana."
            .to_string();
    }
    let f = input.finish_position;
    if f <= potential - 2 {
        "Você entregou ACIMA do que o carro prometia — a pilotagem fez a diferença.".to_string()
    } else if f <= potential + 1 {
        "Você entregou perto do limite do conjunto — extraiu o que o carro tinha.".to_string()
    } else {
        "O conjunto tinha mais a oferecer; sobrou ritmo na garagem que não virou posição."
            .to_string()
    }
}

/// "P8" para uma posição.
fn ord(p: i32) -> String {
    format!("P{p}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monta um grid de `n` carros e dá ao jogador o mérito que o coloca no rank
    /// `player_potential` (1-based). Méritos decrescentes 100, 99, …
    fn field_with_player_rank(n: i32, player_potential: i32) -> Vec<DriverMerit> {
        (1..=n)
            .map(|i| DriverMerit {
                pilot_id: if i == player_potential {
                    "P".to_string()
                } else {
                    format!("ai-{i}")
                },
                merit: 100.0 - i as f64, // i=1 melhor; rank = i
            })
            .collect()
    }

    #[test]
    fn recuperacao_acima_do_esperado() {
        // Largou P14, potencial P8 → meta ~P10-P13, terminou P8 = acima.
        let input = RaceEvalInput {
            player_id: "P".into(),
            grid_position: 14,
            finish_position: 8,
            is_dnf: false,
            incidents: 0,
            field: field_with_player_rank(14, 8),
        };
        let e = evaluate(&input);
        assert_eq!((e.target_low, e.target_high), (10, 13));
        assert_eq!(e.assessment, Assessment::Acima);
        assert!(e.grade >= 7.5, "recuperação forte = nota alta, foi {}", e.grade);
        assert!(e.headline.contains("recuperação"));
    }

    #[test]
    fn largou_bem_carro_fraco_termina_acima_do_potencial() {
        // Largou P3, potencial P10 (carro fraco), terminou P5.
        let input = RaceEvalInput {
            player_id: "P".into(),
            grid_position: 3,
            finish_position: 5,
            is_dnf: false,
            incidents: 0,
            field: field_with_player_rank(12, 10),
        };
        let e = evaluate(&input);
        // expected = 0.6*3 + 0.4*10 = 5.8 → meta ~P4-P7; P5 dentro.
        assert_eq!(e.assessment, Assessment::Dentro);
        // Terminou MUITO acima do potencial (P10) → leitura de superação.
        assert!(e.team_read.contains("ACIMA do que o carro prometia"));
    }

    #[test]
    fn carro_bom_largou_bem_termina_abaixo() {
        // Potencial P3, largou P5, terminou P11 → abaixo.
        let input = RaceEvalInput {
            player_id: "P".into(),
            grid_position: 5,
            finish_position: 11,
            is_dnf: false,
            incidents: 1,
            field: field_with_player_rank(14, 3),
        };
        let e = evaluate(&input);
        // expected = 0.6*5 + 0.4*3 = 4.2 → meta ~P3-P6; P11 muito abaixo.
        assert!(matches!(
            e.assessment,
            Assessment::Abaixo | Assessment::MuitoAbaixo
        ));
        assert!(e.grade < 5.0, "abaixo do esperado = nota baixa, foi {}", e.grade);
        assert!(e.team_read.contains("O conjunto tinha mais a oferecer"));
    }

    #[test]
    #[ignore = "só para inspecionar a calibração manualmente"]
    fn imprime_exemplos() {
        let casos = [
            ("Recuperação", 14, 8, false, 0, 14, 8),
            ("Subdesempenho", 5, 11, false, 1, 14, 3),
            ("Recuperação fora dos pontos", 22, 12, false, 0, 22, 10),
        ];
        for (nome, grid, finish, dnf, inc, n, pot) in casos {
            let e = evaluate(&RaceEvalInput {
                player_id: "P".into(),
                grid_position: grid,
                finish_position: finish,
                is_dnf: dnf,
                incidents: inc,
                field: field_with_player_rank(n, pot),
            });
            println!(
                "\n[{nome}] P{grid}->P{finish} | Potencial P{}-P{} | Meta P{}-P{} | {} | Nota {:.1}\n  {}\n  {}",
                e.potential_low, e.potential_high, e.target_low, e.target_high,
                e.assessment.label(), e.grade, e.headline, e.team_read
            );
        }
    }

    #[test]
    fn dnf_e_sempre_muito_abaixo() {
        let input = RaceEvalInput {
            player_id: "P".into(),
            grid_position: 2,
            finish_position: 12,
            is_dnf: true,
            incidents: 1,
            field: field_with_player_rank(12, 2),
        };
        let e = evaluate(&input);
        assert_eq!(e.assessment, Assessment::MuitoAbaixo);
        assert!(e.headline.to_lowercase().contains("abandono"));
        assert!(e.grade < 4.0);
    }
}
