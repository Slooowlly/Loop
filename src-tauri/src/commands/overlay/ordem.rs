//! Ordem da torre de tempos: a posição que o overlay mostra.
//!
//! Por que não usar o `CarIdxClassPosition` do SDK e pronto? Porque ele é a posição
//! **oficial**, e o iRacing só a recalcula quando o carro cruza a linha de chegada.
//! Ao vivo isso quer dizer que uma ultrapassagem no meio da volta só aparece na torre
//! na volta seguinte — e, antes da largada (ou logo na transição quali→corrida), ele
//! vem zerado/estagnado, o que jogava a torre numa ordem aparentemente aleatória.
//!
//! Então a torre calcula a ordem por conta própria, e o critério muda com o momento:
//!
//! | Momento                            | Critério                                    |
//! |------------------------------------|---------------------------------------------|
//! | Treino / classificatória           | melhor volta da sessão (mais rápido na frente) |
//! | Corrida antes do verde             | grid = melhor volta da quali                |
//! | Corrida no verde                   | progresso real (voltas + fração da volta)   |
//! | Bandeirada / cooldown              | posição OFICIAL do SDK (o resultado)        |

/// Uma entrada da torre, reduzida ao que a ordenação precisa.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OrderInput {
    /// `CarIdxClassPosition` (oficial; 0 = sem posição).
    pub class_position: i32,
    /// `CarIdxLapCompleted` (-1/0 = nenhuma volta completa).
    pub lap_completed: i32,
    /// `CarIdxLapDistPct` 0..1 (-1 = fora do mundo).
    pub lap_dist_pct: f64,
    /// Melhor volta da sessão ATUAL em ms (i64::MAX = nenhuma).
    pub best_lap_ms: i64,
    /// Melhor volta da CLASSIFICATÓRIA em ms (i64::MAX = nenhuma).
    pub qualy_best_ms: i64,
    /// Ordem 1..n de ANTES de haver tempo (campeonato ou expectativa — ver
    /// [`ordem_pre_sessao`]). i64::MAX = piloto sem dono na carreira.
    pub pre_ordem: i64,
    /// Número do carro — desempate estável e último recurso.
    pub car_number: i64,
}

/// O que decide a ordem enquanto NINGUÉM marcou volta. Antes da 1ª corrida da
/// temporada o campeonato ainda está zerado e a única hierarquia que existe é a
/// expectativa; a partir dela, a tabela manda.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PreSinal {
    /// Corridas do piloto NESTA temporada — o gatilho entre os dois critérios.
    pub corridas_temporada: i32,
    pub pontos: i32,
    pub vitorias: i32,
    pub podios: i32,
    /// Percepção pública de pré-temporada (a mesma da matéria "O Que Esperar").
    /// Maior = mais cotado.
    pub expectativa: f64,
    /// O piloto foi resolvido na carreira. Quem não foi não tem hierarquia nenhuma.
    pub conhecido: bool,
}

