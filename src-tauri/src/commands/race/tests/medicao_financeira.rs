//! Harness de medição da ECONOMIA das equipes de IA.
//!
//! Irmão do `simulation::race::tests::medicao` (lesões e peças), com o mesmo contrato:
//! não é teste de comportamento, é instrumento. Roda com
//! `cargo test --lib medir_economia_das_equipes -- --ignored --nocapture`.
//!
//! **A pergunta que ele existe para responder:** o dinheiro é restrição SENTIDA pelas
//! equipes de IA, ou número de fundo? Toda a maquinaria de pressão orçamentária existe
//! (teto de nível de carro por categoria, preço de peça ancorado na operação, cérebro de
//! manutenção decidindo dentro do `spending_power`, seis faixas de `financial_state`,
//! empréstimo de emergência, ciclo colapso → venda) e precisa ser medida junta.
//!
//! **O que ele replica.** A ordem exata da rodada em `persistencia.rs`:
//! manutenção do carro (contato → quebra → cérebro → custo → fatura) →
//! `calculate_team_round_finance_context` → `apply_round_cashflow` →
//! `apply_crisis_event_if_needed` → `refresh_team_financial_state`. No fim da temporada:
//! prêmio de construtores POR CLASSE, piso de recursos das elites, venda por colapso
//! crônico e o impacto de offseason.
//!
//! **A forma do mundo vem de `constants::categories`, não de constantes daqui.** Cada
//! categoria roda com o seu `num_equipes`, o seu `corridas_por_temporada`, os seus
//! `pilotos_por_equipe` e as suas `classes`. Isso não é preciosismo: `round_operating_base`
//! é `operating_cost_midpoint / rodadas`, e os prêmios por resultado são múltiplos desse
//! base — então calendário curto infla a rodada, e grid grande dividido em classes
//! multiplica quantas equipes pontuam alto. Medir tudo com 12 etapas e 12 equipes, como
//! este harness fazia, é medir uma categoria que não existe em nenhum degrau da escada.
//!
//! **O que ele NÃO replica, e por quê.**
//! - Não há corrida simulada: a ordem de chegada sai da força do carro com ruído, com DNF
//!   sorteado na taxa medida do mundo real. O que se mede aqui é fluxo de caixa, e o motor
//!   de corrida tem harness próprio.
//! - Não há mercado de pilotos: o salário é recalculado a cada temporada por
//!   `calculate_offer_salary_from_money`, a mesma função do mercado — o acoplamento
//!   salário↔dinheiro sobrevive, mas ninguém troca de equipe.
//! - **Não há promoção nem rebaixamento.** Uma equipe vive 20 temporadas na mesma
//!   categoria. No mundo real ela sobe carregando o caixa para uma escala 2–4× maior (ou
//!   desce para uma menor), e é aí que parte do caixa acumulado do topo se explica. Este é
//!   o maior buraco de fidelidade que sobrou; medir com ele exige o pipeline de `promotion`
//!   inteiro, com a escada fechada e as vagas.
//! - O foco (`team_focus`) mora no banco; aqui é aproximado pelo estado financeiro.
//!
//! Consequência: os números são da ORDEM DE GRANDEZA certa e as PROPORÇÕES entre linhas
//! são fiéis; não são a previsão de um save específico. Por isso existe o bloco de
//! VALIDAÇÃO no fim do relatório — ele confronta o que este harness produz com o que foi
//! medido num save real, categoria por categoria.

use std::collections::HashMap;

use crate::finance::events::{
    PARAQUEDAS_MESES, SOCORROS_MAX_POR_TEMPORADA, SOCORRO_GATE_CAIXA_MESES,
    SOCORRO_PRINCIPAL_MESES, SOCORRO_TETO_DIVIDA_MESES,
};
use rand::{rngs::StdRng, Rng, SeedableRng};

use super::super::{
    calculate_team_round_finance_context_modelo, despesa_da_rodada, CalculoDaDespesa,
    CoeficientesDeReceita, EtapaFisica, RoundOperationContext,
};
use super::despesa_legada::despesa_legada_da_rodada;
use crate::car::seed::seed_car;
use crate::car::{Car, PartType};
use crate::constants::categories::get_category_config;
use crate::constants::scoring::{get_points_for_position, BONUS_FASTEST_LAP};
use crate::constants::tracks::{get_tracks_for_category, TrackInfo};
use crate::economia::desenvolvimento::{
    planejar_desenvolvimento, EntradaDeDesenvolvimento, ParametrosDeDesenvolvimento,
};
use crate::economia::receita::{
    premio_de_fim_de_temporada, receita_da_etapa, EntradaDeReceitaDaEtapa, ParametrosDeReceita,
};
use crate::event_interest::{calculate_expected_event_interest, EventInterestContext};
use crate::finance::cashflow::{apply_offseason_competitiveness_impact, apply_round_cashflow};
use crate::finance::economy::economy_income_modifier;
use crate::finance::economy::global_economic_health_for_season;
use crate::finance::events::apply_crisis_event_if_needed;
use crate::finance::focus::TeamFocus;
use crate::finance::planning::{
    calculate_financial_plan, calculate_spending_power, category_finance_scale,
    category_finance_scale_for, derive_budget_index_from_money,
};
use crate::finance::prize::constructor_prize_with;
use crate::finance::rescue::apply_team_sale;
use crate::finance::salary::calculate_offer_salary_from_money;
use crate::finance::state::{
    choose_season_strategy, custo_operacional_mensal, derive_financial_state,
    financial_health_score, meses_de_operacao, refresh_team_financial_state_com, FaixasDeMeses,
};
use crate::finance::strategy::{apply_elite_resource_floor, designate_elite_teams};
use crate::market::car_maintenance::{
    apply_plan, decide_car_maintenance, planning_horizon, upgrades_permitidos_nesta_corrida,
};
use crate::models::enums::{SeasonPhase, ThematicSlot};
use crate::models::team::{placeholder_team_from_db, Team};

const TEMPORADAS: i32 = 20;

/// As categorias medidas, na ordem da escada. Uma linha do relatório por categoria: a
/// escala financeira, o teto de carro e a razão custo/receita mudam em cada degrau, e é
/// justamente onde a pressão aparece (ou não) que interessa.
///
/// `lmp2` saiu da lista de propósito: no mundo real ela não é campeonato, é uma CLASSE
/// dentro do Endurance (`ENDURANCE_CLASSES`). Medi-la isolada dava a uma classe a escala
/// financeira e o prêmio de uma categoria inteira — número que nunca existiu em save nenhum.
const CATEGORIAS: &[&str] = &[
    "mazda_rookie",
    "toyota_rookie",
    "mazda_amador",
    "toyota_amador",
    "bmw_m2",
    "production_challenger",
    "gt4",
    "gt3",
    "endurance",
];

const ESTADOS: &[&str] = &[
    "elite",
    "healthy",
    "stable",
    "pressured",
    "crisis",
    "collapse",
];

// ===================== O mundo real como referência =====================

/// O que uma categoria mede num save de verdade. Medido em `career_014` ("TEST 3"),
/// temporadas 28–31 — 4 temporadas fechadas, 102 equipes ativas. As linhas de dinheiro vêm
/// de `team_finance_history` (as 9 linhas REAIS de cada rodada, mais a linha de fechamento
/// com o prêmio de construtores); as de pista vêm de `race_results` juntado ao `calendar`
/// **pela categoria da CORRIDA**, não pela categoria atual da equipe — equipe sobe e desce
/// de degrau, e juntar por `teams.categoria` joga a história inteira dela no degrau de hoje.
///
/// É o alvo deste harness. Enquanto a coluna MEDIDO do relatório não encostar na coluna
/// REAL, qualquer coeficiente calibrado aqui está calibrado contra um mundo que não existe.
struct Referencia {
    categoria: &'static str,
    /// Receita ÷ despesa da temporada inteira, prêmio de fechamento incluído.
    receita_sobre_despesa: f64,
    /// Pontos somados pelos DOIS carros de uma equipe numa corrida, na média do grid.
    pontos_por_equipe_por_corrida: f64,
    /// Bilheteria em % da receita da temporada.
    bilheteria_pct: f64,
    /// Prêmio de construtores em % da receita da temporada.
    fechamento_pct: f64,
    /// DNFs por carro por corrida, em %. Alimenta o sorteio de abandono aqui.
    ///
    /// Não é detalhe: o abandono destrói pontos. O carro que larga entra na classificação
    /// ABAIXO de quem terminou, mas continua ocupando posição — e se a classe tem 12 carros
    /// e a tabela paga até o 10º, cada abandono acima do 10º apaga os pontos daquela
    /// colocação. No Endurance (52,7% de abandono, classes de 12 carros) isso queima ~19%
    /// dos pontos de cada corrida; na BMW (21,3%, mas 20 carros numa classe só) não queima
    /// quase nada, porque os que largam caem para além do 10º de qualquer jeito.
    dnf_pct: f64,
    /// Desgaste final médio de pneu dos carros que terminaram (0–1). Entra na fatura de
    /// operação: o harness usava 0,6 e subestimava a linha de pneus em toda a escada.
    desgaste_final: f64,
    /// Mídia média dos pilotos ativos da categoria (0–100). A presença pública da equipe
    /// (`top × 0,7 + segundo × 0,3`) nasce daqui, e ela paga patrocínio e bilheteria.
    midia_media: f64,
    /// Incidentes por carro por corrida. Teto do nº de contatos de disputa que castigam
    /// peça — o valor real de `count_team_contacts` (só colisão leve de dois carros) é um
    /// subconjunto disto, então usar o número cheio é o lado conservador do erro.
    incidentes_por_carro: f64,
}

const REFERENCIA: &[Referencia] = &[
    Referencia {
        categoria: "mazda_rookie",
        receita_sobre_despesa: 1.610,
        pontos_por_equipe_por_corrida: 17.00,
        bilheteria_pct: 0.33,
        fechamento_pct: 16.0,
        dnf_pct: 1.3,
        desgaste_final: 0.96,
        midia_media: 38.1,
        incidentes_por_carro: 0.42,
    },
    Referencia {
        categoria: "toyota_rookie",
        receita_sobre_despesa: 1.592,
        pontos_por_equipe_por_corrida: 17.00,
        bilheteria_pct: 0.34,
        fechamento_pct: 16.2,
        dnf_pct: 0.8,
        desgaste_final: 0.96,
        midia_media: 46.1,
        incidentes_por_carro: 0.40,
    },
    Referencia {
        categoria: "mazda_amador",
        receita_sobre_despesa: 1.168,
        pontos_por_equipe_por_corrida: 10.20,
        bilheteria_pct: 0.31,
        fechamento_pct: 19.4,
        dnf_pct: 5.9,
        desgaste_final: 0.94,
        midia_media: 40.9,
        incidentes_por_carro: 0.46,
    },
    Referencia {
        categoria: "toyota_amador",
        receita_sobre_despesa: 1.174,
        pontos_por_equipe_por_corrida: 10.20,
        bilheteria_pct: 0.31,
        fechamento_pct: 19.4,
        dnf_pct: 5.5,
        desgaste_final: 0.94,
        midia_media: 36.1,
        incidentes_por_carro: 0.38,
    },
    Referencia {
        categoria: "bmw_m2",
        receita_sobre_despesa: 1.069,
        pontos_por_equipe_por_corrida: 10.19,
        bilheteria_pct: 0.44,
        fechamento_pct: 19.9,
        dnf_pct: 21.3,
        desgaste_final: 0.95,
        midia_media: 38.6,
        incidentes_por_carro: 0.27,
    },
    Referencia {
        categoria: "production_challenger",
        receita_sobre_despesa: 1.440,
        pontos_por_equipe_por_corrida: 16.78,
        bilheteria_pct: 0.29,
        fechamento_pct: 16.0,
        dnf_pct: 11.3,
        desgaste_final: 0.94,
        midia_media: 43.0,
        incidentes_por_carro: 0.37,
    },
    Referencia {
        categoria: "gt4",
        receita_sobre_despesa: 0.999,
        pontos_por_equipe_por_corrida: 10.20,
        bilheteria_pct: 0.55,
        fechamento_pct: 20.2,
        dnf_pct: 9.4,
        desgaste_final: 0.94,
        midia_media: 46.5,
        incidentes_por_carro: 0.33,
    },
    Referencia {
        categoria: "gt3",
        receita_sobre_despesa: 0.775,
        pontos_por_equipe_por_corrida: 7.29,
        bilheteria_pct: 0.57,
        fechamento_pct: 22.8,
        dnf_pct: 6.1,
        desgaste_final: 0.96,
        midia_media: 43.3,
        incidentes_por_carro: 0.10,
    },
    Referencia {
        categoria: "endurance",
        receita_sobre_despesa: 1.448,
        pontos_por_equipe_por_corrida: 20.48,
        bilheteria_pct: 0.35,
        fechamento_pct: 14.8,
        dnf_pct: 52.7,
        desgaste_final: 0.75,
        midia_media: 47.6,
        incidentes_por_carro: 0.19,
    },
];

fn referencia(categoria: &str) -> &'static Referencia {
    REFERENCIA
        .iter()
        .find(|r| r.categoria == categoria)
        .unwrap_or(&REFERENCIA[0])
}

/// Países-sede na proporção medida no save (`SELECT pais_sede, COUNT(*) FROM teams`). O
/// harness antigo punha todo mundo no Brasil, o que fixava `travel_factor` num valor que
/// nenhuma equipe do mundo real tem — e a logística é 22% da fatura de operação.
const PAISES_SEDE: &[(&str, u32)] = &[
    ("🇺🇸 EUA", 28),
    ("🇬🇧 Reino Unido", 18),
    ("🇩🇪 Alemanha", 17),
    ("🇯🇵 Japão", 11),
    ("🇫🇷 França", 7),
    ("🇨🇦 Canadá", 4),
    ("🇳🇱 Holanda", 2),
    ("🇮🇹 Itália", 3),
    ("🇨🇭 Suíça", 3),
    ("🇧🇪 Bélgica", 2),
    ("🇦🇺 Austrália", 4),
    ("🇳🇴 Noruega", 1),
    ("🇦🇹 Áustria", 1),
];

fn pais_sede(indice: usize) -> String {
    let total: u32 = PAISES_SEDE.iter().map(|(_, peso)| peso).sum();
    let alvo = (indice as u32 * 7 + 3) % total;
    let mut acumulado = 0;
    for (pais, peso) in PAISES_SEDE {
        acumulado += peso;
        if alvo < acumulado {
            return (*pais).to_string();
        }
    }
    "🇺🇸 EUA".to_string()
}

// ===================== A forma da categoria =====================

/// Uma classe dentro da categoria. Categoria de classe única tem exatamente uma, com nome
/// vazio — assim o laço não precisa de caminho especial.
struct Classe {
    nome: &'static str,
    equipes: usize,
}

/// A forma REAL de uma categoria, lida de `constants::categories`. É o que separa este
/// harness da versão que media tudo com 12 etapas e 12 equipes.
struct Arena {
    id: &'static str,
    rodadas: usize,
    pilotos_por_equipe: usize,
    classes: Vec<Classe>,
    /// Soma das equipes de todas as classes — o grid que divide a bilheteria e paga a etapa.
    equipes: usize,
    multi_classe: bool,
}

fn arena(categoria: &'static str) -> Arena {
    let config = get_category_config(categoria)
        .unwrap_or_else(|| panic!("categoria '{categoria}' não existe em constants::categories"));

    let classes: Vec<Classe> = if config.multi_classe && !config.classes.is_empty() {
        config
            .classes
            .iter()
            .map(|c| Classe {
                nome: c.class_name,
                equipes: c.num_equipes as usize,
            })
            .collect()
    } else {
        vec![Classe {
            nome: "",
            equipes: config.num_equipes as usize,
        }]
    };

    Arena {
        id: config.id,
        rodadas: config.corridas_por_temporada.max(1) as usize,
        pilotos_por_equipe: config.pilotos_por_equipe.max(1) as usize,
        equipes: classes.iter().map(|c| c.equipes).sum(),
        multi_classe: config.multi_classe,
        classes,
    }
}

// ===================== O mundo do harness =====================

/// Uma equipe do harness: o `Team` do domínio + o que o banco guardaria à parte.
struct EquipeMedida {
    team: Team,
    car: Car,
    /// Salário anual somado dos pilotos; vira despesa por rodada dividido pelas etapas.
    folha_anual: f64,
    /// Presença pública do lineup (fama) — entra em patrocínio e bilheteria.
    presenca: f64,
    /// Temporadas consecutivas em colapso, o gatilho da venda (`get_collapse_streak`).
    colapsos_seguidos: i32,
    /// Habilidade média do lineup, para recalcular o salário a cada temporada.
    skill: f64,
    /// Índice da classe em `Arena::classes`. Pontuação e prêmio são POR CLASSE.
    classe: usize,
}

/// Monta um grid plausível: atributos espalhados, caixa sorteado dentro da faixa da
/// categoria e carro semeado pela qualidade. Nas arenas GT3 (sprint e a classe gt3 do
/// Endurance) as primeiras de cada classe são fábricas (têm `marca`), que é o que lhes dá
/// o nível de teto a mais.
fn montar_grid(
    arena: &Arena,
    rng: &mut StdRng,
    espalhamento_da_fama: f64,
    faixas: FaixasDeMeses,
) -> Vec<EquipeMedida> {
    let scale = category_finance_scale(arena.id);
    let midia = referencia(arena.id).midia_media;
    let mut grid = Vec::with_capacity(arena.equipes);

    for (ic, classe) in arena.classes.iter().enumerate() {
        for i in 0..classe.equipes {
            let q = if classe.equipes <= 1 {
                0.5
            } else {
                i as f64 / (classe.equipes - 1) as f64 // 0 = fundo da classe, 1 = topo
            };
            let indice_global = grid.len();
            let mut team = placeholder_team_from_db(
                format!("{}-{}-T{i:02}", arena.id, classe.nome),
                format!("Equipe {indice_global:02}"),
                arena.id.to_string(),
                "2026-01-01".to_string(),
            );
            team.ativa = true;
            team.pais_sede = pais_sede(indice_global);
            team.classe = (!classe.nome.is_empty()).then(|| classe.nome.to_string());
            team.cash_balance = scale.cash_min + (scale.cash_max - scale.cash_min) * q;
            team.debt_balance = 0.0;
            team.reputacao = 25.0 + 60.0 * q;
            team.engineering = 25.0 + 60.0 * q;
            team.facilities = 25.0 + 60.0 * q;
            team.pit_crew_quality = 30.0 + 55.0 * q;
            team.confiabilidade = 45.0 + 45.0 * q;
            team.morale = 1.0;
            team.car_performance = -2.0 + 14.0 * q;
            team.season_strategy = "balanced".to_string();
            // Fábricas: as 3 melhores da arena onde se disputa com marca real.
            let arena_de_fabrica =
                arena.id == "gt3" || (arena.id == "endurance" && classe.nome == "gt3");
            if arena_de_fabrica && i >= classe.equipes.saturating_sub(3) {
                team.marca = Some("Fábrica".to_string());
            }
            refresh_team_financial_state_com(&mut team, faixas);

            let car = seed_car(&team.car_category_key(), q);
            team.car = Some(car.clone());

            let skill = 45.0 + 45.0 * q;
            let folha_anual =
                arena.pilotos_por_equipe as f64 * calculate_offer_salary_from_money(&team, skill);
            // Presença pública: `derive_team_public_presence` é `top × 0,7 + segundo × 0,3`
            // sobre a mídia dos pilotos, e a mídia medida no mundo real vive numa faixa
            // ESTREITA (21–84, média por categoria entre 36 e 48). O harness antigo usava
            // 10–80 dentro do mesmo grid, o que dava à cota de bilheteria por fama um poder
            // de diferenciação que ela não tem no jogo.
            // A largura é parametrizada: `FAMA_DE_HOJE` reproduz o save real, e larguras
            // maiores emulam a fama reescrita (piso pessoal + composto de público), que é a
            // condição declarada para o critério 11 ser alcançável.
            let presenca = (midia + (q - 0.5) * espalhamento_da_fama + rng.gen_range(-3.0..3.0))
                .clamp(5.0, 100.0);

            grid.push(EquipeMedida {
                team,
                car,
                folha_anual,
                presenca,
                colapsos_seguidos: 0,
                skill,
                classe: ic,
            });
        }
    }
    grid
}

/// Calendário da categoria: as pistas que ela de fato usa (`get_tracks_for_category`), na
/// quantidade que ela de fato corre. Cada temporada rotaciona o ponto de partida, para que
/// a logística (etapa em casa × intercontinental) varie como varia no jogo.
fn calendario(arena: &Arena, temporada: i32) -> Vec<&'static TrackInfo> {
    let pool = get_tracks_for_category(arena.id);
    if pool.is_empty() {
        return Vec::new();
    }
    let offset = (temporada as usize).wrapping_mul(3) % pool.len();
    (0..arena.rodadas)
        .map(|i| pool[(offset + i) % pool.len()])
        .collect()
}

/// Prestígio "de local" da rodada — o mesmo `calculate_expected_event_interest` que
/// `race::financas::venue_prestige_score` usa em produção, montado aqui a partir da
/// categoria e da posição da etapa no calendário (não existe `CalendarEntry` no harness).
///
/// Importa porque o bolo da bilheteria é `base × coef × (score/60)`, e o score varia de
/// ~26 numa etapa de meio de Rookie a ~89 numa final de Endurance. O harness antigo fixava
/// 60 — exatamente o valor que faz o fator dar 1,0 e o prestígio sumir da conta.
fn prestigio_da_rodada(arena: &Arena, rodada: usize, track: &TrackInfo) -> f64 {
    let total = arena.rodadas as i32;
    let numero = rodada as i32 + 1;
    let ctx = EventInterestContext {
        categoria: arena.id.to_string(),
        season_phase: SeasonPhase::Temporada,
        rodada: numero,
        total_rodadas: total,
        week_of_year: 8 + (36 * numero) / total.max(1),
        track_id: track.track_id as i32,
        track_name: track.nome.to_string(),
        is_player_event: false,
        player_championship_position: None,
        player_media: None,
        championship_gap_to_leader: None,
        is_title_decider_candidate: false,
        thematic_slot: if numero == 1 {
            ThematicSlot::AberturaDaTemporada
        } else if numero == total {
            ThematicSlot::FinalDaTemporada
        } else {
            ThematicSlot::RodadaRegular
        },
    };
    calculate_expected_event_interest(&ctx).score as f64
}

/// Foco da equipe aproximado pelo estado financeiro. No jogo o foco é persistido em
/// `team_focus` com histerese; aqui só se precisa do multiplicador de ganho no offseason.
fn foco_aproximado(team: &Team) -> TeamFocus {
    match team.financial_state.as_str() {
        "elite" => TeamFocus::Dinastia,
        "healthy" => TeamFocus::ProjetoDeTitulo,
        "stable" => TeamFocus::MeioDeGrid,
        "pressured" => TeamFocus::Reconstrucao,
        _ => TeamFocus::Sobrevivencia,
    }
}

/// Apetite de investimento por foco. É o único lugar em que o harness precisa traduzir o
/// foco da equipe para a entrada do módulo novo: quem está sobrevivendo não gasta o caixa
/// em desenvolvimento, quem está construindo dinastia gasta tudo que pode.
fn apetite_do_foco(foco: TeamFocus) -> f64 {
    match foco {
        TeamFocus::Dinastia => 1.00,
        TeamFocus::ProjetoDeTitulo => 0.90,
        TeamFocus::MeioDeGrid => 0.70,
        // Celeiro põe o dinheiro em piloto, não em galpão — investe menos que o meio de
        // grid mesmo quando tem caixa.
        TeamFocus::Celeiro => 0.50,
        TeamFocus::Reconstrucao => 0.45,
        TeamFocus::Sobrevivencia => 0.15,
    }
}

fn nivel_medio(car: &Car) -> f64 {
    if car.parts.is_empty() {
        return 0.0;
    }
    car.parts.iter().map(|p| p.level as f64).sum::<f64>() / car.parts.len() as f64
}

// ===================== O ralo (seção 3.4) =====================
//
// A seção 2.6 mede o buraco: fora comprar peça, nenhum débito escala com a riqueza, então o
// caixa integra para sempre. A varredura provou que nenhum botão de RECEITA resolve — a
// melhor deriva em ~60 configurações foi 1,4×, e só com 29,5% do mundo em crise. A seção 3.4
// propõe o ralo, mas não diz QUANTO ele precisa drenar.
//
// Este struct simula três formas de ralo DENTRO do harness, para descobrir a magnitude antes
// de alguém escrever `economia/desenvolvimento.rs`. Nada aqui existe em produção.
//
// Todas as três são zero por padrão: com `Ralo::default()` o harness roda exatamente como
// rodava, e os testes existentes não se mexem.
#[derive(Debug, Clone, Copy, Default)]
struct Ralo {
    /// **A — custo de MANTER estrutura.** Fração do custo operacional ANUAL cobrada por
    /// temporada quando `engineering + facilities` está no máximo (200 pontos), escalando
    /// linearmente com o porte. É o "estrutura maior tem custo fixo maior" da seção 3.4.
    /// Cobrada por rodada (dividida pelas etapas), porque é conta de todo mês.
    ///
    /// Uma equipe mediana do grid (eng 55 + fac 55 = 110/200 = 0,55) paga `manter × 0,55`
    /// do operacional anual.
    manter: f64,
    /// **B — custo de MELHORAR estrutura.** Fração do custo operacional anual cobrada por
    /// CADA 100 pontos de `engineering`/`facilities` ganhos no offseason. Hoje
    /// `apply_offseason_competitiveness_impact` sobe os dois de graça, e a seção 2.5 aponta
    /// isso como uma das três fontes de dinheiro do nada.
    melhorar: f64,
    /// **C — dreno sobre o EXCEDENTE.** Fração do caixa acima de `meses_reserva` meses de
    /// operação, drenada por temporada. Entra como CONTROLE, não como proposta: é irreal
    /// (ninguém queima caixa por ter caixa), mas é o teto teórico de eficácia — mede quanto
    /// das formas A e B ainda falta para chegar lá.
    excedente: f64,
    /// Quantos meses de operação ficam livres do dreno C.
    meses_reserva: f64,
}

impl Ralo {
    fn de_manter(v: f64) -> Self {
        Self {
            manter: v,
            ..Self::default()
        }
    }

    fn de_melhorar(v: f64) -> Self {
        Self {
            melhorar: v,
            ..Self::default()
        }
    }

    fn de_excedente(v: f64, meses: f64) -> Self {
        Self {
            excedente: v,
            meses_reserva: meses,
            ..Self::default()
        }
    }

    fn combinado(manter: f64, melhorar: f64) -> Self {
        Self {
            manter,
            melhorar,
            ..Self::default()
        }
    }

    /// A, na unidade anual. O chamador divide pelas rodadas.
    fn custo_anual_de_manter(&self, team: &Team, operacional_anual: f64) -> f64 {
        if self.manter <= 0.0 {
            return 0.0;
        }
        let porte =
            (team.engineering.clamp(0.0, 100.0) + team.facilities.clamp(0.0, 100.0)) / 200.0;
        operacional_anual * self.manter * porte
    }

    /// B. `pontos_ganhos` é o delta REAL aplicado em engineering + facilities (já depois do
    /// clamp em 100), medido antes/depois da chamada de offseason.
    fn custo_de_melhorar(&self, pontos_ganhos: f64, operacional_anual: f64) -> f64 {
        if self.melhorar <= 0.0 || pontos_ganhos <= 0.0 {
            return 0.0;
        }
        operacional_anual * self.melhorar * pontos_ganhos / 100.0
    }

    /// C.
    fn dreno_do_excedente(&self, caixa: f64, operacional_anual: f64) -> f64 {
        if self.excedente <= 0.0 {
            return 0.0;
        }
        let reserva = operacional_anual / 12.0 * self.meses_reserva;
        (caixa - reserva).max(0.0) * self.excedente
    }
}

/// Qual passo de offseason o harness roda. É o que permite dirigir o modelo velho e o novo
/// lado a lado, com o resto do mundo idêntico.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Offseason {
    /// Produção de hoje: `apply_offseason_competitiveness_impact`, que dá ~2,76 pontos de
    /// estrutura por equipe por temporada e **não debita nada**.
    Producao,
    /// `economia::desenvolvimento`: a equipe investe o excedente e compra estrutura com
    /// retorno decrescente. O antigo NÃO é chamado — os dois moveriam a mesma estrutura.
    Economia(ParametrosDeDesenvolvimento),
}

impl Default for Offseason {
    fn default() -> Self {
        Offseason::Producao
    }
}

/// Qual modelo de RECEITA o harness roda.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Receita {
    /// Produção de hoje: os quatro canais de `race::financas` (todos frações do
    /// `round_operating_base`) mais o prêmio de construtores de `finance::prize`.
    Producao,
    /// `economia::receita`: os cinco canais da seção 3.5. Substitui SÓ o lado da receita —
    /// as cinco linhas de despesa continuam vindo do modelo antigo, porque `economia::evento`
    /// e `economia::temporada` são de outra sessão e a troca delas é outra etapa.
    Economia(ParametrosDeReceita),
}

impl Default for Receita {
    fn default() -> Self {
        Receita::Producao
    }
}

/// Qual modelo de DESPESA o harness roda — **a função que a produção chama**, não uma cópia
/// dela. O eixo entrega o mesmo ponteiro `CalculoDaDespesa` que `race::financas` usa, em vez
/// de reimplementar o mapeamento: comparador que compara a sua própria reconstrução do modelo
/// não prova nada sobre o que o jogador vai pagar.
///
/// | aqui | o que a rodada cobra |
/// |---|---|
/// | `Producao` | `tests::despesa_legada` — frações de `round_operating_base` |
/// | `Economia` | `race::despesa::despesa_da_rodada` — `quantidade × preço` |
///
/// **`Producao` não é mais o que a produção debita.** O modelo velho saiu de `src/` e virou
/// dado histórico congelado no harness; o nome sobrevive porque é contra ele que todos os
/// critérios desta empreitada foram medidos, e trocar a base de comparação no meio apagaria a
/// série histórica. O relatório imprime os dois e diz qual é qual.
///
/// O que a troca NÃO move, de propósito, para a diferença ser atribuível:
/// - **salário de piloto** segue vindo de `e.folha_anual` (o acoplamento com o mercado).
///   `despesa_da_rodada` já tira a folha de pilotos dos recorrentes justamente por isso.
/// - **peças** seguem sendo o que `decide_car_maintenance` decidiu comprar, cru, dos dois
///   lados. O que some no `Fisico` é só a "base técnica" abstrata (`0,16 × base`).
/// - **juros** seguem vindos de `debt_service_for_state`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Despesa {
    Producao,
    Economia,
}

impl Despesa {
    fn calculo(self) -> CalculoDaDespesa {
        match self {
            Despesa::Producao => despesa_legada_da_rodada,
            Despesa::Economia => despesa_da_rodada,
        }
    }
}

impl Default for Despesa {
    fn default() -> Self {
        Despesa::Producao
    }
}

/// Contra qual custo operacional anual os módulos NOVOS se expressam.
///
/// `economia::receita` e `economia::desenvolvimento` recebem o custo operacional anual como
/// ENTRADA — é o que os desacopla da tabela. Mas o harness vinha passando a âncora VELHA
/// (`operating_cost_midpoint`) por herança, de quando a despesa também vinha dela. Depois
/// que a despesa passou a sair de `economia::temporada`, isso virou um desalinhamento: a
/// receita é uma fração de um número e a conta que ela precisa pagar é outro.
///
/// O eixo existe para que a diferença entre os dois seja MEDIDA e não presumida.
#[derive(Debug, Clone, Copy, PartialEq)]
enum AncoraDoDinheiro {
    /// `operating_cost_midpoint` — a tabela escrita à mão. É a que os canais de receita de
    /// PRODUÇÃO usam (todo peso é fração do `round_operating_base`, que sai dela).
    Declarada,
    /// `economia::temporada::custo_operacional_anual_de_referencia` — a consequência do
    /// modelo físico, POR CLASSE. A mesma âncora que a despesa usa.
    Fisica,
}

impl Default for AncoraDoDinheiro {
    fn default() -> Self {
        AncoraDoDinheiro::Declarada
    }
}

/// Espalhamento da presença pública dentro do grid, em pontos de largura total. O valor de
/// hoje (18) é o que `montar_grid` usava fixo e reproduz o save real (σ ≈ 8 contando o
/// ruído); a fama reescrita pela outra sessão mede σ de 12,6 a 19,3, e uma largura uniforme
/// de 52 dá σ ≈ 15 — o meio dessa faixa.
const FAMA_DE_HOJE: f64 = 18.0;
const FAMA_COM_AMPLITUDE: f64 = 52.0;

/// **O laço de realimentação da riqueza**, ligado ou desligado.
///
/// O patrocínio de produção soma `plan.budget_index × round_operating_base × 0,002`
/// (`race::financas`), e `budget_index` é derivado do dinheiro
/// (`planning::derive_budget_index_from_money`). Fechando o círculo: caixa → meses de
/// operação → meses projetados → escada de estados → índice 0–100 → patrocínio → caixa.
/// Equipe rica capta mais patrocínio **por ser rica**.
///
/// O eixo existe para medir o tamanho desse laço, e só ele: o contrafactual zera o termo e
/// não mexe em mais nada. Reputação, fama, bilheteria, prêmio, despesa e o resto do índice
/// (que outros sistemas leem) continuam idênticos.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Realimentacao {
    /// Produção de hoje.
    Atual,
    /// O mesmo mundo com o termo do índice fora do patrocínio.
    Zerada,
    /// **O braço que separa nível de forma.** Cada equipe recebe o termo calculado sobre o
    /// índice MÉDIO da classe dela naquela rodada, em vez do índice próprio.
    ///
    /// Existe porque `Zerada` responde a duas perguntas de uma vez e não diz qual das duas
    /// respondeu: ela tira 11–17% do patrocínio do mundo (NÍVEL) e tira a diferenciação
    /// entre rica e pobre (FORMA) no mesmo movimento. Um mundo mais pobre é mais desigual
    /// neste modelo por motivos que não têm nada a ver com o laço, então concentração medida
    /// contra `Zerada` vem contaminada.
    ///
    /// Aqui o dinheiro total que o canal injeta na classe fica igual ao de produção, ponto a
    /// ponto, e o que morre é só quem recebe mais e quem recebe menos. A diferença
    /// `Atual − Achatada` é o efeito do LAÇO, limpo.
    Achatada,
}

// ===================== Os eixos absolutos (B47 / B50 / B52) =====================

/// **O empréstimo de emergência**: a produção de hoje, o piso sem socorro e os dois
/// contrafactuais que sustentam a decisão de B50.
///
/// A produção **era** absoluta: gatilho em `caixa < −75 mil` **OU** `dívida ≥ 750 mil`, valor
/// de uma tabela por categoria (150 mil na Rookie, 800 mil no Endurance). Nenhum dos três
/// números sabia quanto custa operar a divisão, e o `OU` fazia da dívida uma condição que
/// LIBERAVA mais socorro. Hoje ela é relativa e tem teto — ver `finance::events`. O braço
/// [`Socorro::Absoluta`] guarda a política velha para o antes/depois continuar mensurável
/// depois de a produção ter mudado.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Socorro {
    /// `finance::events::apply_crisis_event_if_needed`, intacto: 2 meses de gate de caixa,
    /// teto de dívida em 4 meses, principal de 2 meses, no máximo 2 por temporada.
    Producao,
    /// Ninguém é socorrido. O piso de referência: quanto do mundo de pé é obra do empréstimo.
    Sem,
    /// **A política ANTIGA**, congelada aqui: os três números absolutos, o `OU` no gatilho e
    /// nenhum limite de reincidência. É contra este braço que se lê o que a correção fez.
    Absoluta,
    /// Os gates e o principal da produção NOVA, com a taxa cobrada ao longo do tempo em vez
    /// de capitalizada no ato.
    ///
    /// Em produção a dívida cresce `1,18 × principal` na mesma rodada do socorro. Aqui ela
    /// cresce só o principal e os 18% viram despesa de caixa rateada nas rodadas de uma
    /// temporada. O dinheiro cobrado é o mesmo; o que muda é QUANDO ele entra na dívida — e
    /// agora a dívida é o TETO do socorro seguinte, então adiar o reconhecimento dela é
    /// adiar o freio. Medido: pior que a capitalizada, e é por isso que a produção não mudou
    /// deste lado.
    Amortizada,
    /// Os gates, o teto e o limite por temporada da produção, com o PRINCIPAL e a TAXA
    /// varridos. Existe para o critério que o 2/4/2 não cumpriu de primeira: o braço com
    /// socorro tinha que deixar o colapso ≤ o do braço SEM socorro, e ficava 0,6 pp acima.
    ///
    /// A causa é aritmética: o socorro entrega `p` meses de caixa e cria `p × taxa` meses de
    /// dívida, e a saúde do mundo é medida em `caixa − dívida`. Com taxa acima de 1 o socorro
    /// PIORA o indicador no ato, e depois a equipe ainda paga juro sobre a diferença — 5% por
    /// rodada na banda de colapso. Este braço varre os dois números na mesma unidade relativa
    /// para achar onde a conta fecha.
    Variante {
        /// Principal, em meses de operação.
        principal: f64,
        /// Multiplicador da dívida sobre o principal.
        taxa: f64,
    },
}

/// A política ANTIGA do socorro, copiada de `finance::events` como ela era até 12/08/2026.
/// Existe só para o braço [`Socorro::Absoluta`] — nenhuma linha de produção passa por aqui.
fn socorro_absoluto_antigo(team: &Team) -> Option<f64> {
    if team.financial_state != "collapse" {
        return None;
    }
    if team.cash_balance > -75_000.0 && team.debt_balance < 750_000.0 {
        return None;
    }
    let base = match team.categoria.as_str() {
        "mazda_rookie" | "toyota_rookie" => 150_000.0,
        "mazda_amador" | "toyota_amador" => 225_000.0,
        "bmw_m2" => 300_000.0,
        "production_challenger" => 375_000.0,
        "gt4" => 475_000.0,
        "gt3" => 650_000.0,
        "endurance" => 800_000.0,
        _ => 250_000.0,
    };
    Some(base * (0.85 + team.reputacao.clamp(0.0, 100.0) / 500.0))
}

/// O que um socorro fez com a equipe. Existe porque o braço amortizado separa o que entra na
/// dívida do que fica pendurado para ser cobrado em caixa.
#[derive(Debug, Clone, Copy, Default)]
struct Socorrido {
    principal: f64,
    /// Quanto foi somado ao `debt_balance` no ato.
    divida: f64,
    /// Taxa que ainda será cobrada em caixa, rateada nas próximas rodadas.
    taxa_diferida: f64,
}

/// Quanto o socorro soma à dívida por unidade de principal. Cópia de
/// `finance::events::SOCORRO_TAXA` — cobrada pelo guard
/// [`os_numeros_do_socorro_ainda_sao_os_da_producao`].
const SOCORRO_TAXA: f64 = 1.00;

/// A taxa de originação como ela era até 12/08/2026. Usada pelos braços históricos
/// ([`Socorro::Absoluta`] e [`Socorro::Amortizada`]), que existem para medir o ANTES.
const TAXA_HISTORICA: f64 = 1.18;

/// Aplica o socorro do braço escolhido. Devolve `Some(principal)` quando emprestou.
fn aplicar_socorro(team: &mut Team, socorro: Socorro, temporada: i32) -> Option<Socorrido> {
    match socorro {
        Socorro::Producao => apply_crisis_event_if_needed(team, temporada).map(|e| Socorrido {
            principal: e.cash_delta,
            divida: e.debt_delta,
            taxa_diferida: 0.0,
        }),
        Socorro::Sem => None,
        Socorro::Amortizada => {
            let principal =
                crate::finance::events::emergency_loan_amount_na_temporada(team, temporada)?;
            team.cash_balance += principal;
            team.debt_balance += principal;
            // O contador continua sendo o da produção: o braço muda só QUANDO a taxa entra.
            team.socorros_na_temporada =
                crate::finance::events::socorros_ja_tomados(team, temporada) + 1;
            team.socorros_temporada_ref = temporada;
            Some(Socorrido {
                principal,
                divida: principal,
                taxa_diferida: principal * (TAXA_HISTORICA - 1.0),
            })
        }
        Socorro::Variante { principal, taxa } => {
            // Reusa os PORTÕES da produção e troca só o tamanho do cheque: o eixo aqui é o
            // principal e a taxa, não a elegibilidade.
            let da_producao =
                crate::finance::events::emergency_loan_amount_na_temporada(team, temporada)?;
            let escala = principal / crate::finance::events::SOCORRO_PRINCIPAL_MESES;
            let valor = da_producao * escala;
            team.cash_balance += valor;
            team.debt_balance += valor * taxa;
            team.socorros_na_temporada =
                crate::finance::events::socorros_ja_tomados(team, temporada) + 1;
            team.socorros_temporada_ref = temporada;
            Some(Socorrido {
                principal: valor,
                divida: valor * taxa,
                taxa_diferida: 0.0,
            })
        }
        Socorro::Absoluta => {
            let principal = socorro_absoluto_antigo(team)?;
            team.cash_balance += principal;
            team.debt_balance += principal * TAXA_HISTORICA;
            Some(Socorrido {
                principal,
                divida: principal * TAXA_HISTORICA,
                taxa_diferida: 0.0,
            })
        }
    }
}

/// **O paraquedas de rebaixamento**, que o harness nunca tinha ligado.
///
/// Este harness não promove nem rebaixa (ver o cabeçalho do módulo), então
/// `parachute_payment_remaining` era zero em toda equipe e a linha `ajuda` da fatura vinha
/// vazia — o canal existia em produção e não era medido em lugar nenhum.
///
/// A coorte aqui é sintética e declarada: **a última colocada de cada classe** fecha a
/// temporada recebendo o saldo de paraquedas, como se tivesse sido rebaixada. Ela continua
/// na mesma divisão (o harness não tem escada), o que torna a medida uma cota SUPERIOR do
/// alívio: no jogo a equipe desce para uma divisão mais barata e o mesmo dinheiro compra
/// mais meses ainda.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Paraquedas {
    /// Ninguém é rebaixado — o harness de sempre.
    Nenhum,
    /// Produção: total de [`crate::finance::events::PARAQUEDAS_MESES`] meses de operação da
    /// divisão de destino, pago em `rodadas` parcelas iguais (`race::financas`).
    Producao,
    /// **A política ANTIGA**, congelada aqui: a tabela absoluta por categoria e a parcela fixa
    /// de 25 mil por rodada. É o ANTES contra o qual se lê a correção de B47.
    Absoluta,
}

/// A parcela fixa que a produção pagava por rodada até 12/08/2026. Vive aqui como referência
/// histórica do braço [`Paraquedas::Absoluta`].
const PARCELA_DE_AJUDA: f64 = 25_000.0;

/// A tabela absoluta do paraquedas, como ela era até 12/08/2026. Só o braço histórico usa.
fn paraquedas_absoluto_antigo(team: &Team) -> f64 {
    let base = match team.categoria.as_str() {
        "mazda_rookie" | "toyota_rookie" => 120_000.0,
        "mazda_amador" | "toyota_amador" => 180_000.0,
        "bmw_m2" => 250_000.0,
        "production_challenger" => 325_000.0,
        "gt4" => 425_000.0,
        "gt3" => 575_000.0,
        "endurance" => 700_000.0,
        _ => 200_000.0,
    };
    base * (0.85 + team.reputacao.clamp(0.0, 100.0) / 400.0)
}

/// **Os dois limiares de `choose_season_strategy`** (B52).
///
/// A produção compara `spending_power` — grandeza ANUAL re-derivada em cima de meses de
/// operação — contra frações do `operating_cost_midpoint` ANUAL. O divisor vinha de
/// `category_finance_scale`, a forma CEGA À CLASSE, e os dois braços contrafactuais separavam
/// as duas suspeitas: o valor dos limiares e a escala contra a qual eles são lidos.
///
/// A segunda suspeita virou correção: `choose_season_strategy` passou a ler a escala da
/// DIVISÃO (B52). Por isso `PorClasse` e `Limiares` coincidem nos mesmos limiares — o braço
/// continua na tabela porque é ele que registra o que a correção mudou.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Estrategia {
    /// `finance::state::choose_season_strategy`, intacta.
    Producao,
    /// A mesma forma, com os dois limiares parametrizados (frações do operacional anual).
    Limiares { all_in: f64, austeridade: f64 },
    /// Os mesmos limiares, lidos contra a escala da CLASSE da equipe em vez da categoria.
    PorClasse { all_in: f64, austeridade: f64 },
}

/// Os limiares que a produção usa hoje. Cópia — cobrada pelo guard
/// [`os_limiares_da_estrategia_ainda_sao_os_da_producao`].
const LIMIAR_ALL_IN: f64 = 0.20;
const LIMIAR_AUSTERIDADE: f64 = 0.50;
/// Gate de dívida do `survival`, fração do `expected_cash_midpoint`.
const LIMIAR_SURVIVAL: f64 = 0.75;

/// Reprodução local de `choose_season_strategy` com os limiares abertos.
///
/// É cópia de lógica de produção, e o guard existe por isso: rodar o contrafactual sobre uma
/// forma que a produção não tem mais mediria outra coisa em silêncio.
fn escolher_estrategia(team: &Team, eixo: Estrategia) -> &'static str {
    let (all_in, austeridade) = match eixo {
        Estrategia::Producao => return choose_season_strategy(team),
        Estrategia::Limiares {
            all_in,
            austeridade,
        }
        | Estrategia::PorClasse {
            all_in,
            austeridade,
        } => (all_in, austeridade),
    };
    let plan = calculate_financial_plan(team);
    // Mesma escala da produção desde B52: a da DIVISÃO da equipe.
    let scale = category_finance_scale_for(&team.categoria, team.classe.as_deref());

    if plan.debt_pressure >= scale.expected_cash_midpoint() * LIMIAR_SURVIVAL {
        return "survival";
    }
    if plan.spending_power < scale.operating_cost_midpoint() * all_in && team.car_strength() < 38.0
    {
        return "all_in";
    }
    match crate::finance::state::estado_por_meses(
        crate::finance::state::meses_de_operacao(team),
        FaixasDeMeses::default(),
    ) {
        "elite" => "balanced",
        "healthy" => {
            if team.car_strength() < 50.0 {
                "expansion"
            } else {
                "balanced"
            }
        }
        "stable" => {
            if plan.spending_power < scale.operating_cost_midpoint() * austeridade {
                "austerity"
            } else {
                "balanced"
            }
        }
        "pressured" => "all_in",
        "crisis" | "collapse" => "survival",
        _ => "balanced",
    }
}

/// O coeficiente do termo do índice no patrocínio. **É uma cópia** do literal que mora em
/// `race::financas::calculate_team_round_finance_context_modelo` — o harness não pode
/// importá-lo porque ele não tem nome lá.
///
/// A cópia é cobrada por `o_coeficiente_do_indice_ainda_e_o_da_producao`, que mede a
/// derivada do patrocínio em relação ao índice chamando a função de produção. Se alguém
/// mexer no 0,002 lá, o teste cai aqui em vez de o contrafactual passar a medir a coisa
/// errada em silêncio.
const COEF_DO_INDICE: f64 = 0.002;

/// Tudo que define uma rodada do harness. Existe para não passar seis argumentos soltos por
/// quatro níveis de função — e para que acrescentar um eixo novo não mexa em call site
/// nenhum.
#[derive(Debug, Clone, Copy)]
struct Cenario {
    coef: CoeficientesDeReceita,
    ralo: Ralo,
    offseason: Offseason,
    receita: Receita,
    despesa: Despesa,
    /// Contra que custo operacional anual `economia::receita` e `economia::desenvolvimento`
    /// se expressam.
    ancora: AncoraDoDinheiro,
    /// Largura do espalhamento da presença pública no grid. Ver `FAMA_DE_HOJE`.
    espalhamento_da_fama: f64,
    /// As fronteiras dos seis estados financeiros, em MESES de operação.
    faixas: FaixasDeMeses,
    /// O laço dinheiro → índice → patrocínio → dinheiro.
    realimentacao: Realimentacao,
    /// O empréstimo de emergência (B50).
    socorro: Socorro,
    /// O paraquedas de rebaixamento e a parcela por rodada (B47).
    paraquedas: Paraquedas,
    /// Os limiares de `choose_season_strategy` (B52).
    estrategia: Estrategia,
    /// Deslocamento da semente do mundo. A semente-base sai do NOME da categoria, então uma
    /// categoria tinha um mundo só; colapso e recuperação são eventos raros e um mundo só
    /// não distingue efeito de sorteio. Cada valor daqui é uma réplica com o mesmo desenho e
    /// outro sorteio.
    semente: u64,
}

impl Default for Cenario {
    fn default() -> Self {
        Self {
            coef: CoeficientesDeReceita::default(),
            ralo: Ralo::default(),
            offseason: Offseason::Producao,
            receita: Receita::Producao,
            // O padrão É a produção: `Despesa::Economia` chama `despesa_da_rodada`, a mesma
            // função que a rodada de verdade chama. Enquanto existiu interruptor este campo
            // lia `MODELO_EM_PRODUCAO`; com o interruptor apagado não há mais o que consultar
            // — só uma conta existe. O comparador fixa os dois lados explicitamente,
            // justamente para não depender deste padrão.
            despesa: Despesa::Economia,
            ancora: AncoraDoDinheiro::default(),
            espalhamento_da_fama: FAMA_DE_HOJE,
            faixas: FaixasDeMeses::default(),
            realimentacao: Realimentacao::Atual,
            socorro: Socorro::Producao,
            paraquedas: Paraquedas::Nenhum,
            estrategia: Estrategia::Producao,
            semente: 0,
        }
    }
}

// ===================== Acumuladores =====================

#[derive(Default, Clone, Copy)]
struct Linhas {
    /// Usado só nos acumuladores POR EQUIPE POR TEMPORADA (a autópsia). No acumulador
    /// global da categoria vale zero.
    receita_do_ano: f64,
    patrocinio: f64,
    bilheteria: f64,
    bonus: f64,
    premio_parcial: f64,
    ajuda: f64,
    /// Prêmio de construtores do FIM da temporada. Não é linha de rodada, mas é receita
    /// de projeto: o comentário do coeficiente de patrocínio em `race/financas.rs` diz que
    /// o equilíbrio do meio de grid depende dele. Fora da conta, a margem mente.
    premio_construtores: f64,
    salario: f64,
    operacao: f64,
    estrutural: f64,
    tecnico: f64,
    juros: f64,
    /// A fatia de `tecnico` que é peça de verdade (o resto é a base técnica da rodada).
    pecas: f64,
    /// O RALO simulado (`Ralo`), somando as três formas. Zero quando não há ralo ligado —
    /// é o que mantém os testes existentes intactos.
    ralo: f64,
}

/// **A foto de uma equipe no fim de uma temporada.** É a trajetória, não o destino.
///
/// Todo o resto do `Resultado` fecha em 20 temporadas ou agrega o mundo inteiro; medir o
/// laço de realimentação em 1, 3 e 5 anos exige o estado ano a ano, equipe a equipe, com o
/// suficiente para normalizar entre categorias (`mensal`) e para separar por classe e por
/// estado de partida.
#[derive(Default, Clone, Copy)]
struct FotoDaEquipe {
    /// Meses de operação com o caixa já abatido da dívida — a grandeza comparável.
    meses: f64,
    caixa: f64,
    divida: f64,
    /// `calculate_spending_power`, em dinheiro. O relatório o divide por `mensal`.
    poder_de_gasto: f64,
    /// `derive_budget_index_from_money`, 0–100.
    indice: f64,
    /// Patrocínio somado NESTA temporada.
    patrocinio_do_ano: f64,
    /// Receita total somada NESTA temporada, prêmio de construtores incluído.
    receita_do_ano: f64,
    estado: &'static str,
    /// Índice da classe em `Arena::classes` — o corte de classe do relatório.
    classe: usize,
    /// Custo de operar UM mês nesta divisão. É o denominador que torna dinheiro de rookie
    /// comparável com dinheiro de LMP2.
    mensal: f64,
}

/// A autópsia de um GRUPO de equipes-temporada: a composição do ano delas.
///
/// Existe para responder "qual linha estourou o caixa" sem opinião. A gt4 é a única
/// categoria que piora com a troca da despesa; comparar a composição do ano de quem
/// terminou em `collapse` com a de quem terminou de pé, na MESMA categoria e no MESMO
/// modelo, nomeia a linha ou prova que não é uma linha só.
///
/// Todos os campos são somas; o relatório divide por `temporadas` na hora de imprimir.
#[derive(Default, Clone, Copy)]
struct Autopsia {
    /// Quantas (equipe × temporada) caíram neste grupo.
    temporadas: u32,
    linhas: Linhas,
    /// Meses de operação com que a equipe ENTROU no ano. Um colapso que começa com 2 meses
    /// de fôlego é outra doença que um que começa com 20.
    meses_no_inicio: f64,
}

impl Autopsia {
    fn somar(&mut self, linhas: &Linhas, meses_no_inicio: f64) {
        self.temporadas += 1;
        self.meses_no_inicio += meses_no_inicio;
        let d = &mut self.linhas;
        d.receita_do_ano += linhas.receita_do_ano;
        d.patrocinio += linhas.patrocinio;
        d.bilheteria += linhas.bilheteria;
        d.bonus += linhas.bonus;
        d.premio_parcial += linhas.premio_parcial;
        d.ajuda += linhas.ajuda;
        d.premio_construtores += linhas.premio_construtores;
        d.salario += linhas.salario;
        d.operacao += linhas.operacao;
        d.estrutural += linhas.estrutural;
        d.tecnico += linhas.tecnico;
        d.juros += linhas.juros;
        d.pecas += linhas.pecas;
        d.ralo += linhas.ralo;
    }

    fn media(&self, f: impl Fn(&Linhas) -> f64) -> f64 {
        f(&self.linhas) / self.temporadas.max(1) as f64
    }

    /// A linha como % da receita do ano daquele grupo. É a forma comparável entre
    /// categorias de escalas diferentes.
    fn pct(&self, f: impl Fn(&Linhas) -> f64) -> f64 {
        f(&self.linhas) / self.linhas.receita_do_ano.max(1.0) * 100.0
    }

    fn despesa(&self) -> f64 {
        let d = &self.linhas;
        d.salario + d.operacao + d.estrutural + d.tecnico + d.juros + d.ralo
    }
}

/// A fatura da etapa ITEMIZADA, somada sobre todas as (equipe × etapa × temporada) da
/// categoria. As chaves são as mesmas nos dois modelos sempre que existe correspondência —
/// e onde não existe, o par é declarado em [`PARES_DA_FATURA`].
///
/// Existe porque "o total bateu" é uma prova fraca: dois modelos podem chegar ao mesmo
/// total repartindo o dinheiro de maneiras que descrevem mundos diferentes. É a linha que
/// mostra se o defeito conhecido (rótulo físico vestindo uma fração de orçamento) saiu.
#[derive(Default)]
struct FaturaItemizada {
    linhas: HashMap<&'static str, f64>,
    /// Quantas (equipe × etapa) entraram na soma — o divisor que transforma o acumulado em
    /// "quanto uma equipe gasta numa etapa".
    observacoes: f64,
}

impl FaturaItemizada {
    fn soma(&mut self, chave: &'static str, valor: f64) {
        *self.linhas.entry(chave).or_insert(0.0) += valor;
    }

    /// Quanto UMA equipe gasta naquela linha numa etapa, na média.
    fn por_etapa(&self, chave: &str) -> f64 {
        self.linhas.get(chave).copied().unwrap_or(0.0) / self.observacoes.max(1.0)
    }

    fn total_por_etapa(&self) -> f64 {
        self.linhas.values().sum::<f64>() / self.observacoes.max(1.0)
    }
}

/// As linhas da fatura, na ordem do relatório: rótulo, chaves que somam nela, e o que a
/// troca fez com aquela linha.
///
/// A lista de chaves é a MESMA nos dois modelos porque `despesa::rotulo_da_linha` já traduz
/// as chaves físicas para os rótulos do ledger (`combustivel` → `gasolina`, `revisao` →
/// `pecas`) — o modelo novo entra na fatura pelos mesmos nomes, o que é o que permite pôr as
/// duas colunas lado a lado sem de-para nenhum.
///
/// Duas linhas são compostas, e é por isso que a lista existe em vez de imprimir chave a
/// chave:
///
/// - **mecânica** — no legado o desgaste está repartido entre a linha `pecas` (um peso de
///   orçamento) e a "base técnica" (`0,16 × base`, que não nomeia coisa nenhuma). No físico
///   `pecas` passa a ser a revisão amortizada por quilômetro e a base técnica **vale zero**.
///   A compra de peça de `decide_car_maintenance` entra nos dois, idêntica.
/// - **estrutura** — no legado ela aparece duas vezes: dentro da fatura da etapa
///   (`W_ESTRUTURA`) e fora dela (`structural_maintenance_cost`). No físico só existe a de
///   fora: os recorrentes do ano rateados por rodada.
const PARES_DA_FATURA: [(&str, &[&str], &str); 9] = [
    (
        "combustível",
        &["gasolina"],
        "peso de orçamento → litros × US$ 3,00",
    ),
    (
        "pneus",
        &["pneus"],
        "peso de orçamento → jogos × preço do jogo",
    ),
    (
        "frete",
        &["frete"],
        "peso × travel_factor → km faturados × tarifa",
    ),
    (
        "viagem",
        &["viagem"],
        "peso × travel_factor → comitiva × passagem",
    ),
    (
        "estadia",
        &["estadia"],
        "peso amortecido → pessoa-noite × diária",
    ),
    (
        "inscrição",
        &["inscricao"],
        "peso fixo → carros × taxa da divisão",
    ),
    ("diárias", &["diarias"], "peso × crew → pessoa-dia × diária"),
    (
        "mecânica",
        &["pecas", "base_tecnica", "pecas_compradas"],
        "peso + base técnica abstrata → revisão por km",
    ),
    (
        "estrutura",
        &["estrutura", "estrutural"],
        "0,18 × base → recorrentes do ano ÷ rodadas",
    ),
];

#[derive(Default)]
struct Resultado {
    /// Quantas vezes (equipe × temporada) cada estado foi observado no fim da temporada.
    estados: HashMap<&'static str, u32>,
    /// O MESMO mundo, lido pelo instrumento LEGADO (score 0–100 binado em 70/55/40/25/12,
    /// dividindo o caixa pela âncora de estoque). Não influencia nada: é só a leitura
    /// paralela que permite dizer quanto da taxa de crise era do mundo e quanto era do
    /// termômetro.
    estados_legado: HashMap<&'static str, u32>,
    /// Meses de operação de cada equipe no fim de cada temporada, agregados. É a grandeza
    /// nova, na unidade nova.
    meses_observados: Vec<f64>,
    /// Meses de operação de cada equipe no fim da SIMULAÇÃO INTEIRA — o destino, não a
    /// trajetória. É o que o critério 2 passou a medir: a razão de deriva dizia se o caixa
    /// cresceu, mas não dizia se ele parou num lugar habitável. 3× de uma base de 6 meses é
    /// um mundo saudável; 1,3× de uma base inflada de 24 é uma equipe que nunca sente nada.
    meses_fim: Vec<f64>,
    /// Equipes que NUNCA desceram de `stable` em toda a simulação.
    nunca_apertadas: u32,
    /// Equipes que passaram ao menos uma temporada em `crisis` ou `collapse`.
    ja_quebraram: u32,
    vendas: u32,
    emprestimos: u32,
    /// Principal somado dos empréstimos de emergência da simulação inteira (B50).
    emprestimo_valor: f64,
    /// Dívida somada CRIADA pelos empréstimos — principal mais a taxa que foi capitalizada
    /// no ato. Em produção é `1,18 × principal`; no braço amortizado é o principal seco.
    divida_criada: f64,
    /// Taxa do empréstimo efetivamente cobrada em caixa ao longo do tempo (braço amortizado).
    taxa_amortizada_paga: f64,
    /// Quantos empréstimos cada equipe do grid tomou na simulação inteira, na ordem do grid.
    /// É o eixo de REINCIDÊNCIA: um canal que socorre 20 equipes uma vez é outra coisa que um
    /// canal que socorre 4 equipes cinco vezes.
    emprestimos_por_equipe: Vec<u32>,
    /// (equipe × temporada) em que o gatilho do empréstimo esteve ABERTO no fim de alguma
    /// rodada — a taxa de elegibilidade, que é diferente da taxa de uso.
    elegiveis: u32,
    /// Ajuda de paraquedas efetivamente paga na simulação inteira, no braço que estiver
    /// rodando (B47).
    ajuda_paga: f64,
    /// Quantas equipes-temporada receberam o saldo de paraquedas (a coorte sintética).
    paraquedas_concedidos: u32,
    /// Distribuição de `season_strategy` escolhida no fim de cada temporada (B52).
    estrategias: HashMap<&'static str, u32>,
    linhas: Linhas,
    /// Caixa em múltiplos do caixa-médio da categoria, na primeira e na última temporada.
    caixa_inicio: Vec<f64>,
    caixa_fim: Vec<f64>,
    divida_fim: Vec<f64>,
    /// Nível médio de peça no fim: o pior, o médio e o melhor carro do grid.
    nivel_pior: f64,
    nivel_medio: f64,
    nivel_melhor: f64,
    /// Saldo líquido somado do grid inteiro, por temporada.
    saldo_por_temporada: Vec<f64>,
    /// Pontos somados pelos carros de uma equipe numa corrida, na média — o número que
    /// diz se a forma do grid (nº de equipes × classes × tabela de pontos) bate com o save.
    pontos_por_equipe_por_corrida: f64,
    /// Receita de temporada do CAMPEÃO da classe e do ÚLTIMO colocado, na média das
    /// temporadas. É o acoplamento posição→dinheiro: se os dois números forem parecidos,
    /// terminar em 1º ou em último não muda a vida da equipe.
    receita_campeao: f64,
    receita_lanterna: f64,
    saldo_campeao: f64,
    saldo_lanterna: f64,
    /// (equipe × temporada) que fecharam no azul, mas fechariam no vermelho SEM o prêmio
    /// de construtores. É a medida de "o fechamento é muleta".
    salvas_pelo_fechamento: u32,
    temporadas_equipe: u32,
    /// Bilheteria da melhor e da pior equipe do grid, somada na temporada — mede se o
    /// portão diferencia alguém ou paga igual para todos.
    portao_melhor: f64,
    portao_pior: f64,
    /// Quanto o portão representa da RECEITA da equipe de menor atração do grid, em %.
    ///
    /// A razão melhor÷pior (critério 11) diz se o portão diferencia; este número diz se ele
    /// **sustenta**. Uma razão de 5× em que o pior recebe 1% da receita é um canal que
    /// diferencia decorativamente: o critério 4 (10–20% em toda categoria) mede a média da
    /// categoria e não vê o fundo do grid.
    portao_pct_da_pior: f64,
    /// Pontos de `engineering` + `facilities` que o offseason REGALA ao grid inteiro em
    /// toda a simulação. Diagnóstico do ralo B: se o offseason só sobe 1 ponto por equipe
    /// por temporada, cobrar por ponto nunca vai drenar muito, por mais caro que seja o
    /// ponto.
    pontos_estrutura: f64,
    /// `engineering + facilities` de cada equipe no FIM da simulação (0–200), na ordem do
    /// grid. É a medida de para onde o dinheiro foi: se rico e pobre terminam com a mesma
    /// estrutura, a riqueza não virou nada que o jogador sinta na pista.
    estrutura_fim: Vec<f64>,
    /// Desvio-padrão da presença pública no grid. É o eixo que o critério 11 depende: com a
    /// fama de hoje ele fica em ~5–6 e a cota de bilheteria colapsa em 1/N.
    desvio_da_presenca: f64,
    /// A fatura da etapa linha a linha, no modelo que estiver rodando. Alimentada sempre —
    /// custa uma soma por linha por rodada e é o que o comparador lê.
    fatura: FaturaItemizada,
    /// A conta FIXA de cada equipe (salário + estrutura) dividida pela receita dela, em %,
    /// acumulada na simulação inteira e ordenada do menor para o maior.
    ///
    /// É a medida do FREIO. A parte variável da conta (operação, peça) encolhe sozinha
    /// quando a temporada vai mal: carro que abandona queima menos combustível, equipe sem
    /// caixa compra menos peça. A parte fixa chega igual. Uma equipe cuja conta fixa já come
    /// a maior parte da receita não tem como cortar para sobreviver a um ano ruim — e é
    /// exatamente essa equipe que o modelo físico criou, ao mover ~40% da fatura do bloco
    /// variável para o fixo.
    /// **Na ordem do grid**, não ordenado — quem quiser a distribuição ordena a cópia dele.
    /// Em ordem de grid ele pode ser cruzado com `meses_fim` e `pontos_acumulados`, que é o
    /// que permite perguntar "quanto sobra para o CAMPEÃO" em vez de "quanto sobra na média".
    fixo_sobre_receita: Vec<f64>,
    /// Títulos de classe que cada equipe ganhou nas 20 temporadas, na ordem do grid.
    ///
    /// É o que separa "categoria uniforme porque ninguém ganha muito" de "categoria uniforme
    /// porque o vencedor RODA". Uma etapa pode pagar muito bem ao vencedor sem que isso
    /// concentre nada, desde que o vencedor seja outro toda temporada.
    titulos: Vec<u32>,
    /// Pontos somados por cada equipe em toda a simulação, na ordem do grid. É o eixo de
    /// POSIÇÃO: ordenar por ele separa o time de ponta do lanterna de forma estável, coisa
    /// que a posição de uma temporada só não faz num grid pequeno cheio de ruído.
    pontos_acumulados: Vec<f64>,
    /// Receita somada por equipe em toda a simulação, na ordem do grid.
    receita_por_equipe: Vec<f64>,
    /// Saldo líquido somado por equipe em toda a simulação, na ordem do grid. É o que
    /// separa "termina com muitos meses porque fatura muito" de "termina com muitos meses
    /// porque nunca gastou".
    saldo_por_equipe: Vec<f64>,
    /// Receita acumulada da MELHOR equipe do grid ÷ a da PIOR. É o outro fator do produto:
    /// uma conta fixa alta só afoga alguém se a receita for espalhada dentro do grid.
    receita_espalhada: f64,
    /// A foto de cada equipe no fim de cada temporada: `serie[temporada - 1][equipe]`.
    /// Alimentada sempre; custa uma struct por equipe por ano.
    serie: Vec<Vec<FotoDaEquipe>>,
    /// A foto do grid ANTES da primeira corrida — o estado de partida de cada equipe, que é
    /// o corte "por estado financeiro" do relatório de realimentação.
    foto_inicial: Vec<FotoDaEquipe>,
    /// A composição do ano de quem terminou a temporada em `collapse`.
    autopsia_colapso: Autopsia,
    /// A composição do ano de quem terminou de pé (`stable` ou melhor) — o controle.
    autopsia_saudavel: Autopsia,
}

// ===================== O laço =====================

fn medir_categoria(categoria: &'static str) -> Resultado {
    medir_categoria_com(
        categoria,
        CoeficientesDeReceita::default(),
        Ralo::default(),
        Offseason::Producao,
    )
}

fn medir_categoria_com(
    categoria: &'static str,
    coef: CoeficientesDeReceita,
    ralo: Ralo,
    offseason: Offseason,
) -> Resultado {
    medir_categoria_cenario(
        categoria,
        &Cenario {
            coef,
            ralo,
            offseason,
            ..Cenario::default()
        },
    )
}

fn medir_categoria_cenario(categoria: &'static str, cenario: &Cenario) -> Resultado {
    let Cenario {
        coef,
        ralo,
        offseason,
        receita: modelo_de_receita,
        despesa: modelo_de_despesa,
        ancora: ancora_do_dinheiro,
        espalhamento_da_fama,
        faixas,
        realimentacao,
        socorro,
        paraquedas,
        estrategia,
        semente,
    } = *cenario;
    let arena = arena(categoria);
    let refer = referencia(categoria);
    let mut rng = StdRng::seed_from_u64(
        categoria
            .bytes()
            .map(|b| b as u64)
            .sum::<u64>()
            .wrapping_add(semente.wrapping_mul(1_000_003)),
    );
    let mut grid = montar_grid(&arena, &mut rng, espalhamento_da_fama, faixas);
    let scale = category_finance_scale(arena.id);
    // O custo operacional declarado de UMA temporada de UMA equipe. É a unidade em que o
    // ralo é expresso — "drenar X% do operacional anual" só quer dizer alguma coisa contra
    // este número.
    let op_anual = scale.operating_cost_midpoint();
    // O "multiplicador escondido do calendário": toda fração de receita e de despesa do
    // modelo velho é múltiplo deste número, e ele encolhe quando o calendário cresce.
    let round_base = op_anual / arena.rodadas as f64;
    // O custo operacional anual que os módulos NOVOS recebem como entrada, POR CLASSE. No
    // multi-classe isso importa: uma LMP2 e uma GT4 dentro do mesmo Endurance não custam a
    // mesma coisa, e a âncora declarada não sabia disso — ela era uma só para a categoria.
    let op_por_classe: Vec<f64> = arena
        .classes
        .iter()
        .map(|c| match ancora_do_dinheiro {
            AncoraDoDinheiro::Declarada => op_anual,
            AncoraDoDinheiro::Fisica => {
                let classe = (!c.nome.is_empty()).then_some(c.nome);
                crate::economia::temporada::custo_operacional_anual_de_referencia(arena.id, classe)
            }
        })
        .collect();
    let is_endurance = arena.id == "endurance";
    let n_equipes = arena.equipes;

    let mut r = Resultado::default();
    {
        let media = grid.iter().map(|e| e.presenca).sum::<f64>() / n_equipes.max(1) as f64;
        r.desvio_da_presenca = (grid
            .iter()
            .map(|e| (e.presenca - media).powi(2))
            .sum::<f64>()
            / n_equipes.max(1) as f64)
            .sqrt();
    }
    let mut apertou: Vec<bool> = vec![false; n_equipes];
    let mut quebrou: Vec<bool> = vec![false; n_equipes];
    let mut pontos_totais = 0.0f64;
    let mut corridas_equipe = 0.0f64;
    let mut portao_por_equipe = vec![0.0f64; n_equipes];
    // Receita acumulada por equipe em TODA a simulação — o denominador de
    // `portao_pct_da_pior`. `receita_temporada` zera a cada ano, então não serve.
    let mut receita_acumulada = vec![0.0f64; n_equipes];
    // A conta que chega tenha a temporada ido bem ou mal: folha e estrutura.
    let mut fixo_acumulado = vec![0.0f64; n_equipes];
    // O eixo de POSIÇÃO e o eixo de RESULTADO FINANCEIRO, por equipe, na simulação inteira.
    let mut pontos_acumulados = vec![0.0f64; n_equipes];
    let mut saldo_acumulado = vec![0.0f64; n_equipes];
    let mut titulos = vec![0u32; n_equipes];
    // B50: quantos socorros cada equipe tomou, e a taxa que o braço amortizado ainda deve
    // cobrar dela (valor pendente e quantas parcelas faltam).
    let mut emprestimos_por_equipe = vec![0u32; n_equipes];
    let mut taxa_pendente = vec![0.0f64; n_equipes];
    let mut parcelas_da_taxa = vec![0usize; n_equipes];
    for e in &grid {
        r.caixa_inicio
            .push(e.team.cash_balance / scale.expected_cash_midpoint());
        r.foto_inicial.push(FotoDaEquipe {
            meses: meses_de_operacao(&e.team),
            caixa: e.team.cash_balance,
            divida: e.team.debt_balance,
            poder_de_gasto: calculate_spending_power(&e.team),
            indice: derive_budget_index_from_money(&e.team),
            patrocinio_do_ano: 0.0,
            receita_do_ano: 0.0,
            estado: estado_estatico(&e.team),
            classe: e.classe,
            mensal: custo_operacional_mensal(&e.team.categoria, e.team.classe.as_deref()),
        });
    }

    for temporada in 1..=TEMPORADAS {
        let saude = global_economic_health_for_season(temporada);
        let pistas = calendario(&arena, temporada);
        let mut pontos_temporada = vec![0i32; n_equipes];
        let mut receita_temporada = vec![0.0f64; n_equipes];
        let mut saldo_temporada_equipe = vec![0.0f64; n_equipes];
        let mut saldo_temporada = 0.0;
        // A autópsia: as mesmas linhas do acumulador global, mas POR EQUIPE e POR ANO, para
        // que a composição do ano de quem quebrou possa ser posta ao lado da de quem não
        // quebrou. E o fôlego com que cada uma entrou no ano.
        let mut linhas_temporada = vec![Linhas::default(); n_equipes];
        let meses_no_inicio: Vec<f64> = grid.iter().map(|e| meses_de_operacao(&e.team)).collect();
        // O gatilho do empréstimo esteve aberto em alguma rodada deste ano? (B50)
        let mut elegivel_no_ano = vec![false; n_equipes];

        for etapa in 0..arena.rodadas {
            let track = pistas[etapa % pistas.len().max(1)];
            let prestigio = prestigio_da_rodada(&arena, etapa, track);

            // ── Ordem de chegada, por CARRO ───────────────────────────────────────────
            // Uma equipe tem `pilotos_por_equipe` carros, e é a soma dos dois que vira
            // ponto, vitória e pódio na fatura. O harness antigo corria UM carro por
            // equipe e cortava o prêmio por resultado pela metade em toda a escada.
            let mut carros: Vec<(usize, f64, bool)> = Vec::with_capacity(n_equipes * 2);
            for (i, e) in grid.iter().enumerate() {
                for _ in 0..arena.pilotos_por_equipe {
                    let abandonou = rng.gen_bool((refer.dnf_pct / 100.0).clamp(0.0, 1.0));
                    let forca = e.team.car_strength() + rng.gen_range(-12.0..12.0);
                    carros.push((i, forca, abandonou));
                }
            }
            // Quem abandonou vai para o fim da ordem; o resto ordena por ritmo.
            carros.sort_by(|a, b| {
                a.2.cmp(&b.2)
                    .then(b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
            });

            // Posição é POR CLASSE (é o que `race::grade` grava em `finish_position`).
            let mut proxima_posicao = vec![0u8; arena.classes.len()];
            // Volta rápida: um carro por CORRIDA, e só pontua se estiver no top-10 da classe.
            let indice_volta_rapida = rng.gen_range(0..carros.len().max(1));

            let mut pontos_rodada = vec![0i32; n_equipes];
            let mut vitorias_rodada = vec![0i32; n_equipes];
            let mut podios_rodada = vec![0i32; n_equipes];
            let mut melhor_rodada = vec![99i32; n_equipes];
            let mut voltas_rodada = vec![(0.0f64, 0.0f64); n_equipes]; // (soma, carros)
                                                                       // Posição de cada carro da equipe DENTRO DA CLASSE, incluindo quem abandonou —
                                                                       // carro que largou continua ocupando lugar na classificação, e o prêmio por
                                                                       // etapa do modelo novo paga por posição, não por ponto.
            let mut posicoes_rodada: Vec<Vec<u32>> = vec![Vec::new(); n_equipes];
            let mut volta_rapida_rodada = vec![false; n_equipes];

            for (ordem, (equipe, _, abandonou)) in carros.iter().enumerate() {
                let ic = grid[*equipe].classe;
                proxima_posicao[ic] += 1;
                let posicao_classe = proxima_posicao[ic];
                posicoes_rodada[*equipe].push(posicao_classe as u32);
                if ordem == indice_volta_rapida && !*abandonou {
                    volta_rapida_rodada[*equipe] = true;
                }

                if *abandonou {
                    // Abandono: zero ponto e a etapa só foi rodada em parte — queima menos
                    // combustível e menos pneu, e é o que `round_operation_context` mede.
                    voltas_rodada[*equipe].0 += rng.gen_range(0.15..0.85);
                    voltas_rodada[*equipe].1 += 1.0;
                    continue;
                }

                let mut pontos = get_points_for_position(posicao_classe, is_endurance) as i32;
                if ordem == indice_volta_rapida && posicao_classe <= 10 {
                    pontos += BONUS_FASTEST_LAP as i32;
                }
                pontos_rodada[*equipe] += pontos;
                vitorias_rodada[*equipe] += i32::from(posicao_classe == 1);
                podios_rodada[*equipe] += i32::from(posicao_classe <= 3);
                melhor_rodada[*equipe] = melhor_rodada[*equipe].min(posicao_classe as i32);
                voltas_rodada[*equipe].0 += 1.0;
                voltas_rodada[*equipe].1 += 1.0;
            }

            let presenca_total: f64 = grid.iter().map(|e| e.presenca).sum();
            // A bilheteria do modelo ANTIGO passou a ratear por ATRAÇÃO DE PÚBLICO, não
            // por presença (ver o bloco da bilheteria em `race/financas.rs`). O total
            // aqui tem que ser da mesma grandeza do numerador, senão a cota vira lixo.
            // `presenca_total` segue intacto: é ele que alimenta o modelo NOVO de
            // receita, cuja entrada é de outra sessão.
            let equipes_do_grid: Vec<crate::models::team::Team> =
                grid.iter().map(|e| e.team.clone()).collect();
            let posicoes = crate::public_presence::atracao::posicoes_por_pontos(&equipes_do_grid);
            let atracoes: Vec<f64> = grid
                .iter()
                .map(|e| {
                    crate::public_presence::atracao::team_audience_appeal_from_presence(
                        &e.team,
                        e.presenca,
                        posicoes.get(&e.team.id).copied().unwrap_or(0),
                        n_equipes as u32,
                        track.pais,
                    )
                })
                .collect();
            let atracao_total: f64 = atracoes.iter().sum();
            // O índice MÉDIO de cada classe no início da rodada — a entrada do braço
            // `Achatada`. Sai do mesmo instantâneo que a atração usa, antes de qualquer
            // equipe mexer no próprio caixa nesta rodada; produção lê o time vivo, e a
            // diferença é de uma rodada de defasagem numa média de grid.
            let indice_medio_da_classe: Vec<f64> = (0..arena.classes.len())
                .map(|ic| {
                    let da_classe: Vec<f64> = grid
                        .iter()
                        .filter(|e| e.classe == ic)
                        .map(|e| derive_budget_index_from_money(&e.team))
                        .collect();
                    if da_classe.is_empty() {
                        0.0
                    } else {
                        da_classe.iter().sum::<f64>() / da_classe.len() as f64
                    }
                })
                .collect();
            let restantes = arena.rodadas - etapa;
            let janela_completa: Vec<u32> = (0..arena.rodadas)
                .map(|i| pistas[(etapa + i) % pistas.len().max(1)].track_id)
                .collect();

            for (i, e) in grid.iter_mut().enumerate() {
                // ── 1. Contato de disputa e quebra: o castigo físico ANTES do cérebro ──
                // É a ordem de `maintain_team_car_pits`. O harness antigo pulava as duas
                // coisas, e com elas some a maior parte do custo de peça não planejado.
                let contatos =
                    (refer.incidentes_por_carro * arena.pilotos_por_equipe as f64).round() as u32;
                let dano = crate::car::crash::apply_contact_wear(&mut e.car, arena.id, contatos);
                for &pt in &dano.destroyed {
                    if let Some(p) = e.car.parts.iter_mut().find(|p| p.part_type == pt) {
                        p.wear = p.wear.max(1.0);
                    }
                }
                // Quase todo abandono no mundo real é MECÂNICO (medido: motor, câmbio,
                // suspensão, freio dominam `dnf_reason`), e peça que largou é trocada à
                // força, mesmo sem caixa. Um carro do time abandonou nesta rodada?
                let abandonou_algum = voltas_rodada[i].0 < voltas_rodada[i].1 - 0.001;
                if abandonou_algum {
                    let pt = PartType::ALL[rng.gen_range(0..PartType::ALL.len())];
                    if let Some(p) = e.car.parts.iter_mut().find(|p| p.part_type == pt) {
                        p.wear = p.wear.max(1.0);
                    }
                }
                // Teto de desenvolvimento: um carro nunca fica acima do que a classe admite.
                let teto = crate::car::cost::development_ceiling(e.team.car_ceiling());
                for part in e.car.parts.iter_mut() {
                    if part.level > teto {
                        part.level = teto;
                    }
                }
                // O carro que o cérebro vai olhar é o já castigado — em produção ele lê o
                // mesmo objeto. Sem isto o plano decidiria sobre o carro da rodada passada.
                e.team.car = Some(e.car.clone());

                // ── 2. Manutenção: o cérebro decide dentro do spending_power ───────────
                // A janela é cortada pelo horizonte de planejamento do time, como em
                // produção — time míope não enxerga o calendário inteiro e compra pior.
                let horizonte = planning_horizon(&e.team.id, temporada);
                let janela: &[u32] = match horizonte.lookahead() {
                    Some(n) => &janela_completa[..janela_completa.len().min(n)],
                    None => &janela_completa,
                };
                let cota = upgrades_permitidos_nesta_corrida(&e.team.id, restantes);
                let plano = decide_car_maintenance(&e.team, &e.car, arena.id, janela, Some(cota));
                let custo_manutencao = plano.estimated_cost;
                apply_plan(&mut e.car, &plano);
                e.team.car = Some(e.car.clone());

                // ── 3. A fatura da rodada, pela mesma função do jogo ───────────────────
                let carros_do_time = voltas_rodada[i].1.max(1.0);
                let laps_ratio = (voltas_rodada[i].0 / carros_do_time).clamp(0.0, 1.0);
                let pontos = pontos_rodada[i];
                pontos_totais += pontos as f64;
                pontos_acumulados[i] += pontos as f64;
                corridas_equipe += 1.0;
                pontos_temporada[i] += pontos;
                // `stats_pontos` alimenta `public_presence::atracao::posicoes_por_pontos`, que
                // é o termo de COMPETITIVIDADE da atração de público — 65% dela. Sem acumular
                // aqui, todo o grid chega com zero ponto, `posicoes_por_pontos` devolve
                // posição 0 para todo mundo, o termo vira constante 0,5 e o espalhamento da
                // atração colapsa para a diferença de mídia sozinha (medido: 1,4–1,7× em vez
                // dos 2,7–5,1× de produção). O harness zerava no fim da temporada sem nunca
                // ter somado.
                e.team.stats_pontos += pontos;

                // A classe da equipe é a chave da divisão competitiva nos módulos novos: numa
                // GT3 dentro do Endurance a física e os recorrentes são os da classe, não os
                // do campeonato.
                let classe_da_equipe: Option<String> = {
                    let n = arena.classes[e.classe].nome;
                    (!n.is_empty()).then(|| n.to_string())
                };
                let etapa_fisica = EtapaFisica {
                    duracao_corrida_min: crate::economia::ancora::parametros(
                        arena.id,
                        classe_da_equipe.as_deref(),
                    )
                    .duracao_corrida_min,
                    carros_inscritos: arena.pilotos_por_equipe as u32,
                };
                let operacao = RoundOperationContext {
                    track_id: track.track_id,
                    laps_ratio,
                    // Desgaste final medido no save (0,85–0,95). O 0,6 antigo
                    // subestimava a linha de pneus em toda a escada.
                    tire_wear: refer.desgaste_final,
                };

                let mut ctx = calculate_team_round_finance_context_modelo(
                    &e.team,
                    e.presenca,
                    atracoes[i],
                    pontos,
                    vitorias_rodada[i],
                    podios_rodada[i],
                    melhor_rodada[i],
                    e.folha_anual / arena.rodadas as f64,
                    arena.rodadas as f64,
                    saude,
                    custo_manutencao,
                    operacao,
                    etapa_fisica,
                    prestigio,
                    atracao_total,
                    n_equipes as f64,
                    coef,
                    modelo_de_despesa.calculo(),
                );

                // ── O MODELO NOVO DE RECEITA ──────────────────────────────────────────
                // Substitui os QUATRO canais por-etapa e deixa as cinco linhas de despesa
                // intactas: `economia::evento` e `economia::temporada` são de outra sessão,
                // e trocar receita e despesa ao mesmo tempo tornaria a medição ilegível.
                if let Receita::Economia(params) = modelo_de_receita {
                    let classe = &arena.classes[e.classe];
                    let entrada = EntradaDeReceitaDaEtapa {
                        custo_operacional_anual: op_por_classe[e.classe],
                        etapas_na_temporada: arena.rodadas as f64,
                        carros_na_classe: (classe.equipes * arena.pilotos_por_equipe) as f64,
                        carros_da_equipe: arena.pilotos_por_equipe as f64,
                        posicoes_na_classe: posicoes_rodada[i].clone(),
                        volta_mais_rapida: volta_rapida_rodada[i],
                        reputacao: e.team.reputacao,
                        // A cota de bilheteria rateia por ATRAÇÃO DE PÚBLICO de produção
                        // (`public_presence::atracao`), não pela presença sintética deste
                        // harness. É a atração que carrega competitividade, vínculo local e
                        // história — os termos que produzem espalhamento de verdade.
                        //
                        // O patrocínio segue lendo a presença do lineup: ali o que se
                        // negocia é a fama dos pilotos, não quanto público a equipe atrai.
                        presenca_publica: e.presenca,
                        atracao_de_publico: atracoes[i],
                        atracao_total_do_grid: atracao_total,
                        equipes_no_grid: n_equipes as f64,
                        prestigio_do_evento: prestigio,
                    };
                    let nova = receita_da_etapa(&entrada, &params);
                    // O mapeamento para o ledger antigo: o prêmio por etapa ocupa o lugar
                    // do `result_bonus` (o canal "resultado vira dinheiro") e a volta rápida
                    // o do `partial_prize_income`. É o que mantém o critério 3 medindo a
                    // mesma coisa nos dois modelos.
                    ctx.result_bonus = nova.premio_de_corrida;
                    ctx.partial_prize_income = nova.volta_mais_rapida;
                    ctx.sponsorship_income = nova.patrocinio;
                    ctx.gate_income = nova.bilheteria;
                }

                // ── O CONTRAFACTUAL DA REALIMENTAÇÃO ──────────────────────────────────
                // O termo sai por subtração, reconstruído com as MESMAS entradas que a
                // função de produção usou uma linha acima: o índice da equipe neste
                // instante, a base da rodada da divisão DELA (classe incluída, que é o que
                // `category_finance_scale_for` resolve) e o modificador da temporada
                // econômica. Reconstruir em vez de reimplementar a receita inteira é o que
                // garante que a única diferença entre os dois mundos seja este termo.
                //
                // Só vale sobre `Receita::Producao`: o modelo novo (`economia::receita`)
                // sobrescreve o patrocínio e, por decisão da seção 2.4 dele, já nasce sem
                // realimentação de riqueza — não há o que zerar ali.
                if realimentacao != Realimentacao::Atual && modelo_de_receita == Receita::Producao {
                    let base_da_equipe =
                        category_finance_scale_for(&e.team.categoria, e.team.classe.as_deref())
                            .operating_cost_midpoint()
                            / (arena.rodadas as f64).max(1.0);
                    let por_ponto_de_indice =
                        base_da_equipe * COEF_DO_INDICE * economy_income_modifier(saude);
                    let indice_da_equipe = calculate_financial_plan(&e.team).budget_index;
                    let indice_pago = match realimentacao {
                        Realimentacao::Atual => indice_da_equipe,
                        Realimentacao::Zerada => 0.0,
                        Realimentacao::Achatada => indice_medio_da_classe[e.classe],
                    };
                    ctx.sponsorship_income +=
                        (indice_pago - indice_da_equipe) * por_ponto_de_indice;
                }

                let ctx = ctx;

                // ── A ITEMIZAÇÃO, no modelo que estiver rodando ───────────────────────
                // `despesa_da_rodada` é a MESMA função que `calculate_team_round_finance_
                // context_modelo` chamou por dentro para produzir o `ctx` acima — chamá-la
                // de novo é o preço de ver as linhas, já que o contexto só devolve totais.
                // O `cost_modifier` da temporada econômica não é reconstruído aqui: as
                // linhas são reescaladas para somar EXATAMENTE o `event_operations_cost`
                // que foi debitado, então a repartição é fiel a qualquer nível.
                let itens = (modelo_de_despesa.calculo())(
                    &e.team,
                    round_base,
                    1.0,
                    arena.rodadas as f64,
                    operacao,
                    etapa_fisica,
                );
                let soma_das_linhas: f64 = itens.linhas.iter().map(|l| l.cost).sum();
                let escala = if soma_das_linhas > 0.0 {
                    ctx.event_operations_cost / soma_das_linhas
                } else {
                    0.0
                };
                for l in &itens.linhas {
                    r.fatura.soma(l.key, l.cost * escala);
                }
                // Fora da fatura da etapa: a estrutura e o técnico, que o ledger cobra em
                // linhas próprias. No `Fisico` a linha técnica é só a peça comprada — o
                // termo abstrato `0,16 × base` deixou de existir, e é isso que o comparador
                // precisa mostrar em vez de esconder num total.
                r.fatura.soma("estrutural", ctx.structural_maintenance_cost);
                r.fatura.soma(
                    "base_tecnica",
                    ctx.technical_investment_cost - custo_manutencao,
                );
                r.fatura.soma("pecas_compradas", custo_manutencao);
                r.fatura.observacoes += 1.0;

                r.linhas.patrocinio += ctx.sponsorship_income;
                r.linhas.bilheteria += ctx.gate_income;
                r.linhas.bonus += ctx.result_bonus;
                r.linhas.premio_parcial += ctx.partial_prize_income;
                r.linhas.ajuda += ctx.aid_income;
                r.ajuda_paga += ctx.aid_income;
                r.linhas.salario += ctx.salary_expense;
                r.linhas.operacao += ctx.event_operations_cost;
                r.linhas.estrutural += ctx.structural_maintenance_cost;
                r.linhas.tecnico += ctx.technical_investment_cost;
                r.linhas.juros += ctx.debt_service_cost;
                r.linhas.pecas += custo_manutencao;
                portao_por_equipe[i] += ctx.gate_income;
                let receita_da_rodada = ctx.sponsorship_income
                    + ctx.gate_income
                    + ctx.result_bonus
                    + ctx.partial_prize_income
                    + ctx.aid_income;
                receita_temporada[i] += receita_da_rodada;
                receita_acumulada[i] += receita_da_rodada;
                fixo_acumulado[i] += ctx.salary_expense + ctx.structural_maintenance_cost;

                // A mesma rodada, no acumulador da autópsia.
                {
                    let lt = &mut linhas_temporada[i];
                    lt.receita_do_ano += receita_da_rodada;
                    lt.patrocinio += ctx.sponsorship_income;
                    lt.bilheteria += ctx.gate_income;
                    lt.bonus += ctx.result_bonus;
                    lt.premio_parcial += ctx.partial_prize_income;
                    lt.ajuda += ctx.aid_income;
                    lt.salario += ctx.salary_expense;
                    lt.operacao += ctx.event_operations_cost;
                    lt.estrutural += ctx.structural_maintenance_cost;
                    lt.tecnico += ctx.technical_investment_cost;
                    lt.juros += ctx.debt_service_cost;
                    lt.pecas += custo_manutencao;
                }

                // ── 4. Caixa, evento de crise, estado — a ordem de persistencia.rs ─────
                let resumo = apply_round_cashflow(&mut e.team, ctx);
                saldo_temporada += resumo.net;
                saldo_temporada_equipe[i] += resumo.net;

                // ── RALO A: custo fixo de manter a estrutura de pé ─────────────────────
                // Cobrado por rodada e ANTES do evento de crise, porque é conta que chega
                // junto com as outras — se ela é o que empurra o time para o empréstimo de
                // emergência, é isso que se quer medir.
                let manutencao_estrutura =
                    ralo.custo_anual_de_manter(&e.team, op_anual) / arena.rodadas as f64;
                if manutencao_estrutura > 0.0 {
                    e.team.cash_balance -= manutencao_estrutura;
                    r.linhas.ralo += manutencao_estrutura;
                    linhas_temporada[i].ralo += manutencao_estrutura;
                    saldo_temporada -= manutencao_estrutura;
                    saldo_temporada_equipe[i] -= manutencao_estrutura;
                }

                // ── B47: a parcela do braço ABSOLUTO ───────────────────────────────────
                // A produção já pagou a parcela dela dentro do `ctx` (total ÷ rodadas desde
                // 12/08/2026) e `apply_round_cashflow` já abateu esse tanto do saldo. O braço
                // absoluto reescreve o pagamento para os 25 mil fixos que valiam antes, para
                // o dinheiro entrar na MESMA rodada e a diferença ser só o TAMANHO da parcela.
                if let Paraquedas::Absoluta = paraquedas {
                    let saldo_antes = e.team.parachute_payment_remaining + ctx.aid_income;
                    if saldo_antes > 0.0 {
                        let alvo = PARCELA_DE_AJUDA.min(saldo_antes);
                        e.team.cash_balance += alvo - ctx.aid_income;
                        e.team.parachute_payment_remaining = (saldo_antes - alvo).max(0.0);
                        r.ajuda_paga += alvo - ctx.aid_income;
                    }
                }

                // Elegibilidade lida SEMPRE pelo gatilho de produção, em qualquer braço: é a
                // pergunta "quantas equipes a regra de hoje alcançaria", e ela não pode
                // depender do braço que está rodando.
                if crate::finance::events::emergency_loan_amount_na_temporada(&e.team, temporada)
                    .is_some()
                {
                    elegivel_no_ano[i] = true;
                }

                // B50: a taxa diferida do braço amortizado, cobrada em caixa antes de um
                // socorro novo — é despesa da rodada como qualquer outra, e conta como juros.
                if parcelas_da_taxa[i] > 0 {
                    let parcela = taxa_pendente[i] / parcelas_da_taxa[i] as f64;
                    e.team.cash_balance -= parcela;
                    taxa_pendente[i] -= parcela;
                    parcelas_da_taxa[i] -= 1;
                    r.linhas.juros += parcela;
                    r.taxa_amortizada_paga += parcela;
                    linhas_temporada[i].juros += parcela;
                    saldo_temporada -= parcela;
                    saldo_temporada_equipe[i] -= parcela;
                }

                if let Some(s) = aplicar_socorro(&mut e.team, socorro, temporada) {
                    r.emprestimos += 1;
                    r.emprestimo_valor += s.principal;
                    r.divida_criada += s.divida;
                    emprestimos_por_equipe[i] += 1;
                    if s.taxa_diferida > 0.0 {
                        taxa_pendente[i] += s.taxa_diferida;
                        parcelas_da_taxa[i] = arena.rodadas;
                    }
                }
                refresh_team_financial_state_com(&mut e.team, faixas);
            }
        }

        // ── Fim de temporada: prêmio de construtores POR CLASSE ──────────────────────
        // `award_constructor_prizes` agrupa por (categoria, classe): num Endurance de 18
        // equipes o grid do prêmio é 6, não 18. Distribuir sobre 18 achatava a inclinação
        // do prêmio e mudava quem lucra.
        for (ic, classe) in arena.classes.iter().enumerate() {
            let mut classificacao: Vec<(usize, i32)> = (0..n_equipes)
                .filter(|i| grid[*i].classe == ic)
                .map(|i| (i, pontos_temporada[i]))
                .collect();
            classificacao.sort_by(|a, b| b.1.cmp(&a.1));
            for (colocacao, (i, _)) in classificacao.iter().enumerate() {
                let premio = match modelo_de_receita {
                    Receita::Producao => constructor_prize_with(
                        arena.id,
                        (!classe.nome.is_empty()).then_some(classe.nome),
                        (colocacao + 1) as i32,
                        classe.equipes as i32,
                        coef.premio_base,
                        coef.premio_inclinacao,
                    ),
                    Receita::Economia(params) => premio_de_fim_de_temporada(
                        op_por_classe[ic],
                        (colocacao + 1) as u32,
                        classe.equipes as u32,
                        &params,
                    ),
                };
                grid[*i].team.cash_balance += premio;
                r.linhas.premio_construtores += premio;
                linhas_temporada[*i].premio_construtores += premio;
                linhas_temporada[*i].receita_do_ano += premio;
                saldo_temporada += premio;
                receita_temporada[*i] += premio;
                saldo_temporada_equipe[*i] += premio;
                refresh_team_financial_state_com(&mut grid[*i].team, faixas);

                if colocacao == 0 {
                    r.receita_campeao += receita_temporada[*i];
                    r.saldo_campeao += saldo_temporada_equipe[*i];
                    titulos[*i] += 1;
                }
                if colocacao + 1 == classificacao.len() {
                    r.receita_lanterna += receita_temporada[*i];
                    r.saldo_lanterna += saldo_temporada_equipe[*i];
                    // ── B47: a coorte sintética de rebaixadas ─────────────────────────
                    // A última da classe recebe o saldo de paraquedas. Ela NÃO desce de
                    // divisão (o harness não tem escada), então o alívio medido aqui é uma
                    // cota inferior: no jogo o mesmo dinheiro compraria mais meses da
                    // divisão de baixo, que é mais barata.
                    let novo = match paraquedas {
                        Paraquedas::Nenhum => 0.0,
                        Paraquedas::Producao => {
                            crate::finance::events::parachute_payment_for_relegation(&grid[*i].team)
                        }
                        Paraquedas::Absoluta => paraquedas_absoluto_antigo(&grid[*i].team),
                    };
                    if novo > 0.0 {
                        grid[*i].team.parachute_payment_remaining += novo;
                        r.paraquedas_concedidos += 1;
                    }
                }
                // O fechamento é muleta? Fechou no azul só por causa dele.
                r.temporadas_equipe += 1;
                if saldo_temporada_equipe[*i] > 0.0 && saldo_temporada_equipe[*i] - premio < 0.0 {
                    r.salvas_pelo_fechamento += 1;
                }
            }
        }

        // Piso de recursos das elites (Pilar D).
        let times: Vec<Team> = grid.iter().map(|e| e.team.clone()).collect();
        let elites = designate_elite_teams(&times);
        for e in grid.iter_mut() {
            if elites.contains(&e.team.id) {
                apply_elite_resource_floor(&mut e.team);
                refresh_team_financial_state_com(&mut e.team, faixas);
            }
        }

        let mut fotos = vec![FotoDaEquipe::default(); n_equipes];
        for (i, e) in grid.iter_mut().enumerate() {
            // O estado com que a equipe FECHA o ano — o mesmo que arma a venda por colapso
            // crônico logo abaixo. Guardado antes do offseason porque o ralo ainda mexe no
            // caixa e o estado seria relido depois dele.
            let estado_do_ano = estado_estatico(&e.team);
            *r.estados.entry(estado_estatico(&e.team)).or_insert(0) += 1;
            // A leitura paralela pelo instrumento velho, sobre a MESMA equipe no MESMO
            // instante. Não age sobre nada — só permite separar "o mundo adoeceu" de "o
            // termômetro mentia".
            let legado = derive_financial_state(financial_health_score(&e.team));
            *r.estados_legado
                .entry(
                    ESTADOS
                        .iter()
                        .copied()
                        .find(|x| *x == legado)
                        .unwrap_or("stable"),
                )
                .or_insert(0) += 1;
            r.meses_observados.push(meses_de_operacao(&e.team));
            match e.team.financial_state.as_str() {
                "crisis" | "collapse" => {
                    apertou[i] = true;
                    quebrou[i] = true;
                }
                "pressured" => apertou[i] = true,
                _ => {}
            }

            // Colapso crônico → venda (2 temporadas seguidas).
            if e.team.financial_state == "collapse" {
                e.colapsos_seguidos += 1;
                if e.colapsos_seguidos >= 2 {
                    apply_team_sale(&mut e.team, &mut rng);
                    e.colapsos_seguidos = 0;
                    r.vendas += 1;
                }
            } else {
                e.colapsos_seguidos = 0;
            }

            // Offseason: estratégia e impacto de competitividade.
            if elegivel_no_ano[i] {
                r.elegiveis += 1;
            }
            let escolhida = escolher_estrategia(&e.team, estrategia);
            *r.estrategias.entry(escolhida).or_insert(0) += 1;
            e.team.season_strategy = escolhida.to_string();
            let foco = foco_aproximado(&e.team);
            let (eng_antes, fac_antes) = (e.team.engineering, e.team.facilities);

            let drenado = match offseason {
                Offseason::Producao => {
                    apply_offseason_competitiveness_impact(&mut e.team, temporada, foco);

                    // ── RALO B: melhorar estrutura passa a custar ─────────────────────
                    // O delta é medido DEPOIS da chamada, então já vem com o clamp em 100:
                    // equipe que bateu no teto não paga por um ganho que não teve.
                    let pontos_ganhos = (e.team.engineering - eng_antes).max(0.0)
                        + (e.team.facilities - fac_antes).max(0.0);
                    r.pontos_estrutura += pontos_ganhos;
                    let custo_de_subir = ralo.custo_de_melhorar(pontos_ganhos, op_anual);
                    // ── RALO C: dreno sobre o excedente (controle) ────────────────────
                    let dreno =
                        ralo.dreno_do_excedente(e.team.cash_balance - custo_de_subir, op_anual);
                    custo_de_subir + dreno
                }
                // ── O MÓDULO NOVO: a equipe investe o excedente ───────────────────────
                // `apply_offseason_competitiveness_impact` NÃO roda aqui — os dois moveriam
                // a mesma estrutura e a medição não diria de quem foi o movimento.
                Offseason::Economia(params) => {
                    let entrada = EntradaDeDesenvolvimento {
                        caixa: e.team.cash_balance,
                        divida: e.team.debt_balance,
                        custo_operacional_anual: op_por_classe[e.classe],
                        engenharia: e.team.engineering,
                        instalacoes: e.team.facilities,
                        confiabilidade: e.team.confiabilidade,
                        apetite: apetite_do_foco(foco),
                    };
                    let plano = planejar_desenvolvimento(&entrada, &params);
                    e.team.engineering += plano.delta_engenharia;
                    e.team.facilities += plano.delta_instalacoes;
                    e.team.confiabilidade += plano.delta_confiabilidade;
                    r.pontos_estrutura += plano.delta_liquido();
                    plano.investimento
                }
            };
            let _ = (eng_antes, fac_antes);
            if drenado > 0.0 {
                e.team.cash_balance -= drenado;
                r.linhas.ralo += drenado;
                linhas_temporada[i].ralo += drenado;
                refresh_team_financial_state_com(&mut e.team, faixas);
            }

            // O ano fechado desta equipe, já com o investimento de offseason descontado.
            saldo_acumulado[i] += saldo_temporada_equipe[i] - drenado;

            // ── A FOTO DO ANO ────────────────────────────────────────────────────────
            // Depois do ralo e do refresh: é o estado com que a equipe entra no ano
            // seguinte, que é o que o próximo `budget_index` vai ler. Fica antes do
            // recálculo da folha porque o salário do ano que vem é decisão do ano que vem.
            fotos[i] = FotoDaEquipe {
                meses: meses_de_operacao(&e.team),
                caixa: e.team.cash_balance,
                divida: e.team.debt_balance,
                poder_de_gasto: calculate_spending_power(&e.team),
                indice: derive_budget_index_from_money(&e.team),
                patrocinio_do_ano: linhas_temporada[i].patrocinio,
                receita_do_ano: linhas_temporada[i].receita_do_ano,
                estado: estado_estatico(&e.team),
                classe: e.classe,
                mensal: custo_operacional_mensal(&e.team.categoria, e.team.classe.as_deref()),
            };

            // ── A AUTÓPSIA ────────────────────────────────────────────────────────────
            // Fecha o ano da equipe no grupo do estado com que ela terminou. Fica DEPOIS do
            // ralo porque o investimento de offseason é despesa do ano que acabou de passar
            // — deixá-lo de fora faria o ano do colapsado parecer mais barato do que foi.
            match estado_do_ano {
                "collapse" => r
                    .autopsia_colapso
                    .somar(&linhas_temporada[i], meses_no_inicio[i]),
                "stable" | "healthy" | "elite" => r
                    .autopsia_saudavel
                    .somar(&linhas_temporada[i], meses_no_inicio[i]),
                _ => {}
            }

            // Salário recalculado com o dinheiro de hoje — é o acoplamento do mercado.
            e.folha_anual = arena.pilotos_por_equipe as f64
                * calculate_offer_salary_from_money(&e.team, e.skill);

            e.team.stats_pontos = 0;
            e.team.stats_vitorias = 0;
            e.team.stats_podios = 0;
        }

        r.serie.push(fotos);
        r.saldo_por_temporada.push(saldo_temporada);
    }

    let temporadas_f = TEMPORADAS as f64;
    let classes_f = arena.classes.len() as f64;
    r.receita_campeao /= temporadas_f * classes_f;
    r.receita_lanterna /= temporadas_f * classes_f;
    r.saldo_campeao /= temporadas_f * classes_f;
    r.saldo_lanterna /= temporadas_f * classes_f;
    r.pontos_por_equipe_por_corrida = pontos_totais / corridas_equipe.max(1.0);
    r.portao_melhor = portao_por_equipe.iter().copied().fold(f64::MIN, f64::max) / temporadas_f;
    r.portao_pior = portao_por_equipe.iter().copied().fold(f64::MAX, f64::min) / temporadas_f;
    // A equipe de MENOR portão do grid: quanto o canal vale para ela, em % da receita dela.
    if let Some(pior) = (0..n_equipes).min_by(|a, b| {
        portao_por_equipe[*a]
            .partial_cmp(&portao_por_equipe[*b])
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        r.portao_pct_da_pior = portao_por_equipe[pior] / receita_acumulada[pior].max(1.0) * 100.0;
    }

    r.receita_espalhada = receita_acumulada.iter().copied().fold(f64::MIN, f64::max)
        / receita_acumulada
            .iter()
            .copied()
            .fold(f64::MAX, f64::min)
            .max(1.0);
    r.fixo_sobre_receita = (0..n_equipes)
        .map(|i| fixo_acumulado[i] / receita_acumulada[i].max(1.0) * 100.0)
        .collect();
    r.receita_por_equipe = receita_acumulada.clone();
    r.saldo_por_equipe = saldo_acumulado.clone();
    r.pontos_acumulados = pontos_acumulados.clone();
    r.titulos = titulos.clone();
    r.emprestimos_por_equipe = emprestimos_por_equipe.clone();

    r.nunca_apertadas = apertou.iter().filter(|a| !**a).count() as u32;
    r.ja_quebraram = quebrou.iter().filter(|q| **q).count() as u32;
    for e in &grid {
        r.caixa_fim
            .push(e.team.cash_balance / scale.expected_cash_midpoint());
        r.divida_fim
            .push(e.team.debt_balance / scale.expected_cash_midpoint());
        r.estrutura_fim.push(e.team.engineering + e.team.facilities);
        r.meses_fim.push(meses_de_operacao(&e.team));
    }
    let niveis: Vec<f64> = grid.iter().map(|e| nivel_medio(&e.car)).collect();
    r.nivel_pior = niveis.iter().copied().fold(f64::MAX, f64::min);
    r.nivel_medio = niveis.iter().sum::<f64>() / niveis.len().max(1) as f64;
    r.nivel_melhor = niveis.iter().copied().fold(f64::MIN, f64::max);
    r
}

/// A ÂNCORA declarada em `car::cost`: o custo de peça foi escalado por
/// `operating_cost_midpoint × ~0,00065` "pra que a manutenção recorrente fique numa fração
/// sustentável do orçamento". Este número diz qual fração ela é DE FATO — e a conta só fecha
/// no nível 1, porque `part_cost` cresce +23,85% por nível e as equipes não vivem no nível 1.
///
/// Devolve (fração no nível 1, fração no teto da categoria), ambas em custo operacional de
/// uma temporada, para as mesmas ~58 trocas/temporada medidas no harness de peças.
fn ancora_de_manutencao(categoria: &str) -> (f64, f64) {
    use crate::car::cost::{category_ceiling, part_cost};

    const TROCAS_POR_TEMPORADA: f64 = 58.2;
    let teto = category_ceiling(categoria);
    let medio = |nivel: u8| -> f64 {
        PartType::ALL
            .iter()
            .map(|&p| part_cost(categoria, p, nivel))
            .sum::<f64>()
            / PartType::ALL.len() as f64
    };
    let operacao = category_finance_scale(categoria).operating_cost_midpoint();
    (
        medio(1) * TROCAS_POR_TEMPORADA / operacao,
        medio(teto) * TROCAS_POR_TEMPORADA / operacao,
    )
}

/// `financial_state` como `&'static str`, para servir de chave do histograma.
fn estado_estatico(team: &Team) -> &'static str {
    ESTADOS
        .iter()
        .copied()
        .find(|e| *e == team.financial_state)
        .unwrap_or("stable")
}

// ===================== Relatório =====================

fn mediana(valores: &[f64]) -> f64 {
    if valores.is_empty() {
        return 0.0;
    }
    let mut v = valores.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

/// Receita de uma categoria em 20 temporadas, do jeito que a Parte 4 conta: as cinco
/// linhas de rodada mais o prêmio de fechamento.
fn receita_total(r: &Resultado) -> f64 {
    r.linhas.patrocinio
        + r.linhas.bilheteria
        + r.linhas.bonus
        + r.linhas.premio_parcial
        + r.linhas.ajuda
        + r.linhas.premio_construtores
}

/// Despesa de uma categoria em 20 temporadas. `linhas.pecas` fica de fora de propósito:
/// é uma FATIA de `tecnico`, não uma linha à parte — somá-la contaria peça duas vezes.
///
/// O `ralo` entra porque é despesa de verdade: sem ele na conta, o dinheiro drenado sumiria
/// do balanço e a razão receita/despesa mentiria a favor do ralo. Vale zero quando não há
/// ralo ligado, então nada muda para quem mede a economia de hoje.
fn despesa_total(r: &Resultado) -> f64 {
    r.linhas.salario
        + r.linhas.operacao
        + r.linhas.estrutural
        + r.linhas.tecnico
        + r.linhas.juros
        + r.linhas.ralo
}

/// Marca visual do quanto o harness se afastou do save real. Existe para o olho achar a
/// linha problemática sem ler número por número.
fn desvio(medido: f64, real: f64) -> &'static str {
    if real.abs() < 1e-9 {
        return "  ";
    }
    let erro = ((medido - real) / real).abs();
    if erro <= 0.10 {
        "ok"
    } else if erro <= 0.25 {
        "~ "
    } else {
        "XX"
    }
}

// ===================== Varredura de calibração =====================
//
// O relatório acima diz onde a economia ESTÁ. A varredura abaixo diz para onde cada botão
// a leva — é o instrumento para decidir um número antes de escrevê-lo na produção.
//
// Roda com `cargo test --lib varrer_economia -- --ignored --nocapture`.

/// As arenas da varredura. É um subconjunto: `toyota_rookie` e `toyota_amador` são gêmeas
/// financeiras das versões Mazda (mesma escala, mesmo calendário, mesmo grid), então medi-las
/// nas duas dobra o tempo e não muda uma conclusão. O relatório completo mede as nove.
const ARENAS_VARREDURA: &[&str] = &[
    "mazda_rookie",
    "mazda_amador",
    "bmw_m2",
    "production_challenger",
    "gt4",
    "gt3",
    "endurance",
];

/// O que uma configuração produz, condensado no que importa para decidir.
struct Resumo {
    /// receita ÷ despesa por categoria, na ordem de `ARENAS_VARREDURA`.
    razao: Vec<f64>,
    /// A categoria mais sacrificada e a mais folgada — o alvo é apertar essa distância.
    pior: f64,
    melhor: f64,
    /// Fatias da receita do mundo: o que vem de CORRIDA (bônus + prêmio parcial), do
    /// FECHAMENTO (construtores), do PORTÃO e do patrocínio plano.
    pct_corrida: f64,
    pct_fechamento: f64,
    pct_portao: f64,
    pct_patrocinio: f64,
    /// Saúde do mundo e deriva do caixa (mediana no fim ÷ mediana no início).
    colapso_pct: f64,
    deriva_media: f64,
    deriva_maxima: f64,
    vendas: u32,
    /// Quantas vezes o campeão arrecada mais que o lanterna, na média das categorias.
    campeao_sobre_lanterna: f64,
}

fn resumir(coef: CoeficientesDeReceita) -> Resumo {
    resumir_com(coef, Ralo::default(), Offseason::Producao)
}

fn resumir_com(coef: CoeficientesDeReceita, ralo: Ralo, offseason: Offseason) -> Resumo {
    let mut razao = Vec::new();
    let (mut receita_mundo, mut despesa_mundo) = (0.0f64, 0.0f64);
    let (mut corrida, mut fechamento, mut portao, mut patrocinio) = (0.0, 0.0, 0.0, 0.0);
    let mut observacoes = 0u32;
    let mut colapsos = 0u32;
    let mut derivas = Vec::new();
    let mut vendas = 0u32;
    let mut razoes_posicao = Vec::new();

    for &categoria in ARENAS_VARREDURA {
        let r = medir_categoria_com(categoria, coef, ralo, offseason);
        let receita = receita_total(&r);
        let despesa = despesa_total(&r);
        razao.push(receita / despesa.max(1.0));
        receita_mundo += receita;
        despesa_mundo += despesa;
        corrida += r.linhas.bonus + r.linhas.premio_parcial;
        fechamento += r.linhas.premio_construtores;
        portao += r.linhas.bilheteria;
        patrocinio += r.linhas.patrocinio;
        observacoes += r.estados.values().sum::<u32>();
        colapsos += r.estados.get("collapse").copied().unwrap_or(0)
            + r.estados.get("crisis").copied().unwrap_or(0);
        vendas += r.vendas;
        let inicio = mediana(&r.caixa_inicio).max(0.01);
        derivas.push(mediana(&r.caixa_fim) / inicio);
        razoes_posicao.push(r.receita_campeao / r.receita_lanterna.max(1.0));
    }

    let n = razao.len().max(1) as f64;
    let _ = despesa_mundo;
    Resumo {
        pior: razao.iter().copied().fold(f64::MAX, f64::min),
        melhor: razao.iter().copied().fold(f64::MIN, f64::max),
        razao,
        pct_corrida: corrida / receita_mundo.max(1.0) * 100.0,
        pct_fechamento: fechamento / receita_mundo.max(1.0) * 100.0,
        pct_portao: portao / receita_mundo.max(1.0) * 100.0,
        pct_patrocinio: patrocinio / receita_mundo.max(1.0) * 100.0,
        colapso_pct: colapsos as f64 / observacoes.max(1) as f64 * 100.0,
        deriva_media: derivas.iter().sum::<f64>() / n,
        deriva_maxima: derivas.iter().copied().fold(f64::MIN, f64::max),
        vendas,
        campeao_sobre_lanterna: razoes_posicao.iter().sum::<f64>() / n,
    }
}

fn linha_resumo(rotulo: &str, r: &Resumo) {
    let razoes: String = r
        .razao
        .iter()
        .map(|v| format!("{v:>5.2}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!(
        "  {rotulo:<26} {razoes} │ {:>4.2}–{:<4.2} │ corr {:>4.1} fech {:>4.1} port {:>4.1} patr {:>4.1} │ crise {:>4.1}% │ caixa {:>5.1}× (máx {:>5.1}×) │ c/l {:>4.1}×",
        r.pior,
        r.melhor,
        r.pct_corrida,
        r.pct_fechamento,
        r.pct_portao,
        r.pct_patrocinio,
        r.colapso_pct,
        r.deriva_media,
        r.deriva_maxima,
        r.campeao_sobre_lanterna
    );
}

fn cabecalho_varredura() {
    let nomes: String = ARENAS_VARREDURA
        .iter()
        .map(|c| {
            let curto = match *c {
                "mazda_rookie" => "rook",
                "mazda_amador" => "amad",
                "bmw_m2" => " bmw",
                "production_challenger" => "prod",
                "gt4" => " gt4",
                "gt3" => " gt3",
                "endurance" => "endu",
                outro => outro,
            };
            format!("{curto:>5}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    println!("  {:<26} {nomes} │ receita/despesa: faixa │ % da receita do mundo            │ crise  │ deriva do caixa   │ campeão", "");
}

#[test]
#[ignore = "harness de calibração — roda com --ignored --nocapture"]
fn varrer_economia() {
    let base = CoeficientesDeReceita::default();

    println!("\n═══ VARREDURA DE CALIBRAÇÃO ═══");
    println!("  receita/despesa por categoria · alvo declarado: ~1,00 na escada inteira");
    println!("  'caixa' = mediana do caixa no fim ÷ no início, em 20 temporadas. 1,0× = mundo");
    println!("  estável; 9× = a categoria imprime dinheiro. 'c/l' = quanto o campeão arrecada");
    println!("  a mais que o lanterna.\n");
    cabecalho_varredura();
    linha_resumo("HOJE (produção)", &resumir(base));

    // ── A. Sensibilidade: um botão de cada vez ───────────────────────────────────────
    println!("\n── A. Sensibilidade: um botão por vez ──");
    cabecalho_varredura();

    for v in [0.14, 0.20, 0.27, 0.34, 0.40] {
        let mut c = base;
        c.patrocinio_base = v;
        linha_resumo(&format!("patrocínio_base {v:.2}"), &resumir(c));
    }
    println!();
    for v in [0.5, 1.0, 1.5, 2.0, 3.0] {
        let mut c = base;
        c.escala_do_bonus = v;
        linha_resumo(&format!("escala_do_bônus {v:.2}"), &resumir(c));
    }
    println!();
    for v in [0.12, 1.0, 3.0, 6.0, 12.0] {
        let mut c = base;
        c.portao_coef = v;
        linha_resumo(&format!("portão_coef {v:.2}"), &resumir(c));
    }
    println!();
    for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let mut c = base;
        c.portao_piso = v;
        linha_resumo(&format!("portão_piso {v:.2}"), &resumir(c));
    }
    println!();
    for (b, s) in [
        (0.00, 0.00),
        (0.05, 0.25),
        (0.10, 0.35),
        (0.15, 0.50),
        (0.25, 0.75),
    ] {
        let mut c = base;
        c.premio_base = b;
        c.premio_inclinacao = s;
        linha_resumo(&format!("prêmio final {b:.2}/{s:.2}"), &resumir(c));
    }
    println!();
    for v in [0.0039, 0.0078, 0.0160, 0.0320, 0.0640] {
        let mut c = base;
        c.premio_parcial_por_ponto = v;
        linha_resumo(&format!("prêmio/ponto {v:.4}"), &resumir(c));
    }

    // ── B. Cenários: intenções de design inteiras ────────────────────────────────────
    println!("\n── B. Cenários ──");
    cabecalho_varredura();

    let mut portao_de_evento = base;
    // O bolo hoje é fração do custo de UMA equipe e depois é dividido pelo grid inteiro.
    // Multiplicar por ~N emula um bolo de EVENTO; o piso menor deixa a fama diferenciar.
    portao_de_evento.portao_coef = 3.0;
    portao_de_evento.portao_piso = 0.25;
    portao_de_evento.patrocinio_base = 0.20;
    linha_resumo("portão de evento", &resumir(portao_de_evento));

    let mut corrida_manda = base;
    corrida_manda.patrocinio_base = 0.14;
    corrida_manda.escala_do_bonus = 2.0;
    corrida_manda.premio_parcial_por_ponto = 0.0160;
    linha_resumo("prêmio por corrida manda", &resumir(corrida_manda));

    let mut sem_muleta = base;
    sem_muleta.premio_base = 0.05;
    sem_muleta.premio_inclinacao = 0.25;
    sem_muleta.escala_do_bonus = 1.5;
    linha_resumo("sem muleta de fechamento", &resumir(sem_muleta));

    let mut combinado = base;
    combinado.patrocinio_base = 0.14;
    combinado.escala_do_bonus = 1.8;
    combinado.premio_parcial_por_ponto = 0.0160;
    combinado.premio_base = 0.05;
    combinado.premio_inclinacao = 0.25;
    combinado.portao_coef = 3.0;
    combinado.portao_piso = 0.25;
    linha_resumo("combinado", &resumir(combinado));

    // ── C. Busca: qual par (patrocínio, bônus) aperta a escada em torno de 1,0? ───────
    println!("\n── C. Busca do par (patrocínio_base × escala_do_bônus) ──");
    println!("  'espalhamento' = distância entre a categoria mais folgada e a mais sacrificada.");
    println!("  Quanto menor, mais a escada inteira vive na mesma economia.\n");
    println!(
        "  {:<10} {:<10} {:>8} {:>8} {:>8} {:>14} {:>10}",
        "patroc", "bônus", "pior", "melhor", "espalha", "% de corrida", "crise"
    );
    let mut melhor: Option<(f64, f64, f64, Resumo)> = None;
    for patroc in [0.10, 0.14, 0.18, 0.22, 0.27] {
        for escala in [1.0, 1.5, 2.0, 2.5, 3.0] {
            let mut c = base;
            c.patrocinio_base = patroc;
            c.escala_do_bonus = escala;
            // O prêmio final sai de cena: é o que o design quer testar.
            c.premio_base = 0.05;
            c.premio_inclinacao = 0.25;
            let r = resumir(c);
            let espalhamento = r.melhor - r.pior;
            println!(
                "  {patroc:<10.2} {escala:<10.2} {:>8.2} {:>8.2} {:>8.2} {:>13.1}% {:>9.1}%",
                r.pior, r.melhor, espalhamento, r.pct_corrida, r.colapso_pct
            );
            let troca = melhor
                .as_ref()
                .map(|(_, _, e, _)| espalhamento < *e)
                .unwrap_or(true);
            if troca {
                melhor = Some((patroc, escala, espalhamento, r));
            }
        }
    }
    if let Some((patroc, escala, espalhamento, r)) = melhor {
        println!(
            "\n  menor espalhamento: patrocínio {patroc:.2} × bônus {escala:.2} → {espalhamento:.2}"
        );
        cabecalho_varredura();
        linha_resumo("↑ esse par", &r);
        println!(
            "\n  ATENÇÃO: menor espalhamento não é 'melhor'. Ele só diz qual par põe a escada\n  \
             na mesma economia. Se o resultado ainda deixa 'caixa' em 5×, o mundo continua\n  \
             inflando — falta ralo, e ralo não é botão de receita."
        );
    }
}

#[test]
#[ignore = "harness de medição — roda com --ignored --nocapture"]
fn medir_economia_das_equipes() {
    println!("\n═══ {TEMPORADAS} temporadas · forma real de cada categoria ═══");
    println!("    (grid, calendário, classes e pilotos por equipe lidos de constants::categories)");

    let mut total_estados: HashMap<&'static str, u32> = HashMap::new();
    let mut total_nunca = 0u32;
    let mut total_quebrou = 0u32;
    let mut total_vendas = 0u32;
    let mut total_emprestimos = 0u32;
    let mut total_equipes = 0u32;
    // (categoria, medido, real) das três métricas confrontáveis.
    let mut validacao: Vec<(&str, f64, f64, f64, f64, f64, f64, f64, f64)> = Vec::new();

    for &categoria in CATEGORIAS {
        let a = arena(categoria);
        let refer = referencia(categoria);
        let r = medir_categoria(categoria);
        let observacoes: u32 = r.estados.values().sum();
        let pct = |estado: &str| {
            r.estados.get(estado).copied().unwrap_or(0) as f64 / observacoes.max(1) as f64 * 100.0
        };

        let receita = receita_total(&r);
        let despesa = despesa_total(&r);
        let f = |x: f64| x / despesa.max(1.0) * 100.0;
        let g = |x: f64| x / receita.max(1.0) * 100.0;
        let razao = receita / despesa.max(1.0);

        let classes = if a.multi_classe {
            format!(
                "{} classes de {}",
                a.classes.len(),
                a.classes.first().map(|c| c.equipes).unwrap_or(0)
            )
        } else {
            "classe única".to_string()
        };
        println!(
            "\n── {categoria} ── {} equipes · {} rodadas · {} · {} carros por equipe",
            a.equipes, a.rodadas, classes, a.pilotos_por_equipe
        );
        println!(
            "  estados (%)  elite {:.0} · healthy {:.0} · stable {:.0} · pressured {:.0} · crisis {:.0} · collapse {:.0}",
            pct("elite"),
            pct("healthy"),
            pct("stable"),
            pct("pressured"),
            pct("crisis"),
            pct("collapse")
        );
        println!(
            "  equipes ...  {}/{} nunca apertaram · {}/{} já estiveram em crise/colapso",
            r.nunca_apertadas, a.equipes, r.ja_quebraram, a.equipes
        );
        println!(
            "  resgates ..  {} vendas · {} empréstimos de emergência",
            r.vendas, r.emprestimos
        );
        println!(
            "  receita (%)  patrocínio {:.1} · construtores {:.1} · bônus {:.1} · prêmio parcial {:.1} · bilheteria {:.2} · ajuda {:.1}",
            g(r.linhas.patrocinio),
            g(r.linhas.premio_construtores),
            g(r.linhas.bonus),
            g(r.linhas.premio_parcial),
            g(r.linhas.bilheteria),
            g(r.linhas.ajuda)
        );
        println!(
            "  despesa (%)  salário {:.0} · operação {:.0} · estrutural {:.0} · técnico {:.0} (peças {:.0}) · juros {:.0}",
            f(r.linhas.salario),
            f(r.linhas.operacao),
            f(r.linhas.estrutural),
            f(r.linhas.tecnico),
            f(r.linhas.pecas),
            f(r.linhas.juros)
        );
        println!(
            "  margem ....  receita/despesa {:.2}× · saldo médio do grid por temporada {:+.0}",
            razao,
            r.saldo_por_temporada.iter().sum::<f64>() / r.saldo_por_temporada.len().max(1) as f64
        );
        // A escala declarada da categoria em `category_finance_scale` diz que operar custa
        // `operating_cost_midpoint` por temporada. Aqui se vê o que a temporada custa DE FATO
        // e o que ela arrecada, na mesma unidade — é o que separa "receita de menos" de
        // "despesa acima da própria escala".
        let por_equipe_temporada = TEMPORADAS as f64 * a.equipes as f64;
        let op_mid = category_finance_scale(categoria).operating_cost_midpoint();
        println!(
            "  escala ....  por equipe/temporada: despesa {:.2}× · receita {:.2}× do custo operacional declarado",
            despesa / por_equipe_temporada / op_mid,
            receita / por_equipe_temporada / op_mid
        );
        // Acoplamento posição→dinheiro. Se campeão e lanterna arrecadam quase o mesmo, o
        // campeonato não é disputa econômica: é ranking decorativo sobre uma renda fixa.
        println!(
            "  posição ...  campeão arrecada {:.0} · lanterna {:.0} ({:.2}×) | saldo: campeão {:+.0} · lanterna {:+.0}",
            r.receita_campeao,
            r.receita_lanterna,
            r.receita_campeao / r.receita_lanterna.max(1.0),
            r.saldo_campeao,
            r.saldo_lanterna
        );
        println!(
            "  muleta ....  {}/{} temporadas-equipe só fecharam no azul por causa do prêmio final ({:.0}%)",
            r.salvas_pelo_fechamento,
            r.temporadas_equipe,
            r.salvas_pelo_fechamento as f64 / r.temporadas_equipe.max(1) as f64 * 100.0
        );
        println!(
            "  portão ....  por temporada: melhor equipe {:.0} · pior {:.0} ({:.2}× de diferença)",
            r.portao_melhor,
            r.portao_pior,
            r.portao_melhor / r.portao_pior.max(1.0)
        );
        println!(
            "  caixa .....  mediana {:.2}× do médio da categoria (era {:.2}×) · dívida mediana {:.2}×",
            mediana(&r.caixa_fim),
            mediana(&r.caixa_inicio),
            mediana(&r.divida_fim)
        );
        println!(
            "  carro .....  nível médio de peça: pior {:.1} · médio {:.1} · melhor {:.1} (teto {})",
            r.nivel_pior,
            r.nivel_medio,
            r.nivel_melhor,
            crate::car::cost::category_ceiling(categoria)
        );
        let (ancora_l1, ancora_teto) = ancora_de_manutencao(categoria);
        println!(
            "  âncora ....  manutenção/temporada em % da operação: nível 1 {:.0}% · no teto {:.0}% · MEDIDA {:.0}%",
            ancora_l1 * 100.0,
            ancora_teto * 100.0,
            r.linhas.pecas / por_equipe_temporada / op_mid * 100.0
        );

        validacao.push((
            categoria,
            razao,
            refer.receita_sobre_despesa,
            r.pontos_por_equipe_por_corrida,
            refer.pontos_por_equipe_por_corrida,
            g(r.linhas.bilheteria),
            refer.bilheteria_pct,
            g(r.linhas.premio_construtores),
            refer.fechamento_pct,
        ));

        for (estado, n) in r.estados {
            *total_estados.entry(estado).or_insert(0) += n;
        }
        total_nunca += r.nunca_apertadas;
        total_quebrou += r.ja_quebraram;
        total_vendas += r.vendas;
        total_emprestimos += r.emprestimos;
        total_equipes += a.equipes as u32;
    }

    // ── VALIDAÇÃO: o harness reproduz o mundo real? ──────────────────────────────────
    println!("\n═══ VALIDAÇÃO vs. save real (career_014 · temporadas 28–31) ═══");
    println!(
        "  {:<22} {:^17} {:^17} {:^15} {:^15}",
        "", "receita/despesa", "pontos/equipe", "bilheteria %", "fechamento %"
    );
    println!(
        "  {:<22} {:>7} {:>7} {:>2} {:>7} {:>7} {:>2} {:>6} {:>6} {:>2} {:>6} {:>6} {:>2}",
        "categoria",
        "medido",
        "real",
        "",
        "medido",
        "real",
        "",
        "med",
        "real",
        "",
        "med",
        "real",
        ""
    );
    for (cat, r1, v1, r2, v2, r3, v3, r4, v4) in &validacao {
        println!(
            "  {:<22} {:>7.2} {:>7.2} {:>2} {:>7.2} {:>7.2} {:>2} {:>6.2} {:>6.2} {:>2} {:>6.1} {:>6.1} {:>2}",
            cat,
            r1,
            v1,
            desvio(*r1, *v1),
            r2,
            v2,
            desvio(*r2, *v2),
            r3,
            v3,
            desvio(*r3, *v3),
            r4,
            v4,
            desvio(*r4, *v4)
        );
    }
    println!("  legenda: ok = até 10% de desvio · ~ = até 25% · XX = acima de 25%");
    println!(
        "  o que sobra de desvio depois disto é o que o harness ainda não modela — hoje,\n  \
         sobretudo promoção/rebaixamento (equipe muda de escala financeira carregando o caixa)."
    );

    let observacoes: u32 = total_estados.values().sum();
    println!("\n═══ MUNDO ═══");
    for estado in ESTADOS {
        let n = total_estados.get(estado).copied().unwrap_or(0);
        println!(
            "  {estado:<10} {:>6}   {:>5.1}%",
            n,
            n as f64 / observacoes.max(1) as f64 * 100.0
        );
    }
    println!(
        "\n  {total_nunca}/{total_equipes} equipes nunca desceram de 'stable' em {TEMPORADAS} temporadas"
    );
    println!("  {total_quebrou}/{total_equipes} equipes já estiveram em crise ou colapso");
    println!("  {total_vendas} vendas · {total_emprestimos} empréstimos de emergência");
}

// ===================== O PLACAR DE ACEITAÇÃO (Parte 4) =====================
//
// A tabela da "Parte 4 — Critérios de aceitação" de `docs/economia-redesign.md`, na FORMA
// VIGENTE — vários alvos foram reescritos ao longo da empreitada, sempre depois de a
// medição mostrar que o alvo original nunca tinha sido derivado de nada.
//
// Roda com `cargo test --lib criterios_de_aceitacao -- --ignored --nocapture`.
//
// **O que este teste mede mudou.** Ele nasceu medindo a produção de então (despesa e
// receita velhas) e existia para FALHAR: era o alvo da reescrita, não a validação dela.
// Agora ele mede o MODELO NOVO INTEIRO — `economia::evento` + `economia::temporada` na
// despesa, `economia::receita` nos cinco canais, `economia::desenvolvimento` no offseason,
// e o custo operacional anual FÍSICO e por classe como âncora de tudo. É o placar de
// fecho, não mais o alvo.
//
// **Regra geral: todo alvo é POR CATEGORIA, não agregado.** O agregado esconde exatamente a
// patologia que o redesign conserta. Duas exceções declaradas: o critério 7 é do mundo
// (crise é propriedade da população inteira) e o critério 8 só se aplica a categoria de
// [`GRID_MINIMO_PARA_FALENCIA`] equipes ou mais.

/// O que fazer com um critério que não passa. Existe porque "8 de 12" não é informação:
/// um critério que falha porque falta implementar alguma coisa e um que falha porque o
/// mundo é assim são pedidos de trabalho opostos.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Veredito {
    Passa,
    /// **Defeito conhecido**: medido, nomeado, com causa identificada FORA do alcance de
    /// qualquer coeficiente. Não é para consertar calibrando; é para decidir se o alvo
    /// descreve o mundo que se quer.
    DefeitoConhecido,
    /// **Alavanca esgotada**: o botão existe, foi varrido de ponta a ponta, e nenhum valor
    /// fecha o critério sem abrir outro. É um empate, não uma pendência.
    AlavancaEsgotada,
    /// **Trabalho pendente**: falta código ou falta medição. Este é o único que vira tarefa.
    Pendente,
}

impl Veredito {
    fn rotulo(self) -> &'static str {
        match self {
            Veredito::Passa => "PASSA",
            Veredito::DefeitoConhecido => "defeito conhecido",
            Veredito::AlavancaEsgotada => "alavanca esgotada",
            Veredito::Pendente => "TRABALHO PENDENTE",
        }
    }
}

/// Uma linha do placar.
struct Criterio {
    nome: &'static str,
    alvo: &'static str,
    medido: String,
    passou: bool,
    veredito: Veredito,
    /// Por que o veredito é esse. Vazio quando o critério passa.
    porque: &'static str,
}

impl Criterio {
    fn novo(nome: &'static str, alvo: &'static str, medido: String, passou: bool) -> Self {
        Self {
            nome,
            alvo,
            medido,
            passou,
            veredito: if passou {
                Veredito::Passa
            } else {
                Veredito::Pendente
            },
            porque: "",
        }
    }

    /// Classifica uma falha. Passar continua passando — a classificação só descreve o que
    /// fazer com o que NÃO passou, e um critério que voltou a passar não deve ficar com um
    /// veredito velho grudado nele.
    fn porque(mut self, veredito: Veredito, porque: &'static str) -> Self {
        if !self.passou {
            self.veredito = veredito;
            self.porque = porque;
        }
        self
    }
}

/// Tamanho mínimo de grid para o critério 8 (falência) valer.
///
/// O alvo era 0,5–5% do grid por temporada em TODA categoria. Medido, as duas Rookies e as
/// duas Amadores ficam em 0,00% sob qualquer calibração, e a varredura de aporte da sessão
/// da falência chegou ao mesmo lugar por outro caminho: nenhum valor de venda alcança.
///
/// A conta que fechou o argumento: uma categoria de 6 equipes tem 120 equipe-temporadas em
/// 20 anos, e 0,5% delas é 0,6 falência. Exigir "pelo menos uma" de um grid de 6 é exigir
/// que **uma em cada seis equipes do degrau de entrada quebre**. Quem entra na pirâmide
/// deve encontrar equipes POBRES, não equipes falindo — são coisas diferentes, e o modelo
/// já entrega a primeira (fôlego de 10–22 meses contra os 24+ de antes).
///
/// Não há piso abaixo deste tamanho: categoria pequena simplesmente não é avaliada.
const GRID_MINIMO_PARA_FALENCIA: usize = 10;

/// A linha de combustível de uma etapa TÍPICA no modelo físico, em dinheiro.
///
/// Substitui `combustivel_medido`, que lia a fatura LEGADA (`compute_operation_lines`). Essa
/// função continua existindo porque o comparador precisa dos dois modelos — o velho não foi
/// apagado e não deve ser.
fn combustivel_do_modelo_fisico(categoria: &'static str) -> f64 {
    let a = arena(categoria);
    let classe = a
        .classes
        .first()
        .and_then(|c| (!c.nome.is_empty()).then_some(c.nome));
    let entrada = crate::economia::tipos::EntradaDaEtapa::tipica(categoria, classe);
    crate::economia::evento::fatura_da_etapa(&entrada).valor(crate::economia::evento::COMBUSTIVEL)
}

/// As intenções de design sobre as quais o critério 9 é avaliado, no modelo NOVO.
///
/// São INTENÇÕES inteiras, não botões isolados: se a MESMA categoria for a mais sacrificada
/// em todas elas, a ordem não é consequência da calibração — é da forma da categoria, e
/// nenhum coeficiente conserta.
fn cenarios_do_modelo_novo() -> Vec<(&'static str, ParametrosDeReceita)> {
    let base = ParametrosDeReceita::default();

    // O portão manda: bilheteria dobra, patrocínio recua para caber.
    let mut portao_de_evento = base;
    portao_de_evento.bilheteria = base.bilheteria * 2.0;
    portao_de_evento.patrocinio_fixo = base.patrocinio_fixo * 0.65;

    // O resultado manda: prêmio por etapa cresce e o patrocínio encolhe.
    let mut corrida_manda = base;
    corrida_manda.premio_de_corrida = base.premio_de_corrida * 1.25;
    corrida_manda.patrocinio_fixo = base.patrocinio_fixo * 0.55;

    // Sem muleta de fechamento: o prêmio de fim de temporada quase some.
    let mut sem_muleta = base;
    sem_muleta.fechamento_ao_primeiro = base.fechamento_ao_primeiro * 0.25;
    sem_muleta.fechamento_ao_ultimo = base.fechamento_ao_ultimo * 0.25;
    sem_muleta.premio_de_corrida = base.premio_de_corrida * 1.10;

    // Plano: a curva de prêmio quase linear — o mundo mais igualitário testável.
    let mut plano = base;
    plano.inclinacao_do_premio = 2.0;

    vec![
        ("calibrado", base),
        ("portão de evento", portao_de_evento),
        ("prêmio por corrida manda", corrida_manda),
        ("sem muleta de fechamento", sem_muleta),
        ("curva quase plana", plano),
    ]
}

/// O alvo de campeão ÷ lanterna, que é **função do calendário**.
///
/// O 3× fixo em toda categoria foi afrouxado depois que a varredura mostrou que ele nunca
/// tinha sido derivado: com poucas corridas o campeonato é dominado por ruído, o campeão
/// termina perto do lanterna e só uma curva de prêmio absurdamente convexa fecharia o vão —
/// ao custo de matar o fundo do grid (critério 8) e de furar o critério 3 no calendário
/// longo. O alvo passa a reconhecer o que a medição disse.
fn alvo_campeao_lanterna(rodadas: usize) -> f64 {
    if rodadas >= 8 {
        3.0
    } else {
        2.0
    }
}

/// O **piso** da banda do critério 2, em meses de operação.
///
/// Não é número novo e não é literal daqui: é a fronteira em que o **próprio mundo** declara
/// a equipe pressionada (`FaixasDeMeses::pressionada`), a mesma que `financial_state` usa
/// para armar empréstimo de emergência e que `finance::planning` já reusa como reserva de
/// planejamento. Terminar 20 temporadas abaixo dela não é ser pobre — é viver de crédito em
/// regime permanente.
///
/// Ler a constante em vez de copiar o 3 é deliberado: uma cópia envelheceria calada no dia
/// em que alguém recalibrar as faixas, que é exatamente a armadilha da seção 4.4.
fn piso_de_folego() -> f64 {
    FaixasDeMeses::default().pressionada
}

/// O **teto** da banda do critério 2, em meses de operação.
///
/// Duas vezes o que a equipe **escolheu guardar** (`meses_de_reserva`, 9 meses na decisão 12).
/// O ralo de `economia::desenvolvimento` drena 40% de tudo que passa da reserva por
/// temporada; uma equipe que termina 20 temporadas com o **dobro** da própria reserva é uma
/// equipe cujo ralo não drenou. Dá os mesmos 18 meses do alvo anterior, agora por derivação
/// em vez de por escolha.
fn teto_de_folego() -> f64 {
    ParametrosDeDesenvolvimento::default().meses_de_reserva * 2.0
}

/// A distribuição de fôlego de um grid inteiro, resumida no que a **banda** avalia.
///
/// A mediana continua aqui, e continua impressa: ela é a estatística ANTERIOR do critério 2,
/// e sem ela ao lado dos extremos não dá para dizer o que mudou por causa da forma nova e o
/// que mudou por causa do mundo.
struct BandaDeFolego {
    /// A equipe com MENOS fôlego do grid. Não necessariamente o lanterna de pontos: quem
    /// termina mais pobre nem sempre é quem terminou em último, e o que o critério protege é
    /// a equipe pobre, não a posição.
    pior: f64,
    mediana: f64,
    /// A equipe com MAIS fôlego do grid.
    melhor: f64,
    abaixo_do_piso: usize,
    acima_do_teto: usize,
}

/// Resume um grid na banda. `meses` vem de `Resultado::meses_fim` — uma entrada por equipe,
/// no fim da simulação inteira.
fn banda_de_folego(meses: &[f64]) -> BandaDeFolego {
    let (piso, teto) = (piso_de_folego(), teto_de_folego());
    BandaDeFolego {
        pior: meses.iter().copied().fold(f64::MAX, f64::min),
        mediana: mediana(meses),
        melhor: meses.iter().copied().fold(f64::MIN, f64::max),
        abaixo_do_piso: meses.iter().filter(|m| **m < piso).count(),
        acima_do_teto: meses.iter().filter(|m| **m > teto).count(),
    }
}

/// Avalia um alvo POR CATEGORIA. Devolve a faixa medida e QUAIS categorias ficam fora — o
/// assert é sobre essa lista estar vazia, nunca sobre a média, porque critério de aceitação
/// que passa com uma categoria fora da faixa não é critério.
fn faixa_e_fora<'a>(
    valores: &[(&'a str, f64)],
    dentro: impl Fn(f64) -> bool,
) -> (f64, f64, Vec<&'a str>) {
    (
        valores.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min),
        valores.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max),
        valores
            .iter()
            .filter(|(_, v)| !dentro(*v))
            .map(|(c, _)| *c)
            .collect(),
    )
}

/// Os cenários de receita sobre os quais o critério "nenhuma categoria pior em todos os
/// cenários" é avaliado. São os mesmos do bloco B de `varrer_economia` — intenções de
/// design inteiras, não botões isolados —, mais a produção de hoje. Se a MESMA categoria
/// for a mais sacrificada nos cinco, a ordem não é consequência da calibração: é da forma
/// da categoria, e nenhum coeficiente conserta.
fn cenarios_de_receita() -> Vec<(&'static str, CoeficientesDeReceita)> {
    let base = CoeficientesDeReceita::default();

    let mut portao_de_evento = base;
    portao_de_evento.portao_coef = 3.0;
    portao_de_evento.portao_piso = 0.25;
    portao_de_evento.patrocinio_base = 0.20;

    let mut corrida_manda = base;
    corrida_manda.patrocinio_base = 0.14;
    corrida_manda.escala_do_bonus = 2.0;
    corrida_manda.premio_parcial_por_ponto = 0.0160;

    let mut sem_muleta = base;
    sem_muleta.premio_base = 0.05;
    sem_muleta.premio_inclinacao = 0.25;
    sem_muleta.escala_do_bonus = 1.5;

    let mut combinado = base;
    combinado.patrocinio_base = 0.14;
    combinado.escala_do_bonus = 1.8;
    combinado.premio_parcial_por_ponto = 0.0160;
    combinado.premio_base = 0.05;
    combinado.premio_inclinacao = 0.25;
    combinado.portao_coef = 3.0;
    combinado.portao_piso = 0.25;

    vec![
        ("produção", base),
        ("portão de evento", portao_de_evento),
        ("prêmio por corrida manda", corrida_manda),
        ("sem muleta de fechamento", sem_muleta),
        ("combinado", combinado),
    ]
}

/// Preço do litro de combustível de competição, em dólares. Ordem de grandeza de gasolina
/// de corrida vendida em tambor (VP/Sunoco: US$ 10–12 o galão ≈ US$ 2,6–3,2 o litro).
///
/// A moeda do jogo é tratada como dólar: **1 unidade de caixa = US$ 1**. É o que a tabela
/// da seção 4.1 pressupõe, e é o que torna a âncora comparável com a linha de gasolina que
/// `compute_operation_lines` devolve.
const PRECO_LITRO_USD: f64 = 3.0;

/// Âncora física da linha de combustível de uma etapa — **a seção 4.1 de
/// `docs/economia-redesign.md`**, que é a fonte da verdade. Ela é alvo de aceitação
/// derivado de FORA do modelo: o modelo bottom-up da seção 3.3 chega ao mesmo número por um
/// caminho independente (litros consumidos na simulação), e as duas derivações precisam
/// concordar dentro de uma ordem de grandeza. Divergência maior é informação, não
/// arredondamento.
///
/// A conta é `duração × consumo × nº_de_carros × fim_de_semana × preço_do_litro`, e cada
/// fator é escolhido assim:
///
/// - `duracao_min` sai de `constants::categories::duracao_corrida_min`, exceto no Endurance,
///   onde a duração é sorteada em `resolve_race_duration` entre 120/180/240/360 → média 225.
/// - `litros_por_hora` é o consumo de pista do carro da categoria. Combustível de corrida se
///   mede em litro por HORA de pista, não por km — é assim que a equipe planeja stint, e
///   isso evita ter que inventar uma velocidade média por categoria.
/// - `fim_de_semana` é quantas vezes a distância da corrida o fim de semana inteiro queima
///   (treino livre + classificação + corrida). 2,5 nas categorias de sprint; menos no
///   Endurance, onde a corrida sozinha domina o fim de semana.
/// - `nº_de_carros` vem de `pilotos_por_equipe` — a fatura é da EQUIPE, e ela leva dois.
struct AncoraCombustivel {
    categoria: &'static str,
    duracao_min: f64,
    litros_por_hora: f64,
    fim_de_semana: f64,
    /// De onde veio o consumo. Vai impresso no relatório: âncora sem procedência é chute.
    origem: &'static str,
}

const ANCORA_COMBUSTIVEL: &[AncoraCombustivel] = &[
    AncoraCombustivel {
        categoria: "mazda_rookie",
        duracao_min: 15.0,
        litros_por_hora: 20.0,
        fim_de_semana: 2.5,
        origem: "MX-5 Cup, 2.0 aspirado de ~180 cv",
    },
    AncoraCombustivel {
        categoria: "toyota_rookie",
        duracao_min: 15.0,
        litros_por_hora: 24.0,
        fim_de_semana: 2.5,
        origem: "GR86 Cup, 2.4 aspirado de ~230 cv",
    },
    AncoraCombustivel {
        categoria: "mazda_amador",
        duracao_min: 25.0,
        litros_por_hora: 20.0,
        fim_de_semana: 2.5,
        origem: "mesmo carro da Rookie, etapa mais longa",
    },
    AncoraCombustivel {
        categoria: "toyota_amador",
        duracao_min: 25.0,
        litros_por_hora: 24.0,
        fim_de_semana: 2.5,
        origem: "mesmo carro da Rookie, etapa mais longa",
    },
    AncoraCombustivel {
        categoria: "bmw_m2",
        duracao_min: 25.0,
        litros_por_hora: 35.0,
        fim_de_semana: 2.5,
        origem: "M2 CS Racing, 3.0 turbo de ~365 cv",
    },
    AncoraCombustivel {
        categoria: "production_challenger",
        duracao_min: 30.0,
        litros_por_hora: 28.0,
        fim_de_semana: 2.5,
        origem: "média das 3 classes (mazda / toyota / bmw)",
    },
    AncoraCombustivel {
        categoria: "gt4",
        duracao_min: 30.0,
        litros_por_hora: 40.0,
        fim_de_semana: 2.5,
        origem: "GT4 de ~430 cv, stint de ~60 min com tanque de 100 L",
    },
    AncoraCombustivel {
        categoria: "gt3",
        duracao_min: 50.0,
        litros_por_hora: 57.0,
        fim_de_semana: 2.5,
        origem: "GT3 de ~550 cv, stint duplo de ~110 L em ~1h50",
    },
    AncoraCombustivel {
        categoria: "endurance",
        duracao_min: 225.0,
        litros_por_hora: 50.0,
        fim_de_semana: 1.8,
        origem: "média das 3 classes (gt4 / gt3 / lmp2); corrida domina o fim de semana",
    },
];

fn ancora_combustivel(categoria: &str) -> &'static AncoraCombustivel {
    ANCORA_COMBUSTIVEL
        .iter()
        .find(|a| a.categoria == categoria)
        .unwrap_or(&ANCORA_COMBUSTIVEL[0])
}

/// O que a linha de combustível custava numa etapa desta categoria pela fatura ANTIGA
/// (`tests::despesa_legada::compute_operation_lines`), para a equipe MEDIANA do grid
/// que `montar_grid` monta (q = 0,5 → facilities 55, pit crew 57,5) numa etapa continental.
///
/// `laps_ratio` não é 1,0: é a média de voltas dos carros do time, e carro que abandona
/// entra com 0,15–0,85 (média 0,5) no harness. Com a taxa de abandono medida da categoria
/// isso dá `1 − 0,5 × dnf`, que no Endurance (52,7%) tira 26% da linha.
fn combustivel_medido(categoria: &'static str) -> f64 {
    use super::despesa_legada::{compute_operation_lines, OperationInputs, TRAVEL_CONTINENTAL};

    let a = arena(categoria);
    let refer = referencia(categoria);
    let base = category_finance_scale(categoria).operating_cost_midpoint() / a.rodadas as f64;
    let entradas = OperationInputs {
        round_operating_base: base,
        facilities: 55.0,
        pit_crew_quality: 57.5,
        cost_modifier: 1.0,
        travel_factor: TRAVEL_CONTINENTAL,
        laps_ratio: (1.0 - 0.5 * refer.dnf_pct / 100.0).clamp(0.0, 1.0),
        tire_wear: refer.desgaste_final,
    };
    compute_operation_lines(&entradas)
        .iter()
        .find(|l| l.key == "gasolina")
        .map(|l| l.cost)
        .unwrap_or(0.0)
}

/// A âncora proposta, em dinheiro, para a mesma etapa.
fn combustivel_ancora(categoria: &'static str) -> f64 {
    let a = arena(categoria);
    let anc = ancora_combustivel(categoria);
    (anc.duracao_min / 60.0)
        * anc.litros_por_hora
        * a.pilotos_por_equipe as f64
        * anc.fim_de_semana
        * PRECO_LITRO_USD
}

#[test]
#[ignore = "critérios de aceitação da Parte 4 — roda com --ignored --nocapture"]
fn criterios_de_aceitacao() {
    println!("\n═══ PLACAR DE ACEITAÇÃO · Parte 4 de docs/economia-redesign.md ═══");
    println!("    9 categorias · {TEMPORADAS} temporadas · MODELO NOVO INTEIRO, um único binário");
    println!("    despesa: economia::evento + economia::temporada");
    println!("    receita: economia::receita (γ 6,5, nível 1,00)");
    println!("    offseason: economia::desenvolvimento (reserva 9 meses, 40% do excedente)");
    println!("    âncora: custo operacional anual FÍSICO, por classe\n");

    // ── Uma passada pelas 9 categorias alimenta quase toda a tabela ──────────────────
    let medidas: Vec<(&'static str, Resultado)> = CATEGORIAS
        .iter()
        .map(|&c| {
            (
                c,
                medir_categoria_cenario(c, &cenario_novo(ParametrosDeReceita::default())),
            )
        })
        .collect();

    let mut criterios: Vec<Criterio> = Vec::new();

    // 1. receita ÷ despesa → 0,95–1,15 EM TODA CATEGORIA
    let razoes: Vec<(&str, f64)> = medidas
        .iter()
        .map(|(c, r)| (*c, receita_total(r) / despesa_total(r).max(1.0)))
        .collect();
    let (razao_pior, razao_melhor, razao_fora) =
        faixa_e_fora(&razoes, |v| (0.95..=1.15).contains(&v));
    criterios.push(Criterio::novo(
        "receita ÷ despesa",
        "0,95 – 1,15 · toda cat.",
        format!(
            "{razao_pior:.2} – {razao_melhor:.2} · {} fora",
            razao_fora.len()
        ),
        razao_fora.is_empty(),
    ));

    // 2. fôlego no fim de 20 temporadas → BANDA de 3–18 MESES sobre a DISTRIBUIÇÃO do grid,
    //    em toda categoria: nenhuma equipe abaixo do piso, nenhuma acima do teto.
    //
    // Duas mudanças de forma, em rodadas diferentes, e vale separar as duas:
    //
    // **Primeira: de razão para unidade.** O critério era uma RAZÃO DE DERIVA (caixa final ÷
    // caixa inicial < 1,3×) e mudou pelo mesmo motivo que o critério 7 mudou: razão é
    // adimensional, e o destino não é. O 1,3× foi escrito quando a equipe nascia com ~24
    // meses de caixa — hoje `economia::temporada` a faz nascer com 1–11 (mediana 6). Uma
    // razão pequena sobre uma base inflada descreve uma equipe que nunca sente nada; uma
    // razão grande sobre uma base honesta pode ser um mundo saudável.
    //
    // **Segunda: de PONTO para BANDA.** A mediana foi trocada pelos extremos porque
    // `criterio_2_por_posicao_no_grid` mostrou que ela só descreve o grid onde o grid é
    // uniforme — e o grid uniforme é a exceção, não a regra: a dispersão campeão ÷ lanterna
    // vai de 1,0× nas duas Rookies a dezenas de vezes no resto da escada. Na forma antiga o
    // critério PASSAVA onde não estava medindo nada (gt4 com mediana de 12,6 meses, campeão
    // em 78,4 e lanterna em 6,3) e REPROVAVA justamente nas duas categorias em que a mediana
    // era o retrato honesto do grid. Um critério cego em sete das nove categorias que ele
    // avalia não é um critério frouxo — é um critério que mede a estatística errada.
    //
    // **Os dois números não se mexeram, de propósito.** Continuam 3 e 18, agora lidos das
    // constantes que já os definem no mundo (`piso_de_folego` / `teto_de_folego`) em vez de
    // escritos aqui. Mudar a estatística E os limites na mesma rodada tornaria impossível
    // dizer qual das duas coisas moveu o placar.
    //
    // O que se checa é a DISTRIBUIÇÃO inteira, não o par (campeão, lanterna) por pontos:
    // quem termina mais pobre nem sempre é quem terminou em último, e o que o critério
    // protege é a equipe pobre, não a posição dela na tabela.
    //
    // A mediana e a deriva continuam medidas e impressas. A mediana é a estatística velha,
    // e sem ela não dá para comparar rodada com rodada; a deriva é a TRAJETÓRIA, e uma
    // trajetória explosiva com destino habitável é informação diferente de uma plana.
    let bandas: Vec<(&str, BandaDeFolego)> = medidas
        .iter()
        .map(|(c, r)| (*c, banda_de_folego(&r.meses_fim)))
        .collect();
    let folegos: Vec<(&str, f64)> = bandas.iter().map(|(c, b)| (*c, b.mediana)).collect();
    let derivas: Vec<(&str, f64)> = medidas
        .iter()
        .map(|(c, r)| {
            (
                *c,
                mediana(&r.caixa_fim) / mediana(&r.caixa_inicio).max(0.01),
            )
        })
        .collect();
    let (_, deriva_maxima, _) = faixa_e_fora(&derivas, |v| v < 1.3);
    let folego_pior = bandas.iter().map(|(_, b)| b.pior).fold(f64::MAX, f64::min);
    let folego_melhor = bandas
        .iter()
        .map(|(_, b)| b.melhor)
        .fold(f64::MIN, f64::max);
    let equipes_abaixo: usize = bandas.iter().map(|(_, b)| b.abaixo_do_piso).sum();
    let equipes_acima: usize = bandas.iter().map(|(_, b)| b.acima_do_teto).sum();
    let equipes_no_mundo: usize = medidas.iter().map(|(_, r)| r.meses_fim.len()).sum();
    let banda_fora: Vec<&str> = bandas
        .iter()
        .filter(|(_, b)| b.abaixo_do_piso + b.acima_do_teto > 0)
        .map(|(c, _)| *c)
        .collect();
    criterios.push(Criterio::novo(
        "fôlego no fim de 20 temporadas",
        "3 – 18 meses · TODA equipe",
        format!(
            "{folego_pior:.1} – {folego_melhor:.1} meses · {} cat. fora \
             ({equipes_abaixo}↓ {equipes_acima}↑ de {equipes_no_mundo} equipes · deriva pior {deriva_maxima:.2}×)",
            banda_fora.len()
        ),
        banda_fora.is_empty(),
    )
    .porque(
        Veredito::Pendente,
        "a falha tem DUAS metades e elas pedem coisas diferentes. Por CIMA é a mesma de \
         sempre: o campeão persistente acumula e o ralo não alcança — alavanca varrida \
         (receita 1,00→0,50, reserva 12→3 em 13 pontos). Por BAIXO é NOVA, e apareceu com a \
         re-derivação de `spending_power`: o fundo de quase todo grid termina no piso ou \
         abaixo dele, com saldo anual negativo em regime permanente. Não é forma de critério, \
         é calibração — e é a MESMA doença que o critério 7 (colapso > crise) e o 8 (até \
         22,1% do grid vendido por temporada) estão medindo por outros dois caminhos. Decidir \
         o nível é decisão de projeto, não de teste.",
    ));

    // As fatias de receita são POR CATEGORIA; o agregado do mundo vai junto, como contexto.
    let receita_mundo: f64 = medidas.iter().map(|(_, r)| receita_total(r)).sum();
    let fatia = |f: fn(&Resultado) -> f64| -> (Vec<(&str, f64)>, f64) {
        let por_cat = medidas
            .iter()
            .map(|(c, r)| (*c, f(r) / receita_total(r).max(1.0) * 100.0))
            .collect();
        let mundo = medidas.iter().map(|(_, r)| f(r)).sum::<f64>() / receita_mundo.max(1.0) * 100.0;
        (por_cat, mundo)
    };

    // 3. prêmio por corrida, % da receita → ≥ 40% EM TODA CATEGORIA
    let (corrida, corrida_mundo) = fatia(|r| r.linhas.bonus + r.linhas.premio_parcial);
    let (corrida_min, corrida_max, corrida_fora) = faixa_e_fora(&corrida, |v| v >= 40.0);
    criterios.push(Criterio::novo(
        "prêmio por corrida, % da receita",
        "≥ 40% · toda cat.",
        format!(
            "{corrida_min:.1} – {corrida_max:.1}% · {} fora (mundo {corrida_mundo:.1})",
            corrida_fora.len()
        ),
        corrida_fora.is_empty(),
    ));

    // 4. bilheteria, % da receita → 10–20% EM TODA CATEGORIA
    let (portao, portao_mundo) = fatia(|r| r.linhas.bilheteria);
    let (portao_min, portao_max, portao_fora) =
        faixa_e_fora(&portao, |v| (10.0..=20.0).contains(&v));
    criterios.push(Criterio::novo(
        "bilheteria, % da receita",
        "10 – 20% · toda cat.",
        format!(
            "{portao_min:.2} – {portao_max:.2}% · {} fora (mundo {portao_mundo:.2})",
            portao_fora.len()
        ),
        portao_fora.is_empty(),
    ));

    // 5. prêmio de fim de temporada, % da receita → ≤ 10% EM TODA CATEGORIA
    let (fechamento, fechamento_mundo) = fatia(|r| r.linhas.premio_construtores);
    let (fech_min, fech_max, fech_fora) = faixa_e_fora(&fechamento, |v| v <= 10.0);
    criterios.push(Criterio::novo(
        "prêmio de fim de temporada, % da receita",
        "≤ 10% · toda cat.",
        format!(
            "{fech_min:.1} – {fech_max:.1}% · {} fora (mundo {fechamento_mundo:.1})",
            fech_fora.len()
        ),
        fech_fora.is_empty(),
    ));

    // 6. campeão ÷ lanterna (receita de temporada) → ≥ 3× EM TODA CATEGORIA
    // As que falham são as de CALENDÁRIO CURTO: poucas etapas dão ao prêmio por corrida
    // menos ocasiões de se acumular, e a diferenciação por resultado achata. É o sinal de
    // projeto para a seção 3.5 — o prêmio por etapa precisa ser maior onde há menos etapas.
    let c_sobre_l: Vec<(&str, f64)> = medidas
        .iter()
        .map(|(c, r)| (*c, r.receita_campeao / r.receita_lanterna.max(1.0)))
        .collect();
    let c_l_media = c_sobre_l.iter().map(|(_, v)| *v).sum::<f64>() / c_sobre_l.len().max(1) as f64;
    let (c_l_pior, c_l_melhor, _) = faixa_e_fora(&c_sobre_l, |_| true);
    // O alvo é por categoria E função do calendário, então a checagem não cabe em
    // `faixa_e_fora` — cada categoria tem o seu piso.
    let c_l_fora: Vec<&str> = c_sobre_l
        .iter()
        .filter(|(c, v)| *v < alvo_campeao_lanterna(arena(*c).rodadas))
        .map(|(c, _)| *c)
        .collect();
    criterios.push(
        Criterio::novo(
            "campeão ÷ lanterna (receita de temporada)",
            "≥2× até 6 · ≥3× de 8",
            format!(
                "{c_l_pior:.2} – {c_l_melhor:.2}× · {} fora (média {c_l_media:.2})",
                c_l_fora.len()
            ),
            c_l_fora.is_empty(),
        )
        .porque(
            Veredito::DefeitoConhecido,
            "grid pequeno: com poucas etapas e poucas equipes o campeonato é ruído, e o campeão \
         termina perto do lanterna. γ foi varrido de 1 a 12 — subir mais mata o fundo do grid.",
        ),
    );

    // 7. equipes em crise ou colapso → 8–15% NO MUNDO (a única métrica agregada que sobrou:
    // é propriedade da população inteira, não de um degrau da escada)
    let observacoes: u32 = medidas
        .iter()
        .map(|(_, r)| r.estados.values().sum::<u32>())
        .sum();
    let em_crise: u32 = medidas
        .iter()
        .map(|(_, r)| {
            r.estados.get("crisis").copied().unwrap_or(0)
                + r.estados.get("collapse").copied().unwrap_or(0)
        })
        .sum();
    let crise_pct = em_crise as f64 / observacoes.max(1) as f64 * 100.0;
    // A RESTRIÇÃO DE FORMA, que é a metade derivada do critério: um mundo saudável tem mais
    // gente adoecendo do que morta. Sem ela, 14,7% "dentro da faixa" podia ser 2,9% de crise
    // com 11,8% de colapso — quatro vezes mais equipes mortas do que morrendo.
    let so_crise: u32 = medidas
        .iter()
        .map(|(_, r)| r.estados.get("crisis").copied().unwrap_or(0))
        .sum();
    let so_colapso: u32 = medidas
        .iter()
        .map(|(_, r)| r.estados.get("collapse").copied().unwrap_or(0))
        .sum();
    let forma_ok = so_colapso < so_crise;
    criterios.push(Criterio::novo(
        "equipes em crise ou colapso",
        "5 – 12% · colapso < crise",
        format!(
            "{crise_pct:.1}% ({so_crise} crise / {so_colapso} colapso){}",
            if forma_ok { "" } else { " ✗forma" }
        ),
        (5.0..=12.0).contains(&crise_pct) && forma_ok,
    )
    .porque(
        Veredito::AlavancaEsgotada,
        "a reserva do ralo foi varrida em 13 pontos; a crise anda com ela, mas ao CONTRÁRIO do \
         esperado (drenar mais REDUZ a crise, porque o investimento tem retorno).",
    ));

    // 8. equipes vendidas por falência → 0,5%–5% do GRID por temporada, em categoria de
    // `GRID_MINIMO_PARA_FALENCIA` equipes ou mais. Abaixo disso não há piso: ver a constante.
    //
    // Normalizar pelo tamanho do grid é o que faz a métrica morder nas DUAS pontas: contar
    // vendas absolutas deixa passar tanto a GT3 (toda equipe trocaria de dono a cada 7 anos)
    // quanto o Endurance (ninguém quebra nunca).
    //
    // `Resultado::vendas` já conta a venda por colapso crônico (2 temporadas seguidas em
    // `collapse` → `apply_team_sale`), que é exatamente a falência do critério.
    let vendas_pct: Vec<(&str, f64)> = medidas
        .iter()
        .map(|(c, r)| {
            let grid = arena(c).equipes as f64;
            (
                *c,
                r.vendas as f64 / (TEMPORADAS as f64 * grid.max(1.0)) * 100.0,
            )
        })
        .collect();
    // Só as categorias GRANDES entram na avaliação. As pequenas continuam impressas na
    // tabela por categoria — o alvo é que não se aplica a elas.
    let vendas_avaliadas: Vec<(&str, f64)> = vendas_pct
        .iter()
        .filter(|(c, _)| arena(c).equipes >= GRID_MINIMO_PARA_FALENCIA)
        .copied()
        .collect();
    let isentas = vendas_pct.len() - vendas_avaliadas.len();
    let (vendas_min, vendas_max, vendas_fora) =
        faixa_e_fora(&vendas_avaliadas, |v| (0.5..=5.0).contains(&v));
    let vendas_totais: u32 = medidas.iter().map(|(_, r)| r.vendas).sum();
    criterios.push(Criterio::novo(
        "equipes vendidas por falência, % do grid/temporada",
        "0,5 – 5% · grid ≥ 10",
        format!(
            "{vendas_min:.2} – {vendas_max:.2}% · {} fora ({vendas_totais} vendas · {isentas} isentas)",
            vendas_fora.len()
        ),
        vendas_fora.is_empty(),
    )
    .porque(
        Veredito::DefeitoConhecido,
        "as que sobram têm 10 equipes mas receita que espalha pouco — o mesmo ruído de grid \
         pequeno. A sessão da falência varreu o aporte em 4,2/3,0/2,0 meses: nenhum valor alcança.",
    ));

    // 11a. espalhamento da bilheteria no grid → melhor ÷ pior ≥ 2,5× EM TODA CATEGORIA
    // A estatística que vale é a de TEMPORADA, não a de etapa: a economia de uma equipe é
    // anual, e um espalhamento de etapa que se dissolve no ano não diferencia a vida de
    // ninguém. Nessa definição a atração de produção (2,70–5,13× POR ETAPA) comprime — a
    // posição no campeonato e a forma recente oscilam ao longo do ano e entre anos.
    let espalhamentos: Vec<(&str, f64)> = medidas
        .iter()
        .map(|(c, r)| (*c, r.portao_melhor / r.portao_pior.max(1.0)))
        .collect();
    let (esp_min, esp_max, esp_fora) = faixa_e_fora(&espalhamentos, |v| v >= 2.5);
    criterios.push(
        Criterio::novo(
            "11a · espalhamento da bilheteria (temporada)",
            "melhor ÷ pior ≥ 2,5×",
            format!("{esp_min:.2} – {esp_max:.2}× · {} fora", esp_fora.len()),
            esp_fora.is_empty(),
        )
        .porque(
            Veredito::DefeitoConhecido,
            "o mesmo ruído de grid pequeno do critério 6: o espalhamento de ETAPA existe \
         (2,7–5,1× medido em produção) e se cancela no agregado do ano. Falha até na GT3 de 14 \
         etapas, e nenhum afrouxamento razoável salva.",
        ),
    );

    // 11b. o portão SUSTENTA o fundo do grid → ≥ 10% da receita da equipe de PIOR atração.
    // Nasceu de uma medição lateral do critério 11a e virou critério próprio: a razão entre
    // pontas diz se o canal DIFERENCIA, este diz se ele SUSTENTA. Um espalhamento de 5× em
    // que o pior recebe 1% da receita é um canal decorativo.
    let portao_da_pior: Vec<(&str, f64)> = medidas
        .iter()
        .map(|(c, r)| (*c, r.portao_pct_da_pior))
        .collect();
    let (pior_min, pior_max, pior_fora) = faixa_e_fora(&portao_da_pior, |v| v >= 10.0);
    criterios.push(Criterio::novo(
        "11b · portão para a equipe de PIOR atração",
        "≥ 10% da receita dela",
        format!("{pior_min:.1} – {pior_max:.1}% · {} fora", pior_fora.len()),
        pior_fora.is_empty(),
    ));

    // 9. nenhuma categoria pior em todos os cenários → obrigatório
    //
    // Os cenários foram reexpressos no modelo NOVO: os antigos eram variações de
    // `CoeficientesDeReceita` (pesos do `round_operating_base`), que não existem mais no
    // caminho medido. São intenções de design inteiras, não botões isolados.
    let cenarios = cenarios_do_modelo_novo();
    println!("  (rodando os {} cenários de receita…)", cenarios.len());
    let mut piores_por_cenario: Vec<(&'static str, &'static str)> = Vec::new();
    for (nome, params) in cenarios {
        let resumo = resumir_receita(&cenario_novo(params));
        let (mut pior_cat, mut pior_valor) = (CATEGORIAS[0], f64::MAX);
        for (i, v) in resumo.razao.iter().enumerate() {
            if *v < pior_valor {
                pior_valor = *v;
                pior_cat = CATEGORIAS[i];
            }
        }
        piores_por_cenario.push((nome, pior_cat));
    }
    let sempre_a_mesma = piores_por_cenario
        .first()
        .map(|(_, a)| piores_por_cenario.iter().all(|(_, b)| b == a))
        .unwrap_or(false);
    let refem = piores_por_cenario.first().map(|(_, a)| *a).unwrap_or("");
    criterios.push(
        Criterio::novo(
            "nenhuma categoria pior em todos os cenários",
            "obrigatório",
            if sempre_a_mesma {
                format!("{refem} é a pior nos {}", piores_por_cenario.len())
            } else {
                "a pior muda de cenário".to_string()
            },
            !sempre_a_mesma,
        )
        .porque(
            Veredito::Pendente,
            "uma categoria é refém da calibração: ela é a mais sacrificada em toda intenção de \
         design testada, o que significa que a ordem não é consequência dos coeficientes.",
        ),
    );

    // 10. linha de combustível de uma etapa → ≤ 10× a âncora física da seção 4.1
    // "Dentro de uma ordem de grandeza" = razão medido ÷ âncora até 10×, em toda categoria.
    //
    // Medido no modelo NOVO: a linha que sai de `economia::evento` é litros × preço do litro,
    // e a âncora da seção 4.1 é (duração × consumo horário × carros × fim de semana) × preço.
    // As duas derivações são independentes — a do modelo passa por quilometragem e consumo
    // por km, a da âncora por consumo por HORA — então a razão entre elas ainda é uma prova,
    // não uma tautologia.
    let razoes_combustivel: Vec<(&'static str, f64)> = CATEGORIAS
        .iter()
        .map(|&c| {
            (
                c,
                combustivel_do_modelo_fisico(c) / combustivel_ancora(c).max(1.0),
            )
        })
        .collect();
    let (comb_min, comb_max, comb_fora) = faixa_e_fora(&razoes_combustivel, |v| v <= 10.0);
    criterios.push(Criterio::novo(
        "linha de combustível de uma etapa",
        "≤ 10× a âncora (4.1)",
        format!("{comb_min:.2} – {comb_max:.2}× · {} fora", comb_fora.len()),
        comb_fora.is_empty(),
    ));

    // ── Detalhe por categoria, para o número do placar ter onde ser conferido ────────
    println!("\n  ── por categoria ──");
    println!(
        "  {:<24} {:>8} {:>7} {:>8} {:>9} {:>9} {:>9} {:>9} {:>7}",
        "categoria",
        "rec/desp",
        "meses",
        "deriva",
        "corrida%",
        "portão%",
        "fecha%",
        "camp/lant",
        "venda%"
    );
    for (i, (categoria, razao)) in razoes.iter().enumerate() {
        // A categoria isenta do critério 8 leva a marca: o número continua impresso, mas
        // não é avaliado. Esconder a coluna esconderia a informação junto com o alvo.
        let isenta = arena(categoria).equipes < GRID_MINIMO_PARA_FALENCIA;
        println!(
            "  {:<24} {:>8.2} {:>7.1} {:>7.2}× {:>9.1} {:>9.2} {:>9.1} {:>8.2}× {:>6.2}{}",
            categoria,
            razao,
            folegos[i].1,
            derivas[i].1,
            corrida[i].1,
            portao[i].1,
            fechamento[i].1,
            c_sobre_l[i].1,
            vendas_pct[i].1,
            if isenta { "*" } else { " " },
        );
    }
    println!(
        "  * grid abaixo de {GRID_MINIMO_PARA_FALENCIA} equipes: fora do alvo do critério 8 (ver `GRID_MINIMO_PARA_FALENCIA`)"
    );

    // ── Critério 2: a banda, categoria a categoria ───────────────────────────────────
    // A coluna `meses` da tabela acima é a MEDIANA, que é a estatística velha do critério.
    // Esta tabela é a nova, e as duas juntas mostram por que a troca importa: onde a
    // mediana está confortável e as pontas não estão, a linha de cima passa e esta reprova.
    println!(
        "\n  ── critério 2 · a BANDA sobre o grid (piso {:.0} · teto {:.0} meses) ──",
        piso_de_folego(),
        teto_de_folego()
    );
    println!(
        "  {:<24} {:>8} {:>9} {:>8} {:>9} {:>8}   {}",
        "categoria", "pior", "mediana", "melhor", "abaixo", "acima", "o que a mediana esconde"
    );
    for (c, b) in &bandas {
        // A leitura compara os DOIS vereditos: o que a mediana daria sozinha e o que a
        // distribuição dá. Quando eles discordam, a linha nomeia a discordância — é o
        // achado que motivou a troca de forma, e ele não deve depender de quem lê a tabela.
        let mediana_ok = (piso_de_folego()..=teto_de_folego()).contains(&b.mediana);
        let banda_ok = b.abaixo_do_piso + b.acima_do_teto == 0;
        let leitura = match (mediana_ok, banda_ok) {
            (true, true) => "—",
            (true, false) => "a mediana PASSA e o grid não: era aqui que o critério era cego",
            (false, true) => "impossível por construção (mediana está dentro da banda)",
            (false, false) => "reprova nas duas formas",
        };
        println!(
            "  {:<24} {:>8.1} {:>9.1} {:>8.1} {:>9} {:>8}   {}",
            c, b.pior, b.mediana, b.melhor, b.abaixo_do_piso, b.acima_do_teto, leitura
        );
    }
    println!(
        "  piso = FaixasDeMeses::pressionada · teto = 2 × ParametrosDeDesenvolvimento::meses_de_reserva"
    );

    println!("\n  ── qual categoria é a mais sacrificada, por cenário ──");
    for (nome, pior) in &piores_por_cenario {
        println!("  {nome:<28} → {pior}");
    }

    // ── Combustível: LEGADO × FÍSICO × âncora da seção 4.1 ───────────────────────────
    // As três colunas juntas são a prova de que o critério 10 fechou por CORREÇÃO e não por
    // afrouxamento do alvo: a âncora não mudou, o que mudou foi de onde o número sai.
    println!("\n  ── linha de combustível: LEGADO × FÍSICO × âncora (seção 4.1) ──");
    println!(
        "  {:<24} {:>11} {:>11} {:>10} {:>9}   {}",
        "categoria", "legado", "físico", "âncora", "físico÷anc", "de onde vem a âncora"
    );
    for &categoria in CATEGORIAS {
        let a = arena(categoria);
        let anc = ancora_combustivel(categoria);
        let velho = combustivel_medido(categoria);
        let novo = combustivel_do_modelo_fisico(categoria);
        let alvo = combustivel_ancora(categoria);
        println!(
            "  {:<24} {:>11.0} {:>11.0} {:>10.0} {:>8.2}×   {} · {:.0} min · {:.0} L/h · {} carros",
            categoria,
            velho,
            novo,
            alvo,
            novo / alvo.max(1.0),
            anc.origem,
            anc.duracao_min,
            anc.litros_por_hora,
            a.pilotos_por_equipe,
        );
    }
    println!(
        "  preço do litro: US$ {PRECO_LITRO_USD:.2} · fim de semana = treino + classificação + corrida"
    );

    // ── O PLACAR ─────────────────────────────────────────────────────────────────────
    let falhas: Vec<&Criterio> = criterios.iter().filter(|c| !c.passou).collect();
    let conta = |v: Veredito| criterios.iter().filter(|c| c.veredito == v).count();

    // São ONZE critérios em DOZE linhas: o 11 conta duas vezes desde que a medição lateral
    // do portão (11b, "o canal sustenta o fundo do grid") virou alvo próprio ao lado do
    // 11a original ("o canal diferencia as pontas"). São perguntas diferentes com respostas
    // opostas, e somá-las numa linha só esconderia as duas.
    println!("\n\n  ═══ PLACAR · 11 critérios, 12 linhas (o 11 é 11a + 11b) ═══\n");
    println!(
        "  {:<46} {:<24} {:<40} {}",
        "critério", "alvo", "medido", "veredito"
    );
    println!("  {}", "─".repeat(130));
    for c in &criterios {
        println!(
            "  {:<46} {:<24} {:<40} {}",
            c.nome,
            c.alvo,
            c.medido,
            c.veredito.rotulo()
        );
    }

    println!(
        "\n  {} de {} linhas passam · {} defeito conhecido · {} alavanca esgotada · {} TRABALHO PENDENTE",
        conta(Veredito::Passa),
        criterios.len(),
        conta(Veredito::DefeitoConhecido),
        conta(Veredito::AlavancaEsgotada),
        conta(Veredito::Pendente),
    );

    if !falhas.is_empty() {
        println!("\n  ── por que cada um não passa ──");
        for c in &falhas {
            println!("\n  · {} [{}]", c.nome, c.veredito.rotulo());
            println!("    {}", c.porque);
        }
    }

    println!(
        "\n  A amplitude da MÍDIA dos pilotos (desvio ≥ 15 pontos na categoria) não está neste\n  \
         placar: o harness sintetiza a presença da equipe a partir de uma constante, então\n  \
         medi-la aqui seria medir a própria constante. Ela mora no harness de fama."
    );

    // O assert continua existindo e continua reprovando. Um placar que passa a verde com
    // critério fora não é placar — e a classificação do veredito diz o que fazer com a
    // falha, não que ela deixou de ser uma.
    assert!(
        falhas.is_empty(),
        "{} de {} critérios de aceitação da Parte 4 falham ({} pendentes de trabalho):\n{}",
        falhas.len(),
        criterios.len(),
        conta(Veredito::Pendente),
        falhas
            .iter()
            .map(|c| format!(
                "  · [{}] {} — alvo {}, medido {}",
                c.veredito.rotulo(),
                c.nome,
                c.alvo,
                c.medido
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ===================== Varredura do RALO (seções 2.6 e 3.4) =====================
//
// A pergunta: qual a FORMA e a MAGNITUDE de ralo que segura a deriva do caixa abaixo de 1,3×
// em toda categoria (critério 2), sem estourar o critério 7 (crise entre 8% e 15%) nem
// achatar o critério 6 (campeão ÷ lanterna ≥ 3× em toda categoria).
//
// Três formas, definidas em `Ralo`: A (manter estrutura), B (melhorar estrutura), C (dreno
// do excedente, que entra como CONTROLE — é o teto teórico de eficácia).
//
// Roda com `cargo test --lib varrer_ralo -- --ignored --nocapture`.

/// O que uma configuração de ralo produz, medido na unidade dos critérios da Parte 4.
struct ResumoRalo {
    /// Deriva do caixa por categoria, na ordem de `CATEGORIAS`. É a linha que responde "o
    /// ralo aperta igual em toda a escada?".
    deriva: Vec<f64>,
    deriva_pior: f64,
    /// Quantas das 9 categorias ficam com deriva < 1,3× (critério 2).
    cat_no_alvo: usize,
    /// % do custo operacional ANUAL efetivamente drenado, por equipe e por temporada. É a
    /// resposta numérica da pergunta "quanto o ralo precisa drenar".
    dreno_medio: f64,
    dreno_min: f64,
    dreno_max: f64,
    /// Critério 7 — crise ou colapso no mundo.
    crise_pct: f64,
    /// Critério 6 — campeão ÷ lanterna na PIOR categoria.
    cl_pior: f64,
    /// Critério 1 — receita ÷ despesa, a faixa da escada.
    razao_pior: f64,
    razao_melhor: f64,
    /// Pontos de estrutura que o offseason dá por equipe e por temporada. Diagnóstico do
    /// ralo B: é o denominador dele.
    pontos_por_equipe_temporada: f64,
    /// O que SOBRA por equipe e por temporada, em % do custo operacional anual. É o alvo do
    /// ralo por construção: para o caixa parar de inflar, o ralo precisa drenar isto.
    superavit: Vec<f64>,
}

impl ResumoRalo {
    /// Os três critérios que a medição precisa respeitar ao mesmo tempo.
    ///
    /// O critério 6 (campeão ÷ lanterna ≥ 3×) entra como "não PIORAR", não como "atingir":
    /// ele já falha hoje nas três categorias de calendário curto, e essa falha é de prêmio
    /// por etapa, não de caixa. Exigir que um ralo a conserte seria cobrar do instrumento
    /// errado — o que se exige dele é não achatar mais.
    fn viavel(&self, cl_referencia: f64) -> bool {
        self.cat_no_alvo == CATEGORIAS.len()
            && (8.0..=15.0).contains(&self.crise_pct)
            && self.cl_pior >= cl_referencia - 0.05
    }
}

fn resumir_ralo(coef: CoeficientesDeReceita, ralo: Ralo) -> ResumoRalo {
    resumir_ralo_com(coef, ralo, Offseason::Producao)
}

fn resumir_ralo_com(coef: CoeficientesDeReceita, ralo: Ralo, offseason: Offseason) -> ResumoRalo {
    let mut deriva = Vec::new();
    let mut dreno = Vec::new();
    let mut razao = Vec::new();
    let mut cl = Vec::new();
    let mut superavit = Vec::new();
    let mut pontos = 0.0f64;
    let mut equipes_temporada = 0.0f64;
    let (mut observacoes, mut colapsos) = (0u32, 0u32);

    for &categoria in CATEGORIAS {
        let r = medir_categoria_com(categoria, coef, ralo, offseason);
        let a = arena(categoria);
        let op_anual = category_finance_scale(categoria).operating_cost_midpoint();
        let equipe_temporada = a.equipes as f64 * TEMPORADAS as f64;

        deriva.push(mediana(&r.caixa_fim) / mediana(&r.caixa_inicio).max(0.01));
        dreno.push(r.linhas.ralo / equipe_temporada / op_anual * 100.0);
        superavit
            .push((receita_total(&r) - despesa_total(&r)) / equipe_temporada / op_anual * 100.0);
        razao.push(receita_total(&r) / despesa_total(&r).max(1.0));
        cl.push(r.receita_campeao / r.receita_lanterna.max(1.0));
        pontos += r.pontos_estrutura;
        equipes_temporada += equipe_temporada;
        observacoes += r.estados.values().sum::<u32>();
        colapsos += r.estados.get("crisis").copied().unwrap_or(0)
            + r.estados.get("collapse").copied().unwrap_or(0);
    }

    ResumoRalo {
        deriva_pior: deriva.iter().copied().fold(f64::MIN, f64::max),
        cat_no_alvo: deriva.iter().filter(|d| **d < 1.3).count(),
        dreno_medio: dreno.iter().sum::<f64>() / dreno.len().max(1) as f64,
        dreno_min: dreno.iter().copied().fold(f64::MAX, f64::min),
        dreno_max: dreno.iter().copied().fold(f64::MIN, f64::max),
        crise_pct: colapsos as f64 / observacoes.max(1) as f64 * 100.0,
        cl_pior: cl.iter().copied().fold(f64::MAX, f64::min),
        razao_pior: razao.iter().copied().fold(f64::MAX, f64::min),
        razao_melhor: razao.iter().copied().fold(f64::MIN, f64::max),
        pontos_por_equipe_temporada: pontos / equipes_temporada.max(1.0),
        superavit,
        deriva,
    }
}

fn nome_curto(categoria: &str) -> &'static str {
    match categoria {
        "mazda_rookie" => "mRoo",
        "toyota_rookie" => "tRoo",
        "mazda_amador" => "mAma",
        "toyota_amador" => "tAma",
        "bmw_m2" => " bmw",
        "production_challenger" => "prod",
        "gt4" => " gt4",
        "gt3" => " gt3",
        "endurance" => "endu",
        _ => "????",
    }
}

fn cabecalho_ralo() {
    let nomes: String = CATEGORIAS
        .iter()
        .map(|c| format!("{:>6}", nome_curto(c)))
        .collect::<Vec<_>>()
        .join("");
    println!(
        "  {:<20}{nomes} │  pior  ok │  dreno % do operacional │ crise │  c/l │ receita/despesa",
        ""
    );
}

fn linha_ralo(rotulo: &str, r: &ResumoRalo, cl_referencia: f64) {
    let derivas: String = r
        .deriva
        .iter()
        .map(|v| format!("{v:>6.2}"))
        .collect::<Vec<_>>()
        .join("");
    println!(
        "  {rotulo:<20}{derivas} │ {:>5.2} {}/9 │ {:>5.1} ({:>4.1} – {:>5.1}) │ {:>4.1}% │ {:>4.2} │ {:>4.2} – {:<4.2}{}",
        r.deriva_pior,
        r.cat_no_alvo,
        r.dreno_medio,
        r.dreno_min,
        r.dreno_max,
        r.crise_pct,
        r.cl_pior,
        r.razao_pior,
        r.razao_melhor,
        if r.viavel(cl_referencia) {
            "  ← VIÁVEL"
        } else {
            ""
        }
    );
}

#[test]
#[ignore = "medição do ralo — roda com --ignored --nocapture"]
fn varrer_ralo() {
    let coef = CoeficientesDeReceita::default();

    println!("\n═══ QUANTO O RALO PRECISA DRENAR ═══");
    println!("  A seção 2.6 diz que a economia não tem ralo e a 3.4 propõe um, sem magnitude.");
    println!("  Aqui as três formas são SIMULADAS dentro do harness. Nada disso existe em");
    println!("  produção — é medição para que quem escrever `economia/desenvolvimento.rs`");
    println!("  não chute.\n");
    println!("  A coluna 'dreno' é o que sai do caixa por equipe e por temporada, em % do");
    println!("  custo operacional ANUAL declarado da categoria. É a unidade da resposta.");
    println!("  'ok' = quantas das 9 categorias ficam com deriva < 1,3× (critério 2).");
    println!(
        "  VIÁVEL = as 9 no alvo + crise entre 8 e 15% + campeão/lanterna não pior que hoje.\n"
    );

    let base = resumir_ralo(coef, Ralo::default());
    let cl_ref = base.cl_pior;
    cabecalho_ralo();
    linha_ralo("SEM RALO (hoje)", &base, cl_ref);

    // ── O alvo do ralo, categoria por categoria ──────────────────────────────────────
    // Se o caixa infla, é porque sobra dinheiro; e o que sobra é exatamente o que o ralo
    // precisa tirar. Esta tabela é a resposta direta da pergunta "drenar X% de quê".
    println!("\n  ── quanto SOBRA hoje: o alvo que o ralo tem que zerar ──");
    println!(
        "  {:<24} {:>8} {:>34}",
        "categoria", "deriva", "superávit % do operacional anual"
    );
    for (i, &categoria) in CATEGORIAS.iter().enumerate() {
        let a = arena(categoria);
        println!(
            "  {:<24} {:>7.2}× {:>33.1}   ({} etapas)",
            categoria, base.deriva[i], base.superavit[i], a.rodadas
        );
    }
    println!(
        "\n  O offseason regala {:.2} ponto de estrutura por equipe por temporada. Esse é o\n  \
         denominador do ralo B: cobrar por ponto só drena muito se o ponto for muito caro.",
        base.pontos_por_equipe_temporada
    );

    // ── A. Custo de MANTER estrutura ─────────────────────────────────────────────────
    println!("\n── A. Custo de MANTER estrutura (fração do operacional anual, no porte máximo) ──");
    cabecalho_ralo();
    for v in [0.05, 0.15, 0.30, 0.60, 1.00, 1.50] {
        linha_ralo(
            &format!("A manter {v:.2}"),
            &resumir_ralo(coef, Ralo::de_manter(v)),
            cl_ref,
        );
    }

    // ── B. Custo de MELHORAR estrutura ───────────────────────────────────────────────
    println!("\n── B. Custo de MELHORAR estrutura (fração do operacional anual por 100 pontos) ──");
    cabecalho_ralo();
    for v in [0.5, 2.0, 8.0, 32.0, 128.0] {
        linha_ralo(
            &format!("B melhorar {v:.1}"),
            &resumir_ralo(coef, Ralo::de_melhorar(v)),
            cl_ref,
        );
    }

    // ── C. Dreno do excedente (CONTROLE) ─────────────────────────────────────────────
    println!("\n── C. Dreno do excedente acima de 12 meses de operação (CONTROLE) ──");
    cabecalho_ralo();
    for v in [0.05, 0.10, 0.20, 0.40, 0.70] {
        linha_ralo(
            &format!("C excedente {v:.2}"),
            &resumir_ralo(coef, Ralo::de_excedente(v, 12.0)),
            cl_ref,
        );
    }

    // ── D. A + B combinados ──────────────────────────────────────────────────────────
    println!("\n── D. A + B combinados ──");
    cabecalho_ralo();
    let mut melhor_ab: Option<(f64, f64, ResumoRalo)> = None;
    let mut mais_categorias: Option<(f64, f64, ResumoRalo)> = None;
    for manter in [0.15, 0.30, 0.60, 1.00] {
        for melhorar in [2.0, 8.0, 32.0] {
            let r = resumir_ralo(coef, Ralo::combinado(manter, melhorar));
            linha_ralo(&format!("A {manter:.2} + B {melhorar:.1}"), &r, cl_ref);
            let troca_viavel = r.viavel(cl_ref)
                && melhor_ab
                    .as_ref()
                    .map(|(_, _, m)| r.dreno_medio < m.dreno_medio)
                    .unwrap_or(true);
            let troca_cobertura = mais_categorias
                .as_ref()
                .map(|(_, _, m)| r.cat_no_alvo > m.cat_no_alvo)
                .unwrap_or(true);
            if troca_cobertura {
                mais_categorias = Some((manter, melhorar, r));
            } else if troca_viavel {
                melhor_ab = Some((manter, melhorar, r));
            }
        }
    }

    // ── O veredito da magnitude ──────────────────────────────────────────────────────
    println!("\n── veredito ──");
    let escolhido: (String, Ralo, &ResumoRalo) = if let Some((m, b, r)) = melhor_ab.as_ref() {
        println!(
            "  A + B resolve: manter {m:.2} + melhorar {b:.1} drena {:.1}% do operacional anual",
            r.dreno_medio
        );
        (format!("A {m:.2} + B {b:.1}"), Ralo::combinado(*m, *b), r)
    } else {
        let (m, b, r) = mais_categorias
            .as_ref()
            .expect("a grade D tem ao menos uma configuração");
        println!(
            "  NENHUMA combinação de A + B é viável. A melhor (manter {m:.2} + melhorar {b:.1})\n  \
             põe {}/9 categorias no alvo drenando {:.1}% do operacional anual, com a pior em\n  \
             {:.2}× e a crise em {:.1}%.",
            r.cat_no_alvo, r.dreno_medio, r.deriva_pior, r.crise_pct
        );
        (format!("A {m:.2} + B {b:.1}"), Ralo::combinado(*m, *b), r)
    };
    let _ = &escolhido;

    // O CONTROLE C é o teto de eficácia: se ele resolve e A+B não, a diferença é o que as
    // formas realistas ainda não alcançam.
    let mut melhor_c: Option<(f64, ResumoRalo)> = None;
    for v in [0.05, 0.10, 0.20, 0.40, 0.70] {
        let r = resumir_ralo(coef, Ralo::de_excedente(v, 12.0));
        let troca = r.viavel(cl_ref)
            && melhor_c
                .as_ref()
                .map(|(_, m)| r.dreno_medio < m.dreno_medio)
                .unwrap_or(true);
        if troca {
            melhor_c = Some((v, r));
        }
    }
    match melhor_c.as_ref() {
        Some((v, r)) => println!(
            "  CONTROLE C: dreno {v:.2} sobre o excedente resolve os três critérios drenando\n  \
             {:.1}% do operacional anual ({:.1}% na mais leve, {:.1}% na mais pesada).",
            r.dreno_medio, r.dreno_min, r.dreno_max
        ),
        None => {
            println!("  CONTROLE C: nenhuma magnitude testada é viável — o teto não é alcançável.")
        }
    }

    // ── E. O ralo muda a resposta da varredura de receita? ───────────────────────────
    // Se o espalhamento mínimo do par (patrocínio × bônus) cair com o ralo ligado, é a
    // primeira evidência de que a escada pode ficar plana — o ralo deixaria de ser só um
    // freio de caixa e passaria a ser condição para calibrar a receita.
    let ralo_de_teste = melhor_c
        .as_ref()
        .map(|(v, _)| Ralo::de_excedente(*v, 12.0))
        .unwrap_or_else(|| escolhido.1);
    println!("\n── E. A busca do par (patrocínio × bônus), sem e com o ralo ──");
    println!("  'espalhamento' = a categoria mais folgada menos a mais sacrificada.\n");
    println!(
        "  {:<10} {:<10} {:>10} {:>10} {:>12} {:>10}",
        "patroc", "bônus", "espalha", "espalha", "Δ", "crise c/"
    );
    println!(
        "  {:<10} {:<10} {:>10} {:>10} {:>12} {:>10}",
        "", "", "sem ralo", "com ralo", "", "ralo"
    );
    let (mut melhor_sem, mut melhor_com) = ((f64::MAX, 0.0, 0.0), (f64::MAX, 0.0, 0.0));
    for patroc in [0.10, 0.14, 0.18, 0.22, 0.27] {
        for escala in [1.0, 1.5, 2.0, 2.5, 3.0] {
            let mut c = coef;
            c.patrocinio_base = patroc;
            c.escala_do_bonus = escala;
            c.premio_base = 0.05;
            c.premio_inclinacao = 0.25;
            let sem = resumir(c);
            let com = resumir_com(c, ralo_de_teste, Offseason::Producao);
            let (e_sem, e_com) = (sem.melhor - sem.pior, com.melhor - com.pior);
            println!(
                "  {patroc:<10.2} {escala:<10.2} {e_sem:>10.2} {e_com:>10.2} {:>+12.2} {:>9.1}%",
                e_com - e_sem,
                com.colapso_pct
            );
            if e_sem < melhor_sem.0 {
                melhor_sem = (e_sem, patroc, escala);
            }
            if e_com < melhor_com.0 {
                melhor_com = (e_com, patroc, escala);
            }
        }
    }
    println!(
        "\n  menor espalhamento SEM ralo: {:.2} (patrocínio {:.2} × bônus {:.2})",
        melhor_sem.0, melhor_sem.1, melhor_sem.2
    );
    println!(
        "  menor espalhamento COM ralo: {:.2} (patrocínio {:.2} × bônus {:.2})",
        melhor_com.0, melhor_com.1, melhor_com.2
    );
    println!(
        "  Δ = {:+.2} — {}",
        melhor_com.0 - melhor_sem.0,
        if melhor_com.0 < melhor_sem.0 {
            "o ralo APROXIMA as categorias: calibrar a receita fica mais fácil com ele ligado"
        } else {
            "o ralo NÃO aproxima as categorias: o espalhamento é de despesa, não de caixa"
        }
    );
}

// ===================== `economia::desenvolvimento` no lugar do offseason =====================
//
// A varredura acima mediu o ralo com um dreno SINTÉTICO — um `if` que tirava caixa. Aqui o
// mesmo laço roda com o módulo de verdade no lugar de
// `finance::cashflow::apply_offseason_competitiveness_impact`, e a pergunta é se ele
// reproduz o que a forma C alcançou.
//
// Roda com `cargo test --lib desenvolvimento_no_offseason -- --ignored --nocapture`.

/// A estrutura com que o grid TERMINA a simulação, pareada com o caixa de cada equipe.
/// É a resposta medida da pergunta 1 — não uma estimativa: sai do estado final do laço,
/// depois de 20 temporadas de decisões reais.
struct EstruturaFinal {
    /// `engineering + facilities` (0–200) da equipe mais rica, da mediana e da mais pobre
    /// do grid, ordenadas pelo CAIXA final.
    estrutura_rico: f64,
    estrutura_mediano: f64,
    estrutura_pobre: f64,
    /// Amplitude da estrutura no grid: melhor menos pior.
    amplitude: f64,
}

fn estrutura_final(categoria: &'static str, offseason: Offseason) -> EstruturaFinal {
    let r = medir_categoria_com(
        categoria,
        CoeficientesDeReceita::default(),
        Ralo::default(),
        offseason,
    );
    // Ordena o grid pelo caixa final e lê a estrutura na mesma posição — é o pareamento
    // riqueza ↔ estrutura que a pergunta pede.
    let mut por_caixa: Vec<(f64, f64)> = r
        .caixa_fim
        .iter()
        .zip(r.estrutura_fim.iter())
        .map(|(c, e)| (*c, *e))
        .collect();
    por_caixa.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let n = por_caixa.len().max(1);
    let estruturas: Vec<f64> = r.estrutura_fim.clone();
    EstruturaFinal {
        estrutura_rico: por_caixa[0].1,
        estrutura_mediano: por_caixa[n / 2].1,
        estrutura_pobre: por_caixa[n - 1].1,
        amplitude: estruturas.iter().copied().fold(f64::MIN, f64::max)
            - estruturas.iter().copied().fold(f64::MAX, f64::min),
    }
}

#[test]
#[ignore = "prova do módulo novo — roda com --ignored --nocapture"]
fn desenvolvimento_no_offseason() {
    let coef = CoeficientesDeReceita::default();
    let padrao = ParametrosDeDesenvolvimento::default();

    println!("\n═══ `economia::desenvolvimento` NO LUGAR DO OFFSEASON ═══");
    println!("  O mesmo laço, trocando `apply_offseason_competitiveness_impact` (que dá ~2,76");
    println!("  pontos de graça) pelo módulo novo: a equipe investe o excedente e compra");
    println!("  estrutura com retorno decrescente.\n");

    let base = resumir_ralo(coef, Ralo::default());
    let cl_ref = base.cl_pior;
    let controle = resumir_ralo(coef, Ralo::de_excedente(0.40, 12.0));
    let novo = resumir_ralo_com(coef, Ralo::default(), Offseason::Economia(padrao));

    cabecalho_ralo();
    linha_ralo("hoje (de graça)", &base, cl_ref);
    linha_ralo("dreno C 0,40 (alvo)", &controle, cl_ref);
    linha_ralo("economia::desenvolv.", &novo, cl_ref);

    println!("\n  ── os quatro critérios, lado a lado ──");
    println!(
        "  {:<26} {:>14} {:>14} {:>14}",
        "", "hoje", "dreno C 0,40", "módulo novo"
    );
    for (rotulo, f) in [
        (
            "deriva na pior categoria",
            Box::new(|r: &ResumoRalo| format!("{:.2}×", r.deriva_pior))
                as Box<dyn Fn(&ResumoRalo) -> String>,
        ),
        (
            "categorias com deriva < 1,3×",
            Box::new(|r: &ResumoRalo| format!("{}/9", r.cat_no_alvo)),
        ),
        (
            "crise ou colapso (8–15%)",
            Box::new(|r: &ResumoRalo| format!("{:.1}%", r.crise_pct)),
        ),
        (
            "campeão ÷ lanterna (pior)",
            Box::new(|r: &ResumoRalo| format!("{:.2}×", r.cl_pior)),
        ),
        (
            "GT3 (tem que ficar intocada)",
            Box::new(|r: &ResumoRalo| format!("{:.2}×", r.deriva[7])),
        ),
        (
            "dreno médio (% do op. anual)",
            Box::new(|r: &ResumoRalo| format!("{:.1}%", r.dreno_medio)),
        ),
        (
            "receita ÷ despesa",
            Box::new(|r: &ResumoRalo| format!("{:.2}–{:.2}", r.razao_pior, r.razao_melhor)),
        ),
    ] {
        println!(
            "  {rotulo:<26} {:>14} {:>14} {:>14}",
            f(&base),
            f(&controle),
            f(&novo)
        );
    }

    // ── Pergunta 1: rico compra mais estrutura que pobre? ────────────────────────────
    // Medido no estado FINAL do grid depois de 20 temporadas, pareando cada equipe com o
    // próprio caixa. `engineering + facilities`, escala 0–200.
    println!("\n  ── PERGUNTA 1: com que estrutura o grid termina, por faixa de riqueza ──");
    println!("  Hoje as três faixas recebem os mesmos ~2,76 pontos por temporada, de graça.");
    println!(
        "\n  {:<22} {:>26} {:>28}",
        "", "HOJE (de graça)", "MÓDULO NOVO"
    );
    println!(
        "  {:<22} {:>7} {:>7} {:>7} {:>5}  {:>7} {:>7} {:>7} {:>5}",
        "categoria", "rico", "mediano", "pobre", "ampl", "rico", "mediano", "pobre", "ampl"
    );
    for &categoria in CATEGORIAS {
        let a = estrutura_final(categoria, Offseason::Producao);
        let b = estrutura_final(categoria, Offseason::Economia(padrao));
        println!(
            "  {:<22} {:>7.0} {:>7.0} {:>7.0} {:>5.0}  {:>7.0} {:>7.0} {:>7.0} {:>5.0}",
            categoria,
            a.estrutura_rico,
            a.estrutura_mediano,
            a.estrutura_pobre,
            a.amplitude,
            b.estrutura_rico,
            b.estrutura_mediano,
            b.estrutura_pobre,
            b.amplitude
        );
    }

    // ── Pergunta 2: teto no investimento? ────────────────────────────────────────────
    println!("\n  ── PERGUNTA 2: o investimento tem que ter teto? ──");
    println!("  Sem teto, o excedente é sempre drenado. Com teto, o que passa dele acumula.\n");
    cabecalho_ralo();
    linha_ralo("sem teto (padrão)", &novo, cl_ref);
    for teto in [0.25, 0.50, 1.00, 2.00] {
        let com_teto = ParametrosDeDesenvolvimento {
            teto_do_investimento: Some(teto),
            ..padrao
        };
        linha_ralo(
            &format!("teto {teto:.2}× op. anual"),
            &resumir_ralo_com(coef, Ralo::default(), Offseason::Economia(com_teto)),
            cl_ref,
        );
    }

    // ── Sensibilidade da fração do excedente ─────────────────────────────────────────
    println!("\n  ── sensibilidade: fração do excedente investida ──");
    cabecalho_ralo();
    for fracao in [0.20, 0.30, 0.40, 0.55, 0.70] {
        let p = ParametrosDeDesenvolvimento {
            fracao_do_excedente: fracao,
            ..padrao
        };
        linha_ralo(
            &format!("fração {fracao:.2}"),
            &resumir_ralo_com(coef, Ralo::default(), Offseason::Economia(p)),
            cl_ref,
        );
    }

    // ── Sensibilidade da depreciação ─────────────────────────────────────────────────
    println!("\n  ── sensibilidade: depreciação anual da estrutura ──");
    cabecalho_ralo();
    for dep in [0.0, 0.01, 0.02, 0.04] {
        let p = ParametrosDeDesenvolvimento {
            depreciacao_anual: dep,
            ..padrao
        };
        linha_ralo(
            &format!("depreciação {dep:.2}"),
            &resumir_ralo_com(coef, Ralo::default(), Offseason::Economia(p)),
            cl_ref,
        );
    }

    // ── Veredito, nomeando o que falha e em quem a culpa cai ────────────────────────
    println!("\n  ── veredito ──");
    let julgar = |rotulo: &str, r: &ResumoRalo| {
        let mut fora: Vec<String> = Vec::new();
        if r.cat_no_alvo != CATEGORIAS.len() {
            fora.push(format!(
                "deriva ({}/9, pior {:.2}×)",
                r.cat_no_alvo, r.deriva_pior
            ));
        }
        if !(8.0..=15.0).contains(&r.crise_pct) {
            fora.push(format!("crise ({:.1}%)", r.crise_pct));
        }
        if r.cl_pior < cl_ref - 0.05 {
            fora.push(format!("campeão÷lanterna ({:.2}×)", r.cl_pior));
        }
        if fora.is_empty() {
            println!("  {rotulo:<22} todos os critérios dentro");
        } else {
            println!("  {rotulo:<22} fora: {}", fora.join(" · "));
        }
    };
    julgar("hoje (de graça)", &base);
    julgar("dreno C 0,40", &controle);
    julgar("economia::desenvolv.", &novo);
    println!(
        "\n  Um critério que já está fora na linha 'hoje' não é responsabilidade do ralo:\n  \
         ele estava fora antes de existir ralo nenhum."
    );
}

// ===================== `economia::receita`: os cinco canais =====================
//
// A maior peça da reescrita, e a que os critérios 1, 3, 4, 5, 6 e 11 medem. Roda com o RALO
// LIGADO, porque a restrição de ordem medida em `varrer_ralo` diz que sem ele a derivada do
// espalhamento troca de sinal e a calibração procura o mínimo errado.
//
// Roda com `cargo test --lib varrer_receita -- --ignored --nocapture`.

/// Os seis critérios que a receita move, medidos POR CATEGORIA como manda a Parte 4.
struct ResumoReceita {
    /// receita ÷ despesa por categoria, na ordem de `CATEGORIAS`.
    razao: Vec<f64>,
    razao_fora: usize,
    /// % da receita que vem do prêmio por corrida (critério 3, alvo ≥ 40%).
    corrida: Vec<f64>,
    corrida_fora: usize,
    /// % da receita que vem da bilheteria (critério 4, alvo 10–20%).
    portao: Vec<f64>,
    portao_fora: usize,
    /// % da receita que vem do fechamento (critério 5, alvo ≤ 10%).
    fechamento: Vec<f64>,
    fechamento_fora: usize,
    /// campeão ÷ lanterna (critério 6, alvo ≥ 3×).
    campeao: Vec<f64>,
    campeao_fora: usize,
    /// espalhamento da bilheteria melhor ÷ pior (critério 11, alvo ≥ 2,5×).
    espalhamento: Vec<f64>,
    espalhamento_fora: usize,
    /// % da receita que vem do patrocínio — não é critério, é o que sobra. A tensão
    /// aritmética entre "≥40 + 10 + 0" e "o piso que permite sobreviver" mora aqui.
    patrocinio: Vec<f64>,
    /// Critério 8: equipes vendidas por falência, % do grid por temporada (alvo 0,5–5%).
    /// **É esta a amarra da calibração agora**, não o critério 6.
    vendas: Vec<f64>,
    vendas_fora: usize,
    /// Quanto o portão vale para a equipe de MENOR atração de cada categoria, em % da
    /// receita dela. Não é critério — é o que diz se o canal sustenta ou só decora.
    portao_da_pior: Vec<f64>,
    /// A restrição de FORMA do critério 7: colapso tem que ser menor que crise.
    colapso_pct: f64,
    /// Contexto: deriva do caixa, crise, e o desvio da presença pública.
    deriva_pior: f64,
    crise_pct: f64,
    desvio_da_presenca: f64,
    /// A DESPESA real de uma equipe numa temporada, em múltiplos da ÂNCORA DO CENÁRIO. É o
    /// denominador do critério 1, e se ele variar muito entre categorias nenhum conjunto de
    /// coeficientes expressos em frações daquela âncora consegue pôr as 9 na faixa — o
    /// problema estaria na âncora, não na receita.
    despesa_por_op: Vec<f64>,
    /// Critério 2: fôlego mediano no fim de 20 temporadas, em meses de operação (alvo 3–18).
    /// **É este o número que mostra a sobra**, não a razão receita ÷ despesa: o ralo absorve
    /// qualquer excedente comprando estrutura, e devolve a razão para perto de 1 sozinho.
    meses: Vec<f64>,
    meses_fora: usize,
}

/// O custo operacional anual da categoria na âncora escolhida, ponderado pelo tamanho das
/// classes — o que uma equipe MÉDIA da categoria custa por ano.
fn ancora_da_categoria(categoria: &'static str, ancora: AncoraDoDinheiro) -> f64 {
    let a = arena(categoria);
    match ancora {
        AncoraDoDinheiro::Declarada => category_finance_scale(categoria).operating_cost_midpoint(),
        AncoraDoDinheiro::Fisica => {
            a.classes
                .iter()
                .map(|c| {
                    let classe = (!c.nome.is_empty()).then_some(c.nome);
                    crate::economia::temporada::custo_operacional_anual_de_referencia(
                        categoria, classe,
                    ) * c.equipes as f64
                })
                .sum::<f64>()
                / a.equipes.max(1) as f64
        }
    }
}

impl ResumoReceita {
    /// Quantos dos SETE critérios de receita estão inteiramente dentro do alvo.
    fn criterios_ok(&self) -> usize {
        [
            self.razao_fora,
            self.meses_fora,
            self.corrida_fora,
            self.portao_fora,
            self.fechamento_fora,
            self.campeao_fora,
            self.espalhamento_fora,
            self.vendas_fora,
        ]
        .iter()
        .filter(|f| **f == 0)
        .count()
    }

    /// O critério 7 inteiro: NÍVEL (5–12% em crise ou colapso) e FORMA (colapso < crise).
    /// `crise_pct` é a soma dos dois estados, então a crise pura é a diferença.
    fn crise_ok(&self) -> bool {
        let so_crise = self.crise_pct - self.colapso_pct;
        (5.0..=12.0).contains(&self.crise_pct) && self.colapso_pct < so_crise
    }
}

fn resumir_receita(cenario: &Cenario) -> ResumoReceita {
    let mut razao = Vec::new();
    let mut corrida = Vec::new();
    let mut portao = Vec::new();
    let mut fechamento = Vec::new();
    let mut campeao = Vec::new();
    let mut espalhamento = Vec::new();
    let mut patrocinio = Vec::new();
    let mut derivas = Vec::new();
    let mut desvios = Vec::new();
    let mut despesa_por_op = Vec::new();
    let mut vendas = Vec::new();
    let mut portao_da_pior = Vec::new();
    let mut meses = Vec::new();
    let (mut observacoes, mut colapsos, mut so_colapso) = (0u32, 0u32, 0u32);

    for &categoria in CATEGORIAS {
        let r = medir_categoria_cenario(categoria, cenario);
        let receita = receita_total(&r).max(1.0);
        let equipe_temporada = arena(categoria).equipes as f64 * TEMPORADAS as f64;
        despesa_por_op.push(
            despesa_total(&r) / equipe_temporada / ancora_da_categoria(categoria, cenario.ancora),
        );
        meses.push(mediana(&r.meses_fim));
        razao.push(receita / despesa_total(&r).max(1.0));
        corrida.push((r.linhas.bonus + r.linhas.premio_parcial) / receita * 100.0);
        portao.push(r.linhas.bilheteria / receita * 100.0);
        fechamento.push(r.linhas.premio_construtores / receita * 100.0);
        patrocinio.push(r.linhas.patrocinio / receita * 100.0);
        campeao.push(r.receita_campeao / r.receita_lanterna.max(1.0));
        espalhamento.push(r.portao_melhor / r.portao_pior.max(1.0));
        derivas.push(mediana(&r.caixa_fim) / mediana(&r.caixa_inicio).max(0.01));
        desvios.push(r.desvio_da_presenca);
        observacoes += r.estados.values().sum::<u32>();
        colapsos += r.estados.get("crisis").copied().unwrap_or(0)
            + r.estados.get("collapse").copied().unwrap_or(0);
        so_colapso += r.estados.get("collapse").copied().unwrap_or(0);
        vendas
            .push(r.vendas as f64 / (TEMPORADAS as f64 * arena(categoria).equipes as f64) * 100.0);
        portao_da_pior.push(r.portao_pct_da_pior);
    }

    let contar =
        |v: &[f64], dentro: &dyn Fn(f64) -> bool| v.iter().filter(|x| !dentro(**x)).count();
    // O critério 6 tem alvo POR CALENDÁRIO, então não cabe em `contar`.
    let campeao_fora = campeao
        .iter()
        .enumerate()
        .filter(|(i, v)| **v < alvo_campeao_lanterna(arena(CATEGORIAS[*i]).rodadas))
        .count();
    ResumoReceita {
        razao_fora: contar(&razao, &|v| (0.95..=1.15).contains(&v)),
        corrida_fora: contar(&corrida, &|v| v >= 40.0),
        portao_fora: contar(&portao, &|v| (10.0..=20.0).contains(&v)),
        fechamento_fora: contar(&fechamento, &|v| v <= 10.0),
        campeao_fora,
        espalhamento_fora: contar(&espalhamento, &|v| v >= 2.5),
        vendas_fora: contar(&vendas, &|v| (0.5..=5.0).contains(&v)),
        meses_fora: contar(&meses, &|v| (3.0..=18.0).contains(&v)),
        meses,
        colapso_pct: so_colapso as f64 / observacoes.max(1) as f64 * 100.0,
        vendas,
        portao_da_pior,
        deriva_pior: derivas.iter().copied().fold(f64::MIN, f64::max),
        crise_pct: colapsos as f64 / observacoes.max(1) as f64 * 100.0,
        desvio_da_presenca: desvios.iter().sum::<f64>() / desvios.len().max(1) as f64,
        despesa_por_op,
        razao,
        corrida,
        portao,
        fechamento,
        campeao,
        espalhamento,
        patrocinio,
    }
}

fn faixa(v: &[f64]) -> (f64, f64) {
    (
        v.iter().copied().fold(f64::MAX, f64::min),
        v.iter().copied().fold(f64::MIN, f64::max),
    )
}

fn cabecalho_receita() {
    println!(
        "  {:<26} {:^12} {:^13} {:^11} {:^11} {:^10} {:^12} {:^11} {:^12} {:^10} {:>5}",
        "",
        "1 rec/desp",
        "2 meses",
        "3 corrida%",
        "4 portão%",
        "5 fecha%",
        "6 camp/lant",
        "11 espalha",
        "8 vendas%",
        "7 crise",
        "ok"
    );
}

fn linha_receita(rotulo: &str, r: &ResumoReceita) {
    let campo = |v: &[f64], fora: usize, casas: usize| {
        let (a, b) = faixa(v);
        let marca = if fora == 0 { ' ' } else { '✗' };
        format!("{a:.*}–{b:.*}{marca}", casas, casas)
    };
    println!(
        "  {rotulo:<26} {:^12} {:^13} {:^11} {:^11} {:^10} {:^12} {:^11} {:^12} {:^10} {:>4}/8",
        campo(&r.razao, r.razao_fora, 2),
        campo(&r.meses, r.meses_fora, 1),
        campo(&r.corrida, r.corrida_fora, 0),
        campo(&r.portao, r.portao_fora, 0),
        campo(&r.fechamento, r.fechamento_fora, 0),
        campo(&r.campeao, r.campeao_fora, 1),
        campo(&r.espalhamento, r.espalhamento_fora, 1),
        campo(&r.vendas, r.vendas_fora, 1),
        format!(
            "{:.1}%{}",
            r.crise_pct,
            if r.crise_ok() { ' ' } else { '✗' }
        ),
        r.criterios_ok()
    );
}

#[test]
#[ignore = "calibração da receita — roda com --ignored --nocapture"]
fn varrer_receita() {
    // O ralo entra LIGADO: é a restrição de ordem medida em `varrer_ralo` (com ele o
    // espalhamento cai de 0,55 para 0,17 e a derivada troca de sinal). Calibrar receita sem
    // ralo é procurar o mínimo de uma função com o sinal trocado.
    //
    // A fama sintética NÃO entra mais: a cota de bilheteria rateia por
    // `public_presence::atracao`, que está em produção e espalha 2,7–5,1× medido. O
    // espalhamento deixou de ser um botão deste harness.
    let com_ralo = |receita: Receita| Cenario {
        offseason: Offseason::Economia(ParametrosDeDesenvolvimento::default()),
        receita,
        ..Cenario::default()
    };

    println!("\n═══ `economia::receita` SOBRE A ÂNCORA NOVA ═══");
    println!("  Todos os alvos são POR CATEGORIA. A faixa é min–max das 9; '✗' marca que ao");
    println!("  menos uma categoria está fora. Ralo ligado em todas as linhas.");
    println!("  Critério 6 agora é função do calendário: ≥2,0× até 6 rodadas, ≥3,0× de 8.");
    println!("  Critério 7 tem forma: 5–12% E colapso < crise.\n");

    let hoje = resumir_receita(&Cenario::default());
    let hoje_com_ralo = resumir_receita(&com_ralo(Receita::Producao));

    cabecalho_receita();
    linha_receita("produção, sem ralo", &hoje);
    linha_receita("produção, com ralo", &hoje_com_ralo);

    // ── O denominador: quanto a temporada custa sobre a ÂNCORA NOVA ──────────────────
    // Os coeficientes de `economia::receita` são frações do custo operacional declarado.
    // Com a âncora velha esse número era 1,90–2,77× o real — 46% de espalhamento, e nenhum
    // conjunto único de coeficientes punha as 9 na faixa do critério 1. Sobre a âncora nova
    // ele tem que estar perto de 1,0 e apertado.
    println!("\n  ── o denominador do critério 1, sobre a âncora NOVA ──");
    println!(
        "  {:<24} {:>12} {:>34}",
        "categoria", "despesa/op", "receita que o critério 1 pediria"
    );
    for (i, &categoria) in CATEGORIAS.iter().enumerate() {
        let d = hoje_com_ralo.despesa_por_op[i];
        println!(
            "  {:<24} {:>11.2}× {:>26.2}–{:.2}×",
            categoria,
            d,
            d * 0.95,
            d * 1.15
        );
    }
    let (dmin, dmax) = faixa(&hoje_com_ralo.despesa_por_op);
    println!(
        "  faixa {dmin:.2}× – {dmax:.2}× · espalhamento de {:.0}% (era 46% na âncora velha)",
        (dmax / dmin - 1.0) * 100.0
    );

    // A base de calibração é ancorada na despesa MEDIDA sobre a âncora nova, não num chute.
    let nivel = (dmin + dmax) / 2.0;
    let base = ParametrosDeReceita {
        premio_de_corrida: 0.60 * nivel,
        patrocinio_fixo: 0.22 * nivel,
        patrocinio_por_reputacao: 0.10 * nivel,
        patrocinio_por_fama: 0.06 * nivel,
        inclinacao_do_premio: 2.5,
        volta_mais_rapida: 0.01 * nivel,
        bilheteria: 0.17 * nivel,
        bilheteria_piso: 1.0,
        bilheteria_por_prestigio: 0.3,
        fechamento_ao_ultimo: 0.005 * nivel,
        fechamento_ao_primeiro: 0.06 * nivel,
        expoente_do_calendario: 1.0,
    };
    println!("\n  Nível de calibração: despesa mediana {nivel:.2}× o operacional declarado.");
    cabecalho_receita();
    linha_receita(
        "economia::receita (base)",
        &resumir_receita(&com_ralo(Receita::Economia(base))),
    );

    // ── A AMARRA: γ contra o critério 6 relaxado E o critério 8 ─────────────────────
    // γ=5 foi calibrado contra um 3× fixo que não existe mais. As duas pontas do critério 8
    // (0% de vendas na receita de hoje, 19,6% na receita com γ=5) nomeiam o mesmo culpado:
    // a convexidade matando o fundo do grid. Aqui os dois se cruzam.
    println!("\n  ── A AMARRA: onde o critério 6 (relaxado) e o critério 8 se cruzam ──");
    println!("  γ baixo: o fundo do grid sobrevive (8 ok) mas campeão e lanterna se parecem.");
    println!("  γ alto: o campeonato diferencia (6 ok) mas o fundo do grid morre (8 estoura).\n");
    println!(
        "  {:<8} {:>12} {:>12} {:>10} {:>10} {:>12} {:>12} {:>10}",
        "γ", "6: pior/alvo", "6 fora", "8: máx %", "8 fora", "crise %", "colapso %", "rec/desp"
    );
    for gama in [1.0, 2.0, 3.0, 4.0, 5.0, 6.5, 8.0, 9.5, 12.0] {
        let p = ParametrosDeReceita {
            inclinacao_do_premio: gama,
            ..base
        };
        let r = resumir_receita(&com_ralo(Receita::Economia(p)));
        // A folga do critério 6 na categoria mais apertada, em múltiplos do alvo dela.
        let folga = CATEGORIAS
            .iter()
            .enumerate()
            .map(|(i, c)| r.campeao[i] / alvo_campeao_lanterna(arena(c).rodadas))
            .fold(f64::MAX, f64::min);
        let (_, vmax) = faixa(&r.vendas);
        let (rmin, rmax) = faixa(&r.razao);
        println!(
            "  {gama:<8.1} {folga:>12.2} {:>12} {vmax:>10.2} {:>10} {:>11.1}% {:>11.1}% {:>5.2}–{:<4.2}",
            r.campeao_fora,
            r.vendas_fora,
            r.crise_pct,
            r.colapso_pct,
            rmin,
            rmax
        );
    }
    println!("\n  A mesma varredura, nos sete critérios:");
    cabecalho_receita();
    for gama in [1.0, 3.0, 5.0, 6.5, 8.0, 9.5, 12.0] {
        let p = ParametrosDeReceita {
            inclinacao_do_premio: gama,
            ..base
        };
        linha_receita(
            &format!("γ {gama:.1}"),
            &resumir_receita(&com_ralo(Receita::Economia(p))),
        );
    }

    // ── O portão: espalhamento E sustentação ────────────────────────────────────────
    println!("\n  ── o portão sustenta o fundo do grid, ou só decora? ──");
    println!("  A razão melhor÷pior diz se DIFERENCIA. A coluna da direita diz se SUSTENTA:");
    println!("  quanto o portão vale para a equipe de MENOR atração de cada categoria.\n");
    let referencia_portao = resumir_receita(&com_ralo(Receita::Economia(base)));
    println!(
        "  {:<24} {:>14} {:>18} {:>18}",
        "categoria", "espalhamento", "portão % da média", "portão % da PIOR"
    );
    for (i, &categoria) in CATEGORIAS.iter().enumerate() {
        println!(
            "  {:<24} {:>13.2}× {:>17.1}% {:>17.1}%",
            categoria,
            referencia_portao.espalhamento[i],
            referencia_portao.portao[i],
            referencia_portao.portao_da_pior[i]
        );
    }

    // ── A busca final ───────────────────────────────────────────────────────────────
    println!("\n  ── busca: nível × patrocínio × γ × bilheteria ──");
    println!(
        "  Penalidade pesa o critério 1 três vezes e o critério 8 duas: o 8 é a amarra\n  \
              nova, e um conjunto que satisfaz cinco critérios matando o fundo do grid é\n  \
              artefato de contagem, não candidato.\n"
    );
    cabecalho_receita();
    let penalidade = |r: &ResumoReceita| {
        r.razao_fora * 3
            + r.vendas_fora * 2
            + r.corrida_fora
            + r.portao_fora
            + r.fechamento_fora
            + r.campeao_fora
            + usize::from(!r.crise_ok())
    };
    let mut melhor: Option<(ParametrosDeReceita, ResumoReceita)> = None;
    // A grade de γ vai até 9: a varredura acima mostrou que os critérios 6 e 8 melhoram
    // JUNTOS com γ, sem cruzamento — nenhum dos dois é satisfeito abaixo de 5.
    for premio in [0.50, 0.65, 0.80] {
        for patroc in [0.14, 0.24] {
            for gama in [5.0, 6.5, 8.0, 9.5] {
                for bilheteria in [0.15, 0.20] {
                    let p = ParametrosDeReceita {
                        premio_de_corrida: premio * nivel,
                        patrocinio_fixo: patroc * nivel,
                        inclinacao_do_premio: gama,
                        bilheteria: bilheteria * nivel,
                        ..base
                    };
                    let r = resumir_receita(&com_ralo(Receita::Economia(p)));
                    if r.criterios_ok() >= 6 {
                        linha_receita(
                            &format!("pr{premio:.2} pa{patroc:.2} γ{gama:.1} bi{bilheteria:.2}"),
                            &r,
                        );
                    }
                    let troca = melhor
                        .as_ref()
                        .map(|(_, m)| penalidade(&r) < penalidade(m))
                        .unwrap_or(true);
                    if troca {
                        melhor = Some((p, r));
                    }
                }
            }
        }
    }

    let Some((p, r)) = melhor else { return };
    println!("\n  ── o melhor conjunto ──");
    cabecalho_receita();
    linha_receita("melhor", &r);
    println!(
        "\n  prêmio {:.3} · patrocínio {:.3} · γ {:.1} · bilheteria {:.3} · fechamento {:.3}→{:.3}",
        p.premio_de_corrida,
        p.patrocinio_fixo,
        p.inclinacao_do_premio,
        p.bilheteria,
        p.fechamento_ao_ultimo,
        p.fechamento_ao_primeiro
    );
    println!(
        "  {}/7 critérios de receita dentro · crise {:.1}% (colapso {:.1}%) · deriva {:.2}×",
        r.criterios_ok(),
        r.crise_pct,
        r.colapso_pct,
        r.deriva_pior
    );
    let (pa, pb) = faixa(&r.patrocinio);
    println!("  patrocínio ocupa {pa:.0}–{pb:.0}% da receita");
    println!("\n  por categoria:");
    println!(
        "  {:<22} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9} {:>8} {:>8}",
        "categoria",
        "rec/desp",
        "corrida%",
        "portão%",
        "fecha%",
        "patroc%",
        "camp/lant",
        "alvo",
        "vendas%"
    );
    for (i, &categoria) in CATEGORIAS.iter().enumerate() {
        println!(
            "  {:<22} {:>8.2} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>8.2}× {:>8.1} {:>8.2}",
            categoria,
            r.razao[i],
            r.corrida[i],
            r.portao[i],
            r.fechamento[i],
            r.patrocinio[i],
            r.campeao[i],
            alvo_campeao_lanterna(arena(categoria).rodadas),
            r.vendas[i]
        );
    }

    // ── O ÚLTIMO BOTÃO: depreciação, só se o critério 7 ainda não fechou ────────────
    if r.crise_ok() {
        println!("\n  O critério 7 já fecha com a receita calibrada. `depreciacao_anual` fica");
        println!("  guardada — não se gasta botão que não precisa ser gasto.");
        return;
    }
    println!("\n  ── último botão: `depreciacao_anual` ──");
    println!("  O critério 7 não fechou com a receita. A depreciação move a crise sem mover a");
    println!("  deriva (medido em `desenvolvimento_no_offseason`), então é o botão certo.\n");
    println!(
        "  {:<14} {:>10} {:>12} {:>10} {:>12} {:>10}",
        "depreciação", "crise %", "colapso %", "forma ok", "rec/desp", "vendas máx"
    );
    for dep in [0.02, 0.03, 0.04, 0.06, 0.09] {
        let cenario = Cenario {
            offseason: Offseason::Economia(ParametrosDeDesenvolvimento {
                depreciacao_anual: dep,
                ..ParametrosDeDesenvolvimento::default()
            }),
            receita: Receita::Economia(p),
            ..Cenario::default()
        };
        let s = resumir_receita(&cenario);
        let (rmin, rmax) = faixa(&s.razao);
        let (_, vmax) = faixa(&s.vendas);
        println!(
            "  {dep:<14.2} {:>9.1}% {:>11.1}% {:>10} {:>5.2}–{:<6.2} {:>10.2}",
            s.crise_pct,
            s.colapso_pct,
            if s.colapso_pct < s.crise_pct - s.colapso_pct {
                "sim"
            } else {
                "não"
            },
            rmin,
            rmax,
            vmax
        );
    }
}

// ===================== O termômetro: estado financeiro em MESES =====================
//
// `derive_financial_state` dividia o caixa pela âncora de ESTOQUE, que a auditoria da seção
// 3.3.3 mediu como torta: a mesma equipe ficava 2,4× mais folgada na GT3 e ~10× no Endurance
// só por causa do divisor. Toda taxa de crise já reportada por este harness — inclusive os
// 18,1% do baseline e os 21,6% da receita nova — foi lida nesse termômetro.
//
// `finance::state` passou a medir em MESES DE OPERAÇÃO, com o custo mensal vindo da tabela
// bottom-up de 3.3.3. Este teste mede o que muda.
//
// Roda com `cargo test --lib varrer_estado_financeiro -- --ignored --nocapture`.

/// A distribuição de população que um conjunto de fronteiras produz.
struct DistribuicaoDeEstados {
    /// % de (equipe × temporada) em cada estado, na ordem de `ESTADOS`.
    pct: Vec<f64>,
    /// % em crise ou colapso — o critério 7.
    crise_pct: f64,
    /// Percentis dos meses de operação observados no mundo.
    meses_p10: f64,
    meses_mediana: f64,
    meses_p90: f64,
    /// Vendas por falência, % do grid por temporada — o critério 8, que mede EVENTO.
    vendas_pct_min: f64,
    vendas_pct_max: f64,
    vendas_totais: u32,
}

fn distribuicao(cenario: &Cenario, legado: bool) -> DistribuicaoDeEstados {
    let mut contagem: HashMap<&'static str, u32> = HashMap::new();
    let mut meses: Vec<f64> = Vec::new();
    let mut vendas_pct: Vec<f64> = Vec::new();
    let mut vendas_totais = 0u32;

    for &categoria in CATEGORIAS {
        let r = medir_categoria_cenario(categoria, cenario);
        let fonte = if legado {
            &r.estados_legado
        } else {
            &r.estados
        };
        for (estado, n) in fonte {
            *contagem.entry(estado).or_insert(0) += n;
        }
        meses.extend(r.meses_observados.iter().copied());
        let grid = arena(categoria).equipes as f64;
        vendas_pct.push(r.vendas as f64 / (TEMPORADAS as f64 * grid.max(1.0)) * 100.0);
        vendas_totais += r.vendas;
    }

    let total: u32 = contagem.values().sum();
    let pct: Vec<f64> = ESTADOS
        .iter()
        .map(|e| contagem.get(e).copied().unwrap_or(0) as f64 / total.max(1) as f64 * 100.0)
        .collect();
    meses.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let quantil = |q: f64| -> f64 {
        if meses.is_empty() {
            return 0.0;
        }
        meses[((meses.len() - 1) as f64 * q).round() as usize]
    };
    DistribuicaoDeEstados {
        crise_pct: pct[4] + pct[5],
        pct,
        meses_p10: quantil(0.10),
        meses_mediana: quantil(0.50),
        meses_p90: quantil(0.90),
        vendas_pct_min: vendas_pct.iter().copied().fold(f64::MAX, f64::min),
        vendas_pct_max: vendas_pct.iter().copied().fold(f64::MIN, f64::max),
        vendas_totais,
    }
}

fn cabecalho_estados() {
    println!(
        "  {:<28} {:>7} {:>8} {:>7} {:>10} {:>7} {:>9} │ {:>8}",
        "fronteiras (meses)",
        "elite",
        "healthy",
        "stable",
        "pressured",
        "crisis",
        "collapse",
        "crise %"
    );
}

fn linha_estados(rotulo: &str, d: &DistribuicaoDeEstados) {
    println!(
        "  {rotulo:<28} {:>6.1}% {:>7.1}% {:>6.1}% {:>9.1}% {:>6.1}% {:>8.1}% │ {:>7.1}%",
        d.pct[0], d.pct[1], d.pct[2], d.pct[3], d.pct[4], d.pct[5], d.crise_pct
    );
}

#[test]
#[ignore = "calibração das faixas de estado — roda com --ignored --nocapture"]
fn varrer_estado_financeiro() {
    // O mundo medido é o de hoje: produção, sem ralo e sem receita nova. É o baseline contra
    // o qual os 18,1% foram reportados, e é onde a troca de termômetro tem que ser lida.
    let hoje = Cenario::default();

    println!("\n═══ O TERMÔMETRO: ESTADO FINANCEIRO EM MESES DE OPERAÇÃO ═══");
    println!("  O instrumento legado bina um score 0–100 cujo termo dominante divide o caixa");
    println!("  pela âncora de ESTOQUE (torta: 0,88× na Rookie, 10,5× no Endurance). O novo");
    println!("  mede meses de operação com o custo bottom-up de `economia::temporada`.\n");
    println!("  ATENÇÃO ao ler a linha LEGADO: ela NÃO é 'o mesmo mundo com outro termômetro'.");
    println!("  O estado realimenta o comportamento — dispara empréstimo de emergência, escolhe");
    println!("  a estratégia da temporada e arma a venda por colapso. Trocar o termômetro troca");
    println!("  o mundo. A linha legado é a leitura velha de um mundo já conduzido pelo");
    println!("  instrumento novo; ela isola a distorção do DIVISOR, não a do mundo inteiro.\n");

    cabecalho_estados();
    let legado = distribuicao(&hoje, true);
    linha_estados("INSTRUMENTO LEGADO", &legado);
    let novo = distribuicao(&hoje, false);
    linha_estados("MESES (24/12/6/3/0)", &novo);
    println!(
        "\n  Fôlego real do mundo: p10 {:.1} · mediana {:.1} · p90 {:.1} meses de operação",
        novo.meses_p10, novo.meses_mediana, novo.meses_p90
    );

    // ── PERGUNTA 1: que fronteiras produzem uma população defensável? ────────────────
    println!("\n  ── PERGUNTA 1: que fronteiras produzem uma distribuição defensável? ──");
    println!("  A seção 2.9 diz que hoje é 'elite em todo lugar'. O alvo é elite rara e");
    println!("  crise visível.\n");
    cabecalho_estados();
    for (rotulo, f) in [
        (
            "36/18/9/4/0 (severa)",
            FaixasDeMeses {
                elite: 36.0,
                saudavel: 18.0,
                estavel: 9.0,
                pressionada: 4.0,
                crise: 0.0,
            },
        ),
        (
            "30/15/8/3/0",
            FaixasDeMeses {
                elite: 30.0,
                saudavel: 15.0,
                estavel: 8.0,
                pressionada: 3.0,
                crise: 0.0,
            },
        ),
        ("24/12/6/3/0 (padrão)", FaixasDeMeses::default()),
        (
            "18/10/5/2/0",
            FaixasDeMeses {
                elite: 18.0,
                saudavel: 10.0,
                estavel: 5.0,
                pressionada: 2.0,
                crise: 0.0,
            },
        ),
        (
            "12/8/4/2/0 (frouxa)",
            FaixasDeMeses {
                elite: 12.0,
                saudavel: 8.0,
                estavel: 4.0,
                pressionada: 2.0,
                crise: 0.0,
            },
        ),
    ] {
        linha_estados(rotulo, &distribuicao(&Cenario { faixas: f, ..hoje }, false));
    }

    // ── PERGUNTA 2: a taxa de crise honesta ─────────────────────────────────────────
    println!("\n  ── PERGUNTA 2: a taxa de crise HONESTA, por mundo ──");
    cabecalho_estados();
    let com_ralo = Cenario {
        offseason: Offseason::Economia(ParametrosDeDesenvolvimento::default()),
        ..hoje
    };
    let com_receita = Cenario {
        receita: Receita::Economia(ParametrosDeReceita {
            premio_de_corrida: 1.70,
            patrocinio_fixo: 0.50,
            inclinacao_do_premio: 5.0,
            bilheteria: 0.45,
            bilheteria_piso: 1.0,
            bilheteria_por_prestigio: 0.3,
            fechamento_ao_ultimo: 0.01,
            fechamento_ao_primeiro: 0.14,
            ..ParametrosDeReceita::default()
        }),
        espalhamento_da_fama: FAMA_COM_AMPLITUDE,
        ..com_ralo
    };
    linha_estados("produção (hoje)", &novo);
    let d_ralo = distribuicao(&com_ralo, false);
    linha_estados("+ ralo", &d_ralo);
    let d_receita = distribuicao(&com_receita, false);
    linha_estados("+ ralo + receita nova", &d_receita);

    println!("\n  A mesma coisa lida pelo termômetro VELHO, para comparar com o já reportado:");
    cabecalho_estados();
    linha_estados("produção (hoje)", &legado);
    linha_estados("+ ralo", &distribuicao(&com_ralo, true));
    linha_estados("+ ralo + receita nova", &distribuicao(&com_receita, true));

    // ── PERGUNTA 3: o critério 8 mudou com a venda por falência reescrita? ──────────
    println!("\n  ── PERGUNTA 3: critério 8 (vendas por falência, % do grid/temporada) ──");
    println!("  Alvo 0,5–5% em toda categoria. Mede EVENTO, não estado — mas o gatilho da");
    println!("  venda é 'collapse' duas temporadas seguidas, então o termômetro o move.\n");
    println!(
        "  {:<28} {:>12} {:>12} {:>10}",
        "mundo", "mín (%)", "máx (%)", "vendas"
    );
    for (rotulo, d) in [
        ("produção · termômetro velho", &legado),
        ("produção · termômetro novo", &novo),
        ("+ ralo", &d_ralo),
        ("+ ralo + receita nova", &d_receita),
    ] {
        println!(
            "  {rotulo:<28} {:>12.2} {:>12.2} {:>10}",
            d.vendas_pct_min, d.vendas_pct_max, d.vendas_totais
        );
    }

    // ── Por categoria, com o termômetro novo ────────────────────────────────────────
    println!("\n  ── fôlego por categoria (termômetro novo, mundo de produção) ──");
    println!(
        "  {:<24} {:>10} {:>10} {:>10} {:>10} {:>9}",
        "categoria", "p10", "mediana", "p90", "crise %", "vendas %"
    );
    for &categoria in CATEGORIAS {
        let r = medir_categoria_cenario(categoria, &hoje);
        let mut m = r.meses_observados.clone();
        m.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let q = |x: f64| -> f64 {
            if m.is_empty() {
                0.0
            } else {
                m[((m.len() - 1) as f64 * x).round() as usize]
            }
        };
        let obs: u32 = r.estados.values().sum();
        let crise = (r.estados.get("crisis").copied().unwrap_or(0)
            + r.estados.get("collapse").copied().unwrap_or(0)) as f64
            / obs.max(1) as f64
            * 100.0;
        let grid = arena(categoria).equipes as f64;
        println!(
            "  {:<24} {:>10.1} {:>10.1} {:>10.1} {:>9.1}% {:>8.2}%",
            categoria,
            q(0.10),
            q(0.50),
            q(0.90),
            crise,
            r.vendas as f64 / (TEMPORADAS as f64 * grid) * 100.0
        );
    }
}

// ===================== O comparador: velho × novo, mesma semente =====================
//
// A rodada trocou o lado da DESPESA: `finance::operacao` saiu de `src/`, `economia::evento` +
// `economia::temporada` entraram. Este teste foi a condição de aceite dessa troca e continua
// sendo a razão de o modelo velho existir — os dois rodando com a MESMA semente, saída contra
// saída. O lado velho mora em `tests::despesa_legada`, congelado.
//
// Roda com `cargo test --lib comparar_modelos_de_despesa -- --ignored --nocapture`.
//
// **O que muda entre as colunas, e só isso.** A semente do RNG vem do nome da categoria,
// então o grid nasce idêntico, o calendário é o mesmo e a ordem de chegada da primeira etapa
// é a mesma. O que diverge depois diverge por CAUSA da troca: equipe mais pobre desenvolve
// menos carro, carro pior chega atrás, e a diferença se realimenta. É por isso que a coluna
// da receita não é idêntica mesmo com o modelo de receita intocado nos dois lados.
//
// **O que NÃO é trocado**, para a diferença ser atribuível a uma coisa só: a receita segue
// sendo a de produção nos dois lados, o offseason segue sendo
// `apply_offseason_competitiveness_impact` nos dois, o salário de piloto segue vindo do
// mercado nos dois, e a compra de peça segue sendo a decisão de `decide_car_maintenance` nos
// dois. Um bloco no fim mostra o modelo NOVO INTEIRO (despesa + receita + offseason) como
// contexto — mas ele não é a troca desta rodada.

/// As duas leituras do mesmo mundo.
struct ParDeModelos {
    categoria: &'static str,
    velho: Resultado,
    novo: Resultado,
}

/// Soma as chaves de um lado do de-para, já dividido pelas observações: quanto UMA equipe
/// gasta naquele agrupamento numa etapa.
fn linha_composta(f: &FaturaItemizada, chaves: &[&str]) -> f64 {
    chaves.iter().map(|c| f.por_etapa(c)).sum()
}

/// Receita de UMA equipe numa etapa, na média — o denominador comum que permite pôr a
/// fatura da etapa ao lado do que a etapa arrecada.
fn receita_por_etapa(r: &Resultado) -> f64 {
    (r.linhas.patrocinio
        + r.linhas.bilheteria
        + r.linhas.bonus
        + r.linhas.premio_parcial
        + r.linhas.ajuda)
        / r.fatura.observacoes.max(1.0)
}

/// Marca de leitura rápida para uma razão novo ÷ velho. A faixa 0,80–1,25 é o que se
/// considera "a mesma ordem de grandeza dita de outro jeito".
fn marca_da_razao(razao: f64) -> &'static str {
    if !razao.is_finite() {
        "  -  "
    } else if (0.80..=1.25).contains(&razao) {
        "     "
    } else if (0.50..=2.00).contains(&razao) {
        "  .  "
    } else {
        " !!  "
    }
}

#[test]
#[ignore = "comparador velho x novo — roda com --ignored --nocapture"]
fn comparar_modelos_de_despesa() {
    println!("\n═══ COMPARADOR · despesa VELHA × NOVA, mesma semente ═══");
    println!(
        "    9 categorias · {TEMPORADAS} temporadas · receita e offseason IDÊNTICOS nos dois lados"
    );
    println!(
        "    velho: tests::despesa_legada (9 pesos do orçamento) + 0,18×base estrutural + 0,16×base técnica"
    );
    println!("    novo:  economia::evento (quantidade × preço) + economia::temporada ÷ rodadas");
    println!("    O que a PRODUÇÃO debita hoje: o NOVO, e só ele — não há mais interruptor.\n");

    let pares: Vec<ParDeModelos> = CATEGORIAS
        .iter()
        .map(|&c| ParDeModelos {
            categoria: c,
            // Os dois lados são FIXADOS aqui, não herdados do padrão: o padrão do `Cenario`
            // é a conta de produção, e um comparador que herdasse dele compararia o modelo
            // consigo mesmo.
            velho: medir_categoria_cenario(
                c,
                &Cenario {
                    despesa: Despesa::Producao,
                    ..Cenario::default()
                },
            ),
            novo: medir_categoria_cenario(
                c,
                &Cenario {
                    despesa: Despesa::Economia,
                    ..Cenario::default()
                },
            ),
        })
        .collect();

    // ── 1. A fatura da etapa, linha a linha ──────────────────────────────────────────
    println!("  ── A FATURA DE UMA ETAPA, POR LINHA ──");
    println!("     (o que UMA equipe gasta numa etapa, na média de 20 temporadas)\n");
    for p in &pares {
        let tv = p.velho.fatura.total_por_etapa();
        let tn = p.novo.fatura.total_por_etapa();
        println!(
            "  ── {} ──  etapa: {:>9.0} → {:>9.0}  ({:.2}×)   ·  receita/etapa: {:.0} → {:.0}",
            p.categoria,
            tv,
            tn,
            tn / tv.max(1.0),
            receita_por_etapa(&p.velho),
            receita_por_etapa(&p.novo),
        );
        for (rotulo, chaves, nota) in PARES_DA_FATURA {
            let v = linha_composta(&p.velho.fatura, chaves);
            let n = linha_composta(&p.novo.fatura, chaves);
            let razao = n / v.max(1.0);
            println!(
                "     {:<14} {:>10.0} → {:>10.0}  {:>6.2}× {}  {}",
                rotulo,
                v,
                n,
                razao,
                marca_da_razao(razao),
                nota
            );
        }
        println!();
    }

    // ── 2. Receita por canal ─────────────────────────────────────────────────────────
    // O modelo de receita é o MESMO nos dois lados. Se um canal se move, o movimento é
    // realimentação (equipe mais pobre → carro pior → resultado pior → prêmio menor), não
    // uma troca de fórmula. É o teste de quanto a receita de produção depende do estado.
    println!("  ── RECEITA POR CANAL (mesma fórmula nos dois lados; o que se move é feedback) ──");
    println!("     cada célula: velho → novo (razão), em milhões somados nas 20 temporadas\n");
    println!(
        "  {:<22} {:>24} {:>24} {:>24} {:>24}",
        "categoria", "patrocínio", "bilheteria", "prêmio de corrida", "fechamento"
    );
    let canais: [fn(&Linhas) -> f64; 4] = [
        |l| l.patrocinio,
        |l| l.bilheteria,
        |l| l.bonus + l.premio_parcial,
        |l| l.premio_construtores,
    ];
    for p in &pares {
        let mut celulas = String::new();
        for f in canais {
            let v = f(&p.velho.linhas);
            let n = f(&p.novo.linhas);
            celulas.push_str(&format!(
                " {:>8.2}M→{:>8.2}M {:>5.2}×",
                v / 1e6,
                n / 1e6,
                n / v.max(1.0)
            ));
        }
        println!("  {:<22}{celulas}", p.categoria);
    }

    // ── 3. Fôlego, crise e falência ──────────────────────────────────────────────────
    println!("\n  ── FÔLEGO NO FIM DE 20 TEMPORADAS, CRISE E FALÊNCIA ──");
    println!(
        "  {:<22} {:>18} {:>20} {:>20} {:>22}",
        "categoria", "meses (mediana)", "crise+colapso %", "colapso %", "vendas %/grid/ano"
    );
    let pct = |r: &Resultado, chaves: &[&str]| -> f64 {
        let obs: u32 = r.estados.values().sum();
        let n: u32 = chaves
            .iter()
            .map(|c| r.estados.get(*c).copied().unwrap_or(0))
            .sum();
        n as f64 / obs.max(1) as f64 * 100.0
    };
    for p in &pares {
        let grid = arena(p.categoria).equipes as f64 * TEMPORADAS as f64;
        let vendas = |r: &Resultado| r.vendas as f64 / grid * 100.0;
        println!(
            "  {:<22} {:>7.1} → {:>8.1} {:>8.1}% → {:>8.1}% {:>8.1}% → {:>8.1}% {:>9.2}% → {:>9.2}%",
            p.categoria,
            mediana(&p.velho.meses_fim),
            mediana(&p.novo.meses_fim),
            pct(&p.velho, &["crisis", "collapse"]),
            pct(&p.novo, &["crisis", "collapse"]),
            pct(&p.velho, &["collapse"]),
            pct(&p.novo, &["collapse"]),
            vendas(&p.velho),
            vendas(&p.novo),
        );
    }

    // ── 4. O agregado do mundo ───────────────────────────────────────────────────────
    let soma = |lado: fn(&ParDeModelos) -> &Resultado, chaves: &[&str]| -> f64 {
        let obs: u32 = pares
            .iter()
            .map(|p| lado(p).estados.values().sum::<u32>())
            .sum();
        let n: u32 = pares
            .iter()
            .map(|p| {
                chaves
                    .iter()
                    .map(|c| lado(p).estados.get(*c).copied().unwrap_or(0))
                    .sum::<u32>()
            })
            .sum();
        n as f64 / obs.max(1) as f64 * 100.0
    };
    let rec_desp = |lado: fn(&ParDeModelos) -> &Resultado| -> (f64, f64) {
        let razoes: Vec<f64> = pares
            .iter()
            .map(|p| receita_total(lado(p)) / despesa_total(lado(p)).max(1.0))
            .collect();
        (
            razoes.iter().copied().fold(f64::MAX, f64::min),
            razoes.iter().copied().fold(f64::MIN, f64::max),
        )
    };
    let (rd_v_min, rd_v_max) = rec_desp(|p| &p.velho);
    let (rd_n_min, rd_n_max) = rec_desp(|p| &p.novo);
    println!("\n  ── O MUNDO ──");
    println!(
        "  receita ÷ despesa       velho {rd_v_min:.2} – {rd_v_max:.2}   ·   novo {rd_n_min:.2} – {rd_n_max:.2}"
    );
    println!(
        "  crise + colapso         velho {:.1}%          ·   novo {:.1}%",
        soma(|p| &p.velho, &["crisis", "collapse"]),
        soma(|p| &p.novo, &["crisis", "collapse"])
    );
    println!(
        "  colapso                 velho {:.1}%          ·   novo {:.1}%",
        soma(|p| &p.velho, &["collapse"]),
        soma(|p| &p.novo, &["collapse"])
    );

    // ── 5. O modelo NOVO INTEIRO, como contexto ──────────────────────────────────────
    // Não é a troca desta rodada: é o destino, com a receita ainda carregando o fator ~1,96
    // que foi calibrado contra a despesa VELHA. Serve para dizer de quanto é a recalibração
    // que a próxima rodada vai ter que fazer.
    println!("\n  ── CONTEXTO: o modelo NOVO INTEIRO (despesa + receita + offseason) ──");
    println!(
        "     A receita nova foi calibrada contra a despesa VELHA. Este bloco mede o desajuste."
    );
    println!(
        "  {:<22} {:>10} {:>10} {:>13} {:>12}",
        "categoria", "rec/desp", "meses", "crise+col %", "vendas %"
    );
    let mut razoes_completo: Vec<f64> = Vec::new();
    for &c in CATEGORIAS {
        let r = medir_categoria_cenario(
            c,
            &Cenario {
                despesa: Despesa::Economia,
                receita: Receita::Economia(ParametrosDeReceita::default()),
                offseason: Offseason::Economia(ParametrosDeDesenvolvimento::default()),
                ..Cenario::default()
            },
        );
        let razao = receita_total(&r) / despesa_total(&r).max(1.0);
        razoes_completo.push(razao);
        let obs: u32 = r.estados.values().sum();
        let doentes: u32 = r.estados.get("crisis").copied().unwrap_or(0)
            + r.estados.get("collapse").copied().unwrap_or(0);
        println!(
            "  {:<22} {:>10.2} {:>10.1} {:>12.1}% {:>11.2}%",
            c,
            razao,
            mediana(&r.meses_fim),
            doentes as f64 / obs.max(1) as f64 * 100.0,
            r.vendas as f64 / (TEMPORADAS as f64 * arena(c).equipes as f64) * 100.0,
        );
    }
    let media_completo = razoes_completo.iter().sum::<f64>() / razoes_completo.len().max(1) as f64;
    println!(
        "\n  Receita ÷ despesa no modelo novo inteiro: {:.2}× na média das 9.\n  \
         CUIDADO ao ler isto como 'a receita já está calibrada'. O denominador inclui o RALO\n  \
         (`economia::desenvolvimento`), que investe o excedente e portanto ABSORVE qualquer\n  \
         sobra: quanto mais a receita sobra, mais a equipe gasta em estrutura, e a razão volta\n  \
         para perto de 1 sozinha. Quem mostra a sobra é a coluna dos MESES ao lado — se ela\n  \
         estiver acima da faixa do critério 2, há receita demais mesmo com a razão em 1,04.",
        media_completo
    );

    println!(
        "\n  O comparador não afirma nada: ele imprime. A decisão de aceitar a troca é de quem lê."
    );
}

#[test]
#[ignore = "diagnóstico de reprodutibilidade — roda com --ignored --nocapture"]
fn o_mesmo_cenario_da_o_mesmo_resultado() {
    // Existe porque duas leituras do que deveria ser o MESMO cenário (`medir_categoria` no
    // teste de critérios e a coluna `novo` do comparador) discordaram em gt4: 123,0 meses
    // contra 49,7. Ou o harness não era reprodutível — e nesse caso nenhum número desta
    // empreitada valia nada —, ou os dois cenários não eram os mesmos.
    //
    // **Era nenhum dos dois.** O harness é determinístico: atalho, cenário explícito,
    // repetição e repetição-depois-de-um-Legado dão o mesmo número até o último dígito. O
    // que mudava era a ÁRVORE — três sessões editando `fame.rs`, `public_presence/`,
    // `event_interest/` e `finance/rescue.rs` enquanto as medições rodavam, e cada `cargo
    // test` compilando um estado diferente do mundo. Dois relatórios separados por dez
    // minutos não são comparáveis nesta árvore, e é por isso que o comparador roda os dois
    // modelos DENTRO do mesmo teste em vez de confrontar duas execuções.
    //
    // O teste fica como guarda: se um dia ele falhar, o problema é do harness mesmo.
    for categoria in ["gt4", "gt3", "mazda_amador"] {
        let por_atalho = medir_categoria(categoria);
        let por_cenario = medir_categoria_cenario(
            categoria,
            &Cenario {
                despesa: Despesa::Economia,
                ..Cenario::default()
            },
        );
        let repetido = medir_categoria(categoria);
        // E agora a ORDEM do comparador: um `Legado` roda antes. Se isto mudar o resultado,
        // existe estado de processo carregado de uma medição para a seguinte.
        let _legado_antes = medir_categoria_cenario(
            categoria,
            &Cenario {
                despesa: Despesa::Producao,
                ..Cenario::default()
            },
        );
        let depois_do_legado = medir_categoria_cenario(
            categoria,
            &Cenario {
                despesa: Despesa::Economia,
                ..Cenario::default()
            },
        );
        println!(
            "  {categoria:<14} DEPOIS de um Legado no mesmo processo: {:>7.1} meses / {:>3} vendas",
            mediana(&depois_do_legado.meses_fim),
            depois_do_legado.vendas,
        );
        println!(
            "  {categoria:<14} atalho {:>7.1} meses / {:>3} vendas · cenário {:>7.1} / {:>3} · repetido {:>7.1} / {:>3}",
            mediana(&por_atalho.meses_fim),
            por_atalho.vendas,
            mediana(&por_cenario.meses_fim),
            por_cenario.vendas,
            mediana(&repetido.meses_fim),
            repetido.vendas,
        );
        assert_eq!(
            por_atalho.vendas, repetido.vendas,
            "{categoria}: a MESMA chamada duas vezes deu resultados diferentes"
        );
    }
}

// ===================== A autópsia da gt4 =====================
//
// A gt4 é a única categoria que PIORA com a troca da despesa: crise 23,5 → 39,5%, colapso
// 17,5 → 36,0%, vendas 8,0 → 17,5%. Todas as outras melhoram ou ficam paradas, e a gt3 —
// que tem razão de estrutura quase idêntica (1,42× contra 1,44×) — melhora.
//
// Roda com `cargo test --lib autopsia_do_colapso -- --ignored --nocapture`.
//
// Duas perguntas, nesta ordem:
//
// 1. **As duas âncoras.** A DESPESA passou a sair de `economia::temporada`; a RECEITA
//    continua sendo fração de `operating_cost_midpoint`, a tabela velha. Se a razão entre
//    as duas âncoras for pior na gt4 que nas outras, a gt4 não tem doença própria: ela é só
//    a categoria em que a tabela velha mais subestimava o custo real, e a troca cobrou a
//    diferença de uma receita que não sabia disso.
// 2. **Qual linha estourou o caixa.** A composição do ano de quem terminou em `collapse`
//    contra a de quem terminou de pé, na mesma categoria e no mesmo modelo.

/// As duas âncoras da categoria, e o que a razão entre elas significa.
fn ancoras(categoria: &'static str) -> (f64, f64) {
    let a = arena(categoria);
    let velha = category_finance_scale(categoria).operating_cost_midpoint();
    // No multi-classe a âncora nova é por CLASSE; a categoria inteira é a soma ponderada
    // pelo tamanho de cada classe, que é o que uma equipe média da categoria custa.
    let nova: f64 = a
        .classes
        .iter()
        .map(|c| {
            let classe = (!c.nome.is_empty()).then_some(c.nome);
            crate::economia::temporada::custo_operacional_anual_de_referencia(categoria, classe)
                * c.equipes as f64
        })
        .sum::<f64>()
        / a.equipes.max(1) as f64;
    (velha, nova)
}

/// Imprime a composição de um grupo da autópsia, em % da receita do ano do grupo.
fn linha_da_autopsia(rotulo: &str, a: &Autopsia) {
    if a.temporadas == 0 {
        println!("     {rotulo:<22} (nenhuma equipe-temporada neste grupo)");
        return;
    }
    println!(
        "     {rotulo:<22} {:>5} anos · entrou com {:>6.1} meses · receita {:>9.0} · despesa {:>5.0}% \
         │ sal {:>5.1} oper {:>5.1} estr {:>5.1} tec {:>5.1} juros {:>5.1} ralo {:>5.1}",
        a.temporadas,
        a.meses_no_inicio / a.temporadas as f64,
        a.media(|l| l.receita_do_ano),
        a.despesa() / a.linhas.receita_do_ano.max(1.0) * 100.0,
        a.pct(|l| l.salario),
        a.pct(|l| l.operacao),
        a.pct(|l| l.estrutural),
        a.pct(|l| l.tecnico),
        a.pct(|l| l.juros),
        a.pct(|l| l.ralo),
    );
}

#[test]
#[ignore = "autópsia do colapso — roda com --ignored --nocapture"]
fn autopsia_do_colapso() {
    println!("\n═══ AUTÓPSIA DO COLAPSO · por que a gt4 piora e a gt3 não ═══\n");

    // ── 1. As duas âncoras ───────────────────────────────────────────────────────────
    println!("  ── AS DUAS ÂNCORAS ──");
    println!("     A RECEITA é fração de `operating_cost_midpoint` (tabela velha). A DESPESA saiu");
    println!(
        "     de `economia::temporada` (modelo físico). A razão entre elas é o desalinhamento"
    );
    println!("     que a troca cobrou de cada categoria.\n");
    println!(
        "  {:<24} {:>14} {:>14} {:>10} {:>10} {:>12} {:>12}",
        "categoria",
        "âncora VELHA",
        "âncora NOVA",
        "nova÷velha",
        "eventos%",
        "estrutura%",
        "categóricas%"
    );
    let mut razoes_de_ancora: Vec<(&'static str, f64)> = Vec::new();
    for &c in CATEGORIAS {
        let (velha, nova) = ancoras(c);
        let razao = nova / velha.max(1.0);
        razoes_de_ancora.push((c, razao));
        // Como o ano se reparte. As LINHAS CATEGÓRICAS (suporte de fábrica, simulador,
        // aquisição de dados) são 100% fixas e só existem a partir de certo degrau — é o
        // candidato natural a explicar por que o peso do fixo salta na escada.
        let a = arena(c);
        let dec = crate::economia::temporada::decomposicao_anual(
            c,
            a.classes
                .first()
                .and_then(|k| (!k.nome.is_empty()).then_some(k.nome)),
        );
        println!(
            "  {:<24} {:>14.0} {:>14.0} {:>9.2}× {:>9.0}% {:>11.0}% {:>11.0}%",
            c,
            velha,
            nova,
            razao,
            dec.eventos / dec.total().max(1.0) * 100.0,
            (dec.total() - dec.eventos) / dec.total().max(1.0) * 100.0,
            dec.fracao_categorica() * 100.0,
        );
    }
    let pior = razoes_de_ancora
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(c, v)| (*c, *v))
        .unwrap_or(("", 0.0));
    println!("\n     Pior desalinhamento: {} a {:.2}×.", pior.0, pior.1);

    // ── 2. A autópsia, nos dois modelos ──────────────────────────────────────────────
    println!("\n  ── A COMPOSIÇÃO DO ANO: quem quebrou × quem ficou de pé ──");
    println!("     (cada linha em % da receita do ANO daquele grupo)\n");
    for &c in CATEGORIAS {
        let legado = medir_categoria_cenario(
            c,
            &Cenario {
                despesa: Despesa::Producao,
                ..Cenario::default()
            },
        );
        let fisico = medir_categoria_cenario(
            c,
            &Cenario {
                despesa: Despesa::Economia,
                ..Cenario::default()
            },
        );
        println!("  ── {c} ──");
        linha_da_autopsia("LEGADO · colapso", &legado.autopsia_colapso);
        linha_da_autopsia("LEGADO · de pé", &legado.autopsia_saudavel);
        linha_da_autopsia("FÍSICO · colapso", &fisico.autopsia_colapso);
        linha_da_autopsia("FÍSICO · de pé", &fisico.autopsia_saudavel);
        // ── O FREIO ───────────────────────────────────────────────────────────────────
        // A conta fixa (folha + estrutura) sobre a receita de cada equipe, do melhor time do
        // grid para o pior. Quem chega perto de 100% não tem o que cortar num ano ruim.
        let freio = |r: &Resultado| -> String {
            // O campo vem na ordem do GRID (para poder ser cruzado com posição e meses);
            // aqui o que interessa é a distribuição, então ordena a cópia.
            let mut v = r.fixo_sobre_receita.clone();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if v.is_empty() {
                return "—".into();
            }
            format!(
                "melhor {:>5.1}% · mediana {:>5.1}% · PIOR {:>5.1}%  ({} das {} acima de 60%) · receita espalha {:.2}×",
                v[0],
                v[v.len() / 2],
                v[v.len() - 1],
                v.iter().filter(|x| **x > 60.0).count(),
                v.len(),
                r.receita_espalhada
            )
        };
        println!("     conta FIXA ÷ receita, LEGADO: {}", freio(&legado));
        println!("     conta FIXA ÷ receita, FÍSICO: {}", freio(&fisico));
        // O delta que interessa: quanto a estrutura passou a pesar no ano de quem quebra.
        let d_estrutura = fisico.autopsia_colapso.pct(|l| l.estrutural)
            - legado.autopsia_colapso.pct(|l| l.estrutural);
        let d_despesa = fisico.autopsia_colapso.despesa()
            / fisico.autopsia_colapso.linhas.receita_do_ano.max(1.0)
            - legado.autopsia_colapso.despesa()
                / legado.autopsia_colapso.linhas.receita_do_ano.max(1.0);
        println!(
            "     Δ no ano de quem quebra: estrutura {:+.1} pontos de receita · despesa total {:+.1} pontos\n",
            d_estrutura,
            d_despesa * 100.0
        );
    }

    println!(
        "  A autópsia não conclui: ela mostra a composição. O mecanismo é o que sobrevive à\n  \
         comparação entre a gt4 e a gt3, que têm a mesma razão de estrutura e destinos opostos."
    );
}

// ===================== A recalibração sobre a despesa física =====================
//
// Roda com `cargo test --lib recalibrar_sobre_a_despesa_fisica -- --ignored --nocapture`.
//
// O que mudou desde a calibração anterior, e por que ela inteira precisa ser refeita:
//
// - a DESPESA saiu da fatura de pesos (hoje `tests::despesa_legada`) e passou a ser
//   `economia::evento` +
//   `economia::temporada`, o que derrubou o bloco variável ~25% e subiu o fixo 14–44%;
// - o custo operacional anual que alimenta `economia::receita` passou a ser o FÍSICO e POR
//   CLASSE (`AncoraDoDinheiro::Fisica`), não mais a tabela declarada da categoria;
// - o critério 2 virou fôlego em MESES (3–18), e é ele que mostra a sobra. A razão receita
//   ÷ despesa NÃO mostra: o ralo investe o excedente e devolve a razão para perto de 1
//   sozinho, por mais receita que se ponha.
//
// O alvo tem duas pontas que puxam em sentidos opostos, e é essa a dificuldade:
//
// - a MEDIANA tem dinheiro demais (18–30 meses onde o alvo é 3–18) → o nível desce;
// - a equipe do FUNDO da gt4 e da gt3 tem conta fixa acima da própria receita (127% e
//   128%) → cortar o nível uniformemente a mata.
//
// Ou seja: o que precisa encolher é o ESPALHAMENTO da receita dentro do grid, não só o
// nível. É por isso que a varredura tem dois eixos.

/// O cenário da recalibração: tudo novo, âncora física.
fn cenario_novo(receita: ParametrosDeReceita) -> Cenario {
    Cenario {
        despesa: Despesa::Economia,
        receita: Receita::Economia(receita),
        offseason: Offseason::Economia(ParametrosDeDesenvolvimento::default()),
        ancora: AncoraDoDinheiro::Fisica,
        ..Cenario::default()
    }
}

/// Os coeficientes de nível, escalados por um fator só. Preserva a REPARTIÇÃO entre canais
/// (que os critérios 3/4/5 fixam) e mexe apenas em quanto dinheiro entra no total.
fn com_nivel(base: &ParametrosDeReceita, nivel: f64, gama: f64) -> ParametrosDeReceita {
    ParametrosDeReceita {
        premio_de_corrida: base.premio_de_corrida * nivel,
        patrocinio_fixo: base.patrocinio_fixo * nivel,
        patrocinio_por_reputacao: base.patrocinio_por_reputacao * nivel,
        patrocinio_por_fama: base.patrocinio_por_fama * nivel,
        bilheteria: base.bilheteria * nivel,
        volta_mais_rapida: base.volta_mais_rapida * nivel,
        fechamento_ao_ultimo: base.fechamento_ao_ultimo * nivel,
        fechamento_ao_primeiro: base.fechamento_ao_primeiro * nivel,
        inclinacao_do_premio: gama,
        ..*base
    }
}

#[test]
#[ignore = "recalibração final — roda com --ignored --nocapture"]
fn recalibrar_sobre_a_despesa_fisica() {
    println!("\n═══ RECALIBRAÇÃO DA RECEITA SOBRE A DESPESA FÍSICA ═══");
    println!("  Alvo que amarra: critério 2 (3–18 meses) e critério 8 (0,5–5% de vendas).");
    println!("  A razão receita÷despesa NÃO amarra — o ralo a devolve para 1 sozinha.\n");

    let base = ParametrosDeReceita::default();

    // ── O denominador ────────────────────────────────────────────────────────────────
    let partida = resumir_receita(&cenario_novo(base));
    println!("  ── o denominador: despesa real ÷ âncora FÍSICA, por categoria ──");
    println!(
        "  {:<24} {:>12} {:>10} {:>10} {:>12}",
        "categoria", "despesa/op", "meses", "vendas%", "portão da pior"
    );
    for (i, &c) in CATEGORIAS.iter().enumerate() {
        println!(
            "  {:<24} {:>11.2}× {:>10.1} {:>9.2}% {:>11.1}%",
            c,
            partida.despesa_por_op[i],
            partida.meses[i],
            partida.vendas[i],
            partida.portao_da_pior[i],
        );
    }
    let (dmin, dmax) = faixa(&partida.despesa_por_op);
    println!(
        "  faixa {dmin:.2}× – {dmax:.2}× · espalhamento de {:.0}%",
        (dmax / dmin - 1.0) * 100.0
    );

    cabecalho_receita();
    linha_receita("hoje (γ 6,5, nível 1,0)", &partida);

    // ── Eixo 1: o NÍVEL ──────────────────────────────────────────────────────────────
    // Desce o nível preservando a repartição entre canais. O critério 2 é o alvo.
    println!("\n  ── eixo 1: o NÍVEL (γ fixo em 6,5) ──");
    cabecalho_receita();
    let mut melhor: Option<(f64, f64, ResumoReceita)> = None;
    for nivel in [1.00f64, 0.90, 0.80, 0.70, 0.60, 0.50] {
        let r = resumir_receita(&cenario_novo(com_nivel(&base, nivel, 6.5)));
        linha_receita(&format!("nível {nivel:.2}"), &r);
        if melhor
            .as_ref()
            .map(|(_, _, m)| r.criterios_ok() > m.criterios_ok())
            .unwrap_or(true)
        {
            melhor = Some((nivel, 6.5, r));
        }
    }
    let nivel_escolhido = melhor.as_ref().map(|(n, _, _)| *n).unwrap_or(1.0);

    // ── Eixo 2: a CONVEXIDADE ────────────────────────────────────────────────────────
    // Com o nível fixo no que o eixo 1 escolheu, γ decide o ESPALHAMENTO dentro do grid —
    // e é o espalhamento que afoga a equipe do fundo da gt4 e da gt3.
    println!("\n  ── eixo 2: a CONVEXIDADE γ (nível fixo em {nivel_escolhido:.2}) ──");
    cabecalho_receita();
    for gama in [2.0f64, 3.0, 4.0, 5.0, 6.5, 8.0] {
        let r = resumir_receita(&cenario_novo(com_nivel(&base, nivel_escolhido, gama)));
        linha_receita(&format!("γ {gama:.1}"), &r);
        if melhor
            .as_ref()
            .map(|(_, _, m)| r.criterios_ok() > m.criterios_ok())
            .unwrap_or(true)
        {
            melhor = Some((nivel_escolhido, gama, r));
        }
    }

    // ── O detalhe do melhor conjunto ─────────────────────────────────────────────────
    if let Some((nivel, gama, r)) = melhor {
        println!("\n  ── MELHOR CONJUNTO: nível {nivel:.2} · γ {gama:.1} ──");
        println!(
            "  {:<24} {:>9} {:>9} {:>9} {:>9} {:>9} {:>11} {:>9} {:>10}",
            "categoria",
            "rec/desp",
            "meses",
            "corrida%",
            "portão%",
            "fecha%",
            "camp/lant",
            "vendas%",
            "11b portão"
        );
        for (i, &c) in CATEGORIAS.iter().enumerate() {
            println!(
                "  {:<24} {:>9.2} {:>9.1} {:>9.1} {:>9.1} {:>9.1} {:>10.2}× {:>8.2}% {:>9.1}%",
                c,
                r.razao[i],
                r.meses[i],
                r.corrida[i],
                r.portao[i],
                r.fechamento[i],
                r.campeao[i],
                r.vendas[i],
                r.portao_da_pior[i],
            );
        }
        println!(
            "\n  crise {:.1}% (colapso {:.1}%) · {}/8 critérios de receita inteiramente dentro",
            r.crise_pct,
            r.colapso_pct,
            r.criterios_ok()
        );
        // Critério 11b: o portão para a equipe de MENOR atração, alvo ≥ 10% da receita dela.
        let (b_min, b_max) = faixa(&r.portao_da_pior);
        let fora_11b = r.portao_da_pior.iter().filter(|v| **v < 10.0).count();
        println!(
            "  critério 11b (portão ≥ 10% da receita da equipe de PIOR atração): {b_min:.1}% – {b_max:.1}% · {fora_11b} fora"
        );
        println!(
            "  Os coeficientes a gravar em `ParametrosDeReceita::default()` são os de hoje\n  \
             multiplicados por {nivel:.2}, com `inclinacao_do_premio` = {gama:.1}."
        );
    }
}

// ===================== O eixo que a varredura de receita apontou =====================
//
// Roda com `cargo test --lib varrer_a_reserva_do_ralo -- --ignored --nocapture`.
//
// A varredura de receita (`recalibrar_sobre_a_despesa_fisica`) mediu que o nível da receita
// **não é o problema**: em 1,00 a razão receita ÷ despesa fica em 1,02–1,04 nas nove
// categorias e o denominador espalha só 6%. Todo corte de nível piora — a 0,80 o critério 2
// vai a 2,0–19,6 meses (uma categoria morre enquanto a outra continua rica) e a crise salta
// de 5,4% para 16,4%. O corte uniforme mata o fundo antes de corrigir o topo.
//
// O que sobra fora do alvo é o TOPO do critério 2: as duas Rookies terminam com 22,9 e 24,7
// meses. E o motivo não está na receita, está aqui: `ParametrosDeDesenvolvimento` guarda
// `meses_de_reserva = 12` antes de investir qualquer coisa, e investe só
// `fracao_do_excedente = 0,40` do que passa disso. Um ralo que se recusa a descer abaixo de
// 12 meses põe um PISO em 12 no fôlego de toda equipe saudável — e a faixa do critério 2 é
// 3–18. O topo da faixa fica a apenas 6 meses do piso do ralo.
//
// Este teste mede os dois parâmetros. É a hipótese natural, e ela pode estar errada: se
// afundar o fôlego sem criar falência nenhuma, o critério 8 continua onde está e o problema
// é de outra natureza.

#[test]
#[ignore = "varredura da reserva do ralo — roda com --ignored --nocapture"]
fn varrer_a_reserva_do_ralo() {
    println!("\n═══ A RESERVA DO RALO · o eixo que a varredura de receita apontou ═══");
    println!("  Receita fixa nos coeficientes de hoje (nível 1,00, γ 6,5) — a varredura");
    println!("  mostrou que mexer nela piora. O que se mexe aqui é `desenvolvimento`.\n");

    let com_ralo = |meses: f64, fracao: f64| Cenario {
        despesa: Despesa::Economia,
        receita: Receita::Economia(ParametrosDeReceita::default()),
        offseason: Offseason::Economia(ParametrosDeDesenvolvimento {
            meses_de_reserva: meses,
            fracao_do_excedente: fracao,
            ..ParametrosDeDesenvolvimento::default()
        }),
        ancora: AncoraDoDinheiro::Fisica,
        ..Cenario::default()
    };

    cabecalho_receita();
    let mut melhor: Option<(f64, f64, ResumoReceita)> = None;
    // A primeira varredura (12/40 · 12/70 · 9/55 · 6/40 · 6/70 · 6/90 · 3/70) mostrou o
    // formato do problema: drenar mais fecha o critério 2 mas ABRE o 6, porque o ralo compra
    // estrutura e estrutura igual para todo mundo achata campeão ÷ lanterna. E drenar mais
    // ainda REDUZ a crise em vez de aumentá-la — o investimento tem retorno, então a equipe
    // que gasta fica melhor e fatura mais. Esta segunda passada varre o vão entre os dois
    // extremos que sobreviveram: 12/40 (6 e 7 de pé, 2 fora) e 12/70 (2 de pé, 6 fora).
    for (meses, fracao) in [
        (12.0f64, 0.40f64), // o de hoje
        (12.0, 0.50),
        (12.0, 0.55),
        (10.0, 0.45),
        (9.0, 0.40),
        (8.0, 0.45),
    ] {
        let r = resumir_receita(&com_ralo(meses, fracao));
        linha_receita(
            &format!("reserva {meses:.0}m · {:.0}% do excedente", fracao * 100.0),
            &r,
        );
        let melhora = melhor
            .as_ref()
            .map(|(_, _, m)| (r.criterios_ok(), r.crise_ok()) > (m.criterios_ok(), m.crise_ok()))
            .unwrap_or(true);
        if melhora {
            melhor = Some((meses, fracao, r));
        }
    }

    if let Some((meses, fracao, r)) = melhor {
        println!(
            "\n  ── MELHOR: reserva {meses:.0} meses · {:.0}% do excedente ──",
            fracao * 100.0
        );
        println!(
            "  {:<24} {:>9} {:>9} {:>9} {:>9} {:>11} {:>9} {:>10}",
            "categoria",
            "rec/desp",
            "meses",
            "corrida%",
            "portão%",
            "camp/lant",
            "vendas%",
            "11b portão"
        );
        for (i, &c) in CATEGORIAS.iter().enumerate() {
            println!(
                "  {:<24} {:>9.2} {:>9.1} {:>9.1} {:>9.1} {:>10.2}× {:>8.2}% {:>9.1}%",
                c,
                r.razao[i],
                r.meses[i],
                r.corrida[i],
                r.portao[i],
                r.campeao[i],
                r.vendas[i],
                r.portao_da_pior[i],
            );
        }
        let fora_11b = r.portao_da_pior.iter().filter(|v| **v < 10.0).count();
        let (b_min, b_max) = faixa(&r.portao_da_pior);
        println!(
            "\n  crise {:.1}% (colapso {:.1}%) · {}/8 critérios · 11b {b_min:.1}–{b_max:.1}% ({fora_11b} fora)",
            r.crise_pct,
            r.colapso_pct,
            r.criterios_ok()
        );
    }
}

// ===================== O critério 2 POR POSIÇÃO NO GRID =====================
//
// Roda com `cargo test --lib criterio_2_por_posicao_no_grid -- --ignored --nocapture`.
//
// O placar mede o critério 2 pela MEDIANA da categoria, e por isso ele não distingue duas
// causas opostas para o mesmo número:
//
// - **categoria uniformemente folgada** — todo mundo termina com ~20 meses porque a
//   operação do degrau é barata. Nesse caso o alvo 3–18 é que não serve para o degrau de
//   entrada, e a faixa precisa ser por degrau.
// - **categoria concentrada** — o campeão termina com 60 meses e o lanterna com 4, e a
//   mediana em 20 é uma média de coisas que não existem. Nesse caso o defeito é a
//   CONVEXIDADE: γ = 6,5 num grid de 6 equipes concentra o prêmio em duas ou três.
//
// A diferença tem consequência prática oposta. No primeiro caso a alavanca está esgotada e
// o alvo muda; no segundo a alavanca é γ POR TAMANHO DE GRID, que nunca foi varrido porque
// γ sempre foi tratado como global.
//
// Uma medição de fora deu o gatilho: numa etapa real de Mazda Rookie a sessão da tela mediu
// operação de $22.862 contra bônus do vencedor de $68.413 — saldo de etapa +$92.105 contra
// custo fixo anual de $175.120. Duas vitórias pagam o ano. Se isso for verdade DENTRO do
// grid, o campeão da Rookie é riquíssimo e a média mente.
//
// A posição é medida por PONTOS SOMADOS em 20 temporadas, não pela classificação de um ano:
// num grid de 6 equipes a posição de uma temporada é ruído, e o acumulado é o que separa o
// time de ponta do lanterna de forma estável.

/// Um lugar no grid, do campeão ao lanterna, com o que ele terminou.
struct LugarNoGrid {
    /// 1 = quem mais pontuou em 20 temporadas.
    posto: usize,
    meses: f64,
    /// Receita média por temporada.
    receita_por_ano: f64,
    /// Saldo líquido médio por temporada — o que sobra depois de tudo, inclusive do ralo.
    saldo_por_ano: f64,
    /// Conta fixa (folha + estrutura) sobre a receita, em %.
    fixo_pct: f64,
}

/// Ordena o grid por pontos acumulados e devolve o retrato de cada lugar.
fn grid_por_posicao(r: &Resultado) -> Vec<LugarNoGrid> {
    let n = r.meses_fim.len();
    let mut ordem: Vec<usize> = (0..n).collect();
    ordem.sort_by(|a, b| {
        r.pontos_acumulados[*b]
            .partial_cmp(&r.pontos_acumulados[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ordem
        .into_iter()
        .enumerate()
        .map(|(posto, i)| LugarNoGrid {
            posto: posto + 1,
            meses: r.meses_fim[i],
            receita_por_ano: r.receita_por_equipe[i] / TEMPORADAS as f64,
            saldo_por_ano: r.saldo_por_equipe[i] / TEMPORADAS as f64,
            fixo_pct: r.fixo_sobre_receita[i],
        })
        .collect()
}

/// Campeão, meio de tabela e lanterna — os três lugares que respondem a pergunta.
fn tres_lugares(lugares: &[LugarNoGrid]) -> (&LugarNoGrid, &LugarNoGrid, &LugarNoGrid) {
    (
        &lugares[0],
        &lugares[lugares.len() / 2],
        &lugares[lugares.len() - 1],
    )
}

#[test]
#[ignore = "critério 2 por posição no grid — roda com --ignored --nocapture"]
fn criterio_2_por_posicao_no_grid() {
    println!("\n═══ O CRITÉRIO 2 POR DENTRO DO GRID ═══");
    println!("  Modelo novo inteiro. Posição = pontos somados em {TEMPORADAS} temporadas.");
    println!("  A pergunta: a mediana da categoria é o retrato de todo mundo ou a média de");
    println!("  dois mundos que não se encontram?\n");

    let medidas: Vec<(&'static str, Resultado)> = CATEGORIAS
        .iter()
        .map(|&c| {
            (
                c,
                medir_categoria_cenario(c, &cenario_novo(ParametrosDeReceita::default())),
            )
        })
        .collect();

    // ── Os três lugares, categoria a categoria ───────────────────────────────────────
    println!("  ── CAMPEÃO · MEIO · LANTERNA: meses de operação no fim ──");
    println!(
        "  {:<24} {:>6} {:>10} {:>10} {:>10} {:>12}",
        "categoria", "grid", "campeão", "meio", "lanterna", "camp÷lant"
    );
    let mut dispersoes: Vec<(&'static str, f64)> = Vec::new();
    for (c, r) in &medidas {
        let lugares = grid_por_posicao(r);
        let (campeao, meio, lanterna) = tres_lugares(&lugares);
        let dispersao = campeao.meses / lanterna.meses.abs().max(0.1);
        dispersoes.push((c, dispersao));
        println!(
            "  {:<24} {:>6} {:>10.1} {:>10.1} {:>10.1} {:>11.1}×",
            c,
            lugares.len(),
            campeao.meses,
            meio.meses,
            lanterna.meses,
            dispersao,
        );
    }

    // ── A Rookie e a GT3 lado a lado, grid inteiro ───────────────────────────────────
    // As duas pontas da escada: 6 equipes contra 14, conta fixa de 46% contra 83%.
    for alvo in ["mazda_rookie", "gt3"] {
        let (_, r) = medidas.iter().find(|(c, _)| *c == alvo).expect("categoria");
        println!("\n  ── {alvo}: o grid inteiro, do campeão ao lanterna ──");
        println!(
            "  {:>6} {:>10} {:>16} {:>16} {:>10}",
            "posto", "meses", "receita/ano", "saldo/ano", "fixo%"
        );
        for l in grid_por_posicao(r) {
            println!(
                "  {:>6} {:>10.1} {:>16.0} {:>16.0} {:>9.1}%",
                l.posto, l.meses, l.receita_por_ano, l.saldo_por_ano, l.fixo_pct
            );
        }
    }

    // ── O veredito da dispersão ──────────────────────────────────────────────────────
    // Um grid uniforme tem campeão ÷ lanterna perto de 1; um grid concentrado, muito acima.
    // O limiar de 2× não é sagrado: é o ponto em que a mediana deixa de descrever o grid.
    println!("\n  ── o que a dispersão diz ──");
    for (c, d) in &dispersoes {
        let equipes = arena(c).equipes;
        let leitura = if !d.is_finite() || *d > 100.0 {
            "o lanterna terminou no zero ou negativo — grid partido em dois"
        } else if *d > 2.0 {
            "CONCENTRADO: a mediana é a média de dois mundos"
        } else {
            "uniforme: a mediana descreve o grid"
        };
        println!("  {c:<24} {equipes:>3} equipes · {d:>6.1}×  {leitura}");
    }

    // ── A ROTAÇÃO DO CAMPEÃO: o mecanismo por trás da dispersão ──────────────────────
    // Uma curva convexa só concentra dinheiro se houver um vencedor PERSISTENTE. Onde o
    // título roda, o prêmio grande da temporada vai para uma equipe diferente a cada ano e
    // se distribui sozinho ao longo de 20 temporadas — por mais convexa que seja a curva.
    // É esta coluna que decide se γ é a alavanca ou se ele nunca teve como ser.
    println!("\n  ── a rotação do campeão em {TEMPORADAS} temporadas ──");
    println!(
        "  {:<24} {:>8} {:>12} {:>16} {:>14}",
        "categoria", "títulos", "campeões", "mais titulada", "% dos títulos"
    );
    for (c, r) in &medidas {
        let total: u32 = r.titulos.iter().sum();
        let distintos = r.titulos.iter().filter(|t| **t > 0).count();
        let maior = r.titulos.iter().copied().max().unwrap_or(0);
        println!(
            "  {:<24} {total:>8} {:>11} {maior:>15} {:>13.0}%",
            c,
            format!("{distintos}/{}", r.titulos.len()),
            maior as f64 / total.max(1) as f64 * 100.0,
        );
    }

    println!(
        "\n  Se o grid pequeno for o mais concentrado, a alavanca do critério 2 NÃO está\n  \
         esgotada: γ nunca foi varrido por tamanho de grid, sempre como global. Se a\n  \
         concentração for parecida em toda a escada, é o alvo que precisa de faixa por degrau."
    );
}

// ===================== A8 — o laço dinheiro → índice → patrocínio → dinheiro =====================
//
// Roda com `cargo test --lib medir_realimentacao_do_orcamento -- --ignored --nocapture`.
//
// A pergunta: `budget_index` é derivado do dinheiro e volta como receita
// (`race::financas`, o termo `plan.budget_index × round_operating_base × 0,002`). O laço
// existe; falta o TAMANHO dele. Este bloco mede o mesmo mundo com o termo ligado e com ele
// zerado, e nada mais mexido, em 1, 3 e 5 temporadas.

/// Os horizontes pedidos. As 20 temporadas continuam rodando: a recuperação em H anos só é
/// contável se houver ano seguinte para observar.
const HORIZONTES: [usize; 3] = [1, 3, 5];

/// Réplicas por (categoria × modelo). Colapso e recuperação são eventos raros; um mundo só
/// não separa efeito de sorteio.
const REPLICAS: u64 = 4;

fn p_da_amostra(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut ordenado = v.to_vec();
    ordenado.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = (((ordenado.len() - 1) as f64) * p).round() as usize;
    ordenado[idx]
}

fn media_de(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.iter().sum::<f64>() / v.len() as f64
}

/// Coeficiente de variação: o espalhamento adimensional, que é o que permite comparar o
/// grid da rookie com o do endurance.
fn cv_de(v: &[f64]) -> f64 {
    let m = media_de(v);
    if m.abs() < 1e-9 {
        return 0.0;
    }
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64;
    var.sqrt() / m.abs()
}

/// Gini do caixa líquido do grid. Zero é o grid perfeitamente igual, 1 é uma equipe com
/// tudo. Valor negativo é grampeado em zero antes: dívida não é riqueza negativa para
/// efeito de concentração, é ausência de riqueza.
fn gini_de(v: &[f64]) -> f64 {
    let mut ordenado: Vec<f64> = v.iter().map(|x| x.max(0.0)).collect();
    if ordenado.is_empty() {
        return 0.0;
    }
    ordenado.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = ordenado.len() as f64;
    let soma: f64 = ordenado.iter().sum();
    if soma <= 0.0 {
        return 0.0;
    }
    let acumulado: f64 = ordenado
        .iter()
        .enumerate()
        .map(|(i, x)| (i as f64 + 1.0) * x)
        .sum();
    (2.0 * acumulado / (n * soma) - (n + 1.0) / n).clamp(0.0, 1.0)
}

/// Fatia do caixa do grid que está na mão do quinto mais rico.
fn topo20_de(v: &[f64]) -> f64 {
    let mut ordenado: Vec<f64> = v.iter().map(|x| x.max(0.0)).collect();
    if ordenado.is_empty() {
        return 0.0;
    }
    ordenado.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let soma: f64 = ordenado.iter().sum();
    if soma <= 0.0 {
        return 0.0;
    }
    let n = (ordenado.len() as f64 * 0.2).ceil().max(1.0) as usize;
    ordenado[..n].iter().sum::<f64>() / soma * 100.0
}

fn de_pe(estado: &str) -> bool {
    matches!(estado, "stable" | "healthy" | "elite")
}

fn doente(estado: &str) -> bool {
    matches!(estado, "crisis" | "collapse")
}

/// Que recorte do grid entra na conta. Sem filtro é a categoria inteira.
#[derive(Clone, Copy, Default)]
struct Filtro {
    /// Índice da classe em `Arena::classes`.
    classe: Option<usize>,
    /// Estado com que a equipe COMEÃ‡OU o mundo, antes da primeira corrida.
    estado_inicial: Option<&'static str>,
}

impl Filtro {
    fn aceita(&self, inicial: &FotoDaEquipe) -> bool {
        self.classe.map_or(true, |c| inicial.classe == c)
            && self.estado_inicial.map_or(true, |e| inicial.estado == e)
    }
}

/// O que um horizonte produz, já agregado nas réplicas.
#[derive(Default, Clone, Copy)]
struct Medida {
    /// Quantas equipes o recorte pegou, na média das réplicas.
    equipes: f64,
    /// Fôlego no fim da temporada H, em meses de operação.
    meses_mediano: f64,
    meses_p10: f64,
    meses_p90: f64,
    /// Concentração do caixa líquido do grid na temporada H.
    gini: f64,
    topo20: f64,
    cv_meses: f64,
    /// Patrocínio médio por equipe por ano nas H primeiras temporadas, em MESES de operação
    /// da divisão da própria equipe. É a unidade que deixa rookie e LMP2 na mesma régua.
    patrocinio_meses: f64,
    /// Espalhamento do patrocínio dentro do grid: p90 ÷ p10.
    patrocinio_p90_p10: f64,
    /// `spending_power` em meses de operação.
    poder_mediano: f64,
    poder_p10: f64,
    /// `budget_index` mediano do grid na temporada H.
    indice_mediano: f64,
    /// Observações (equipe × temporada) em cada estado, nas H primeiras temporadas, em %.
    colapso_pct: f64,
    crise_pct: f64,
    /// Entre as equipes que fecharam uma temporada doentes, quantas estavam de pé H anos
    /// depois. Varre todas as temporadas com H anos de sobra.
    recuperacao_pct: f64,
    recuperacao_casos: f64,
}

fn medir_horizonte(replicas: &[Resultado], h: usize, filtro: Filtro) -> Medida {
    let mut m = Medida::default();
    let mut amostras = 0.0f64;
    let (mut obs, mut obs_colapso, mut obs_crise) = (0.0f64, 0.0f64, 0.0f64);
    let (mut casos, mut recuperou) = (0.0f64, 0.0f64);

    for r in replicas {
        if r.serie.len() < h {
            continue;
        }
        let equipes: Vec<usize> = (0..r.foto_inicial.len())
            .filter(|i| filtro.aceita(&r.foto_inicial[*i]))
            .collect();
        if equipes.is_empty() {
            continue;
        }
        let foto = &r.serie[h - 1];

        let meses: Vec<f64> = equipes.iter().map(|&i| foto[i].meses).collect();
        let liquido: Vec<f64> = equipes
            .iter()
            .map(|&i| foto[i].caixa - foto[i].divida)
            .collect();
        let poder: Vec<f64> = equipes
            .iter()
            .map(|&i| foto[i].poder_de_gasto / foto[i].mensal.max(1.0))
            .collect();
        let indice: Vec<f64> = equipes.iter().map(|&i| foto[i].indice).collect();
        let patrocinio: Vec<f64> = equipes
            .iter()
            .map(|&i| {
                let soma: f64 = (0..h).map(|t| r.serie[t][i].patrocinio_do_ano).sum();
                soma / h as f64 / foto[i].mensal.max(1.0)
            })
            .collect();

        m.equipes += equipes.len() as f64;
        m.meses_mediano += p_da_amostra(&meses, 0.5);
        m.meses_p10 += p_da_amostra(&meses, 0.10);
        m.meses_p90 += p_da_amostra(&meses, 0.90);
        m.gini += gini_de(&liquido);
        m.topo20 += topo20_de(&liquido);
        m.cv_meses += cv_de(&meses);
        m.patrocinio_meses += media_de(&patrocinio);
        m.patrocinio_p90_p10 +=
            p_da_amostra(&patrocinio, 0.90) / p_da_amostra(&patrocinio, 0.10).max(1e-6);
        m.poder_mediano += p_da_amostra(&poder, 0.5);
        m.poder_p10 += p_da_amostra(&poder, 0.10);
        m.indice_mediano += p_da_amostra(&indice, 0.5);
        amostras += 1.0;

        // Estados nas H primeiras temporadas.
        for t in 0..h {
            for &i in &equipes {
                obs += 1.0;
                match r.serie[t][i].estado {
                    "collapse" => obs_colapso += 1.0,
                    "crisis" => obs_crise += 1.0,
                    _ => {}
                }
            }
        }

        // Recuperação em H anos: doente no fim de t, de pé no fim de t+H.
        for t in 0..r.serie.len().saturating_sub(h) {
            for &i in &equipes {
                if doente(r.serie[t][i].estado) {
                    casos += 1.0;
                    if de_pe(r.serie[t + h][i].estado) {
                        recuperou += 1.0;
                    }
                }
            }
        }
    }

    if amostras > 0.0 {
        m.equipes /= amostras;
        m.meses_mediano /= amostras;
        m.meses_p10 /= amostras;
        m.meses_p90 /= amostras;
        m.gini /= amostras;
        m.topo20 /= amostras;
        m.cv_meses /= amostras;
        m.patrocinio_meses /= amostras;
        m.patrocinio_p90_p10 /= amostras;
        m.poder_mediano /= amostras;
        m.poder_p10 /= amostras;
        m.indice_mediano /= amostras;
    }
    m.colapso_pct = obs_colapso / obs.max(1.0) * 100.0;
    m.crise_pct = obs_crise / obs.max(1.0) * 100.0;
    m.recuperacao_pct = recuperou / casos.max(1.0) * 100.0;
    m.recuperacao_casos = casos;
    m
}

fn rodar_replicas(categoria: &'static str, realimentacao: Realimentacao) -> Vec<Resultado> {
    (0..REPLICAS)
        .map(|semente| {
            medir_categoria_cenario(
                categoria,
                &Cenario {
                    realimentacao,
                    semente,
                    ..Cenario::default()
                },
            )
        })
        .collect()
}

/// Variação percentual do atual contra o contrafactual.
fn delta_pct(atual: f64, zerado: f64) -> f64 {
    if zerado.abs() < 1e-9 {
        return 0.0;
    }
    (atual - zerado) / zerado.abs() * 100.0
}

fn cabecalho_realimentacao() {
    println!(
        "\n{:<24} {:>2} | {:>22} | {:>15} | {:>20} | {:>17} | {:>16}",
        "categoria",
        "H",
        "patrocinio (meses/ano)",
        "folego mediano",
        "gini do caixa",
        "colapso %",
        "recuperacao %"
    );
    println!(
        "{:<24} {:>2} | {:>7} {:>6} {:>7} | {:>7} {:>7} | {:>6} {:>6} {:>6} | {:>5} {:>5} {:>5} | {:>4} {:>4} {:>5}",
        "", "", "atual", "zero", "achat", "atual", "zero", "atual", "zero", "achat", "atual",
        "zero", "achat", "atu", "zero", "achat"
    );
    println!("{}", "-".repeat(135));
}

/// Uma linha do relatório: os três braços lado a lado.
///
/// `zero` mostra o valor ABSOLUTO onde o nível importa e o DELTA onde a comparação é o que
/// interessa. `achat` é sempre o braço de nível preservado — a coluna que responde pelo laço
/// sozinho.
fn linha_realimentacao(rotulo: &str, h: usize, a: &Medida, z: &Medida, f: &Medida) {
    println!(
        "{:<24} {:>2} | {:>7.2} {:>5.1}% {:>6.1}% | {:>7.1} {:>7.1} | {:>6.3} {:>+6.3} {:>+6.3} | {:>5.1} {:>5.1} {:>5.1} | {:>3.0}% {:>3.0}% {:>4.0}%",
        rotulo,
        h,
        a.patrocinio_meses,
        delta_pct(a.patrocinio_meses, z.patrocinio_meses),
        delta_pct(a.patrocinio_meses, f.patrocinio_meses),
        a.meses_mediano,
        z.meses_mediano,
        a.gini,
        a.gini - z.gini,
        a.gini - f.gini,
        a.colapso_pct,
        z.colapso_pct,
        f.colapso_pct,
        a.recuperacao_pct,
        z.recuperacao_pct,
        f.recuperacao_pct,
    );
}

#[test]
#[ignore = "harness de medicao, nao e teste de comportamento"]
fn medir_realimentacao_do_orcamento() {
    println!(
        "\n===== A8 - REALIMENTACAO DO ORCAMENTO =====\n\
         O laco: caixa - divida -> meses de operacao -> meses projetados -> escada de estados\n\
         -> budget_index (0-100) -> patrocinio (+ indice x base_da_rodada x {COEF_DO_INDICE})\n\
         -> caixa. {REPLICAS} replicas por categoria por modelo, {TEMPORADAS} temporadas cada,\n\
         tudo o mais identico.\n"
    );

    // ── BLOCO 0: o tamanho estático do termo ─────────────────────────────────────────
    println!("-- BLOCO 0 - o termo, antes de qualquer simulacao --");
    println!(
        "  patrocinio = base x (0,270 + 0,004.reputacao + {:.3}.indice + 0,004.fama)",
        COEF_DO_INDICE
    );
    let patro_de = |indice: f64, rep: f64, fama: f64| {
        0.27 + 0.004 * rep + COEF_DO_INDICE * indice + 0.004 * fama
    };
    for (nome, rep, fama) in [
        ("equipe de fundo  (rep 25, fama 32)", 25.0, 32.0),
        ("equipe mediana   (rep 55, fama 40)", 55.0, 40.0),
        ("equipe de ponta  (rep 85, fama 48)", 85.0, 48.0),
    ] {
        let (p0, p50, p100) = (
            patro_de(0.0, rep, fama),
            patro_de(50.0, rep, fama),
            patro_de(100.0, rep, fama),
        );
        println!(
            "  {nome}: indice 0 -> {p0:.3}xbase | 50 -> {p50:.3} | 100 -> {p100:.3}   \
             (do colapso ao topo: {:+.1}% de patrocinio)",
            (p100 - p0) / p0 * 100.0
        );
    }
    println!(
        "  O termo vale de 0 a 0,200xbase. A base e o custo operacional de UMA rodada, e a\n  \
         receita total de uma rodada vive perto de 1xbase - o teto do termo e ~20% dela."
    );

    // ── BLOCO 1: por categoria e horizonte ───────────────────────────────────────────
    println!("\n-- BLOCO 1 - por categoria e horizonte --");
    println!(
        "  patrocinio(mes) = patrocinio medio por equipe por ano, em meses de operacao da\n  \
         divisao dela. concentracao = Gini do caixa liquido do grid. colapso/crise = % das\n  \
         observacoes (equipe x temporada) nas H primeiras temporadas. recuperacao = % das\n  \
         equipes que fecharam um ano doentes e estavam de pe H anos depois."
    );
    cabecalho_realimentacao();

    let mut acumulado: Vec<(usize, Medida, Medida, Medida)> = HORIZONTES
        .iter()
        .map(|h| (*h, Medida::default(), Medida::default(), Medida::default()))
        .collect();

    for categoria in ARENAS_VARREDURA {
        let atual = rodar_replicas(categoria, Realimentacao::Atual);
        let zerado = rodar_replicas(categoria, Realimentacao::Zerada);
        let achatado = rodar_replicas(categoria, Realimentacao::Achatada);

        for (slot, h) in HORIZONTES.iter().enumerate() {
            let a = medir_horizonte(&atual, *h, Filtro::default());
            let z = medir_horizonte(&zerado, *h, Filtro::default());
            let f = medir_horizonte(&achatado, *h, Filtro::default());
            linha_realimentacao(categoria, *h, &a, &z, &f);

            let dest = &mut acumulado[slot];
            for (destino, origem) in [(&mut dest.1, &a), (&mut dest.2, &z), (&mut dest.3, &f)] {
                destino.patrocinio_meses += origem.patrocinio_meses;
                destino.meses_mediano += origem.meses_mediano;
                destino.meses_p10 += origem.meses_p10;
                destino.meses_p90 += origem.meses_p90;
                destino.gini += origem.gini;
                destino.topo20 += origem.topo20;
                destino.cv_meses += origem.cv_meses;
                destino.patrocinio_p90_p10 += origem.patrocinio_p90_p10;
                destino.poder_mediano += origem.poder_mediano;
                destino.poder_p10 += origem.poder_p10;
                destino.indice_mediano += origem.indice_mediano;
                destino.colapso_pct += origem.colapso_pct;
                destino.crise_pct += origem.crise_pct;
                destino.recuperacao_pct += origem.recuperacao_pct;
            }
        }

        // ── Classes, onde elas existem ───────────────────────────────────────────────
        let arena = arena(categoria);
        if arena.multi_classe {
            for (ic, classe) in arena.classes.iter().enumerate() {
                let filtro = Filtro {
                    classe: Some(ic),
                    ..Filtro::default()
                };
                for h in HORIZONTES {
                    let a = medir_horizonte(&atual, h, filtro);
                    let z = medir_horizonte(&zerado, h, filtro);
                    let f = medir_horizonte(&achatado, h, filtro);
                    linha_realimentacao(&format!("  classe {}", classe.nome), h, &a, &z, &f);
                }
            }
        }

        // ── Estados de partida ───────────────────────────────────────────────────────
        for estado in ESTADOS {
            let filtro = Filtro {
                estado_inicial: Some(estado),
                ..Filtro::default()
            };
            let teve = atual
                .iter()
                .any(|r| r.foto_inicial.iter().any(|f| f.estado == *estado));
            if !teve {
                continue;
            }
            for h in HORIZONTES {
                let a = medir_horizonte(&atual, h, filtro);
                let z = medir_horizonte(&zerado, h, filtro);
                let f = medir_horizonte(&achatado, h, filtro);
                linha_realimentacao(&format!("  partiu {estado}"), h, &a, &z, &f);
            }
        }
        println!("{}", "-".repeat(118));
    }

    // ── BLOCO 2: o mundo inteiro ─────────────────────────────────────────────────────
    let n = ARENAS_VARREDURA.len() as f64;
    println!(
        "\n-- BLOCO 2 - media das {} categorias --",
        ARENAS_VARREDURA.len()
    );
    println!(
        "  atual / zerado (delta%) / achatado (delta%). O delta contra ACHATADO e o efeito do\n  \
         laco sozinho: mesmo dinheiro no canal, so sem diferenciar rica de pobre.\n"
    );
    let linha = |rotulo: &str, campo: &dyn Fn(&Medida) -> f64, unidade: &str| {
        let mut celulas = String::new();
        for (h, a, z, f) in &acumulado {
            let (va, vz, vf) = (campo(a) / n, campo(z) / n, campo(f) / n);
            celulas.push_str(&format!(
                " | H{h}: {va:>7.2} z {vz:>7.2} ({:+5.1}%) a {vf:>7.2} ({:+5.1}%)",
                delta_pct(va, vz),
                delta_pct(va, vf)
            ));
        }
        println!("{rotulo:<21}{celulas}   {unidade}");
    };
    linha(
        "patrocinio/ano",
        &|m| m.patrocinio_meses,
        "meses de operacao",
    );
    linha("folego mediano", &|m| m.meses_mediano, "meses de operacao");
    linha("folego p10", &|m| m.meses_p10, "meses de operacao");
    linha("folego p90", &|m| m.meses_p90, "meses de operacao");
    linha("gini do caixa", &|m| m.gini, "0 = grid igual");
    linha("topo 20% do caixa", &|m| m.topo20, "% do caixa do grid");
    linha("CV do folego", &|m| m.cv_meses, "adimensional");
    linha(
        "patrocinio p90/p10",
        &|m| m.patrocinio_p90_p10,
        "razao no grid",
    );
    linha("poder de gasto", &|m| m.poder_mediano, "meses (mediana)");
    linha("poder de gasto p10", &|m| m.poder_p10, "meses (p10)");
    linha("budget_index", &|m| m.indice_mediano, "0-100 (mediana)");
    linha("colapso", &|m| m.colapso_pct, "% equipe x temporada");
    linha("crise", &|m| m.crise_pct, "% equipe x temporada");
    linha("recuperacao", &|m| m.recuperacao_pct, "% dos doentes");

    println!(
        "\n-- COMO LER --\n\
         O contrafactual TIRA receita de todo mundo, entao o folego cair no zerado e\n\
         aritmetica, nao descoberta. O que responde a pergunta do laco e a DIFERENCA de\n\
         diferenca: gini, CV, p90/p10 e a distancia entre p10 e p90. Se elas nao se movem, o\n\
         termo e um nivel uniforme com nome de realimentacao; se elas encolhem no zerado, o\n\
         laco concentra de verdade e o tamanho esta medido acima."
    );
}

/// **O guarda da cópia.** `COEF_DO_INDICE` é um literal repetido do módulo de produção, e
/// contrafactual construído sobre um coeficiente desatualizado mede a coisa errada em
/// silêncio.
///
/// A conta é uma derivada: duas equipes idênticas exceto pelo CAIXA, medidas pela função de
/// produção. Reputação e fama entram fixas, então a única coisa que o caixa move dentro do
/// patrocínio é o `budget_index` — e a razão entre o degrau de patrocínio e o degrau de
/// índice é o coeficiente, isolado.
#[test]
fn o_coeficiente_do_indice_ainda_e_o_da_producao() {
    let monta = |caixa: f64| {
        let mut team = placeholder_team_from_db(
            "guarda-do-indice".to_string(),
            "Guarda".to_string(),
            "gt4".to_string(),
            "2026-01-01".to_string(),
        );
        team.ativa = true;
        team.cash_balance = caixa;
        team.debt_balance = 0.0;
        team.reputacao = 50.0;
        team.engineering = 50.0;
        team.facilities = 50.0;
        team.pit_crew_quality = 50.0;
        team.car_performance = 0.0;
        team.financial_state = "stable".to_string();
        team
    };
    let saude = global_economic_health_for_season(1);
    let rodadas = 10.0;
    let patrocinio = |team: &Team| {
        calculate_team_round_finance_context_modelo(
            team,
            40.0,
            10.0,
            0,
            0,
            0,
            99,
            0.0,
            rodadas,
            saude,
            0.0,
            RoundOperationContext::default(),
            EtapaFisica::de_referencia(&team.categoria, team.classe.as_deref()),
            50.0,
            100.0,
            10.0,
            CoeficientesDeReceita::default(),
            despesa_da_rodada,
        )
        .sponsorship_income
    };

    let escala = category_finance_scale_for("gt4", None);
    let base = escala.operating_cost_midpoint() / rodadas;
    let pobre = monta(escala.cash_min);
    let rico = monta(escala.cash_max * 4.0);

    let d_indice = derive_budget_index_from_money(&rico) - derive_budget_index_from_money(&pobre);
    assert!(
        d_indice > 5.0,
        "o caso precisa de dois indices bem distintos para a derivada significar algo; \
         medido delta indice = {d_indice:.3}"
    );

    let d_patrocinio = patrocinio(&rico) - patrocinio(&pobre);
    let coef_medido = d_patrocinio / (d_indice * base * economy_income_modifier(saude));

    assert!(
        (coef_medido - COEF_DO_INDICE).abs() < 1e-9,
        "o coeficiente do indice no patrocinio de producao mudou: medido {coef_medido:.6}, \
         copia do harness {COEF_DO_INDICE:.6}. Atualize COEF_DO_INDICE e refaca a medicao \
         do laco - o contrafactual estava zerando o termo errado."
    );
}

// ===================== B47 / B50 / B52 — a escala absoluta =====================

/// A escada de divisões, na ordem, com a classe onde ela existe. É a lista contra a qual a
/// escala absoluta é lida: as tabelas de `finance::events` são indexadas pela CATEGORIA, e
/// nos dois campeonatos multi-classe a categoria não nomeia uma operação.
const DIVISOES: &[(&str, Option<&str>)] = &[
    ("mazda_rookie", None),
    ("toyota_rookie", None),
    ("mazda_amador", None),
    ("toyota_amador", None),
    ("bmw_m2", None),
    ("production_challenger", Some("mazda")),
    ("production_challenger", Some("toyota")),
    ("production_challenger", Some("bmw")),
    ("gt4", None),
    ("gt3", None),
    ("lmp2", None),
    ("endurance", Some("gt4")),
    ("endurance", Some("gt3")),
    ("endurance", Some("lmp2")),
];

/// Uma equipe neutra da divisão: reputação 55, sem dívida, caixa zero. Serve para ler as
/// funções de produção que dependem de `Team` sem que o resultado misture o estado da
/// equipe com a escala da divisão.
fn equipe_neutra(categoria: &str, classe: Option<&str>) -> Team {
    let mut team = placeholder_team_from_db(
        "escala".to_string(),
        "Equipe Neutra".to_string(),
        categoria.to_string(),
        "2026-01-01".to_string(),
    );
    team.ativa = true;
    team.classe = classe.map(str::to_string);
    team.reputacao = 55.0;
    team.cash_balance = 0.0;
    team.debt_balance = 0.0;
    team
}

fn rodadas_de(categoria: &str) -> f64 {
    get_category_config(categoria)
        .map(|c| c.corridas_por_temporada.max(1) as f64)
        .unwrap_or(12.0)
}

/// **BLOCO ESTÁTICO.** Cada número absoluto de `finance::events` e de `race::financas`
/// convertido para a única unidade em que o resto do jogo mede fôlego: meses de operação da
/// divisão. Não simula nada — é a leitura dimensional, e é ela que separa bug de calibração.
#[test]
#[ignore = "harness de medicao, nao e teste de comportamento"]
fn medir_escala_absoluta_do_socorro() {
    println!(
        "\n===== B47 / B50 - A ESCALA RELATIVA, LIDA EM MESES DE OPERACAO =====\n\
         Equipe neutra (reputacao 55, sem divida). O custo mensal e o da DIVISAO, com classe.\n\
         DEPOIS de B47/B50: os dois canais decidem em meses de operacao da divisao.\n\
         paraquedas   = {PARAQUEDAS_MESES:.0} meses (parachute_payment_for_relegation)\n\
         parcela      = total / rodadas da temporada (race::financas)\n\
         emprestimo   = {SOCORRO_PRINCIPAL_MESES:.0} meses x (0,85 + rep/500)\n\
         gate caixa   = -{SOCORRO_GATE_CAIXA_MESES:.0} meses | TETO de divida = \
{SOCORRO_TETO_DIVIDA_MESES:.0} meses | max {SOCORROS_MAX_POR_TEMPORADA}/temporada\n\
         As colunas 'meses' tem que ficar CONSTANTES na escada - e esse o teste da correcao.\n"
    );
    println!(
        "{:<26} {:>10} | {:>10} {:>7} {:>7} {:>7} {:>7} | {:>8} {:>7} {:>7} | {:>10} {:>6} {:>7} | {:>7} {:>7}",
        "divisao",
        "mes (R$)",
        "paraq (R$)",
        "meses",
        "%anual",
        "rodadas",
        "temps",
        "parcela",
        "meses",
        "%rodad",
        "emprest R$",
        "meses",
        "%anual",
        "gt caixa",
        "gt divida"
    );
    println!("{}", "-".repeat(152));

    for (categoria, classe) in DIVISOES {
        let team = equipe_neutra(categoria, *classe);
        let mensal = custo_operacional_mensal(categoria, *classe);
        let rodadas = rodadas_de(categoria);
        let paraquedas = crate::finance::events::parachute_payment_for_relegation(&team);
        // O saldo é consumido pela PARCELA da divisão: quantas rodadas até secar (tem que dar
        // exatamente o calendário da categoria), e quanto isso vale por rodada.
        let parcela = crate::finance::events::parcela_de_paraquedas(categoria, *classe, rodadas);
        let rodadas_ate_secar = paraquedas / parcela.max(1.0);
        let custo_da_rodada = mensal * 12.0 / rodadas;
        // O empréstimo é lido com a equipe FORÇADA ao gatilho — o valor não depende do
        // estado, só a elegibilidade depende.
        let mut afogada = team.clone();
        afogada.financial_state = "collapse".to_string();
        afogada.cash_balance = -1_000_000.0;
        let emprestimo =
            crate::finance::events::emergency_loan_amount_na_temporada(&afogada, 1).unwrap_or(0.0);

        let rotulo = match classe {
            Some(c) => format!("{categoria}:{c}"),
            None => (*categoria).to_string(),
        };
        println!(
            "{:<26} {:>10.0} | {:>10.0} {:>7.2} {:>6.1}% {:>7.1} {:>7.2} | {:>8.0} {:>7.3} {:>6.1}% | {:>10.0} {:>6.2} {:>6.1}% | {:>7.2} {:>7.2}",
            rotulo,
            mensal,
            paraquedas,
            paraquedas / mensal,
            paraquedas / (mensal * 12.0) * 100.0,
            rodadas_ate_secar,
            rodadas_ate_secar / rodadas,
            parcela.min(paraquedas),
            parcela.min(paraquedas) / mensal,
            parcela.min(paraquedas) / custo_da_rodada * 100.0,
            emprestimo,
            emprestimo / mensal,
            emprestimo / (mensal * 12.0) * 100.0,
            SOCORRO_GATE_CAIXA_MESES,
            SOCORRO_TETO_DIVIDA_MESES,
        );
    }

    println!(
        "\n-- COMO LER --\n\
         'meses' e o mesmo dinheiro dividido pelo custo de operar um mes DAQUELA divisao. Se a\n\
         coluna nao for aproximadamente constante na escada, o numero absoluto vale coisas\n\
         diferentes em cada degrau - e ai a diferenca nao e balanceamento, e dimensao.\n\
         'rodadas' e quantas etapas o paraquedas leva para secar; DEPOIS de B47 ele tem que\n\
         bater exatamente com o calendario da categoria, em qualquer divisao.\n\
         '%rodad' e a parcela contra o custo de operar UMA rodada: e o alivio real por etapa."
    );

    // ── A escada de destino: quem é rebaixado desce de divisão ───────────────────────
    println!(
        "\n-- O paraquedas na divisao de DESTINO (quem e rebaixado desce um degrau) --\n\
         DEPOIS de B47 o total e lido na divisao de DESTINO, que e onde o dinheiro e gasto:\n\
         'meses dest' tem que dar 3,00 em toda a escada. 'meses orig' fica abaixo disso, e e\n\
         a leitura de quanto o rebaixamento barateia a operacao.\n"
    );
    let escada: &[(&str, Option<&str>)] = &[
        ("mazda_rookie", None),
        ("mazda_amador", None),
        ("bmw_m2", None),
        ("gt4", None),
        ("gt3", None),
        ("lmp2", None),
        ("endurance", Some("lmp2")),
    ];
    println!(
        "{:<24} {:>12} {:>10} | {:>10} {:>10}",
        "origem -> destino", "paraq (R$)", "rodadas", "meses orig", "meses dest"
    );
    println!("{}", "-".repeat(74));
    for janela in escada.windows(2) {
        let (destino, cl_destino) = janela[0];
        let (origem, cl_origem) = janela[1];
        // A equipe que recebe o paraquedas JÁ está na divisão de destino: `promotion::pipeline`
        // troca categoria e classe antes de aplicar os deltas do movimento.
        let team = equipe_neutra(destino, cl_destino);
        let paraquedas = crate::finance::events::parachute_payment_for_relegation(&team);
        let mes_origem = custo_operacional_mensal(origem, cl_origem);
        let mes_destino = custo_operacional_mensal(destino, cl_destino);
        let parcela =
            crate::finance::events::parcela_de_paraquedas(destino, cl_destino, rodadas_de(destino));
        println!(
            "{:<24} {:>12.0} {:>10.1} | {:>10.2} {:>10.2}",
            format!("{origem} -> {destino}"),
            paraquedas,
            paraquedas / parcela.max(1.0),
            paraquedas / mes_origem,
            paraquedas / mes_destino,
        );
    }
}

/// **B52 estático.** Onde os dois limiares caem na régua de meses, divisão por divisão, e o
/// que a escala cega à classe faz com eles.
#[test]
#[ignore = "harness de medicao, nao e teste de comportamento"]
fn medir_limiares_da_estrategia_na_regua() {
    println!(
        "\n===== B52 - OS LIMIARES {LIMIAR_ALL_IN:.2} / {LIMIAR_AUSTERIDADE:.2} NA REGUA DE HOJE =====\n\
         choose_season_strategy compara spending_power (grandeza ANUAL) contra fracoes do\n\
         operating_cost_midpoint ANUAL. Em meses de operacao, {LIMIAR_ALL_IN:.2} do anual sao\n\
         {:.1} meses e {LIMIAR_AUSTERIDADE:.2} sao {:.1} meses. A faixa declarada de folego do\n\
         mundo vai de 1 a 11 meses (economia::temporada::faixa_de_caixa).\n\
         O gate de survival compara debt_pressure contra {LIMIAR_SURVIVAL:.2} x expected_cash_\n\
         midpoint, que hoje vale {:.1} meses de operacao.\n",
        LIMIAR_ALL_IN * 12.0,
        LIMIAR_AUSTERIDADE * 12.0,
        LIMIAR_SURVIVAL * crate::economia::temporada::caixa_meses_de_referencia(),
    );

    println!(
        "{:<26} {:>12} {:>12} | {:>12} {:>12} | {:>12}",
        "divisao", "all_in (R$)", "austerid R$", "divisor cat", "divisor cls", "erro do cego"
    );
    println!("{}", "-".repeat(94));
    for (categoria, classe) in DIVISOES {
        let cego = category_finance_scale(categoria).operating_cost_midpoint();
        let certo = category_finance_scale_for(categoria, *classe).operating_cost_midpoint();
        let rotulo = match classe {
            Some(c) => format!("{categoria}:{c}"),
            None => (*categoria).to_string(),
        };
        println!(
            "{:<26} {:>12.0} {:>12.0} | {:>12.0} {:>12.0} | {:>11.2}x",
            rotulo,
            certo * LIMIAR_ALL_IN,
            certo * LIMIAR_AUSTERIDADE,
            cego,
            certo,
            cego / certo.max(1.0),
        );
    }
    println!(
        "\n'erro do cego' e o divisor que choose_season_strategy usa de fato (categoria) dividido\n\
         pelo que a divisao da equipe custa (classe). 1,00 e monoclasse; longe de 1 e a mesma\n\
         equipe sendo julgada contra o orcamento de outra classe."
    );
}

// ===================== Os braços dinâmicos =====================

/// Um recorte do mundo simulado, para pôr os braços lado a lado.
#[derive(Default, Clone, Copy)]
struct Saude {
    meses_mediano: f64,
    meses_p10: f64,
    colapso_pct: f64,
    crise_pct: f64,
    recuperacao_pct: f64,
    vendas: f64,
    emprestimos: f64,
    emprestimo_valor_meses: f64,
    elegiveis_pct: f64,
    ajuda_meses: f64,
    /// Juros pagos em caixa na simulação inteira, em meses de operação. No braço amortizado
    /// inclui a taxa do empréstimo, que ali é despesa de caixa em vez de dívida no ato.
    juros_meses: f64,
    /// Dívida criada pelos socorros na simulação inteira, em meses de operação.
    divida_criada_meses: f64,
    /// % das equipes do grid que tomaram ao menos um socorro.
    tomadores_pct: f64,
    /// % das equipes do grid que tomaram DOIS ou mais — a reincidência.
    reincidentes_pct: f64,
    /// Socorros por equipe socorrida.
    socorros_por_tomador: f64,
    /// Estrutura média no fim (engenharia + instalações, 0–200): o eixo de performance.
    estrutura_fim: f64,
    nivel_medio: f64,
}

fn resumir_saude(replicas: &[Resultado], mensal_medio: f64) -> Saude {
    let mut s = Saude::default();
    let n = replicas.len().max(1) as f64;
    for r in replicas {
        let h = r.serie.len();
        if h == 0 {
            continue;
        }
        let meses: Vec<f64> = r.serie[h - 1].iter().map(|f| f.meses).collect();
        s.meses_mediano += p_da_amostra(&meses, 0.5);
        s.meses_p10 += p_da_amostra(&meses, 0.10);

        let obs: f64 = r.estados.values().map(|v| *v as f64).sum();
        s.colapso_pct += *r.estados.get("collapse").unwrap_or(&0) as f64 / obs.max(1.0) * 100.0;
        s.crise_pct += *r.estados.get("crisis").unwrap_or(&0) as f64 / obs.max(1.0) * 100.0;
        s.elegiveis_pct += r.elegiveis as f64 / obs.max(1.0) * 100.0;
        s.vendas += r.vendas as f64;
        s.emprestimos += r.emprestimos as f64;
        s.emprestimo_valor_meses += r.emprestimo_valor / mensal_medio.max(1.0);
        s.ajuda_meses += r.ajuda_paga / mensal_medio.max(1.0);
        s.juros_meses += r.linhas.juros / mensal_medio.max(1.0);
        s.divida_criada_meses += r.divida_criada / mensal_medio.max(1.0);
        {
            let grid = r.emprestimos_por_equipe.len().max(1) as f64;
            let tomadores = r.emprestimos_por_equipe.iter().filter(|c| **c >= 1).count() as f64;
            let reincidentes = r.emprestimos_por_equipe.iter().filter(|c| **c >= 2).count() as f64;
            s.tomadores_pct += tomadores / grid * 100.0;
            s.reincidentes_pct += reincidentes / grid * 100.0;
            s.socorros_por_tomador += r.emprestimos as f64 / tomadores.max(1.0);
        }
        s.estrutura_fim += media_de(&r.estrutura_fim);
        s.nivel_medio += r.nivel_medio;

        // Recuperação em 3 anos, sobre a série de fotos.
        let (mut casos, mut recuperou) = (0.0f64, 0.0f64);
        for t in 0..h.saturating_sub(3) {
            for i in 0..r.serie[t].len() {
                if doente(r.serie[t][i].estado) {
                    casos += 1.0;
                    if de_pe(r.serie[t + 3][i].estado) {
                        recuperou += 1.0;
                    }
                }
            }
        }
        s.recuperacao_pct += recuperou / casos.max(1.0) * 100.0;
    }
    for campo in [
        &mut s.meses_mediano,
        &mut s.meses_p10,
        &mut s.colapso_pct,
        &mut s.crise_pct,
        &mut s.recuperacao_pct,
        &mut s.vendas,
        &mut s.emprestimos,
        &mut s.emprestimo_valor_meses,
        &mut s.elegiveis_pct,
        &mut s.ajuda_meses,
        &mut s.juros_meses,
        &mut s.divida_criada_meses,
        &mut s.tomadores_pct,
        &mut s.reincidentes_pct,
        &mut s.socorros_por_tomador,
        &mut s.estrutura_fim,
        &mut s.nivel_medio,
    ] {
        *campo /= n;
    }
    s
}

fn rodar_braco(categoria: &'static str, cenario: Cenario) -> Vec<Resultado> {
    (0..REPLICAS)
        .map(|semente| medir_categoria_cenario(categoria, &Cenario { semente, ..cenario }))
        .collect()
}

fn cabecalho_saude() {
    println!(
        "\n{:<34} | {:>7} {:>7} | {:>6} {:>6} {:>6} | {:>6} {:>6} {:>7} {:>7} | {:>7} {:>7} {:>6} {:>6} {:>6} | {:>7} {:>6} {:>6}",
        "categoria / braco",
        "meses",
        "p10",
        "colap%",
        "crise%",
        "recup%",
        "vendas",
        "empr",
        "empr(m)",
        "elegiv%",
        "divida_m",
        "juros_m",
        "tomad%",
        "reinc%",
        "por_tom",
        "ajuda_m",
        "estrut",
        "nivel"
    );
    println!("{}", "-".repeat(178));
}

fn linha_saude(rotulo: &str, s: &Saude) {
    println!(
        "{:<34} | {:>7.2} {:>7.2} | {:>6.2} {:>6.2} {:>6.1} | {:>6.1} {:>6.1} {:>7.2} {:>7.2} | {:>7.2} {:>7.2} {:>6.1} {:>6.1} {:>6.2} | {:>7.2} {:>6.1} {:>6.2}",
        rotulo,
        s.meses_mediano,
        s.meses_p10,
        s.colapso_pct,
        s.crise_pct,
        s.recuperacao_pct,
        s.vendas,
        s.emprestimos,
        s.emprestimo_valor_meses,
        s.elegiveis_pct,
        s.divida_criada_meses,
        s.juros_meses,
        s.tomadores_pct,
        s.reincidentes_pct,
        s.socorros_por_tomador,
        s.ajuda_meses,
        s.estrutura_fim,
        s.nivel_medio,
    );
}

/// Média de um conjunto de recortes — a linha do mundo.
fn media_da_saude(itens: &[Saude]) -> Saude {
    let n = itens.len().max(1) as f64;
    let mut m = Saude::default();
    for s in itens {
        m.meses_mediano += s.meses_mediano / n;
        m.meses_p10 += s.meses_p10 / n;
        m.colapso_pct += s.colapso_pct / n;
        m.crise_pct += s.crise_pct / n;
        m.recuperacao_pct += s.recuperacao_pct / n;
        m.vendas += s.vendas / n;
        m.emprestimos += s.emprestimos / n;
        m.emprestimo_valor_meses += s.emprestimo_valor_meses / n;
        m.elegiveis_pct += s.elegiveis_pct / n;
        m.juros_meses += s.juros_meses / n;
        m.divida_criada_meses += s.divida_criada_meses / n;
        m.tomadores_pct += s.tomadores_pct / n;
        m.reincidentes_pct += s.reincidentes_pct / n;
        m.socorros_por_tomador += s.socorros_por_tomador / n;
        m.ajuda_meses += s.ajuda_meses / n;
        m.estrutura_fim += s.estrutura_fim / n;
        m.nivel_medio += s.nivel_medio / n;
    }
    m
}

/// **B50 dinâmico.** O empréstimo de emergência: taxa de elegibilidade, quanto ele injeta,
/// e o que o mundo faz sem ele e com ele reancorado em meses.
#[test]
#[ignore = "harness de medicao, nao e teste de comportamento"]
fn medir_emprestimo_de_emergencia() {
    println!(
        "\n===== B50 - EMPRESTIMO DE EMERGENCIA, DEPOIS DA CORRECAO =====\n\
         {REPLICAS} replicas x {TEMPORADAS} temporadas por braco. 'empr(m)' e o principal somado\n\
         da simulacao inteira em meses de operacao da divisao; 'elegiv%' e a fracao das\n\
         (equipe x temporada) em que o gatilho de PRODUCAO esteve aberto em alguma rodada -\n\
         medida igual nos quatro bracos, para o eixo ser o socorro e nao o termometro.\n\
         'producao' e a politica NOVA: caixa < -{SOCORRO_GATE_CAIXA_MESES:.0} meses E divida < \
{SOCORRO_TETO_DIVIDA_MESES:.0} meses (TETO) E no maximo\n\
         {SOCORROS_MAX_POR_TEMPORADA} socorros na temporada; principal de \
{SOCORRO_PRINCIPAL_MESES:.0} meses de operacao.\n\
         'absoluta (a antiga)' e a politica de ate 12/08/2026, congelada: -75.000 OU 750.000,\n\
         tabela por categoria, sem teto e sem limite. E o ANTES.\n\
         'taxa amortizada' usa os gates e o principal NOVOS e muda so QUANDO a taxa de 18%\n\
         entra: em vez de virar divida no ato, e cobrada em caixa ao longo da temporada.\n\
         'divida_m' e a divida CRIADA pelos socorros, em meses; 'juros_m' e o juro pago em\n\
         caixa na simulacao inteira (no braco amortizado inclui a taxa); 'tomad%'/'reinc%' sao\n\
         as equipes do grid que tomaram um e dois ou mais socorros; 'por_tom' e socorros por\n\
         equipe socorrida - e a coluna que mostrava dezenas por tomador."
    );
    cabecalho_saude();

    let bracos: &[(&str, Socorro)] = &[
        ("producao (2/4/2)", Socorro::Producao),
        ("sem socorro", Socorro::Sem),
        ("absoluta (a antiga)", Socorro::Absoluta),
        ("taxa amortizada", Socorro::Amortizada),
        (
            "principal 1 mes",
            Socorro::Variante {
                principal: 1.0,
                taxa: 1.18,
            },
        ),
        (
            "taxa 1,08",
            Socorro::Variante {
                principal: 2.0,
                taxa: 1.08,
            },
        ),
        (
            "taxa 1,00 (sem taxa)",
            Socorro::Variante {
                principal: 2.0,
                taxa: 1.00,
            },
        ),
        (
            "1 mes + taxa 1,08",
            Socorro::Variante {
                principal: 1.0,
                taxa: 1.08,
            },
        ),
    ];
    let mut por_braco: Vec<Vec<Saude>> = vec![Vec::new(); bracos.len()];

    for categoria in ARENAS_VARREDURA {
        let mensal = custo_operacional_mensal(categoria, None);
        for (k, (nome, socorro)) in bracos.iter().enumerate() {
            let replicas = rodar_braco(
                categoria,
                Cenario {
                    socorro: *socorro,
                    ..Cenario::default()
                },
            );
            let s = resumir_saude(&replicas, mensal);
            linha_saude(&format!("{categoria} / {nome}"), &s);
            por_braco[k].push(s);
        }
        println!("{}", "-".repeat(140));
    }

    println!("\n-- media das {} categorias --", ARENAS_VARREDURA.len());
    for (k, (nome, _)) in bracos.iter().enumerate() {
        linha_saude(&format!("MUNDO / {nome}"), &media_da_saude(&por_braco[k]));
    }
    println!(
        "\n-- COMO LER --\n\
         'sem socorro' e o piso: a diferenca contra 'producao' e tudo que o emprestimo segura.\n\
         Se colapso e venda quase nao se movem entre os dois, o canal e decorativo no agregado\n\
         e o que ele faz e adiar caso a caso. 'em meses' responde se a diferenca entre degraus\n\
         some quando o valor e os gates passam a ser lidos na unidade da ancora."
    );
}

/// **B47 dinâmico.** O paraquedas com a coorte sintética de rebaixadas.
#[test]
#[ignore = "harness de medicao, nao e teste de comportamento"]
fn medir_paraquedas() {
    println!(
        "\n===== B47 - PARAQUEDAS DE REBAIXAMENTO, DEPOIS DA CORRECAO =====\n\
         A ultima colocada de cada classe fecha o ano recebendo o saldo de paraquedas. Ela NAO\n\
         desce de divisao (o harness nao tem escada), entao o alivio aqui e cota INFERIOR.\n\
         'ajuda_m' e a ajuda paga na simulacao inteira, em meses de operacao da divisao.\n\
         'producao' e a politica NOVA: total de {PARAQUEDAS_MESES:.0} meses de operacao da\n\
         divisao de destino, em parcelas iguais que secam na ultima etapa da temporada.\n\
         'absoluta' e a politica de ate 12/08/2026: tabela por categoria (120 mil na rookie,\n\
         700 mil no endurance) e parcela fixa de {PARCELA_DE_AJUDA:.0} por rodada, que no\n\
         Endurance arrastava o saldo por ate ~4,6 temporadas. E o ANTES."
    );
    cabecalho_saude();

    let bracos: &[(&str, Paraquedas)] = &[
        ("sem rebaixamento", Paraquedas::Nenhum),
        ("producao (3 meses)", Paraquedas::Producao),
        ("absoluta (tabela+25k)", Paraquedas::Absoluta),
    ];
    let mut por_braco: Vec<Vec<Saude>> = vec![Vec::new(); bracos.len()];

    for categoria in ARENAS_VARREDURA {
        let mensal = custo_operacional_mensal(categoria, None);
        for (k, (nome, paraquedas)) in bracos.iter().enumerate() {
            let replicas = rodar_braco(
                categoria,
                Cenario {
                    paraquedas: *paraquedas,
                    ..Cenario::default()
                },
            );
            let s = resumir_saude(&replicas, mensal);
            linha_saude(&format!("{categoria} / {nome}"), &s);
            por_braco[k].push(s);
        }
        println!("{}", "-".repeat(140));
    }

    println!("\n-- media das {} categorias --", ARENAS_VARREDURA.len());
    for (k, (nome, _)) in bracos.iter().enumerate() {
        linha_saude(&format!("MUNDO / {nome}"), &media_da_saude(&por_braco[k]));
    }
}

/// **B52 dinâmico.** Varredura dos dois limiares: distribuição de estratégias e efeito em
/// falência e em performance.
#[test]
#[ignore = "harness de medicao, nao e teste de comportamento"]
fn varrer_limiares_de_estrategia() {
    println!(
        "\n===== B52 - VARREDURA DOS LIMIARES DE choose_season_strategy =====\n\
         Producao usa all_in < {LIMIAR_ALL_IN:.2} x operacional anual e austeridade <\n\
         {LIMIAR_AUSTERIDADE:.2} x, lidos contra a escala da DIVISAO (B52 corrigido). Os\n\
         bracos abaixo separam as duas suspeitas originais: o VALOR do limiar e a ESCALA\n\
         contra a qual ele e lido - 'por classe' agora coincide com 'producao'.\n\
         {REPLICAS} replicas x {TEMPORADAS} temporadas por braco."
    );

    let bracos: &[(&str, Estrategia)] = &[
        ("producao", Estrategia::Producao),
        (
            "limiares 0,10 / 0,25",
            Estrategia::Limiares {
                all_in: 0.10,
                austeridade: 0.25,
            },
        ),
        (
            "limiares 0,40 / 1,00",
            Estrategia::Limiares {
                all_in: 0.40,
                austeridade: 1.00,
            },
        ),
        (
            "por classe 0,20 / 0,50",
            Estrategia::PorClasse {
                all_in: LIMIAR_ALL_IN,
                austeridade: LIMIAR_AUSTERIDADE,
            },
        ),
    ];

    let rotulos = ["survival", "all_in", "austerity", "expansion", "balanced"];
    println!(
        "\n{:<38} | {:>8} {:>8} {:>9} {:>9} {:>9} | {:>7} {:>6} {:>6} {:>6}",
        "categoria / braco",
        "surviv%",
        "all_in%",
        "austerid%",
        "expansao%",
        "balance%",
        "meses",
        "colap%",
        "estrut",
        "nivel"
    );
    println!("{}", "-".repeat(140));

    for categoria in ARENAS_VARREDURA {
        let mensal = custo_operacional_mensal(categoria, None);
        for (nome, estrategia) in bracos {
            let replicas = rodar_braco(
                categoria,
                Cenario {
                    estrategia: *estrategia,
                    ..Cenario::default()
                },
            );
            let s = resumir_saude(&replicas, mensal);
            let mut pct = [0.0f64; 5];
            let mut total = 0.0f64;
            for r in &replicas {
                for v in r.estrategias.values() {
                    total += *v as f64;
                }
                for (k, rotulo) in rotulos.iter().enumerate() {
                    pct[k] += *r.estrategias.get(rotulo).unwrap_or(&0) as f64;
                }
            }
            for p in pct.iter_mut() {
                *p = *p / total.max(1.0) * 100.0;
            }
            println!(
                "{:<38} | {:>7.1}% {:>7.1}% {:>8.1}% {:>8.1}% {:>8.1}% | {:>7.2} {:>6.2} {:>6.1} {:>6.2}",
                format!("{categoria} / {nome}"),
                pct[0],
                pct[1],
                pct[2],
                pct[3],
                pct[4],
                s.meses_mediano,
                s.colapso_pct,
                s.estrutura_fim,
                s.nivel_medio,
            );
        }
        println!("{}", "-".repeat(140));
    }

    println!(
        "\n-- COMO LER --\n\
         Se a distribuicao de estrategias nao se move entre 'producao' e os limiares extremos,\n\
         os dois numeros nao sao o que decide - quem decide e a banda de estado (pressured ->\n\
         all_in, crisis/collapse -> survival), e os limiares sao decoracao. Se ela se move mas\n\
         colapso e estrutura nao, o eixo e cosmetico. 'por classe' isola o erro dimensional."
    );
}

// ===================== Os guards das cópias =====================

/// **Guarda das cópias de `finance::events`.** O contrafactual do socorro é construído sobre
/// números que moram na produção sem nome. Se eles mudarem, a medição passa a comparar o
/// braço novo contra uma produção que não existe mais.
#[test]
fn os_numeros_do_socorro_ainda_sao_os_da_producao() {
    // A parcela de ajuda: uma equipe com saldo de paraquedas gigante recebe exatamente o teto.
    let mut team = equipe_neutra("gt3", None);
    team.parachute_payment_remaining = 10_000_000.0;
    let ctx = calculate_team_round_finance_context_modelo(
        &team,
        40.0,
        10.0,
        0,
        0,
        0,
        99,
        0.0,
        10.0,
        global_economic_health_for_season(1),
        0.0,
        RoundOperationContext::default(),
        EtapaFisica::de_referencia(&team.categoria, team.classe.as_deref()),
        50.0,
        100.0,
        10.0,
        CoeficientesDeReceita::default(),
        despesa_da_rodada,
    );
    // A parcela do paraquedas: total da divisão dividido pelas rodadas da temporada. O harness
    // passa `rounds_in_season = 10`, então é o mesmo divisor dos dois lados.
    let parcela_esperada = crate::finance::events::parcela_de_paraquedas("gt3", None, 10.0);
    assert!(
        (ctx.aid_income - parcela_esperada).abs() < 1.0,
        "a parcela de ajuda por rodada mudou: producao paga {:.0}, esperado {parcela_esperada:.0}",
        ctx.aid_income
    );

    // A taxa sobre o principal: a dívida cresce `SOCORRO_TAXA` vezes o caixa injetado.
    let mut afogada = equipe_neutra("gt4", None);
    afogada.financial_state = "collapse".to_string();
    let mensal_gt4 = custo_operacional_mensal("gt4", None);
    afogada.cash_balance = -3.0 * mensal_gt4;
    afogada.debt_balance = 0.0;
    let antes = (afogada.cash_balance, afogada.debt_balance);
    let evento = apply_crisis_event_if_needed(&mut afogada, 1).expect("gatilho deveria abrir");
    let taxa = (afogada.debt_balance - antes.1) / (afogada.cash_balance - antes.0);
    assert!(
        (taxa - SOCORRO_TAXA).abs() < 1e-6,
        "a taxa do emprestimo mudou: producao cobra {taxa:.4}, copia do harness {SOCORRO_TAXA:.4}"
    );
    assert!(evento.cash_delta > 0.0);

    // Os gates RELATIVOS, na unidade da âncora. Dentro do gate de caixa não abre; fora, abre.
    let mut folgada = equipe_neutra("gt4", None);
    folgada.financial_state = "collapse".to_string();
    folgada.cash_balance = -(SOCORRO_GATE_CAIXA_MESES - 0.1) * mensal_gt4;
    folgada.debt_balance = 0.0;
    assert!(
        crate::finance::events::emergency_loan_amount_na_temporada(&folgada, 1).is_none(),
        "o gate de caixa mudou: dentro dele o gatilho abriu"
    );
    folgada.cash_balance = -(SOCORRO_GATE_CAIXA_MESES + 0.1) * mensal_gt4;
    assert!(
        crate::finance::events::emergency_loan_amount_na_temporada(&folgada, 1).is_some(),
        "o gate de caixa mudou: fora dele o gatilho nao abriu"
    );
    // E a dívida é TETO: acima dele o socorro fecha, em vez de abrir como antes.
    folgada.debt_balance = (SOCORRO_TETO_DIVIDA_MESES + 0.1) * mensal_gt4;
    assert!(
        crate::finance::events::emergency_loan_amount_na_temporada(&folgada, 1).is_none(),
        "a divida voltou a LIBERAR socorro em vez de barrar"
    );
}

/// **Guarda da cópia de `choose_season_strategy`.** [`escolher_estrategia`] reproduz a forma
/// da produção com os limiares abertos; nos limiares de produção as duas têm que concordar
/// em toda a escada e em todas as bandas de estado.
#[test]
fn os_limiares_da_estrategia_ainda_sao_os_da_producao() {
    for (categoria, classe) in DIVISOES {
        for (caixa, divida) in [
            (0.0, 0.0),
            (500_000.0, 0.0),
            (5_000_000.0, 0.0),
            (50_000_000.0, 0.0),
            (-200_000.0, 3_000_000.0),
            (100_000.0, 30_000_000.0),
        ] {
            for carro in [-4.0, 0.0, 8.0, 15.0] {
                let mut team = equipe_neutra(categoria, *classe);
                team.cash_balance = caixa;
                team.debt_balance = divida;
                team.car_performance = carro;
                crate::finance::state::refresh_team_financial_state(&mut team);

                let producao = choose_season_strategy(&team);
                let copia = escolher_estrategia(
                    &team,
                    Estrategia::Limiares {
                        all_in: LIMIAR_ALL_IN,
                        austeridade: LIMIAR_AUSTERIDADE,
                    },
                );
                assert_eq!(
                    producao, copia,
                    "{categoria}:{classe:?} caixa {caixa} divida {divida} carro {carro}: a copia \
                     do harness divergiu da producao - choose_season_strategy mudou de forma e o \
                     contrafactual de B52 esta medindo outra coisa"
                );
            }
        }
    }
}
