//! O ESTADO AGORA: um retrato pequeno e já mastigado da corrida neste instante.
//!
//! Existe porque nenhuma das três superfícies do monitor servia a quem precisa responder
//! uma pergunta falada. O [`poll`](super::poll) nasceu para diagnóstico de batida e não
//! carrega nem a posição do jogador; o `get_history`/`get_feedback` carregam a corrida
//! inteira — todas as voltas de todos os carros — que é caro de serializar e ainda deixa a
//! conta por fazer do outro lado.
//!
//! A regra desta camada é essa: **aqui se calcula, lá fora só se redige.** Quem consome
//! (o engenheiro do push-to-talk, o spotter, um overlay) recebe "faltam 12 voltas e o
//! combustível dá para 14" — não recebe litros e um vetor de voltas para descobrir sozinho.
//! Toda conta que exigir mais de uma subtração mora deste lado.
//!
//! ## Por que o gap NÃO usa `f2_time`
//!
//! O `CarIdxF2Time` é o gap ao LÍDER, e em corrida de IA — que é o caso do Loop — ele ora
//! vem 0, ora vem populado mas só reescrito na passagem pela linha, misturando diferença de
//! voltas com distância de pista. Numa captura real, dois carros a 40 s um do outro tinham
//! `f2_time` a 0,165 s de distância. O `CarIdxEstTime` (segundos desde a linha até onde o
//! carro está) é populado em qualquer sessão, e a diferença entre dois carros nele é o gap
//! na pista. É por ele que se mede aqui — e no [histórico](super::historico).
//!
//! A conta é CIRCULAR: quem está à frente pode já ter cruzado a linha, e aí o `est_time`
//! dele é MENOR que o meu. Sem fechar o círculo com o tempo de volta, um adversário a três
//! décimos apareceria como um minuto e meio.

use super::*;

/// Sentinela de "ilimitado" que o iRacing manda em `SessionLapsRemainEx` durante prova por
/// TEMPO. Ler esse número como voltas restantes é o erro clássico — daí a constante ter
/// nome em vez de aparecer solta numa comparação.
const LAPS_REMAIN_SENTINELA: i32 = 32767;

/// Quantas amostras de gap guardar para dizer se o carro está VINDO ou INDO EMBORA.
/// A 4 Hz, 240 amostras são 60 segundos — cobre a maior parte de uma volta em quase toda
/// categoria, que é a janela em que "ele está tirando dois décimos" significa alguma coisa.
const GAP_HIST_MAX: usize = 240;

/// Janela mínima de observação para a tendência valer. Abaixo disso o número é ruído de
/// tráfego e freada, e um engenheiro que anuncia tendência a cada dois segundos mente mais
/// do que informa.
const TENDENCIA_JANELA_MIN_S: f64 = 8.0;

/// Uma amostra do par de gaps num instante — a matéria-prima da tendência.
#[derive(Clone, Copy)]
pub(super) struct AmostraGap {
    pub(super) t: f64,
    pub(super) idx_frente: i32,
    pub(super) gap_frente: f64,
    pub(super) idx_atras: i32,
    pub(super) gap_atras: f64,
}

/// O carro imediatamente à frente ou atrás, com tudo que dá para dizer sobre ele.
#[derive(Clone, Debug, Serialize)]
pub struct Vizinho {
    pub idx: i32,
    /// Nome do piloto (do YAML da sessão). Vazio se a sessão ainda não foi lida.
    pub nome: String,
    /// Número do carro — a ponte para o `driver_id` da carreira.
    pub numero: i32,
    /// Gap na pista em segundos, pelo `est_time` e fechando o círculo da volta.
    /// -1 quando não deu para calcular (sem tempo de volta de referência).
    pub gap_s: f64,
    /// Ritmo dele: última volta completa em segundos. -1 se não houver volta válida.
    pub ritmo_s: f64,
    /// Meu ritmo menos o dele. **Negativo = eu sou mais rápido.** É o número que decide
    /// se a disputa vai acontecer ou se ela já acabou.
    pub delta_ritmo_s: f64,
    /// Voltas até encostar nele (ou até ele encostar em mim), no ritmo atual dos dois.
    /// -1 quando a distância está AUMENTANDO — porque "nunca" não é um número.
    pub voltas_para_alcancar: f64,
    /// Variação do gap por volta, medida na janela de observação. Negativo = **vindo**;
    /// positivo = indo embora. 0 quando ainda não há janela suficiente.
    pub tendencia_s_por_volta: f64,
    /// Está no pit road agora — muda completamente o valor da informação.
    pub no_box: bool,
    /// Está uma volta ou mais atrás/à frente: é tráfego a ser dobrado, não adversário.
    pub volta_a_parte: bool,
    /// Índice do composto de pneu dele (`CarIdxTireCompound`), 0-based por série.
    /// -1 = desconhecido. Ver a nota sobre composto em [`EstadoAgora`].
    pub composto: i32,
    /// Há quantas voltas ele está com este jogo de pneus, contado da última parada com
    /// troca. -1 quando não dá para saber. Ver [`idade_do_pneu`].
    pub pneu_voltas: i32,
}

