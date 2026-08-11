//! Geração da AI season (calendário do iRacing) e a escada de dificuldade por tier.

use super::*;

/// Resultado da geração de AI season.
#[derive(serde::Serialize)]
pub struct SeasonGenResult {
    pub path: String,
    pub name: String,
    pub events: usize,
    /// Pista mirada para a margem de skill (nome). Auto = próxima corrida do
    /// calendário; ou a do override manual.
    pub targeted_track: Option<String>,
    /// Skill efetivo da ponta da IA (maxSkill aplicado) para a pista mirada.
    pub ai_skill: i64,
    /// true se a pista veio do calendário (próxima corrida); false se foi override.
    pub auto_targeted: bool,
}

/// Ponto de dificuldade (SWEET SPOT) base por tier — NÃO é um teto rígido. É o nível
/// efetivo que a ponta da IA deve correr pra dar a dificuldade IDEAL daquele tier, na
/// pista de referência. A ponta fica competitiva mas BATÍVEL, deixando margem pro
/// jogador (decisão do user). Sweet spot final da pista = este base + offset da pista.
/// Acima de rookie é progressivamente mais difícil (sweet spot maior).
/// Degrau FIXO abaixo do sweet spot no rookie (tier 0), seja qual for a pista. Rookie é a
/// categoria de iniciante: rebaixamos SEMPRE este tanto de pontos, independente do offset da
/// pista (Rudskogen 62→54, Oulton 68→60, VIR 95→87). Só o rookie recebe; as outras divisões
/// ficam iguais. Aplicado sobre o sweet spot final — pode descer abaixo do baseline do tier.
const ROOKIE_DIFFICULTY_DISCOUNT: i64 = 8;

fn tier_difficulty_base(tier: u8) -> i64 {
    // ESCADA ACHATADA (frente efetiva). A diferença entre divisões é pequena de propósito:
    // no iRacing o CARRO já faz a divisão de cima ser mais rápida; o driverSkill% só precisa
    // subir um pouco. Isso deixa MARGEM (até 125) pro offset de pista + adaptativo + futuro
    // efeito de carro, e a dificuldade real mora no adaptativo (que se estica pra quem é bom).
    // Escada REBAIXADA em 10 pontos em 10/08/2026 (base anterior 72–84): a dificuldade de
    // partida estava alta demais. Os sweet spots citados nos comentários de calibração de
    // `track_skill_offset` foram medidos na escada antiga (some 10 pra comparar).
    match tier {
        0 => 62, // Rookie   (base baixa = "rodinhas"; o adaptativo sobe se dominar)
        1 => 69, // Amador
        2 => 71, // Pro / Production / BMW
        3 => 72, // GT4
        4 => 73, // GT3
        5 => 74, // LMP2
        _ => 74, // Endurance / Elite
    }
}

/// Amortecimento do delta GLOBAL por tier. O `global` é por-jogador e compartilhado entre
/// divisões/carreiras; sem isto, um alien acumularia +40 na elite e o rookie da carreira
/// nova viraria absurdo. Aqui o boost pesa menos nas divisões baixas: o alien sente a elite
/// cheia, mas o rookie continua sendo rookie.
fn tier_difficulty_damp(tier: u8) -> f64 {
    match tier {
        0 => 0.50, // Rookie
        1 => 0.70, // Amador
        2 => 0.85, // Pro
        3 => 0.90, // GT4
        4 => 0.95, // GT3
        _ => 1.00, // LMP2 / Endurance
    }
}

/// Sweet spot da ponta da IA (efetivo do melhor) ANTES da penalidade de chuva: base do
/// tier + offset da pista + perfil adaptativo do jogador (global amortecido por tier). É a
/// ÂNCORA da curva de skill — a season (banda) e o roster (skill por piloto) chamam o
/// MESMO valor pra a forma e o cap da cauda baterem dos dois lados.
pub(crate) fn ai_sweet_spot(
    tier: u8,
    track_id: Option<i64>,
    base_dir: &std::path::Path,
    custid: i64,
) -> i64 {
    let track_offset = track_id.map(track_skill_offset).unwrap_or(0);
    let profile = load_adaptive_profile(base_dir, custid);
    let adapt_track = track_id.map(|id| profile.track_delta(id)).unwrap_or(0);
    // Boost global amortecido por tier (não infla as divisões baixas — ver tier_difficulty_damp).
    let global_eff = (profile.global as f64 * tier_difficulty_damp(tier)).round() as i64;
    // Rookie (tier 0) rebaixa o sweet spot um degrau FIXO, seja qual for a pista.
    let rookie_discount = if tier == 0 {
        ROOKIE_DIFFICULTY_DISCOUNT
    } else {
        0
    };
    (tier_difficulty_base(tier) + track_offset + global_eff + adapt_track - rookie_discount)
        .clamp(0, 125)
}