/// Ordem 1..n dos pilotos de UMA classe para quando não há tempo de volta, na mesma
/// posição do vetor de entrada (i64::MAX = sem hierarquia conhecida → cai no número
/// do carro). Antes da 1ª corrida da temporada usa a EXPECTATIVA de pré-temporada;
/// depois dela, a POSIÇÃO NO CAMPEONATO (pontos → vitórias → pódios), o mesmo critério
/// da aba de classificação.
pub(crate) fn ordem_pre_sessao(sinais: &[PreSinal]) -> Vec<i64> {
    let ja_correu = sinais.iter().any(|s| s.corridas_temporada > 0);
    let mut conhecidos: Vec<usize> = (0..sinais.len()).filter(|&i| sinais[i].conhecido).collect();
    conhecidos.sort_by(|&a, &b| {
        let (x, y) = (&sinais[a], &sinais[b]);
        if ja_correu {
            y.pontos
                .cmp(&x.pontos)
                .then_with(|| y.vitorias.cmp(&x.vitorias))
                .then_with(|| y.podios.cmp(&x.podios))
        } else {
            y.expectativa
                .partial_cmp(&x.expectativa)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    let mut out = vec![i64::MAX; sinais.len()];
    for (lugar, &i) in conhecidos.iter().enumerate() {
        out[i] = lugar as i64 + 1;
    }
    out
}

/// Em que critério a torre se ordena agora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrderMode {
    /// Treino/quali: quem tem a melhor volta vai na frente.
    Tempo,
    /// Corrida antes do verde: a ordem de largada (grid da quali).
    Grid,
    /// Corrida rolando: progresso real na pista, atualizado a cada tick.
    Progresso,
    /// Fim de prova: a posição oficial do SDK é o resultado.
    Oficial,
}

/// Escolhe o critério a partir do tipo de sessão e do `SessionState` do SDK
/// (1 GetInCar, 2 Warmup, 3 ParadeLaps, 4 Racing, 5 Checkered, 6 CoolDown).
pub(crate) fn modo_da_sessao(kind: &str, session_state: i32) -> OrderMode {
    if kind != "R" {
        return OrderMode::Tempo;
    }
    match session_state {
        s if s >= 5 => OrderMode::Oficial,
        4 => OrderMode::Progresso,
        _ => OrderMode::Grid,
    }
}

/// Progresso total do carro na prova: voltas completas + fração da volta atual.
/// `lap_completed` negativo (nenhuma volta) e `lap_dist_pct` de carro fora do mundo
/// viram 0 — senão um carro sem volta ficaria à frente de quem já largou.
fn progresso(c: &OrderInput) -> f64 {
    let voltas = c.lap_completed.max(0) as f64;
    let fracao = if c.lap_dist_pct.is_finite() {
        c.lap_dist_pct.clamp(0.0, 1.0)
    } else {
        0.0
    };
    voltas + fracao
}

/// Chave de ordenação (menor = mais à frente) de um carro no modo dado.
/// `tem_grid` diz se ALGUÉM da classe tem tempo de quali — sem isso o modo Grid
/// cairia numa ordem por número de carro e é melhor herdar a posição do SDK.
/// A chave é sempre `(classificado?, critério do modo, ordem prévia, nº do carro)`. O
/// terceiro campo é o que dá sentido à torre ANTES de qualquer volta cronometrada: com
/// todo mundo empatado em "sem tempo", a ordem cai no campeonato/expectativa em vez de
/// numa fila por número de carro.
fn chave(modo: OrderMode, c: &OrderInput, tem_grid: bool) -> (i64, i64, i64, i64) {
    // Oficial: classificado primeiro (por posição), o resto pelo grid → prévia → número.
    let oficial = |c: &OrderInput| {
        if c.class_position > 0 {
            (0, i64::from(c.class_position), 0, 0)
        } else {
            (1, c.qualy_best_ms, c.pre_ordem, c.car_number)
        }
    };
    match modo {
        OrderMode::Oficial => oficial(c),
        OrderMode::Tempo => (0, c.best_lap_ms, c.pre_ordem, c.car_number),
        OrderMode::Grid => {
            if tem_grid {
                (0, c.qualy_best_ms, c.pre_ordem, c.car_number)
            } else {
                oficial(c)
            }
        }
        // Progresso DECRESCENTE: quem andou mais vai na frente. Em milésimos de
        // volta, negativado, pra caber na mesma chave inteira das outras.
        OrderMode::Progresso => (
            0,
            -((progresso(c) * 1000.0).round() as i64),
            c.pre_ordem,
            c.car_number,
        ),
    }
}

/// Ordena os carros de UMA classe e devolve os índices de `cars` na ordem da torre
/// (a posição mostrada é `rank + 1`).
pub(crate) fn ordenar(modo: OrderMode, cars: &[OrderInput]) -> Vec<usize> {
    let tem_grid = cars.iter().any(|c| c.qualy_best_ms < i64::MAX);
    let mut idx: Vec<usize> = (0..cars.len()).collect();
    idx.sort_by_key(|&i| chave(modo, &cars[i], tem_grid));
    idx
}
