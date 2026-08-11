//! Montagem do "fact bundle" pós-corrida enviado ao servidor.
//!
//! Um bloco por assunto (cenário, resultado, curso da corrida, rivalidade, lesão…), cada
//! um numa função própria que devolve o texto pronto — ou string vazia quando não há o
//! que dizer. `build_post_race_facts` só junta: escolhe a tese, promove o núcleo e
//! monta as camadas. Os blocos moravam todos DENTRO dela, e regressão em um só aparecia
//! lendo o texto gerado inteiro.

use super::*;

use std::fmt::Write;

use crate::calendar::CalendarEntry;
use crate::commands::race::MaintenanceBreakdown;
use crate::models::driver::Driver;
use crate::race_eval::RaceEvaluation;
use crate::simulation::race::{RaceDriverResult, RaceResult};

// ── Limiares dos blocos ──────────────────────────────────────────────────────────
//
// Os quatro números abaixo estavam soltos no meio da montagem, sem nome e sem
// procedência. Ganharam nome, mas não ganharam medição: são ESCOLHAS DE PRODUTO sobre
// quando um fato merece virar frase, e não constantes calibradas contra dado nenhum.
// Mexer neles muda a FREQUÊNCIA com que a frase aparece nas corridas, nunca o texto de
// uma corrida específica — e é assim que devem ser avaliados (jogando, não medindo).

/// Mídia acima disto e o piloto é tratado como personagem pela imprensa do mundo.
const FAMA_MIN_ESTRELA: f64 = 70.0;
/// O segundo corte separa "estrela" de "ídolo".
const FAMA_MIN_IDOLO: f64 = 87.0;
/// Acima disto a pressão de título é anunciada como ALTA (abaixo, como média). A escala
/// de `pressure_intensity` é a mesma que a corrida usou.
const PRESSAO_INTENSIDADE_ALTA: f64 = 2.0;
/// Deadzone do `pace_delta`: dentro dela o fim de semana sob pressão é NEUTRO. Existe
/// porque um delta quase zero, com sinal aleatório, viraria "segurou" ou "afundou" no
/// texto — cravando uma direção que o número não sustenta.
const PRESSAO_DEADZONE_PACE: f64 = 0.4;

fn weather_label(w: &str) -> String {
    let key = match w {
        "HeavyRain" => "ai_news.weather.heavy_rain",
        "Wet" => "ai_news.weather.wet",
        "Damp" => "ai_news.weather.damp",
        _ => "ai_news.weather.dry",
    };
    rust_i18n::t!(key).to_string()
}

/// Escolhe a frase de fechamento do fim de semana: o que a equipe ANUNCIOU no sábado
/// contra o que o piloto ENTREGOU no domingo.
///
/// `soma` é a favorabilidade geral anunciada (as três faixas somadas, em [-6, 6]);
/// `assessment` é o veredito do cérebro pós-corrida.
///
/// **O eixo do tom é um só: quem leva a conta.** Quando anunciado e entregue CONCORDAM, o
/// fim de semana explica o resultado; quando DIVERGEM, quem explica é o piloto. Os dois
/// casos de divergência são os informativos — previsão que falha ensina que a leitura é
/// probabilística e não promessa; previsão que acerta apenas confirma.
///
/// | anunciado | entregue | quem leva a conta | tom |
/// |---|---|---|---|
/// | a favor | acima | condições | rodapé, confirma e sai |
/// | a favor | abaixo | **piloto** | o mais duro — as condições não explicam |
/// | contra | abaixo | condições | contexto, sem absolver |
/// | contra | acima | **piloto** | crédito — tirou o que não estava lá |
///
/// O caso "a favor × abaixo" é o que impede a leitura de virar álibi automático: sem ele,
/// anunciar o fim de semana só serviria para desculpar resultado ruim.
///
/// `None` quando não há o que fechar — anúncio neutro (`soma == 0`) ou resultado dentro do
/// esperado. Nem toda corrida rende uma frase, e forçar uma seria ruído; é o mesmo
/// princípio da regra do vazio.
// `pub(crate)` e não `pub(super)`: `ai_news.rs` reexporta este módulo com
// `pub(crate) use fatos::*`, e um glob não reexporta item MENOS visível que ele — o
// `pub(super)` sumia em silêncio antes de chegar ao módulo de testes.
pub(crate) fn caso_do_anuncio(
    soma: i32,
    assessment: crate::race_eval::Assessment,
) -> Option<&'static str> {
    use crate::race_eval::Assessment;
    let acima = matches!(assessment, Assessment::MuitoAcima | Assessment::Acima);
    let abaixo = matches!(assessment, Assessment::Abaixo | Assessment::MuitoAbaixo);

    match (soma.signum(), acima, abaixo) {
        (1, true, _) => Some("ai_news.facts.forecast_good_delivered"),
        (1, _, true) => Some("ai_news.facts.forecast_good_missed"),
        (-1, _, true) => Some("ai_news.facts.forecast_bad_confirmed"),
        (-1, true, _) => Some("ai_news.facts.forecast_bad_beaten"),
        _ => None,
    }
}

// ── Os blocos ────────────────────────────────────────────────────────────────────
//
// Contrato comum a todos: devolvem o texto do bloco, ou string VAZIA quando não há o que
// contar. Bloco vazio não entra no pacote — é a regra do vazio, e é ela que faz o texto
// não ter parágrafo dizendo "sem incidentes, sem paradas, sem nada".

/// Cabeçalho: pista, clima, categoria, rodada e distância. É a única parte que sempre sai.
fn bloco_cenario(
    result: &RaceResult,
    categoria: &str,
    season_num: i32,
    calendar_entry: Option<&CalendarEntry>,
) -> String {
    let mut cenario = String::new();
    let _ = write!(
        cenario,
        "{}",
        rust_i18n::t!(
            "ai_news.facts.scenario_head",
            track = result.track_name.as_str(),
            weather = weather_label(&result.weather).as_str()
        )
    );
    if !categoria.is_empty() {
        let _ = write!(
            cenario,
            "{}",
            rust_i18n::t!("ai_news.facts.scenario_category", category = categoria)
        );
    }
    if let Some(entry) = calendar_entry {
        let _ = write!(
            cenario,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.scenario_round",
                season = season_num,
                round = entry.rodada
            )
        );
    }
    let _ = write!(
        cenario,
        "{}",
        rust_i18n::t!("ai_news.facts.scenario_laps", laps = result.total_laps)
    );
    cenario
}

/// META + NOTA: o que o cérebro (`race_eval`) esperava e o que ele deu.
fn bloco_avaliacao(evaluation: Option<&RaceEvaluation>) -> String {
    let mut eval_b = String::new();
    if let Some(ev) = evaluation {
        let _ = writeln!(
            eval_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.target",
                low = ev.target_low,
                high = ev.target_high
            )
        );
        let _ = write!(
            eval_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.grade",
                grade = format!("{:.1}", ev.grade),
                label = ev.assessment.label()
            )
        );
    }
    eval_b
}