/// Vantagens de carro (car-perf) do CAMPO e do JOGADOR na pista alvo, para a inversão
/// carro→dificuldade (Sistema de Nível do Carro). Mapeia cada piloto de IA → time → carro
/// (`team_car`); carro ausente ou rookie spec → vantagem 0. Devolve
/// `(vantagem_do_jogador, vantagens_da_ia, mapa piloto→vantagem)`. Cache por time (os
/// companheiros dividem o mesmo carro). Usado pela season (banda) e pelo roster (banda +
/// spread) com a MESMA fonte, pra os dois lados baterem sob o esticão do iRacing.
pub(crate) fn field_car_advantages(
    conn: &rusqlite::Connection,
    categoria: &str,
    player_team_id: Option<&str>,
    track_id: i64,
) -> (f64, Vec<f64>, std::collections::HashMap<String, f64>) {
    use crate::car::sim_bridge::car_advantage;
    use crate::db::queries::{contracts as cq, drivers as dq, team_car as tcq};
    use crate::simulation::track_profile::get_track_simulation_data;
    use std::collections::HashMap;

    let tsd = get_track_simulation_data(track_id as u32);
    let track = (
        tsd.acceleration_weight,
        tsd.power_weight,
        tsd.handling_weight,
    );

    let load = |team_id: &str, cache: &mut HashMap<String, f64>| -> f64 {
        if let Some(v) = cache.get(team_id) {
            return *v;
        }
        let v = tcq::get_team_car(conn, team_id)
            .ok()
            .flatten()
            .map(|car| car_advantage(&car, track))
            .unwrap_or(0.0);
        cache.insert(team_id.to_string(), v);
        v
    };

    let mut cache: HashMap<String, f64> = HashMap::new();
    let player_adv = player_team_id.map(|t| load(t, &mut cache)).unwrap_or(0.0);

    let mut ai_advs = Vec::new();
    let mut per_ai = HashMap::new();
    for d in dq::get_drivers_by_category(conn, categoria).unwrap_or_default() {
        if d.is_jogador {
            continue;
        }
        let team = cq::get_active_contract_for_pilot(conn, &d.id)
            .ok()
            .flatten()
            .map(|c| c.equipe_id);
        let adv = team.as_deref().map(|t| load(t, &mut cache)).unwrap_or(0.0);
        ai_advs.push(adv);
        per_ai.insert(d.id, adv);
    }
    (player_adv, ai_advs, per_ai)
}

