//! Gatilhos que criam ou reforçam rivalidades: hierarquia interna, disputa de
//! campeonato e colisões em pista (extraído de `rivalry/mod.rs`).

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::drivers::get_driver;
use crate::models::rivalry::{normalize_pair, RivalryType};

use super::eventos::{apply_rivalry_event, RivalryEvent};
use super::intensidade::crossed_threshold;
use super::noticias::{load_rivalry_driver_midia, persist_rivalry_news};

// ── Passo 6: Rivalidade por hierarquia interna ────────────────────────────────

/// Avalia transição de status hierárquico e aplica evento de rivalidade com dois eixos.
///
/// Deltas semânticos (Passo 13):
/// - Inversão:                       historical=8,  recent=18  → percebido ≈14
/// - Transição → Crise (nova):       historical=5,  recent=14  → percebido ≈10
/// - Transição → Reavaliação (nova): historical=3,  recent=10  → percebido ≈7
/// - Transição → Tensão (nova):      historical=2,  recent=7   → percebido ≈5
///
/// TENSÃO É A NOVA PORTA DE ENTRADA. Antes o primeiro degrau era Reavaliação, e
/// custava tensão 60 — meia temporada de domínio invertido dentro da equipe.
/// Nenhuma equipe do mundo chegava lá, e o gatilho nunca disparava. "Os dois
/// estão se incomodando" começa em Tensão (40), que é onde o clima já mudou o
/// bastante para ter nome próprio; os degraus acima continuam valendo mais.
pub fn process_hierarchy_rivalry(
    conn: &Connection,
    n1_id: &str,
    n2_id: &str,
    old_status_str: &str,
    new_status_str: &str,
    inversao: bool,
    categoria_id: &str,
    team_id: &str,
    rodada: i32,
    temporada: i32,
) -> Result<(), DbError> {
    use crate::models::team::TeamHierarchyClimate;

    let old_status = TeamHierarchyClimate::from_str(old_status_str);
    let new_status = TeamHierarchyClimate::from_str(new_status_str);

    let (h_delta, r_delta): (f64, f64) = if inversao {
        (8.0, 18.0)
    } else if new_status == TeamHierarchyClimate::Crise && old_status != TeamHierarchyClimate::Crise
    {
        (5.0, 14.0)
    } else if new_status == TeamHierarchyClimate::Reavaliacao
        && !matches!(
            old_status,
            TeamHierarchyClimate::Reavaliacao | TeamHierarchyClimate::Crise
        )
    {
        (3.0, 10.0)
    } else if new_status == TeamHierarchyClimate::Tensao
        && !matches!(
            old_status,
            TeamHierarchyClimate::Tensao
                | TeamHierarchyClimate::Reavaliacao
                | TeamHierarchyClimate::Inversao
                | TeamHierarchyClimate::Crise
        )
    {
        // A entrada só conta SUBINDO. Uma equipe que caiu de Crise para Tensão
        // não está começando nada — ela está esfriando, e cobrar o evento aí
        // faria a rivalidade renascer de um clima que melhorou.
        (2.0, 7.0)
    } else {
        return Ok(());
    };

    let applied = apply_rivalry_event(
        conn,
        &RivalryEvent {
            piloto_a: n1_id.to_string(),
            piloto_b: n2_id.to_string(),
            tipo: RivalryType::Companheiros,
            historical_delta: h_delta,
            recent_delta: r_delta,
            temporada,
        },
    )?;

    if crossed_threshold(applied.old_perceived, applied.new_perceived).is_some() {
        let nome_a = get_driver(conn, n1_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| n1_id.to_string());
        let nome_b = get_driver(conn, n2_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| n2_id.to_string());
        let driver_midia = load_rivalry_driver_midia(conn, n1_id, n2_id);

        persist_rivalry_news(
            conn,
            &applied,
            &RivalryType::Companheiros,
            &nome_a,
            &nome_b,
            categoria_id,
            temporada,
            rodada,
            n1_id,
            n2_id,
            Some(team_id),
            &driver_midia,
        )?;
    }

    Ok(())
}