/// SEU RESULTADO: grid, chegada (ou abandono), saldo, gap, melhor volta, pontos, incidentes.
fn bloco_resultado(player: &RaceDriverResult) -> String {
    let mut res_b = String::new();
    let _ = writeln!(res_b, "{}", rust_i18n::t!("ai_news.facts.result_head"));
    let _ = writeln!(
        res_b,
        "{}",
        rust_i18n::t!("ai_news.facts.started", grid = player.grid_position)
    );
    if player.is_dnf {
        match player.dnf_reason.as_deref().filter(|s| !s.is_empty()) {
            Some(m) => {
                let _ = writeln!(
                    res_b,
                    "{}",
                    rust_i18n::t!("ai_news.facts.dnf_reason", reason = m)
                );
            }
            None => {
                let _ = writeln!(res_b, "{}", rust_i18n::t!("ai_news.facts.dnf"));
            }
        }
    } else {
        let _ = writeln!(
            res_b,
            "{}",
            rust_i18n::t!("ai_news.facts.finished", pos = player.finish_position)
        );
    }
    let saldo = player.positions_gained;
    let saldo_txt = if saldo > 0 {
        rust_i18n::t!("ai_news.facts.gained", n = saldo).to_string()
    } else if saldo < 0 {
        rust_i18n::t!("ai_news.facts.lost", n = saldo.abs()).to_string()
    } else {
        rust_i18n::t!("ai_news.facts.held").to_string()
    };
    let _ = writeln!(
        res_b,
        "{}",
        rust_i18n::t!("ai_news.facts.balance", txt = saldo_txt)
    );
    if !player.is_dnf && player.finish_position > 1 {
        let gap = player.gap_to_winner_ms / 1000.0;
        if gap > 0.0 {
            let _ = writeln!(
                res_b,
                "{}",
                rust_i18n::t!("ai_news.facts.gap_to_winner", secs = format!("{gap:.3}"))
            );
        }
    }
    if player.best_lap_time_ms > 0.0 {
        let s = player.best_lap_time_ms / 1000.0;
        let m = (s / 60.0).floor();
        let rest = s - m * 60.0;
        let fastest = if player.has_fastest_lap {
            rust_i18n::t!("ai_news.facts.fastest_flag").to_string()
        } else {
            String::new()
        };
        let _ = writeln!(
            res_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.best_lap",
                time = format!("{}:{:06.3}", m as i64, rest),
                fastest = fastest
            )
        );
    }
    let _ = writeln!(
        res_b,
        "{}",
        rust_i18n::t!("ai_news.facts.points", n = player.points_earned)
    );
    let _ = write!(
        res_b,
        "{}",
        rust_i18n::t!("ai_news.facts.incidents", n = player.incidents_count)
    );
    res_b
}

/// O CURSO da corrida: trânsito, box, safety car.
///
/// O motor calcula isto desde a reforma da simulação e nada disso chegava ao texto: o
/// debrief falava de P8 sem nunca falar do que produziu o P8. É a diferença entre
/// "você foi mal" e "você ficou 3 trechos atrás do Braun, tentou duas vezes e não
/// passou" — a segunda é a mesma corrida, contada de um jeito que não parece dado.
///
/// Só entra quando há de fato o que contar. Corrida gravada antes da v55, e import do
/// iRacing (que não tem trecho nenhum), deixam o bloco vazio e ele nem aparece.
fn bloco_curso(player: &RaceDriverResult, result: &RaceResult) -> String {
    let mut curso_b = String::new();
    let tentou = player.tentativas_ultrapassagem;
    let passou = player.ultrapassagens_concluidas;
    let preso = player.maior_sequencia_preso;
    let ar_sujo = player.segmentos_em_ar_sujo;
    let tem_transito = tentou > 0 || preso > 0 || ar_sujo > 0;
    let tem_parada = !player.volta_da_parada.is_empty();
    let tem_sc = !result.safety_cars.is_empty();

    if tem_transito || tem_parada || tem_sc {
        let _ = writeln!(curso_b, "{}", rust_i18n::t!("ai_news.facts.course_head"));
    }

    if tem_transito {
        // Tentativa que não vira ultrapassagem é o mecanismo mais novo do motor e o
        // mais invisível: antes da reforma a taxa de conversão era 100% implícita.
        let _ = writeln!(
            curso_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.course_traffic",
                tried = tentou,
                done = passou,
                suffered = player.tentativas_sofridas
            )
        );
        if preso > 0 {
            let _ = writeln!(
                curso_b,
                "{}",
                rust_i18n::t!("ai_news.facts.course_stuck", n = preso, dirty = ar_sujo)
            );
        }
    }

    // Custo do box: "largou P4, parou na volta 12, voltou P9, terminou P6" é uma
    // história completa em quatro números — e é o que faz o jogador perceber que
    // estratégia existe. Os três vetores da parada são paralelos por contrato.
    for (idx, volta) in player.volta_da_parada.iter().enumerate() {
        let antes = player.posicao_antes_da_parada.get(idx).copied();
        let depois = player.posicao_depois.get(idx).copied();
        match (antes, depois) {
            (Some(a), Some(d)) => {
                let _ = writeln!(
                    curso_b,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.facts.course_pit",
                        lap = volta,
                        before = a,
                        after = d,
                        delta = d - a
                    )
                );
            }
            // Parada gravada sem o par de posições ainda é fato ("parou na volta 12").
            _ => {
                let _ = writeln!(
                    curso_b,
                    "{}",
                    rust_i18n::t!("ai_news.facts.course_pit_bare", lap = volta)
                );
            }
        }
    }
    if !player.estrategia_id.is_empty() {
        let _ = writeln!(
            curso_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.course_strategy",
                strategy = player.estrategia_id.as_str()
            )
        );
    }

    // Safety car: o do jogo troca o vencedor com frequência e o jogador hoje não tem
    // como saber que foi ele. A posição do piloto ANTES da amarela é o número que
    // transforma "perdi posições" em "a amarela me custou posições".
    for (idx, volta) in result.safety_cars.iter().enumerate() {
        let antes = result
            .ordem_pre_safety_car
            .get(idx)
            .and_then(|ordem| ordem.iter().position(|id| id == &player.pilot_id))
            .map(|i| i as i32 + 1);
        match antes {
            Some(pos) if !player.is_dnf => {
                let _ = writeln!(
                    curso_b,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.facts.course_safety_car",
                        lap = volta,
                        before = pos,
                        after = player.finish_position
                    )
                );
            }
            _ => {
                let _ = writeln!(
                    curso_b,
                    "{}",
                    rust_i18n::t!("ai_news.facts.course_safety_car_bare", lap = volta)
                );
            }
        }
    }
    curso_b
}

