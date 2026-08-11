//! Observação por tick: lê a telemetria, detecta os eventos do jogador e da IA
//! e acumula as evidências da tentativa.

use super::*;

/// Passo puro da máquina de apagar o alerta de PENALIDADE: dado o "já entrou no box desde a
/// quebra", o "está no box agora" e o "estava no box no tick anterior", devolve
/// `(novo_entered_since, apagar)`. Apaga quando o carro SAI do box (true→false) já tendo
/// entrado desde a quebra = serviu a penalidade / reparou. (DNF não passa por aqui — é fixo.)
pub(crate) fn pit_clear_step(entered_since: bool, on_pit: bool, prev_on_pit: bool) -> (bool, bool) {
    let entered = entered_since || on_pit;
    let clear = prev_on_pit && !on_pit && entered;
    (entered, clear)
}

impl RaceMonitor {
    /// Carro elegível para as regras de IA: é IA e NÃO é pace car.
    pub(super) fn is_monitorable_ai(&self, idx: i32) -> bool {
        idx >= 0
            && (idx as usize) < 64
            && self.car_is_ai[idx as usize]
            && !self.car_is_pace[idx as usize]
    }

    /// Registra um evento no log (mantém os últimos [`MAX_EVENTS`]).
    pub(super) fn emit(
        &mut self,
        session_time: f64,
        lap: i32,
        kind: &str,
        car_idx: Option<i32>,
        detail: String,
        severity: Option<String>,
    ) {
        self.events.push(RaceEvent {
            session_time,
            lap,
            kind: kind.to_string(),
            car_idx,
            detail,
            severity,
        });
        if self.events.len() > MAX_EVENTS {
            let excess = self.events.len() - MAX_EVENTS;
            self.events.drain(0..excess);
        }
    }

    pub(super) fn observe(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;

        // Salto de tempo (rebobinar/avançar o replay): zera os relógios da IA, para não
        // virar falso "parado".
        //
        // O `prev` do jogador NÃO entra aqui, e é o ponto todo: um REINÍCIO de corrida é,
        // para o relógio, exatamente um salto — o `SessionTime` volta de uma vez para perto
        // de zero. Zerando o `prev` neste tick, `restarted()` ficava sem o que comparar (nem
        // pelo tempo, nem pela queda de `lap_completed`) e o reinício NUNCA era detectado: a
        // tentativa abandonada seguia aberta e tudo o que aconteceu nela — batidas, quebras,
        // incidentes, voltas — continuava acumulando na corrida que valeu, e daí para a
        // notícia, o resumo, o histórico e a carreira.
        //
        // Manter o `prev` não reabre o falso positivo que este guarda temia: quem separa o
        // reinício da rebobinada agora é o "jogador dentro do carro" em `process_player`, que
        // congela o `prev` enquanto ele está no replay ou na garagem.
        let jumped = self.live_session_time != 0.0
            && (now - self.live_session_time).abs() > REPLAY_JUMP_SECS;
        if jumped {
            self.car_monitors = [CarMonitor::DEFAULT; 64];
            self.race_green_time = None; // novo cooldown após o salto
            // A tendência de gap é a diferença entre duas amostras no tempo; atravessar um
            // salto de replay com o histórico intacto produziria "ele ganhou quarenta
            // segundos numa volta" a partir de dois instantes que nunca foram vizinhos.
            self.gap_hist.clear();
            self.estado_ultimo_refresh = f64::NEG_INFINITY;
        }

        // Marca o momento do verde (largada) para o cooldown de início. Reseta
        // fora de Racing (pré-largada/pós-corrida).
        if t.session_state == STATE_RACING {
            if self.race_green_time.is_none() {
                self.race_green_time = Some(now);
            }
        } else {
            self.race_green_time = None;
        }

        // Bandeira amarela da sessão (SessionFlags) — sempre, pois vale também
        // assistindo (inclusive para confirmar uma bandeira que enviamos).
        let flags = t.session_flags as u32;
        let caution = flags & (FLAG_CAUTION | FLAG_CAUTION_WAVING) != 0;
        if !self.prev_caution && caution {
            self.emit(
                now,
                t.lap_completed,
                "yellow_triggered",
                None,
                "Bandeira amarela".to_string(),
                None,
            );
            if let Some(rec) = self.pending_yellow_time {
                if now - rec <= YELLOW_CONFIRM_WINDOW_SECS {
                    self.emit(
                        now,
                        t.lap_completed,
                        "yellow_confirmed",
                        None,
                        "Amarela confirmada pelo SessionFlags".to_string(),
                        None,
                    );
                }
                self.pending_yellow_time = None;
            }
        }
        self.prev_caution = caution;

        // Lógica do JOGADOR (tentativa/batida/eventos) só AO VIVO. No replay ele
        // está apenas assistindo.
        if t.is_replay_playing {
            self.live_score = 0.0;
        } else {
            self.process_player(t);
        }

        // Monitoramento das IAs + decisão de bandeira + diagnóstico: SEMPRE (ao
        // vivo e no replay), pois os carros são reais em ambos os casos.
        self.process_ai_cars(t);
        // Disparo de quebra da GRADE TODA (usa a volta de cada carro do `t.cars`).
        self.tick_breakdown_grid(t);
        // Vitrine da 1ª corrida do save: quebra GARANTIDA num backmarker (nunca o jogador).
        self.tick_showcase_breakdown(t);
        self.evaluate_race_control(t);
        self.build_cars_debug(t);
        self.capture_qualy(t);
        self.record_history(t);

        // Snapshot ao vivo (display).
        self.connected = true;
        self.was_connected = true;
        self.live_g = g_force(t);
        self.live_speed_kmh = t.speed_kmh;
        self.live_tow = t.tow_time;
        self.live_state = t.session_state;
        self.live_surface = t.track_surface;
        self.live_lap = t.lap_completed;
        self.live_incident = t.incident_count;
        self.live_session_time = now;
        self.live_cars_count = t.cars.len() as i32;

        // O retrato narrado, estrangulado a ~4 Hz. Vem POR ÚLTIMO de propósito: ele lê
        // `live_is_green` e o histórico de voltas, que os passos acima acabaram de escrever.
        self.guardar_estado_agora(t);
    }