/// Retrato da corrida agora. Todo campo é ou um fato lido direto, ou uma conta já fechada.
///
/// ## Sobre o composto e a idade do pneu
///
/// O `CarIdxTireCompound` é um índice 0-based por série, e ele **vira nome** porque o
/// iRacing tem exatamente dois compostos: `0` é seco, `1` é chuva. A tradução mora em
/// [`Compound::from_indice`](crate::iracing_sdk::tire_strategy::Compound::from_indice), que
/// devolve `Unknown` para qualquer outro índice em vez de arredondar para o vizinho — se um
/// dia a premissa dos dois compostos cair, o código cala em vez de mentir.
///
/// O campo vem vazio (`-1`) em carro **mono-composto**, como o MX-5: lá não há escolha a
/// informar. Isso não é falha de leitura, e o dossiê trata os dois casos diferente.
///
/// A **idade** é outra história e não vem do SDK: o iRacing não expõe desgaste, temperatura
/// nem pressão. Ela é reconstruída do tempo PARADO na caixa — trocar pneu é um bloco fixo de
/// ~21,5 s por cima do abastecimento, então uma parada acima de
/// [`TIRE_CHANGE_MIN_SECS`](crate::iracing_sdk::tire_strategy::TIRE_CHANGE_MIN_SECS) trocou
/// pneu e a contagem recomeça. Ver [`idade_do_pneu`].
#[derive(Clone, Debug, Serialize)]
pub struct EstadoAgora {
    pub conectado: bool,
    /// A sessão está em corrida (não treino, quali, formação ou pós-bandeirada).
    ///
    /// Até 17/08/2026 isto era só `session_state == Racing`, e esse estado vale **também em
    /// treino e classificação** — o próprio [`crate::radio_registro`] já registrava a
    /// armadilha. O efeito estava no rádio: no fim de uma classificatória o engenheiro abria
    /// com "Novato, que corrida", tendo o jogador nem marcado tempo. Hoje o estado é cruzado
    /// com a sessão de corrida do YAML, que é o que [`tipo_sessao`](Self::tipo_sessao)
    /// responde.
    pub em_corrida: bool,
    /// Qual sessão do fim de semana: `corrida`, `classificacao` ou `treino`.
    ///
    /// Existe porque `em_corrida` responde sim ou não, e quem escreve a fala precisa saber
    /// **qual** sessão é para não chamar a classificatória de treino. Chave estável.
    pub tipo_sessao: &'static str,
    /// Bandeira verde ondulando agora (sem caution).
    pub verde: bool,
    /// Sessão de corrida ANTES do primeiro verde da tentativa: grid, volta de apresentação,
    /// pace car. Nada do que se diga sobre disputa vale ainda.
    ///
    /// Até 17/08/2026 isto era `PaceMode > 0`, e o `PaceMode` das sessões com IA mente dos
    /// dois lados: vale 4 (NotPacing) a classificação inteira e fica preso em 1 a 3 a corrida
    /// inteira — medido em Okayama e Oschersleben. O efeito era `em_formacao` verdadeiro
    /// QUASE SEMPRE, e ele é o portão de `em_corrida_de_verdade`: ~820 peças gravadas do
    /// acervo de resposta nunca tocavam, o briefing de "antes da largada" saía dentro da
    /// classificação, e o dossiê dizia "volta de formação" no meio da prova.
    pub em_formacao: bool,

    // ── Posição ──
    pub posicao: i32,
    pub posicao_classe: i32,
    pub total_carros: i32,

    // ── Volta e o que falta ──
    pub volta: i32,
    /// Total de voltas da prova. 0 em prova por tempo.
    pub voltas_totais: i32,
    /// Voltas que ainda faltam. -1 quando não dá para saber.
    pub voltas_restantes: i32,
    /// **Se `true`, `voltas_restantes` é ESTIMATIVA** (prova por tempo, calculada do tempo
    /// restante e do ritmo). Quem for redigir tem de dizer "por volta de", não um número
    /// seco — é a diferença entre um engenheiro e um cronômetro quebrado.
    pub voltas_restantes_estimadas: bool,
    /// Tempo restante de sessão em segundos. -1 em prova por voltas.
    pub tempo_restante_s: f64,

    // ── Vizinhança ──
    pub frente: Option<Vizinho>,
    pub atras: Option<Vizinho>,

