//! Superfície pública do monitor: as consultas que o frontend faz por invoke e
//! os gatilhos de debug/produção do sistema de quebra.

use super::*;

/// Se o iRacing está conectado agora (último tick do sampler).
pub fn is_connected() -> bool {
    start_sampler();
    lock().connected
}

/// DEBUG: arma uma quebra garantida no carro do jogador pra próxima volta cruzada (testa o
/// disparo ao vivo na pista). `true` = armado (jogador em sessão + número conhecido).
pub fn arm_test_breakdown() -> bool {
    start_sampler();
    lock().arm_test_breakdown()
}

/// DEBUG: pede armar a GRADE TODA com uma peça perto de quebrar por carro (montada no próximo
/// tick, correndo). As quebras pingam ao longo das voltas, estranguladas pra não spammar.
pub fn arm_test_breakdown_grid() {
    start_sampler();
    lock().request_arm_grid();
}

/// PRODUÇÃO: instala o diretor de quebra da corrida montado com o DESGASTE REAL de cada time
/// (chamado pelo export, que tem DB). `player_live` = estado do jogador (vinculado ao número
/// dele no verde). `weather` = clima fixo da corrida. Prende os carros na volta atual no
/// primeiro tick verde e passa a disparar `!black`/`!dq` conforme cada carro cruza.
pub fn install_breakdown_director(
    dir: crate::car::breakdown::BreakdownDirector,
    player_live: Option<crate::car::breakdown::LiveBreakdown>,
    weather: crate::car::breakdown::Weather,
    showcase: bool,
) {
    start_sampler();
    lock().install_breakdown_director(dir, player_live, weather, showcase);
}

/// Alertas de quebra ativos por car_idx, pro overlay: `(car_idx, kind)` com
/// kind ∈ "light" | "heavy" | "dnf". Vazio quando não há quebra em andamento.
pub fn get_breakdown_alerts() -> Vec<(i32, &'static str)> {
    lock().breakdown_alerts_snapshot()
}

/// Paradas de REPARO de peça: `(car_idx, volta de entrada no box)`. O overlay marca o ícone
/// de "peça" no lugar do pneu na parada daquela volta.
pub fn get_breakdown_repair_laps() -> Vec<(i32, u32)> {
    lock().breakdown_repair_laps.clone()
}

/// Espia (sem drenar) o log de quebras da corrida em andamento — pro overlay do RÁDIO DA
/// EQUIPE mostrar cada quebra ao vivo. O drain de verdade (→ tabela/debrief) só acontece no
/// import; aqui é leitura pura, acumulativa durante a corrida.
pub fn peek_breakdown_log() -> Vec<BreakdownOutcome> {
    lock().breakdown_log.clone()
}

/// O número do carro do JOGADOR nesta sessão, se ele estiver identificado.
///
/// Existe para o rádio da grade poder EXCLUIR a linha dele do feed: a quebra do carro do jogador
/// entra no mesmo `breakdown_log` que a dos outros (é ela que manda o `!black`/`!dq`), e falar
/// dela ali sairia em 3ª pessoa — o jogador ouvindo falarem de si como de um estranho. O desfecho
/// dele sai pelo canal de avisos, em 2ª pessoa.
pub fn player_car_number() -> Option<u32> {
    lock().player_car_number()
}

/// Espia (sem drenar) os AVISOS pessoais do jogador (peça entrou na zona de risco) — pro
/// overlay do rádio mostrar num card DISTINTO. Leitura pura, acumulativa durante a corrida.
pub fn peek_player_warnings() -> Vec<PlayerWarning> {
    lock().player_warning_log.clone()
}

/// Espia (sem drenar) o log do RÁDIO DE RITMO — a volta mais rápida da corrida e a
/// aproximação dela. Leitura pura, acumulativa durante a corrida, como o log de quebras.
pub fn peek_ritmo_log() -> Vec<FalaDeRitmo> {
    lock().ritmo_log.clone()
}