/// O companheiro de equipe — a única comparação com o mesmo carro embaixo.
fn bloco_companheiro(player: &RaceDriverResult, result: &RaceResult) -> String {
    let mut mate_b = String::new();
    if let Some(mate) = result
        .race_results
        .iter()
        .find(|r| r.team_id == player.team_id && !r.is_jogador)
    {
        let mate_pos = if mate.is_dnf {
            rust_i18n::t!("ai_news.facts.dnf_short").to_string()
        } else {
            format!("P{}", mate.finish_position)
        };
        let cmp = if player.is_dnf || mate.is_dnf {
            String::new()
        } else if player.finish_position < mate.finish_position {
            rust_i18n::t!("ai_news.facts.teammate_ahead").to_string()
        } else {
            rust_i18n::t!("ai_news.facts.teammate_behind").to_string()
        };
        let _ = write!(
            mate_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.teammate",
                name = mate.pilot_name.as_str(),
                pos = mate_pos,
                cmp = cmp
            )
        );
    }
    mate_b
}

/// O rival de CAMPEONATO — o MESMO que a pré-corrida marcou.
fn bloco_rival_campeonato(
    conn: &rusqlite::Connection,
    player: &RaceDriverResult,
    result: &RaceResult,
    categoria: &str,
) -> String {
    let mut champ_b = String::new();
    if categoria.is_empty() {
        return champ_b;
    }
    if let Ok(Some(rival)) =
        crate::commands::career::build_primary_rival_summary(conn, &player.pilot_id, categoria)
    {
        // Empate em pontos vira frase própria: "atrás de você por 0pts" mandaria a
        // IA cravar uma direção que a tabela não sustenta.
        let standing = if rival.gap_points == 0 {
            rust_i18n::t!("ai_news.facts.champ_tied").to_string()
        } else if rival.is_ahead {
            rust_i18n::t!("ai_news.facts.champ_ahead", pts = rival.gap_points).to_string()
        } else {
            rust_i18n::t!("ai_news.facts.champ_behind", pts = rival.gap_points).to_string()
        };
        match result
            .race_results
            .iter()
            .find(|r| r.pilot_id == rival.driver_id)
        {
            Some(rr) => {
                let rpos = if rr.is_dnf {
                    rust_i18n::t!("ai_news.facts.dnf_short").to_string()
                } else {
                    format!("P{}", rr.finish_position)
                };
                let cmp = if !player.is_dnf && !rr.is_dnf {
                    if player.finish_position < rr.finish_position {
                        rust_i18n::t!("ai_news.facts.champ_you_ahead").to_string()
                    } else {
                        rust_i18n::t!("ai_news.facts.champ_you_behind").to_string()
                    }
                } else {
                    String::new()
                };
                let _ = write!(
                    champ_b,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.facts.champ_rival",
                        name = rival.driver_name.as_str(),
                        standing = standing,
                        pos = rpos,
                        cmp = cmp
                    )
                );
            }
            None => {
                let _ = write!(
                    champ_b,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.facts.champ_rival_absent",
                        name = rival.driver_name.as_str(),
                        standing = standing
                    )
                );
            }
        }
    }
    champ_b
}

/// A rivalidade VIVIDA (nêmesis + rivais) e, junto, a captura do DUELO decidido.
///
/// O duelo sai daqui porque é o mesmo varrido: o 1º duelo decidido (o nêmesis vem
/// primeiro, então tem prioridade) vira sinal para a tese quando o resultado foi morno.
fn bloco_rivalidade_vivida(
    conn: &rusqlite::Connection,
    player: &RaceDriverResult,
    result: &RaceResult,
) -> (String, Option<PostRaceDuel>) {
    use std::cmp::Ordering;

    let mut lived_b = String::new();
    let mut duel: Option<PostRaceDuel> = None;
    let current = crate::db::queries::player_nemesis::get_current_nemesis(conn).unwrap_or(None);
    let interests = crate::commands::career::select_player_interests(conn, current.as_deref());
    let mut rows: Vec<(&str, crate::commands::career::RivalInterest)> = Vec::new();
    if let Some(n) = interests.nemesis {
        rows.push(("NEMESIS", n));
    }
    for r in interests.rivais {
        rows.push(("RIVAL", r));
    }
    for (role, ri) in rows {
        if duel.is_none() && !player.is_dnf {
            if let Some(rr) = result
                .race_results
                .iter()
                .find(|d| d.pilot_id == ri.driver_id)
            {
                if !rr.is_dnf && rr.finish_position != player.finish_position {
                    duel = Some(PostRaceDuel {
                        name: ri.driver_name.clone(),
                        player_won: player.finish_position < rr.finish_position,
                        is_nemesis: role == "NEMESIS",
                        h2h: if ri.chapters > 0 {
                            Some((ri.h2h_player_wins, ri.h2h_rival_wins))
                        } else {
                            None
                        },
                    });
                }
            }
        }
        let today = match result
            .race_results
            .iter()
            .find(|d| d.pilot_id == ri.driver_id)
        {
            Some(rr) => {
                let pos = if rr.is_dnf {
                    rust_i18n::t!("ai_news.facts.dnf_short").to_string()
                } else {
                    format!("P{}", rr.finish_position)
                };
                let cmp = if !player.is_dnf && !rr.is_dnf {
                    match player.finish_position.cmp(&rr.finish_position) {
                        Ordering::Less => {
                            rust_i18n::t!("ai_news.facts.lived_you_ahead").to_string()
                        }
                        Ordering::Greater => {
                            rust_i18n::t!("ai_news.facts.lived_you_behind").to_string()
                        }
                        Ordering::Equal => String::new(),
                    }
                } else {
                    String::new()
                };
                rust_i18n::t!("ai_news.facts.lived_finished", pos = pos, cmp = cmp).to_string()
            }
            None => rust_i18n::t!("ai_news.facts.lived_absent").to_string(),
        };
        // `role` é chave de LÓGICA (comparada acima); o rótulo exibido é resolvido à parte.
        let role_label = if role == "NEMESIS" {
            rust_i18n::t!("ai_news.facts.role_nemesis")
        } else {
            rust_i18n::t!("ai_news.facts.role_rival")
        };
        let label = ri.label.map(|l| format!(" \"{l}\"")).unwrap_or_default();
        let h2h = if ri.chapters > 0 {
            rust_i18n::t!(
                "ai_news.facts.lived_h2h",
                p = ri.h2h_player_wins,
                r = ri.h2h_rival_wins
            )
            .to_string()
        } else {
            String::new()
        };
        let _ = writeln!(
            lived_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.lived_line",
                role = role_label,
                label = label,
                name = ri.driver_name.as_str(),
                today = today,
                h2h = h2h
            )
        );
    }
    (lived_b, duel)
}

