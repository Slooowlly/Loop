//! Agregação do histórico volta a volta: race trace, voltas e setores do
//! jogador, paradas de box/clima e o diagnóstico por carro para a UI.

use super::*;

impl RaceMonitor {
    /// Monta o diagnóstico por carro (o que o RaceControl enxerga de cada um).
    pub(super) fn build_cars_debug(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;
        let flags = t.session_flags as u32;
        self.live_is_green =
            t.session_state == STATE_RACING && flags & (FLAG_CAUTION | FLAG_CAUTION_WAVING) == 0;
        let ref_pace = self
            .car_monitors
            .iter()
            .map(|cm| cm.pace)
            .fold(0.0_f64, f64::max);

        let mut out = Vec::with_capacity(t.cars.len());
        for car in &t.cars {
            let valid = car.idx >= 0 && (car.idx as usize) < 64;
            let cm = if valid {
                self.car_monitors[car.idx as usize]
            } else {
                CarMonitor::DEFAULT
            };
            let stalled = if cm.has_moved {
                now - cm.last_move_time
            } else {
                0.0
            };
            let pace_pct = if ref_pace > 0.0 {
                (cm.pace / ref_pace * 100.0).clamp(0.0, 999.0)
            } else {
                0.0
            };
            let on_racing =
                car.track_surface == SURFACE_ON_TRACK || car.track_surface == SURFACE_OFF_TRACK;
            let monitorable = !car.is_player && self.is_monitorable_ai(car.idx);
            // "Em apuros" = PARADO na pista (mesmo critério dos gatilhos de
            // bandeira). Lento-vs-líder é tráfego, não conta.
            let in_trouble = monitorable
                && !car.on_pit_road
                && cm.has_moved
                && on_racing
                && stalled > YELLOW_MIN_STOP_SECS;
            out.push(CarDebug {
                idx: car.idx,
                is_player: car.is_player,
                is_ai: valid && self.car_is_ai[car.idx as usize],
                is_pace: valid && self.car_is_pace[car.idx as usize],
                position: car.position,
                lap_dist_pct: car.lap_dist_pct,
                sector: (car.lap_dist_pct * NUM_SECTORS as f64).floor() as i32,
                track_surface: surface_label(car.track_surface),
                on_pit_road: car.on_pit_road,
                has_moved: cm.has_moved,
                stalled_secs: stalled,
                pace_pct_of_leader: pace_pct,
                in_trouble,
            });
        }
        self.cars_debug = out;
    }