    // ── Ritmo ──
    pub ultima_volta_s: f64,
    pub melhor_volta_s: f64,
    /// Última menos a melhor. Positivo = esta volta foi pior que a melhor.
    pub delta_melhor_s: f64,
    /// A volta mais rápida DA CORRIDA, de quem quer que seja. -1 quando ninguém marcou ainda.
    ///
    /// Diferente de [`melhor_volta_s`](Self::melhor_volta_s), que é a do jogador. É a que o
    /// rádio anuncia e a que se persegue — não dá para "conseguir a melhor volta" perseguindo
    /// a própria.
    pub melhor_da_corrida_s: f64,
    /// `car_idx` de quem detém a volta mais rápida da corrida. -1 se não há.
    pub melhor_da_corrida_idx: i32,
    /// A volta mais rápida da corrida é do jogador.
    pub melhor_da_corrida_e_minha: bool,

    // ── Combustível ──
    pub combustivel_l: f64,
    /// Consumo médio por volta (L). -1 se ainda não há duas voltas com tanque captado.
    pub consumo_por_volta_l: f64,
    /// Quantas voltas o tanque ainda dá. -1 se o consumo não é conhecido.
    pub autonomia_voltas: f64,
    /// **Autonomia menos voltas restantes.** Positivo = sobra; negativo = falta e vai
    /// precisar parar. É a única forma útil do dado — "dá para 14 voltas" não responde
    /// nada sem saber quantas faltam.
    ///
    /// Desconhecido é `NaN`, não -1: aqui o negativo é resposta, não sentinela.
    pub saldo_combustivel_voltas: f64,

    // ── Carro ──
    /// Segundos de reparo OBRIGATÓRIO pendentes (`PitRepairLeft`).
    pub reparo_obrigatorio_s: f64,
    /// Segundos de reparo OPCIONAL (`PitOptRepairLeft`) — reflete melhor o estrago total.
    pub reparo_opcional_s: f64,
    pub reparos_rapidos_usados: i32,

    // ── Bandeira e disciplina ──
    /// Rótulo legível da bandeira mais grave ativa ("Amarela", "Preta", "Branca"…).
    /// Vazio quando não há nada digno de nota.
    pub bandeira: String,
    pub bandeira_preta: bool,
    pub desclassificado: bool,
    pub incidentes: i32,

    // ── Pista e tempo ──
    pub pista_molhada: bool,
    /// `TrackWetness` cru, 0..7 — a intensidade, não só o sim/não.
    pub umidade_pista: i32,
    /// Chuva caindo AGORA (fração). Diferente de a pista estar molhada: é o que antecipa.
    pub chuva_agora: f64,
    /// A prova foi declarada molhada pela direção.
    pub declarada_molhada: bool,
    pub temp_pista_c: f64,
    pub temp_ar_c: f64,
    pub boxes_abertos: bool,

    // ── Pneu ──
    /// Índice do composto do JOGADOR. -1 = desconhecido.
    pub meu_composto: i32,
    /// Há quantas voltas o jogador está com este jogo de pneus. -1 = desconhecido.
    pub meu_pneu_voltas: i32,
    /// `(índice do composto, quantos carros nele)` — o campo inteiro, ordenado por índice.
    /// A matéria-prima para descobrir qual índice é chuva sem tabela nenhuma.
    pub compostos_no_grid: Vec<(i32, i32)>,
}

// ─── Cálculos puros (testáveis sem SDK, sem monitor, sem Tauri) ──────────────

/// Distância no tempo indo PARA FRENTE de um ponto da pista até outro, fechando o círculo
/// da volta. `de` e `para` são `est_time` (segundos desde a linha).
///
/// Quando `para` é menor que `de`, o alvo já cruzou a linha e está na volta seguinte: a
/// distância é o que falta para eu completar a volta mais o que ele já andou nela. Sem isso
/// um carro a três décimos apareceria como uma volta inteira menos três décimos.
pub(super) fn gap_circular(de: f64, para: f64, volta_ref_s: f64) -> f64 {
    if !de.is_finite() || !para.is_finite() || volta_ref_s <= 0.0 {
        return -1.0;
    }
    let d = para - de;
    if d >= 0.0 {
        d
    } else {
        d + volta_ref_s
    }
}

/// Quantas voltas ainda faltam, e se o número é estimativa.
///
/// Em prova por VOLTAS o próprio sim responde. Em prova por TEMPO ele manda o sentinela de
/// "ilimitado", e a única saída é dividir o tempo restante pelo ritmo — o que dá um número
/// honesto e aproximado, nunca exato. O `bool` de retorno existe para que essa diferença
/// não se perca no caminho até a fala.
pub(super) fn voltas_restantes(
    laps_remain_ex: i32,
    tempo_restante_s: f64,
    ritmo_s: f64,
) -> (i32, bool) {
    if laps_remain_ex > 0 && laps_remain_ex < LAPS_REMAIN_SENTINELA {
        return (laps_remain_ex, false);
    }
    if tempo_restante_s > 0.0 && ritmo_s > 0.0 {
        return ((tempo_restante_s / ritmo_s).ceil() as i32, true);
    }
    (-1, false)
}