/// LESÃO sofrida nesta corrida ou carregada de antes.
///
/// Fecha o loop físico: se o jogador se machucou HOJE (a lesão ativa aponta para esta
/// corrida em `race_occurred`) o debrief referencia isso; senão, nota que já corria
/// machucado. A geração da lesão é de outro sistema — aqui só LEMOS o que existe.
fn bloco_lesao(conn: &rusqlite::Connection, player: &RaceDriverResult, race_id: &str) -> String {
    let mut inj_b = String::new();
    if let Ok(Some(inj)) =
        crate::db::queries::injuries::get_active_injury_for_pilot(conn, &player.pilot_id)
    {
        use crate::models::enums::InjuryType;
        let severity = match inj.injury_type {
            InjuryType::Grave | InjuryType::Critica => rust_i18n::t!("ai_news.facts.injury_severe"),
            InjuryType::Moderada => rust_i18n::t!("ai_news.facts.injury_moderate"),
            InjuryType::Leve => rust_i18n::t!("ai_news.facts.injury_light"),
        };
        let name = if inj.injury_name.trim().is_empty() {
            inj.injury_type.as_str().to_string()
        } else {
            inj.injury_name.clone()
        };
        let _ = if inj.race_occurred == race_id {
            write!(
                inj_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.injury_new",
                    name = name,
                    severity = severity,
                    races = inj.races_remaining
                )
            )
        } else {
            write!(
                inj_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.injury_ongoing",
                    name = name,
                    severity = severity,
                    races = inj.races_remaining
                )
            )
        };
    }
    inj_b
}

/// ESTRELATO (fama) — só quando é estrela de verdade.
///
/// Os dois cortes (70 e 87) são de PRODUTO e não saem de medição: abaixo de 70 a mídia
/// não trataria o piloto como personagem, e o segundo corte separa "estrela" de "ídolo".
/// Mudar um deles muda quantas corridas rendem a frase, não o texto de uma corrida.
fn bloco_fama(midia: Option<f64>) -> String {
    let mut fame_b = String::new();
    if let Some(midia) = midia {
        if midia > FAMA_MIN_ESTRELA {
            let level = if midia > FAMA_MIN_IDOLO {
                rust_i18n::t!("ai_news.facts.fame_idol")
            } else {
                rust_i18n::t!("ai_news.facts.fame_star")
            };
            let _ = write!(
                fame_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.fame",
                    level = level,
                    value = midia.round() as i64
                )
            );
        }
    }
    fame_b
}

/// PRESSÃO DE TÍTULO (clutch/choke) — espelha `simulation::pressure`.
///
/// O sim não persiste o efeito de pressão; aqui recomputamos o MESMO estado com as
/// MESMAS funções (título + intensidade + resiliência) que a corrida aplicou, e o
/// viramos fato narrativo (segurou/afundou sob pressão). Só existe sob pressão real
/// de campeonato (intensidade > 0) — fora disso não vira ruído.
fn bloco_pressao(
    conn: &rusqlite::Connection,
    player_driver: Option<&Driver>,
    categoria: &str,
    calendar_entry: Option<&CalendarEntry>,
) -> String {
    let mut prs_b = String::new();
    let (Some(pd), Some(cat), Some(entry)) = (
        player_driver,
        crate::constants::categories::get_category(categoria),
        calendar_entry,
    ) else {
        return prs_b;
    };

    let races_left = (cat.corridas_por_temporada as i32 - entry.rodada + 1).max(1) as u32;
    let cat_drivers = crate::db::queries::drivers::get_drivers_by_active_category(conn, categoria)
        .unwrap_or_default();
    let all_points: Vec<f64> = cat_drivers
        .iter()
        .map(|d| d.stats_temporada.pontos)
        .collect();
    let max_race_points =
        (crate::constants::scoring::get_points_for_position(1, categoria == "endurance")
            + crate::constants::scoring::BONUS_FASTEST_LAP) as f64;
    let ctx = crate::simulation::pressure::title_context(
        pd.stats_temporada.pontos,
        &all_points,
        races_left,
        max_race_points,
    );
    let intensity = crate::simulation::pressure::pressure_intensity(&ctx, races_left);
    // Precisa de uma tabela REAL (≥2 pilotos) — categorias especiais/vazias não têm
    // matemática de título e não devem disparar pressão fantasma.
    if all_points.len() < 2 || intensity <= 0.0 {
        return prs_b;
    }
    let is_chaser = ctx.in_contention && !ctx.is_leader;
    let resilience = crate::simulation::pressure::pressure_resilience(
        pd.atributos.mentalidade,
        pd.atributos.experiencia,
    );
    let eff = crate::simulation::pressure::pressure_effect(intensity, resilience, is_chaser);
    let band = if races_left <= 1 {
        rust_i18n::t!("ai_news.facts.pressure_max")
    } else if intensity >= PRESSAO_INTENSIDADE_ALTA {
        rust_i18n::t!("ai_news.facts.pressure_high")
    } else {
        rust_i18n::t!("ai_news.facts.pressure_mid")
    };
    // Direção pelo SINAL do pace_delta (deadzone pequena p/ o caso ~neutro).
    let dir = if eff.pace_delta > PRESSAO_DEADZONE_PACE {
        rust_i18n::t!("ai_news.facts.pressure_clutch")
    } else if eff.pace_delta < -PRESSAO_DEADZONE_PACE {
        rust_i18n::t!("ai_news.facts.pressure_choke")
    } else {
        rust_i18n::t!("ai_news.facts.pressure_neutral")
    };
    let _ = write!(
        prs_b,
        "{}",
        rust_i18n::t!("ai_news.facts.pressure_line", band = band, dir = dir)
    );
    prs_b
}

/// MANUTENÇÃO: a conta do fim de semana, e o que dela foi batida.
fn bloco_manutencao(maintenance: &MaintenanceBreakdown) -> String {
    let mut mnt_b = String::new();
    if maintenance.total > 0.0 {
        let _ = writeln!(
            mnt_b,
            "{}",
            rust_i18n::t!(
                "ai_news.facts.maintenance_total",
                total = maintenance.total.round() as i64
            )
        );
        let danos: Vec<String> = maintenance
            .items
            .iter()
            .filter(|i| !matches!(i.key.as_str(), "gasolina" | "pneus"))
            .map(|i| format!("{} $ {}", i.label, i.cost.round() as i64))
            .collect();
        if !danos.is_empty() {
            let _ = write!(
                mnt_b,
                "{}",
                rust_i18n::t!("ai_news.facts.maintenance_crash", items = danos.join(", "))
            );
        }
    }
    mnt_b
}

