//! Os números do monitor de corrida, separados por DOMÍNIO e com a procedência de cada um.
//!
//! Eles viviam num bloco só no topo do [`super`], oitenta e cinco linhas em que batida,
//! race control, cluster de box, quebra na classificação e teto de memória se misturavam.
//! O problema não era o tamanho: era não dar para saber, olhando o número que você está
//! prestes a mexer, se ele saiu de uma medição na pista ou de um chute que funcionou.
//!
//! Daí a ETIQUETA no fim de cada comentário. Ela responde uma pergunta só — *o que
//! acontece se eu mudar isto?*
//!
//! | Etiqueta | O que significa | O que fazer antes de mexer |
//! |---|---|---|
//! | `[SDK]` | Valor do contrato do simulador (enum, bitmask, limite do buffer). Não é calibração: mudar é reinterpretar o dado. | Conferir contra a documentação do SDK / o inventário de canais. |
//! | `[MEDIDO]` | Saiu de uma medição registrada, com data e circunstância no comentário. | Repetir a medição. O comentário diz qual. |
//! | `[ORÇAMENTO]` | Escolha de engenharia sobre custo (CPU por tique, memória guardada). Não descreve a corrida, descreve a máquina. | Medir o custo, não o comportamento. |
//! | `[A OLHO]` | Julgamento sem medição escrita. Funciona, ninguém mediu. | É o mais barato de mexer e o mais arriscado de confiar. |
//!
//! `[A OLHO]` não é acusação — boa parte destes números vem de assistir à corrida e não
//! tinha como sair de outro lugar. O que a etiqueta compra é honestidade: quem for
//! recalibrar sabe de saída onde há evidência para contrariar e onde não há.

use super::Severidade;

// ─── Contrato do SDK: estados, superfícies e bandeiras ───────────────────────
// Nada aqui é calibração. São os valores que o iRacing publica; um número trocado não
// desafina o comportamento, faz o monitor ler outra coisa.

/// `SessionState` = corrida em andamento. `[SDK]`
pub(super) const STATE_RACING: i32 = 4;
/// `SessionState` = bandeirada dada. `[SDK]`
pub(super) const STATE_CHECKERED: i32 = 5;
/// `CarIdxTrackSurface`: carro fora do mundo (garagem, guincho, ainda não entrou). `[SDK]`
pub(super) const SURFACE_NOT_IN_WORLD: i32 = -1;
/// `CarIdxTrackSurface`: fora da pista. `[SDK]`
pub(super) const SURFACE_OFF_TRACK: i32 = 0;
/// `CarIdxTrackSurface`: parado na própria caixa. `[SDK]`
pub(super) const SURFACE_IN_PIT_STALL: i32 = 1;
/// `CarIdxTrackSurface`: no asfalto. `[SDK]`
pub(super) const SURFACE_ON_TRACK: i32 = 3;

/// Bits de `SessionFlags`/`CarIdxSessionFlags`. `[SDK]`
pub(super) const FLAG_CHECKERED: u32 = 0x0000_0001;
/// `[SDK]`
pub(super) const FLAG_WHITE: u32 = 0x0000_0002;
/// `[SDK]`
pub(super) const FLAG_YELLOW: u32 = 0x0000_0008;
/// `[SDK]`
pub(super) const FLAG_RED: u32 = 0x0000_0010;
/// `[SDK]`
pub(super) const FLAG_BLUE: u32 = 0x0000_0020;
/// `[SDK]`
pub(super) const FLAG_YELLOW_WAVING: u32 = 0x0000_0100;
/// `[SDK]`
pub(super) const FLAG_CAUTION: u32 = 0x0000_4000;
/// `[SDK]`
pub(super) const FLAG_CAUTION_WAVING: u32 = 0x0000_8000;
/// `[SDK]`
pub(super) const FLAG_BLACK: u32 = 0x0001_0000;
/// `[SDK]`
pub(super) const FLAG_DISQUALIFY: u32 = 0x0002_0000;
/// Meatball — reparo obrigatório declarado pelo sim. `[SDK]`
pub(super) const FLAG_REPAIR: u32 = 0x0010_0000;