/// Volta em que o jogador está e total da prova, com os sentinelas do SDK traduzidos.
///
/// `SessionLapsTotal` vale [`LAPS_REMAIN_SENTINELA`] em prova por TEMPO, e o contrato do
/// campo `voltas_totais` promete 0 nesse caso. Sem a tradução o contexto do engenheiro
/// recebia "Volta: 12 de 32767" e o rádio anunciava o sentinela como se fosse a prova.
///
/// O teto na volta é a mesma regra do cabeçalho da torre: passada a bandeirada o
/// `lap_completed` continua subindo com quem segue girando na volta de desaceleração, e
/// uma prova de 8 voltas não tem uma nona para anunciar.
pub(super) fn volta_e_total(lap_completed: i32, laps_total: i32) -> (i32, i32) {
    let total = if laps_total > 0 && laps_total < LAPS_REMAIN_SENTINELA {
        laps_total
    } else {
        0
    };
    let volta = lap_completed.max(0);
    let volta = if total > 0 { volta.min(total) } else { volta };
    (volta, total)
}

/// Voltas até encostar, dado o gap e a diferença de ritmo. `delta_ritmo_s` é o MEU ritmo
/// menos o dele, então negativo significa que eu sou mais rápido.
///
/// Devolve -1 quando a distância está aumentando ou estável: o alcance nunca acontece, e
/// inventar um número gigante seria pior que admitir que não há resposta.
pub(super) fn voltas_para_alcancar(gap_s: f64, delta_ritmo_s: f64) -> f64 {
    if gap_s <= 0.0 || !gap_s.is_finite() || !delta_ritmo_s.is_finite() || delta_ritmo_s >= 0.0 {
        return -1.0;
    }
    gap_s / -delta_ritmo_s
}

/// Variação do gap por VOLTA a partir das amostras (tempo, gap) de um mesmo adversário.
///
/// Mede-se por volta e não por segundo porque é assim que um piloto pensa a corrida. A
/// conta é a inclinação bruta entre a primeira e a última amostra da janela — regressão
/// completa não compraria nada aqui, já que o ruído dominante (freada, tráfego) não é
/// gaussiano e some sozinho numa janela longa.
pub(super) fn tendencia_por_volta(amostras: &[(f64, f64)], ritmo_s: f64) -> f64 {
    if amostras.len() < 2 || ritmo_s <= 0.0 {
        return 0.0;
    }
    let (t0, g0) = amostras[0];
    let (t1, g1) = amostras[amostras.len() - 1];
    let janela = t1 - t0;
    if janela < TENDENCIA_JANELA_MIN_S {
        return 0.0;
    }
    ((g1 - g0) / janela) * ritmo_s
}

/// A bandeira mais grave ativa, em PT. Ordem de gravidade, não de bit: uma preta com uma
/// amarela ao mesmo tempo é uma preta, e anunciar a amarela nesse caso seria enterrar a
/// informação que exige ação.
pub(super) fn rotulo_bandeira(flags: u32) -> String {
    // A PRETA NÃO ENTRA. No Loop ela é o mecanismo de dano do próprio jogo — o
    // `car::breakdown` manda `!black #N <segundos>` para simular quebra de peça —, então
    // rotulá-la aqui faria o único consumidor deste campo (o engenheiro, em
    // `engenheiro::fatos`/`fala`) anunciar punição onde houve motor quebrado. Pior: por
    // ser a segunda na ordem de gravidade, ela ESCONDIA uma amarela de verdade que
    // estivesse ativa junto. O sim/não continua em `bandeira_preta`, para quem
    // legitimamente precise do bit.
    let rotulo = if flags & FLAG_DISQUALIFY != 0 {
        "Desclassificado"
    } else if flags & FLAG_REPAIR != 0 {
        "Reparo obrigatório"
    } else if flags & FLAG_RED != 0 {
        "Bandeira vermelha"
    } else if flags & (FLAG_CAUTION | FLAG_CAUTION_WAVING | FLAG_YELLOW | FLAG_YELLOW_WAVING) != 0 {
        "Bandeira amarela"
    } else if flags & FLAG_BLUE != 0 {
        "Bandeira azul"
    } else if flags & FLAG_CHECKERED != 0 {
        "Bandeirada"
    } else if flags & FLAG_WHITE != 0 {
        "Última volta"
    } else {
        ""
    };
    rotulo.to_string()
}