    // ── Loop principal ───────────────────────────────────────────────────────
    /// Grava o histórico volta a volta da tentativa ativa: a cada volta do líder
    /// um snapshot de gaps/posições, o tempo de cada volta do jogador e as voltas
    /// em que a amarela esteve ativa. Reinicia quando começa uma nova tentativa.
    /// Captura, a cada tick ATIVO, o contexto de clima da corrida e as PARADAS de box
    /// de todos os carros (entrada/saída do `InPitStall` + dwell + pista molhada no
    /// instante). Alimenta a inferência de estratégia de pneu (`tire_strategy`).
    pub(super) fn capture_tire_strategy(&mut self, t: &IracingTelemetry, now: f64) {
        let wet = t.track_is_wet();

        // ── Contexto de clima ──
        // Largada: capturada uma vez quando a corrida já está em "Racing".
        if !self.weather_start_captured && t.session_state == STATE_RACING {
            self.history.weather.wet_at_start = wet;
            self.weather_start_captured = true;
        }
        if wet {
            self.history.weather.ever_wet = true;
        }
        // wet_at_finish = wetness do último tick ativo (record_history só roda enquanto
        // a tentativa está ativa; o último valor escrito ≈ a condição na bandeirada).
        if t.session_state == STATE_RACING {
            self.history.weather.wet_at_finish = wet;
        }

        // ── Paradas de box (todos os carros, menos o pace car) ──
        // Só rastreia durante a corrida VERDE: a espera no box antes da largada
        // (grid) ou numa classificatória não pode virar uma parada. Fora do verde,
        // zera o rastreio pra uma caixa pré-largada não abrir uma "entrada" que
        // fecharia (com dwell enorme) no instante da largada.
        if t.session_state < STATE_RACING {
            self.pit_in_stall = [false; 64];
            self.pit_left_stall = [false; 64];
            return;
        }
        for car in &t.cars {
            let i = car.idx as usize;
            if i >= 64 || self.car_is_pace[i] {
                continue;
            }
            let in_stall = car.track_surface == SURFACE_IN_PIT_STALL;
            if !in_stall {
                // Visto fora da caixa: a partir daqui as entradas dele são paradas de verdade.
                self.pit_left_stall[i] = true;
            }
            if in_stall && !self.pit_in_stall[i] {
                // Entrou na caixa → abre o cronômetro de dwell.
                self.pit_in_stall[i] = true;
                self.pit_stall_enter_time[i] = now;
                self.pit_stall_enter_lap[i] = car.lap.max(car.lap_completed + 1).max(1);
                self.pit_stall_wet[i] = wet;
                // A CAIXA DE PARTIDA não é uma parada. Todo carro começa a sessão dentro
                // dela (é a "estampa" inicial do `InPitStall`), e sem esta trava a saída
                // pra pista fechava uma parada com o dwell inteiro da espera — na quali,
                // onde a sessão nasce com todo mundo no box, a coluna de pneu da torre
                // vinha cheia de paradas que nunca aconteceram.
                self.pit_stall_valid[i] = self.pit_left_stall[i];
            } else if !in_stall && self.pit_in_stall[i] {
                // Saiu da caixa → fecha a parada.
                self.pit_in_stall[i] = false;
                if !self.pit_stall_valid[i] {
                    continue;
                }
                let dwell = (now - self.pit_stall_enter_time[i]).max(0.0);
                // Ignora blips transientes de `InPitStall` (dwell ~0s): o carro
                // apenas cruzou a zona da caixa, não parou. Só conta parada real.
                if dwell < MIN_PIT_STALL_DWELL_SECS {
                    continue;
                }
                self.history
                    .pit_stops
                    .push(crate::iracing_sdk::tire_strategy::PitStop {
                        car_idx: car.idx,
                        lap: self.pit_stall_enter_lap[i],
                        stationary_secs: dwell,
                        track_wet_at_stop: self.pit_stall_wet[i],
                    });
                if self.history.pit_stops.len() > MAX_PIT_STOPS {
                    self.history.pit_stops.remove(0);
                }
            }
        }
    }

    /// Cronometra os 3 setores do jogador dividindo `lap_dist_pct` em terços. Só na
    /// corrida verde e com o carro na pista; sair da pista ou pular setor (teleporte/
    /// reset) invalida o parcial em andamento pra não gravar tempo-lixo.
    pub(super) fn capture_player_sectors(&mut self, t: &IracingTelemetry) {
        if t.session_state < STATE_RACING || t.track_surface != SURFACE_ON_TRACK {
            self.sec_prev = -1;
            return;
        }
        let pct = t.lap_dist_pct;
        if !(0.0..=1.0).contains(&pct) {
            return;
        }
        let sec = if pct < 1.0 / 3.0 {
            0
        } else if pct < 2.0 / 3.0 {
            1
        } else {
            2
        };
        let now = t.session_time;
        if self.sec_prev < 0 {
            // Primeira base: começa a cronometrar, mas ESTE setor é parcial (entramos
            // no meio dele) → não grava ao fechá-lo.
            self.sec_prev = sec;
            self.sec_enter_time = now;
            self.sec_clean = false;
            return;
        }
        if sec == self.sec_prev {
            return;
        }
        let expected = (self.sec_prev + 1) % 3;
        if sec == expected {
            if self.sec_clean {
                let dur = now - self.sec_enter_time;
                if dur > 0.0 && dur < 600.0 {
                    // O S3 fecha na linha (2→0) → pertence à volta recém-completada.
                    let lap = if self.sec_prev == 2 {
                        (t.lap - 1).max(1)
                    } else {
                        t.lap.max(1)
                    };
                    self.history.player_sectors.push(SectorSplit {
                        lap,
                        sector: self.sec_prev + 1,
                        time: dur,
                    });
                    if self.history.player_sectors.len() > MAX_HISTORY_LAPS {
                        self.history.player_sectors.remove(0);
                    }
                }
            }
            self.sec_prev = sec;
            self.sec_enter_time = now;
            self.sec_clean = true;
        } else {
            // Pulou setor (teleporte/reset/volta anulada) → invalida o parcial.
            self.sec_prev = sec;
            self.sec_enter_time = now;
            self.sec_clean = false;
        }
    }