/// Aceleração da gravidade, para converter o acelerômetro do SDK em G. `[SDK]`
pub(super) const GRAVITY: f64 = 9.81;

// ─── Continuidade do relógio da sessão: replay, rebobinada e reinício ────────
// A família de números que separa "o mundo andou" de "o mundo pulou". Errar para baixo
// faz o monitor zerar estado a toda hora; errar para cima faz uma rebobinada passar por
// corrida.

/// Queda de `SessionTime` tolerada antes de considerar descontinuidade. `[A OLHO]`
pub(super) const SESSION_TIME_DROP_TOLERANCE: f64 = 1.0;
/// Salto de `SessionTime` (rebobinar/avançar replay) acima do qual zeramos os
/// relógios da IA para não interpretar como "parado"/restart. `[A OLHO]`
pub(super) const REPLAY_JUMP_SECS: f64 = 2.0;
/// Um reinício de verdade leva o `SessionTime` de volta para perto de ZERO (o
/// relógio do jogo zera, os carros voltam ao grid). Uma simples queda do tempo
/// (ex.: ao entrar no replay) NÃO é reinício. `[A OLHO]`
pub(super) const RESTART_RESET_MAX_SECS: f64 = 10.0;

// ─── Cadência do amostrador e dos consumidores ───────────────────────────────
// Domínio de CUSTO, não de corrida. Cada número aqui compra resolução com CPU por tique;
// nenhum deles descreve o que acontece na pista.

/// Período do sampler com o iRacing CONECTADO (~60 Hz) — para não perder picos de
/// impacto. `[ORÇAMENTO]`
pub(super) const SAMPLER_PERIOD_MS: u64 = 16;
/// Período OCIOSO (iRacing fechado): só espia a conexão devagar, custo ~zero.
/// `[ORÇAMENTO]`
pub(super) const SAMPLER_IDLE_PERIOD_MS: u64 = 1000;
/// De quanto em quanto tempo o [`super::EstadoAgora`] recolhe uma amostra. O sampler roda
/// a 60 Hz para o spotter e a pontuação de batida; o estado narrado não precisa disso — a
/// fala mais curta do engenheiro leva meio segundo, e clonar o vetor de carros 60 vezes
/// por segundo pagaria caro por uma resolução que ninguém consome. `[ORÇAMENTO]`
pub(super) const ESTADO_REFRESH_SECS: f64 = 0.25;
/// A cada quantos ticks (~60 Hz) recarregar a classificação de carros do YAML — ela muda
/// raramente. `[ORÇAMENTO]`
pub(super) const YAML_REFRESH_TICKS: u64 = 180;
/// A cada quantos ticks conferir se o PORTÃO DO RÁDIO virou (~8 Hz). Não precisa de 60 Hz:
/// o que se mede aqui é uma transição que dura segundos, e a fala mais rápida do rádio leva
/// mais de um décimo para começar a sair. Mais fino que isso só pagaria
/// `montar_estado_agora` mais vezes para descrever a mesma virada. `[ORÇAMENTO]`
pub(super) const TICKS_PORTAO: u64 = 8;
/// Intervalo (s) entre amostras da batalha do jogador (à frente/atrás). ~1 Hz
/// captura a briga sem peso (1 h de corrida ≈ 7200 pontos minúsculos a 2 Hz).
/// `[ORÇAMENTO]`
pub(super) const NEIGHBOR_SAMPLE_SECS: f64 = 0.5;
/// Intervalo mínimo (segundos) entre snapshots do trace disparados por TROCA de
/// posição. Evita que uma disputa lado-a-lado (posições trocando tick a tick) gere
/// um snapshot por frame. A virada de volta do líder ignora este throttle. `[ORÇAMENTO]`
pub(super) const MIN_TRACE_EVENT_GAP_SECS: f64 = 0.25;

// ─── Tetos de memória ────────────────────────────────────────────────────────
// Guardas contra crescimento ilimitado numa sessão longa. Uma corrida real fica bem
// abaixo de todos eles; se algum estiver sendo atingido em uso normal, o teto não é o
// problema — o produtor é.

