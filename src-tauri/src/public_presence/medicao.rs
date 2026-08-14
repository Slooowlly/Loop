//! Harness de MEDIÇÃO da distribuição de FAMA de um grid — régua, não teste de regressão.
//!
//! Irmão do `simulation::race::tests::medicao` e do `commands::race::tests::medicao_financeira`,
//! com o mesmo contrato: não asserta comportamento, IMPRIME o que a calibração atual produz.
//!
//! ```text
//! cargo test --lib medir_distribuicao_de_fama -- --ignored --nocapture
//! ```
//!
//! **A pergunta que ele existe para responder:** a fama tem espalhamento suficiente para
//! servir de chave de rateio? Medido num save real, a mídia dos pilotos tinha média entre
//! 36 e 48 por categoria com desvio de ~8 pontos — toda equipe do grid com a mesma presença
//! pública, e qualquer receita por cota de fama pagando igual para todo mundo.
//!
//! **O que ele replica.** A ordem exata do bloco de fama em `commands/race/financas.rs`:
//! papéis de chegada → [`crate::event_interest::compute_public_media_impacts`] →
//! [`crate::fame::apply_carisma_to_fame_delta`] → decaimento passivo de TODO o grid rumo
//! ao piso. O regime `Antes` refaz a regra anterior no mesmo laço (só vencedor/pole/pódio,
//! sem escala de categoria, piso global de 25), então os dois números saem do mesmo mundo
//! sorteado e a comparação é limpa.
//!
//! **O que ele NÃO replica, e por quê.**
//! - Não há motor de corrida: a ordem de chegada sai de carro + piloto com ruído. O que se
//!   mede aqui é a DINÂMICA DA FAMA, e a corrida tem harness próprio.
//! - Não há mercado nem promoção: ninguém troca de equipe nem sobe de categoria. Há, sim,
//!   RENOVAÇÃO: cada piloto tem uma carreira de 10–20 temporadas e depois um estreante
//!   ocupa a vaga — sem isso o mesmo nome acumularia 30 títulos e o topo saturaria por
//!   artefato do harness. No mundo real o piloto ainda sobe a escada carregando a fama, o
//!   que ESPALHA mais, não menos: o número daqui é conservador.
//! - Multi-classe é tratado como grid único, que é exatamente o que o código de fama faz:
//!   `RaceResult.winner_id` e `finish_position` são do campo inteiro, não da classe.
//! - Não há lesão nem incidente principal: são papéis raros e simétricos entre os regimes.

use std::collections::HashMap;

use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::constants::categories::get_category_config;
use crate::constants::scoring::get_points_for_position;
use crate::event_interest::models::{HeadlineStrength, InterestTier, RealizedEventInterest};
use crate::event_interest::{compute_public_media_impacts, RaceEventContext};
use crate::fame;
use crate::public_presence::atracao::{compute_team_audience_appeal, TeamAudienceInput};

/// Temporadas simuladas por categoria. 30 é uma carreira longa — tempo de sobra para o
/// decaimento fazer o seu estrago (era esse o problema).
const TEMPORADAS: u32 = 30;
/// Semente fixa: o relatório tem que ser o mesmo entre execuções para servir de régua.
const SEMENTE: u64 = 0xFA_1E_2026;
/// Categorias medidas — uma por degrau relevante da escada, da base ao topo.
const CATEGORIAS: [&str; 5] = ["mazda_rookie", "bmw_m2", "gt4", "gt3", "endurance"];

// ── Mundo sorteado ────────────────────────────────────────────────────────────

#[derive(Clone)]
struct PilotoSim {
    id: String,
    equipe: usize,
    skill: f64,
    carisma: f64,
    midia: f64,
    titulos: u32,
    vitorias: u32,
    temporadas_elite: u32,
    /// Pontos da temporada corrente (define o campeão) e das últimas etapas (forma).
    pontos_temporada: f64,
    pontos_recentes: Vec<f64>,
    /// Temporadas já disputadas e o tamanho sorteado da carreira. Sem renovação de grid
    /// um piloto acumularia 30 títulos e a régua mediria um mundo que não existe.
    temporadas_disputadas: u32,
    carreira_max: u32,
}

struct Grid {
    pilotos: Vec<PilotoSim>,
    carros: Vec<f64>,
    /// Pontos de construtores da temporada corrente, por equipe.
    pontos_equipe: Vec<f64>,
    titulos_equipe: Vec<i32>,
}