/// QUEBRAS de peça (as do jogador em detalhe, as do grid em contagem) e, junto, se
/// alguma delas foi grave o bastante para o abandono contar como MECÂNICO.
fn bloco_quebras(
    conn: &rusqlite::Connection,
    player: &RaceDriverResult,
    race_id: &str,
) -> (String, bool) {
    let breakdowns = crate::db::queries::race_breakdowns::get_breakdowns_for_race(conn, race_id)
        .unwrap_or_default();
    let mut brk_b = String::new();
    let mut player_mech_break = false;
    if breakdowns.is_empty() {
        return (brk_b, player_mech_break);
    }
    let mine: Vec<_> = breakdowns
        .iter()
        .filter(|b| b.driver_id == player.pilot_id)
        .collect();
    if !mine.is_empty() {
        player_mech_break = mine.iter().any(|b| b.severity.e_severa());
        let _ = writeln!(brk_b, "{}", rust_i18n::t!("ai_news.facts.parts_head"));
        for b in &mine {
            let desfecho = match b.penalty_secs {
                Some(s) => rust_i18n::t!("ai_news.facts.part_pit", secs = s).to_string(),
                None => rust_i18n::t!("ai_news.facts.part_dnf").to_string(),
            };
            let grav = if b.severity.e_severa() {
                rust_i18n::t!("ai_news.facts.part_severe")
            } else {
                rust_i18n::t!("ai_news.facts.part_light")
            };
            let _ = writeln!(
                brk_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.part_line",
                    lap = b.lap,
                    label = b.label.as_str(),
                    outcome = desfecho,
                    severity = grav
                )
            );
        }
    }
    let grid_dnf = breakdowns
        .iter()
        .filter(|b| b.severity.e_abandono() && b.driver_id != player.pilot_id)
        .count();
    let grid_pen = breakdowns
        .iter()
        .filter(|b| !b.severity.e_abandono() && b.driver_id != player.pilot_id)
        .count();
    if grid_dnf + grid_pen > 0 {
        let _ = write!(
            brk_b,
            "{}",
            rust_i18n::t!("ai_news.facts.grid_breaks", dnf = grid_dnf, pen = grid_pen)
        );
    }
    (brk_b, player_mech_break)
}

/// O ANÚNCIO do fim de semana × o que foi entregue.
///
/// O fechamento do loop da fase 3. A regra é que o pós REFERENCIE a previsão, nunca
/// repita o estado: "o acerto estava ruim" lido depois é desculpa; "a equipe te avisou
/// no sábado, e não veio" é mecanismo se cumprindo. O que licencia o pós é o pré.
///
/// Lê a faixa ANUNCIADA do banco (v56), nunca recomputa: recomputar faria uma
/// recalibração do σ mudar retroativamente o que foi dito no sábado.
fn bloco_anuncio(
    conn: &rusqlite::Connection,
    player: &RaceDriverResult,
    evaluation: Option<&RaceEvaluation>,
    race_id: &str,
) -> String {
    let mut anuncio_b = String::new();
    let Some(ev) = evaluation else {
        return anuncio_b;
    };
    // Da tabela própria da v57 — a MESMA linha que a Sala de Estratégia leu no sábado,
    // não uma recomputação. É isso que garante que o pós não contradiga o pré.
    let anunciado: Option<i32> =
        crate::db::queries::races::get_race_weekend_reading(conn, race_id, &player.pilot_id)
            .ok()
            .flatten()
            .and_then(|json| {
                serde_json::from_str::<crate::commands::career_types::WeekendReading>(&json).ok()
            })
            .filter(|r| r.available)
            // A soma das três camadas é a favorabilidade GERAL do fim de semana — é disso
            // que a frase de fechamento fala. A decomposição em três continua sendo da
            // tela; aqui a pergunta é "o fim de semana estava a favor ou contra?".
            .map(|r| (r.track_affinity.band + r.driver_form.band + r.car_setup.band) as i32);

    if let Some(soma) = anunciado {
        if let Some(chave) = caso_do_anuncio(soma, ev.assessment) {
            let _ = writeln!(
                anuncio_b,
                "{}",
                rust_i18n::t!("ai_news.facts.forecast_head")
            );
            let _ = write!(anuncio_b, "{}", rust_i18n::t!(chave));
        }
    }
    anuncio_b
}

/// A PRÉ-CORRIDA, que é o que o pós tem de fechar.
fn bloco_pre_corrida(conn: &rusqlite::Connection, race_id: &str) -> String {
    let mut pre_b = String::new();
    if let Ok(Some(pre)) = crate::db::queries::ai_pre_race::get_pre_race(conn, race_id) {
        let _ = writeln!(pre_b, "{}", rust_i18n::t!("ai_news.facts.pre_head"));
        if !pre.headline.is_empty() {
            let _ = writeln!(
                pre_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.pre_headline",
                    headline = pre.headline.as_str()
                )
            );
        }
        if !pre.narrative.is_empty() {
            let _ = write!(
                pre_b,
                "{}",
                rust_i18n::t!(
                    "ai_news.facts.pre_briefing",
                    narrative = pre.narrative.as_str()
                )
            );
        }
    }
    pre_b
}

/// Os blocos que o núcleo promove ao APOIO mesmo quando a tese não os pediu.
///
/// O resultado (o que aconteceu) e o pré-corrida (fecha o loop) nunca podem cair no
/// rodapé. "curso" entra junto porque sem ele o APOIO descreve o que aconteceu e cala
/// sobre o porquê. Lesão e pressão são beats raros e de peso (físico e mental) que a
/// narrativa não pode tratar como nota de rodapé quando existem.
const NUCLEO_PROMOVIDO: [&str; 6] = [
    "resultado",
    "curso",
    "anuncio",
    "pre_race",
    "injury",
    "pressure",
];

/// Junta o pacote nas três camadas: CENÁRIO, EIXO (a tese), APOIO e PANO DE FUNDO.
///
/// `blocos` chega na ORDEM canônica de leitura — antes havia um array de ordem e um
/// `match` de id lado a lado, e manter os dois em sincronia era responsabilidade de
/// quem lembrasse. Bloco vazio é pulado; o que a tese (mais o núcleo promovido) marcou
/// vai para o APOIO, o resto desce para o PANO DE FUNDO.
fn montar_pacote(
    cenario: &str,
    statement: &str,
    support: &[&str],
    blocos: &[(&str, String)],
) -> String {
    let mut apoio = String::new();
    let mut fundo = String::new();
    for (id, texto) in blocos {
        let texto = texto.trim();
        if texto.is_empty() {
            continue;
        }
        let target = if support.contains(id) {
            &mut apoio
        } else {
            &mut fundo
        };
        let _ = writeln!(target, "\n{texto}");
    }

    let mut f = String::new();
    let _ = writeln!(
        f,
        "{}",
        rust_i18n::t!("ai_news.facts.scenario_line", scenario = cenario.trim())
    );
    let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.facts.axis_head"));
    let _ = writeln!(f, "{statement}");
    if !apoio.trim().is_empty() {
        let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.facts.support_head"));
        let _ = write!(f, "{}", apoio.trim_start_matches('\n'));
    }
    if !fundo.trim().is_empty() {
        let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.facts.background_head"));
        let _ = write!(f, "{}", fundo.trim_start_matches('\n'));
    }
    f
}