/// Quantos eventos manter no log (os mais antigos saem). `[ORÇAMENTO]`
pub(super) const MAX_EVENTS: usize = 60;
/// Teto de voltas guardadas no histórico (race trace). `[ORÇAMENTO]`
pub(super) const MAX_HISTORY_LAPS: usize = 600;
/// Teto de amostras da batalha guardadas (≈ 41 min a 2 Hz). `[ORÇAMENTO]`
pub(super) const MAX_TRACK_POINTS: usize = 5000;
/// Limite de voltas-por-carro guardadas (todos os carros × várias voltas). `[ORÇAMENTO]`
pub(super) const MAX_CAR_LAPS: usize = 4000;
/// Limite de paradas de box guardadas (todos os carros × várias paradas). `[ORÇAMENTO]`
pub(super) const MAX_PIT_STOPS: usize = 600;

// ─── Eventos de IA: carro parado, fora da pista, DNF provável ────────────────

/// Variação mínima de `LapDistPct` para considerar que um carro "andou". `[A OLHO]`
pub(super) const AI_PROGRESS_EPS: f64 = 0.0015;
/// Segundos sem progresso para sinalizar carro parado. `[A OLHO]`
pub(super) const AI_STOPPED_SECS: f64 = 10.0;
/// Segundos sem progresso para sinalizar provável DNF de IA. `[A OLHO]`
pub(super) const AI_DNF_SECS: f64 = 25.0;

// ─── Race control: recomendação de bandeira amarela ──────────────────────────
// Toda esta seção é julgamento. Ela não tem harness: a checagem de acerto seria cruzar a
// nossa recomendação com a amarela que o sim de fato deu, e o que existe hoje
// ([`YELLOW_CONFIRM_WINDOW_SECS`]) só correlaciona, não pontua. Enquanto isso não
// existir, estes números são hipóteses que ninguém contrariou.

/// Tempo mínimo com progresso ZERADO para um carro parado virar candidato a
/// bandeira. 2 s sem progresso já é claramente anormal vs. o ritmo de corrida —
/// o carro realmente parou, então já consideramos quem vem atrás. `[A OLHO]`
pub(super) const YELLOW_MIN_STOP_SECS: f64 = 2.0;
/// Janela de pista ATRÁS do carro parado (fração da volta) dentro da qual outro
/// carro é considerado "chegando" — risco de colisão. `[A OLHO]`
pub(super) const DANGER_GAP: f64 = 0.10;
/// Quantos carros chegando configuram perigo (>= isto → recomenda bandeira). `[A OLHO]`
pub(super) const DANGER_CARS_MIN: usize = 1;
/// Janela para correlacionar a amarela do `SessionFlags` com nossa recomendação.
/// `[A OLHO]`
pub(super) const YELLOW_CONFIRM_WINDOW_SECS: f64 = 12.0;
/// Cooldown após o verde: nos primeiros segundos da corrida o grid está
/// engarrafado (carros rápidos presos atrás dos lentos), então não decidimos
/// bandeira até o pelotão abrir. `[A OLHO]`
pub(super) const START_GRACE_SECS: f64 = 8.0;

// ─── Race control: cluster de box = acidente coletivo ────────────────────────
// Carros que reduziram muito o ritmo e foram ao box. Mesma ressalva da seção acima:
// sem medição por trás.

/// Janela de medição do ritmo (pace) de cada carro. `[A OLHO]`
pub(super) const PACE_WINDOW_SECS: f64 = 1.5;
/// Abaixo desta fração do ritmo do líder, o carro está "lento" (danificado). `[A OLHO]`
pub(super) const SLOW_PACE_FRACTION: f64 = 0.4;
/// Acima desta fração, consideramos que o carro JÁ atingiu ritmo de corrida.
/// "Lento" só conta depois disso — senão a largada (todos acelerando) vira
/// falso "carro lento". `[A OLHO]`
pub(super) const RACING_PACE_FRACTION: f64 = 0.6;
/// Pit logo após ficar lento NA PISTA conta como pit de incidente. `[A OLHO]`
pub(super) const SLOW_PIT_WINDOW_SECS: f64 = 6.0;
/// Janela para suspeitar de acidente coletivo por pits agrupados. `[A OLHO]`
pub(super) const PIT_CLUSTER_WINDOW_SECS: f64 = 15.0;
/// Mínimo de pits de incidente na janela acima. `[A OLHO]`
pub(super) const PIT_CLUSTER_MIN: usize = 2;
/// Não realertar sobre o mesmo cluster por este tempo. `[A OLHO]`
pub(super) const PIT_CLUSTER_COOLDOWN_SECS: f64 = 30.0;