    pub(super) fn record_history(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;

        // A CLASSIFICATÓRIA nunca entra no histórico da CORRIDA. Ela roda na mesma
        // conexão de telemetria (com outro `session_num`) e, sem este gate, suas
        // voltas + o retorno ao box no fim viravam voltas/paradas "da corrida" no
        // pós-corrida. As voltas de quali que importam (grid) já são capturadas à
        // parte em `capture_qualy` → `qualy_laps`.
        if self.qualy_session_num >= 0 && t.session_num == self.qualy_session_num {
            // Sair sem gravar não basta: o histórico da sessão ANTERIOR (o treino livre,
            // onde todo mundo sai do box) continuaria vivo por baixo, e a torre leria
            // aquelas paradas como estratégia de pneu da quali. Entrar na quali zera.
            if self.hist_session_num != t.session_num {
                self.history = RaceHistory::empty();
                self.hist_session_num = t.session_num;
                self.pit_in_stall = [false; 64];
                self.pit_left_stall = [false; 64];
            }
            return;
        }

        // Marca o histórico como encerrado quando a tentativa que ele cobre fecha
        // (checkered/DNF/etc) — sinaliza ao painel que pode salvar/auto-salvar.
        let closed = self
            .attempts
            .last()
            .filter(|a| a.number == self.history.attempt_number && a.status != "active")
            .map(|a| status_pt(&a.status));
        if let Some(label) = closed {
            self.history.finished = true;
            self.history.outcome = label.to_string();
        }

        // Só grava com uma tentativa ATIVA (corrida em andamento ao vivo). Após o
        // fim (finished/dnf) paramos, mas mantemos o que já foi capturado.
        let attempt = match self.attempts.last() {
            Some(a) if a.status == "active" => a.number,
            _ => return,
        };
        // Tentativa nova OU sessão de telemetria nova (quali/treino → corrida) →
        // começa um histórico limpo. O gate por sessão é a rede de segurança caso
        // a quali não tenha sido identificada a tempo pelo YAML.
        let subsession_changed = self.session_subsession_id > 0
            && self.history.subsession_id != self.session_subsession_id;
        if self.history.attempt_number != attempt
            || self.hist_session_num != t.session_num
            || subsession_changed
        {
            self.history = RaceHistory::empty();
            self.history.attempt_number = attempt;
            self.history.subsession_id = self.session_subsession_id;
            self.hist_session_num = t.session_num;
            self.hist_leader_lap = 0;
            self.hist_player_lap = 0;
            self.hist_car_lap_completed = [0; 64];
            self.hist_car_last_pos = [0; 64];
            self.hist_trace_pos = [0; 64];
            self.hist_last_trace_event_time = 0.0;
            self.hist_last_neighbor_time = 0.0;
            // A quali precede a corrida → carrega as voltas dela no histórico (uma vez).
            self.history.qualy_laps = self.qualy_laps.clone();
            // Reseta o detector de pit / clima / setor para a tentativa nova.
            self.pit_in_stall = [false; 64];
            self.pit_left_stall = [false; 64];
            self.weather_start_captured = false;
            self.sec_prev = -1;
            self.sec_enter_time = 0.0;
            self.sec_clean = false;
        }
        self.history.player_car_idx = t.player_car_idx;

        // Clima da corrida + paradas de box (estratégia de pneu de todos os carros).
        self.capture_tire_strategy(t, now);
        self.capture_player_sectors(t);

        // Volta do líder = voltas do carro em P1 (ou o maior valor, na largada
        // quando as posições ainda não assentaram).
        let leader_lap = t
            .cars
            .iter()
            .filter(|c| c.position == 1)
            .map(|c| c.lap_completed)
            .max()
            .or_else(|| t.cars.iter().map(|c| c.lap_completed).max())
            .unwrap_or(0);

        // Amarela ativa nesta volta do líder → pinta a volta (faixa amarela).
        let caution = (t.session_flags as u32) & (FLAG_CAUTION | FLAG_CAUTION_WAVING) != 0;
        if caution && leader_lap >= 1 && !self.history.yellow_laps.contains(&leader_lap) {
            self.history.yellow_laps.push(leader_lap);
        }

        // Guarda a última posição VÁLIDA de cada carro a cada tick. `CarIdxPosition`
        // pisca 0 quando o carro está no box/entre estados; sem isso, um único tick
        // ruim no instante do snapshot faria o carro sumir daquela volta do trace.
        for c in &t.cars {
            let i = c.idx;
            if i >= 0 && (i as usize) < 64 && c.position >= 1 {
                self.hist_car_last_pos[i as usize] = c.position;
            }
        }

        // Snapshot do trace: na VIRADA de volta do líder (âncora, garante 1 ponto por
        // volta) OU quando QUALQUER posição muda — aí a ultrapassagem entra no gráfico
        // NA HORA, não só quando o líder fecha a volta. O X vira fracionário
        // (volta do líder + progresso dele na volta).
        let new_leader_lap = leader_lap > self.hist_leader_lap && leader_lap >= 1;
        let positions_changed = t.session_state >= STATE_RACING
            && leader_lap >= 1
            && now - self.hist_last_trace_event_time >= MIN_TRACE_EVENT_GAP_SECS
            && t.cars.iter().any(|c| {
                let i = c.idx;
                i >= 0
                    && (i as usize) < 64
                    && c.position >= 1
                    && c.position != self.hist_trace_pos[i as usize]
            });
        if new_leader_lap || positions_changed {
            if new_leader_lap {
                self.hist_leader_lap = leader_lap;
            }
            if positions_changed {
                self.hist_last_trace_event_time = now;
            }
            // Progresso do líder DENTRO da volta (0..1) → parte fracionária do X.
            let leader_progress = t
                .cars
                .iter()
                .find(|c| c.position == 1)
                .map(|c| c.lap_dist_pct.clamp(0.0, 1.0))
                .unwrap_or(0.0);
            // Tempo de volta de referência do líder — usado pra "empilhar" as voltas
            // atrás no gap de quem está lapado/parado. Sem isso, um carro parado no box
            // (F2Time ~0, ver CarSnapshot::f2_time) fica colado no líder no gráfico.
            // Preferência: volta do líder → melhor volta do líder → volta mais rápida
            // do campo → fallback 90s (só relevante nos primeiros instantes).
            let leader_lap_ref = t
                .cars
                .iter()
                .find(|c| c.position == 1)
                .map(|c| {
                    if c.last_lap_time > 0.0 {
                        c.last_lap_time
                    } else {
                        c.best_lap_time
                    }
                })
                .filter(|&v| v > 0.0)
                .or_else(|| {
                    t.cars
                        .iter()
                        .map(|c| c.last_lap_time)
                        .filter(|&v| v > 0.0)
                        .fold(None, |acc: Option<f64>, v| {
                            Some(acc.map_or(v, |a| a.min(v)))
                        })
                })
                .unwrap_or(90.0);
            // Posição do líder DENTRO da volta em segundos (`CarIdxEstTime`), base do
            // gap contínuo abaixo.
            let leader_est = t
                .cars
                .iter()
                .find(|c| c.position == 1)
                .map(|c| c.est_time)
                .unwrap_or(0.0);
            let cars: Vec<CarGapPoint> = t
                .cars
                .iter()
                .filter_map(|c| {
                    let i = c.idx;
                    if i < 0 || i as usize >= 64 {
                        return None;
                    }
                    // Posição atual (≥1), ou a última válida conhecida (carro num
                    // blip de box). Sem nenhuma → pace car / nunca classificado.
                    let position = if c.position >= 1 {
                        c.position
                    } else {
                        self.hist_car_last_pos[i as usize]
                    };
                    if position < 1 {
                        return None;
                    }
                    // Gap ao líder CONTÍNUO. O `CarIdxF2Time` do SDK só é reescrito
                    // quando o carro CRUZA a linha: usado direto, a linha do trace fica
                    // congelada por uma volta inteira e depois desce em DEGRAU — um
                    // artefato de amostragem que não aconteceu na corrida. Aqui o gap sai
                    // do "tempo de pista": `voltas × tempo de volta de referência +
                    // est_time` (segundos desde a linha na volta atual, populado em
                    // qualquer sessão, inclusive de IA) — varia a cada tick e já embute
                    // as voltas de quem está lapado ou parado no box.
                    let car_progress = if c.lap_dist_pct.is_finite() {
                        c.lap_dist_pct.clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    // Parte intra-volta: est_time quando vier; senão, progresso × volta
                    // de referência (backstop pra sessão sem EstTime).
                    let intra = |est: f64, pct: f64| {
                        if est.is_finite() && est > 0.0 {
                            est
                        } else {
                            pct * leader_lap_ref
                        }
                    };
                    let leader_secs =
                        leader_lap as f64 * leader_lap_ref + intra(leader_est, leader_progress);
                    let car_secs =
                        c.lap_completed as f64 * leader_lap_ref + intra(c.est_time, car_progress);
                    let dist_gap = (leader_secs - car_secs).max(0.0);
                    // F2Time só como fallback: sem distância utilizável (carro colado na
                    // linha, sem EstTime), ele ainda é melhor que zerar o gap.
                    let gap = if dist_gap > 0.0 {
                        dist_gap
                    } else if c.f2_time.is_finite() {
                        c.f2_time.max(0.0)
                    } else {
                        0.0
                    };
                    Some(CarGapPoint {
                        idx: c.idx,
                        position,
                        gap,
                        lap_dist_pct: if c.lap_dist_pct.is_finite() {
                            c.lap_dist_pct.clamp(0.0, 1.0) as f32
                        } else {
                            0.0
                        },
                        est_time: if c.est_time.is_finite() {
                            c.est_time.max(0.0) as f32
                        } else {
                            0.0
                        },
                    })
                })
                .collect();
            // Memoriza as posições gravadas pra detectar a PRÓXIMA troca.
            for c in &t.cars {
                let i = c.idx;
                if i >= 0 && (i as usize) < 64 && c.position >= 1 {
                    self.hist_trace_pos[i as usize] = c.position;
                }
            }
            self.history.laps.push(LapSnapshot {
                lap: leader_lap,
                progress: leader_progress as f32,
                cars,
            });
            if self.history.laps.len() > MAX_HISTORY_LAPS {
                self.history.laps.remove(0);
            }
        }

        // Pit do jogador: acumula se passou pelo pit road na volta em andamento.
        if t.player_on_pit_road {
            self.player_pit_seen = true;
        }

        // Jogador completou uma volta nova → registra o tempo dela.
        if t.lap_completed > self.hist_player_lap {
            self.hist_player_lap = t.lap_completed;
            if t.last_lap_time > 0.0 {
                self.history.player_laps.push(PlayerLap {
                    lap: t.lap_completed,
                    time: t.last_lap_time,
                    // Combustível restante ao fechar a volta (litros); consumo/volta
                    // sai da diferença entre voltas. <0 no SDK = ignora.
                    fuel_remaining: if t.fuel_level >= 0.0 {
                        t.fuel_level
                    } else {
                        -1.0
                    },
                });
                if self.history.player_laps.len() > MAX_HISTORY_LAPS {
                    self.history.player_laps.remove(0);
                }
            }
            // A volta recém-completada teve pit? Marca e zera para a próxima.
            if self.player_pit_seen {
                self.player_pit_laps.push(t.lap_completed);
                if self.player_pit_laps.len() > MAX_HISTORY_LAPS {
                    self.player_pit_laps.remove(0);
                }
            }
            self.player_pit_seen = false;
        }

        // Cada carro (jogador + IA) completou uma volta → registra o tempo dela.
        // Base da adaptação (ritmo da frente da classe). Evento por carro, barato.
        for car in &t.cars {
            let i = car.idx;
            if i < 0 || i as usize >= 64 || self.car_is_pace[i as usize] {
                continue;
            }
            if car.lap_completed > self.hist_car_lap_completed[i as usize] {
                self.hist_car_lap_completed[i as usize] = car.lap_completed;
                if car.last_lap_time > 0.0 && car.lap_completed >= 1 {
                    self.history.car_laps.push(CarLap {
                        car_idx: i,
                        lap: car.lap_completed,
                        time: car.last_lap_time,
                    });
                    if self.history.car_laps.len() > MAX_CAR_LAPS {
                        self.history.car_laps.remove(0);
                    }
                }
            }
        }

        // Resumo por carro (classe, IA, posição na classe) — ACUMULADO por idx
        // (upsert), nunca encolhe. Um carro que sai do mundo (DNF, ou o cooldown
        // pós-bandeira em que todos voltam ao menu) MANTÉM a última amostra; assim
        // as posições finais não são apagadas no fim da corrida. (Antes era um
        // replace total, que zerava cars_meta quando o campo esvaziava no cooldown.)
        let in_race = self.in_race_session(t);
        for c in t
            .cars
            .iter()
            .filter(|c| c.idx >= 0 && (c.idx as usize) < 64)
        {
            let i = c.idx as usize;
            // Grid ROBUSTO: além do snapshot exato do verde, fixa a PRIMEIRA posição
            // na classe já observada (set-once). Se o monitor só começou a amostrar
            // depois da largada (transição do verde perdida), o grid ainda é a
            // posição mais antiga vista — bem melhor que vazio.
            //
            // Só na CORRIDA. `record_history` barra a quali, mas roda no treino livre, e
            // ali este set-once congelava o "grid" na ordem em que os carros apareceram
            // no treino — que depois não era corrigida, porque o snapshot do verde tem
            // gate de uma vez só por tentativa.
            if in_race && self.grid_class_pos[i] == 0 && c.class_position >= 1 {
                self.grid_class_pos[i] = c.class_position;
            }
            let meta = CarMeta {
                idx: c.idx,
                is_ai: self.car_is_ai[i],
                is_pace: self.car_is_pace[i],
                class_id: self.car_class_id[i],
                class_position: c.class_position,
                car_number: self.car_number[i],
                grid_class_position: self.grid_class_pos[i],
            };
            match self.history.cars_meta.iter_mut().find(|m| m.idx == c.idx) {
                // Preserva o grid já capturado se este tick ainda não o tem.
                Some(existing) => {
                    let grid = if meta.grid_class_position > 0 {
                        meta.grid_class_position
                    } else {
                        existing.grid_class_position
                    };
                    *existing = CarMeta {
                        grid_class_position: grid,
                        ..meta
                    };
                }
                None => self.history.cars_meta.push(meta),
            }
        }
        self.history.track_id = self.session_track_id;

        // Batalha do jogador (carro à frente/atrás) — amostra leve a ~1Hz,
        // capturando a briga se desenvolvendo ao longo da corrida.
        if now - self.hist_last_neighbor_time >= NEIGHBOR_SAMPLE_SECS {
            if let Some(me) = t.cars.iter().find(|c| c.idx == t.player_car_idx) {
                if me.position >= 1 {
                    self.hist_last_neighbor_time = now;
                    // O `position >= 1` no candidato NÃO é redundante com o do jogador:
                    // com o jogador em P1, `me.position - 1` vale 0, e 0 é o sentinela do
                    // iRacing para "sem posição atribuída" — carro na garagem, fora do
                    // mundo, ou a sessão inteira num quali. Sem a guarda, o líder ganha um
                    // vizinho FABRICADO: o primeiro carro sentinelado do vetor, com o gap
                    // saindo do `est_time` dele. Medido em captura real
                    // (race_1785889561.jsonl.gz): 29% das amostras com vizinho apontavam
                    // para um carro de position 0. O de trás não precisa da guarda —
                    // `me.position + 1` nunca é 0 com `me.position >= 1`.
                    let ahead = t
                        .cars
                        .iter()
                        .find(|c| c.position >= 1 && c.position == me.position - 1);
                    let behind = t.cars.iter().find(|c| c.position == me.position + 1);
                    // O gap sai do `est_time` — NÃO do `f2_time`, que é o gap ao LÍDER e
                    // só é reescrito na passagem pela linha. Medido numa captura real de
                    // corrida de IA: com o carro da frente a 40 s de pista, a diferença
                    // dos `f2_time` dizia 0,165 s (os dois estavam a ~98 s do líder), e
                    // divergia do gap real em mais de um segundo em 60% das amostras. Um
                    // "briga de 0,1 s" fabricado assim vira card de rival e série do
                    // gráfico. Mesma conta circular do `EstadoAgora`, e pelo mesmo motivo:
                    // quem está à frente pode já ter cruzado a linha.
                    let volta_ref = self.volta_referencia(t, me);
                    self.history.player_track.push(PlayerTrackPoint {
                        session_time: now,
                        lap: me.lap_completed,
                        position: me.position,
                        speed_kmh: t.speed_kmh,
                        ahead_idx: ahead.map(|c| c.idx).unwrap_or(-1),
                        gap_ahead: ahead
                            .map(|c| gap_circular(me.est_time, c.est_time, volta_ref))
                            .unwrap_or(-1.0),
                        behind_idx: behind.map(|c| c.idx).unwrap_or(-1),
                        gap_behind: behind
                            .map(|c| gap_circular(c.est_time, me.est_time, volta_ref))
                            .unwrap_or(-1.0),
                    });
                    if self.history.player_track.len() > MAX_TRACK_POINTS {
                        self.history.player_track.remove(0);
                    }
                }
            }
        }
    }
}