/// Monta o "fact bundle" do pós-corrida a partir da tela salva (resultado +
/// manutenção + avaliação) cruzada com o banco (companheiro, rival de campeonato,
/// últimas 3 corridas, contexto da pré-corrida). Organizado em torno de uma TESE
/// dominante (EIXO → APOIO → PANO DE FUNDO); o tom/voz fica no prompt do servidor.
/// String vazia → sem fatos suficientes (front cai no template).
pub(crate) fn build_post_race_facts(
    conn: &rusqlite::Connection,
    career_dir: &std::path::Path,
    race_id: &str,
) -> String {
    let path = career_dir
        .join("race_screens")
        .join(format!("{race_id}.json"));
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return String::new();
    };
    let Some(result) = v
        .get("race_result")
        .and_then(|x| serde_json::from_value::<RaceResult>(x.clone()).ok())
    else {
        return String::new();
    };
    let maintenance = v
        .get("maintenance")
        .and_then(|x| serde_json::from_value::<MaintenanceBreakdown>(x.clone()).ok())
        .unwrap_or_default();
    let evaluation = v
        .get("evaluation")
        .and_then(|x| serde_json::from_value::<RaceEvaluation>(x.clone()).ok());

    let Some(player) = result.race_results.iter().find(|r| r.is_jogador) else {
        return String::new();
    };

    let categoria = crate::db::queries::teams::get_team_by_id(conn, &player.team_id)
        .ok()
        .flatten()
        .map(|t| t.categoria)
        .unwrap_or_default();

    // ---- CENÁRIO (cabeçalho) ----
    let calendar_entry = crate::db::queries::calendar::get_calendar_entry_by_id(conn, race_id)
        .ok()
        .flatten();
    let season_num = crate::db::queries::seasons::get_active_season(conn)
        .ok()
        .flatten()
        .map(|s| s.numero)
        .unwrap_or(0);
    let cenario = bloco_cenario(&result, &categoria, season_num, calendar_entry.as_ref());

    // Registro do piloto (fama + atributos de pressão): uma leitura só, reusada abaixo.
    let player_driver = crate::db::queries::drivers::get_driver(conn, &player.pilot_id).ok();

    // Estes dois devolvem, além do texto, o sinal que a TESE vai consultar: o duelo
    // decidido e o abandono mecânico. Por isso são chamados antes dos outros.
    let (lived_b, duel) = bloco_rivalidade_vivida(conn, player, &result);
    let (brk_b, player_mech_break) = bloco_quebras(conn, player, race_id);

    // "O carro te traiu" vs "você/alguém rodou". A classificação é a MESMA que o
    // boletim usa (`race_signals::dnf_kind`); aqui os insumos disponíveis são a peça
    // grave registrada no banco e o motivo textual — o incidente cru não sobrevive ao
    // save.
    let dnf_mechanical = player.is_dnf
        && crate::race_signals::dnf_kind(None, player_mech_break, player.dnf_reason.as_deref())
            .is_mecanico();

    // ---- TESE DOMINANTE ----
    let signals = PostRaceSignals {
        is_dnf: player.is_dnf,
        dnf_mechanical,
        grid: player.grid_position,
        finish: player.finish_position,
        positions_gained: player.positions_gained,
        has_fastest_lap: player.has_fastest_lap,
        assessment: evaluation.as_ref().map(|e| e.assessment),
        target_low: evaluation.as_ref().map(|e| e.target_low).unwrap_or(0),
        target_high: evaluation.as_ref().map(|e| e.target_high).unwrap_or(0),
        duel,
        track_name: result.track_name.clone(),
    };
    let (statement, mut support) = select_post_race_thesis(&signals);
    for id in NUCLEO_PROMOVIDO {
        if !support.contains(&id) {
            support.push(id);
        }
    }

    // ---- Os blocos, na ORDEM canônica de leitura ----
    let blocos = [
        ("eval", bloco_avaliacao(evaluation.as_ref())),
        ("resultado", bloco_resultado(player)),
        // Vem logo depois do resultado porque é a CAUSA dele: o texto precisa ler "P6" e
        // já ter na mão o box e o trânsito que produziram o P6.
        ("curso", bloco_curso(player, &result)),
        // Logo depois do curso: o curso diz o que aconteceu na corrida, o anúncio diz se
        // o fim de semana tinha avisado. Juntos formam a atribuição completa.
        (
            "anuncio",
            bloco_anuncio(conn, player, evaluation.as_ref(), race_id),
        ),
        ("injury", bloco_lesao(conn, player, race_id)),
        (
            "pressure",
            bloco_pressao(
                conn,
                player_driver.as_ref(),
                &categoria,
                calendar_entry.as_ref(),
            ),
        ),
        // Telemetria real: só quando terminou. Quem abandonou tem meia corrida de dado, e
        // média de meia corrida diz o que não aconteceu.
        (
            "telemetry",
            if player.is_dnf {
                String::new()
            } else {
                telemetry_facts(v.get("telemetry"), player.grid_position)
            },
        ),
        ("teammate", bloco_companheiro(player, &result)),
        (
            "champ_rival",
            bloco_rival_campeonato(conn, player, &result, &categoria),
        ),
        ("lived_rivalry", lived_b),
        ("breakdowns", brk_b),
        ("maintenance", bloco_manutencao(&maintenance)),
        (
            "fame",
            bloco_fama(player_driver.as_ref().map(|d| d.atributos.midia)),
        ),
        ("pre_race", bloco_pre_corrida(conn, race_id)),
        // Memória entre etapas: sempre PANO DE FUNDO, nunca disputa o eixo.
        ("arc", build_recent_arc_facts(conn, race_id)),
    ];

    montar_pacote(&cenario, &statement, &support, &blocos)
}

#[cfg(test)]
mod tests {
    //! Testes dos BLOCOS, um a um.
    //!
    //! Moram aqui, e não em `ai_news/tests/mod.rs` (onde ficam a tese, a telemetria e a
    //! montagem de ponta a ponta), porque os blocos são privados deste módulo: só um
    //! filho dele os enxerga sem que cada um precise virar `pub(crate)` só por causa do
    //! teste.
    //!
    //! O que eles fecham é o modo de falha do pacote inteiro: o fact bundle é TEXTO, nada
    //! o valida em runtime, e um bloco que some ou que passa a mentir só aparece lendo o
    //! boletim gerado e achando estranho.

    use super::*;
    use serde_json::json;
    use serial_test::serial;

    /// Os fatos saem no locale ativo e estes testes conferem a prosa PT. `#[serial]`
    /// porque o locale é estado global do processo.
    fn pt() {
        rust_i18n::set_locale("pt-BR");
    }

    /// Um piloto do resultado, com os campos obrigatórios preenchidos; o resto vem do
    /// `serde(default)`, que é exatamente o que uma corrida antiga entrega.
    fn piloto(extra: serde_json::Value) -> RaceDriverResult {
        let mut base = json!({
            "pilot_id": "P001",
            "pilot_name": "Piloto Um",
            "team_id": "T001",
            "team_name": "Equipe Um",
            "grid_position": 6,
            "finish_position": 6,
            "positions_gained": 0,
            "best_lap_time_ms": 0.0,
            "total_race_time_ms": 2_800_000.0,
            "gap_to_winner_ms": 0.0,
            "is_dnf": false,
            "dnf_reason": null,
            "dnf_segment": null,
            "has_fastest_lap": false,
            "points_earned": 0,
            "is_jogador": true,
            "laps_completed": 30,
            "final_tire_wear": 0.4,
            "final_physical": 0.8,
            "classification_status": "Finished",
        });
        let (Some(base_map), Some(extra_map)) = (base.as_object_mut(), extra.as_object()) else {
            panic!("piloto: os dois lados têm de ser objeto");
        };
        for (k, v) in extra_map {
            base_map.insert(k.clone(), v.clone());
        }
        serde_json::from_value(base).expect("piloto")
    }