// ─── Race control: setores e acidente coletivo ───────────────────────────────

/// Em quantos setores a pista é dividida para agrupar carros em apuros. `[A OLHO]`
pub(super) const NUM_SECTORS: i32 = 20;
/// Carros em apuros no mesmo setor (ou vizinhos) que configuram acidente coletivo.
/// `[A OLHO]`
pub(super) const COLLECTIVE_MIN: usize = 2;
/// Cooldown do alerta de acidente coletivo por setor. `[A OLHO]`
pub(super) const COLLECTIVE_COOLDOWN_SECS: f64 = 30.0;

// ─── Detector de parada de box ───────────────────────────────────────────────

/// Dwell mínimo (segundos) para uma passagem pela caixa contar como parada real.
/// `CarIdxTrackSurface == InPitStall` pode piscar por 1 frame (carro cruzando a
/// zona da caixa, ou leitura transiente do SDK) e gerar "pits" de ~0 s. Uma parada
/// de verdade sempre fica estacionada por vários segundos. `[MEDIDO]` — o pisco de
/// 1 frame foi observado na captura de corrida; o valor do teto é folga sobre ele.
pub(super) const MIN_PIT_STALL_DWELL_SECS: f64 = 2.5;

// ─── Pontuação de batida ─────────────────────────────────────────────────────
// ATENÇÃO à procedência. O cabeçalho antigo dizia "calibradas nos testes"; conferido em
// 11/08/2026, NENHUM destes pesos é asseverado pela suíte. Os testes exercitam o
// pipeline (um contato pontua, uma rodada sem impacto não pontua, o pico sobrevive à
// bandeirada) e usam `peak_crash_score` com valores literais — mudar qualquer peso
// abaixo muda o comportamento no jogo sem quebrar teste nenhum.
//
// A forma da conta é: cada componente (G, velocidade perdida, guinada, rotação) vira
// pontos por `(valor - limiar) * taxa`, saturado no teto, e a soma cai nas faixas
// `SEV_*`. Limiar = a partir de onde conta; taxa = quanto vale cada unidade; teto = onde
// para de crescer, para um impacto enorme não estourar a escala.

/// Janela para fundir dois picos na MESMA batida (o carro bate, gira e bate de novo).
/// `[A OLHO]`
pub(super) const MERGE_WINDOW_SECS: f64 = 10.0;

/// Pontos por incidente de 1x do sim. `[A OLHO]`
pub(super) const INCIDENT_1X: f64 = 10.0;
/// Pontos por incidente de 2x. `[A OLHO]`
pub(super) const INCIDENT_2X: f64 = 20.0;
/// Pontos por incidente de 4x. `[A OLHO]`
pub(super) const INCIDENT_4X: f64 = 35.0;

/// G a partir do qual o impacto começa a contar. `[A OLHO]`
pub(super) const G_THRESHOLD: f64 = 3.0;
/// Pontos por G acima do limiar. `[A OLHO]`
pub(super) const G_RATE: f64 = 1.0;
/// Teto da componente de G. `[A OLHO]`
pub(super) const G_CAP: f64 = 30.0;

/// Velocidade perdida (m/s) a partir da qual conta. `[A OLHO]`
pub(super) const SPEED_LOST_THRESHOLD: f64 = 3.0;
/// Pontos por m/s perdido acima do limiar. `[A OLHO]`
pub(super) const SPEED_LOST_RATE: f64 = 4.0;
/// Teto da componente de velocidade perdida. `[A OLHO]`
pub(super) const SPEED_LOST_CAP: f64 = 160.0;

/// Guinada (rad/s) a partir da qual conta. `[A OLHO]`
pub(super) const YAW_THRESHOLD: f64 = 3.0;
/// Pontos por rad/s de guinada acima do limiar. `[A OLHO]`
pub(super) const YAW_RATE_W: f64 = 10.0;
/// Teto da componente de guinada. `[A OLHO]`
pub(super) const YAW_CAP: f64 = 18.0;