/// Há quantas voltas o carro está com este jogo de pneus.
///
/// A conta sai do dwell na caixa, e não de nenhum canal de desgaste — o iRacing não expõe
/// desgaste, temperatura nem pressão de pneu. O que ele expõe é quanto tempo cada carro
/// ficou **parado** no box, e trocar pneu é um bloco fixo de ~21,5 s por cima do
/// abastecimento. Acima de [`TIRE_CHANGE_MIN_SECS`] a parada trocou pneu; abaixo, só
/// abasteceu.
///
/// Sem parada com troca, os pneus são os da largada e a idade é a corrida inteira — que é
/// a resposta certa, não uma ausência.
///
/// Usa o limiar ABSOLUTO e não o relativo do `tire_strategy`. Ao vivo o relativo é pior:
/// ele acha o corte olhando o histograma de paradas do pelotão, e no início da corrida,
/// quando quase ninguém parou, o histograma ainda não tem as duas faixas que ele procura.
pub(super) fn idade_do_pneu(
    paradas: &[crate::iracing_sdk::tire_strategy::PitStop],
    car_idx: i32,
    volta_atual: i32,
) -> i32 {
    use crate::iracing_sdk::tire_strategy::TIRE_CHANGE_MIN_SECS;
    if volta_atual < 0 {
        return -1;
    }
    let ultima_troca = paradas
        .iter()
        .filter(|p| p.car_idx == car_idx && p.stationary_secs >= TIRE_CHANGE_MIN_SECS)
        .map(|p| p.lap)
        .max();
    match ultima_troca {
        Some(volta) => (volta_atual - volta).max(0),
        None => volta_atual.max(0),
    }
}

/// Saldo de combustível em voltas: o que o tanque dá menos o que a prova ainda pede.
///
/// O desconhecido é `NaN`, e **não** `-1` como nos outros campos deste módulo. A diferença
/// é obrigatória aqui: este é o único valor cujo negativo é uma resposta legítima — um
/// saldo de `-1` significa "falta combustível para uma volta", que é o oposto exato de
/// "não sei", e é a informação que manda o piloto ao box. Com o sentinela numérico os dois
/// casos seriam o mesmo número.
pub(super) fn saldo_combustivel(autonomia: f64, restantes: i32) -> f64 {
    if autonomia < 0.0 || restantes < 0 {
        return f64::NAN;
    }
    autonomia - restantes as f64
}

// ─── Montagem a partir do monitor ────────────────────────────────────────────

impl RaceMonitor {
    /// Guarda a telemetria completa para o estado ao vivo, a ~4 Hz.
    ///
    /// O sampler roda a 60 Hz porque o spotter e a pontuação de batida precisam desse
    /// ritmo; o estado narrado, não. Clonar o vetor de carros 60 vezes por segundo seria
    /// pagar caro por uma resolução que ninguém consome — a fala mais rápida do engenheiro
    /// leva meio segundo.
    pub(super) fn guardar_estado_agora(&mut self, t: &IracingTelemetry) {
        if t.session_time - self.estado_ultimo_refresh < ESTADO_REFRESH_SECS {
            return;
        }
        self.estado_ultimo_refresh = t.session_time;

        // A amostra de gap entra ANTES do clone porque é dela que sai a tendência, e ela
        // depende do estado anterior — não do atual.
        if let Some(eu) = t.cars.iter().find(|c| c.idx == t.player_car_idx) {
            let volta_ref = self.volta_referencia(t, eu);
            // `position >= 1` no candidato: com o jogador em P1 a conta dá 0, que é o
            // sentinela de "sem posição atribuída" do iRacing, e casaria com qualquer
            // carro na garagem ou fora do mundo. Ver a nota longa em `historico.rs`.
            let frente = t
                .cars
                .iter()
                .find(|c| c.position >= 1 && c.position == eu.position - 1);
            let atras = t.cars.iter().find(|c| c.position == eu.position + 1);
            self.gap_hist.push(AmostraGap {
                t: t.session_time,
                idx_frente: frente.map(|c| c.idx).unwrap_or(-1),
                gap_frente: frente
                    .map(|c| gap_circular(eu.est_time, c.est_time, volta_ref))
                    .unwrap_or(-1.0),
                idx_atras: atras.map(|c| c.idx).unwrap_or(-1),
                gap_atras: atras
                    .map(|c| gap_circular(c.est_time, eu.est_time, volta_ref))
                    .unwrap_or(-1.0),
            });
            if self.gap_hist.len() > GAP_HIST_MAX {
                self.gap_hist.remove(0);
            }
        }

        self.ultima_telemetria = Some(t.clone());
    }

