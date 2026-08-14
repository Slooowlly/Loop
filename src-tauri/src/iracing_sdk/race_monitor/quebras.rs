//! Disparo de quebra AO VIVO: instalação do diretor, avaliação por volta do
//! jogador e da grade, alertas do overlay e a fila de comandos pro chat.

use super::*;

/// CARÊNCIA DE LARGADA: nenhuma peça larga nos primeiros minutos de corrida. A primeira volta
/// é o momento mais movimentado da prova, e um `!black`/`!dq` ali chega no meio de uma disputa
/// que o jogador ainda está tentando entender — quebra de peça é história de corrida rodada,
/// não de largada. O desgaste corre normalmente durante a carência; só a falha fica trancada.
pub(super) const BREAKDOWN_GRACE_SECS: f64 = 180.0;

impl RaceMonitor {
    /// A sessão AO VIVO é aquela em que a quebra pode existir?
    ///
    /// Só a CORRIDA. Treino e classificatória também passam por `SessionState = Racing`, então o
    /// estado sozinho nunca serviu de régua — era por isso que uma peça largava na quali e o
    /// carro levava penalidade de tempo numa sessão em que não há tempo a perder. Sem sessão de
    /// corrida no YAML (`-1`) a resposta é `false`: melhor não quebrar nada do que quebrar na
    /// sessão errada. O arme de DEBUG passa por cima (é ferramenta de teste na pista).
    pub(super) fn breakdown_session_ok(&self, t: &IracingTelemetry) -> bool {
        self.breakdown_debug || self.in_race_session(t)
    }

    /// A quebra está LIBERADA neste tick? Exige a sessão de corrida e a [`BREAKDOWN_GRACE_SECS`]
    /// já cumprida desde o verde. Marca o verde na primeira passagem (e o remarca se o tempo de
    /// sessão andar pra trás, o que acontece num restart).
    pub(super) fn breakdown_allowed(&mut self, t: &IracingTelemetry) -> bool {
        if self.breakdown_debug {
            return true;
        }
        if !self.in_race_session(t) {
            return false;
        }
        let green = match self.breakdown_green_at {
            Some(g) if t.session_time >= g => g,
            _ => {
                self.breakdown_green_at = Some(t.session_time);
                t.session_time
            }
        };
        t.session_time - green >= BREAKDOWN_GRACE_SECS
    }

    /// PRODUÇÃO: instala o diretor de quebra montado com o DESGASTE REAL de cada time (vem do
    /// export, que tem DB + o grid + os números). O `player_live` guarda o estado do JOGADOR
    /// (número dele só é conhecido ao vivo → vinculado no verde). O clima da corrida (fixo, do
    /// calendário) alimenta o `on_lap`. Substitui qualquer diretor/arme debug anterior.
    pub(super) fn install_breakdown_director(
        &mut self,
        dir: crate::car::breakdown::BreakdownDirector,
        player_live: Option<crate::car::breakdown::LiveBreakdown>,
        weather: crate::car::breakdown::Weather,
        showcase: bool,
    ) {
        // Guarda o estado de INSTALAÇÃO antes de qualquer volta correr: é ele que a tentativa
        // reiniciada recebe de volta (ver `restaurar_diretor_de_quebra`).
        self.breakdown_base = Some(dir.clone());
        self.breakdown_player_base = player_live.clone();
        self.showcase_armed = showcase;
        self.breakdown = Some(dir);
        self.showcase_pending = showcase; // 1ª corrida do save → vitrine garantida da quebra
        self.pending_player_live = player_live;
        self.breakdown_weather = weather;
        self.breakdown_needs_prime = true;
        self.arm_grid_pending = false; // produção sobrepõe o arme debug da grade
        self.breakdown_debug = false; // …e o gate de sessão volta a valer
        self.breakdown_green_at = None; // corrida nova → a carência de largada recomeça
        self.breakdown_alert = [None; 64]; // corrida nova → zera os alertas do overlay
        self.breakdown_prev_on_pit = [false; 64];
        self.breakdown_repair_laps.clear();
        self.breakdown_flash_at = [0.0; 64];
        self.breakdown_progress = 0.0;
        self.avancar_radio_epoch(); // ids do rádio seguem em frente (ver `radio_epoch`)
        self.breakdown_log.clear(); // corrida nova → zera o log de desfechos (Peça 3)
        self.player_risk_warned = [false; 11]; // corrida nova → rearma os avisos pessoais
        self.player_warning_log.clear();
        self.poupar_avisado = false;
        self.ritmo.reiniciar();
        self.ritmo_log.clear();
        self.ritmo_ultima_volta = -1;
        self.chat_send_warned = false; // corrida nova → rearma o aviso de comando não-entregue
    }

