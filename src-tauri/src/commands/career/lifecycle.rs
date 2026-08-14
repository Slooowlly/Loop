//! Ciclo de vida do save: criacao, carga, exclusao e listagem de carreiras, alem da
//! abertura dos recursos do save e do reparo de consistencia dos contratos regulares.

use super::*;
use crate::constants::historical_timeline::PLAYABLE_START_YEAR;

static CAREER_OPEN_REPAIR_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(crate) fn create_career_in_base_dir(
    base_dir: &Path,
    input: CreateCareerInput,
) -> Result<CreateCareerResult, String> {
    validate_create_career_input(&input)?;

    let normalized_name = input.player_name.trim().to_string();
    let normalized_nationality = input.player_nationality.trim().to_lowercase();
    let normalized_category = input.category.trim().to_lowercase();
    let normalized_difficulty = input.difficulty.trim().to_lowercase();
    let normalized_age = input.player_age.unwrap_or(20).clamp(16, 60);
    let nationality_label = format_nationality(&normalized_nationality, "M", "pt-BR");

    let mut config = AppConfig::load_or_default(base_dir);
    let saves_dir = config.saves_dir();
    let career_id = next_career_id(&saves_dir);
    let career_number = career_number_from_id(&career_id)
        .ok_or_else(|| format!("Falha ao interpretar career_id '{career_id}'"))?;
    let career_dir = saves_dir.join(&career_id);
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");

    std::fs::create_dir_all(&career_dir)
        .map_err(|e| format!("Falha ao criar diretorio da carreira: {e}"))?;

    let creation_result = (|| -> Result<CreateCareerResult, String> {
        let mut db = Database::create_new(&db_path)
            .map_err(|e| format!("Falha ao criar banco da carreira: {e}"))?;

        let mut world = generate_world(
            &normalized_name,
            &nationality_label,
            normalized_age,
            &normalized_category,
            input.team_index,
            &normalized_difficulty,
        )?;

        let season_id = next_id(&db.conn, IdType::Season)
            .map_err(|e| format!("Falha ao gerar ID da temporada: {e}"))?;
        // Mesmo início de mundo do draft histórico (`PLAYABLE_START_YEAR`): a carreira
        // regular começa direto no ano em que o mundo fica jogável, em vez de dois anos
        // antes por literal solto.
        let mut season = Season::new(season_id.clone(), 1, PLAYABLE_START_YEAR);
        season.fase = SeasonPhase::Temporada;
        align_world_career_start_years(&mut world, season.ano as u32);
        let calendar_seed: u64 = rand::random();

        let total_races = db
            .transaction(|tx| {
                for driver in &world.drivers {
                    driver_queries::insert_driver(tx, driver)?;
                }

                team_queries::insert_teams(tx, &world.teams)?;
                // Semeia o carro inicial de cada time (Sistema de Nível do Carro):
                // correlacionado com a qualidade na categoria; rookie = spec.
                crate::market::car_maintenance::seed_and_persist_team_cars(tx, &world.teams)?;
                contract_queries::insert_contracts(tx, &world.contracts)?;
                for contract in &world.contracts {
                    grant_driver_license_for_division_if_needed(
                        tx,
                        &contract.piloto_id,
                        &contract.categoria,
                        contract.classe.as_deref(),
                    )
                    .map_err(crate::db::connection::DbError::Migration)?;
                }
                season_queries::insert_season(tx, &season)?;
                let n = generate_full_season_calendar(tx, &season_id, season.ano, calendar_seed)?;
                sync_meta_counters(
                    tx,
                    world.drivers.len(),
                    world.teams.len(),
                    world.contracts.len(),
                    1,
                    n,
                )?;
                // O ANO DO MUNDO na tabela `meta`, escrito na criação. As migrações
                // semeiam os dois com um literal antigo, e a carreira regular nunca os
                // reescrevia: `current_year` só era tocado pela virada de temporada (a
                // temporada 1 inteira era jogada com o ano do seed) e `career_start_year`
                // ficava no seed para sempre. O draft histórico já grava os dois na
                // criação — ver `sync_draft_meta_counters`.
                meta_queries::set_meta_value(tx, "current_year", &season.ano.to_string())?;
                meta_queries::set_meta_value(tx, "career_start_year", &season.ano.to_string())?;
                Ok(n)
            })
            .map_err(|e| format!("Falha ao persistir dados da carreira: {e}"))?;

        let player_team = world
            .teams
            .iter()
            .find(|team| team.id == world.player_team_id)
            .ok_or_else(errors::player_team_missing_after_world)?;

        let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let meta = serde_json::json!({
            "version": 1,
            "career_number": career_number,
            "player_name": normalized_name,
            "current_season": 1,
            "current_year": PLAYABLE_START_YEAR,
            "created_at": now,
            "last_played": now,
            "team_name": player_team.nome,
            "category": normalized_category,
            "difficulty": normalized_difficulty,
            "total_races": total_races as i32,
        });

        let meta_json = serde_json::to_string_pretty(&meta)
            .map_err(|e| format!("Falha ao serializar meta.json: {e}"))?;
        std::fs::write(&meta_path, meta_json)
            .map_err(|e| format!("Falha ao gravar meta.json: {e}"))?;

        config.last_career = Some(career_number);
        config
            .save()
            .map_err(|e| format!("Falha ao salvar config do app: {e}"))?;

        Ok(CreateCareerResult {
            success: true,
            career_id,
            save_path: career_dir.to_string_lossy().to_string(),
            player_id: world.player.id,
            player_team_id: player_team.id.clone(),
            player_team_name: player_team.nome.clone(),
            season_id,
            total_drivers: world.drivers.len(),
            total_teams: world.teams.len(),
            total_races,
            message: rust_i18n::t!("career.message.created").to_string(),
        })
    })();

    if creation_result.is_err() && career_dir.exists() {
        let _ = std::fs::remove_dir_all(&career_dir);
    }

    creation_result
}