    /// Tempo de volta de referência para fechar o círculo do gap: a melhor do jogador, a
    /// última se não houver melhor, e a melhor do líder como último recurso.
    ///
    /// O último recurso importa mais do que parece: nas primeiras voltas o jogador ainda
    /// não tem volta válida nenhuma, e é exatamente aí — grid embolado, todo mundo junto —
    /// que a pergunta "quem está do meu lado" mais aparece.
    pub(super) fn volta_referencia(&self, t: &IracingTelemetry, eu: &CarSnapshot) -> f64 {
        if eu.best_lap_time > 0.0 {
            return eu.best_lap_time;
        }
        if eu.last_lap_time > 0.0 {
            return eu.last_lap_time;
        }
        t.cars
            .iter()
            .filter(|c| c.best_lap_time > 0.0)
            .map(|c| c.best_lap_time)
            .fold(f64::INFINITY, f64::min)
            .to_owned()
            .pipe_finito()
    }

    /// As amostras (tempo, gap) de um adversário específico, do lado pedido.
    fn amostras_de(&self, idx: i32, frente: bool) -> Vec<(f64, f64)> {
        self.gap_hist
            .iter()
            .filter(|a| {
                let (i, g) = if frente {
                    (a.idx_frente, a.gap_frente)
                } else {
                    (a.idx_atras, a.gap_atras)
                };
                i == idx && g >= 0.0
            })
            .map(|a| {
                let g = if frente { a.gap_frente } else { a.gap_atras };
                (a.t, g)
            })
            .collect()
    }

    fn monta_vizinho(
        &self,
        outro: &CarSnapshot,
        eu: &CarSnapshot,
        volta_ref: f64,
        meu_ritmo: f64,
        frente: bool,
    ) -> Vizinho {
        let gap_s = if frente {
            gap_circular(eu.est_time, outro.est_time, volta_ref)
        } else {
            gap_circular(outro.est_time, eu.est_time, volta_ref)
        };
        let ritmo_s = if outro.last_lap_time > 0.0 {
            outro.last_lap_time
        } else {
            -1.0
        };
        // O delta só existe com os DOIS ritmos. Com um lado faltando, um zero aqui seria
        // lido como "ritmo idêntico", que é uma afirmação — e uma afirmação falsa.
        let delta_ritmo_s = if meu_ritmo > 0.0 && ritmo_s > 0.0 {
            meu_ritmo - ritmo_s
        } else {
            f64::NAN
        };
        let amostras = self.amostras_de(outro.idx, frente);
        Vizinho {
            idx: outro.idx,
            nome: self
                .driver_names
                .iter()
                .find(|(i, _)| *i == outro.idx)
                .map(|(_, n)| n.clone())
                .unwrap_or_default(),
            numero: self.car_number[outro.idx.clamp(0, 63) as usize],
            gap_s,
            ritmo_s,
            delta_ritmo_s,
            voltas_para_alcancar: voltas_para_alcancar(gap_s, delta_ritmo_s),
            tendencia_s_por_volta: tendencia_por_volta(&amostras, meu_ritmo.max(volta_ref)),
            no_box: outro.on_pit_road,
            volta_a_parte: (outro.lap_completed - eu.lap_completed).abs() >= 1,
            composto: outro.tire_compound,
            // A idade conta as voltas DELE, não as minhas: quem está uma volta à parte
            // rodou um número diferente de voltas desde a própria parada.
            pneu_voltas: idade_do_pneu(&self.history.pit_stops, outro.idx, outro.lap_completed),
        }
    }

    /// Monta o retrato. Sem telemetria guardada ainda, devolve um estado desconectado —
    /// nunca um erro: quem pergunta durante a tela de carregamento merece um "não sei",
    /// não uma exceção.
    /// A ORDEM da corrida agora: `(posição geral, número do carro)`, do líder para trás.
    ///
    /// Fica FORA de [`EstadoAgora`] de propósito. São até 64 pares que interessam a um
    /// consumidor só — a projeção do campeonato —, e o `EstadoAgora` é serializado inteiro
    /// para a interface a cada pergunta ao rádio.
    ///
    /// O pace car sai da lista: ele tem posição no `CarIdxPosition` e não corre. Carro com
    /// posição zero também — é carro que o iRacing ainda não classificou, e chutar que ele
    /// está em último produziria uma projeção errada com cara de certa.
    pub(super) fn ordem_agora(&self) -> Vec<(i32, i32)> {
        let Some(t) = self.ultima_telemetria.as_ref() else {
            return Vec::new();
        };
        let mut v: Vec<(i32, i32)> = t
            .cars
            .iter()
            .filter(|c| c.position >= 1)
            .filter(|c| !self.car_is_pace[c.idx.clamp(0, 63) as usize])
            .map(|c| (c.position, self.car_number[c.idx.clamp(0, 63) as usize]))
            .collect();
        v.sort_unstable();
        v
    }