    /// O resultado da corrida com um grid qualquer.
    fn corrida(pilotos: Vec<RaceDriverResult>, extra: serde_json::Value) -> RaceResult {
        let mut base = json!({
            "qualifying_results": [],
            "race_results": pilotos,
            "pole_sitter_id": "P002",
            "winner_id": "P002",
            "fastest_lap_id": "P002",
            "total_laps": 30,
            "weather": "Dry",
            "track_name": "Interlagos",
        });
        let (Some(base_map), Some(extra_map)) = (base.as_object_mut(), extra.as_object()) else {
            panic!("corrida: os dois lados têm de ser objeto");
        };
        for (k, v) in extra_map {
            base_map.insert(k.clone(), v.clone());
        }
        serde_json::from_value(base).expect("corrida")
    }

    // ── Resultado ────────────────────────────────────────────────────────────────

    #[test]
    #[serial]
    fn o_resultado_conta_largada_chegada_e_saldo() {
        pt();
        let p = piloto(json!({
            "grid_position": 6,
            "finish_position": 3,
            "positions_gained": 3,
            "points_earned": 15,
            "incidents_count": 2,
            "gap_to_winner_ms": 12_400.0,
        }));
        let texto = bloco_resultado(&p);

        assert!(texto.contains("Largou em P6"), "texto:\n{texto}");
        assert!(texto.contains("Chegou em P3"), "texto:\n{texto}");
        assert!(texto.contains("ganhou 3 posições"), "texto:\n{texto}");
        assert!(
            texto.contains("+12.400s"),
            "gap pro vencedor. Texto:\n{texto}"
        );
        assert!(texto.contains("Pontos: 15"), "texto:\n{texto}");
        assert!(texto.contains("Incidentes: 2"), "texto:\n{texto}");
    }

    /// Saldo zero não pode virar "ganhou 0 posições": a frase própria existe porque o
    /// número sozinho já enganou o texto uma vez.
    #[test]
    #[serial]
    fn saldo_zero_e_frase_propria_e_saldo_negativo_conta_a_perda() {
        pt();
        let parado = bloco_resultado(&piloto(json!({ "positions_gained": 0 })));
        assert!(parado.contains("manteve a posição de largada"), "{parado}");

        let perdeu = bloco_resultado(&piloto(json!({
            "finish_position": 11,
            "positions_gained": -5,
        })));
        assert!(perdeu.contains("perdeu 5 posições"), "{perdeu}");
    }

    /// O motivo do abandono é o fato central da corrida quando existe; sem ele, o bloco
    /// ainda tem de dizer que abandonou.
    #[test]
    #[serial]
    fn o_abandono_leva_o_motivo_quando_ha_um() {
        pt();
        let com_motivo = bloco_resultado(&piloto(json!({
            "is_dnf": true,
            "dnf_reason": "motor fundiu",
            "classification_status": "Dnf",
        })));
        assert!(com_motivo.contains("ABANDONOU (DNF): motor fundiu"));
        assert!(
            !com_motivo.contains("Chegou em"),
            "quem abandonou não chegou. Texto:\n{com_motivo}"
        );

        let sem_motivo = bloco_resultado(&piloto(json!({
            "is_dnf": true,
            "dnf_reason": "",
            "classification_status": "Dnf",
        })));
        assert!(sem_motivo.contains("ABANDONOU (DNF)"), "{sem_motivo}");
    }

    /// Melhor volta em ms vira minuto:segundo. O formato é lido por um modelo de
    /// linguagem: 91.500 ms tem de sair "1:31.500", nunca o número cru.
    #[test]
    #[serial]
    fn a_melhor_volta_sai_em_minuto_e_segundo() {
        pt();
        let texto = bloco_resultado(&piloto(json!({
            "best_lap_time_ms": 91_500.0,
            "has_fastest_lap": true,
        })));
        assert!(texto.contains("1:31.500"), "texto:\n{texto}");
        assert!(texto.contains("VOLTA MAIS RÁPIDA"), "texto:\n{texto}");

        let sem = bloco_resultado(&piloto(json!({ "best_lap_time_ms": 0.0 })));
        assert!(
            !sem.contains("Melhor volta"),
            "corrida sem volta cronometrada não inventa uma. Texto:\n{sem}"
        );
    }

    // ── Curso da corrida ─────────────────────────────────────────────────────────

    /// A regra do vazio, no bloco em que ela mais importa: corrida importada do iRacing
    /// (sem trecho, sem parada, sem safety car) não pode render um cabeçalho pendurado.
    #[test]
    #[serial]
    fn sem_transito_parada_ou_safety_car_o_curso_nao_existe() {
        pt();
        let p = piloto(json!({}));
        let r = corrida(vec![p.clone()], json!({}));
        assert_eq!(bloco_curso(&p, &r), "");
    }

    #[test]
    #[serial]
    fn a_parada_conta_o_ciclo_inteiro_e_a_amarela_a_posicao_de_antes() {
        pt();
        let p = piloto(json!({
            "finish_position": 6,
            "volta_da_parada": [12],
            "posicao_antes_da_parada": [4],
            "posicao_depois": [9],
        }));
        let r = corrida(
            vec![p.clone()],
            json!({
                "safety_cars": [18],
                "ordem_pre_safety_car": [["P002", "P001"]],
            }),
        );
        let texto = bloco_curso(&p, &r);

        assert!(texto.contains("O CURSO DA CORRIDA"), "texto:\n{texto}");
        assert!(
            texto.contains("Box na volta 12: entrou P4, voltou P9"),
            "o ciclo da parada é o que torna estratégia visível. Texto:\n{texto}"
        );
        assert!(
            texto.contains("Safety car na volta 18: você estava P2"),
            "a posição ANTES da amarela é o número que explica a perda. Texto:\n{texto}"
        );
    }

    /// Parada gravada sem o par de posições (save antigo) ainda é fato, e não pode
    /// derrubar o bloco nem inventar posição.
    #[test]
    #[serial]
    fn parada_sem_o_par_de_posicoes_vira_a_frase_curta() {
        pt();
        let p = piloto(json!({ "volta_da_parada": [12] }));
        let r = corrida(vec![p.clone()], json!({}));
        let texto = bloco_curso(&p, &r);

        assert!(texto.contains("Box na volta 12"), "texto:\n{texto}");
        assert!(
            !texto.contains("entrou P"),
            "sem o par gravado não há de onde tirar as posições. Texto:\n{texto}"
        );
    }

    // ── Companheiro de equipe ────────────────────────────────────────────────────