    /// Devolve o diretor de quebra ao estado em que ele foi INSTALADO, e rearma o que a
    /// instalação arma. Chamado ao abrir uma tentativa nova: o desgaste que as voltas da
    /// tentativa abandonada consumiram e as peças que largaram nela não podem seguir valendo,
    /// porque aquela corrida não aconteceu — e desgaste e quebra são consequência de carreira
    /// (custo de conserto, abandono, notícia).
    ///
    /// O jogador volta a ficar PENDENTE de vínculo (o número dele só existe ao vivo) e o
    /// `prime_lap` é refeito no primeiro tick verde, senão o diretor seguiria preso na contagem
    /// de voltas da tentativa que morreu.
    ///
    /// Sem diretor de PRODUÇÃO instalado (arme de debug, ou nada) não há a que voltar, e o
    /// estado fica como está — a ferramenta de teste na pista continua valendo depois do
    /// reinício, que é o que quem está testando espera.
    pub(super) fn restaurar_diretor_de_quebra(&mut self) {
        let Some(base) = self.breakdown_base.as_ref() else {
            return;
        };
        self.breakdown = Some(base.clone());
        self.pending_player_live = self.breakdown_player_base.clone();
        self.breakdown_needs_prime = true;
        self.showcase_pending = self.showcase_armed;
        // Voltando ao diretor de PRODUÇÃO, o gate de sessão e a carência voltam a valer: o arme
        // de debug que estava por cima acabou de ser descartado junto com o resto.
        self.breakdown_debug = false;
        // SORTE NOVA por tentativa. O estado restaurado é o da instalação, semente inclusive —
        // e com a mesma semente os mesmos carros quebrariam as mesmas peças nas mesmas voltas,
        // toda vez. A corrida refeita tem que ter a mesma CHANCE de quebra, não o mesmo roteiro:
        // quem largou antes pode chegar inteiro agora, e quem sobreviveu pode largar. A primeira
        // tentativa (salt 0) mantém a sorte instalada no export.
        let salt = self.current_attempt.max(1) as u64 - 1;
        if let Some(dir) = self.breakdown.as_mut() {
            dir.reroll_luck(salt);
        }
        if let Some(live) = self.pending_player_live.as_mut() {
            live.reroll_luck(salt);
        }
    }

    /// Registra que um comando de quebra NÃO chegou ao iRacing (latch por corrida). O disparo
    /// é estrangulado (~1 a cada 1,5 s), então sem o latch um sim em fullscreen exclusivo
    /// geraria um aviso por tick; aqui vira UM aviso âmbar no rádio. Devolve `true` só na
    /// PRIMEIRA falha da corrida (pra quem quiser logar uma vez).
    pub(super) fn note_chat_send_failure(&mut self) -> bool {
        if self.chat_send_warned {
            return false;
        }
        self.chat_send_warned = true;
        true
    }