    /// O carro voltou destruído da CLASSIFICAÇÃO? Roda uma vez, na virada da quali para a
    /// corrida, sobre a tentativa que ACABOU de fechar. Não envia nada: só decide o castigo e
    /// o deixa pendente (ver [`Self::despachar_castigo_da_quali`]).
    ///
    /// Dois sinais têm de concordar, como no resto do dano: o reparo OBRIGATÓRIO que o sim
    /// pediu (o carro não anda sem consertar) e um impacto confirmado pelo monitor. Os
    /// limiares são altos de propósito: asa amassada não pode custar a etapa.
    ///
    /// O iRacing devolve o carro inteiro para a corrida, então a consequência é regra nossa,
    /// imposta por comando de admin — mesmo caminho do Sistema de Quebra.
    /// Empurra o relógio de ids do rádio para frente do que já foi emitido, ANTES de esvaziar
    /// os logs. Cada canal forma o `id` como `radio_epoch + índice`; somando aqui o tamanho de
    /// TODOS eles, nenhum id novo repete um antigo, e o overlay (que só mostra id inédito)
    /// volta a falar depois de um reinício. Ver [`RaceMonitor::radio_epoch`].
    pub(super) fn avancar_radio_epoch(&mut self) {
        self.radio_epoch += self.player_warning_log.len()
            + self.breakdown_log.len()
            + self.ritmo_log.len()
            + self.classificacao_log.len();
    }

    /// A regra está armada? (lida de `IRACER_QUALI_WRECK` uma vez, cacheada no monitor).
    fn quali_wreck_armado(&mut self) -> bool {
        *self
            .quali_wreck_on
            .get_or_insert_with(|| std::env::var(QUALI_WRECK_ENV).is_ok())
    }

    /// O POSTO efetivo do estrago da quali, juntando os três sinais na ordem de autoridade:
    /// a SEVERIDADE da batida (pico × fechadas, só com impacto) gradua; o MEATBALL — o sim
    /// declarando reparo obrigatório — é piso de "grave" (cobre o score ficar curto em pista
    /// molhada/G subamostrado); os segundos de reparo, quando existirem, também são piso.
    /// Sem impacto confirmado devolve 0: rodada limpa não castiga, meatball incluso.
    fn tier_do_estrago(attempt: &Attempt) -> usize {
        let severidade = worst_raw_severity(attempt);
        if severidade == "nenhum" {
            return 0;
        }
        let mut rank = severity_rank(severidade);
        if attempt.evidence.meatball {
            rank = rank.max(severity_rank(QUALI_WRECK_PENALTY_SEV));
        }
        if attempt.sim_repair_required_s >= QUALI_WRECK_DQ_S {
            rank = rank.max(severity_rank(QUALI_WRECK_DQ_SEV));
        } else if attempt.sim_repair_required_s >= QUALI_WRECK_PENALTY_S {
            rank = rank.max(severity_rank(QUALI_WRECK_PENALTY_SEV));
        }
        rank
    }