/// Normal(μ, σ) por Box-Muller — `rand` 0.8 sem `rand_distr`.
fn normal(rng: &mut StdRng, media: f64, desvio: f64) -> f64 {
    let u1: f64 = rng.gen_range(1e-9..1.0);
    let u2: f64 = rng.gen_range(0.0..1.0);
    media + desvio * (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// Um piloto novo para uma vaga. `estreante` distingue a reposição (fama de rookie,
/// 20–55, pedigree zerado) do sorteio inicial do mundo (26–74, como o gerador).
fn novo_piloto(
    rng: &mut StdRng,
    id: String,
    equipe: usize,
    carro: f64,
    estreante: bool,
) -> PilotoSim {
    PilotoSim {
        id,
        equipe,
        // Equipe boa atrai piloto bom — a correlação é o que dá hierarquia ao grid.
        skill: (carro * 0.55 + 22.0 + normal(rng, 0.0, 7.0)).clamp(20.0, 95.0),
        carisma: normal(rng, 50.0, 15.0).clamp(5.0, 95.0),
        midia: if estreante {
            rng.gen_range(20.0..55.0)
        } else {
            rng.gen_range(36.0..74.0)
        },
        titulos: 0,
        vitorias: 0,
        temporadas_elite: 0,
        pontos_temporada: 0.0,
        pontos_recentes: Vec::new(),
        temporadas_disputadas: 0,
        carreira_max: rng.gen_range(10..20),
    }
}

fn montar_grid(rng: &mut StdRng, categoria: &str) -> Grid {
    let cfg = get_category_config(categoria).expect("categoria do catálogo");
    let por_equipe = cfg.pilotos_por_equipe.max(1) as usize;
    let n_equipes = (cfg.grid_total as usize / por_equipe).max(2);

    let carros: Vec<f64> = (0..n_equipes).map(|_| rng.gen_range(40.0..90.0)).collect();
    let mut pilotos = Vec::new();
    for (equipe, carro) in carros.iter().enumerate() {
        for _slot in 0..por_equipe {
            let id = format!("P{:03}", pilotos.len() + 1);
            let mut p = novo_piloto(rng, id, equipe, *carro, false);
            // Na largada o grid não é todo de estreantes: espalha o tempo de casa.
            p.temporadas_disputadas = rng.gen_range(0..p.carreira_max);
            pilotos.push(p);
        }
    }
    Grid {
        pontos_equipe: vec![0.0; n_equipes],
        titulos_equipe: vec![0; n_equipes],
        carros,
        pilotos,
    }
}

// ── Regimes ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Regime {
    /// Regra anterior: ganho só para vencedor/pole/2 pódios, sem escala de categoria,
    /// piso de decaimento global de 25 para todo mundo.
    Antes,
    /// Regra atual: faixas de chegada até P10, ganho escalado pelo tier, piso pessoal.
    Depois,
}

fn realizado(tier: InterestTier) -> RealizedEventInterest {
    RealizedEventInterest {
        expected_display_value: 0,
        expected_tier: InterestTier::Moderado,
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

/// Distribuição de interesse de evento ao longo de um calendário: a maioria é rotina,
/// poucas etapas são o evento grande do ano.
fn sortear_interesse(rng: &mut StdRng) -> InterestTier {
    match rng.gen_range(0.0..1.0) {
        x if x < 0.30 => InterestTier::Baixo,
        x if x < 0.70 => InterestTier::Moderado,
        x if x < 0.90 => InterestTier::Alto,
        x if x < 0.97 => InterestTier::MuitoAlto,
        _ => InterestTier::EventoPrincipal,
    }
}

fn multiplicador_de_interesse(tier: &InterestTier) -> f64 {
    match tier {
        InterestTier::Baixo => 0.3,
        InterestTier::Moderado => 0.7,
        InterestTier::Alto => 1.0,
        InterestTier::MuitoAlto => 1.5,
        InterestTier::EventoPrincipal => 2.5,
    }
}

/// Ganhos de fama do regime ANTERIOR, refeitos aqui porque o código deles não existe
/// mais: vencedor +3, pole +1,5 (se ≠ vencedor), P2/P3 +1, sem escala de categoria.
fn impactos_antes(ordem: &[usize], pole: usize, tier: &InterestTier) -> HashMap<usize, f64> {
    let mult = multiplicador_de_interesse(tier);
    let mut out: HashMap<usize, f64> = HashMap::new();
    if let Some(&vencedor) = ordem.first() {
        *out.entry(vencedor).or_insert(0.0) += 3.0 * mult;
        if pole != vencedor {
            *out.entry(pole).or_insert(0.0) += 1.5 * mult;
        }
    }
    for &idx in ordem.iter().skip(1).take(2) {
        *out.entry(idx).or_insert(0.0) += 1.0 * mult;
    }
    out
}

// ── Simulação ─────────────────────────────────────────────────────────────────

fn simular(categoria: &str, regime: Regime) -> Grid {
    let cfg = get_category_config(categoria).expect("categoria do catálogo");
    let tier_categoria = cfg.tier;
    let e_endurance = categoria == "endurance";
    let etapas = cfg.corridas_por_temporada.max(1) as u32;
    // Mesma semente nos dois regimes: o mundo sorteado é literalmente o mesmo.
    let mut rng = StdRng::seed_from_u64(SEMENTE ^ categoria.len() as u64);
    let mut grid = montar_grid(&mut rng, categoria);

    for _temporada in 0..TEMPORADAS {
        for p in grid.pilotos.iter_mut() {
            p.pontos_temporada = 0.0;
        }
        for e in grid.pontos_equipe.iter_mut() {
            *e = 0.0;
        }

        for _etapa in 0..etapas {
            // Ritmo do fim de semana: carro + piloto + ruído.
            let mut ritmo: Vec<(usize, f64)> = grid
                .pilotos
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let carro = grid.carros[p.equipe];
                    (
                        i,
                        0.55 * p.skill + 0.45 * carro + normal(&mut rng, 0.0, 6.0),
                    )
                })
                .collect();
            ritmo.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let ordem: Vec<usize> = ritmo.iter().map(|(i, _)| *i).collect();
            // A pole tem o seu próprio sorteio (quem voa numa volta nem sempre vence).
            let mut quali: Vec<(usize, f64)> = grid
                .pilotos
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let carro = grid.carros[p.equipe];
                    (
                        i,
                        0.55 * p.skill + 0.45 * carro + normal(&mut rng, 0.0, 7.0),
                    )
                })
                .collect();
            quali.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let pole = quali[0].0;

            let tier = sortear_interesse(&mut rng);

            // Pontos e forma.
            for (pos0, &idx) in ordem.iter().enumerate() {
                let pontos = get_points_for_position((pos0 + 1).min(255) as u8, e_endurance) as f64;
                let equipe = grid.pilotos[idx].equipe;
                let p = &mut grid.pilotos[idx];
                p.pontos_temporada += pontos;
                p.pontos_recentes.push(pontos);
                if p.pontos_recentes.len() > 3 {
                    p.pontos_recentes.remove(0);
                }
                grid.pontos_equipe[equipe] += pontos;
            }
            grid.pilotos[ordem[0]].vitorias += 1;

            // Ganhos de fama.
            let ganhos: HashMap<usize, f64> = match regime {
                Regime::Antes => impactos_antes(&ordem, pole, &tier),
                Regime::Depois => {
                    let id_de = |i: usize| grid.pilotos[i].id.clone();
                    let vencedor = id_de(ordem[0]);
                    let pole_id = id_de(pole);
                    let faixa = |de: usize, ate: usize| -> Vec<String> {
                        ordem
                            .iter()
                            .skip(de - 1)
                            .take(ate + 1 - de)
                            .map(|&i| id_de(i))
                            .collect()
                    };
                    let visivel = fame::fame_visibility_last_position(ordem.len()) as usize;
                    let podio = faixa(2, 3);
                    let top5 = faixa(4, 5.min(visivel));
                    let top10 = faixa(6, visivel);
                    // Os `Vec<String>` acima vivem até o fim do bloco; as fatias de &str
                    // são só a forma que o contexto pede.
                    let podio_r: Vec<&str> = podio.iter().map(|s| s.as_str()).collect();
                    let top5_r: Vec<&str> = top5.iter().map(|s| s.as_str()).collect();
                    let top10_r: Vec<&str> = top10.iter().map(|s| s.as_str()).collect();
                    let ctx = RaceEventContext {
                        winner_id: &vencedor,
                        pole_sitter_id: &pole_id,
                        podium_ids: &podio_r,
                        top5_ids: &top5_r,
                        top10_ids: &top10_r,
                        main_incident_pilot_id: None,
                        excluded_driver_id: "",
                        category_tier: tier_categoria,
                    };
                    let impactos = compute_public_media_impacts(&ctx, &[], &realizado(tier));
                    let por_id: HashMap<&str, usize> = grid
                        .pilotos
                        .iter()
                        .enumerate()
                        .map(|(i, p)| (p.id.as_str(), i))
                        .collect();
                    impactos
                        .iter()
                        .filter_map(|imp| {
                            por_id.get(imp.driver_id.as_str()).map(|&i| (i, imp.delta))
                        })
                        .collect()
                }
            };
            for (idx, bruto) in ganhos {
                let p = &mut grid.pilotos[idx];
                let delta = fame::apply_carisma_to_fame_delta(bruto, p.carisma);
                p.midia = fame::apply_fame_gain(p.midia, delta);
            }

            // Decaimento passivo de TODO o grid que correu.
            for p in grid.pilotos.iter_mut() {
                let piso = match regime {
                    Regime::Antes => fame::FAME_DECAY_FLOOR,
                    Regime::Depois => {
                        fame::personal_fame_floor(p.titulos, p.vitorias, p.temporadas_elite)
                    }
                };
                p.midia =
                    fame::decay_fame_toward(p.midia, piso, fame::FAME_DECAY_BASE_RATE, p.carisma);
            }
        }

        // Fim de temporada: título de pilotos e de construtores, e um ano a mais na elite.
        let campeao = (0..grid.pilotos.len())
            .max_by(|&a, &b| {
                grid.pilotos[a]
                    .pontos_temporada
                    .partial_cmp(&grid.pilotos[b].pontos_temporada)
                    .unwrap()
            })
            .unwrap();
        grid.pilotos[campeao].titulos += 1;
        let campea = (0..grid.pontos_equipe.len())
            .max_by(|&a, &b| {
                grid.pontos_equipe[a]
                    .partial_cmp(&grid.pontos_equipe[b])
                    .unwrap()
            })
            .unwrap();
        grid.titulos_equipe[campea] += 1;
        for p in grid.pilotos.iter_mut() {
            p.temporadas_disputadas += 1;
            if tier_categoria >= fame::FAME_ELITE_TIER_MIN {
                p.temporadas_elite += 1;
            }
        }

        // Renovação do grid: quem cumpriu a carreira sai e um estreante ocupa a vaga.
        // Sem isto o mesmo piloto acumula 30 títulos e a medição descreve um mundo que
        // não existe (no jogo o piloto envelhece, é promovido ou se aposenta).
        for i in 0..grid.pilotos.len() {
            if grid.pilotos[i].temporadas_disputadas >= grid.pilotos[i].carreira_max {
                let equipe = grid.pilotos[i].equipe;
                let carro = grid.carros[equipe];
                let id = grid.pilotos[i].id.clone();
                grid.pilotos[i] = novo_piloto(&mut rng, id, equipe, carro, true);
            }
        }
    }
    grid
}