/// Espia (sem drenar) o log do engenheiro na CLASSIFICAÇÃO — a despedida antes da volta lançada
/// e o comentário da volta que morreu. Canal SEPARADO dos outros pelo mesmo motivo de sempre:
/// os logs crescem em ritmos próprios e um id só embaralharia os cursores do overlay.
pub fn peek_classificacao_log() -> Vec<crate::engenheiro::classificacao::Fala> {
    lock().classificacao_log.clone()
}

/// `true` se algum comando de quebra falhou em chegar ao iRacing nesta corrida (janela não
/// encontrada / foreground recusado / `SendInput` bloqueado). O overlay do rádio transforma
/// isso num aviso âmbar único orientando o jogador a rodar o sim em janela/borderless.
pub fn chat_send_blocked() -> bool {
    lock().chat_send_warned
}

/// car_idx que devem PISCAR na torre agora (quebraram nos últimos 5 s) — sincroniza o flash
/// da linha do piloto com o anúncio do rádio do engenheiro.
pub fn get_breakdown_flashes() -> Vec<i32> {
    lock().breakdown_flash_idxs()
}

/// PEÇA 3: DRENA o log estruturado de desfechos de quebra da corrida (esvazia). Chamado UMA vez
/// no import (`build_session_race_result`) → resolve car_number→driver_id e persiste na
/// `race_breakdowns`. `std::mem::take` garante que cada desfecho seja importado uma só vez.
pub fn drain_breakdown_log() -> Vec<BreakdownOutcome> {
    start_sampler();
    std::mem::take(&mut lock().breakdown_log)
}

/// Lê o snapshot atual do monitor (alimentado a ~60 Hz pelo sampler).
pub fn poll() -> RaceStatus {
    start_sampler();
    let mut m = lock();
    let event = m.pending_event.take();
    RaceStatus {
        connected: m.connected,
        attempt_number: m.current_attempt,
        event,
        session_state_label: state_label(m.live_state),
        track_surface_label: surface_label(m.live_surface),
        lap_completed: m.live_lap,
        incident_count: m.live_incident,
        crash_score: m.live_score,
        crash_severity_now: severity_label(m.live_score).to_string(),
        g_force: m.live_g,
        speed_kmh: m.live_speed_kmh,
        tow_time: m.live_tow,
        cars_count: m.live_cars_count,
        crash_in_progress: m.in_crash,
        crash_progress_score: if m.in_crash {
            m.crash_components.total()
        } else {
            0.0
        },
        crash_progress_severity: if m.in_crash {
            severity_label(m.crash_components.total()).to_string()
        } else {
            "nenhum".to_string()
        },
        is_green: m.live_is_green,
        cars_debug: m.cars_debug.clone(),
        attempts: m.attempts.clone(),
        events: m.events.clone(),
    }
}

/// O retrato da corrida NESTE instante, com as contas já fechadas — a fonte do
/// engenheiro do push-to-talk e de qualquer fala que precise responder "e agora?".
///
/// Diferente do [`poll`], que é diagnóstico de batida, e do [`get_history`], que é a
/// corrida inteira: aqui é só o presente, pequeno o bastante para caber num prompt.
pub fn estado_agora() -> EstadoAgora {
    start_sampler();
    lock().montar_estado_agora()
}

/// A ordem da corrida agora: `(posição geral, número do carro)`. Ver
/// [`Monitor::ordem_agora`](super::MonitorState::ordem_agora).
pub fn ordem_agora() -> Vec<(i32, i32)> {
    start_sampler();
    lock().ordem_agora()
}

/// Lê o histórico volta a volta acumulado (race trace + ritmo) para o painel
/// pós-corrida. Alimentado pelo mesmo sampler de ~60 Hz.
pub fn get_history() -> RaceHistory {
    start_sampler();
    lock().history.clone()
}

/// Marcadores de incidente do JOGADOR (pontos do próprio iRacing + volta). Moram no
/// monitor, não no `RaceHistory`, daí o acessor próprio — é o ÚNICO sinal de batida de
/// quem TERMINOU a corrida, já que o resultado oficial zera os incidentes.
pub fn get_player_incidents() -> Vec<PlayerIncidentMark> {
    start_sampler();
    lock().player_incidents.clone()
}

