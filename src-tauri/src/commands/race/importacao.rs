//! Entrada de uma corrida real do iRacing na carreira: resumo para a UI, avaliacao do desempenho e reaproveitamento do mesmo caminho de persistencia da simulacao.

use super::*;

/// Resumo do que foi gravado ao importar uma corrida do iRacing — devolvido ao
/// front para o aviso/pop-up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedRaceSummary {
    pub race_id: String,
    pub track_name: String,
    pub categoria: String,
    pub rodada: i32,
    /// Pilotos efetivamente gravados (após filtrar carros sem piloto da carreira).
    pub saved_drivers: usize,
    /// Carros da sessão descartados por não casarem com nenhum piloto da carreira.
    pub dropped_unmatched: usize,
    pub player_position: Option<i32>,
    pub player_points: Option<i32>,
    pub player_is_dnf: bool,
    pub winner_name: Option<String>,
    /// Custo do conserto do carro do jogador (0 se não houve batida cobrável).
    pub repair_cost: f64,
    /// Severidade da pior batida do jogador (leve/moderado/grave/destruído/catastrófico/nenhum).
    pub repair_severity: String,
    /// Quantas vezes o jogador bateu (de forma cobrável) NESTA temporada, incluindo esta.
    pub repair_count: i32,
    /// Frase pronta do pop-up (com valor e contagem já preenchidos). Vazia se cost=0.
    pub repair_message: String,
    /// Fatura de manutenção do carro (gasolina/pneus + itens do conserto). Sempre presente.
    #[serde(default)]
    pub maintenance: MaintenanceBreakdown,
    /// Repercussão pública do evento (esperado × realizado) — mesma leitura da corrida
    /// simulada. `None` se a categoria não tem config.
    #[serde(default)]
    pub event_repercussion: Option<EventRepercussionSummary>,
}

pub(crate) fn compute_race_evaluation(
    conn: &rusqlite::Connection,
    result: &RaceResult,
) -> Option<crate::race_eval::RaceEvaluation> {
    use crate::race_eval::{evaluate, RaceEvalInput};

    let player = result.race_results.iter().find(|r| r.is_jogador)?;
    let field = build_merit_field(conn, result);
    if field.is_empty() {
        return None;
    }
    Some(evaluate(&RaceEvalInput {
        player_id: player.pilot_id.clone(),
        grid_position: player.grid_position.max(1),
        finish_position: player.finish_position.max(1),
        is_dnf: player.is_dnf,
        incidents: player.incidents_count.max(0),
        field,
    }))
}