pub(crate) fn load_career_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<CareerData, String> {
    let career_number = career_number_from_id(career_id).ok_or_else(errors::invalid_career_id)?;
    let mut config = AppConfig::load_or_default(base_dir);
    let (db, career_dir, mut meta) = open_career_resources(base_dir, career_id)?;
    let meta_path = career_dir.join("meta.json");
    let mut active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(errors::active_season_not_found)?;
    repair_stale_convocation_phase(&db.conn, &mut active_season)?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar piloto do jogador: {e}"))?;
    let player_team = find_player_team(&db.conn, &player.id, active_season.fase)?;

    record_career_telemetry_context(&active_season, &player, player_team.as_ref(), &meta);
    spawn_preseason_preview_prewarm(
        &db.conn,
        base_dir,
        career_id,
        &active_season,
        player_team.as_ref(),
    );

    let next_race = if let Some(ref team) = player_team {
        calendar_queries::get_next_race(&db.conn, &active_season.id, &team.categoria)
            .map_err(|e| format!("Falha ao carregar proxima corrida: {e}"))?
    } else {
        None
    };

    let total_drivers = driver_queries::count_drivers(&db.conn)
        .map_err(|e| format!("Falha ao contar pilotos: {e}"))? as usize;
    let total_teams =
        count_rows(&db.conn, "teams").map_err(|e| format!("Falha ao contar equipes: {e}"))?;
    let total_rodadas = if let Some(ref team) = player_team {
        count_calendar_entries(&db.conn, &active_season.id, &team.categoria)
            .map_err(|e| format!("Falha ao contar corridas da temporada: {e}"))?
    } else {
        0
    };

    // Calcular interesse esperado da próxima corrida (fallback silencioso se falhar).
    let event_interest_summary: Option<EventInterestSummary> = next_race
        .as_ref()
        .map(|race| build_next_race_interest_summary(&db.conn, race, &player, total_rodadas));

    stamp_last_played(&meta_path, &mut meta, &mut config, career_number)?;

    let team_summary = player_team
        .as_ref()
        .map(|team| {
            build_team_summary(&db.conn, team)
                .map_err(|e| format!("Falha ao montar resumo da equipe: {e}"))
        })
        .transpose()?;
    let accepted_special_offer = build_accepted_special_offer_summary(&db.conn, &player)?;
    // `None` sem equipe — ver `compute_public_fame_share`.
    let public_fame_share: Option<f64> = next_race.as_ref().and_then(|race| {
        let team = player_team.as_ref()?;
        compute_public_fame_share(&db.conn, &race.categoria, &team.id)
    });
    let next_race_summary = next_race.as_ref().map(|race| {
        build_next_race_summary(race, event_interest_summary.clone(), public_fame_share)
    });
    let next_race_briefing_summary = next_race.as_ref().map(|race| {
        build_next_race_briefing_summary(&db.conn, &player.id, active_season.numero, race)
            .unwrap_or_else(|_error| empty_next_race_briefing_summary())
    });
    let resume_context = read_resume_context(&career_dir)?;

    Ok(CareerData {
        career_id: career_id.to_string(),
        save_path: career_dir.to_string_lossy().to_string(),
        difficulty: meta.difficulty.clone(),
        player: build_player_summary(&player, player_team.as_ref()),
        player_team: team_summary,
        season: SeasonSummary {
            id: active_season.id.clone(),
            numero: active_season.numero,
            ano: active_season.ano,
            rodada_atual: active_season.rodada_atual,
            total_rodadas,
            status: active_season.status.as_str().to_string(),
            fase: active_season.fase.as_str().to_string(),
        },
        accepted_special_offer,
        next_race: next_race_summary,
        next_race_briefing: next_race_briefing_summary,
        total_drivers,
        total_teams,
        resume_context,
    })
}