/// Lê as voltas de qualify capturadas ao vivo, sem misturá-las ao histórico da corrida.
///
/// São voltas CRUAS (`CarIdxLastLapTime`): uma volta anulada por limite de pista está
/// aqui com o tempo que ela marcou. Para o melhor tempo da classificatória use
/// [`get_qualy_best_valid`].
pub fn get_qualy_laps() -> Vec<CarLap> {
    start_sampler();
    lock().qualy_laps_snapshot()
}

/// Melhor volta VÁLIDA da quali por carro, `(car_idx, segundos)`, travada do
/// `CarIdxBestLapTime` ao vivo — volta anulada fica de fora, e o valor sobrevive ao
/// carro sair do mundo (garagem).
pub fn get_qualy_best_valid() -> Vec<(i32, f64)> {
    start_sampler();
    lock().qualy_best_valid_snapshot()
}

/// Voltas completas do líder no instante da bandeirada. 0 = a corrida ainda não acabou,
/// ou o monitor só entrou em cena depois dela.
///
/// O consumidor é o cabeçalho da torre: `CarIdxLapCompleted` continua subindo no cool
/// down, e sem este congelamento a volta exibida passa do fim da prova.
pub fn get_final_lead_lap() -> i32 {
    start_sampler();
    lock().volta_final_lider
}

/// Quantas falas de rádio já foram descartadas por reinícios anteriores. Os feeds do overlay
/// somam isto ao índice para formar o `id` — sem ela, o log esvaziado devolve ids repetidos e
/// o overlay, que só mostra id NOVO, emudece para o resto da sessão. Ver `radio_epoch`.
pub fn radio_epoch() -> usize {
    lock().radio_epoch
}

/// Número da tentativa que cobriu a CLASSIFICAÇÃO deste fim de semana; 0 se não houve.
///
/// Existe para o import cobrar o conserto da batida da quali com a MESMA régua da corrida:
/// o chamador passa este número a `player_worst_severity`. Sem ele, destruir o carro na
/// classificação saía de graça — o import só olha a tentativa da corrida.
pub fn quali_attempt_number() -> i32 {
    start_sampler();
    lock().quali_attempt_number
}

/// Lê a identidade única do evento atualmente observado pelo monitor.
pub fn get_subsession_id() -> i64 {
    start_sampler();
    lock().session_subsession_id
}

pub fn get_feedback() -> RaceFeedback {
    start_sampler();
    let m = lock();
    // Identidade "ao vivo": todo carro que o YAML da sessão conhece (tem nome de
    // piloto ou número). Independe de tentativa ativa / não-quali.
    let named: std::collections::HashSet<i32> =
        m.driver_names.iter().map(|(idx, _)| *idx).collect();
    let cars_yaml_meta = (0..64i32)
        .filter(|&i| named.contains(&i) || m.car_number[i as usize] > 0)
        .map(|i| YamlCarMeta {
            idx: i,
            is_ai: m.car_is_ai[i as usize],
            is_pace: m.car_is_pace[i as usize],
            class_id: m.car_class_id[i as usize],
            car_number: m.car_number[i as usize],
        })
        .collect();
    RaceFeedback {
        laps: m.history.laps.clone(),
        player_laps: m.history.player_laps.clone(),
        player_track: m.history.player_track.clone(),
        yellow_laps: m.history.yellow_laps.clone(),
        cars_meta: m.history.cars_meta.clone(),
        cars_yaml_meta,
        player_car_idx: m.history.player_car_idx,
        class_names: m.class_names.iter().cloned().collect(),
        driver_names: m.driver_names.iter().cloned().collect(),
        player_pit_laps: m.player_pit_laps.clone(),
        car_laps: m.history.car_laps.clone(),
        player_incidents: m.player_incidents.clone(),
    }
}

/// Zera o monitor para começar um novo teste.
pub fn reset() {
    *lock() = RaceMonitor::new();
}