    #[test]
    #[serial]
    fn o_companheiro_e_comparado_so_quando_os_dois_terminam() {
        pt();
        let eu = piloto(json!({ "finish_position": 4 }));
        let mate = piloto(json!({
            "pilot_id": "P002",
            "pilot_name": "Piloto Dois",
            "is_jogador": false,
            "finish_position": 9,
        }));
        let r = corrida(vec![eu.clone(), mate], json!({}));
        let texto = bloco_companheiro(&eu, &r);
        assert!(texto.contains("Piloto Dois"), "texto:\n{texto}");
        assert!(
            texto.contains("você ficou na frente dele"),
            "texto:\n{texto}"
        );

        let mate_dnf = piloto(json!({
            "pilot_id": "P002",
            "pilot_name": "Piloto Dois",
            "is_jogador": false,
            "is_dnf": true,
            "classification_status": "Dnf",
        }));
        let r = corrida(vec![eu.clone(), mate_dnf], json!({}));
        let texto = bloco_companheiro(&eu, &r);
        assert!(texto.contains("DNF"), "texto:\n{texto}");
        assert!(
            !texto.contains("ficou na frente"),
            "comparar com quem abandonou é comparação vazia. Texto:\n{texto}"
        );
    }

    /// Piloto sem companheiro no grid (equipe de um carro só) não rende bloco.
    #[test]
    #[serial]
    fn equipe_de_um_carro_so_nao_rende_bloco_de_companheiro() {
        pt();
        let eu = piloto(json!({}));
        let outro = piloto(json!({
            "pilot_id": "P002",
            "team_id": "T002",
            "is_jogador": false,
        }));
        let r = corrida(vec![eu.clone(), outro], json!({}));
        assert_eq!(bloco_companheiro(&eu, &r), "");
    }

    // ── Fama e manutenção ────────────────────────────────────────────────────────

    /// Os dois cortes de fama são escolha de produto, mas a BORDA é contrato: exatamente
    /// no limiar ainda não é estrela (a comparação é estrita).
    #[test]
    #[serial]
    fn a_fama_so_vira_fato_acima_do_corte() {
        pt();
        assert_eq!(bloco_fama(None), "");
        assert_eq!(bloco_fama(Some(FAMA_MIN_ESTRELA)), "");

        let estrela = bloco_fama(Some(FAMA_MIN_ESTRELA + 1.0));
        assert!(estrela.contains("estrela"), "{estrela}");

        let idolo = bloco_fama(Some(FAMA_MIN_IDOLO + 1.0));
        assert!(idolo.contains("ídolo"), "{idolo}");
    }

    /// Gasolina e pneu são custo de rotina e não entram na linha de CONSERTO: um fim de
    /// semana limpo com tanque cheio não pode virar "bateu o carro" no texto.
    #[test]
    #[serial]
    fn a_manutencao_separa_rotina_de_batida() {
        pt();
        let limpo: MaintenanceBreakdown = serde_json::from_value(json!({
            "total": 0.0,
            "items": [],
        }))
        .expect("manutenção");
        assert_eq!(bloco_manutencao(&limpo), "");

        let batida: MaintenanceBreakdown = serde_json::from_value(json!({
            "total": 4_200.0,
            "items": [
                { "key": "gasolina", "label": "Combustível", "cost": 200.0 },
                { "key": "pneus", "label": "Pneus", "cost": 1_000.0 },
                { "key": "asa_dianteira", "label": "Asa dianteira", "cost": 3_000.0 },
            ],
        }))
        .expect("manutenção");
        let texto = bloco_manutencao(&batida);

        assert!(texto.contains("$ 4200"), "texto:\n{texto}");
        assert!(texto.contains("Asa dianteira"), "texto:\n{texto}");
        assert!(
            !texto.contains("Combustível") && !texto.contains("Pneus"),
            "rotina não é conserto de batida. Texto:\n{texto}"
        );
    }

    // ── Montagem em camadas ──────────────────────────────────────────────────────

    /// O contrato da montagem: cenário e eixo sempre; o que a tese apoiou vai para o
    /// APOIO; o resto desce para o PANO DE FUNDO; bloco vazio não aparece em lugar nenhum.
    #[test]
    #[serial]
    fn a_montagem_separa_apoio_de_pano_de_fundo_e_pula_o_vazio() {
        pt();
        let blocos = [
            ("resultado", "SEU RESULTADO:\n- Largou em P6".to_string()),
            ("arc", "ARCO: vinha de dois pódios".to_string()),
            ("fame", String::new()),
        ];
        let texto = montar_pacote(
            "Interlagos, seco",
            "EIXO: dia de somar",
            &["resultado"],
            &blocos,
        );

        let apoio = texto.find("APOIO").expect("cabeçalho de apoio");
        let fundo = texto.find("PANO DE FUNDO").expect("cabeçalho de fundo");
        let resultado = texto.find("SEU RESULTADO").expect("bloco de resultado");
        let arco = texto.find("ARCO").expect("bloco de arco");

        assert!(
            texto.starts_with("CENÁRIO: Interlagos, seco"),
            "texto:\n{texto}"
        );
        assert!(texto.contains("EIXO: dia de somar"), "texto:\n{texto}");
        assert!(
            apoio < resultado && resultado < fundo && fundo < arco,
            "cada bloco tem de cair sob o seu cabeçalho. Texto:\n{texto}"
        );
    }

    /// Sem nenhum bloco de apoio o cabeçalho de APOIO não aparece — cabeçalho sozinho é
    /// instrução vazia para quem redige do outro lado.
    #[test]
    #[serial]
    fn cabecalho_de_camada_vazia_nao_aparece() {
        pt();
        let blocos = [("arc", "ARCO: vinha de dois pódios".to_string())];
        let texto = montar_pacote(
            "Interlagos, seco",
            "EIXO: dia de somar",
            &["resultado"],
            &blocos,
        );

        assert!(!texto.contains("APOIO"), "texto:\n{texto}");
        assert!(texto.contains("PANO DE FUNDO"), "texto:\n{texto}");

        let so_apoio = montar_pacote("Interlagos", "EIXO", &["arc"], &blocos);
        assert!(so_apoio.contains("APOIO"), "texto:\n{so_apoio}");
        assert!(!so_apoio.contains("PANO DE FUNDO"), "texto:\n{so_apoio}");
    }

    /// O núcleo é promovido mesmo quando a tese não pediu: sem isso o resultado da
    /// corrida podia terminar no rodapé, junto do contexto secundário.
    #[test]
    #[serial]
    fn o_nucleo_promovido_nunca_cai_no_rodape() {
        pt();
        for id in NUCLEO_PROMOVIDO {
            let blocos = [(id, format!("BLOCO {id}"))];
            let mut support: Vec<&str> = Vec::new();
            for promovido in NUCLEO_PROMOVIDO {
                support.push(promovido);
            }
            let texto = montar_pacote("Interlagos", "EIXO", &support, &blocos);
            assert!(
                !texto.contains("PANO DE FUNDO"),
                "'{id}' faz parte do núcleo e não pode descer. Texto:\n{texto}"
            );
        }
    }
}