pub(crate) fn delete_career_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<String, String> {
    let career_number = career_number_from_id(career_id).ok_or_else(errors::invalid_career_id)?;
    let mut config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(career_id);

    if !career_dir.exists() {
        return Err(errors::save_not_found());
    }

    std::fs::remove_dir_all(&career_dir).map_err(|e| format!("Falha ao deletar save: {e}"))?;

    if config.last_career == Some(career_number) {
        config.last_career = None;
        config
            .save()
            .map_err(|e| format!("Falha ao atualizar config do app: {e}"))?;
    }

    Ok(rust_i18n::t!("career.message.deleted", id = career_id).to_string())
}

pub(crate) fn list_saves_in_base_dir(base_dir: &Path) -> Result<Vec<SaveInfo>, String> {
    let config = AppConfig::load_or_default(base_dir);
    Ok(config
        .list_saves()
        .into_iter()
        .map(save_meta_to_info)
        .collect())
}

pub(crate) fn validate_create_career_input(input: &CreateCareerInput) -> Result<(), String> {
    let name = input.player_name.trim();
    let nationality_id = input.player_nationality.trim().to_lowercase();
    let category = input.category.trim().to_lowercase();
    let difficulty = input.difficulty.trim().to_lowercase();
    if name.is_empty() {
        return Err(errors::name_required());
    }
    if name.chars().count() > 50 {
        return Err(errors::name_too_long());
    }
    if get_nationality(&nationality_id).is_none() {
        return Err(errors::invalid_nationality());
    }
    if !matches!(category.as_str(), "mazda_rookie" | "toyota_rookie") {
        return Err(errors::invalid_starting_category());
    }
    if input.team_index > 5 {
        return Err(errors::invalid_team_index());
    }
    if scoring::get_difficulty_config(&difficulty).is_none() {
        return Err(errors::invalid_difficulty());
    }
    if let Some(age) = input.player_age {
        if !(16..=60).contains(&age) {
            return Err(errors::invalid_age());
        }
    }
    Ok(())
}

pub(crate) fn next_career_id(saves_dir: &Path) -> String {
    if !saves_dir.exists() {
        return "career_001".to_string();
    }

    let next_number = std::fs::read_dir(saves_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.strip_prefix("career_")?.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0)
        + 1;

    format!("career_{next_number:03}")
}

pub(crate) fn career_number_from_id(career_id: &str) -> Option<u32> {
    career_id.strip_prefix("career_")?.parse::<u32>().ok()
}

pub(crate) fn sync_meta_counters(
    conn: &rusqlite::Connection,
    total_drivers: usize,
    total_teams: usize,
    total_contracts: usize,
    total_seasons: usize,
    total_races: usize,
) -> Result<(), crate::db::connection::DbError> {
    meta_queries::set_meta_value(
        conn,
        "next_driver_id",
        &(total_drivers as u32 + 1).to_string(),
    )?;
    meta_queries::set_meta_value(conn, "next_team_id", &(total_teams as u32 + 1).to_string())?;
    meta_queries::set_meta_value(
        conn,
        "next_contract_id",
        &(total_contracts as u32 + 1).to_string(),
    )?;
    meta_queries::set_meta_value(
        conn,
        "next_season_id",
        &(total_seasons as u32 + 1).to_string(),
    )?;
    meta_queries::set_meta_value(conn, "next_race_id", &(total_races as u32 + 1).to_string())?;
    meta_queries::set_meta_value(conn, "current_season", &total_seasons.to_string())?;
    Ok(())
}

/// REPARO DE FASE: save que ficou marcado em `JanelaConvocacao` com etapa regular
/// pendente volta para `BlocoRegular`. Acontece quando a janela abre antes de a
/// categoria do jogador terminar o bloco; sem isto a UI mostra a janela e esconde a
/// corrida que ainda tem de ser disputada. Muda `active_season.fase` no lugar.
fn repair_stale_convocation_phase(
    conn: &rusqlite::Connection,
    active_season: &mut Season,
) -> Result<(), String> {
    if active_season.fase != SeasonPhase::JanelaConvocacao {
        return Ok(());
    }
    let pending_regular_races = calendar_queries::count_pending_races_in_phase(
        conn,
        &active_season.id,
        &SeasonPhase::BlocoRegular,
    )
    .map_err(|e| format!("Falha ao verificar corridas regulares pendentes: {e}"))?;
    if pending_regular_races > 0 {
        season_queries::update_season_fase(conn, &active_season.id, &SeasonPhase::BlocoRegular)
            .map_err(|e| format!("Falha ao corrigir fase da temporada: {e}"))?;
        active_season.fase = SeasonPhase::BlocoRegular;
    }
    Ok(())
}