/// Rotação combinada (pitch/roll) a partir da qual conta. `[A OLHO]`
pub(super) const ROT_THRESHOLD: f64 = 3.5;
/// Pontos por unidade de rotação acima do limiar. `[A OLHO]`
pub(super) const ROT_RATE_W: f64 = 10.0;
/// Teto da componente de rotação. `[A OLHO]`
pub(super) const ROT_CAP: f64 = 15.0;

/// Pontos por estar sendo guinchado (`PlayerCarTowTime`). `[A OLHO]`
pub(super) const TOW_PTS: f64 = 25.0;
/// Pontos por sair da pista. `[A OLHO]`
pub(super) const OFFTRACK_PTS: f64 = 8.0;

/// Aumento (s) em `PitRepairLeft + PitOptRepairLeft` que conta como dano NOVO. Só
/// separa ruído de ponto flutuante do que o sim de fato acrescentou. `[MEDIDO]` — os
/// canais de reparo são MUDOS fora do box (medido: carro destruído, meatball na tela,
/// 0.0 em todos os frames), então este limiar só age quando o carro já está na caixa.
pub(super) const REPAIR_JUMP_SECS: f64 = 0.05;

/// Distância máxima na volta (fração 0–1) para considerar outro carro "no mesmo
/// ponto" do jogador num contato — ~2% da pista (poucos comprimentos de carro).
/// `[A OLHO]`
pub(super) const CONTACT_NEAR_PCT: f64 = 0.02;

// ─── Faixas de severidade ────────────────────────────────────────────────────
// Onde a soma dos pesos acima vira um NOME. Estas cinco linhas decidem se o jogador ouve
// "toque", "batida feia" ou "acabou"; e, com a regra do carro destruído ligada, decidem
// também se ele larga do fundo ou não larga.
//
// `[A OLHO]`, todas. A referência viva é o comentário dos testes que arma
// `peak_crash_score` na mão: 46 é "leve", 180 é "destruído", 240 é "catastrófico".

/// `[A OLHO]`
pub(super) const SEV_MINOR: f64 = 15.0;
/// `[A OLHO]`
pub(super) const SEV_MODERATE: f64 = 50.0;
/// `[A OLHO]`
pub(super) const SEV_SEVERE: f64 = 110.0;
/// `[A OLHO]`
pub(super) const SEV_TOTALED: f64 = 170.0;
/// `[A OLHO]`
pub(super) const SEV_CATASTROPHIC: f64 = 230.0;

// ─── Carro destruído na CLASSIFICAÇÃO ────────────────────────────────────────
// O iRacing devolve o carro inteiro para a corrida; a consequência é regra NOSSA, imposta
// por comando de admin, como já é o Sistema de Quebra.
//
// O castigo age em DOIS momentos, e as duas faixas usam ferramentas DIFERENTES porque o
// iRacing não deixa escolher. AO VIVO, na quali: o rádio anuncia que a classificação acabou
// no instante da batida — e só a faixa terminal manda `!dq` junto. NA CORRIDA: "grave" larga
// do fundo (`!eol`, medido funcionando em 2026-08-10); "destruído"+ vira `!dq`, o carro não
// corre. "Catastrófico" só muda a fala do rádio ("você está inteiro?").
//
// Por que "grave" NÃO trava a quali no sim: o `!dq` do iRacing é terminal e vale o EVENTO,
// não a sessão, e o `!clear` é recusado sobre ele — "Can't clear penalties for car with DQ
// scoring invalidated" (medido na pista em 2026-08-10). Um `!dq` na classificação portanto
// APAGA o `!eol` que viria depois: o jogador não largaria, que é o castigo da faixa de cima.
// Como não há comando que encerre só a sessão, em "grave" o sim fica de fora: o rádio
// encerra a classificação e a corrida cobra. O jogador pode seguir rodando — o tempo dele é
// que não vale mais, porque a largada já está decidida.
//
// A SEVERIDADE da batida gradua (G + velocidade perdida, impacto confirmado). O MEATBALL
// (`FLAG_REPAIR`) é piso de "grave": é o sim declarando reparo obrigatório, e cobre o
// score ficar curto em pista molhada/G subamostrado. Os canais `PitRepairLeft` são MUDOS
// fora do box (medido: carro destruído, meatball na tela, 0.0 em todos os frames — e na
// quali o botão de box conserta o carro, então eles nunca falam lá); quando falarem,
// também são piso. Rodada sem impacto não castiga nunca.
//
// PROCEDÊNCIA, item por item — o que foi medido aqui é o COMPORTAMENTO DO SIM (que
// comando pega, qual apaga qual), não ONDE ficam os cortes. A escolha de "grave" para o
// `!eol` e "destruído" para o `!dq` é de PRODUTO: é onde se decidiu que a punição passa a
// doer. Os dois valores em segundos de reparo são `[A OLHO]` puros — não há medição
// escrita ligando 25 s de reparo a "grave" nem 60 s a "destruído", e enquanto os canais
// de reparo forem mudos fora do box eles quase nunca são o gatilho de fato.