// ── O CAMINHO DO JOGADOR ATÉ ÍDOLO ────────────────────────────────────────────
//
// A pergunta certa não é que fatia da população é Ídolo — a raridade entre a IA pode
// ser exatamente o que torna a faixa valiosa. A pergunta é se a faixa é ALCANÇÁVEL para
// quem joga: um jogador que faz carreira longa e vencedora chega lá?
//
// Este bloco simula a trajetória do JOGADOR, não do mundo: a escada do
// `apply_post_race_fame` player-facing (+3 vitória, +2 pódio, +1 top-5, +0,3 top-10,
// −1 resto, −2 DNF), modulada pelo carisma, escalada pelo tier da categoria e puxada
// pelo decaimento rumo ao piso pessoal que os títulos dele constroem.

/// Os dois regimes da escada player-facing.
///
/// `Assimetrica` é o que estava em produção: escada própria (+3 / +2 / +1 / +0,3 / −1 / −2)
/// modulada por `RealizedEventInterest::media_delta_modifier`. `Simetrica` é a escada do
/// mundo (`fame::fame_finish_base_delta`) modulada pelo mesmo `fame_event_interest_mult` que
/// os 27 outros carros do grid recebem.
#[derive(Clone, Copy, PartialEq)]
enum RegimeDoJogador {
    Assimetrica,
    Simetrica,
}