/// Telemetria: onde esta carreira está no mundo (ano/categoria/dificuldade/progresso).
/// Chamado do `load_career` porque é o ponto por onde TODA carreira aberta passa —
/// inclusive depois de virar a temporada, quando a UI recarrega. Só grava num estático
/// em memória; quem envia é a borda de corrida, e só se o jogador tiver consentido.
/// Sem equipe (agente livre) a categoria vem do último campeonato do piloto.
///
/// A dificuldade viaja em TODO evento (não só no fim de corrida) porque é o eixo pelo
/// qual o desfecho é lido: posição e ritmo só calibram a curva se você souber em que
/// nível aquela corrida foi disputada.
/// `numero` e não `ano`: o que interessa é o ANO DA CARREIRA (1, 2, 3…). O ano do
/// calendário não diz onde a pessoa está na progressão, e duas carreiras começadas
/// em anos diferentes ficariam incomparáveis por nada.
fn record_career_telemetry_context(
    active_season: &Season,
    player: &Driver,
    player_team: Option<&Team>,
    meta: &SaveMeta,
) {
    crate::telemetry::set_career_context(
        active_season.numero as i32,
        player_team
            .map(|t| t.categoria.clone())
            .or_else(|| player.categoria_atual.clone())
            .unwrap_or_else(|| "sem_equipe".to_string()),
        meta.difficulty.clone(),
        player.stats_carreira.temporadas as i32,
        player.stats_carreira.corridas as i32,
    );
}

/// PRÉ-TEMPORADA: pré-gera a matéria "O Que Esperar" em background, para a revista já
/// abrir com o texto pronto em vez de mostrar "escrevendo a prévia…" e esperar o
/// servidor. Passada a 1ª etapa da categoria a revista troca essa matéria pela edição da
/// corrida, então fora da pré-temporada não há o que adiantar. O trabalho é idempotente:
/// com a matéria já em cache a thread só faz uma leitura e sai.
fn spawn_preseason_preview_prewarm(
    conn: &rusqlite::Connection,
    base_dir: &Path,
    career_id: &str,
    active_season: &Season,
    player_team: Option<&Team>,
) {
    let Some(team) = player_team else {
        return;
    };
    let corridas_concluidas = calendar_queries::count_races_by_status(
        conn,
        &active_season.id,
        &team.categoria,
        &crate::models::enums::RaceStatus::Concluida,
    )
    .unwrap_or(0);
    if corridas_concluidas == 0 {
        crate::commands::season_preview::spawn_prewarm_season_preview(
            base_dir.to_path_buf(),
            career_id.to_string(),
        );
    }
}

/// Marca o save como jogado agora: `last_played` no meta.json e `last_career` na config
/// do app (é o que faz o menu principal abrir neste save). Roda em toda abertura, então
/// é a escrita que sobra quando o reparo de contratos é dispensado pela triagem.
fn stamp_last_played(
    meta_path: &Path,
    meta: &mut SaveMeta,
    config: &mut AppConfig,
    career_number: u32,
) -> Result<(), String> {
    meta.last_played = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    write_save_meta(meta_path, meta)?;
    config.last_career = Some(career_number);
    config
        .save()
        .map_err(|e| format!("Falha ao atualizar config do app: {e}"))
}

/// A próxima etapa como a tela de carreira a lê. `event_interest` e `public_fame_share`
/// entram prontos porque dependem de consultas que o chamador já fez.
fn build_next_race_summary(
    race: &crate::calendar::CalendarEntry,
    event_interest: Option<EventInterestSummary>,
    public_fame_share: Option<f64>,
) -> RaceSummary {
    RaceSummary {
        id: race.id.clone(),
        rodada: race.rodada,
        track_name: race.track_name.clone(),
        clima: race.clima.as_str().to_string(),
        duracao_corrida_min: race.duracao_corrida_min,
        status: race.status.as_str().to_string(),
        temperatura: race.temperatura,
        horario: race.horario.clone(),
        week_of_year: race.week_of_year,
        season_phase: race.season_phase.as_str().to_string(),
        display_date: race.display_date.clone(),
        thematic_slot: race.thematic_slot.as_str().to_string(),
        event_interest,
        public_fame_share,
    }
}

/// Cabeçalho do piloto do jogador no payload de abertura. Os campos que a tela de
/// carreira não usa (lesão, aposentadoria, posição no campeonato, resultados) saem
/// zerados de propósito — quem os preenche são as telas de detalhe.
fn build_player_summary(player: &Driver, player_team: Option<&Team>) -> DriverSummary {
    DriverSummary {
        id: player.id.clone(),
        nome: player.nome.clone(),
        nacionalidade: player.nacionalidade.clone(),
        idade: player.idade as i32,
        skill: player.atributos.skill.round().clamp(0.0, 100.0) as u8,
        midia: player.atributos.midia.round().clamp(0.0, 100.0) as u8,
        categoria_especial_ativa: player.categoria_especial_ativa.clone(),
        equipe_id: player_team.map(|t| t.id.clone()),
        equipe_nome: player_team.map(|t| t.nome.clone()),
        equipe_nome_curto: player_team.map(|t| t.nome_curto.clone()),
        equipe_cor: player_team
            .map(|t| t.cor_primaria.clone())
            .unwrap_or_default(),
        classe: player_team.and_then(|t| t.classe.clone()),
        is_jogador: player.is_jogador,
        is_estreante: player.temporadas_na_categoria == 0,
        is_estreante_da_vida: player.stats_carreira.corridas == 0,
        lesao_ativa_tipo: None,
        is_aposentado: false,
        pontos: player.stats_temporada.pontos.round() as i32,
        vitorias: player.stats_temporada.vitorias as i32,
        podios: player.stats_temporada.podios as i32,
        posicao_campeonato: 0,
        results: Vec::new(),
    }
}