    /// DEBUG: arma uma quebra GARANTIDA no carro do jogador pra próxima volta cruzada (motor
    /// na parede → falha forçada). Serve pra testar o disparo ao vivo ponta a ponta na pista.
    /// Devolve `true` se conseguiu armar (jogador em sessão + número conhecido).
    ///
    /// O desgaste vem de [`crate::car::breakdown::WEAR_QUEBRA_GARANTIDA`], que é a própria
    /// parede. O número literal já ficou para trás uma vez: enquanto a parede era 1.05, 1.10
    /// estava além dela e a quebra era certa; quando ela subiu para 1.20, o mesmo 1.10 virou
    /// regime de sobreuso — ~13,5%/volta no motor —, e a ferramenta passou a prometer uma
    /// garantia que era sorte alta, sem que nada acusasse.
    pub(super) fn arm_test_breakdown(&mut self) -> bool {
        let Some(car_num) = self.player_car_number() else {
            return false;
        };
        let mut car = crate::car::Car::uniform(3);
        car.set_wear(
            crate::car::PartType::Engine,
            crate::car::breakdown::WEAR_QUEBRA_GARANTIDA,
        );
        let live = crate::car::breakdown::LiveBreakdown::new(&car, 1, 50.0, (1.0, 1.0, 1.0));
        let mut dir = crate::car::breakdown::BreakdownDirector::new();
        dir.add_car(car_num, live, Vec::new());
        // Começa da volta atual → dispara na PRÓXIMA cruzada, não retroage.
        dir.prime_lap(car_num, self.live_lap.max(0) as u32);
        self.breakdown = Some(dir);
        self.breakdown_debug = true; // teste na pista: sem gate de sessão nem carência
        true
    }

    /// Avalia a quebra do carro do jogador nesta volta e enfileira os comandos. Chamado por
    /// tick no `process_player`; o diretor deduplica por volta (só avança pra frente).
    pub(super) fn tick_breakdown_player(&mut self, t: &IracingTelemetry) {
        if self.breakdown.is_none() {
            return;
        }
        // A quebra é da CORRIDA: na quali o diretor nem avança (ver `breakdown_session_ok`).
        if !self.breakdown_session_ok(t) {
            return;
        }
        let allow_break = self.breakdown_allowed(t);
        let Some(car_num) = self.player_car_number() else {
            return;
        };
        let idx = self.history.player_car_idx;
        let lap = t.lap_completed.max(0) as u32;
        // Clima VIVO do SDK, o MESMO de `tick_breakdown_grid` — não pode ser o baseline fixo.
        //
        // Este tick roda ANTES do da grade (`process_player` vem antes em `on_telemetry`), e
        // `on_lap_at` só avança pra frente: quem chega primeiro na volta é quem manda. Com o
        // baseline aqui, o carro do JOGADOR rodava a corrida inteira sob o clima do export
        // enquanto o resto da grade rodava sob a chuva e a temperatura reais — e ainda
        // alternava entre os dois, porque no box este tick não roda e a grade avançava o carro
        // dele com o clima vivo. Fura o §8 do design (o jogador herda o tier do time, sem
        // desconto no risco): num dia quente o motor dele ficava protegido do estresse térmico.
        // Com os dois ticks no mesmo clima, a ordem entre eles deixa de importar.
        let weather = effective_weather(t, self.breakdown_weather);
        let progress = self.breakdown_progress; // enduro: rampa de fim (a grade atualiza)
                                                // Guardado por `is_none()` no topo; `let Some … else` em vez de `unwrap` pra
                                                // nunca derrubar a thread de telemetria se o invariante mudar.
        let Some(dir) = self.breakdown.as_mut() else {
            return;
        };
        let evs = dir.on_lap_at_cfg(car_num, lap, weather, progress, allow_break);
        for ev in evs {
            self.pending_breakdown_cmds.push(ev.command(car_num));
            self.breakdown_log
                .push(BreakdownOutcome::from_event(car_num, &ev));
            if idx >= 0 && (idx as usize) < 64 {
                self.breakdown_alert[idx as usize] = Some(BreakdownAlert {
                    severity: ev.severity,
                    entered_pit_since: false,
                });
            }
            // O DESFECHO, no canal do jogador. A linha continua entrando no `breakdown_log` (é
            // ela que manda o `!black`/`!dq` e alimenta o resultado), mas o rádio da GRADE agora
            // a ignora — ver `get_breakdown_feed`. Sem este par, ou o jogador ouve falarem dele
            // em 3ª pessoa, ou não ouve nada no momento em que o carro dele morre.
            self.player_warning_log.push(PlayerWarning {
                tipo: TipoAvisoProprio::Quebra,
                part: ev.part.as_str(),
                wear_pct: 0,
                severidade: ev.severity.key(),
            });
        }

        // AVISO pessoal: peças do jogador que ENTRARAM na zona de risco (≥ `RISK_OPEN`, hoje
        // 90%, e abaixo da parede de 120%). Avisa cada
        // peça UMA vez; rearma quando ela sai da zona (troca/reparo/quebra). `danger` é dono
        // (Vec), então o borrow do diretor fecha antes de mexer no estado de aviso.
        let Some(dir) = self.breakdown.as_ref() else {
            return;
        };
        let danger = dir.car_parts_in_danger(car_num);
        let mut in_danger = [false; 11];
        for (i, pt, wear) in &danger {
            in_danger[*i] = true;
            if !self.player_risk_warned[*i] {
                self.player_risk_warned[*i] = true;
                self.player_warning_log.push(PlayerWarning {
                    tipo: TipoAvisoProprio::Peca,
                    part: pt.as_str(),
                    wear_pct: (wear * 100.0).round().clamp(0.0, 255.0) as u8,
                    severidade: "",
                });
            }
        }
        for i in 0..11 {
            if !in_danger[i] {
                self.player_risk_warned[i] = false;
            }
        }

        // POUPAR O CARRO: o cruzamento de dois sinais, e só o cruzamento.
        //
        // Peça nossa no limite, sozinha, é o desgaste normal de fim de stint — mandar poupar
        // ali seria mandar poupar em toda corrida. Grid quebrando, sozinho, é problema dos
        // outros. Juntos, dizem que a pista está cobrando mais caro do que o previsto, e aí
        // aliviar é a decisão certa e vale interromper o piloto para dizê-la.
        //
        // Uma vez por corrida, latch. Ela pede uma MUDANÇA DE PILOTAGEM; repetida vira ruído,
        // e ruído é o que faz o piloto desligar o rádio.
        if !self.poupar_avisado && !danger.is_empty() {
            let abandonos = self
                .breakdown_log
                .iter()
                .filter(|o| o.severity.e_abandono())
                .count() as u32;
            if abandonos > crate::engenheiro::quebra::ABANDONOS_PARA_COMENTAR {
                self.poupar_avisado = true;
                self.player_warning_log.push(PlayerWarning {
                    tipo: TipoAvisoProprio::Poupar,
                    part: "",
                    wear_pct: 0,
                    severidade: "",
                });
            }
        }
    }