/// Severidade a partir da qual a classificação é dada por encerrada e a corrida sai do
/// fundo. `[A OLHO]` (decisão de produto sobre onde a punição começa a doer).
pub(super) const QUALI_WRECK_PENALTY_SEV: Severidade = Severidade::Grave;
/// Severidade a partir da qual o carro não corre (DQ na quali E na corrida).
/// `[A OLHO]` (mesma decisão de produto, faixa terminal).
pub(super) const QUALI_WRECK_DQ_SEV: Severidade = Severidade::Destruido;
/// Reparo obrigatório (s) que também basta para o piso de "grave". Corroboração.
/// `[A OLHO]` — sem medição ligando este número à faixa.
pub(super) const QUALI_WRECK_PENALTY_S: f64 = 25.0;
/// Reparo obrigatório (s) que também basta para o piso de "destruído".
/// `[A OLHO]` — idem.
pub(super) const QUALI_WRECK_DQ_S: f64 = 60.0;
/// Penalidade (s) da bandeira preta quando o `!eol` perde a janela da formação. O castigo
/// não pode evaporar só porque o YAML demorou a entregar o número do carro.
/// `[A OLHO]` — a EXISTÊNCIA do fallback é necessária (ver
/// [`super::quebras`] e o registro de envio em `imp::chat`); o tamanho da penalidade em
/// segundos nunca foi comparado com o prejuízo de largar do fundo.
pub(super) const QUALI_WRECK_FALLBACK_PENALTY_S: u32 = 15;

/// Quanto tempo esperar pela PROVA de que o castigo chegou ao simulador, antes de registrar
/// que ela não veio. Ver [`super::ProvaDoCastigo`].
///
/// Generoso de propósito, e o motivo é a fila: o comando não sai no tique do despacho, ele
/// entra em `pending_breakdown_cmds` e o amostrador drena UM a cada ~1,5 s, fora do lock. A
/// janela precisa caber a fila mais o tempo de o sim abrir o chat e aplicar. Errar para o
/// lado curto produziria alarme falso, que é o pior desfecho possível numa medição.
///
/// Não há disputa com o Sistema de Quebra nesta janela: o castigo da quali é despachado nos
/// primeiros tiques da corrida (antes do verde) e a quebra só é liberada depois de
/// `BREAKDOWN_GRACE_SECS` de prova verde. Nenhum outro `!black` nosso pode estar em voo aqui.
///
/// `[ORÇAMENTO]` — é uma janela de diagnóstico, não uma regra de jogo. Alargá-la só atrasa a
/// linha de log.
pub(super) const JANELA_PROVA_DO_CASTIGO_S: f64 = 30.0;

/// Variável de ambiente que arma a regra do carro destruído na classificação.
///
/// Ela deixou de ser a FONTE da decisão (agora é `AppConfig::quali_wreck_penalty`, que
/// sobrevive ao reinício e aparece no `config.json`) e virou um ATALHO de teste na pista:
/// presente no ambiente, liga a regra independentemente da preferência gravada. É o que o
/// `testar-quali-destruida.cmd` usa. `[SDK]`-neutro: é infraestrutura de teste, não
/// calibração.
pub(super) const QUALI_WRECK_ENV: &str = "IRACER_QUALI_WRECK";
