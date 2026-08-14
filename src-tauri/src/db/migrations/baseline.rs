//! DDL da baseline: o schema final completo, criado de uma vez só.
//!
//! Substitui as 53 migrações incrementais que existiam até aqui. O resultado é
//! semanticamente idêntico ao que aquela sequência produzia num banco novo —
//! quem garante isso é a trava em [`super::schema_ouro`], que compara o schema
//! gerado com um fixture capturado ANTES do colapso.
//!
//! Saves antigos não são migrados: a baseline só sabe criar um banco do zero.

/// Schema completo do jogo. Ordem: núcleo (meta/config/pilotos/equipes),
/// competição (temporadas, calendário, corridas, resultados), mercado,
/// arquivos históricos e os sistemas satélites (janela especial, finanças,
/// carro, rivalidades).
pub const BASELINE_DDL: &str = "

-- ══ Núcleo ═══════════════════════════════════════════════════════════════════

-- meta: configuração interna do banco (inclui o schema_version)
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- config: configurações do usuário (espelha config.json)
CREATE TABLE IF NOT EXISTS config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- drivers: pilotos (jogador + IA)
CREATE TABLE IF NOT EXISTS drivers (
    id                       TEXT PRIMARY KEY,
    nome                     TEXT NOT NULL,
    is_jogador               INTEGER NOT NULL DEFAULT 0,
    idade                    INTEGER NOT NULL,
    nacionalidade            TEXT NOT NULL,
    genero                   TEXT NOT NULL DEFAULT 'M',
    categoria_atual          TEXT,
    categoria_especial_ativa TEXT,
    status                   TEXT NOT NULL DEFAULT 'Ativo',
    personalidade_primaria   TEXT,
    personalidade_secundaria TEXT,
    ano_inicio_carreira      INTEGER NOT NULL DEFAULT 2024,

    -- Atributos — todos 0-100
    skill                    REAL NOT NULL DEFAULT 50.0,
    consistencia             REAL NOT NULL DEFAULT 50.0,
    racecraft                REAL NOT NULL DEFAULT 50.0,
    defesa                   REAL NOT NULL DEFAULT 50.0,
    ritmo_classificacao      REAL NOT NULL DEFAULT 50.0,
    gestao_pneus             REAL NOT NULL DEFAULT 50.0,
    habilidade_largada       REAL NOT NULL DEFAULT 50.0,
    adaptabilidade           REAL NOT NULL DEFAULT 50.0,
    fator_chuva              REAL NOT NULL DEFAULT 50.0,
    fitness                  REAL NOT NULL DEFAULT 50.0,
    experiencia              REAL NOT NULL DEFAULT 50.0,
    desenvolvimento          REAL NOT NULL DEFAULT 50.0,
    aggression               REAL NOT NULL DEFAULT 50.0,
    smoothness               REAL NOT NULL DEFAULT 50.0,
    midia                    REAL NOT NULL DEFAULT 50.0,
    carisma                  REAL NOT NULL DEFAULT 50.0,
    mentalidade              REAL NOT NULL DEFAULT 50.0,
    confianca                REAL NOT NULL DEFAULT 50.0,
    potencial                REAL NOT NULL DEFAULT 0.0,

    -- Stats da temporada corrente
    temp_pontos              REAL NOT NULL DEFAULT 0.0,
    temp_vitorias            INTEGER NOT NULL DEFAULT 0,
    temp_podios              INTEGER NOT NULL DEFAULT 0,
    temp_poles               INTEGER NOT NULL DEFAULT 0,
    temp_corridas            INTEGER NOT NULL DEFAULT 0,
    temp_dnfs                INTEGER NOT NULL DEFAULT 0,
    temp_posicao_media       REAL NOT NULL DEFAULT 0.0,

    -- Stats de carreira acumuladas
    carreira_pontos_total    REAL NOT NULL DEFAULT 0.0,
    carreira_vitorias        INTEGER NOT NULL DEFAULT 0,
    carreira_podios          INTEGER NOT NULL DEFAULT 0,
    carreira_poles           INTEGER NOT NULL DEFAULT 0,
    carreira_corridas        INTEGER NOT NULL DEFAULT 0,
    carreira_temporadas      INTEGER NOT NULL DEFAULT 0,
    carreira_titulos         INTEGER NOT NULL DEFAULT 0,
    carreira_dnfs            INTEGER NOT NULL DEFAULT 0,

    -- Tracking dinâmico
    motivacao                  REAL NOT NULL DEFAULT 75.0,
    historico_circuitos        TEXT NOT NULL DEFAULT '{}',
    ultimos_resultados         TEXT NOT NULL DEFAULT '[]',
    melhor_resultado_temp      INTEGER,
    temporadas_na_categoria    INTEGER NOT NULL DEFAULT 0,
    corridas_na_categoria      INTEGER NOT NULL DEFAULT 0,
    temporadas_motivacao_baixa INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_drivers_status    ON drivers(status);
CREATE INDEX IF NOT EXISTS idx_drivers_categoria ON drivers(categoria_atual);

-- teams: equipes
CREATE TABLE IF NOT EXISTS teams (
    id                   TEXT PRIMARY KEY,
    nome                 TEXT NOT NULL,
    nome_curto           TEXT NOT NULL DEFAULT '',
    categoria            TEXT NOT NULL,
    categoria_anterior   TEXT,
    classe               TEXT,
    marca                TEXT,
    is_player_team       INTEGER NOT NULL DEFAULT 0,
    ativa                INTEGER NOT NULL DEFAULT 1,
    cor_primaria         TEXT NOT NULL DEFAULT '#FFFFFF',
    cor_secundaria       TEXT NOT NULL DEFAULT '#000000',
    pais_sede            TEXT NOT NULL DEFAULT 'Unknown',
    ano_fundacao         INTEGER NOT NULL DEFAULT 2024,

    -- Performance e estrutura
    car_performance      REAL NOT NULL DEFAULT 50.0,
    confiabilidade       REAL NOT NULL DEFAULT 50.0,
    budget               REAL NOT NULL DEFAULT 1000000.0,
    reputacao            REAL NOT NULL DEFAULT 50.0,
    facilities           REAL NOT NULL DEFAULT 50.0,
    engineering          REAL NOT NULL DEFAULT 50.0,
    morale               REAL NOT NULL DEFAULT 1.0,
    aerodinamica         REAL NOT NULL DEFAULT 50.0,
    motor                REAL NOT NULL DEFAULT 50.0,
    chassi               REAL NOT NULL DEFAULT 50.0,
    car_build_profile    TEXT NOT NULL DEFAULT 'balanced',
    pit_strategy_risk    REAL NOT NULL DEFAULT 50.0,
    pit_crew_quality     REAL NOT NULL DEFAULT 50.0,

    -- Hierarquia interna e dupla de pilotos
    hierarquia_status              TEXT NOT NULL DEFAULT 'Independente',
    parent_team_id                 TEXT,
    aceita_rookies                 INTEGER NOT NULL DEFAULT 1,
    meta_posicao                   INTEGER NOT NULL DEFAULT 10,
    piloto_1_id                    TEXT REFERENCES drivers(id),
    piloto_2_id                    TEXT REFERENCES drivers(id),
    hierarquia_n1_id               TEXT,
    hierarquia_n2_id               TEXT,
    hierarquia_tensao              REAL NOT NULL DEFAULT 0.0,
    hierarquia_duelos_total        INTEGER NOT NULL DEFAULT 0,
    hierarquia_duelos_n2_vencidos  INTEGER NOT NULL DEFAULT 0,
    hierarquia_sequencia_n2        INTEGER NOT NULL DEFAULT 0,
    hierarquia_sequencia_n1        INTEGER NOT NULL DEFAULT 0,
    hierarquia_inversoes_temporada INTEGER NOT NULL DEFAULT 0,

    -- Stats da temporada corrente e histórico
    temp_posicao              INTEGER NOT NULL DEFAULT 0,
    carreira_titulos          INTEGER NOT NULL DEFAULT 0,
    stats_vitorias            INTEGER NOT NULL DEFAULT 0,
    stats_podios              INTEGER NOT NULL DEFAULT 0,
    stats_poles               INTEGER NOT NULL DEFAULT 0,
    stats_pontos              INTEGER NOT NULL DEFAULT 0,
    stats_melhor_resultado    INTEGER NOT NULL DEFAULT 99,
    historico_vitorias        INTEGER NOT NULL DEFAULT 0,
    historico_podios          INTEGER NOT NULL DEFAULT 0,
    historico_poles           INTEGER NOT NULL DEFAULT 0,
    historico_pontos          INTEGER NOT NULL DEFAULT 0,
    historico_titulos_pilotos INTEGER NOT NULL DEFAULT 0,
    temporada_atual           INTEGER NOT NULL DEFAULT 1,

    -- Finanças
    cash_balance                 REAL NOT NULL DEFAULT 0.0,
    debt_balance                 REAL NOT NULL DEFAULT 0.0,
    financial_state              TEXT NOT NULL DEFAULT 'stable',
    season_strategy              TEXT NOT NULL DEFAULT 'balanced',
    last_round_income            REAL NOT NULL DEFAULT 0.0,
    last_round_expenses          REAL NOT NULL DEFAULT 0.0,
    last_round_net               REAL NOT NULL DEFAULT 0.0,
    parachute_payment_remaining  REAL NOT NULL DEFAULT 0.0,
    -- Socorro de emergência (finance::events): quantos a equipe já tomou e em qual temporada.
    -- O par existe para o limite por temporada se reiniciar sozinho na virada do ano, sem
    -- depender de um passo de transição que alguém pode esquecer de chamar.
    socorros_na_temporada        INTEGER NOT NULL DEFAULT 0,
    socorros_temporada_ref       INTEGER NOT NULL DEFAULT 0,

    created_at           TEXT NOT NULL DEFAULT '',
    updated_at           TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_teams_categoria ON teams(categoria);
CREATE INDEX IF NOT EXISTS idx_teams_ativa     ON teams(ativa);

-- contracts: contratos piloto ↔ equipe
CREATE TABLE IF NOT EXISTS contracts (
    id                TEXT PRIMARY KEY,
    piloto_id         TEXT NOT NULL,
    piloto_nome       TEXT NOT NULL DEFAULT '',
    equipe_id         TEXT NOT NULL,
    equipe_nome       TEXT NOT NULL DEFAULT '',
    categoria         TEXT NOT NULL DEFAULT '',
    classe            TEXT,
    tipo              TEXT NOT NULL DEFAULT 'Regular',
    status            TEXT NOT NULL DEFAULT 'Ativo',
    papel             TEXT NOT NULL DEFAULT 'Titular',
    salario           REAL NOT NULL DEFAULT 0.0,
    salario_anual     REAL NOT NULL DEFAULT 0.0,
    duracao_anos      INTEGER NOT NULL DEFAULT 1,
    temporada_inicio  TEXT NOT NULL,
    temporada_fim     TEXT NOT NULL,
    clausulas         TEXT NOT NULL DEFAULT '{}',
    created_at        TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (piloto_id) REFERENCES drivers(id),
    FOREIGN KEY (equipe_id) REFERENCES teams(id)
);

CREATE INDEX IF NOT EXISTS idx_contracts_piloto    ON contracts(piloto_id);
CREATE INDEX IF NOT EXISTS idx_contracts_equipe    ON contracts(equipe_id);
CREATE INDEX IF NOT EXISTS idx_contracts_status    ON contracts(status);
CREATE INDEX IF NOT EXISTS idx_contracts_categoria ON contracts(categoria);

-- Um único contrato ativo por (piloto, tipo).
CREATE UNIQUE INDEX IF NOT EXISTS idx_contracts_active_pilot_tipo
            ON contracts(piloto_id, tipo)
            WHERE status = 'Ativo';

-- ══ Competição ═══════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS seasons (
    id           TEXT PRIMARY KEY,
    numero       INTEGER NOT NULL,
    ano          INTEGER NOT NULL,
    status       TEXT NOT NULL DEFAULT 'Ativa',
    fase         TEXT NOT NULL DEFAULT 'BlocoRegular',
    rodada_atual INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL DEFAULT '',
    updated_at   TEXT NOT NULL DEFAULT ''
);

-- Uma única temporada em andamento por vez.
CREATE UNIQUE INDEX IF NOT EXISTS idx_seasons_single_active
            ON seasons(status)
            WHERE status = 'EmAndamento';

CREATE TABLE IF NOT EXISTS calendar (
    id           TEXT PRIMARY KEY,
    temporada_id TEXT NOT NULL,
    season_id    TEXT,
    rodada       INTEGER NOT NULL,
    nome         TEXT NOT NULL DEFAULT '',
    pista        TEXT NOT NULL,
    track_id     INTEGER NOT NULL DEFAULT 0,
    track_name   TEXT NOT NULL DEFAULT '',
    track_config TEXT NOT NULL DEFAULT '',
    categoria    TEXT NOT NULL,
    clima        TEXT NOT NULL DEFAULT 'Seco',
    temperatura  REAL NOT NULL DEFAULT 25.0,
    umidade      REAL NOT NULL DEFAULT 45.0,
    vento        REAL NOT NULL DEFAULT 25.0,
    duracao      INTEGER NOT NULL DEFAULT 60,
    voltas       INTEGER NOT NULL DEFAULT 10,
    duracao_corrida_min       INTEGER NOT NULL DEFAULT 60,
    duracao_classificacao_min INTEGER NOT NULL DEFAULT 15,
    status       TEXT NOT NULL DEFAULT 'Pendente',
    data         TEXT NOT NULL DEFAULT '',
    horario      TEXT NOT NULL DEFAULT '14:00',
    week_of_year INTEGER NOT NULL DEFAULT 0,
    season_week  INTEGER,
    season_phase TEXT NOT NULL DEFAULT 'BlocoRegular',
    thematic_slot TEXT,
    FOREIGN KEY (temporada_id) REFERENCES seasons(id)
);

CREATE INDEX IF NOT EXISTS idx_calendar_temporada ON calendar(temporada_id);
CREATE INDEX IF NOT EXISTS idx_calendar_season_id ON calendar(season_id);
CREATE INDEX IF NOT EXISTS idx_calendar_categoria ON calendar(categoria);

-- `races` está MORTA. A fonte de verdade de uma corrida é `calendar`, e só ela.
--
-- As duas tabelas descrevem o mesmo conceito, e por um tempo o código escreveu nas duas.
-- A última escrita de produção saiu no commit e7e91d4, de 05/05/2026; desde então nenhuma
-- consulta do jogo lê, escreve ou junta com ela — as únicas menções que sobraram no
-- repositório são fixtures de teste que semeiam a linha junto com a de `calendar`.
-- `race_results.race_id` aponta para `calendar.id`.
--
-- Quem for escrever consulta nova: use `calendar`. O guard
-- `scripts/tests/races-tabela-morta.test.mjs` falha se código de produção voltar a tocar
-- nesta tabela.
--
-- Por que ela continua aqui: o `DROP TABLE` apaga dado de save em campo e não tem volta.
-- O que ele apagaria já é redundante com `calendar`, mas a decisão de destruir é do dono
-- do jogo, não desta limpeza. Fica declarada e travada até alguém decidir o contrário.
CREATE TABLE IF NOT EXISTS races (
    id           TEXT PRIMARY KEY,
    temporada_id TEXT NOT NULL,
    calendar_id  TEXT NOT NULL,
    rodada       INTEGER NOT NULL,
    pista        TEXT NOT NULL,
    data         TEXT NOT NULL DEFAULT '',
    clima        TEXT NOT NULL DEFAULT 'Seco',
    status       TEXT NOT NULL DEFAULT 'Pendente',
    FOREIGN KEY (temporada_id) REFERENCES seasons(id),
    FOREIGN KEY (calendar_id)  REFERENCES calendar(id)
);

CREATE INDEX IF NOT EXISTS idx_races_temporada ON races(temporada_id);

CREATE TABLE IF NOT EXISTS race_results (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            race_id             TEXT NOT NULL,
            piloto_id           TEXT NOT NULL,
            equipe_id           TEXT NOT NULL,
            posicao_largada     INTEGER NOT NULL DEFAULT 0,
            posicao_final       INTEGER NOT NULL DEFAULT 0,
            voltas_completadas  INTEGER NOT NULL DEFAULT 0,
            dnf                 INTEGER NOT NULL DEFAULT 0,
            pontos              REAL NOT NULL DEFAULT 0.0,
            tempo_total         REAL NOT NULL DEFAULT 0.0,
            gap_to_winner_ms    REAL NOT NULL DEFAULT 0.0,
            fastest_lap         INTEGER NOT NULL DEFAULT 0,
            final_tire_wear     REAL NOT NULL DEFAULT 1.0,
            dnf_reason          TEXT,
            dnf_segment         TEXT,
            dnf_catalog_id      TEXT,
            damage_origin_segment TEXT,
            incidents_count     INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (race_id)    REFERENCES calendar(id),
            FOREIGN KEY (piloto_id)  REFERENCES drivers(id),
            FOREIGN KEY (equipe_id)  REFERENCES teams(id)
        );

-- Um resultado por (corrida, piloto).
CREATE UNIQUE INDEX IF NOT EXISTS idx_race_results_unique_race_pilot
            ON race_results(race_id, piloto_id);

CREATE TABLE IF NOT EXISTS standings (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            temporada_id TEXT NOT NULL,
            piloto_id    TEXT NOT NULL,
            equipe_id    TEXT,
            categoria    TEXT NOT NULL,
            classe       TEXT,
            posicao      INTEGER NOT NULL DEFAULT 0,
            pontos       REAL NOT NULL DEFAULT 0.0,
            vitorias     INTEGER NOT NULL DEFAULT 0,
            podios       INTEGER NOT NULL DEFAULT 0,
            poles        INTEGER NOT NULL DEFAULT 0,
            corridas     INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (temporada_id) REFERENCES seasons(id),
            FOREIGN KEY (piloto_id)    REFERENCES drivers(id),
            FOREIGN KEY (equipe_id)    REFERENCES teams(id)
        );

-- race_breakdowns: quebras de peça por corrida/piloto
CREATE TABLE IF NOT EXISTS race_breakdowns (
            race_id      TEXT NOT NULL,
            driver_id    TEXT NOT NULL,
            part         TEXT NOT NULL,
            problem      INTEGER NOT NULL DEFAULT 0,
            lap          INTEGER NOT NULL,
            severity     TEXT NOT NULL,
            penalty_secs INTEGER,
            forced       INTEGER NOT NULL DEFAULT 0,
            label        TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (race_id, driver_id, part, lap)
        );

CREATE INDEX IF NOT EXISTS idx_race_breakdowns_race ON race_breakdowns(race_id);

-- incident_catalog: catálogo de incidentes (semeado pela baseline)
CREATE TABLE IF NOT EXISTS incident_catalog (
            id                TEXT PRIMARY KEY,
            vehicle_class     TEXT NOT NULL,
            race_format       TEXT NOT NULL,
            incident_source   TEXT NOT NULL,
            trigger_type      TEXT NOT NULL,
            severity_context  TEXT NOT NULL,
            weight_sprint     INTEGER NOT NULL DEFAULT 0,
            weight_endurance  INTEGER NOT NULL DEFAULT 0,
            dnf_template      TEXT NOT NULL,
            non_dnf_template  TEXT,
            description_short TEXT NOT NULL
        );

CREATE INDEX IF NOT EXISTS idx_incident_catalog_class_format
            ON incident_catalog(vehicle_class, race_format);
CREATE INDEX IF NOT EXISTS idx_incident_catalog_source
            ON incident_catalog(incident_source);

CREATE TABLE IF NOT EXISTS track_dnf_history (
            id             TEXT PRIMARY KEY,
            piloto_id      TEXT NOT NULL,
            track_name     TEXT NOT NULL,
            season_num     INTEGER NOT NULL,
            round          INTEGER NOT NULL,
            dnf_reason     TEXT NOT NULL,
            collision_with TEXT,
            created_at     TEXT NOT NULL
        );

CREATE INDEX IF NOT EXISTS idx_track_dnf_piloto_track
            ON track_dnf_history(piloto_id, track_name);

CREATE TABLE IF NOT EXISTS player_race_telemetry (
            race_id          TEXT PRIMARY KEY,
            laps_seen        INTEGER NOT NULL DEFAULT 0,
            race_laps        INTEGER NOT NULL DEFAULT 0,
            consistency      REAL NOT NULL DEFAULT -1.0,
            battle_fraction  REAL NOT NULL DEFAULT -1.0,
            on_track_gained  INTEGER NOT NULL DEFAULT 0,
            on_track_lost    INTEGER NOT NULL DEFAULT 0,
            start_delta      INTEGER NOT NULL DEFAULT 0,
            start_valid      INTEGER NOT NULL DEFAULT 0,
            created_at       TEXT NOT NULL DEFAULT (datetime('now'))
        );

-- ══ Pilotos: lesões, licenças, vínculos ══════════════════════════════════════

CREATE TABLE IF NOT EXISTS injuries (
            id                  TEXT PRIMARY KEY,
            pilot_id            TEXT NOT NULL,
            type                TEXT NOT NULL,
            injury_name         TEXT NOT NULL DEFAULT '',
            modifier            REAL NOT NULL DEFAULT 1.0,
            races_total         INTEGER NOT NULL,
            races_remaining     INTEGER NOT NULL,
            skill_penalty       REAL NOT NULL DEFAULT 0.0,
            season              INTEGER NOT NULL,
            race_occurred       TEXT NOT NULL,
            active              INTEGER NOT NULL DEFAULT 1,
            FOREIGN KEY (pilot_id) REFERENCES drivers(id)
        );

CREATE INDEX IF NOT EXISTS idx_injuries_pilot_id ON injuries(pilot_id);
CREATE INDEX IF NOT EXISTS idx_injuries_active   ON injuries(active);

-- Uma única lesão ativa por piloto.
CREATE UNIQUE INDEX IF NOT EXISTS idx_injuries_single_active_per_pilot
        ON injuries(pilot_id)
        WHERE active = 1;

CREATE TABLE IF NOT EXISTS licenses (
    id                       INTEGER PRIMARY KEY AUTOINCREMENT,
    piloto_id                TEXT NOT NULL,
    nivel                    TEXT NOT NULL,
    categoria_origem         TEXT NOT NULL,
    data_obtencao            TEXT NOT NULL DEFAULT '',
    temporadas_na_categoria  INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (piloto_id) REFERENCES drivers(id)
);

CREATE TABLE IF NOT EXISTS driver_favorites (
            piloto_id  TEXT PRIMARY KEY,
            criado_em  TEXT NOT NULL DEFAULT (datetime('now'))
        );

CREATE TABLE IF NOT EXISTS driver_team_bond (
            piloto_id   TEXT NOT NULL,
            equipe_id   TEXT NOT NULL,
            vinculo     REAL NOT NULL DEFAULT 0.0,
            temporadas  INTEGER NOT NULL DEFAULT 0,
            updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (piloto_id, equipe_id)
        );

CREATE TABLE IF NOT EXISTS rivalries (
    id                   TEXT PRIMARY KEY,
    piloto1_id           TEXT NOT NULL,
    piloto2_id           TEXT NOT NULL,
    intensidade          REAL NOT NULL DEFAULT 0.0,
    historical_intensity REAL NOT NULL DEFAULT 0.0,
    recent_activity      REAL NOT NULL DEFAULT 0.0,
    tipo                 TEXT NOT NULL DEFAULT 'Normal',
    temporada_update     INTEGER NOT NULL DEFAULT 0,
    criado_em            TEXT NOT NULL DEFAULT '',
    ultima_atualizacao   TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (piloto1_id) REFERENCES drivers(id),
    FOREIGN KEY (piloto2_id) REFERENCES drivers(id)
);

CREATE INDEX IF NOT EXISTS idx_rivalries_piloto1 ON rivalries(piloto1_id);
CREATE INDEX IF NOT EXISTS idx_rivalries_piloto2 ON rivalries(piloto2_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_rivalries_pair_unique
    ON rivalries(piloto1_id, piloto2_id);

CREATE TABLE IF NOT EXISTS team_rivalries (
            id                   TEXT PRIMARY KEY,
            team1_id             TEXT NOT NULL,
            team2_id             TEXT NOT NULL,
            historical_intensity REAL NOT NULL DEFAULT 0.0,
            recent_activity      REAL NOT NULL DEFAULT 0.0,
            tipo                 TEXT NOT NULL DEFAULT 'Campeonato',
            criado_em            TEXT NOT NULL DEFAULT '',
            ultima_atualizacao   TEXT NOT NULL DEFAULT '',
            temporada_update     INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (team1_id) REFERENCES teams(id),
            FOREIGN KEY (team2_id) REFERENCES teams(id)
        );

CREATE INDEX IF NOT EXISTS idx_team_rivalries_team1 ON team_rivalries(team1_id);
CREATE INDEX IF NOT EXISTS idx_team_rivalries_team2 ON team_rivalries(team2_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_team_rivalries_pair_unique
            ON team_rivalries(team1_id, team2_id);

-- ══ Mercado ══════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS market (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    temporada_id TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'Fechado',
    fase         TEXT NOT NULL DEFAULT 'PreTemporada',
    inicio       TEXT NOT NULL DEFAULT '',
    fim          TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (temporada_id) REFERENCES seasons(id)
);

CREATE TABLE IF NOT EXISTS market_proposals (
    id              TEXT PRIMARY KEY,
    temporada_id    TEXT NOT NULL,
    equipe_id       TEXT NOT NULL,
    piloto_id       TEXT NOT NULL,
    papel           TEXT NOT NULL DEFAULT 'Titular',
    salario         REAL NOT NULL DEFAULT 0.0,
    status          TEXT NOT NULL DEFAULT 'Pendente',
    motivo_recusa   TEXT,
    criado_em       TEXT NOT NULL DEFAULT '',
    respondido_em   TEXT,
    semana_limite   INTEGER,
    FOREIGN KEY (temporada_id) REFERENCES seasons(id),
    FOREIGN KEY (equipe_id)    REFERENCES teams(id),
    FOREIGN KEY (piloto_id)    REFERENCES drivers(id)
);

CREATE INDEX IF NOT EXISTS idx_market_proposals_piloto ON market_proposals(piloto_id);
CREATE INDEX IF NOT EXISTS idx_market_proposals_equipe ON market_proposals(equipe_id);

CREATE TABLE IF NOT EXISTS transfer_window (
            season_number INTEGER PRIMARY KEY,
            state_json    TEXT NOT NULL,
            status        TEXT NOT NULL DEFAULT 'open',
            updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
        );

-- ══ Bloco especial (janela de convocações) ═══════════════════════════════════

CREATE TABLE IF NOT EXISTS player_special_offers (
    id                TEXT PRIMARY KEY,
    season_id         TEXT NOT NULL,
    player_driver_id  TEXT NOT NULL,
    team_id           TEXT NOT NULL,
    team_name         TEXT NOT NULL,
    special_category  TEXT NOT NULL,
    class_name        TEXT NOT NULL,
    papel             TEXT NOT NULL DEFAULT 'Numero2',
    status            TEXT NOT NULL DEFAULT 'Pendente',
    available_from_day INTEGER NOT NULL DEFAULT 1,
    selected_for_day   INTEGER NOT NULL DEFAULT 0,
    resolved_day       INTEGER,
    created_at        TEXT NOT NULL DEFAULT '',
    responded_at      TEXT,
    FOREIGN KEY (season_id) REFERENCES seasons(id),
    FOREIGN KEY (player_driver_id) REFERENCES drivers(id),
    FOREIGN KEY (team_id) REFERENCES teams(id)
);

CREATE INDEX IF NOT EXISTS idx_player_special_offers_player ON player_special_offers(player_driver_id);
CREATE INDEX IF NOT EXISTS idx_player_special_offers_team   ON player_special_offers(team_id);

CREATE TABLE IF NOT EXISTS special_window_state (
            season_id        TEXT PRIMARY KEY,
            current_day      INTEGER NOT NULL DEFAULT 1,
            total_days       INTEGER NOT NULL DEFAULT 7,
            status           TEXT NOT NULL DEFAULT 'Aberta',
            active_offer_id  TEXT,
            player_result    TEXT,
            created_at       TEXT NOT NULL DEFAULT '',
            updated_at       TEXT NOT NULL DEFAULT ''
        );

CREATE TABLE IF NOT EXISTS special_window_assignments (
            id               TEXT PRIMARY KEY,
            season_id        TEXT NOT NULL,
            special_category TEXT NOT NULL,
            class_name       TEXT NOT NULL,
            team_id          TEXT NOT NULL,
            driver_id        TEXT NOT NULL,
            papel            TEXT NOT NULL,
            reveal_day       INTEGER NOT NULL DEFAULT 1,
            revealed         INTEGER NOT NULL DEFAULT 0,
            new_badge_day    INTEGER,
            is_player        INTEGER NOT NULL DEFAULT 0
        );

CREATE TABLE IF NOT EXISTS special_window_candidate_pool (
            season_id           TEXT NOT NULL,
            driver_id           TEXT NOT NULL,
            driver_name         TEXT NOT NULL,
            origin_category     TEXT NOT NULL,
            license_level       INTEGER,
            desirability        INTEGER NOT NULL DEFAULT 0,
            production_eligible INTEGER NOT NULL DEFAULT 0,
            endurance_eligible  INTEGER NOT NULL DEFAULT 0,
            status              TEXT NOT NULL DEFAULT 'Livre',
            PRIMARY KEY (season_id, driver_id)
        );

CREATE TABLE IF NOT EXISTS special_window_daily_log (
            id               TEXT PRIMARY KEY,
            season_id        TEXT NOT NULL,
            day_number       INTEGER NOT NULL,
            event_type       TEXT NOT NULL,
            message          TEXT NOT NULL,
            special_category TEXT,
            class_name       TEXT,
            team_id          TEXT,
            driver_id        TEXT,
            created_at       TEXT NOT NULL DEFAULT ''
        );

CREATE INDEX IF NOT EXISTS idx_special_window_assignments_season
            ON special_window_assignments(season_id, reveal_day, revealed);
CREATE INDEX IF NOT EXISTS idx_special_window_candidate_pool_season
            ON special_window_candidate_pool(season_id, status);
CREATE INDEX IF NOT EXISTS idx_special_window_daily_log_season
            ON special_window_daily_log(season_id, day_number);

CREATE TABLE IF NOT EXISTS special_team_entries (
            season_id            TEXT NOT NULL,
            special_category     TEXT NOT NULL,
            class_name           TEXT NOT NULL,
            team_id              TEXT NOT NULL,
            source_category      TEXT NOT NULL,
            qualified_via        TEXT NOT NULL,
            guaranteed_next_year INTEGER NOT NULL DEFAULT 0,
            created_at           TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at           TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (season_id, special_category, class_name, team_id),
            FOREIGN KEY (season_id) REFERENCES seasons(id),
            FOREIGN KEY (team_id) REFERENCES teams(id)
        );

CREATE INDEX IF NOT EXISTS idx_special_team_entries_lookup
            ON special_team_entries(season_id, special_category, class_name);
CREATE INDEX IF NOT EXISTS idx_special_team_entries_team
            ON special_team_entries(team_id);

-- ══ Equipes: finanças, carro, gestão ═════════════════════════════════════════

CREATE TABLE IF NOT EXISTS team_finance_history (
            id                          INTEGER PRIMARY KEY AUTOINCREMENT,
            team_id                     TEXT NOT NULL,
            season_number               INTEGER NOT NULL,
            round                       INTEGER NOT NULL,
            category                    TEXT NOT NULL DEFAULT '',
            sponsorship_income          REAL NOT NULL DEFAULT 0.0,
            result_bonus                REAL NOT NULL DEFAULT 0.0,
            partial_prize_income        REAL NOT NULL DEFAULT 0.0,
            constructor_prize_income    REAL NOT NULL DEFAULT 0.0,
            gate_income                 REAL NOT NULL DEFAULT 0.0,
            aid_income                  REAL NOT NULL DEFAULT 0.0,
            salary_expense              REAL NOT NULL DEFAULT 0.0,
            event_operations_cost       REAL NOT NULL DEFAULT 0.0,
            structural_maintenance_cost REAL NOT NULL DEFAULT 0.0,
            technical_investment_cost   REAL NOT NULL DEFAULT 0.0,
            debt_service_cost           REAL NOT NULL DEFAULT 0.0,
            income_total                REAL NOT NULL DEFAULT 0.0,
            expenses_total              REAL NOT NULL DEFAULT 0.0,
            net                         REAL NOT NULL DEFAULT 0.0,
            cash_balance                REAL NOT NULL DEFAULT 0.0,
            debt_balance                REAL NOT NULL DEFAULT 0.0,
            created_at                  TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(team_id, season_number, round),
            FOREIGN KEY (team_id) REFERENCES teams(id)
        );

CREATE INDEX IF NOT EXISTS idx_team_finance_history_team_season
            ON team_finance_history(team_id, season_number, round);

CREATE TABLE IF NOT EXISTS team_car (
            team_id    TEXT NOT NULL,
            part_type  TEXT NOT NULL,
            level      INTEGER NOT NULL DEFAULT 1,
            wear       REAL NOT NULL DEFAULT 0.0,
            spent      INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (team_id, part_type)
        );

CREATE TABLE IF NOT EXISTS team_focus (
            team_id     TEXT PRIMARY KEY,
            foco        TEXT NOT NULL DEFAULT 'meio_de_grid',
            temporadas  INTEGER NOT NULL DEFAULT 0
        );

CREATE TABLE IF NOT EXISTS team_strategic_plan (
            team_id         TEXT PRIMARY KEY,
            plan_type       TEXT NOT NULL DEFAULT 'sustainable',
            remaining_years INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (team_id) REFERENCES teams(id)
        );

CREATE TABLE IF NOT EXISTS team_collapse_streak (
            team_id TEXT PRIMARY KEY,
            streak  INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (team_id) REFERENCES teams(id)
        );

CREATE TABLE IF NOT EXISTS team_rescue_counters (
            key   TEXT PRIMARY KEY,
            value INTEGER NOT NULL DEFAULT 0
        );

CREATE TABLE IF NOT EXISTS team_promotion_history (
            team_id               TEXT PRIMARY KEY,
            last_promotion_season INTEGER NOT NULL DEFAULT 0,
            recent_promotions     INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (team_id) REFERENCES teams(id)
        );

CREATE TABLE IF NOT EXISTS team_ownership_events (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            team_id       TEXT NOT NULL,
            season_number INTEGER NOT NULL,
            ano           INTEGER NOT NULL,
            event_type    TEXT NOT NULL DEFAULT 'sale',
            debt_cleared  REAL NOT NULL DEFAULT 0.0,
            cash_injected REAL NOT NULL DEFAULT 0.0,
            detail        TEXT NOT NULL DEFAULT '',
            created_at    TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (team_id) REFERENCES teams(id)
        );

CREATE INDEX IF NOT EXISTS idx_team_ownership_events_team
            ON team_ownership_events(team_id);

-- ══ Arquivos históricos e notícias ═══════════════════════════════════════════

CREATE TABLE IF NOT EXISTS driver_season_archive (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            piloto_id           TEXT    NOT NULL,
            season_number       INTEGER NOT NULL,
            ano                 INTEGER NOT NULL,
            nome                TEXT    NOT NULL,
            categoria           TEXT    NOT NULL DEFAULT '',
            posicao_campeonato  INTEGER,
            pontos              REAL,
            snapshot_json       TEXT    NOT NULL,
            archived_at         TEXT    NOT NULL DEFAULT (datetime('now')),
            UNIQUE(piloto_id, season_number)
        );

CREATE INDEX IF NOT EXISTS idx_driver_season_archive_piloto
            ON driver_season_archive(piloto_id);
CREATE INDEX IF NOT EXISTS idx_driver_season_archive_season
            ON driver_season_archive(season_number, categoria);

CREATE TABLE IF NOT EXISTS team_season_archive (
            team_id TEXT NOT NULL,
            season_number INTEGER NOT NULL,
            ano INTEGER NOT NULL,
            categoria TEXT NOT NULL,
            classe TEXT,
            posicao_campeonato INTEGER,
            pontos REAL NOT NULL DEFAULT 0.0,
            vitorias INTEGER NOT NULL DEFAULT 0,
            podios INTEGER NOT NULL DEFAULT 0,
            poles INTEGER NOT NULL DEFAULT 0,
            corridas INTEGER NOT NULL DEFAULT 0,
            titulos_construtores INTEGER NOT NULL DEFAULT 0,
            piloto_1_id TEXT,
            piloto_2_id TEXT,
            snapshot_json TEXT NOT NULL DEFAULT '{}',
            archived_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (team_id, season_number, categoria)
        );

CREATE INDEX IF NOT EXISTS idx_team_season_archive_team
            ON team_season_archive(team_id);
CREATE INDEX IF NOT EXISTS idx_team_season_archive_season
            ON team_season_archive(season_number, categoria);

CREATE TABLE IF NOT EXISTS retired (
    piloto_id                TEXT PRIMARY KEY,
    nome                     TEXT NOT NULL,
    temporada_aposentadoria  TEXT NOT NULL,
    categoria_final          TEXT NOT NULL,
    estatisticas             TEXT NOT NULL DEFAULT '{}',
    motivo                   TEXT NOT NULL DEFAULT 'Aposentadoria'
);

CREATE TABLE IF NOT EXISTS history_seasons (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    temporada_id        TEXT NOT NULL,
    ano                 INTEGER NOT NULL,
    categoria           TEXT NOT NULL,
    campeao_piloto_id   TEXT NOT NULL,
    campeao_equipe_id   TEXT NOT NULL,
    classificacao_final TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS history_general (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS news (
    id           TEXT PRIMARY KEY,
    tipo         TEXT NOT NULL,
    titulo       TEXT NOT NULL,
    texto        TEXT NOT NULL,
    chave_dedup  TEXT UNIQUE,
    temporada_id TEXT NOT NULL,
    rodada       INTEGER NOT NULL DEFAULT 0,
    criado_em    TEXT NOT NULL DEFAULT '',
    lida         INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_news_temporada ON news(temporada_id);
CREATE INDEX IF NOT EXISTS idx_news_lida      ON news(lida);

";