    /// O RÁDIO DE RITMO, uma vez por volta cruzada.
    ///
    /// Roda no mesmo lugar do tick de quebra e pelo mesmo motivo: os dois são eventos DE VOLTA
    /// num laço que roda a 60 Hz. A trava de volta fica aqui, e não dentro do observador, para
    /// ele continuar puro — um observador que precisa saber a taxa de amostragem do chamador
    /// deixa de ser testável sem o SDK.
    pub(super) fn tick_ritmo(&mut self, t: &IracingTelemetry) {
        if t.session_state != STATE_RACING {
            return;
        }
        let volta = t.lap_completed;
        if volta <= 0 || volta == self.ritmo_ultima_volta {
            return;
        }
        self.ritmo_ultima_volta = volta;

        // O estado montado, e não os campos crus: a melhor da corrida com dono já é derivada
        // lá, e recalculá-la aqui abriria a chance de as duas leituras discordarem.
        let e = self.montar_estado_agora();
        let fala = self.ritmo.observar(crate::engenheiro::ritmo::Passagem {
            volta,
            minha_volta_s: e.ultima_volta_s,
            melhor_da_corrida_s: e.melhor_da_corrida_s,
            dono_idx: e.melhor_da_corrida_idx,
            e_minha: e.melhor_da_corrida_e_minha,
        });
        let Some(fala) = fala else { return };

        use crate::engenheiro::ritmo::Fala;
        let pronta = match fala {
            Fala::Tomamos(c) => FalaDeRitmo::Tomamos(c),
            Fala::Aproximando(c) => FalaDeRitmo::Aproximando(c),
            Fala::DeOutro {
                lead,
                tempo,
                dono_idx,
            } => {
                // Sem número resolvido não há quem nomear, e a fala perderia o sujeito. Melhor
                // engolir esta e deixar a próxima troca falar.
                let num = self
                    .car_number
                    .get(dono_idx.max(0) as usize)
                    .copied()
                    .unwrap_or(0);
                if dono_idx < 0 || num <= 0 {
                    return;
                }
                FalaDeRitmo::MelhorDeOutro {
                    lead,
                    car_number: num as u32,
                    tempo,
                    decimos: crate::engenheiro::tempo_volta::decimos_de(e.melhor_da_corrida_s),
                }
            }
        };
        self.ritmo_log.push(pronta);
    }