    /// O que o posto custa NA CORRIDA: `eol` (larga do fundo) ou `dq` (não corre).
    fn acao_na_corrida(tier: usize) -> Option<&'static str> {
        if tier >= severity_rank(QUALI_WRECK_DQ_SEV) {
            Some("dq")
        } else if tier >= severity_rank(QUALI_WRECK_PENALTY_SEV) {
            Some("eol")
        } else {
            None
        }
    }

    /// O castigo AO VIVO, dentro da própria classificação: batida "grave"+ tira o jogador da
    /// quali NA HORA (`!dq`) — o fim de semana dele muda ali, não num rodapé da largada. O
    /// rádio diz o porquê no mesmo instante, graduado pelo estrago, e a consequência de
    /// CORRIDA fica pendente para [`Self::despachar_castigo_da_quali`].
    ///
    /// Roda todo tick da quali e desarma no primeiro disparo (latch): o `!dq` é um só, e a
    /// virada de sessão ainda recalcula o posto — se o jogador piorar o carro depois do
    /// lockout, a pendência é PROMOVIDA (eol → dq), nunca rebaixada.
    pub(super) fn punir_quali_ao_vivo(&mut self, t: &IracingTelemetry) {
        if self.quali_lockout_sent
            || self.qualy_session_num < 0
            || t.session_num != self.qualy_session_num
            || !self.quali_wreck_armado()
        {
            return;
        }
        let Some(attempt) = self.attempts.last() else {
            return;
        };
        // A batida EM CURSO conta com a velocidade já perdida, e não só com o que o pico
        // registrou. Sem isto o castigo dependia de a batida FECHAR (dez segundos sem
        // pontuar), e numa quali destruída ela pode nunca fechar antes de o jogador sair da
        // sessão — foi exatamente o que aconteceu no teste de 2026-08-10: o log da fronteira
        // dizia "grave" e o castigo ao vivo nunca saiu.
        let tier = Self::tier_do_estrago(attempt)
            .max(severity_rank(severity_label(self.score_da_batida_em_curso())));
        let Some(acao) = Self::acao_na_corrida(tier) else {
            return;
        };
        // O número do carro vem da telemetria deste tick (o histórico pode não saber ainda).
        let idx = t.player_car_idx;
        let Some(num) = (idx >= 0 && (idx as usize) < 64)
            .then(|| self.car_number[idx as usize])
            .filter(|n| *n > 0)
        else {
            return; // sem número ainda; tenta no próximo tick (a pendência não se perde)
        };
        self.quali_lockout_sent = true;
        // A pendência só PROMOVE (dq vence eol): um lockout "grave" seguido de o jogador
        // conseguir piorar o carro não pode voltar a valer menos.
        if self.quali_wreck_pending != Some("dq") {
            self.quali_wreck_pending = Some(acao);
        }
        let aviso = match acao {
            "dq" if tier >= severity_rank("catastrófico") => "quali_catastrofico",
            "dq" => "quali_destruido",
            _ => "quali_grave",
        };
        self.pending_breakdown_cmds.push(format!("!dq #{num}"));
        self.player_warning_log.push(PlayerWarning {
            tipo: TipoAvisoProprio::QualiDestruida,
            part: "",
            wear_pct: 0,
            severidade: aviso,
        });
        crate::diagnostico::linha(
            "iracing",
            &format!(
                "quali interrompida pelo estrago: !dq #{num} ao vivo, tier {tier} → corrida {acao}"
            ),
        );
        self.emit(
            t.session_time,
            t.lap_completed,
            "quali_wreck_lockout",
            None,
            format!("Classificação encerrada pelo estrago (tier {tier})"),
            Some(aviso.to_string()),
        );
    }

    pub(super) fn avaliar_carro_da_quali(&mut self, t: &IracingTelemetry) {
        if !self.quali_wreck_armado() || t.session_num != self.race_session_num {
            return;
        }
        let Some(quali) = self.attempts.last() else {
            return;
        };
        let tier = Self::tier_do_estrago(quali);
        let acao = Self::acao_na_corrida(tier);
        // A medição do fim de semana, no log de arquivo. Roda UMA vez por quali, e é o único
        // lugar onde os números aparecem: sem eles não dá para calibrar os limiares — saber
        // que "não disparou" não diz se faltou pouco ou muito.
        crate::diagnostico::linha(
            "iracing",
            &format!(
                "carro da quali: meatball {}, batida {}, reparo obrigatório {:.0}s → corrida {}",
                if quali.evidence.meatball { "sim" } else { "não" },
                worst_raw_severity(quali),
                quali.sim_repair_required_s,
                acao.unwrap_or("nenhum")
            ),
        );
        // Só PROMOVE a pendência (dq > eol > nada): o lockout ao vivo pode já ter decidido, e
        // a última batida da quali pode ter piorado o carro depois dele.
        match (self.quali_wreck_pending, acao) {
            (Some("dq"), _) | (_, None) => {}
            (_, acao) => self.quali_wreck_pending = acao,
        }
    }

    /// Manda o castigo da quali assim que dá: precisa do NÚMERO do carro, que só existe
    /// depois de a sessão de corrida popular o YAML e o histórico saber quem é o jogador.
    /// Sai ANTES do verde, que é o ponto todo: o `!eol` só faz sentido nas voltas de
    /// formação, e o `!dq` tem de chegar antes de o jogador achar que vai correr.
    ///
    /// O aviso no rádio acompanha o comando, e não o contrário: se o envio falhar (o sim em
    /// fullscreen exclusivo bloqueia o `SendInput`), o latch âmbar de
    /// [`Self::note_chat_send_failure`] avisa que o comando não chegou.
    pub(super) fn despachar_castigo_da_quali(&mut self, t: &IracingTelemetry) {
        let Some(castigo) = self.quali_wreck_pending else {
            return;
        };
        if t.session_num != self.race_session_num || t.session_state >= STATE_CHECKERED {
            return;
        }
        // Quem é o jogador vem da telemetria DESTE tick, não do histórico: o histórico é
        // reescrito na virada de sessão e ainda não sabe quem ele é no primeiro frame da
        // corrida — que é justamente quando o castigo precisa sair.
        let idx = t.player_car_idx;
        let Some(num) = (idx >= 0 && (idx as usize) < 64)
            .then(|| self.car_number[idx as usize])
            .filter(|n| *n > 0)
        else {
            return; // YAML ainda não deu o número: tenta de novo no próximo tick
        };
        // O lockout da quali mandou `!dq` lá; se ele carregar para a corrida, o `!clear`
        // limpa a ficha antes do castigo certo desta sessão. (Se a quali-DQ NÃO carregar, o
        // `!clear` é inócuo — e é por isso que ele sai sempre, sem depender da resposta que
        // só a pista dá.) No DQ de corrida o `!dq` é reafirmado em vez de confiar no arrasto.
        //
        // O `!eol` só existe enquanto há fila de formação. Se a largada veio antes de o YAML
        // entregar o número, o castigo NÃO some: cai na bandeira preta, que é o mecanismo já
        // provado do Loop e vale a corrida inteira. O evento registra o comando REAL, para a
        // medição não confundir um com o outro.
        let comando = match castigo {
            "dq" => format!("!dq #{num}"),
            _ if t.session_state < STATE_RACING => format!("!eol #{num}"),
            _ => format!("!black #{num} {QUALI_WRECK_FALLBACK_PENALTY_S}"),
        };
        self.quali_wreck_pending = None;
        crate::diagnostico::linha(
            "iracing",
            &format!("castigo da quali enviado: {comando} (estado {})", t.session_state),
        );
        if castigo != "dq" && self.quali_lockout_sent {
            self.pending_breakdown_cmds.push(format!("!clear #{num}"));
        }
        self.pending_breakdown_cmds.push(comando.clone());
        self.player_warning_log.push(PlayerWarning {
            tipo: TipoAvisoProprio::QualiDestruida,
            part: "",
            wear_pct: 0,
            severidade: castigo,
        });
        self.emit(
            t.session_time,
            t.lap_completed,
            "quali_wreck_penalty",
            None,
            format!("Carro destruído na classificação: {comando}"),
            Some(castigo.to_string()),
        );
    }

    /// PICO ao vivo da batida em curso: registra o maior impacto da tentativa mesmo que a
    /// batida nunca "feche" (o jogador bate e segue). É a base do conserto do carro.
    ///
    /// Só acumula com IMPACTO confirmado — pancada de G, o incidente 4x de contato do
    /// próprio iRacing, ou o reparo que o sim passou a pedir. O scorer soma sinais de PERDA
    /// DE CONTROLE (pontos de incidente, guinada, rotação, fora da pista), e uma rodada
    /// limpa passava de "moderado" sem o carro ter tocado em nada: o import cobrava
    /// conserto de uma corrida sem batida alguma. A detecção ao vivo (bandeira, suspeita de
    /// DNF, diagnóstico) segue vendo o evento inteiro; o que exige impacto é o DANO.
    ///
    /// `com_direcao` diz se este tick pode nomear a direção do impacto. Só o caminho do
    /// G-force pode: num tick de aceleração zerada o eixo dominante empataria em zero e a
    /// batida sairia como "vertical", mandando o dano para o assoalho.
    pub(super) fn registrar_pico_de_batida(&mut self, t: &IracingTelemetry, com_direcao: bool) {
        if !self.crash_had_impact {
            return;
        }
        let peak = self.crash_components.total();
        if let Some(attempt) = self.attempts.last_mut() {
            if peak > attempt.peak_crash_score {
                attempt.peak_crash_score = peak;
                if com_direcao {
                    attempt.peak_impact_dir = Some(
                        crate::car::crash::impact_direction(t.lat_accel, t.long_accel, t.vert_accel)
                            .as_str()
                            .to_string(),
                    );
                }
            }
        }
    }

    /// Lógica do jogador AO VIVO: restart, evidências da tentativa, pontuação de
    /// batida e eventos de sessão/jogador.
    pub(super) fn process_player(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;
        let cur = Snapshot {
            session_time: now,
            lap_completed: t.lap_completed,
        };

        // 0) FRONTEIRA DE SESSÃO (treino → classificação → corrida). O iRacing troca de
        // sessão na MESMA conexão, e a tentativa é o container do dano do jogador: sem
        // cortar aqui, a rodada do treino e o toque da quali seguem vivos na corrida —
        // `peak_crash_score`, `crashes` e `collided_with_car_number` atravessam, e o import
        // cobra conserto de uma corrida limpa.
        //
        // Depender do `restarted()` para isto seria depender do relógio: a troca de sessão
        // leva o `SessionTime` de volta a zero e ele até costuma disparar, mas por heurística
        // e com o rótulo errado ("restart"). A fronteira é um FATO — `SessionNum` mudou —, e
        // ela precisa ser identificada, não inferida: é dela que sai a tentativa da quali que
        // o import cobra à parte. O histórico volta a volta já se protegia por `session_num`;
        // a tentativa não.
        if self.prev_session_num >= 0 && t.session_num != self.prev_session_num {
            let saindo_da_quali = self.prev_session_num == self.qualy_session_num;
            self.pending_event = self.finalize_attempt("session_change");
            if saindo_da_quali {
                self.quali_attempt_number = self.current_attempt;
                self.avaliar_carro_da_quali(t);
            }
        }
        self.prev_session_num = t.session_num;

        // 1) Restart contra uma tentativa ativa que já largou.
        //
        // Só vale com o jogador DENTRO do carro, e o `prev` só avança nessa condição. É o que
        // separa o reinício de verdade de uma rebobinada: no replay o `SessionTime` também
        // volta para perto de zero, e o `is_replay_playing` sozinho não protege, porque ele
        // cai a zero com o replay PAUSADO — bastaria pausar na largada para o monitor jogar
        // fora a corrida em andamento. Congelando o `prev` enquanto o jogador está fora do
        // carro (replay, garagem, guincho), a comparação que volta a valer é sempre contra o
        // último instante em que ele estava de fato pilotando, e o reinício continua sendo
        // pego mesmo que o primeiro tick depois dele chegue com o carro ainda fora do mundo.
        let no_carro = t.on_track && !t.is_replay_playing;
        let mut reiniciou_de: Option<i32> = None;
        if let Some(prev) = self.prev.filter(|_| no_carro) {
            let active_raced = self
                .attempts
                .last()
                .map(|a| a.status == "active" && a.evidence.raced)
                .unwrap_or(false);
            if active_raced && Self::restarted(&prev, &cur) {
                reiniciou_de = self.attempts.last().map(|a| a.number);
                // Qual sessão foi refeita. O SDK responde isso de graça: o `SessionNum`
                // do tick diz onde estamos, e o YAML já nos deu os números da quali e da
                // corrida. Sem esta conta, o único número que sobrava era
                // `attempt_number - 1`, que soma as trocas de sessão do fim de semana.
                if t.session_num == self.race_session_num {
                    self.restarts_corrida += 1;
                } else if t.session_num == self.qualy_session_num {
                    self.restarts_quali += 1;
                }
                self.pending_event = self.finalize_attempt("restart");
            }
        }
        self.ensure_active(now);
        // O marcador do reinício entra no log da tentativa NOVA, e não no da que morreu: o log
        // de eventos é DA TENTATIVA e o `start_attempt` acabou de esvaziá-lo. Emitido antes,
        // ele seria descartado junto com o resto — e o reinício, que é justamente o que a gente
        // precisa enxergar ao diagnosticar, sumiria do feed.
        if let Some(numero) = reiniciou_de {
            self.emit(
                now,
                t.lap_completed,
                "race_restarted",
                None,
                format!("Corrida reiniciada (#{numero})"),
                None,
            );
        }
        if no_carro {
            self.prev = Some(cur);
        }

        // 1.2) Castigo por carro destruído na classificação, se houver um pendente. Fica DEPOIS
        // do `ensure_active` de propósito: a fila de comandos é da tentativa, e o
        // `start_attempt` a esvazia — enfileirar antes seria enfileirar no lixo.
        self.despachar_castigo_da_quali(t);

        // 1.5) Estilo de pilotagem: acumula os inputs do jogador SÓ na pista e correndo
        // (pit/garagem/quali não contam). Vira fator de desgaste por peça no import — só o
        // jogador; a IA nunca. Redline desconhecido → o acumulador ignora a rotação.
        if t.track_surface == 3 && t.session_state == 4 {
            let redline = self.car_redline.unwrap_or(0.0);
            if let Some(attempt) = self.attempts.last_mut() {
                attempt
                    .style
                    .ingest(crate::car::driving_style::StyleSample {
                        throttle: t.throttle,
                        brake: t.brake,
                        rpm: t.rpm,
                        redline,
                        gear: t.gear,
                        steering_rad: t.steering_angle_rad,
                        vert_accel: t.vert_accel,
                    });
            }
            // 1.6) Disparo de quebra AO VIVO: avalia o carro do jogador nesta volta e enfileira
            // os comandos (só correndo na pista). O diretor deduplica por volta.
            self.tick_breakdown_player(t);
        }

        // 1.7) Rádio de RITMO: a volta mais rápida da corrida. FORA do gate de pista acima de
        // propósito — o jogador no box continua querendo saber quem cravou a melhor.
        self.tick_ritmo(t);

        // 1.8) O engenheiro na CLASSIFICAÇÃO. Canal próprio e sessão própria: ele sai cedo em
        // tudo que não for a sessão de quali, e é o único do rádio que fala numa volta em que o
        // jogador não está correndo contra ninguém.
        self.tick_classificacao(t);

        // 2) Evidências da tentativa.
        self.accumulate_evidence(t);

        // 2.5) DUPLA CONFIRMAÇÃO DO DANO, dos canais `PitRepairLeft`/`PitOptRepairLeft`:
        // é o próprio sim dizendo que o carro precisa de conserto, e quando esses segundos
        // aparecem costuma ser dano grave. O uso é ASSIMÉTRICO de propósito:
        //
        // - presente → confirma. Um SALTO com batida em curso vale como impacto (cobre o
        //   toque de baixa energia que quebra a asa sem levantar G), e o pico da tentativa
        //   sustenta a severidade no import.
        // - ausente → não conclui nada. O sim só popula esses canais em dano relevante, e
        //   carro amassado sem reparo pendente existe. Por isso nunca cancela dano nem abre
        //   uma batida sozinho: exige a janela já aberta pelo scorer.
        let reparo_s = t.pit_repair_needed + t.pit_opt_repair_needed;
        if reparo_s > 0.0 {
            if let Some(attempt) = self.attempts.last_mut() {
                if reparo_s > attempt.sim_repair_needed_s {
                    attempt.sim_repair_needed_s = reparo_s;
                }
                // O OBRIGATÓRIO à parte: é ele que mede "destruiu o carro" (o opcional sobe
                // com amassado de carroceria), e é a régua do castigo por destruir o carro
                // na classificação.
                if t.pit_repair_needed > attempt.sim_repair_required_s {
                    attempt.sim_repair_required_s = t.pit_repair_needed;
                }
            }
        }
        let dano_novo = self.prev_repair_needed_s >= 0.0
            && reparo_s > self.prev_repair_needed_s + REPAIR_JUMP_SECS;
        self.prev_repair_needed_s = reparo_s;
        if dano_novo && self.in_crash && !self.crash_had_impact {
            self.crash_had_impact = true;
            self.merge_crash_factors(vec![format!("reparo do sim: {reparo_s:.0}s")]);
            // A direção fica com a que o G registrou (ou o padrão frontal): um tick sem
            // aceleração nenhuma apontaria "vertical" por empate em zero.
            self.registrar_pico_de_batida(t, false);
        }

        // 3) Scorer de batida.
        let prev_incident = self.prev_incident;
        let (mut components, mut factors) = Self::score_tick(t, prev_incident);
        if self.live_tow <= 0.0 && t.tow_time > 0.0 {
            components.tow = TOW_PTS;
            factors.push("reboque acionado".to_string());
        }
        let tick_score = components.total();

        // 4) Abre/funde/fecha a batida.
        if tick_score >= SEV_MINOR {
            if !self.in_crash {
                self.in_crash = true;
                self.crash_components = Components::ZERO;
                self.crash_factors = Vec::new();
                self.crash_start_time = now;
                self.crash_start_lap = t.lap_completed;
                self.crash_had_impact = false;
                self.crash_entry_speed_ms = self.cruise_speed_ms;
                self.crash_min_speed_ms = t.speed_ms;
            }
            if components.g > 0.0 || components.incident >= INCIDENT_4X {
                self.crash_had_impact = true;
                // CONTATO: o carro no mesmo ponto da pista que o jogador é o
                // provável culpado ("quem bateu em mim"). Último contato vence.
                let culprit = self.nearest_contact_car(t);
                if let Some(num) = culprit {
                    if let Some(a) = self.attempts.last_mut() {
                        a.collided_with_car_number = Some(num);
                    }
                }
            }
            self.crash_components.merge_max(&components);
            self.merge_crash_factors(factors);
            self.crash_last_above = Some(now);
            self.registrar_pico_de_batida(t, true);
        } else if self.in_crash {
            if let Some(last) = self.crash_last_above {
                if now - last > MERGE_WINDOW_SECS {
                    self.close_crash();
                }
            }
        } else {
            self.cruise_speed_ms = t.speed_ms;
        }
        if self.in_crash {
            self.crash_min_speed_ms = self.crash_min_speed_ms.min(t.speed_ms);
        }

        // 4.2) O castigo AO VIVO da classificação: batida "grave"+ tira o jogador da quali
        // na hora. Fica DEPOIS do scorer para julgar o tick já somado (o fechamento da
        // batida, que agrega a velocidade perdida, acabou de rodar se era a hora).
        self.punir_quali_ao_vivo(t);

        // 4.5) Eventos de sessão/jogador.
        let lap = t.lap_completed;
        // O verde SÓ conta na sessão de corrida. Treino livre e classificatória também
        // passam por `SessionState = Racing`, e como `race_started_emitted` só reabre em
        // `start_attempt`, a primeira sessão do fim de semana consumia o gate: a corrida
        // largava sem tirar snapshot de grid e o `delta` da torre ficava ancorado na
        // ordem do treino. De quebra, o evento de produto `race_start` também disparava
        // em treino.
        if self.in_race_session(t)
            && self.prev_session_state < STATE_RACING
            && t.session_state >= STATE_RACING
            && t.session_state < STATE_CHECKERED
            && !self.race_started_emitted
        {
            self.race_started_emitted = true;
            // Telemetria de produto: bandeira verde = corrida rolando. UPSERT por
            // subsession no servidor, então restart não vira duas corridas.
            crate::telemetry::race_start(self.session_subsession_id, self.session_track_id);
            // Snapshot do GRID: a posição na classe no instante da largada (ainda
            // não houve ultrapassagem) = a ordem de largada. Fonte do grid quando
            // não há quali voadora (AI season larga de grade fixa).
            for car in &t.cars {
                let i = car.idx;
                if (0..64).contains(&i) && car.class_position >= 1 {
                    self.grid_class_pos[i as usize] = car.class_position;
                }
            }
            self.emit(
                now,
                lap,
                "race_started",
                None,
                "Largada (verde)".to_string(),
                None,
            );
        }

        // Bandeirada: congela quantas voltas o líder tinha AQUI. Depois deste frame o
        // número deixa de ser confiável — quem segue girando na volta de desaceleração
        // fecha mais uma, e o maior `lap_completed` da grade continua subindo com a
        // corrida já encerrada. É o teto do cabeçalho da torre em prova por TEMPO, onde
        // não existe total previsto para limitar a conta. Só em sessão de CORRIDA: o
        // treino e a classificatória também chegam a `STATE_CHECKERED`.
        if self.in_race_session(t)
            && self.prev_session_state < STATE_CHECKERED
            && t.session_state >= STATE_CHECKERED
            && self.volta_final_lider == 0
        {
            self.volta_final_lider = t
                .cars
                .iter()
                .map(|c| c.lap_completed)
                .max()
                .unwrap_or(0)
                .max(0);
        }

        let finished = self
            .attempts
            .last()
            .map(|a| a.evidence.reached_checkered)
            .unwrap_or(false);
        if finished && !self.race_finished_emitted {
            self.race_finished_emitted = true;
            self.emit(
                now,
                lap,
                "race_finished",
                None,
                "Cruzou a bandeirada".to_string(),
                None,
            );
        }

        if !self.prev_on_pit_road && t.player_on_pit_road {
            self.emit(
                now,
                lap,
                "pit_entry",
                None,
                "Entrou no pit".to_string(),
                None,
            );
        }
        if self.live_tow <= 0.0 && t.tow_time > 0.0 {
            self.emit(
                now,
                lap,
                "tow_detected",
                None,
                "Reboque acionado".to_string(),
                None,
            );
        }

        // Pins do jogador no race trace: incidentes (com pontos) e saídas de
        // pista, posicionados pela fração da volta. Só durante a corrida.
        if t.session_state >= STATE_RACING {
            let lap_f = t.lap_completed as f64 + t.lap_dist_pct.clamp(0.0, 1.0);
            let delta = self
                .prev_incident
                .map(|p| t.incident_count - p)
                .unwrap_or(0);
            if delta > 0 {
                self.player_incidents.push(PlayerIncidentMark {
                    lap_f,
                    points: delta,
                    off_track: t.track_surface == SURFACE_OFF_TRACK,
                });
            } else if t.track_surface == SURFACE_OFF_TRACK && self.prev_surface != SURFACE_OFF_TRACK
            {
                // Excursão de pista sem ponto de incidente (0x).
                self.player_incidents.push(PlayerIncidentMark {
                    lap_f,
                    points: 0,
                    off_track: true,
                });
            }
            if self.player_incidents.len() > MAX_HISTORY_LAPS {
                self.player_incidents.remove(0);
            }
        }

        // Atualiza prev_* (transições do jogador) + score ao vivo.
        self.prev_surface = t.track_surface;
        self.prev_incident = Some(t.incident_count);
        self.prev_session_state = t.session_state;
        self.prev_on_pit_road = t.player_on_pit_road;
        self.live_score = tick_score;
    }

    /// Eventos de IA: saída de pista, carro parado e provável DNF, a partir do
    /// progresso (`lap_dist_pct`) de cada carro entre ticks.
    pub(super) fn process_ai_cars(&mut self, t: &IracingTelemetry) {
        let now = t.session_time;
        let flags = t.session_flags as u32;
        let is_green =
            t.session_state == STATE_RACING && flags & (FLAG_CAUTION | FLAG_CAUTION_WAVING) == 0;
        // Ritmo de referência = o carro mais rápido (líder) no tick anterior.
        let ref_pace = self
            .car_monitors
            .iter()
            .map(|cm| cm.pace)
            .fold(0.0_f64, f64::max);

        for car in &t.cars {
            // Só carros de IA (não o jogador, não o pace car, não outros humanos).
            if car.is_player || !self.is_monitorable_ai(car.idx) {
                continue;
            }
            let i = car.idx as usize;
            let mut cm = self.car_monitors[i];

            if cm.last_dist_pct < 0.0 {
                // Primeira leitura: estabelece baseline, SEM marcar movimento.
                cm.last_dist_pct = car.lap_dist_pct;
                cm.last_move_time = now;
            } else if (car.lap_dist_pct - cm.last_dist_pct).abs() > AI_PROGRESS_EPS {
                // Andou de verdade → largou; reseta o relógio de parado.
                cm.last_move_time = now;
                cm.last_dist_pct = car.lap_dist_pct;
                cm.has_moved = true;
                cm.stopped_emitted = false;
                cm.dnf_emitted = false;
                cm.yellow_rec_emitted = false;
            }
            let stalled = now - cm.last_move_time;

            let (mut ev_offtrack, mut ev_stopped, mut ev_dnf) = (false, false, false);
            // "Parado" só vale se o carro JÁ largou (has_moved) — senão é grid.
            if is_green && !car.on_pit_road {
                if car.track_surface == SURFACE_OFF_TRACK && !cm.offtrack_emitted {
                    cm.offtrack_emitted = true;
                    ev_offtrack = true;
                }
                if car.track_surface == SURFACE_ON_TRACK {
                    cm.offtrack_emitted = false;
                }
                if cm.has_moved && stalled > AI_STOPPED_SECS && !cm.stopped_emitted {
                    cm.stopped_emitted = true;
                    ev_stopped = true;
                }
                if cm.has_moved && stalled > AI_DNF_SECS && !cm.dnf_emitted {
                    cm.dnf_emitted = true;
                    ev_dnf = true;
                }
            }

            // Ritmo (pace) — atualizado a cada PACE_WINDOW_SECS.
            if cm.pace_anchor_pct < 0.0 {
                cm.pace_anchor_pct = car.lap_dist_pct;
                cm.pace_anchor_time = now;
            } else if now - cm.pace_anchor_time >= PACE_WINDOW_SECS {
                let mut d = car.lap_dist_pct - cm.pace_anchor_pct;
                if d < 0.0 {
                    d += 1.0; // volta circular
                }
                cm.pace = d / (now - cm.pace_anchor_time);
                cm.pace_anchor_pct = car.lap_dist_pct;
                cm.pace_anchor_time = now;
                // Atingiu ritmo de corrida? Marca que já "correu" de verdade.
                if ref_pace > 0.0 && cm.pace >= RACING_PACE_FRACTION * ref_pace {
                    cm.has_raced = true;
                }
            }
            // "Lento por incidente" (alimenta o cluster de pits = acidente coletivo):
            // o ritmo despencou E o carro SAIU DA PISTA (rodada/excursão).
            // Exigir o off-track é o que separa um ACIDENTE de um carro só
            // DESACELERANDO PARA ABASTECER: a fila/rastejo na entrada do box (ON_TRACK,
            // porém quase parado por 2s+) era contada como "quase parado" e um grupo de
            // pits normais virava falso acidente coletivo. Carros que PARAM na pista por
            // batida (sem sair dela) seguem cobertos pela detecção por setor (caminho 4).
            let went_off = car.track_surface == SURFACE_OFF_TRACK;
            if cm.has_raced && ref_pace > 0.0 && cm.pace < SLOW_PACE_FRACTION * ref_pace && went_off
            {
                cm.last_slow_time = now;
            }
            // Pit de incidente: entrou no pit logo após ter ficado lento na pista.
            if car.on_pit_road && !cm.was_on_pit && now - cm.last_slow_time <= SLOW_PIT_WINDOW_SECS
            {
                cm.incident_pit_time = Some(now);
            }
            cm.was_on_pit = car.on_pit_road;

            self.car_monitors[i] = cm;

            let idx = car.idx;
            // Volta ATUAL do carro (CarIdxLap); fallback para completadas + 1.
            let car_lap = if car.lap > 0 {
                car.lap
            } else {
                car.lap_completed + 1
            };
            if ev_offtrack {
                self.emit(
                    now,
                    car_lap,
                    "ai_offtrack",
                    Some(idx),
                    format!("Carro {idx} saiu da pista"),
                    None,
                );
            }
            if ev_stopped {
                self.emit(
                    now,
                    car_lap,
                    "ai_stopped",
                    Some(idx),
                    format!("Carro {idx} parado (~{stalled:.0}s)"),
                    None,
                );
            }
            if ev_dnf {
                self.emit(
                    now,
                    car_lap,
                    "ai_possible_dnf",
                    Some(idx),
                    format!("Carro {idx} provável DNF (parado {stalled:.0}s)"),
                    None,
                );
            }
        }
    }

    pub(super) fn accumulate_evidence(&mut self, t: &IracingTelemetry) {
        let surface = t.track_surface;
        let prev_surface = self.prev_surface;
        let prev_incident = self.prev_incident;
        let flags = t.session_flags as u32;
        let prev_laps = self.attempts.last().map(|a| a.laps_completed).unwrap_or(0);
        let attempt = match self.attempts.last_mut() {
            Some(a) if a.status == "active" => a,
            _ => return,
        };
        let ev = &mut attempt.evidence;

        if t.session_state >= STATE_RACING && surface == SURFACE_ON_TRACK {
            ev.raced = true;
        }
        if ev.raced {
            if surface == SURFACE_OFF_TRACK {
                ev.off_track = true;
            }
            if surface == SURFACE_NOT_IN_WORLD {
                ev.not_in_world = true;
            }
            if surface == SURFACE_IN_PIT_STALL
                && (prev_surface == SURFACE_OFF_TRACK || prev_surface == SURFACE_NOT_IN_WORLD)
            {
                ev.towed_to_pit = true;
            }
            if flags & FLAG_BLACK != 0 {
                ev.black_flag = true;
            }
            if flags & FLAG_DISQUALIFY != 0 {
                ev.disqualified = true;
            }
            // Meatball = o sim declarou reparo OBRIGATÓRIO. Latch da tentativa: é a prova
            // ao vivo de carro quebrado que nenhum canal numérico dá fora do box.
            if flags & FLAG_REPAIR != 0 {
                ev.meatball = true;
            }
            if t.is_in_garage {
                ev.garage = true;
            }
            let checkered_shown = flags & FLAG_CHECKERED != 0 || t.session_state >= STATE_CHECKERED;
            if checkered_shown && t.lap_completed > prev_laps {
                ev.reached_checkered = true;
            }
        }
        if let Some(prev) = prev_incident {
            let delta = t.incident_count - prev;
            if delta > 0 {
                ev.incident_points += delta;
            }
        }
        attempt.laps_completed = attempt.laps_completed.max(t.lap_completed);
    }
}