/// Offset de skill por PISTA (a "margem por pista"). A IA rende diferente em cada
/// circuito para o mesmo skill% (no Rudskogen efetivo 73 → 1:36.15; no Lédenon o
/// mesmo 73 → 1:36.95, ~0,8s mais lenta). Então cada pista soma/subtrai do sweet spot
/// base do tier pra acertar a dificuldade ideal. Calibrado em corrida real; default 0.
/// No fluxo de reexportar antes de cada corrida, recebe a pista daquela corrida.
/// VALORES calibrados na escada ANTIGA (rookie sweet spot = 73 + offset); a escada atual
/// está 10 pontos abaixo (ver `tier_difficulty_base`) — preencher conforme o user for testando:
///
/// **CALIBRAÇÃO PENDENTE, e ela é o item mais caro desta tabela.** Cada offset abaixo foi
/// medido contra uma base de tier 10 pontos mais alta, e o rebaixamento de 10/08/2026
/// mexeu na base sem revalidar nenhum offset. O que está no ar hoje é offset velho somado
/// a baseline nova: em pista já calibrada, o sweet spot efetivo caiu 10 pontos em relação
/// ao que foi medido na pista. Corrigir de gabinete seria escolher número no chute — cada
/// entrada precisa de uma corrida de verdade na pista, que é como todas as outras
/// nasceram. Até lá a dificuldade parte mais fácil do que o medido, o que é o lado seguro
/// de errar, e o adaptativo por custid tende a fechar a diferença ao longo das etapas.
///
/// A tabela também só cresce por edição manual de código: pista nova cai no default 0.
fn track_skill_offset(track_id: i64) -> i64 {
    match track_id {
        // Lédenon: no sweet spot 83 a ponta (Alvarez 1:35.946) EMPATOU com o jogador
        // 1500iR (1:35.941). Pra rookie, recuamos 1 ponto → sweet spot 82 (offset +9),
        // deixando uma pitada de margem pro jogador.
        489 => 9,
        // Navarra: sweet spot 81. Calibrado no 515 (Speed Circuit 3,9 km) por ritmo de
        // corrida — a pista "engole" a IA (teto de ~1:58.8 em ar limpo, best-lap engana
        // por tráfego). 516 (Medium 3,4 km) usa o mesmo valor: traçado quase idêntico,
        // não vale testar à parte (decisão do user).
        515 | 516 => 8,
        // Lime Rock Park - Grand Prix (353, ~1:00 a volta, pista curta): sweet spot 81.
        353 => 8,
        // Lime Rock Park - Classic (352) + Chicanes (354): mesmo venue. User mandou herdar
        // o valor do Lime Rock (sweet spot 81) sem teste à parte.
        352 | 354 => 8,
        // Motorsport Arena Oschersleben (449 GP / 454 Alt / 455 B Course): sweet spot 82.
        // Aplicado nos 3 layouts do venue. Obs.: o B Course é mais curto na vida real —
        // se sentir diferente, a gente separa o 455 depois.
        449 | 454 | 455 => 9,
        // Okayama (166 full 3,7 km / 167 Short 2,4 km): sweet spot 80. User mandou o
        // mesmo valor nos dois layouts livres. (o Short duplicado 542 foi removido do catálogo.)
        166 | 167 => 7,
        // Oran Park Raceway (202 GP 2,6 km / 208 South 2,0 km): sweet spot 74 — quase
        // baseline, IA já competitiva com pouco skill (igual Rudskogen). Mesmo valor nos 2.
        202 | 208 => 1,
        // Oulton Park - International (180, 4.4 km) + variações da Intl: 183 w/out Hislop,
        // 184 w/out Brittens, 185 w/no Chicanes. Sweet spot 79 nos 4 layouts livres da
        // família Intl. (a Intl duplicada 342 foi removida; Fosters/Island não são variação da Intl.)
        180 | 183 | 184 | 185 => 6,
        // Oulton Park - Fosters (181), Island (182): layouts não-Intl do mesmo venue.
        // User mandou herdar o valor do Oulton (sweet spot 79) sem teste à parte.
        181 | 182 => 6,
        // Snetterton Circuit - 300 (297, 4.8 km) + 200 (298, 3.2 km): sweet spot 82.
        // User mandou o mesmo valor nos dois layouts livres.
        297 | 298 => 9,
        // Summit Point - Summit Point Raceway (9, 3.2 km): sweet spot 97 (offset +24).
        // Pista com "macetes" que humanos usam e a IA não pega — mesmo em 95% a IA fica
        // fora do ritmo esperado. Folga OK: o teto real do iRacing é 125%, não 100%.
        // (Jefferson 8 é layout diferente/curto, fora dos pools — não coberto aqui.)
        9 => 24,
        // Tsukuba Circuit - 2000 Full (324, 2.0 km): sweet spot 82. Único layout livre.
        324 => 9,
        // Winton Motor Raceway - National (439, 3.0 km) + Club (440, 2.0 km): sweet spot 80.
        // Mesmo valor nos dois layouts livres do venue.
        439 | 440 => 7,
        // Charlotte Motor Speedway - Roval (554, 3.7 km, versão 2025): sweet spot 86.
        554 => 13,
        // Virginia Int'l Raceway - Full Course (465, 5.3 km) + Grand Course (466, 6.8 km):
        // sweet spot 106 (offset +33). Passa de 100 — só funciona com o clamp(0,125).
        465 | 466 => 33,
        // VIR - North Course (467, 3.6 km): sweet spot 100 (offset +27). (Patriot 259 = pago.)
        467 => 27,
        // Rudskogen Motorsenter (451): pista BASELINE — rookie sweet spot 73 (offset 0),
        // validado em corrida real. Explícito só pra documentar (mesmo valor do default).
        451 => 0,
        _ => 0, // demais: baseline default até calibrar a pista
    }
}

