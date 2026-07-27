//! Amostrador de fundo (~60 Hz): lê a telemetria e o YAML da sessão, alimenta o
//! monitor e drena a fila de comandos de quebra pro chat do iRacing.

use super::*;

pub(super) fn start_sampler() {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    ensure_auto_yellow_loaded();
    std::thread::spawn(|| {
        let mut tick = 0u64;
        loop {
            // O tick devolve se o iRacing estava CONECTADO — controla a cadência:
            // 60 Hz conectado (não perde picos), 1 Hz ocioso (só espia a conexão).
            let connected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                match crate::iracing_sdk::read_telemetry() {
                    Ok(t) => {
                        // Contador de diagnóstico + marca da TRANSIÇÃO para conectado.
                        // `linha_unica` garante uma linha por transição, não por tick.
                        crate::iracing_sdk::nota_tick_observado();
                        crate::diagnostico::linha_unica(
                            "iracing",
                            "conectado — telemetria fluindo",
                        );
                        // Recarrega a classificação de carros (IA/pace car) do YAML de
                        // tempos em tempos — ela muda raramente, então não precisa ser
                        // a cada tick.
                        if tick % YAML_REFRESH_TICKS == 0 {
                            if let Ok(session) = crate::iracing_sdk::read_session() {
                                let classes = parse_driver_classes(&session.session_yaml);
                                let class_names = parse_class_names(&session.session_yaml);
                                let driver_names = parse_driver_names(&session.session_yaml);
                                let track_id = parse_track_id(&session.session_yaml);
                                let subsession_id = parse_subsession_id(&session.session_yaml);
                                let qualy_num = parse_qualy_session_num(&session.session_yaml);
                                let numbers = parse_car_numbers(&session.session_yaml);
                                let redline = crate::iracing_sdk::parse_car_redline(&session.session_yaml);
                                let car_name = parse_player_car_name(&session.session_yaml);
                                {
                                    let mut m = lock();
                                    m.set_car_classes(&classes);
                                    m.set_class_names(class_names);
                                    m.set_driver_names(driver_names);
                                    m.set_session_track_id(track_id);
                                    m.set_session_subsession_id(subsession_id);
                                    m.set_qualy_session_num(qualy_num);
                                    m.set_car_numbers(&numbers);
                                    m.set_car_redline(redline);
                                    m.set_session_car_name(car_name);
                                }
                                // Captura o custid do jogador automaticamente (uma vez).
                                crate::iracing_sdk::note_session_custid(&session.session_yaml);
                                // DEBUG: se a gravação de corrida está ligada, salva o YAML.
                                crate::iracing_sdk::race_capture::record_session(&session.session_yaml);
                            }
                        }
                        lock().observe(&t);
                        // Gravador SEMPRE LIGADO: abre a captura na borda de conexão (no-op
                        // se já há uma gravando) e despeja o frame. Custa ~24 µs por frame.
                        crate::iracing_sdk::race_capture::ensure_started();
                        crate::iracing_sdk::race_capture::record_frame(&t);
                        // Disparo de quebra ESTRANGULADO: 1 comando a cada ~1,5s (a ~60 Hz),
                        // FORA do lock (o send_chat_text foca a janela + SendInput; não pode
                        // segurar o lock). Espaça o roubo de foco pra o jogador seguir dirigindo.
                        if tick % 90 == 0 {
                            if let Some(cmd) = lock().take_one_breakdown_cmd() {
                                // NÃO engole o erro: se o comando não chegou ao sim (janela
                                // não encontrada / foreground recusado / SendInput bloqueado),
                                // a penalidade sumiria em silêncio. Loga e arma UM aviso âmbar
                                // no rádio (latch por corrida) pra o jogador saber que precisa
                                // rodar o sim em janela/borderless.
                                if let Err(err) = crate::iracing_sdk::send_chat_text(&cmd) {
                                    if lock().note_chat_send_failure() {
                                        eprintln!(
                                            "[breakdown] comando '{cmd}' não chegou ao iRacing: {err}"
                                        );
                                    }
                                }
                            }
                        }
                        // Sim conectado de novo: cancela qualquer janela de foco pendente.
                        clear_focus_self();
                        // Telemetria: ping de vida da corrida aberta (30 min). Sai
                        // FORA do lock e é ~grátis quando não há corrida rolando.
                        crate::telemetry::maybe_ping();
                        true
                    }
                    Err(error) => {
                        // Registra a falha UMA vez por transição e, só nesse instante,
                        // paga o diagnóstico cruzado (que varre janelas). Assim o log
                        // já nasce com a resposta — "sim fechado" ou "acesso negado com
                        // o simulador aberto" — sem depender de o jogador abrir tela
                        // nenhuma nem reproduzir o problema de novo.
                        if crate::diagnostico::linha_unica(
                            "iracing",
                            &format!("sem telemetria: {error}"),
                        ) {
                            let d = crate::iracing_sdk::diagnosticar();
                            crate::diagnostico::linha(
                                "iracing",
                                &format!(
                                    "diagnóstico: veredito={:?} memoria_ok={} erro={} janela={} simulador={} elevado={} ticks={}",
                                    d.veredito,
                                    d.memoria_ok,
                                    d.memoria_erro,
                                    d.janela_encontrada,
                                    d.janela_simulador,
                                    d.elevado,
                                    d.ticks_observados
                                ),
                            );
                        }
                        let mut m = lock();
                        // Sim fechado com tentativa ativa = DNF.
                        let sim_closed = matches!(error, crate::iracing_sdk::IracingError::NotRunning(_));
                        if sim_closed && m.was_connected {
                            let active = m
                                .attempts
                                .last()
                                .map(|a| a.status == "active")
                                .unwrap_or(false);
                            if active {
                                m.pending_event = m.finalize_attempt("sim_closed");
                            }
                            // Fecha a captura na MESMA borda: o `history` só é útil no
                            // arquivo se for anexado antes do gzip receber o trailer.
                            // Usa `m.history` direto — `get_history()` daqui travaria,
                            // o lock já está na mão.
                            if crate::iracing_sdk::race_capture::is_active() {
                                if let Ok(v) = serde_json::to_value(&m.history) {
                                    crate::iracing_sdk::race_capture::record_block("history", v);
                                }
                                crate::iracing_sdk::race_capture::stop();
                            }
                            m.was_connected = false;
                            m.prev = None;
                            m.reset_qualy_state();
                            // Telemetria: sim fechado fecha a corrida aberta. No-op
                            // se não havia nenhuma (finalize_attempt acima já pode
                            // ter fechado). Sem isso a corrida viraria fantasma e
                            // só sumiria do contador na expiração de 35 min.
                            // Sem desfecho: a conexão caiu, então não há posição final
                            // nem volta confiável pra reportar.
                            crate::telemetry::race_end("sim_closed", None);
                            // Borda de descida: arma a janela de foco da nossa janela.
                            arm_focus_self();
                        }
                        m.connected = false;
                        false
                    }
                }
            }))
            .unwrap_or_else(|_| {
                eprintln!(
                    "[race_monitor] sampler: panic num tick (recuperado, sampler segue vivo)"
                );
                false
            });
            tick = tick.wrapping_add(1);
            let period = if connected {
                SAMPLER_PERIOD_MS
            } else {
                SAMPLER_IDLE_PERIOD_MS
            };
            std::thread::sleep(std::time::Duration::from_millis(period));
        }
    });
}

/// Liga o sampler de fundo (idempotente). Chamado no boot do app e ao exportar
/// para o iRacing — assim o monitoramento e a captura do custid ligam sozinhos,
/// sem depender de nenhum toggle. Ocioso quando o sim está fechado.
pub fn start_watching() {
    start_sampler();
}