    /// DEBUG: pede armar a GRADE TODA (montada no próximo tick com `t.cars`).
    pub(super) fn request_arm_grid(&mut self) {
        self.arm_grid_pending = true;
        self.breakdown_debug = true; // teste na pista: sem gate de sessão nem carência
    }

    /// Avalia a quebra de TODOS os carros da sessão nesta volta (usa a volta de cada carro do
    /// `t.cars`). Se foi pedido armar a grade, monta agora: uma peça perto de quebrar (~0.97)
    /// por carro, presa na volta atual. Só correndo. O diretor deduplica por volta.
    pub(super) fn tick_breakdown_grid(&mut self, t: &IracingTelemetry) {
        let racing = t.session_state == STATE_RACING;
        if self.arm_grid_pending && racing {
            let mut dir = crate::car::breakdown::BreakdownDirector::new();
            for c in t
                .cars
                .iter()
                .filter(|c| c.idx >= 0 && (c.idx as usize) < 64)
            {
                let num = self.car_number[c.idx as usize];
                if num <= 0 {
                    continue;
                }
                // Uma peça (variando por carro) perto de quebrar; o resto sadio.
                let seed = (num as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x00B4_EA12;
                let part =
                    crate::car::PartType::ALL[(num as usize) % crate::car::PartType::ALL.len()];
                let mut car = crate::car::Car::uniform(3);
                car.set_wear(part, 0.97);
                let live =
                    crate::car::breakdown::LiveBreakdown::new(&car, seed, 50.0, (1.0, 1.0, 1.0));
                dir.add_car(num as u32, live, Vec::new());
                dir.prime_lap(num as u32, c.lap_completed.max(0) as u32);
            }
            self.breakdown = Some(dir);
            self.arm_grid_pending = false;
        }
        if self.breakdown.is_none() || !racing {
            return;
        }
        // A quebra é da CORRIDA. Fora dela o diretor não avança volta nenhuma — nem para
        // quebrar, nem para prender os carros: o `prime_lap` feito na quali travava o diretor
        // numa contagem de voltas que a corrida ia levar meia prova pra alcançar.
        if !self.breakdown_session_ok(t) {
            return;
        }
        let allow_break = self.breakdown_allowed(t);
        // PRODUÇÃO: no primeiro tick verde após instalar o diretor real, prende cada carro na
        // volta atual (não retroage) e VINCULA o jogador (número só conhecido agora, ao vivo).
        if self.breakdown_needs_prime {
            if let Some(live) = self.pending_player_live.take() {
                if let Some(pnum) = self.player_car_number() {
                    if let Some(dir) = self.breakdown.as_mut() {
                        dir.add_car(pnum, live, Vec::new());
                    }
                }
            }
            let Some(dir) = self.breakdown.as_mut() else {
                return;
            };
            for c in t
                .cars
                .iter()
                .filter(|c| c.idx >= 0 && (c.idx as usize) < 64)
            {
                let num = self.car_number[c.idx as usize];
                if num > 0 {
                    dir.prime_lap(num as u32, c.lap_completed.max(0) as u32);
                }
            }
            self.breakdown_needs_prime = false;
        }
        // Clima VIVO do SDK (Peça 2): blend do baseline fixo com os canais ao vivo — captura o
        // arco de chuva e a deriva de temperatura na volta em que cada carro cruza.
        let weather = effective_weather(t, self.breakdown_weather);
        // Progresso por tempo (enduro): guarda pro tick do jogador reusar.
        let progress = session_progress(t);
        self.breakdown_progress = progress;
        for c in t
            .cars
            .iter()
            .filter(|c| c.idx >= 0 && (c.idx as usize) < 64)
        {
            let idx = c.idx as usize;
            let num = self.car_number[idx];
            if num <= 0 {
                continue;
            }
            let car_num = num as u32;
            let lap = c.lap_completed.max(0) as u32;
            let Some(dir) = self.breakdown.as_mut() else {
                break;
            };
            let evs = dir.on_lap_at_cfg(car_num, lap, weather, progress, allow_break);
            for ev in evs {
                self.pending_breakdown_cmds.push(ev.command(car_num));
                self.breakdown_log
                    .push(BreakdownOutcome::from_event(car_num, &ev));
                // Estado de alerta pro overlay: última peça a largar manda (leve/grave/DNF).
                self.breakdown_alert[idx] = Some(BreakdownAlert {
                    severity: ev.severity,
                    entered_pit_since: false,
                });
                // Carimba o instante → flash de 5 s na torre (junto do rádio do engenheiro).
                self.breakdown_flash_at[idx] = t.session_time;
            }
            // Apaga o alerta de PENALIDADE quando o carro sai do box reparado. DNF é fixo.
            let on_pit = c.on_pit_road;
            let prev = self.breakdown_prev_on_pit[idx];
            self.breakdown_prev_on_pit[idx] = on_pit;
            let mut clear = false;
            let mut record_repair = false;
            if let Some(al) = self.breakdown_alert[idx].as_mut() {
                if al.severity != crate::car::breakdown::Severity::Dnf {
                    // Entrou no box (false→true) com peça quebrada = parada de REPARO.
                    record_repair = on_pit && !prev;
                    let (entered, do_clear) = pit_clear_step(al.entered_pit_since, on_pit, prev);
                    al.entered_pit_since = entered;
                    clear = do_clear;
                }
            }
            if record_repair {
                self.breakdown_repair_laps
                    .push((c.idx, c.lap_completed.max(0) as u32));
            }
            if clear {
                self.breakdown_alert[idx] = None;
            }
        }
    }

    /// VITRINE da 1ª corrida do save (dispara UMA vez): garante que o PENÚLTIMO carro — nunca o
    /// jogador — sofra uma quebra GRAVE e pare pra arrumar, pra o jogador ver o sistema logo de
    /// cara. Escolhe o alvo pela posição AO VIVO (penúltimo colocado; se esse for o jogador, o
    /// último). Injeta o `!black` direto (não passa pela sorte do diretor) e registra
    /// alerta/flash/log como uma quebra normal — assim overlay, debrief e notícia acendem igual.
    pub(super) fn tick_showcase_breakdown(&mut self, t: &IracingTelemetry) {
        if !self.showcase_pending {
            return;
        }
        // Só correndo, e depois de o jogador dar 2 voltas (grade espalhada + ele já engajado).
        const SHOWCASE_TRIGGER_LAP: i32 = 2;
        if t.session_state != STATE_RACING || t.lap_completed < SHOWCASE_TRIGGER_LAP {
            return;
        }
        // A vitrine é uma quebra: obedece às mesmas regras (só na corrida, só depois da
        // carência). Injetada à mão, ela não passa pelo diretor — o gate precisa vir aqui.
        if !self.breakdown_session_ok(t) || !self.breakdown_allowed(t) {
            return;
        }
        // Acha os DOIS últimos colocados (posição > 0), guardando se cada um é o jogador. O pace
        // car vem com posição 0 e cai fora naturalmente.
        let mut last: Option<(i32, usize, bool)> = None; // (posição, car_idx, is_player)
        let mut penult: Option<(i32, usize, bool)> = None;
        for c in t.cars.iter() {
            if c.idx < 0 || (c.idx as usize) >= 64 || c.position <= 0 {
                continue;
            }
            let idx = c.idx as usize;
            if self.car_is_pace[idx] {
                continue;
            }
            let entry = (c.position, idx, c.is_player);
            if last.map_or(true, |(p, _, _)| c.position > p) {
                penult = last;
                last = Some(entry);
            } else if penult.map_or(true, |(p, _, _)| c.position > p) {
                penult = Some(entry);
            }
        }
        // Alvo = penúltimo; se for o jogador (ou não existir), cai pro último. Os dois nunca são o
        // mesmo carro, então o fallback garante um alvo != jogador.
        let target = match penult {
            Some((_, idx, false)) => Some(idx),
            _ => last
                .filter(|(_, _, is_player)| !*is_player)
                .map(|(_, idx, _)| idx),
        };
        let Some(idx) = target else {
            return; // grade ainda não classificada / só o jogador — tenta no próximo tick
        };
        let num = self.car_number[idx];
        if num <= 0 {
            return;
        }
        let car_lap = t
            .cars
            .iter()
            .find(|c| c.idx == idx as i32)
            .map(|c| c.lap_completed.max(0))
            .unwrap_or(0) as u32;
        // Peça notável (reparo perceptível) variando por número de carro; sempre GRAVE, nunca DNF.
        use crate::car::PartType;
        const SHOWCASE_PARTS: [PartType; 5] = [
            PartType::Gearbox,
            PartType::Engine,
            PartType::Suspension,
            PartType::Cooling,
            PartType::Brakes,
        ];
        let part = SHOWCASE_PARTS[(num as usize) % SHOWCASE_PARTS.len()];
        let seed = (num as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5340_57CA;
        let ev = crate::car::breakdown::showcase_repair_event(part, car_lap, 50.0, seed);
        self.pending_breakdown_cmds.push(ev.command(num as u32));
        self.breakdown_log
            .push(BreakdownOutcome::from_event(num as u32, &ev));
        self.breakdown_alert[idx] = Some(BreakdownAlert {
            severity: ev.severity,
            entered_pit_since: false,
        });
        self.breakdown_flash_at[idx] = t.session_time;
        self.showcase_pending = false; // uma vez só
    }

    /// Snapshot dos alertas de quebra ativos, por car_idx, pro overlay:
    /// `(car_idx, kind)` com kind ∈ "light" | "heavy" | "dnf". Vazio = sem alerta.
    pub(super) fn breakdown_alerts_snapshot(&self) -> Vec<(i32, &'static str)> {
        use crate::car::breakdown::Severity;
        let mut out = Vec::new();
        for (idx, slot) in self.breakdown_alert.iter().enumerate() {
            if let Some(al) = slot {
                let kind = match al.severity {
                    Severity::Light => "light",
                    Severity::Heavy => "heavy",
                    Severity::Dnf => "dnf",
                };
                out.push((idx as i32, kind));
            }
        }
        out
    }

    /// car_idx cujo FLASH ainda está ativo (quebrou nos últimos [`FLASH_SECS`] s). A torre pisca
    /// a linha desses pilotos, em sincronia com o rádio do engenheiro.
    pub(super) fn breakdown_flash_idxs(&self) -> Vec<i32> {
        const FLASH_SECS: f64 = 5.0;
        let now = self.live_session_time;
        (0..64)
            .filter(|&i| {
                let at = self.breakdown_flash_at[i];
                at > 0.0 && (now - at) < FLASH_SECS
            })
            .map(|i| i as i32)
            .collect()
    }

    /// Tira UM comando da fila (drena estrangulado no sampler, pra não spammar o chat/foco).
    pub(super) fn take_one_breakdown_cmd(&mut self) -> Option<String> {
        if self.pending_breakdown_cmds.is_empty() {
            None
        } else {
            Some(self.pending_breakdown_cmds.remove(0))
        }
    }
}