/// Rodadas restantes e distância para o líder que fazem a próxima corrida ser
/// "decisiva pelo título". Heurística de desenho, sem medição por trás — ficou
/// nomeada porque estava como literal solto no meio do `load_career`.
const TITLE_DECIDER_MAX_REMAINING: i32 = 2;
const TITLE_DECIDER_MAX_GAP: i32 = 50;

/// Interesse esperado da próxima corrida do jogador. Só leitura; a categoria da
/// etapa é a fonte semântica do campeonato do evento.
///
/// Aqui fica só o EFEITO — a leitura da classificação. A montagem do contexto, que é a
/// parte com regra dentro (o recorte de "decisiva pelo título" e os campos que viram
/// `None` quando o jogador ainda não pontuou), saiu para [`next_race_interest_context`],
/// que é pura e por isso tem teste sem banco.
fn build_next_race_interest_summary(
    conn: &rusqlite::Connection,
    race: &crate::calendar::CalendarEntry,
    player: &Driver,
    total_rodadas: i32,
) -> EventInterestSummary {
    let champ = standings_queries::get_championship_context(conn, &race.categoria).unwrap_or(
        ChampionshipContext {
            player_position: 0,
            gap_to_leader: 0,
        },
    );
    let ctx = next_race_interest_context(race, player, total_rodadas, &champ);
    to_summary(&calculate_expected_event_interest(&ctx))
}

/// O contexto de interesse da próxima etapa, montado a partir do que já foi lido do banco.
///
/// Duas regras moram aqui, e nenhuma delas precisa de banco para ser conferida: a etapa é
/// "decisiva pelo título" quando restam poucas rodadas E a distância para o líder é curta
/// E o jogador já tem posição; e posição/distância só viajam preenchidas quando têm
/// sentido — jogador fora da classificação (`player_position == 0`) manda `None`, e líder
/// manda a distância mesmo quando ela é zero.
pub(crate) fn next_race_interest_context(
    race: &crate::calendar::CalendarEntry,
    player: &Driver,
    total_rodadas: i32,
    champ: &ChampionshipContext,
) -> EventInterestContext {
    let remaining = total_rodadas - race.rodada;
    let is_title_decider = remaining <= TITLE_DECIDER_MAX_REMAINING
        && champ.gap_to_leader <= TITLE_DECIDER_MAX_GAP
        && champ.player_position > 0;
    EventInterestContext {
        categoria: race.categoria.clone(),
        season_phase: race.season_phase,
        rodada: race.rodada,
        total_rodadas,
        week_of_year: race.week_of_year,
        track_id: race.track_id as i32,
        track_name: race.track_name.clone(),
        is_player_event: true,
        player_championship_position: if champ.player_position > 0 {
            Some(champ.player_position)
        } else {
            None
        },
        player_media: Some(player.atributos.midia as f32),
        championship_gap_to_leader: if champ.gap_to_leader > 0 || champ.player_position == 1 {
            Some(champ.gap_to_leader)
        } else {
            None
        },
        is_title_decider_candidate: is_title_decider,
        thematic_slot: race.thematic_slot,
    }
}

/// Cota de público do jogador (Fase 3 do Estrelato): fama do lineup da equipe dele
/// contra o grid da próxima corrida → fração do portão que a equipe captura (piso +
/// prêmio de estrela, a mesma conta da bilheteria).
fn compute_public_fame_share(
    conn: &rusqlite::Connection,
    categoria: &str,
    team_id: &str,
) -> Option<f64> {
    let category_teams = team_queries::get_teams_by_category(conn, categoria).ok()?;
    let grid_total: f64 = category_teams
        .iter()
        .map(|t| {
            let medias = team_queries::get_team_lineup_medias(conn, &t.id).unwrap_or_default();
            crate::public_presence::team::derive_team_public_presence(&medias)
        })
        .sum();
    let team_medias = team_queries::get_team_lineup_medias(conn, team_id).unwrap_or_default();
    let team_presence = crate::public_presence::team::derive_team_public_presence(&team_medias);
    let n = category_teams.len().max(1) as f64;
    Some(crate::finance::cashflow::team_gate_share(
        team_presence,
        grid_total,
        n,
    ))
}

pub(crate) fn open_career_resources(
    base_dir: &Path,
    career_id: &str,
) -> Result<(Database, std::path::PathBuf, SaveMeta), String> {
    open_career_resources_with_repair(base_dir, career_id, true)
}

pub(crate) fn open_career_resources_read_only(
    base_dir: &Path,
    career_id: &str,
) -> Result<(Database, std::path::PathBuf, SaveMeta), String> {
    open_career_resources_with_repair(base_dir, career_id, false)
}