    pub(super) fn montar_estado_agora(&self) -> EstadoAgora {
        let Some(t) = self.ultima_telemetria.as_ref() else {
            return EstadoAgora::vazio(self.connected);
        };
        let flags = t.session_flags as u32;
        let eu = t.cars.iter().find(|c| c.idx == t.player_car_idx);

        let meu_ritmo = if t.last_lap_time > 0.0 {
            t.last_lap_time
        } else {
            -1.0
        };
        let melhor = eu.map(|c| c.best_lap_time).unwrap_or(-1.0);
        // A melhor DA CORRIDA, com dono. Percorre `cars` uma vez guardando o menor positivo —
        // `fold` com `f64::min` daria o valor mas perderia de quem é, e o nome é metade do que
        // o rádio anuncia.
        let (melhor_corrida, melhor_corrida_idx) = t
            .cars
            .iter()
            .filter(|c| c.best_lap_time > 0.0)
            .fold((-1.0f64, -1i32), |(melhor, idx), c| {
                if melhor < 0.0 || c.best_lap_time < melhor {
                    (c.best_lap_time, c.idx)
                } else {
                    (melhor, idx)
                }
            });
        let volta_ref = eu.map(|c| self.volta_referencia(t, c)).unwrap_or(-1.0);
        // O ritmo que estima as voltas restantes é a MELHOR volta, não a última: a última
        // pode ter sido uma volta de box ou uma amarela, e uma estimativa de fim de prova
        // não pode balançar porque o piloto pegou tráfego uma vez.
        let ritmo_estimativa = if melhor > 0.0 { melhor } else { meu_ritmo };
        let (voltas_restantes_n, estimadas) = voltas_restantes(
            t.session_laps_remain_ex,
            t.session_time_remain,
            ritmo_estimativa,
        );

        let (frente, atras) = match eu {
            Some(eu) if eu.position >= 1 => (
                // Mesma guarda de sentinela da amostra de gap: `eu.position >= 1` acima
                // protege o jogador, não o candidato — em P1 a conta dá 0.
                t.cars
                    .iter()
                    .find(|c| c.position >= 1 && c.position == eu.position - 1)
                    .map(|c| self.monta_vizinho(c, eu, volta_ref, meu_ritmo, true)),
                t.cars
                    .iter()
                    .find(|c| c.position == eu.position + 1)
                    .map(|c| self.monta_vizinho(c, eu, volta_ref, meu_ritmo, false)),
            ),
            _ => (None, None),
        };

        let (consumo, autonomia) = self.combustivel_agora(t);
        let saldo = saldo_combustivel(autonomia, voltas_restantes_n);
        let (volta, voltas_totais) = volta_e_total(t.lap_completed, t.session_laps_total);

        EstadoAgora {
            conectado: self.connected,
            em_corrida: t.session_state == STATE_RACING && self.in_race_session(t),
            tipo_sessao: self.tipo_da_sessao(t),
            verde: self.live_is_green,
            em_formacao: self.in_race_session(t) && !self.verde_da_tentativa,

            posicao: t.position,
            posicao_classe: eu.map(|c| c.class_position).unwrap_or(0),
            total_carros: self.live_cars_count,

            volta,
            voltas_totais,
            voltas_restantes: voltas_restantes_n,
            voltas_restantes_estimadas: estimadas,
            tempo_restante_s: if t.session_time_remain > 0.0 {
                t.session_time_remain
            } else {
                -1.0
            },

            frente,
            atras,

            ultima_volta_s: meu_ritmo,
            melhor_volta_s: melhor,
            melhor_da_corrida_s: melhor_corrida,
            melhor_da_corrida_idx: melhor_corrida_idx,
            melhor_da_corrida_e_minha: melhor_corrida_idx >= 0
                && eu.map(|c| c.idx) == Some(melhor_corrida_idx),
            delta_melhor_s: if meu_ritmo > 0.0 && melhor > 0.0 {
                meu_ritmo - melhor
            } else {
                f64::NAN
            },

            combustivel_l: t.fuel_level,
            consumo_por_volta_l: consumo,
            autonomia_voltas: autonomia,
            saldo_combustivel_voltas: saldo,

            reparo_obrigatorio_s: t.pit_repair_needed,
            reparo_opcional_s: t.pit_opt_repair_needed,
            reparos_rapidos_usados: eu.map(|c| c.fast_repairs_used).unwrap_or(0),

            bandeira: rotulo_bandeira(flags),
            bandeira_preta: flags & FLAG_BLACK != 0,
            desclassificado: flags & FLAG_DISQUALIFY != 0,
            incidentes: t.incident_count,

            pista_molhada: t.track_is_wet(),
            umidade_pista: t.track_wetness,
            chuva_agora: t.precipitation,
            declarada_molhada: t.weather_declared_wet,
            temp_pista_c: t.track_temp,
            temp_ar_c: t.air_temp,
            boxes_abertos: t.pits_open,

            meu_composto: eu.map(|c| c.tire_compound).unwrap_or(-1),
            meu_pneu_voltas: idade_do_pneu(
                &self.history.pit_stops,
                t.player_car_idx,
                t.lap_completed,
            ),
            compostos_no_grid: histograma_compostos(&t.cars),
        }
    }