/// `minSkill`/`maxSkill` do arquivo de temporada, a partir da banda absoluta que o roster
/// deixou no post-it. Este é o CONSUMIDOR de `normalize_to_roster` — o outro lado do
/// contrato, e é por isso que os casos degenerados dele são testados aqui.
///
/// A banda chega em ponto flutuante e sai como os dois inteiros que o iRacing aceita
/// (0 a 125). Os degenerados são convenção do lado que gera e precisam envelhecer junto:
/// grid de um piloto (ou empate exato de skill) chega como `min` e `min + 1`, então a
/// faixa nunca sai com `min == max` — que é o valor pelo qual o esticão do iRacing divide.
/// O `min` é limitado pelo `max` JÁ arredondado, para que o arredondamento não produza um
/// `minSkill` acima do `maxSkill`, que é arquivo inválido para o sim.
pub(crate) fn limites_da_banda(min: f64, max: f64) -> (i64, i64) {
    let max = (max.round() as i64).clamp(0, 125);
    ((min.round() as i64).clamp(0, max), max)
}

/// Gera a **AI season** (calendário) da categoria, espelhando o exemplo do
/// usuário: lê o calendário da carreira (track_ids já são do iRacing), filtra
/// pistas grátis, usa a duração da categoria e o clima do calendário. Aponta para
/// o roster `roster_name`. Sai em `aiseasons/<série> - <ano>.json`.
/// `target_track_id` (opcional) = a pista da corrida que vai ser disputada: aplica
/// a margem por pista no teto de skill. None → só o teto do tier (sem offset).
///
/// **Escreve no save.** O nome promete gerar um arquivo, e ele também faz um `UPDATE` em
/// `calendar` gravando clima e temperatura de cada etapa. É deliberado: a história do
/// clima é determinística e precisa de UMA fonte, senão a UI, a simulação offline e o que
/// o iRacing roda divergem. Fica declarado aqui porque um efeito colateral escondido num
/// export é exatamente o tipo de coisa que ninguém procura quando o clima não bate.
#[tauri::command]
pub fn iracing_generate_season(
    app: tauri::AppHandle,
    career_id: String,
    categoria: String,
    roster_name: String,
    car_key: String,
    target_track_id: Option<i64>,
    // Modo TESTE: aiseason "zerado" (sem resultados) com a corrida 1 usando o clima
    // roteirizado da 1ª corrida — pra visualizar o roteiro no menu do iRacing.
    test_blank: Option<bool>,
    // TESTE: força a PRÓXIMA corrida pendente como molhada (chuva forte).
    force_wet: Option<bool>,
) -> Result<SeasonGenResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::constants::categories::get_category_config;
    use crate::constants::tracks::{free_or_substitute, get_track};
    use crate::db::connection::Database;
    use crate::db::queries::calendar as calq;
    use crate::db::queries::{drivers as dq, race_history as rhq, seasons as sq};
    use crate::iracing_sdk::{paths, results_gen, roster_gen, season_gen};
    use tauri::Manager;

    let car =
        roster_gen::car_spec(&car_key).ok_or_else(|| format!("Carro desconhecido: {car_key}"))?;
    let cat = get_category_config(&categoria)
        .ok_or_else(|| format!("Categoria desconhecida: {categoria}"))?;

    // Abre o banco e pega a temporada ativa.
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;
    let season = sq::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao ler temporada: {e}"))?
        .ok_or("Nenhuma temporada ativa nesta carreira.")?;

    // Calendário da categoria → eventos (filtra pistas pagas; track_id já é do iRacing).
    let mut entries = calq::get_calendar(&db.conn, &season.id, &categoria)
        .map_err(|e| format!("Falha ao ler calendário: {e}"))?;
    entries.sort_by_key(|e| e.rodada);

    // Números fixos por piloto IA (mesmos do roster) — usados nos resultados das etapas.
    let ai_driver_ids: Vec<String> = dq::get_drivers_by_category(&db.conn, &categoria)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| !d.is_jogador)
        .map(|d| d.id)
        .collect();
    let numbers = ensure_driver_numbers(&base_dir, &career_id, &ai_driver_ids).unwrap_or_default();

    let mut events = Vec::new();
    // Mapa evento→corrida da carreira, NA MESMA ORDEM dos eventos escritos no
    // aiseason. É o "post-it" que o import usa para achar o resultado certo:
    // events[i] no JSON ↔ event_race_map[i] = (race_id, track_id).
    let mut event_race_map: Vec<(String, i64)> = Vec::new();
    // Clima do calendário → bloco weather DINÂMICO (timeline) de cada etapa.
    // Escala real do iRacing:
    //   skies:       0 Limpo · 1 Parcialmente · 2 Predominantemente · 3 Encoberto
    //   track_water: 0 Nenhum … 5 Muito intenso
    //   keyframes event_type: 0 Limpo … 7 Chuva · 8 Chuva intensa
    // Por ora a timeline só "segura" a condição da etapa (a carreira ainda não
    // modela evolução); a ESTRUTURA dinâmica fica provada e pronta para evoluir.
    let custid = iracing_sdk::cached_custid().unwrap_or(0);
    let race_end = cat.duracao_corrida_min as i64;
    // 1ª corrida do save = nenhuma etapa concluída ainda (roteiro especial do clima).
    let career_first_race = entries
        .iter()
        .all(|e| !matches!(e.status, crate::models::enums::RaceStatus::Concluida));
    let first_week = entries
        .iter()
        .map(|e| e.week_of_year)
        .min()
        .unwrap_or(i32::MAX);
    // Modo TESTE (zerado): força o roteiro da 1ª corrida na etapa 1 (mesmo que a carreira
    // já tenha avançado) e omite resultados. Corridas 2+ ficam com o clima variado normal.
    let test_blank = test_blank.unwrap_or(false);
    let first_race_id = entries
        .iter()
        .min_by_key(|e| e.rodada)
        .map(|e| e.id.clone());
    // TESTE: força a PRÓXIMA corrida pendente (não concluída) como molhada.
    let force_wet = force_wet.unwrap_or(false);
    let next_pending_id = entries
        .iter()
        .filter(|e| !matches!(e.status, crate::models::enums::RaceStatus::Concluida))
        .min_by_key(|e| e.rodada)
        .map(|e| e.id.clone());

    // Clima + horário gerados por pista+estação (determinístico por etapa). Guarda a
    // história de cada ETAPA (chave = id da corrida) para a penalidade da chuva na banda.
    // Chaveava por PISTA: calendário que repete a mesma pista sobrescrevia a história e a
    // penalidade da banda podia sair da rodada errada — molhada onde a corrida é seca, ou
    // o contrário. O valor guarda o track_id só para o alvo manual de teste.
    let mut stories: std::collections::HashMap<
        String,
        (i64, crate::iracing_sdk::weather::WeatherStory),
    > = std::collections::HashMap::new();
    let mut substituted = 0;
    for entry in &entries {
        // Fallback de TESTE: se a pista do calendário é conteúdo PAGO (que o jogador
        // pode não possuir), roda numa pista GRÁTIS no lugar — o iRacing só carrega
        // pistas que o jogador tem. Pista já grátis passa intacta. A banda de skill /
        // sweet spot continua ancorada no `entry.track_id` ORIGINAL (alinhada com o
        // roster); só o que o iRacing carrega (EventInput/import) vira a free.
        let Some(track) = free_or_substitute(entry.track_id) else {
            continue;
        };
        {
            if track.track_id != entry.track_id {
                substituted += 1;
            }
            let is_first = (career_first_race && entry.week_of_year == first_week)
                || (test_blank && first_race_id.as_deref() == Some(entry.id.as_str()));
            let wet_here = force_wet && next_pending_id.as_deref() == Some(entry.id.as_str());
            // Etapa noturna designada pelo calendário (≥1 corrida de noite/temporada).
            let night_here = crate::calendar::is_night_horario(&entry.horario);
            let seed = event_seed(&career_id, &entry.id);
            let (ew, story) = build_event_weather(
                track,
                entry.week_of_year,
                season.ano,
                cat.tier,
                custid,
                seed,
                is_first,
                race_end,
                wet_here,
                night_here,
            );
            // FONTE ÚNICA: persiste clima E temperatura desta MESMA história, pra a
            // UI e a simulação offline baterem com o que o iRacing vai rodar (e a
            // temp nunca destoar da chuva real).
            let wc = story_to_weather_condition(&story);
            // Sim, um comando chamado "gerar temporada" ESCREVE no calendário do save. É de
            // propósito (fonte única de clima), e está declarado na doc do comando; o que
            // não pode é a escrita falhar calada e a UI passar a mostrar um clima que o
            // iRacing não vai rodar.
            if let Err(e) = db.conn.execute(
                "UPDATE calendar SET clima = ?1, temperatura = ?2 WHERE id = ?3",
                rusqlite::params![wc.as_str(), ew.temp_c as f64, entry.id],
            ) {
                crate::diagnostico::linha(
                    "iracing",
                    &format!("Falha ao persistir o clima da etapa {}: {e}", entry.id),
                );
            }
            stories.insert(entry.id.clone(), (entry.track_id as i64, story));
            // Etapa já disputada no app → escreve os resultados (iRacing "pula").
            // No modo teste (zerado) nunca escreve resultados.
            let results = if !test_blank
                && matches!(entry.status, crate::models::enums::RaceStatus::Concluida)
            {
                rhq::get_event_results(&db.conn, &entry.id)
                    .ok()
                    .filter(|r| !r.is_empty())
                    .map(|rows| {
                        let drivers: Vec<results_gen::ResultDriver> = rows
                            .into_iter()
                            .map(|r| {
                                let num = numbers.get(&r.piloto_id).copied().unwrap_or(0);
                                results_gen::ResultDriver {
                                    finish: r.finish,
                                    start: r.start,
                                    laps: r.laps,
                                    total_ms: r.total_ms,
                                    gap_ms: r.gap_ms,
                                    incidents: r.incidents,
                                    dnf: r.dnf,
                                    dnf_reason: r.dnf_reason,
                                    has_fastest: r.has_fastest,
                                    car_number: if r.is_jogador {
                                        "0".to_string()
                                    } else {
                                        num.to_string()
                                    },
                                    cust_id: if r.is_jogador { custid } else { 990_000 + num },
                                    name: r.nome,
                                    car_id: car.car_id,
                                    car_class_id: car.car_class_id,
                                }
                            })
                            .collect();
                        results_gen::build_results(&drivers)
                    })
            } else {
                None
            };
            events.push(season_gen::EventInput {
                // Pista EFETIVA que o iRacing carrega (a free substituta, quando a
                // original é paga). Nenhuma pista free é oval de verdade — Roval
                // (Charlotte) é ROAD no iRacing (paceCar road, sem largada lançada).
                track_id: track.track_id as i64,
                is_oval: false,
                event_id: uuid::Uuid::new_v4().to_string(),
                weather: ew,
                results,
            });
            // Guarda a pista EFETIVA no post-it: o import compara o resultado do
            // iRacing contra o que foi de fato exportado (não contra a original paga).
            event_race_map.push((entry.id.clone(), track.track_id as i64));
        }
    }
    let _ = substituted; // (contagem de substituições — reservado p/ UI/log futuro)
    if events.is_empty() {
        return Err(format!(
            "Calendário da categoria '{categoria}' está vazio — nada para exportar."
        ));
    }

    // Faixa de skill — RÉGUA ASSIMÉTRICA por tier. O iRacing ESTICA a ordem do grid
    // para preencher [minSkill, maxSkill]:
    //   ajustada = minSkill + (skill - menor_do_grid)/(maior - menor) * (maxSkill - minSkill)
    // O melhor do grid sempre vira maxSkill; o pior, minSkill. Usamos isso a favor:
    //   - maxSkill = teto do TIER → o melhor piloto corre nesse nível efetivo. Validado
    //     em pista (corrida real 1500iR em Rudskogen): skill ~73 ≈ pace 1500iR
    //     (competitivo médio). Tier 0 (rookie) = 73 → a FRENTE já é disputada, mesmo
    //     sendo rookie ("vieram do kart"). Tiers acima sobem progressivamente.
    //   - minSkill = o pior piloto REAL do grid → o lanterna continua genuinamente ruim.
    // Resultado: frente puxada pro competitivo, fundo ancorado no ruim de verdade.
    let skills: Vec<f64> = dq::get_drivers_by_category(&db.conn, &categoria)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| !d.is_jogador)
        .map(|d| d.atributos.skill)
        .collect();
    // Pista alvo da margem por pista: override manual (target_track_id, p/ testes) OU,
    // se ausente, a PRÓXIMA corrida pendente do calendário (AUTO-TARGETING). Assim,
    // reexportando antes de cada corrida, a banda sempre reflete a pista que vem.
    let auto_track = if target_track_id.is_none() {
        calq::get_next_race(&db.conn, &season.id, &categoria)
            .ok()
            .flatten()
            .map(|r| r.track_id as i64)
    } else {
        None
    };
    let auto_targeted = auto_track.is_some();
    let resolved_track_id = target_track_id.or(auto_track);
    let targeted_track = resolved_track_id
        .and_then(|id| get_track(id as u32))
        .map(|t| t.nome.to_string());
    // Sweet spot de dificuldade = base do tier + offset da pista. Nível efetivo da
    // ponta da IA (não um teto rígido). Teto 125 = limite real do iRacing (não 100):
    // pistas com "macetes" que a IA não pega (ex.: Summit Point) precisam de offsets
    // altos que passam de 100 nos tiers acima do rookie.
    // Sweet spot do tier na pista alvo (âncora da curva; MESMO valor que o roster usa).
    // Perfil adaptativo por custid entra aqui dentro.
    let base_sweet = ai_sweet_spot(cat.tier, resolved_track_id, &base_dir, custid);
    // Rastro do CONSUMO do perfil adaptativo — a outra ponta do ciclo. O pós-corrida
    // loga "global X → Y"; esta linha prova que o Y gravado foi LIDO no export seguinte
    // e entrou no sweet spot da pista alvo. Sem ela, "gravou mas nunca aplicou" e
    // "aplicou" são indistinguíveis no log.
    {
        let profile = load_adaptive_profile(&base_dir, custid);
        let adapt_track = resolved_track_id
            .map(|id| profile.track_delta(id))
            .unwrap_or(0);
        crate::diagnostico::linha(
            "adaptativo",
            &format!(
                "Export {categoria} (tier {}): pista {} · perfil global {:+} · pista {:+} · sweet spot {base_sweet}",
                cat.tier,
                resolved_track_id.unwrap_or(0),
                profile.global,
                adapt_track
            ),
        );
    }
    // Sistema de Nível do Carro → dificuldade: rebaixa/eleva a BANDA inteira pela vantagem do
    // SEU carro vs a média do campo na pista alvo (o spread por-IA vai no roster). MESMO
    // cálculo que o roster usa, pra os dois baterem sob o esticão do iRacing.
    let player_team_id = dq::get_player_driver(&db.conn)
        .ok()
        .and_then(|p| {
            crate::db::queries::contracts::get_active_contract_for_pilot(&db.conn, &p.id)
                .ok()
                .flatten()
        })
        .map(|c| c.equipe_id);
    let car_band = resolved_track_id
        .map(|tid| {
            let (player_adv, ai_advs, _) =
                field_car_advantages(&db.conn, &categoria, player_team_id.as_deref(), tid);
            crate::iracing_sdk::car_difficulty::band_skill_delta(player_adv, &ai_advs)
        })
        .unwrap_or(0.0);
    let max_skill = ((base_sweet as f64 + car_band).round() as i64).clamp(0, 125);
    // Piso da banda pela CURVA DE 2 TRECHOS (ver roster_gen::skill_curve): o melhor da IA
    // vira max_skill (frente fiel/competitiva); o PIOR aterrissa onde a cauda o joga — mas
    // NUNCA abaixo do skill real dele (cap da cauda). No rookie (grid apertado) o fundo
    // afunda de propósito; no GT3 (grid largo) o cap segura o pior no próprio skill real.
    // O roster escreve a MESMA forma por piloto; a banda re-ancora no sweet spot.
    let min_skill = if skills.is_empty() {
        (max_skill - 25).max(0)
    } else {
        let curve = roster_gen::skill_curve_from(&skills, max_skill as f64);
        (roster_gen::skill_curve(curve.lo, &curve).round() as i64).clamp(0, max_skill)
    };
    // Chuva: se a corrida ALVO é molhada, baixa a banda (pelotão mais lento — chuva
    // no iRacing é punitiva; subir a IA faria o humano forçar e rodar). v1: rebaixa o
    // campo todo pela penalidade num fator_chuva médio (~50). Re-rank por piloto depois.
    // História da etapa ALVO: no auto-targeting é a PRÓPRIA próxima corrida pendente (pelo
    // id, não pela pista); no alvo manual de teste, a primeira etapa daquela pista.
    let target_story = if auto_targeted {
        next_pending_id
            .as_deref()
            .and_then(|rid| stories.get(rid))
            .map(|(_, s)| s)
    } else {
        resolved_track_id
            .and_then(|id| stories.values().find(|(tid, _)| *tid == id).map(|(_, s)| s))
    };
    let rain_pen = target_story
        .filter(|s| s.is_wet_race)
        .map(|s| crate::iracing_sdk::weather::rain_skill_penalty(50.0, s.race_intensity))
        .unwrap_or(0);

    // FAIXA EFETIVA vinda do roster (post-it do export que roda logo antes). É ela que faz o
    // esticão do iRacing virar a identidade: o roster sai normalizado em 0–100 a partir
    // exatamente destes dois valores, então cada IA corre no nível que o roster pretendia.
    // Sem isso a faixa saía das skills CRUAS e o esticão apagava tudo o que só existe no
    // roster — o líder com dia ruim voltava ao topo e ainda empurrava o grid inteiro junto.
    // A chuva já entra por piloto no roster, então a faixa dela vem pronta e o `rain_pen`
    // NÃO se aplica aqui.
    // Só vale quando categoria E pista casam; post-it de outro export é post-it velho. O
    // carimbo de tempo é conferido dentro do `load` (ver `postit_esta_fresco`): antes dele
    // um bilhete de dias atrás, na mesma pista e categoria, passava como se fosse de agora.
    let roster_band = load_export_skill_band(&base_dir, custid).filter(|b| {
        b.categoria == categoria && Some(b.track_id) == resolved_track_id && b.max > b.min
    });
    if roster_band.is_none() {
        // A ausência tem consequência (a faixa cai na fórmula antiga e o esticão do
        // iRacing volta a apagar o que o roster pretendia), então ela é registrada em vez
        // de acontecer calada.
        crate::diagnostico::linha(
            "iracing",
            "temporada sem a faixa do roster: exporte o roster desta pista antes da season \
             (a banda cai na fórmula antiga)",
        );
    }
    let (min_skill, max_skill) = match &roster_band {
        Some(b) => limites_da_banda(b.min, b.max),
        // Sem post-it (roster não exportado, ou exportado para outra pista): fórmula antiga,
        // com a penalidade de chuva na banda. Continua correta como aproximação.
        None => {
            let max = (max_skill - rain_pen).clamp(0, 125);
            ((min_skill - rain_pen).clamp(0, max), max)
        }
    };
    let max_drivers = (skills.len() as i64 + 1).max(2);

    // Clima global (fallback) = o da 1ª etapa do calendário.
    // Clima global (fallback p/ eventos sem weather própria) = seco/claro.
    let global_weather = season_gen::EventWeather {
        skies: 1,
        humidity: 45,
        temp_c: 26,
        track_water: 0,
        wind_kmh: 10,
        wind_dir_deg: 0,
        keyframes: vec![
            season_gen::WeatherKeyframe {
                event_type: 1,
                time_offset: -90,
            },
            season_gen::WeatherKeyframe {
                event_type: 0,
                time_offset: 0,
            },
            season_gen::WeatherKeyframe {
                event_type: 1,
                time_offset: race_end,
            },
        ],
        weather_id: format!("{custid}_global"),
        start_time: format!("{}-06-01T16:00:00", sim_safe_year(season.ano)),
    };

    let name = format!("{} - {}", cat.nome_curto, season.ano);
    let params = season_gen::SeasonParams {
        roster_name: roster_name.trim().to_string(),
        name: name.clone(),
        car_id: car.car_id,
        car_class_id: car.car_class_id,
        race_length_min: cat.duracao_corrida_min as i64,
        max_drivers,
        min_skill,
        max_skill,
        year: season.ano,
        global_weather,
        events,
    };
    let season_json = season_gen::build_season(&params);

    // Grava em aiseasons/<nome>.json.
    let safe_name: String = name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let dir =
        paths::aiseasons_dir().ok_or("Não foi possível localizar a pasta aiseasons do iRacing.")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    let path = dir.join(format!("{}.json", safe_name.trim()));
    let json = serde_json::to_string_pretty(&season_json)
        .map_err(|e| format!("Falha ao serializar: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Falha ao gravar: {e}"))?;

    // POST-IT do import: qual arquivo de aiseason e qual evento corresponde a cada
    // corrida da carreira. Sobrescrito a cada export → sempre aponta para o
    // campeonato atual. (Opção "Guardar" — exata, sem varrer/adivinhar.)
    let pointer = serde_json::json!({
        "aiseason_file": path.to_string_lossy(),
        "gravado_em_unix": agora_unix(),
        "events": event_race_map
            .iter()
            .map(|(rid, tid)| serde_json::json!({ "race_id": rid, "track_id": tid }))
            .collect::<Vec<_>>(),
    });
    // Este bilhete é o único caminho do import: sem ele o auto-import repete
    // "não achei o registro do aiseason exportado" para sempre. Falhar em silêncio aqui
    // era transformar um erro de disco num mistério do outro lado da corrida.
    if let Some(ppath) = season_pointer_path(&base_dir, &career_id) {
        let escrita = ppath
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&ppath, pointer.to_string()));
        if let Err(e) = escrita {
            crate::diagnostico::linha(
                "iracing",
                &format!("Falha ao gravar o registro do aiseason (o import não vai achar): {e}"),
            );
        }
    }

    Ok(SeasonGenResult {
        path: path.display().to_string(),
        name,
        events: params_events_len(&season_json),
        targeted_track,
        ai_skill: max_skill,
        auto_targeted,
    })
}

/// Conta os eventos no JSON gerado (para a UI).
fn params_events_len(v: &serde_json::Value) -> usize {
    v["events"].as_array().map(|a| a.len()).unwrap_or(0)
}