/// Importa um `RaceResult` vindo da SESSÃO do iRacing para a carreira. Guarda pela
/// próxima corrida pendente do jogador (mesma pista), filtra carros fantasmas,
/// recalcula classe+pontos e persiste pelo mesmo `persist_race_result_tx` da
/// simulação offline — a carreira reage idêntico a uma corrida simulada.
pub(crate) fn import_iracing_race_result(
    db: &mut Database,
    career_dir: &Path,
    session_track_id: i64,
    player_crash_severity: &str,
    // Direção do impacto no pico (front/rear/side/vertical) — do monitor. Vazia = frontal.
    player_impact_dir: &str,
    mut result: RaceResult,
    // Telemetria REAL do SDK (ritmo/duelo/erro/melhor momento) — vira pano de fundo
    // do boletim de IA. Corrida real do iRacing tem; offline/sem monitor vem vazia.
    telemetry: &crate::iracing_sdk::telemetry_analysis::TelemetryAnalysis,
    // Histórico ao vivo (voltas + batalha) — fonte dos sinais do dossiê de habilidade.
    history: &crate::iracing_sdk::race_monitor::RaceHistory,
    // Estilo de pilotagem do jogador (fatores por peça do monitor) — modula o desgaste só do
    // carro dele. `None`/neutro numa corrida sem estilo capturado.
    player_style: Option<crate::car::driving_style::StyleFactors>,
    // Desfechos de quebra da corrida (Peça 3), já resolvidos para driver_id. Vazio numa corrida
    // sem quebra. Persistidos na `race_breakdowns` (debrief/notícia).
    breakdowns: Vec<crate::db::queries::race_breakdowns::RaceBreakdownRow>,
) -> Result<(ImportedRaceSummary, RaceResult), String> {
    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

    let race_entry = get_next_player_race(&db.conn, &active_season)?
        .ok_or_else(|| "O jogador nao possui corrida pendente para importar.".to_string())?;

    // Guarda: a sessão tem que ser da MESMA pista da próxima corrida pendente.
    if race_entry.track_id as i64 != session_track_id {
        return Err(format!(
            "A sessao do iRacing e de outra pista (id {}). A proxima corrida da carreira e em {} (id {}). Importacao cancelada.",
            session_track_id, race_entry.track_name, race_entry.track_id
        ));
    }

    if runs_in_special_phase(&race_entry.categoria) {
        return Err("Importacao de corridas de fase especial ainda nao e suportada.".to_string());
    }

    let category = get_category_config(&race_entry.categoria)
        .ok_or_else(|| "Categoria da corrida nao encontrada.".to_string())?;
    let teams = team_queries::get_teams_by_category(&db.conn, &race_entry.categoria)
        .map_err(|e| format!("Falha ao buscar equipes da categoria: {e}"))?;

    // Filtra carros que NÃO casaram com pilotos reais da carreira (fantasmas /
    // pace car / slots vazios). Persistir um pilot_id inexistente quebraria o
    // apply_race_result_to_database (get_driver falha).
    let valid_ids: HashSet<String> = result
        .race_results
        .iter()
        .filter(|r| driver_queries::get_driver(&db.conn, &r.pilot_id).is_ok())
        .map(|r| r.pilot_id.clone())
        .collect();
    let total_before = result.race_results.len();
    result.race_results.retain(|r| valid_ids.contains(&r.pilot_id));
    result.qualifying_results.retain(|r| valid_ids.contains(&r.pilot_id));
    let dropped = total_before - result.race_results.len();
    if result.race_results.is_empty() {
        return Err("Nenhum piloto da sessao casou com a carreira — nada a importar.".to_string());
    }
    // Zera referências de topo que possam ter caído no filtro.
    for id in [
        &mut result.winner_id,
        &mut result.pole_sitter_id,
        &mut result.fastest_lap_id,
    ] {
        if !valid_ids.contains(id.as_str()) {
            id.clear();
        }
    }
    if result
        .most_positions_gained_id
        .as_ref()
        .is_some_and(|id| !valid_ids.contains(id))
    {
        result.most_positions_gained_id = None;
    }

    // Peça 3 · Camada B (ponte do DNF perdido): se um comando de quebra do JOGADOR não chegou
    // ao sim (fullscreen exclusivo → `chat_send_blocked`), o `!dq` não abandonou o carro dele e
    // o resultado importado o mostra na pista — mas a carreira JÁ comprometeu a quebra (está no
    // breakdown_log com desfecho "dnf"). Fecha a inconsistência carimbando o DNF a partir do LOG
    // (não do `!dq`). Gate no latch: só quando há evidência de que o comando foi bloqueado, pra
    // não fabricar um abandono que de fato não deveria acontecer. Roda ANTES do rescore, então
    // `apply_special_class_scoring` reconcilia posição/pontos (DNF vai pro fim da classe, 0 ponto)
    // e o bloco de quebra abaixo carimba o `dnf_catalog_id`/motivo (o carro agora é `is_dnf`).
    if crate::iracing_sdk::race_monitor::chat_send_blocked() {
        let player_break_label: Option<String> = result
            .race_results
            .iter()
            .find(|d| d.is_jogador && !d.is_dnf)
            .and_then(|player| {
                breakdowns
                    .iter()
                    .find(|b| b.severity == "dnf" && b.driver_id == player.pilot_id)
                    .map(|b| b.label.clone())
            });
        if let Some(label) = player_break_label {
            if let Some(player) = result.race_results.iter_mut().find(|d| d.is_jogador) {
                player.is_dnf = true;
                player.classification_status =
                    crate::simulation::race::ClassificationStatus::Dnf;
                player.dnf_reason = Some(label);
            }
            // Mantém a contagem de abandonos coerente (narrativa "corrida mais caótica" etc.).
            result.total_dnfs = result.race_results.iter().filter(|r| r.is_dnf).count() as i32;
        }
    }

    // Recalcula classe + pontos a partir de grid/chegada (single e multiclasse).
    apply_special_class_scoring(&mut result, &teams, category.id == "endurance");
    // Grid desconhecido (0) → evita posições-ganhas negativas absurdas.
    for r in result.race_results.iter_mut() {
        if r.grid_position <= 0 {
            r.grid_position = r.finish_position;
            r.positions_gained = 0;
        }
    }

    // Peça 3 · Camada B: os DNFs de QUEBRA viram DNF MECÂNICO no resultado (antes de persistir).
    // Assim a revista (beat "Abandono"), a desconfiança mecânica da IA e o motor editorial acendem
    // de graça: `dnf_catalog_id` aponta pra uma entry `Mechanical` do catálogo (fonte do join) e a
    // frase do problema vira o `dnf_reason` (a narrativa cita a peça). Só carimba quem o iRacing
    // também marcou como DNF — nunca fabrica um abandono que não aconteceu.
    if breakdowns.iter().any(|b| b.severity == "dnf") {
        let mech_id: Option<String> = db
            .conn
            .query_row(
                "SELECT id FROM incident_catalog WHERE incident_source = 'Mechanical' LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok();
        for b in breakdowns.iter().filter(|b| b.severity == "dnf") {
            if let Some(dr) = result
                .race_results
                .iter_mut()
                .find(|d| d.pilot_id == b.driver_id && d.is_dnf)
            {
                if let Some(id) = &mech_id {
                    dr.dnf_catalog_id = Some(id.clone());
                }
                dr.dnf_reason = Some(b.label.clone());
            }
        }
    }

    // Enduro: paradas REAIS do jogador (pneu/combustível) → alívio de gasto de peça (10%/parada,
    // teto 30%, só em corrida >40 min, aplicado no cérebro). Conta só paradas de SERVIÇO genuínas
    // (dwell ≥ limiar) do carro do jogador — não passagem no pit lane. A IA modela pela duração.
    const GENUINE_PIT_MIN_SECS: f64 = 4.0;
    let player_pits = history
        .pit_stops
        .iter()
        .filter(|s| s.car_idx == history.player_car_idx && s.stationary_secs >= GENUINE_PIT_MIN_SECS)
        .count() as u32;

    // Feedback físico da quebra (§4.6): agrupa as peças que largaram por TIME (resolve driver→time
    // pelo resultado). No persist, Leve → segue; Grave → fim de vida; DNF → destruída (troca
    // forçada a débito). É o que recopla a quebra ao vivo com o estado persistido do carro.
    let team_breakdowns: std::collections::HashMap<
        String,
        Vec<(crate::car::PartType, crate::car::breakdown::Severity)>,
    > = {
        use std::collections::HashMap;
        let driver_team: HashMap<&str, &str> = result
            .race_results
            .iter()
            .map(|r| (r.pilot_id.as_str(), r.team_id.as_str()))
            .collect();
        let mut map: HashMap<String, Vec<(crate::car::PartType, crate::car::breakdown::Severity)>> =
            HashMap::new();
        for row in &breakdowns {
            let (Some(pt), Some(sev), Some(team)) = (
                crate::car::PartType::from_str(&row.part),
                crate::car::breakdown::Severity::from_key(&row.severity),
                driver_team.get(row.driver_id.as_str()),
            ) else {
                continue;
            };
            map.entry(team.to_string()).or_default().push((pt, sev));
        }
        map
    };

    let next_round =
        Some((active_season.rodada_atual + 1).min(category.corridas_por_temporada as i32));
    let mut rng = rand::thread_rng();
    let import_injuries = persist_race_result_tx(
        db,
        &race_entry,
        &result,
        &teams,
        &active_season,
        category,
        next_round,
        RacePersistenceMode::Playable,
        player_style,
        player_pits,
        &team_breakdowns,
        &mut rng,
    )?;

    // Peça 3: grava os desfechos de quebra da corrida (debrief/notícia). Best-effort — uma
    // falha aqui não desfaz o resultado já persistido.
    warn_if_side_effect_fails(
        crate::db::queries::race_breakdowns::insert_breakdowns_batch(
            &db.conn,
            &race_entry.id,
            &breakdowns,
        )
        .map_err(|e| e.to_string()),
        "Falha ao gravar race_breakdowns do import",
    );

    // Fama do resultado IMPORTADO do iRacing: MESMA lógica da corrida simulada — o
    // astro nasce igual correndo na pista. Vitória/pódio sobem fama (modulada por
    // carisma), incidente/remontada movem carisma, o grid decai. Best-effort: uma
    // falha aqui não desfaz o resultado já persistido.
    let event_repercussion = apply_post_race_fame(&db.conn, &race_entry, &result, &import_injuries)
        .ok()
        .and_then(|outcome| outcome.player_repercussion);

    // Grava também o histórico por rodada (race_results.json) — é a fonte da grade
    // R1..R5 da classificação (build_driver_histories). O caminho offline faz o
    // mesmo via append_race_result; sem isso, os pontos entram mas a coluna da
    // rodada fica vazia.
    warn_if_side_effect_fails(
        append_race_result(
            career_dir,
            &race_entry.categoria,
            race_entry.rodada,
            &result.race_results,
        ),
        "Falha ao gravar race_results.json do import",
    );

    // ── Telemetria do jogador → dossiê de habilidade (Fase 2) ────────────────
    // Extrai os sinais compactos (consistência, briga, largada) do histórico ao
    // vivo e grava uma linha por corrida. Best-effort: só existe quando o jogador
    // DIRIGIU no iRacing e foi monitorado; falha aqui não desfaz o resultado.
    if let Some(dossier_row) =
        crate::iracing_sdk::telemetry_analysis::extract_player_race_telemetry(history, telemetry)
    {
        warn_if_side_effect_fails(
            crate::db::queries::race_history::upsert_player_race_telemetry(
                &db.conn,
                &race_entry.id,
                &dossier_row,
            )
            .map_err(|e| e.to_string()),
            "Falha ao gravar telemetria do jogador (dossiê de habilidade)",
        );
    }

    // ── Boletim de IA ────────────────────────────────────────────────────────
    // Gera a notícia de Corrida + guarda os fatos do boletim (igual ao fluxo
    // in-app `simulate_race_weekend_in_base_dir`). Sem isto, corridas IMPORTADAS do
    // iRacing não produziam boletim algum. Importância/tier neutros; sem lesões
    // (o import ainda não as computa). Prewarm em background para o boletim já
    // estar em cache quando o jogador abrir Notícias.
    {
        let flat_incidents: Vec<IncidentResult> = result
            .race_results
            .iter()
            .flat_map(|r| r.incidents.clone())
            .collect();
        // Telemetria REAL → fatos de cor sobre a corrida do jogador (ritmo, duelo,
        // erro mais caro, melhor momento). Só corrida real do iRacing tem isto.
        let player_name = result
            .race_results
            .iter()
            .find(|r| r.is_jogador)
            .map(|r| r.pilot_name.clone())
            .unwrap_or_default();
        let telemetry_facts = telemetry_context_facts(telemetry, &player_name);
        match persist_race_news(
            &db.conn,
            &result,
            &active_season,
            race_entry.rodada,
            &race_entry.categoria,
            0,
            race_entry.thematic_slot,
            &InterestTier::Baixo,
            &flat_incidents,
            &[],
            &telemetry_facts,
        ) {
            Ok(Some(news_id)) => {
                if let Some(base_dir) = career_dir.parent().and_then(|p| p.parent()) {
                    let mut cfg = AppConfig::load_or_default(base_dir);
                    let install_id = cfg.get_or_create_install_id();
                    let lang = cfg.language.clone();
                    spawn_prewarm_boletim(
                        career_dir.join("career.db"),
                        news_id,
                        lang,
                        install_id,
                    );
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("[narrative] Falha ao gerar boletim do import iRacing: {e}"),
        }
    }

    // Simula as OUTRAS categorias (IA) até a semana desta corrida — igual ao fluxo
    // in-app (`simulate_race_weekend_in_base_dir`). Sem isto, as corridas das demais
    // categorias acumulavam pendentes e a temporada não fechava (`advance_season`
    // rejeita corridas pendentes). Quando esta é a ÚLTIMA corrida do jogador na
    // temporada, simula todas as restantes até o fim, deixando o calendário pronto
    // para avançar.
    simulate_other_categories(
        db,
        career_dir,
        &race_entry.categoria,
        calendar_queries::calendar_entry_season_week(&race_entry),
        &race_entry.display_date,
        &active_season.id,
        active_season.numero,
    )?;

    let player = result.race_results.iter().find(|r| r.is_jogador);
    let winner_name = result
        .race_results
        .iter()
        .find(|r| !r.is_dnf && r.finish_position == 1)
        .map(|r| r.pilot_name.clone());

    // ── Conserto do carro (SÓ jogador) ──────────────────────────────────────
    // A batida do jogador (severidade já rebaixada se cruzou a linha) vira um
    // custo proporcional à categoria e ao carro, debitado do caixa da equipe.
    let mut repair_cost = 0.0;
    let mut repair_count = 0;
    let mut repair_message = String::new();
    let player_team_id = player.map(|r| r.team_id.clone()).unwrap_or_default();
    // Dano por PEÇA da batida (car::crash): amassa/destrói peças conforme a DIREÇÃO do
    // impacto e a CONDIÇÃO de cada uma; o custo vem das peças (não mais flat). O carro
    // danificado PERSISTE — e como `persist_race_result_tx` (que já rodou acima) mantém o
    // carro antes, aplicamos o dano por CIMA; o cérebro de manutenção responde na PRÓXIMA
    // corrida (trocar/degradar conforme o caixa). Só há dano se houve batida (≠ "nenhum").
    if !player_team_id.is_empty() && !player_crash_severity.eq_ignore_ascii_case("nenhum") {
        if let Ok(Some(mut team)) = team_queries::get_team_by_id(&db.conn, &player_team_id) {
            use crate::car::crash::{apply_crash_damage, CrashSeverity, ImpactDirection};
            use crate::db::queries::team_car;
            let severity = CrashSeverity::from_label(player_crash_severity);
            let direction = ImpactDirection::from_str(player_impact_dir);
            let mut car = team_car::get_team_car(&db.conn, &player_team_id)
                .ok()
                .flatten()
                .unwrap_or_else(|| crate::car::seed::seed_car(&team.categoria, 0.5));
            let damage = apply_crash_damage(&mut car, &team.categoria, severity, direction);
            let _ = team_car::upsert_team_car(&db.conn, &player_team_id, &car);
            let cost = damage.cost.round();
            if cost > 0.0 {
                team.cash_balance -= cost;
                team.last_round_expenses += cost;
                let _ = team_queries::update_team(&db.conn, &team);
                repair_cost = cost;
                repair_count = bump_repair_count(career_dir, active_season.numero);
                repair_message = pick_repair_message(
                    &mut rng,
                    player_crash_severity,
                    &format_brl(cost),
                    repair_count,
                );
            }
        }
    }

    // Fatura do fim de semana: a decomposição do custo de operação JÁ debitado desta
    // rodada (carro, logística, equipe) + o conserto, se houve batida.
    let maintenance = team_queries::get_team_by_id(&db.conn, &player_team_id)
        .ok()
        .flatten()
        .map(|team| {
            compute_maintenance_breakdown(
                &db.conn,
                &team,
                &result,
                race_entry.track_id,
                get_category_config(&race_entry.categoria)
                    .map(|c| c.corridas_por_temporada as f64)
                    .unwrap_or(12.0),
                global_economic_health_for_season(active_season.numero as i32),
                repair_cost,
                player_crash_severity,
                active_season.numero as i32,
                race_entry.rodada,
            )
        })
        .unwrap_or_default();

    let summary = ImportedRaceSummary {
        race_id: race_entry.id.clone(),
        track_name: race_entry.track_name.clone(),
        categoria: race_entry.categoria.clone(),
        rodada: race_entry.rodada,
        saved_drivers: result.race_results.len(),
        dropped_unmatched: dropped,
        player_position: player.map(|r| r.finish_position),
        player_points: player.map(|r| r.points_earned),
        player_is_dnf: player.map(|r| r.is_dnf).unwrap_or(false),
        winner_name,
        repair_cost,
        repair_severity: player_crash_severity.to_string(),
        repair_count,
        repair_message,
        maintenance,
        event_repercussion,
    };
    Ok((summary, result))
}