pub(crate) fn open_career_resources_with_repair(
    base_dir: &Path,
    career_id: &str,
    repair_contracts: bool,
) -> Result<(Database, std::path::PathBuf, SaveMeta), String> {
    let _career_number = career_number_from_id(career_id).ok_or_else(errors::invalid_career_id)?;

    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(career_id);
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");

    if !career_dir.exists() {
        return Err(errors::save_not_found());
    }
    if !db_path.exists() {
        return Err(errors::db_not_found());
    }

    let preseason_plan = load_preseason_plan(&career_dir)?;
    let preseason_active = preseason_plan.is_some();
    // Semanas de abertura da janela (as pré-passes ainda não caíram): o piloto que se
    // aposentou no fim da temporada continua no assento, porque a semana 1 é a foto de
    // como a temporada TERMINOU e ele a correu inteira. Quem tira o contrato dele são as
    // pré-passes, na virada da semana 1 — com evento no feed explicando a saída.
    let keep_retired_seated = preseason_plan.is_some_and(|plan| !plan.prepasses_applied);
    let meta = read_save_meta(&meta_path)?;
    let db = if repair_contracts {
        let _repair_guard = match CAREER_OPEN_REPAIR_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
        {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let db = Database::open_existing(&db_path)
            .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
        repair_regular_contract_consistency(&db.conn, !preseason_active, keep_retired_seated)?;
        db
    } else {
        Database::open_existing(&db_path)
            .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?
    };

    Ok((db, career_dir, meta))
}

/// `keep_retired_seated`: não rescinde o contrato de quem se aposentou, deixando o
/// piloto no assento. Ligado só nas semanas de abertura da janela de mercado, onde o
/// grid é a foto do fim da temporada — ver `open_career_resources_with_repair`. Fora
/// disso a rescisão continua sendo parte do reparo: aposentado com contrato ativo é
/// estado inválido, e as pré-passes da janela são quem o resolve no fluxo normal.
pub(crate) fn repair_regular_contract_consistency(
    conn: &rusqlite::Connection,
    allow_regular_vacancy_fill: bool,
    keep_retired_seated: bool,
) -> Result<(), String> {
    // TRIAGEM antes do reparo: o corpo abaixo abre transação IMMEDIATE (serializando as
    // aberturas concorrentes sob `CAREER_OPEN_REPAIR_LOCK`), varre time por time e
    // recalcula hierarquia — e era pago em TODA abertura de escrita, inclusive por quem
    // só ia atualizar `meta.json`. A triagem responde "há algo a reparar?" com poucas
    // consultas e sem transação; num save consistente o reparo inteiro é pulado.
    //
    // A condição da triagem é CONSERVADORA de propósito: qualquer dúvida devolve `true`
    // e o reparo roda igual ao que rodava antes. Falso positivo custa o que já se
    // pagava; falso negativo deixaria estado inválido no save.
    if needs_regular_contract_repair(conn, keep_retired_seated)? {
        apply_regular_contract_repair(conn, keep_retired_seated)?;
    }

    fill_pending_regular_vacancies_if_racing(conn, allow_regular_vacancy_fill)
}

/// Aplica o reparo de consistência dos contratos regulares. Sempre chamado atrás de
/// `needs_regular_contract_repair` — ver o comentário lá.
fn apply_regular_contract_repair(
    conn: &rusqlite::Connection,
    keep_retired_seated: bool,
) -> Result<(), String> {
    let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|e| format!("Falha ao iniciar reparo de contratos: {e}"))?;
    let mut affected_team_ids = HashSet::new();
    let active_regular_contracts = contract_queries::get_all_active_regular_contracts(&tx)
        .map_err(|e| format!("Falha ao carregar contratos regulares ativos: {e}"))?;
    let mut contracts_by_pilot = HashMap::<String, Vec<_>>::new();

    for contract in active_regular_contracts {
        contracts_by_pilot
            .entry(contract.piloto_id.clone())
            .or_default()
            .push(contract);
    }

    for contracts in contracts_by_pilot.values_mut() {
        if contracts.len() <= 1 {
            continue;
        }

        contracts.sort_by(|a, b| {
            b.temporada_inicio
                .cmp(&a.temporada_inicio)
                .then_with(|| b.created_at.cmp(&a.created_at))
                .then_with(|| b.id.cmp(&a.id))
        });

        for duplicate in contracts.iter().skip(1) {
            contract_queries::update_contract_status(
                &tx,
                &duplicate.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato regular duplicado '{}': {e}",
                    duplicate.id
                )
            })?;
            affected_team_ids.insert(duplicate.equipe_id.clone());
        }

        if let Some(kept) = contracts.first() {
            affected_team_ids.insert(kept.equipe_id.clone());
        }
    }

    let teams =
        team_queries::get_all_teams(&tx).map_err(|e| format!("Falha ao carregar equipes: {e}"))?;
    let teams_by_id = teams
        .iter()
        .map(|team| (team.id.clone(), team.clone()))
        .collect::<HashMap<_, _>>();
    let active_regular_contracts = contract_queries::get_all_active_regular_contracts(&tx)
        .map_err(|e| format!("Falha ao recarregar contratos regulares ativos: {e}"))?;
    // Só os pilotos DOS contratos, não a tabela inteira: o `drivers_by_id` abaixo é
    // consultado exclusivamente por `contract.piloto_id`, e a tabela de pilotos cresce
    // com o mundo (o draft histórico deixa milhares de aposentados no banco).
    let drivers_by_id = active_regular_contracts
        .iter()
        .filter_map(|contract| {
            driver_queries::get_driver(&tx, &contract.piloto_id)
                .ok()
                .map(|driver| (contract.piloto_id.clone(), driver))
        })
        .collect::<HashMap<_, _>>();
    for contract in active_regular_contracts {
        if !categories::is_valid_competitive_division(
            &contract.categoria,
            contract.classe.as_deref(),
        ) {
            contract_queries::update_contract_status(
                &tx,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato regular com divisao invalida '{}': {e}",
                    contract.id
                )
            })?;
            affected_team_ids.insert(contract.equipe_id.clone());
            continue;
        }

        let Some(team) = teams_by_id.get(&contract.equipe_id) else {
            continue;
        };
        if !categories::uses_regular_contracts(&team.categoria) {
            contract_queries::update_contract_status(
                &tx,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato regular em equipe especial '{}': {e}",
                    contract.id
                )
            })?;
            affected_team_ids.insert(contract.equipe_id.clone());
            continue;
        }

        let Some(driver) = drivers_by_id.get(&contract.piloto_id) else {
            continue;
        };
        if driver.status == DriverStatus::Aposentado && !keep_retired_seated {
            contract_queries::update_contract_status(
                &tx,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato regular invalido '{}': {e}",
                    contract.id
                )
            })?;
            affected_team_ids.insert(contract.equipe_id.clone());
            continue;
        }

        if driver.categoria_atual.as_deref() != Some(team.categoria.as_str()) {
            let mut updated_driver = driver.clone();
            updated_driver.mover_para_categoria(Some(team.categoria.clone()));
            driver_queries::update_driver(&tx, &updated_driver).map_err(|e| {
                format!("Falha ao corrigir categoria do piloto '{}': {e}", driver.id)
            })?;
        }
    }

    for team in teams
        .iter()
        .filter(|team| categories::uses_regular_teams(&team.categoria))
    {
        if normalize_regular_contracts_for_team(&tx, &team.id)? {
            affected_team_ids.insert(team.id.clone());
        }
    }

    for team_id in affected_team_ids {
        refresh_team_hierarchy_now(&tx, &team_id)?;
    }

    tx.execute(
        "UPDATE drivers SET categoria_atual = NULL
         WHERE categoria_atual IS NOT NULL
           AND id NOT IN (SELECT piloto_id FROM contracts WHERE status = 'Ativo')",
        [],
    )
    .map_err(|e| format!("Falha ao limpar categoria_atual de pilotos sem contrato: {e}"))?;

    tx.commit()
        .map_err(|e| format!("Falha ao concluir reparo de contratos: {e}"))?;
    Ok(())
}