/// Ganho-base de fama do JOGADOR por posição, no regime antigo. Só existe para a
/// comparação — a produção usa `fame::fame_finish_base_delta`.
fn ganho_base_antigo(posicao: u32, dnf: bool, ultima_visivel: u32) -> f64 {
    if dnf {
        return -2.0;
    }
    match posicao {
        1 => 3.0,
        2..=3 => 2.0,
        4..=5 => 1.0,
        p if p <= ultima_visivel => 0.3,
        _ => -1.0,
    }
}

/// O delta bruto de fama do jogador numa corrida, antes do carisma, nos dois regimes.
/// O `realized` é o MESMO objeto nos dois — é o ponto da medição: a divergência não vem
/// do evento, vem de como cada lado o traduz.
fn delta_bruto_do_jogador(
    regime: RegimeDoJogador,
    realized: &RealizedEventInterest,
    posicao: u32,
    dnf: bool,
    ultima_visivel: u32,
    tier_categoria: u8,
) -> f64 {
    let escala_categoria = fame::fame_category_tier_mult(tier_categoria);
    match regime {
        RegimeDoJogador::Assimetrica => {
            ganho_base_antigo(posicao, dnf, ultima_visivel)
                * realized.media_delta_modifier as f64
                * escala_categoria
        }
        RegimeDoJogador::Simetrica => {
            fame::fame_finish_base_delta(posicao as i32, dnf, ultima_visivel as i32)
                * crate::event_interest::fame_event_interest_mult(&realized.final_tier)
                * escala_categoria
        }
    }
}

/// O que um piloto de IA na MESMA posição, na MESMA corrida, recebe de bruto.
/// É o lado do mundo, e ele não muda entre os regimes — é a régua.
fn delta_bruto_da_ia(
    realized: &RealizedEventInterest,
    posicao: u32,
    dnf: bool,
    ultima_visivel: u32,
    tier_categoria: u8,
) -> f64 {
    if dnf || posicao > ultima_visivel {
        // O mundo não pune quem não apareceu: a queda dele é só o decaimento passivo.
        return 0.0;
    }
    fame::fame_finish_base_delta(posicao as i32, false, ultima_visivel as i32)
        * crate::event_interest::fame_event_interest_mult(&realized.final_tier)
        * fame::fame_category_tier_mult(tier_categoria)
}

/// O interesse REALIZADO de uma corrida do jogador, pela cadeia de produção inteira
/// (`calculate_expected_event_interest` → `calculate_realized_event_interest`). Nada de
/// interesse "médio" chutado: a assimetria é justamente uma função do `final_score`, então
/// chutá-lo seria chutar a resposta.
fn interesse_da_corrida(
    categoria: &str,
    rodada: i32,
    total_rodadas: i32,
    posicao: u32,
    dnf: bool,
    fama_atual: f64,
) -> RealizedEventInterest {
    use crate::event_interest::calculator::{
        calculate_expected_event_interest, calculate_realized_event_interest,
    };
    use crate::event_interest::models::EventInterestContext;
    use crate::models::enums::{SeasonPhase, ThematicSlot};

    let ctx = EventInterestContext {
        categoria: categoria.to_string(),
        season_phase: SeasonPhase::BlocoRegular,
        rodada,
        total_rodadas,
        week_of_year: 10 + rodada,
        track_id: 1,
        track_name: "Pista".to_string(),
        is_player_event: true,
        player_championship_position: Some(2),
        player_media: Some(fama_atual as f32),
        championship_gap_to_leader: Some(15),
        is_title_decider_candidate: false,
        thematic_slot: ThematicSlot::RodadaRegular,
    };
    let expected = calculate_expected_event_interest(&ctx);
    calculate_realized_event_interest(
        &expected,
        &ctx,
        Some(posicao as i32),
        // Larga da mesma posição em que termina: sem remontada fabricada, o termo
        // posicional some e o que sobra é o resultado puro.
        Some(posicao as i32),
        posicao == 1 && !dnf,
        posicao <= 3 && !dnf,
        dnf,
        false,
    )
}

/// Um degrau da carreira do jogador: em que categoria ele está e por quantas temporadas.
struct DegrauDaCarreira {
    categoria: &'static str,
    temporadas: u32,
}

/// Perfil de resultado do jogador numa temporada, como distribuição de chegadas.
#[derive(Clone, Copy)]
struct Perfil {
    nome: &'static str,
    /// Fração das corridas em que vence, sobe ao pódio, faz top-5 e top-10.
    vitorias: f64,
    podios: f64,
    top5: f64,
    top10: f64,
    /// Fração de DNF.
    dnf: f64,
    /// Ganha o título da categoria a cada N temporadas (0 = nunca).
    titulo_a_cada: u32,
}

const CARREIRA: [DegrauDaCarreira; 5] = [
    DegrauDaCarreira {
        categoria: "mazda_rookie",
        temporadas: 2,
    },
    DegrauDaCarreira {
        categoria: "mazda_amador",
        temporadas: 3,
    },
    DegrauDaCarreira {
        categoria: "gt4",
        temporadas: 3,
    },
    DegrauDaCarreira {
        categoria: "gt3",
        temporadas: 5,
    },
    DegrauDaCarreira {
        categoria: "endurance",
        temporadas: 7,
    },
];

const PERFIS: [Perfil; 3] = [
    Perfil {
        nome: "Dominante",
        vitorias: 0.45,
        podios: 0.30,
        top5: 0.15,
        top10: 0.07,
        dnf: 0.03,
        titulo_a_cada: 1,
    },
    Perfil {
        nome: "Vencedor",
        vitorias: 0.22,
        podios: 0.30,
        top5: 0.25,
        top10: 0.15,
        dnf: 0.08,
        titulo_a_cada: 3,
    },
    Perfil {
        nome: "Sólido",
        vitorias: 0.06,
        podios: 0.16,
        top5: 0.28,
        top10: 0.35,
        dnf: 0.15,
        titulo_a_cada: 0,
    },
];

