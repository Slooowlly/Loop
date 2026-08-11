//! Ciclo de vida do save: criacao, carga, exclusao e listagem de carreiras, alem da
//! abertura dos recursos do save e do reparo de consistencia dos contratos regulares.

use super::*;

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
        let mut season = Season::new(season_id.clone(), 1, 2024);
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
                Ok(n)
            })
            .map_err(|e| format!("Falha ao persistir dados da carreira: {e}"))?;

        let player_team = world
            .teams
            .iter()
            .find(|team| team.id == world.player_team_id)
            .ok_or_else(|| "Equipe do jogador nao encontrada apos gerar o mundo".to_string())?;

        let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let meta = serde_json::json!({
            "version": 1,
            "career_number": career_number,
            "player_name": normalized_name,
            "current_season": 1,
            "current_year": 2024,
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
    let career_number =
        career_number_from_id(career_id).ok_or_else(|| "ID de carreira invalido.".to_string())?;
    let mut config = AppConfig::load_or_default(base_dir);
    let (db, career_dir, mut meta) = open_career_resources(base_dir, career_id)?;
    let meta_path = career_dir.join("meta.json");
    let mut active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let pending_regular_races = calendar_queries::count_pending_races_in_phase(
        &db.conn,
        &active_season.id,
        &SeasonPhase::BlocoRegular,
    )
    .map_err(|e| format!("Falha ao verificar corridas regulares pendentes: {e}"))?;
    if active_season.fase == SeasonPhase::JanelaConvocacao && pending_regular_races > 0 {
        season_queries::update_season_fase(&db.conn, &active_season.id, &SeasonPhase::BlocoRegular)
            .map_err(|e| format!("Falha ao corrigir fase da temporada: {e}"))?;
        active_season.fase = SeasonPhase::BlocoRegular;
    }
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar piloto do jogador: {e}"))?;
    let player_team = find_player_team(&db.conn, &player.id, active_season.fase)?;

    // Telemetria: onde esta carreira está no mundo (ano/categoria/dificuldade/progresso).
    // Fica aqui porque `load_career` é o ponto por onde TODA carreira aberta passa —
    // inclusive depois de virar a temporada, quando a UI recarrega. Só grava num estático
    // em memória; quem envia é a borda de corrida, e só se o jogador tiver consentido.
    // Sem equipe (agente livre) a categoria vem do último campeonato do piloto.
    //
    // A dificuldade viaja em TODO evento (não só no fim de corrida) porque é o eixo pelo
    // qual o desfecho é lido: posição e ritmo só calibram a curva se você souber em que
    // nível aquela corrida foi disputada.
    // `numero` e não `ano`: o que interessa é o ANO DA CARREIRA (1, 2, 3…). O ano do
    // calendário não diz onde a pessoa está na progressão, e duas carreiras começadas
    // em anos diferentes ficariam incomparáveis por nada.
    crate::telemetry::set_career_context(
        active_season.numero as i32,
        player_team
            .as_ref()
            .map(|t| t.categoria.clone())
            .or_else(|| player.categoria_atual.clone())
            .unwrap_or_else(|| "sem_equipe".to_string()),
        meta.difficulty.clone(),
        player.stats_carreira.temporadas as i32,
        player.stats_carreira.corridas as i32,
    );

    // PRÉ-TEMPORADA: pré-gera a matéria "O Que Esperar" em background, para a revista já
    // abrir com o texto pronto em vez de mostrar "escrevendo a prévia…" e esperar o
    // servidor. Passada a 1ª etapa da categoria a revista troca essa matéria pela edição da
    // corrida, então fora da pré-temporada não há o que adiantar. O trabalho é idempotente:
    // com a matéria já em cache a thread só faz uma leitura e sai.
    if let Some(ref team) = player_team {
        let corridas_concluidas = calendar_queries::count_races_by_status(
            &db.conn,
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
    // Usa race.categoria como fonte semântica do campeonato do evento.
    let event_interest_summary: Option<EventInterestSummary> = next_race.as_ref().map(|race| {
        let champ = standings_queries::get_championship_context(&db.conn, &race.categoria)
            .unwrap_or(ChampionshipContext {
                player_position: 0,
                gap_to_leader: 0,
            });
        let remaining = total_rodadas - race.rodada;
        let is_title_decider =
            remaining <= 2 && champ.gap_to_leader <= 50 && champ.player_position > 0;
        let ctx = EventInterestContext {
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
        };
        let result = calculate_expected_event_interest(&ctx);
        to_summary(&result)
    });

    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    meta.last_played = now.clone();
    write_save_meta(&meta_path, &meta)?;
    config.last_career = Some(career_number);
    config
        .save()
        .map_err(|e| format!("Falha ao atualizar config do app: {e}"))?;

    let team_summary = player_team
        .as_ref()
        .map(|team| {
            build_team_summary(&db.conn, team)
                .map_err(|e| format!("Falha ao montar resumo da equipe: {e}"))
        })
        .transpose()?;
    let accepted_special_offer = build_accepted_special_offer_summary(&db.conn, &player)?;
    // Cota de público do jogador (Fase 3 do Estrelato): fama do lineup da equipe do
    // jogador vs o grid da próxima corrida → fração do portão que a equipe captura
    // (piso + prêmio de estrela, mesma conta da bilheteria). `None` sem equipe.
    let public_fame_share: Option<f64> = next_race.as_ref().and_then(|race| {
        let team = player_team.as_ref()?;
        let category_teams = team_queries::get_teams_by_category(&db.conn, &race.categoria).ok()?;
        let grid_total: f64 = category_teams
            .iter()
            .map(|t| {
                let medias =
                    team_queries::get_team_lineup_medias(&db.conn, &t.id).unwrap_or_default();
                crate::public_presence::team::derive_team_public_presence(&medias)
            })
            .sum();
        let team_medias =
            team_queries::get_team_lineup_medias(&db.conn, &team.id).unwrap_or_default();
        let team_presence = crate::public_presence::team::derive_team_public_presence(&team_medias);
        let n = category_teams.len().max(1) as f64;
        Some(crate::finance::cashflow::team_gate_share(
            team_presence,
            grid_total,
            n,
        ))
    });
    let next_race_summary = next_race.as_ref().map(|race| RaceSummary {
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
        event_interest: event_interest_summary.clone(),
        public_fame_share,
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
        player: DriverSummary {
            id: player.id.clone(),
            nome: player.nome.clone(),
            nacionalidade: player.nacionalidade.clone(),
            idade: player.idade as i32,
            skill: player.atributos.skill.round().clamp(0.0, 100.0) as u8,
            midia: player.atributos.midia.round().clamp(0.0, 100.0) as u8,
            categoria_especial_ativa: player.categoria_especial_ativa.clone(),
            equipe_id: player_team.as_ref().map(|t| t.id.clone()),
            equipe_nome: player_team.as_ref().map(|t| t.nome.clone()),
            equipe_nome_curto: player_team.as_ref().map(|t| t.nome_curto.clone()),
            equipe_cor: player_team
                .as_ref()
                .map(|t| t.cor_primaria.clone())
                .unwrap_or_default(),
            classe: player_team.as_ref().and_then(|t| t.classe.clone()),
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
        },
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
    let career_number =
        career_number_from_id(career_id).ok_or_else(|| "ID de carreira invalido.".to_string())?;
    let mut config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(career_id);

    if !career_dir.exists() {
        return Err("Save nao encontrado.".to_string());
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
        return Err("Informe um nome para o piloto.".to_string());
    }
    if name.chars().count() > 50 {
        return Err("O nome do piloto deve ter no maximo 50 caracteres.".to_string());
    }
    if get_nationality(&nationality_id).is_none() {
        return Err("Selecione uma nacionalidade valida.".to_string());
    }
    if !matches!(category.as_str(), "mazda_rookie" | "toyota_rookie") {
        return Err("A categoria inicial deve ser Mazda Rookie ou Toyota Rookie.".to_string());
    }
    if input.team_index > 5 {
        return Err("A equipe escolhida e invalida para a categoria inicial.".to_string());
    }
    if scoring::get_difficulty_config(&difficulty).is_none() {
        return Err("Selecione uma dificuldade valida.".to_string());
    }
    if let Some(age) = input.player_age {
        if !(16..=60).contains(&age) {
            return Err("A idade do piloto deve ficar entre 16 e 60 anos.".to_string());
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

// Internal diagnostic helper kept out of the production Tauri command surface.
#[allow(dead_code)]
pub(crate) fn verify_database(
    app: AppHandle,
    career_number: u32,
) -> Result<VerifyDatabaseResponse, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.career_db_path(career_number);

    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let table_count: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("Falha ao contar tabelas: {e}"))?;

    Ok(VerifyDatabaseResponse {
        career_number,
        db_path: db_path.to_string_lossy().to_string(),
        table_count,
        status: "ok".to_string(),
    })
}

// Internal diagnostic helper kept out of the production Tauri command surface.
#[allow(dead_code)]
pub(crate) fn test_create_driver(
    app: AppHandle,
    career_number: u32,
    nome: String,
    nacionalidade: String,
    genero: String,
    category_tier: u32,
    difficulty: String,
) -> Result<Driver, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.career_db_path(career_number);
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let id = next_id(&db.conn, IdType::Driver).map_err(|e| format!("Falha ao gerar ID: {e}"))?;

    let mut rng = rand::thread_rng();
    let category_id = match category_tier {
        0 => "mazda_rookie",
        1 => "mazda_amador",
        2 => "bmw_m2",
        3 => "gt4",
        4 => "gt3",
        5 => "endurance",
        _ => "endurance",
    };
    let mut existing_names = HashSet::new();
    let mut generated = Driver::generate_for_category(
        category_id,
        category_tier.min(5) as u8,
        &difficulty,
        1,
        &mut existing_names,
        &mut rng,
    );
    let mut driver = generated
        .pop()
        .ok_or_else(|| "Falha ao gerar piloto de teste".to_string())?;
    driver.id = id;
    if !nome.trim().is_empty() {
        driver.nome = nome;
    }
    if !nacionalidade.trim().is_empty() {
        driver.nacionalidade = nacionalidade;
    }
    if !genero.trim().is_empty() {
        driver.genero = genero;
    }

    driver_queries::insert_driver(&db.conn, &driver)
        .map_err(|e| format!("Falha ao inserir piloto: {e}"))?;

    Ok(driver)
}

// Internal diagnostic helper kept out of the production Tauri command surface.
#[allow(dead_code)]
pub(crate) fn test_list_drivers(app: AppHandle, career_number: u32) -> Result<Vec<Driver>, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.career_db_path(career_number);
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    driver_queries::get_all_drivers(&db.conn).map_err(|e| format!("Falha ao listar pilotos: {e}"))
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

pub(crate) fn open_career_resources_for_category_read(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<(Database, std::path::PathBuf, SaveMeta), String> {
    let (db, career_dir, meta) = open_career_resources_read_only(base_dir, career_id)?;
    let _ = category;
    Ok((db, career_dir, meta))
}

pub(crate) fn open_career_resources_with_repair(
    base_dir: &Path,
    career_id: &str,
    repair_contracts: bool,
) -> Result<(Database, std::path::PathBuf, SaveMeta), String> {
    let _career_number =
        career_number_from_id(career_id).ok_or_else(|| "ID de carreira invalido.".to_string())?;

    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(career_id);
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");

    if !career_dir.exists() {
        return Err("Save nao encontrado.".to_string());
    }
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
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
    let drivers = driver_queries::get_all_drivers(&tx)
        .map_err(|e| format!("Falha ao carregar pilotos para reparo: {e}"))?;
    let drivers_by_id = drivers
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect::<HashMap<_, _>>();
    let active_regular_contracts = contract_queries::get_all_active_regular_contracts(&tx)
        .map_err(|e| format!("Falha ao recarregar contratos regulares ativos: {e}"))?;
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
    if allow_regular_vacancy_fill {
        if let Some(active_season) = season_queries::get_active_season(conn)
            .map_err(|e| format!("Falha ao buscar temporada ativa para reparo de vagas: {e}"))?
        {
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
            let has_pending = pending_regular_races + pending_temporada_races > 0;
            if active_season.fase.is_racing() && has_pending {
                let mut rng = rand::thread_rng();
                fill_all_remaining_vacancies(conn, active_season.numero, &mut rng)
                    .map_err(|e| format!("Falha ao preencher vagas regulares pendentes: {e}"))?;
            }
        }
    }
    Ok(())
}

pub(crate) fn count_rows(
    conn: &rusqlite::Connection,
    table: &str,
) -> Result<usize, rusqlite::Error> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
    Ok(count as usize)
}