/// Preenche as vagas regulares que sobraram, mas só com a temporada EM CORRIDA e etapa
/// pendente: fora disso um assento vazio é estado legítimo (janela de mercado, ano
/// encerrado) e preencher seria inventar contrato.
fn fill_pending_regular_vacancies_if_racing(
    conn: &rusqlite::Connection,
    allow_regular_vacancy_fill: bool,
) -> Result<(), String> {
    if !allow_regular_vacancy_fill {
        return Ok(());
    }
    let Some(active_season) = season_queries::get_active_season(conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa para reparo de vagas: {e}"))?
    else {
        return Ok(());
    };
    if !active_season.fase.is_racing() {
        return Ok(());
    }
    let pending_regular_races = calendar_queries::count_pending_races_in_phase(
        conn,
        &active_season.id,
        &SeasonPhase::BlocoRegular,
    )
    .map_err(|e| format!("Falha ao contar corridas regulares pendentes: {e}"))?;
    let pending_temporada_races = calendar_queries::count_pending_races_in_phase(
        conn,
        &active_season.id,
        &SeasonPhase::Temporada,
    )
    .map_err(|e| format!("Falha ao contar corridas da temporada pendentes: {e}"))?;
    if pending_regular_races + pending_temporada_races > 0 {
        let mut rng = rand::thread_rng();
        fill_all_remaining_vacancies(conn, active_season.numero, &mut rng)
            .map_err(|e| format!("Falha ao preencher vagas regulares pendentes: {e}"))?;
    }
    Ok(())
}

/// Triagem do reparo de contratos regulares: `true` quando existe pelo menos um estado
/// que o reparo mudaria. Espelha as regras de `apply_regular_contract_repair` na MESMA
/// ordem, lendo os mesmos dados (contratos regulares ativos + equipes), e erra sempre
/// para `true` — nunca para `false`.
///
/// A normalização de lineup é a única regra cuja aplicação é um algoritmo de slots; aqui
/// ela é reduzida à condição SUFICIENTE de no-op (no máximo dois contratos, papéis
/// distintos e as colunas da equipe apontando para esses contratos). Qualquer coisa fora
/// disso devolve `true` e a normalização de verdade decide.
pub(crate) fn needs_regular_contract_repair(
    conn: &rusqlite::Connection,
    keep_retired_seated: bool,
) -> Result<bool, String> {
    let contracts = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao carregar contratos regulares ativos: {e}"))?;
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao carregar equipes para triagem do reparo: {e}"))?;
    let teams_by_id = teams
        .iter()
        .map(|team| (team.id.as_str(), team))
        .collect::<HashMap<_, _>>();

    // Duplicidade: o mesmo piloto com mais de um contrato regular ativo.
    let mut seen_pilots = HashSet::new();
    for contract in &contracts {
        if !seen_pilots.insert(contract.piloto_id.as_str()) {
            return Ok(true);
        }
        // Divisão inválida, ou contrato regular numa equipe que não usa contrato regular.
        if !categories::is_valid_competitive_division(
            &contract.categoria,
            contract.classe.as_deref(),
        ) {
            return Ok(true);
        }
        if let Some(team) = teams_by_id.get(contract.equipe_id.as_str()) {
            if !categories::uses_regular_contracts(&team.categoria) {
                return Ok(true);
            }
        }
    }

    // Aposentado ainda sentado, e piloto com `categoria_atual` fora da categoria da
    // equipe do contrato. Em SQL para não carregar a tabela de pilotos inteira — o custo
    // dela cresce com o mundo (o draft histórico gera milhares).
    if !keep_retired_seated
        && exists(
            conn,
            "SELECT 1 FROM contracts c
               JOIN drivers d ON d.id = c.piloto_id
              WHERE c.status = 'Ativo' AND c.tipo = 'Regular' AND d.status = 'Aposentado'
              LIMIT 1",
            "aposentado com contrato regular ativo",
        )?
    {
        return Ok(true);
    }
    if exists(
        conn,
        "SELECT 1 FROM contracts c
           JOIN teams t ON t.id = c.equipe_id
           JOIN drivers d ON d.id = c.piloto_id
          WHERE c.status = 'Ativo' AND c.tipo = 'Regular'
            AND (d.categoria_atual IS NULL OR d.categoria_atual <> t.categoria)
          LIMIT 1",
        "piloto fora da categoria da equipe",
    )? {
        return Ok(true);
    }
    // Sobra de `categoria_atual` em quem não tem contrato ativo nenhum.
    if exists(
        conn,
        "SELECT 1 FROM drivers
          WHERE categoria_atual IS NOT NULL
            AND id NOT IN (SELECT piloto_id FROM contracts WHERE status = 'Ativo')
          LIMIT 1",
        "categoria_atual sem contrato",
    )? {
        return Ok(true);
    }

    // Lineup das equipes regulares.
    let mut contracts_by_team = HashMap::<&str, Vec<&crate::models::contract::Contract>>::new();
    for contract in &contracts {
        contracts_by_team
            .entry(contract.equipe_id.as_str())
            .or_default()
            .push(contract);
    }
    for team in teams
        .iter()
        .filter(|team| categories::uses_regular_teams(&team.categoria))
    {
        let team_contracts = contracts_by_team
            .get(team.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        if team_contracts.len() > 2 {
            return Ok(true);
        }
        let numero_1 = team_contracts
            .iter()
            .filter(|contract| contract.papel == TeamRole::Numero1)
            .collect::<Vec<_>>();
        let numero_2 = team_contracts
            .iter()
            .filter(|contract| contract.papel == TeamRole::Numero2)
            .collect::<Vec<_>>();
        // Papel duplicado no mesmo time: não há par (piloto, papel) confiável para
        // comparar com as colunas da equipe.
        if numero_1.len() > 1 || numero_2.len() > 1 {
            return Ok(true);
        }
        let esperado_1 = numero_1.first().map(|contract| contract.piloto_id.as_str());
        let esperado_2 = numero_2.first().map(|contract| contract.piloto_id.as_str());
        if team.piloto_1_id.as_deref() != esperado_1 || team.piloto_2_id.as_deref() != esperado_2 {
            return Ok(true);
        }
    }

    Ok(false)
}

fn exists(conn: &rusqlite::Connection, sql: &str, rotulo: &str) -> Result<bool, String> {
    conn.query_row(sql, [], |_row| Ok(true))
        .optional()
        .map(|found| found.unwrap_or(false))
        .map_err(|e| format!("Falha na triagem do reparo ({rotulo}): {e}"))
}

pub(crate) fn count_rows(
    conn: &rusqlite::Connection,
    table: &str,
) -> Result<usize, rusqlite::Error> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
    Ok(count as usize)
}