/// Estado do jogador ao longo da carreira.
struct Jogador {
    midia: f64,
    carisma: f64,
    titulos: u32,
    vitorias: u32,
    temporadas_elite: u32,
}

/// Roda a carreira inteira e devolve (fama por temporada, temporada em que cruzou 87).
fn simular_carreira(
    perfil: &Perfil,
    carisma: f64,
    regime: RegimeDoJogador,
) -> (Vec<(u32, &'static str, f64)>, Option<u32>) {
    simular_carreira_com(
        perfil,
        carisma,
        regime,
        fame::FAME_SATURATION_KNEE,
        fame::FAME_SATURATION_EXP,
    )
}

/// Variante com a curva de saturação explícita — é o que a varredura usa.
fn simular_carreira_com(
    perfil: &Perfil,
    carisma: f64,
    regime: RegimeDoJogador,
    joelho: f64,
    expoente: f64,
) -> (Vec<(u32, &'static str, f64)>, Option<u32>) {
    let mut j = Jogador {
        // Estreante: a faixa de geração de rookie do próprio jogo.
        midia: 35.0,
        carisma,
        titulos: 0,
        vitorias: 0,
        temporadas_elite: 0,
    };
    let mut linha = Vec::new();
    let mut virou_idolo = None;
    let mut temporada = 0u32;

    for degrau in CARREIRA.iter() {
        let cfg = get_category_config(degrau.categoria).expect("categoria");
        let tier = cfg.tier;
        let etapas = cfg.corridas_por_temporada.max(1) as u32;
        let grid = cfg.grid_total.max(2) as usize;
        let ultima_visivel = fame::fame_visibility_last_position(grid) as u32;

        for _ in 0..degrau.temporadas {
            temporada += 1;
            // As corridas da temporada, distribuídas pelo perfil. Determinístico: a
            // pergunta é sobre o CAMINHO, e sorteio só acrescentaria ruído a ele.
            let n = etapas as f64;
            let corridas: Vec<(u32, bool)> = {
                let mut v = Vec::new();
                let mut empurra = |quantas: f64, posicao: u32, dnf: bool| {
                    for _ in 0..(quantas * n).round() as u32 {
                        v.push((posicao, dnf));
                    }
                };
                empurra(perfil.dnf, 99, true);
                empurra(perfil.vitorias, 1, false);
                empurra(perfil.podios, 2, false);
                empurra(perfil.top5, 4, false);
                empurra(perfil.top10, ultima_visivel, false);
                // O que sobrar é resto de grid.
                while (v.len() as u32) < etapas {
                    v.push((ultima_visivel + 1, false));
                }
                v.truncate(etapas as usize);
                v
            };

            for (rodada, (posicao, dnf)) in corridas.into_iter().enumerate() {
                let realized = interesse_da_corrida(
                    degrau.categoria,
                    rodada as i32 + 1,
                    etapas as i32,
                    posicao,
                    dnf,
                    j.midia,
                );
                let bruto =
                    delta_bruto_do_jogador(regime, &realized, posicao, dnf, ultima_visivel, tier);
                let delta = fame::apply_carisma_to_fame_delta(bruto, j.carisma);
                j.midia = fame::apply_fame_gain_com(j.midia, delta, joelho, expoente);
                if posicao == 1 && !dnf {
                    j.vitorias += 1;
                }
                // Decaimento passivo, com o piso pessoal que a carreira dele construiu.
                let piso = fame::personal_fame_floor(j.titulos, j.vitorias, j.temporadas_elite);
                j.midia =
                    fame::decay_fame_toward(j.midia, piso, fame::FAME_DECAY_BASE_RATE, j.carisma);
            }

            if perfil.titulo_a_cada > 0 && temporada % perfil.titulo_a_cada == 0 {
                j.titulos += 1;
            }
            if tier >= fame::FAME_ELITE_TIER_MIN {
                j.temporadas_elite += 1;
            }
            linha.push((temporada, degrau.categoria, j.midia));
            if virou_idolo.is_none() && j.midia > 87.0 {
                virou_idolo = Some(temporada);
            }
        }
    }
    (linha, virou_idolo)
}

/// **A pergunta:** na MESMA corrida, na MESMA posição, o jogador recebia mais fama que um
/// piloto de IA? Era inferência de leitura de código; aqui ela vira número.
///
/// Nada é sorteado e nada é chutado: o `RealizedEventInterest` sai da cadeia de produção
/// (`calculate_expected_event_interest` → `calculate_realized_event_interest`) e é o mesmo
/// objeto para os dois lados. O carisma e a escala de categoria também são idênticos nos
/// dois, então saem da conta — o que sobra é exatamente a divergência.
#[test]
#[ignore = "harness de medição — roda com --ignored --nocapture"]
fn medir_simetria_da_escada_de_fama() {
    println!("\n=== SIMETRIA DA ESCADA DE FAMA: jogador × IA, mesma corrida ===\n");
    println!(
        "Cada linha é a MESMA posição de chegada na MESMA corrida. O interesse do evento\n\
         vem da cadeia de produção; o carisma e a escala de categoria são iguais nos dois\n\
         lados e se cancelam.\n"
    );

    for (categoria, rodada) in [
        ("mazda_rookie", 3),
        ("gt4", 5),
        ("gt3", 7),
        ("endurance", 4),
    ] {
        let cfg = get_category_config(categoria).expect("categoria");
        let tier = cfg.tier;
        let etapas = cfg.corridas_por_temporada.max(1);
        let ultima = fame::fame_visibility_last_position(cfg.grid_total.max(2) as usize) as u32;

        println!(
            "── {categoria} (tier {tier}, grid {}, visível até P{ultima}) ──",
            cfg.grid_total
        );
        println!(
            "{:<10} {:>12} {:>10} {:>10} {:>10} {:>9}",
            "chegada", "interesse", "antes", "agora", "IA", "antes÷IA"
        );
        println!("{}", "─".repeat(64));

        let mut posicoes: Vec<(String, u32, bool)> = vec![
            ("P1 (vitória)".to_string(), 1, false),
            ("P2".to_string(), 2, false),
            ("P4".to_string(), 4, false),
            (format!("P{ultima}"), ultima, false),
            (format!("P{}", ultima + 4), ultima + 4, false),
        ];
        posicoes.push(("DNF".to_string(), 99, true));

        for (rotulo, posicao, dnf) in posicoes {
            let realized =
                interesse_da_corrida(categoria, rodada, etapas as i32, posicao, dnf, 60.0);
            let antes = delta_bruto_do_jogador(
                RegimeDoJogador::Assimetrica,
                &realized,
                posicao,
                dnf,
                ultima,
                tier,
            );
            let agora = delta_bruto_do_jogador(
                RegimeDoJogador::Simetrica,
                &realized,
                posicao,
                dnf,
                ultima,
                tier,
            );
            let ia = delta_bruto_da_ia(&realized, posicao, dnf, ultima, tier);
            let razao = if ia.abs() > 1e-9 {
                format!("{:.2}×", antes / ia)
            } else {
                "—".to_string()
            };
            println!(
                "{:<10} {:>12} {:>10.3} {:>10.3} {:>10.3} {:>9}",
                rotulo,
                format!("{:?}", realized.final_tier),
                antes,
                agora,
                ia,
                razao
            );
        }
        println!();
    }

    // A conta que o jogador sente: uma TEMPORADA inteira de um perfil vencedor.
    println!("── Fama bruta ganha numa temporada de GT3, perfil 'Vencedor' ──\n");
    let cfg = get_category_config("gt3").expect("gt3");
    let etapas = cfg.corridas_por_temporada.max(1) as u32;
    let ultima = fame::fame_visibility_last_position(cfg.grid_total.max(2) as usize) as u32;
    let perfil = &PERFIS[1];
    let mut soma = [0.0f64; 3]; // antes · agora · IA
    for i in 0..etapas {
        let (posicao, dnf) = {
            let f = i as f64 / etapas as f64;
            if f < perfil.dnf {
                (99, true)
            } else if f < perfil.dnf + perfil.vitorias {
                (1, false)
            } else if f < perfil.dnf + perfil.vitorias + perfil.podios {
                (2, false)
            } else if f < perfil.dnf + perfil.vitorias + perfil.podios + perfil.top5 {
                (4, false)
            } else {
                (ultima, false)
            }
        };
        let realized = interesse_da_corrida("gt3", i as i32 + 1, etapas as i32, posicao, dnf, 60.0);
        soma[0] += delta_bruto_do_jogador(
            RegimeDoJogador::Assimetrica,
            &realized,
            posicao,
            dnf,
            ultima,
            cfg.tier,
        );
        soma[1] += delta_bruto_do_jogador(
            RegimeDoJogador::Simetrica,
            &realized,
            posicao,
            dnf,
            ultima,
            cfg.tier,
        );
        soma[2] += delta_bruto_da_ia(&realized, posicao, dnf, ultima, cfg.tier);
    }
    println!(
        "  antes {:.1} · agora {:.1} · um piloto de IA com os mesmos resultados {:.1}",
        soma[0], soma[1], soma[2]
    );
    println!(
        "  o jogador recebia {:.2}× o que o mundo recebe pelos mesmos resultados; agora {:.2}×",
        soma[0] / soma[2].max(1e-9),
        soma[1] / soma[2].max(1e-9)
    );
    println!();
}

#[test]
#[ignore = "harness de medição — roda com --ignored --nocapture"]
fn medir_caminho_do_jogador_ate_idolo() {
    println!("\n=== O CAMINHO DO JOGADOR ATÉ ÍDOLO (>87) ===\n");
    println!(
        "Carreira de 20 temporadas subindo a escada: 2 Rookie · 3 Amador · 3 GT4 · 5 GT3 · 7 Endurance\n"
    );

    for carisma in [30.0, 50.0, 80.0] {
        println!("── Carisma {carisma:.0} ──────────────────────────────────────");
        println!(
            "{:<12} {:>22} {:>22}",
            "Perfil", "ASSIMÉTRICA (antes)", "SIMÉTRICA (agora)"
        );
        println!(
            "{:<12} {:>10} {:>11} {:>10} {:>11}",
            "", "fama fim", "Ídolo em", "fama fim", "Ídolo em"
        );
        println!("{}", "─".repeat(58));
        for perfil in PERFIS.iter() {
            let mut celulas = Vec::new();
            for regime in [RegimeDoJogador::Assimetrica, RegimeDoJogador::Simetrica] {
                let (linha, idolo) = simular_carreira(perfil, carisma, regime);
                let fim = linha.last().map(|(_, _, f)| *f).unwrap_or(0.0);
                celulas.push((
                    fim,
                    match idolo {
                        Some(t) => format!("T{t}"),
                        None => "não".to_string(),
                    },
                ));
            }
            println!(
                "{:<12} {:>10.1} {:>11} {:>10.1} {:>11}",
                perfil.nome, celulas[0].0, celulas[0].1, celulas[1].0, celulas[1].1
            );
        }
        println!();
    }

    // ── A varredura do retorno decrescente ────────────────────────────────────────
    // O que se procura: uma curva em que o Ídolo seja o FIM de uma carreira vencedora,
    // não o meio dela, e em que a fama ainda esteja se movendo na temporada 20.
    println!("=== VARREDURA DA CURVA DE SATURAÇÃO (perfil 'Vencedor', carisma 50) ===\n");
    println!(
        "{:<18} {:>10} {:>10} {:>12} {:>16}",
        "joelho · expoente", "Ídolo em", "fama T20", "fama T10", "ainda sobe em T20?"
    );
    println!("{}", "─".repeat(70));
    for (joelho, expoente) in [
        (100.0, 1.0), // sem saturação: o comportamento de hoje
        (70.0, 1.0),
        (70.0, 1.6),
        (70.0, 2.2),
        (80.0, 1.6),
        (60.0, 1.6),
    ] {
        let (linha, idolo) = simular_carreira_com(
            &PERFIS[1],
            50.0,
            RegimeDoJogador::Simetrica,
            joelho,
            expoente,
        );
        let fim = linha.last().map(|(_, _, f)| *f).unwrap_or(0.0);
        let t10 = linha.get(9).map(|(_, _, f)| *f).unwrap_or(0.0);
        let t19 = linha.get(18).map(|(_, _, f)| *f).unwrap_or(0.0);
        let rotulo = if joelho >= 100.0 {
            "sem saturação".to_string()
        } else {
            format!("{joelho:.0} · {expoente:.1}")
        };
        println!(
            "{:<18} {:>10} {:>10.1} {:>12.1} {:>16}",
            rotulo,
            match idolo {
                Some(t) => format!("T{t}"),
                None => "nunca".to_string(),
            },
            fim,
            t10,
            format!("+{:.2}/temp", fim - t19)
        );
    }
    println!();

    // A trajetória detalhada do caso que interessa: o jogador vencedor de carisma médio.
    println!("── Trajetória do perfil 'Vencedor', carisma 50 (regime simétrico) ──");
    let (linha, _) = simular_carreira(&PERFIS[1], 50.0, RegimeDoJogador::Simetrica);
    for (t, categoria, fama) in &linha {
        let faixa = NIVEIS
            .iter()
            .find(|(_, teto)| fama <= teto)
            .map(|(n, _)| *n)
            .unwrap_or("Ídolo");
        println!("  T{t:<3} {categoria:<16} {fama:>6.1}  {faixa}");
    }
    println!();
}

// ── Estatística e relatório ───────────────────────────────────────────────────

struct Resumo {
    media: f64,
    desvio: f64,
    minimo: f64,
    maximo: f64,
}

fn resumir(valores: &[f64]) -> Resumo {
    let n = valores.len().max(1) as f64;
    let media = valores.iter().sum::<f64>() / n;
    let var = valores.iter().map(|v| (v - media).powi(2)).sum::<f64>() / n;
    Resumo {
        media,
        desvio: var.sqrt(),
        minimo: valores.iter().cloned().fold(f64::INFINITY, f64::min),
        maximo: valores.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    }
}

/// Os 6 níveis da ficha, na mesma fatia que `fame::fame_commercial_units` e a UI usam.
/// Nada aqui muda os limiares — a quebra existe para MEDIR o deslocamento da população
/// entre eles, que é o que o mercado, o salário e o gerador de notícias enxergam.
const NIVEIS: [(&str, f64); 6] = [
    ("Anônimo", 15.0),
    ("Discreto", 30.0),
    ("Conhecido", 50.0),
    ("Nome forte", 70.0),
    ("Estrela", 87.0),
    ("Ídolo", 100.0),
];

fn contar_niveis(midias: &[f64]) -> [usize; 6] {
    let mut out = [0usize; 6];
    for &m in midias {
        let i = NIVEIS
            .iter()
            .position(|(_, teto)| m <= *teto)
            .unwrap_or(NIVEIS.len() - 1);
        out[i] += 1;
    }
    out
}

/// Atração de público de cada equipe do grid, no estado final da simulação.
fn atracao_por_equipe(grid: &Grid) -> Vec<f64> {
    let n_equipes = grid.carros.len();
    // Classificação de construtores da última temporada.
    let mut ordem: Vec<usize> = (0..n_equipes).collect();
    ordem.sort_by(|&a, &b| {
        grid.pontos_equipe[b]
            .partial_cmp(&grid.pontos_equipe[a])
            .unwrap()
    });
    let mut posicao = vec![0u32; n_equipes];
    for (i, &e) in ordem.iter().enumerate() {
        posicao[e] = (i + 1) as u32;
    }
    let pontos_max_recentes = grid
        .pilotos
        .iter()
        .flat_map(|p| p.pontos_recentes.iter())
        .cloned()
        .fold(1.0_f64, f64::max);

    (0..n_equipes)
        .map(|e| {
            let medias: Vec<f64> = grid
                .pilotos
                .iter()
                .filter(|p| p.equipe == e)
                .map(|p| p.midia)
                .collect();
            let recentes: Vec<f64> = grid
                .pilotos
                .iter()
                .filter(|p| p.equipe == e)
                .flat_map(|p| p.pontos_recentes.iter().cloned())
                .collect();
            let forma = if recentes.is_empty() {
                0.5
            } else {
                recentes.iter().sum::<f64>() / (recentes.len() as f64 * pontos_max_recentes)
            };
            compute_team_audience_appeal(&TeamAudienceInput {
                lineup_medias: &medias,
                posicao_campeonato: posicao[e],
                equipes_na_categoria: n_equipes as u32,
                forma_recente: forma,
                // Uma equipe da casa por etapa: a de índice 0 nesta fotografia.
                equipe_local: e == 0,
                titulos_da_equipe: grid.titulos_equipe[e],
            })
        })
        .collect()
}

#[test]
#[ignore = "harness de medição — roda com --ignored --nocapture"]
fn medir_distribuicao_de_fama() {
    println!("\n=== DISTRIBUIÇÃO DE FAMA — {TEMPORADAS} temporadas por categoria ===\n");
    println!(
        "{:<16} {:>8} {:>8} {:>8} {:>8}   {:>8} {:>8} {:>8} {:>8}",
        "Categoria", "μ antes", "σ antes", "mín", "máx", "μ depois", "σ depois", "mín", "máx"
    );
    println!("{}", "-".repeat(92));

    let mut grids_depois: Vec<(&str, Grid)> = Vec::new();
    let mut niveis_antes = [0usize; 6];
    let mut niveis_depois = [0usize; 6];
    // Presença média por equipe nos dois regimes: é a ENTRADA do termo de fama do
    // patrocínio (`presença × base × FAME_SPONSORSHIP_COEFF`), então a razão entre elas
    // é exatamente o quanto aquele canal passou a render. Medido, não recalibrado.
    let mut presenca_antes: Vec<f64> = Vec::new();
    let mut presenca_depois: Vec<f64> = Vec::new();
    for categoria in CATEGORIAS {
        let antes = simular(categoria, Regime::Antes);
        let depois = simular(categoria, Regime::Depois);
        for (grid, acc) in [
            (&antes, &mut presenca_antes),
            (&depois, &mut presenca_depois),
        ] {
            for equipe in 0..grid.carros.len() {
                let medias: Vec<f64> = grid
                    .pilotos
                    .iter()
                    .filter(|p| p.equipe == equipe)
                    .map(|p| p.midia)
                    .collect();
                acc.push(crate::public_presence::team::derive_team_public_presence(
                    &medias,
                ));
            }
        }
        let ma: Vec<f64> = antes.pilotos.iter().map(|p| p.midia).collect();
        let md: Vec<f64> = depois.pilotos.iter().map(|p| p.midia).collect();
        let ra = resumir(&ma);
        let rd = resumir(&md);
        for (acc, n) in niveis_antes.iter_mut().zip(contar_niveis(&ma)) {
            *acc += n;
        }
        for (acc, n) in niveis_depois.iter_mut().zip(contar_niveis(&md)) {
            *acc += n;
        }
        println!(
            "{:<16} {:>8.1} {:>8.1} {:>8.1} {:>8.1}   {:>8.1} {:>8.1} {:>8.1} {:>8.1}",
            categoria,
            ra.media,
            ra.desvio,
            ra.minimo,
            ra.maximo,
            rd.media,
            rd.desvio,
            rd.minimo,
            rd.maximo
        );
        grids_depois.push((categoria, depois));
    }

    println!("\nAlvo: σ dentro da categoria ≥ 15 (era ~8 no save medido).\n");

    // Os 6 níveis NÃO mudaram — mas a população se move entre eles, e é isso que o
    // mercado (`market/poaching`, `finance/salary`) e as notícias enxergam.
    println!("=== POPULAÇÃO POR NÍVEL DA FICHA (todas as categorias somadas) ===\n");
    let total: usize = niveis_antes.iter().sum::<usize>().max(1);
    println!(
        "{:<14} {:>10} {:>10} {:>10} {:>10}",
        "Nível", "antes", "%", "depois", "%"
    );
    println!("{}", "-".repeat(58));
    for (i, (nome, _)) in NIVEIS.iter().enumerate() {
        println!(
            "{:<14} {:>10} {:>9.1}% {:>10} {:>9.1}%",
            nome,
            niveis_antes[i],
            100.0 * niveis_antes[i] as f64 / total as f64,
            niveis_depois[i],
            100.0 * niveis_depois[i] as f64 / total as f64
        );
    }
    println!();

    println!("=== PISO PESSOAL — o que a carreira sedimentou (regime novo) ===\n");
    println!(
        "{:<16} {:>10} {:>10} {:>10} {:>12}",
        "Categoria", "piso mín", "piso méd", "piso máx", "títulos máx"
    );
    println!("{}", "-".repeat(62));
    for (categoria, grid) in &grids_depois {
        let pisos: Vec<f64> = grid
            .pilotos
            .iter()
            .map(|p| fame::personal_fame_floor(p.titulos, p.vitorias, p.temporadas_elite))
            .collect();
        let r = resumir(&pisos);
        let titulos_max = grid.pilotos.iter().map(|p| p.titulos).max().unwrap_or(0);
        println!(
            "{:<16} {:>10.1} {:>10.1} {:>10.1} {:>12}",
            categoria, r.minimo, r.media, r.maximo, titulos_max
        );
    }

    // O efeito colateral que a receita precisa saber: o termo de fama do patrocínio
    // é LINEAR na presença, então a razão das médias É o fator de aumento.
    let pa = resumir(&presenca_antes);
    let pd = resumir(&presenca_depois);
    println!("=== TERMO DE FAMA DO PATROCÍNIO (consequência, não alvo) ===\n");
    println!(
        "presença média por equipe: {:.1} → {:.1}  ({:+.1}%, fator {:.2}×)",
        pa.media,
        pd.media,
        100.0 * (pd.media / pa.media - 1.0),
        pd.media / pa.media
    );
    println!(
        "σ da presença entre equipes: {:.1} → {:.1}\n",
        pa.desvio, pd.desvio
    );

    println!("=== ATRAÇÃO DE PÚBLICO POR EQUIPE ===\n");
    println!(
        "{:<16} {:>10} {:>10} {:>10} {:>14}",
        "Categoria", "melhor", "pior", "σ", "espalhamento"
    );
    println!("{}", "-".repeat(66));
    for (categoria, grid) in &grids_depois {
        let atracoes = atracao_por_equipe(grid);
        let r = resumir(&atracoes);
        println!(
            "{:<16} {:>10.1} {:>10.1} {:>10.1} {:>14.1}",
            categoria,
            r.maximo,
            r.minimo,
            r.desvio,
            r.maximo - r.minimo
        );
    }
    println!();
}