// ── Fim de temporada: rivalidade de companheiros pelo placar do ano ──────────

/// Mínimo de duelos internos para o placar da temporada valer como fato.
/// Mesmo piso da inversão — meia temporada com os dois na pista.
const COMPANHEIROS_MIN_DUELOS: i32 = 8;
/// A partir daqui a equipe está dividida: o N2 não é mais coadjuvante.
const COMPANHEIROS_TAXA_MINIMA: f64 = 0.40;
/// A partir daqui o N2 empatou ou virou o ano — a hierarquia é ficção.
const COMPANHEIROS_TAXA_RACHA: f64 = 0.50;

/// Cria ou reforça rivalidade `Companheiros` a partir do **placar** do duelo interno
/// da temporada, sem passar pelo eixo de tensão.
///
/// Por que existe: a tensão é um acumulador com piso em zero que só sobe quando o N2
/// vence, e o mercado aloca o melhor piloto como N1 por construção. Medido num mundo
/// de 12 temporadas simuladas, o N2 leva 22,7% dos duelos — bem abaixo dos 40% que a
/// tensão precisa só para parar de cair. O eixo mora no chão, e o gatilho por transição
/// de clima ([`process_hierarchy_rivalry`]) nunca dispara: zero rivalidades de
/// companheiro em 27 temporadas de save real.
///
/// Aqui o critério é o fato direto em vez do acumulador. Uma temporada inteira em que
/// o segundo piloto bateu o primeiro em pelo menos 40% dos duelos internos é, por
/// definição, uma equipe dividida — e é exatamente isso que a rivalidade deveria dizer.
///
/// Deltas:
/// - N2 entre 40% e 50%: historical=4, recent=12 → percebido ≈8.8
/// - N2 em 50% ou mais:  historical=6, recent=16 → percebido ≈12
///
/// Deve ser chamada no fim de temporada **depois do decaimento** (o veredito é o
/// resumo da temporada; passá-lo pelo decaimento da mesma temporada esfria duas vezes)
/// e **antes da pré-temporada**, que zera os contadores. Equipe que inverteu no meio do
/// ano zera os contadores na hora e não chega aqui — a inversão já cobra o próprio
/// evento, bem maior (8/18).
pub fn process_teammate_season_rivalry(conn: &Connection, temporada: i32) -> Result<(), DbError> {
    // Categorias especiais têm hierarquia efêmera (bloco de convocação) e reset próprio —
    // o placar delas não é uma temporada de convivência.
    let equipes: Vec<(String, String, String, String, i32, i32)> = {
        let mut stmt = conn.prepare(
            "SELECT id, categoria, hierarquia_n1_id, hierarquia_n2_id,
                    hierarquia_duelos_total, hierarquia_duelos_n2_vencidos
             FROM teams
             WHERE ativa = 1
               AND categoria NOT IN ('production_challenger', 'endurance')
               AND hierarquia_n1_id IS NOT NULL
               AND hierarquia_n2_id IS NOT NULL
               AND hierarquia_duelos_total >= ?1",
        )?;
        let linhas = stmt.query_map([COMPANHEIROS_MIN_DUELOS], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?;
        linhas.collect::<Result<Vec<_>, _>>()?
    };

    for (team_id, categoria, n1_id, n2_id, duelos, n2_vencidos) in equipes {
        let taxa = n2_vencidos as f64 / duelos as f64;
        if taxa < COMPANHEIROS_TAXA_MINIMA {
            continue;
        }

        let (h_delta, r_delta) = if taxa >= COMPANHEIROS_TAXA_RACHA {
            (6.0, 16.0)
        } else {
            (4.0, 12.0)
        };

        let applied = apply_rivalry_event(
            conn,
            &RivalryEvent {
                piloto_a: n1_id.clone(),
                piloto_b: n2_id.clone(),
                tipo: RivalryType::Companheiros,
                historical_delta: h_delta,
                recent_delta: r_delta,
                temporada,
            },
        )?;

        if crossed_threshold(applied.old_perceived, applied.new_perceived).is_some() {
            let nome_a = get_driver(conn, &n1_id)
                .map(|d| d.nome)
                .unwrap_or_else(|_| n1_id.clone());
            let nome_b = get_driver(conn, &n2_id)
                .map(|d| d.nome)
                .unwrap_or_else(|_| n2_id.clone());
            let driver_midia = load_rivalry_driver_midia(conn, &n1_id, &n2_id);

            persist_rivalry_news(
                conn,
                &applied,
                &RivalryType::Companheiros,
                &nome_a,
                &nome_b,
                &categoria,
                temporada,
                0, // sem rodada: o fato é o placar do ano
                &n1_id,
                &n2_id,
                Some(&team_id),
                &driver_midia,
            )?;
        }
    }

    Ok(())
}

// ── Fim de temporada: rivalidade de pista pelo placar de adjacências ─────────

/// Mínimo de chegadas coladas na temporada para o par virar rivalidade de pista.
///
/// Calibrado medindo, e não chutado. Num mundo de 12 temporadas simuladas a
/// distribuição de adjacências por par cai assim: ≥4 dá ~61 pares por temporada
/// (spam), ≥5 dá ~17, ≥6 dá ~3.8, ≥7 dá ~1.6. Seis é onde a cauda começa a
/// significar alguma coisa — num calendário de ~20 corridas é um terço do ano
/// decidido entre exatamente esses dois.
const PISTA_MIN_ADJACENCIAS: i64 = 6;
/// A partir daqui não é mais "se cruzaram muito": é a briga da temporada.
const PISTA_ADJACENCIAS_DUELO: i64 = 8;

/// Cria ou reforça rivalidade `Pista` a partir das chegadas **coladas** da temporada.
///
/// Por que existe: o tipo `Pista` só era aplicado no caminho de importação de corrida
/// real do iRacing ([`crate::commands::iracing::resultado`]). Em corrida simulada —
/// que é quase todo o mundo, e todo o passado de qualquer carreira — ele nunca
/// disparava: zero rivalidades `Pista` em 27 temporadas de save real.
///
/// O critério é o mesmo espírito de [`process_teammate_season_rivalry`]: um placar
/// de fim de temporada em vez de um acumulador. Aqui o placar é quantas vezes os
/// dois cruzaram a linha separados por **exatamente uma posição**, com os dois
/// classificados.
///
/// ADJACÊNCIA, E NÃO PROXIMIDADE. Um corte por "chegaram a até 2 posições" pega o
/// pelotão inteiro — numa faixa de cinco carros do meio do grid todos estão perto de
/// todos, e sairiam ~90 rivalidades por temporada só de andar junto. Um atrás do
/// outro é raro o bastante para significar disputa.
///
/// EQUILÍBRIO NÃO É EXIGIDO, ao contrário da regra de companheiros. Lá o fato era
/// sobre hierarquia, e hierarquia precisa de paridade para virar treta. Aqui o fato
/// é ter passado o ano brigando pelo mesmo lugar — e ficar seis domingos colado atrás
/// de alguém sem conseguir passar é rivalidade tanto quanto ganhar.
///
/// Deltas:
/// - 6 a 7 adjacências: historical=3, recent=9  → percebido ≈6.6
/// - 8 ou mais:         historical=5, recent=13 → percebido ≈9.8
///
/// Deve ser chamada no fim de temporada **depois do decaimento**, pelo mesmo motivo da
/// regra de companheiros. Com ela rodando antes, a faixa de entrada saía do decaimento
/// em percebida 3.9 e era apagada na hora (o limiar de extinção é 5.0) — o harness
/// mediu `pista = 0` em dez das doze temporadas antes da correção.
pub fn process_track_season_rivalry(conn: &Connection, temporada: i32) -> Result<(), DbError> {
    use std::collections::HashMap;

    // Um piloto de lmp2 tem linha no calendário regular DELE e no da Endurance, então
    // o mesmo par pode aparecer em duas categorias. Somar as duas contaria o mesmo
    // domingo duas vezes; fica a categoria em que a dupla mais se encontrou.
    let mut melhor_por_par: HashMap<(String, String), (String, i64)> = HashMap::new();

    let linhas: Vec<(String, String, String, i64)> = {
        let mut stmt = conn.prepare(
            "SELECT c.categoria, a.piloto_id, b.piloto_id, COUNT(*) AS adjacentes
             FROM race_results a
             INNER JOIN race_results b
                     ON b.race_id = a.race_id AND b.piloto_id > a.piloto_id
             INNER JOIN calendar c ON c.id = a.race_id
             INNER JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
             WHERE s.numero = ?1
               AND a.dnf = 0 AND b.dnf = 0
               AND a.posicao_final > 0 AND b.posicao_final > 0
               AND ABS(a.posicao_final - b.posicao_final) = 1
             GROUP BY c.categoria, a.piloto_id, b.piloto_id
             HAVING COUNT(*) >= ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![temporada, PISTA_MIN_ADJACENCIAS], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    for (categoria, p1, p2, adjacentes) in linhas {
        let entrada = melhor_por_par
            .entry((p1, p2))
            .or_insert_with(|| (categoria.clone(), 0));
        if adjacentes > entrada.1 {
            *entrada = (categoria, adjacentes);
        }
    }

    for ((p1, p2), (categoria, adjacentes)) in melhor_por_par {
        let (h_delta, r_delta) = if adjacentes >= PISTA_ADJACENCIAS_DUELO {
            (5.0, 13.0)
        } else {
            (3.0, 9.0)
        };

        let applied = apply_rivalry_event(
            conn,
            &RivalryEvent {
                piloto_a: p1.clone(),
                piloto_b: p2.clone(),
                tipo: RivalryType::Pista,
                historical_delta: h_delta,
                recent_delta: r_delta,
                temporada,
            },
        )?;

        if crossed_threshold(applied.old_perceived, applied.new_perceived).is_some() {
            let nome_a = get_driver(conn, &p1)
                .map(|d| d.nome)
                .unwrap_or_else(|_| p1.clone());
            let nome_b = get_driver(conn, &p2)
                .map(|d| d.nome)
                .unwrap_or_else(|_| p2.clone());
            let driver_midia = load_rivalry_driver_midia(conn, &p1, &p2);

            persist_rivalry_news(
                conn,
                &applied,
                &RivalryType::Pista,
                &nome_a,
                &nome_b,
                &categoria,
                temporada,
                0, // sem rodada: o fato é o placar do ano
                &p1,
                &p2,
                None,
                &driver_midia,
            )?;
        }
    }

    Ok(())
}

// ── Passo 7: Rivalidade por disputa de campeonato ─────────────────────────────

/// Teto do peso da disputa de campeonato — mais contido que o da colisão, porque este
/// gatilho dispara em TODA rodada da janela final, e não num incidente isolado.
const PESO_DISPUTA_MAX: f64 = 2.5;

/// Quanto pesa uma disputa de campeonato pelo que ela tem de aperto.
///
/// `gap` é a diferença de pontos entre os dois; `pela_lideranca` marca o par que
/// envolve o primeiro colocado; `rodada`/`total_rounds` situam na temporada.
pub fn peso_da_disputa(gap: f64, pela_lideranca: bool, rodada: i32, total_rounds: i32) -> f64 {
    let mut peso: f64 = 1.0;

    // Empate técnico (≤ 5 pontos, menos que uma posição de chegada na maioria das
    // tabelas) é briga de verdade; 20 pontos é vantagem administrável.
    if gap <= 5.0 {
        peso *= 1.6;
    } else if gap <= 12.0 {
        peso *= 1.25;
    }
    if pela_lideranca {
        peso *= 1.3;
    }
    // A última rodada decide; as duas antes só encaminham.
    if total_rounds > 0 && rodada >= total_rounds {
        peso *= 1.5;
    }

    peso.min(PESO_DISPUTA_MAX)
}

/// Detecta disputas apertadas nas últimas rodadas e reforça rivalidades entre líderes.
///
/// Deltas (Passo 13): historical=4, recent=10 → percebido ≈7.6
/// Política: só últimas 3 rodadas, só top-3, gap ≤ 20 pontos.
pub fn process_championship_rivalry(
    conn: &Connection,
    categoria_id: &str,
    rodada_atual: i32,
    total_rounds: i32,
    temporada: i32,
) -> Result<(), DbError> {
    if rodada_atual < total_rounds - 2 {
        return Ok(());
    }

    use crate::db::queries::drivers::get_drivers_by_category;

    let mut drivers = get_drivers_by_category(conn, categoria_id)?;
    drivers.sort_by(|a, b| {
        b.stats_temporada
            .pontos
            .partial_cmp(&a.stats_temporada.pontos)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    drivers.truncate(3);

    for i in 0..drivers.len() {
        for j in (i + 1)..drivers.len() {
            let gap = (drivers[i].stats_temporada.pontos - drivers[j].stats_temporada.pontos).abs();
            if gap > 20.0 {
                continue;
            }

            // MESMO PRINCÍPIO DO PESO DE CONTEXTO: 18 pontos de vantagem na antepenúltima
            // rodada e empate técnico na última não são a mesma disputa, e até aqui
            // valiam igual. `1º x 2º` empatado na decisão pesa o dobro de um 2º x 3º
            // com folga — que é a diferença entre uma briga de título e uma tabela.
            let peso = peso_da_disputa(
                gap,
                drivers[i].id == drivers[0].id,
                rodada_atual,
                total_rounds,
            );

            let applied = apply_rivalry_event(
                conn,
                &RivalryEvent {
                    piloto_a: drivers[i].id.clone(),
                    piloto_b: drivers[j].id.clone(),
                    tipo: RivalryType::Campeonato,
                    historical_delta: 4.0 * peso,
                    recent_delta: 10.0 * peso,
                    temporada,
                },
            )?;

            if crossed_threshold(applied.old_perceived, applied.new_perceived).is_some() {
                let driver_midia = load_rivalry_driver_midia(conn, &drivers[i].id, &drivers[j].id);
                persist_rivalry_news(
                    conn,
                    &applied,
                    &RivalryType::Campeonato,
                    &drivers[i].nome,
                    &drivers[j].nome,
                    categoria_id,
                    temporada,
                    rodada_atual,
                    &drivers[i].id,
                    &drivers[j].id,
                    None,
                    &driver_midia,
                )?;
            }
        }
    }

    Ok(())
}

// ── Passo 15: Mapeamento Factual de Colisão ───────────────────────────────────

/// UM TOQUE LEVE NÃO ABRE FICHA. A faixa mais baixa da colisão (severidade menor, menos
/// de 3 posições perdidas) só REFORÇA rivalidade que já existe — sozinha, ela não cria.
///
/// Medido: num mundo de 12 temporadas, 43 das 83 rivalidades vinham de colisão, e a
/// esmagadora maioria de um encostão isolado que vale percebida ~5. Elas nunca somem
/// e nunca crescem; só empilham no piso. E isso tem consequência direta na tela — a
/// aba Rivais mostra os 4 maiores por percebida, e quando tudo está entre 5 e 15 esses
/// quatro saem praticamente por sorteio, que era a queixa original de que a aba não
/// tinha vida.
///
/// A regra diz o que se quer dizer: o primeiro encostão é corrida; o encostão entre
/// dois que já têm história é capítulo. Toque grave, abandono e perda de 3+ posições
/// continuam criando do zero — esses são incidentes, não roçada de roda.
const COLISAO_LEVE: (f64, f64) = (2.0, 8.0);

/// Peso de contexto a partir do qual um toque leve abre ficha mesmo sem história.
///
/// Medido: com o portão em `peso > 1.0` a limpeza da colisão foi desfeita — as
/// rivalidades de colisão voltaram de 12 para 96 num mundo de 12 temporadas, porque
/// estar na zona de pontos sozinho já dá ×1.3 e a maioria dos encostões acontece ali.
/// "Havia algo em jogo" não pode significar "estava no top 8".
///
/// Em 1.5 o corte separa o que se quer separar: top-8 sozinho (1.3) continua barrado;
/// largar na ponta (1.8), pontos na reta final (1.82) e qualquer coisa envolvendo os
/// dois candidatos ao título (≥1.6) passam.
const COLISAO_LEVE_PESO_MINIMO: f64 = 1.5;

// ── Peso do contexto: o que estava em jogo ───────────────────────────────────

/// Teto do multiplicador. Sem ele, os fatores se compõem sem limite e um evento
/// vira a rivalidade inteira de uma vez.
const PESO_MAX: f64 = 4.0;
/// Os dois largaram na ponta — a batida foi pela vitória.
const PESO_PONTA: f64 = 1.8;
/// Zona de pontos: não é a briga da frente, mas também não é fundo de grid.
const PESO_TOP: f64 = 1.3;
/// Últimas três rodadas — a mesma janela em que o campeonato já vira rivalidade.
const PESO_FIM_DE_ANO: f64 = 1.4;
/// Os dois no top-3 do campeonato: o toque mexeu no título.
const PESO_TITULO: f64 = 1.6;

/// Quanto o contexto multiplica um evento de rivalidade.
///
/// O SISTEMA INTEIRO TRATAVA TODO EVENTO COMO IGUAL, e é isso que impedia rivalidade
/// grande de existir. Uma colisão entre o 14º e o 15º na rodada 4 valia exatamente o
/// mesmo que uma colisão entre os dois líderes do campeonato na última corrida do ano.
/// Como nenhum evento passava de percebida 14 e o portão do Nemesis é 40, uma rivalidade
/// só podia nascer de acúmulo lento — e acúmulo lento é o que o mundo dissolve, porque
/// as duplas se separam.
///
/// Com o peso, o momento decisivo existe: uma batida crítica entre dois candidatos ao
/// título largando na frente na última rodada chega perto de 46 de percebida sozinha —
/// vira Nemesis na hora, que é exatamente o que deveria acontecer.
///
/// `grid_dos_dois` é a PIOR das duas largadas, não a melhor. A diferença não é
/// detalhe: com a melhor, um líder tocando num retardatário na volta 30 pontuava
/// como batida pela liderança, porque um deles largou na frente. O que a regra quer
/// dizer é que **os dois** estavam brigando ali, e isso só o pior dos dois garante.
/// `0` ou negativo significa grid desconhecido e não pontua.
pub fn peso_do_contexto(
    grid_dos_dois: i32,
    rodada: i32,
    total_rounds: i32,
    ambos_na_briga_do_titulo: bool,
) -> f64 {
    let mut peso: f64 = 1.0;

    if grid_dos_dois > 0 && grid_dos_dois <= 3 {
        peso *= PESO_PONTA;
    } else if grid_dos_dois > 0 && grid_dos_dois <= 8 {
        peso *= PESO_TOP;
    }
    if total_rounds > 0 && rodada > total_rounds - 3 {
        peso *= PESO_FIM_DE_ANO;
    }
    if ambos_na_briga_do_titulo {
        peso *= PESO_TITULO;
    }

    peso.min(PESO_MAX)
}

/// Os três primeiros do campeonato da categoria — quem tem o título em jogo.
fn top3_do_campeonato(
    conn: &Connection,
    categoria_id: &str,
) -> Result<std::collections::HashSet<String>, DbError> {
    use crate::db::queries::drivers::get_drivers_by_category;

    let mut drivers = get_drivers_by_category(conn, categoria_id)?;
    drivers.sort_by(|a, b| {
        b.stats_temporada
            .pontos
            .partial_cmp(&a.stats_temporada.pontos)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(drivers
        .into_iter()
        .take(3)
        .map(|driver| driver.id)
        .collect())
}

pub fn process_collisions_rivalry(
    conn: &Connection,
    incidents: &[crate::simulation::incidents::IncidentResult],
    race_results: &[crate::simulation::race::RaceDriverResult],
    categoria_id: &str,
    rodada: i32,
    total_rounds: i32,
    temporada: i32,
) -> Result<(), DbError> {
    use crate::simulation::incidents::{IncidentSeverity, IncidentType};
    use std::collections::HashMap;

    let grid: HashMap<&str, i32> = race_results
        .iter()
        .map(|r| (r.pilot_id.as_str(), r.grid_position))
        .collect();
    let briga_do_titulo = top3_do_campeonato(conn, categoria_id)?;

    // Base do evento + peso do contexto, guardados separados: o portão da colisão leve
    // olha a BASE (foi um encostão?) e o peso decide o tamanho (encostão onde?).
    let mut collision_pairs: HashMap<(String, String), ((f64, f64), f64)> = HashMap::new();

    for inc in incidents {
        if inc.incident_type == IncidentType::Collision {
            if let Some(linked_id) = &inc.linked_pilot_id {
                let Some(pair) = normalize_pair(&inc.pilot_id, linked_id) else {
                    continue;
                };
                let p1 = pair.piloto1_id;
                let p2 = pair.piloto2_id;

                let (h, r) = if inc.severity == IncidentSeverity::Critical {
                    (7.0, 18.0)
                } else if inc.is_dnf {
                    (5.0, 14.0)
                } else if inc.severity == IncidentSeverity::Major || inc.positions_lost >= 3 {
                    (3.0, 10.0)
                } else {
                    COLISAO_LEVE
                };

                // A PIOR das duas largadas: só assim "top 3" significa que os dois
                // estavam na briga da frente, e não que um deles era o líder.
                // Largada ausente nos dois lados (save antigo) → 0, sem peso.
                let grids: Vec<i32> = [grid.get(p1.as_str()), grid.get(p2.as_str())]
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|posicao| *posicao > 0)
                    .collect();
                let grid_dos_dois = if grids.len() == 2 {
                    grids.into_iter().max().unwrap_or(0)
                } else {
                    0
                };
                let peso = peso_do_contexto(
                    grid_dos_dois,
                    rodada,
                    total_rounds,
                    briga_do_titulo.contains(&p1) && briga_do_titulo.contains(&p2),
                );

                let current = collision_pairs.entry((p1, p2)).or_insert(((0.0, 0.0), 1.0));
                // O par fica com o incidente mais grave do dia, e com o peso dele.
                if h > current.0 .0 {
                    *current = ((h, r), peso);
                }
            }
        }
    }

    for ((p1, p2), ((h, r), peso)) in collision_pairs {
        // A faixa leve não abre ficha: se ainda não há rivalidade entre os dois, o
        // encostão foi só corrida e não deixa registro. Sobre história existente,
        // conta normalmente.
        //
        // MAS SÓ QUANDO HAVIA MESMO ALGO EM JOGO. Um toque de leve entre os dois
        // candidatos ao título na última rodada não é roçada de roda — é o toque do
        // ano. Estar na zona de pontos, não: ver `COLISAO_LEVE_PESO_MINIMO`.
        if (h, r) == COLISAO_LEVE
            && peso < COLISAO_LEVE_PESO_MINIMO
            && crate::db::queries::rivalries::get_rivalry_by_pair(conn, &p1, &p2)?.is_none()
        {
            continue;
        }

        let applied = apply_rivalry_event(
            conn,
            &RivalryEvent {
                piloto_a: p1.clone(),
                piloto_b: p2.clone(),
                tipo: RivalryType::Colisao,
                historical_delta: h * peso,
                recent_delta: r * peso,
                temporada,
            },
        )?;

        if crossed_threshold(applied.old_perceived, applied.new_perceived).is_some() {
            let nome_a = get_driver(conn, &p1)
                .map(|d| d.nome)
                .unwrap_or_else(|_| p1.clone());
            let nome_b = get_driver(conn, &p2)
                .map(|d| d.nome)
                .unwrap_or_else(|_| p2.clone());
            let driver_midia = load_rivalry_driver_midia(conn, &p1, &p2);

            persist_rivalry_news(
                conn,
                &applied,
                &RivalryType::Colisao,
                &nome_a,
                &nome_b,
                categoria_id,
                temporada,
                rodada,
                &p1,
                &p2,
                None,
                &driver_midia,
            )?;
        }
    }

    Ok(())
}