    /// Consumo por volta e autonomia, do histórico de voltas com tanque captado.
    ///
    /// A conta é a mesma do `telemetry_analysis::combustivel` e de propósito: são as duas
    /// pontas do mesmo dado, e responder uma coisa ao vivo e outra no pós-corrida seria a
    /// pior espécie de bug — o jogador vê os dois números e nenhum dos dois está errado
    /// o bastante para denunciar o outro.
    fn combustivel_agora(&self, t: &IracingTelemetry) -> (f64, f64) {
        let mut pts: Vec<(f64, f64)> = self
            .history
            .player_laps
            .iter()
            .filter(|l| l.fuel_remaining >= 0.0)
            .map(|l| (l.lap as f64, l.fuel_remaining))
            .collect();
        if pts.len() < 2 {
            return (-1.0, -1.0);
        }
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (lap0, fuel0) = pts[0];
        let (lap1, fuel1) = *pts.last().unwrap();
        let voltas = lap1 - lap0;
        let queimado = fuel0 - fuel1;
        if voltas < 1.0 || queimado <= 0.0 {
            return (-1.0, -1.0);
        }
        let consumo = queimado / voltas;
        // A autonomia usa o tanque AO VIVO, não o da última volta cruzada: entre uma volta
        // e outra cabe um reabastecimento inteiro, e responder com o número velho logo
        // depois de um pit é responder o oposto da verdade.
        let restante = if t.fuel_level > 0.0 {
            t.fuel_level
        } else {
            fuel1
        };
        (consumo, restante / consumo)
    }
}

/// Quantos carros em cada índice de composto, ordenado por índice. Só carros com composto
/// conhecido entram — um `-1` no histograma seria uma categoria fantasma.
fn histograma_compostos(cars: &[CarSnapshot]) -> Vec<(i32, i32)> {
    let mut contagem: Vec<(i32, i32)> = Vec::new();
    for c in cars.iter().filter(|c| c.tire_compound >= 0) {
        match contagem.iter_mut().find(|(k, _)| *k == c.tire_compound) {
            Some((_, n)) => *n += 1,
            None => contagem.push((c.tire_compound, 1)),
        }
    }
    contagem.sort_by_key(|(k, _)| *k);
    contagem
}

impl EstadoAgora {
    /// O retrato de quem não tem retrato. Tudo em "desconhecido" explícito (-1 / NaN /
    /// vazio) e nada em zero, porque zero é um valor plausível e mentiria.
    fn vazio(conectado: bool) -> Self {
        Self {
            conectado,
            em_corrida: false,
            // Sem telemetria não se sabe que sessão é. `treino` é o resto por definição, e é
            // o valor que menos autoriza a fala: nada de "que corrida" sobre o desconhecido.
            tipo_sessao: "treino",
            verde: false,
            em_formacao: false,
            posicao: 0,
            posicao_classe: 0,
            total_carros: 0,
            volta: 0,
            voltas_totais: 0,
            voltas_restantes: -1,
            voltas_restantes_estimadas: false,
            tempo_restante_s: -1.0,
            frente: None,
            atras: None,
            ultima_volta_s: -1.0,
            melhor_volta_s: -1.0,
            delta_melhor_s: f64::NAN,
            melhor_da_corrida_s: -1.0,
            melhor_da_corrida_idx: -1,
            melhor_da_corrida_e_minha: false,
            combustivel_l: -1.0,
            consumo_por_volta_l: -1.0,
            autonomia_voltas: -1.0,
            saldo_combustivel_voltas: f64::NAN,
            reparo_obrigatorio_s: 0.0,
            reparo_opcional_s: 0.0,
            reparos_rapidos_usados: 0,
            bandeira: String::new(),
            bandeira_preta: false,
            desclassificado: false,
            incidentes: 0,
            pista_molhada: false,
            umidade_pista: 0,
            chuva_agora: 0.0,
            declarada_molhada: false,
            temp_pista_c: 0.0,
            temp_ar_c: 0.0,
            boxes_abertos: false,
            meu_composto: -1,
            meu_pneu_voltas: -1,
            compostos_no_grid: Vec::new(),
        }
    }
}

/// `f64::INFINITY` vira -1. O `fold` com `min` devolve infinito quando não há nenhum
/// candidato, e infinito escapando para um cálculo de gap produz `NaN` lá adiante — longe
/// o bastante da origem para custar caro de achar.
trait PipeFinito {
    fn pipe_finito(self) -> f64;
}

impl PipeFinito for f64 {
    fn pipe_finito(self) -> f64 {
        if self.is_finite() {
            self
        } else {
            -1.0
        }
    }
}
