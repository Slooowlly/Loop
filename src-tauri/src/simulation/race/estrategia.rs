//! **Estratégia de parada e safety car** — os dois maiores geradores de troca de posição do
//! automobilismo real, e o par natural do modelo de posição na pista.
//!
//! Por que este módulo existe. O pacote D deu atrito: trem de carros, ar sujo, ultrapassagem
//! que custa. O trem é o PROBLEMA; o box é a RESPOSTA — quem não passa na pista passa na
//! parada. E o safety car é o maior embaralhador que existe: zera os gaps, regala quem ia
//! parar mesmo e crucifica quem parou uma volta antes.
//!
//! Cai sobre coisas que já existiam e não tinham consequência:
//!
//! - `gestao_pneus` e a degradação de pneu de `pontuacao.rs` já cobravam ritmo de pneu gasto.
//!   Parar zera o desgaste, e é isso — sem nenhuma constante nova de ritmo — que faz undercut
//!   e overcut existirem.
//! - `derive_caution_segments` já derivava períodos de bandeira amarela dos incidentes. O
//!   gatilho existia e não fazia nada. Agora faz.
//!
//! A decisão de parar é **IA de equipe**: a chamada boa e a chamada ruim são personagem da
//! equipe, e é aqui que a equipe finalmente vira um ator dentro da corrida. O jogador vai
//! querer decidir a dele — isso é UI e não é deste pacote.
//!
//! ## A armadilha, escrita antes de medir
//!
//! **O safety car não pode ser a fonte principal de troca de liderança do campeonato.** É o
//! mecanismo mais barato de embaralhar que existe. Um SC que não muda quem ganha é uma
//! animação; um SC que decide sozinho quem ganha é uma roleta. O alvo vive entre os dois, e
//! as sete métricas de resultado NÃO distinguem um campeonato disputado de um sorteado —
//! quem distingue é a decomposição de variância, com o teto de azar de 25%/20% como limite de
//! design.

use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};

// ─────────────────────── Constantes (PROVISÓRIAS) ───────────────────────
//
// Nenhuma calibrada: pontos de partida legíveis. A campanha de calibração roda uma vez sobre
// o espaço completo depois deste pacote, de propósito, para não calibrar contra alvo móvel.

/// Tempo perdido numa parada normal: entrada, box, saída. PROVISÓRIO.
pub const CUSTO_DE_PARADA_MS: f64 = 22_000.0;

/// Fração do custo de parada quando ela é feita sob safety car — o pelotão está lento, então
/// a parada custa muito menos posição. É o coração do "regala quem ia parar mesmo".
pub const FRACAO_DO_CUSTO_SOB_SAFETY_CAR: f64 = 0.40;

/// Duração mínima de corrida (minutos) para existir janela de parada. Abaixo disto a prova é
/// curta demais e o grid inteiro vai de ponta a ponta — que é o caso da monomarca de entrada.
pub const MINUTOS_MINIMOS_PARA_PARADA: i32 = 40;

/// Janela de parada, como fração da distância da corrida. Fora dela ninguém para por opção.
pub const JANELA_DE_PARADA: (f64, f64) = (0.35, 0.65);

/// Quanto o pelotão se comprime atrás do líder sob safety car (fração do gap original).
pub const FATOR_DE_COMPRESSAO_DO_PELOTAO: f64 = 0.20;

/// Caos extra do RELANÇAMENTO, aplicado ao trecho seguinte ao safety car.
///
/// Sem isto o safety car é a animação que o briefing avisou: comprimir os gaps PRESERVA a
/// ordem (é requisito — ver [`atraso_sob_safety_car`]), e como o líder normalmente é o mais
/// rápido, ninguém tenta passá-lo. Medido: Δ de vencedores distintos = 0.
///
/// O que faltava é que o relançamento É uma segunda largada — pneu frio, pelotão colado,
/// efeito acordeão. Por isso ele reusa a amplificação de caos que o segmento de largada já
/// tinha (`start_chaos_multiplier` × `pack_density_factor`), em vez de inventar um mecanismo
/// novo: é o mesmo fenômeno, na mesma máquina. PROVISÓRIO.
pub const CAOS_DO_RELANCAMENTO: f64 = 1.0;

/// Espaçamento mínimo por posição sob safety car, em ms — o pelotão fica em fila indiana,
/// não empilhado no mesmo ponto.
pub const GAP_MINIMO_SOB_SAFETY_CAR_MS: f64 = 250.0;

// ─────────────────────── Bandeira amarela ───────────────────────

/// Este incidente traz a bandeira amarela?
///
/// Fonte ÚNICA do predicado: `derive_caution_segments` (o registro no resultado) e o safety
/// car (a consequência na corrida) leem daqui. Antes deste pacote só existia o registro, e
/// duplicar o critério seria a maneira mais fácil de eles divergirem em silêncio.
pub fn traz_bandeira_amarela(inc: &IncidentResult) -> bool {
    match (inc.incident_type, inc.severity) {
        // Batida forte: carro destruído e destroços na pista.
        (IncidentType::Collision, IncidentSeverity::Critical) => true,
        // Batida média só neutraliza se tirou o carro da corrida.
        (IncidentType::Collision, IncidentSeverity::Major) => inc.is_dnf,
        // Erro grave que terminou na parede.
        (IncidentType::DriverError, IncidentSeverity::Critical) => inc.is_dnf,
        // Pane mecânica: o carro recolhe pro box, não neutraliza.
        _ => false,
    }
}

/// Onde o pelotão fica depois da compressão do safety car, em ms de atraso para o líder.
///
/// Zera os gaps sem reordenar: as duas parcelas do `max` crescem com a posição, então quem
/// estava na frente continua na frente. O que se perde é a MARGEM — e é isso que transforma
/// uma liderança confortável em briga, e o que torna o SC o maior embaralhador do esporte.
pub fn atraso_sob_safety_car(gap_para_o_lider_ms: f64, posicao: i32) -> f64 {
    let comprimido = gap_para_o_lider_ms * FATOR_DE_COMPRESSAO_DO_PELOTAO;
    let fila = (posicao.max(1) - 1) as f64 * GAP_MINIMO_SOB_SAFETY_CAR_MS;
    comprimido.max(fila)
}

// ─────────────────────── Estratégia de parada ───────────────────────

/// O plano de parada de um carro para esta corrida.
#[derive(Debug, Clone)]
pub struct PlanoDeParada {
    /// Rótulo da estratégia, para o harness contar estratégias distintas sem inferir.
    pub estrategia_id: String,
    /// Voltas em que este carro pretende parar. Vazio = vai de ponta a ponta.
    pub voltas_planejadas: Vec<u32>,
}

impl PlanoDeParada {
    pub fn sem_parada() -> Self {
        Self {
            estrategia_id: "sem-parada".to_string(),
            voltas_planejadas: Vec::new(),
        }
    }
}

/// Hash determinístico (FNV-1a + finalizador splitmix) para a chamada da equipe.
///
/// Determinístico de propósito, e NÃO tirado do `rng` da corrida: a chamada é personagem da
/// equipe naquele evento, não sorte do dia — e assim ela não perturba a sequência de sorteios
/// do motor, o que mantém a prova de equivalência da fase 1 do pacote C intacta.
fn semente(partes: &[&str], numeros: &[i64]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for p in partes {
        for b in p.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    for n in numeros {
        for b in n.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

fn uniforme(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// A chamada da equipe: em que volta este carro para.
///
/// Prova curta (abaixo de [`MINUTOS_MINIMOS_PARA_PARADA`]) não tem parada — é o caso da
/// monomarca de entrada, e o alvo do harness diz "rookie sem parada, 1–2 estratégias
/// distintas no grid".
///
/// Nas provas longas, a equipe escolhe uma volta dentro de [`JANELA_DE_PARADA`]. Equipe boa
/// escolhe perto do meio da janela (o ponto que equilibra pneu novo e track position); equipe
/// ruim erra para um dos lados — **é a chamada ruim, e ela é de propósito**. Os dois carros da
/// mesma equipe recebem chamadas distintas (o `driver_id` entra na semente), que é o que
/// permite ao grid ter mais de uma estratégia viva.
pub fn planejar_paradas(
    driver_id: &str,
    team_id: &str,
    track_id: u32,
    total_laps: i32,
    duracao_min: i32,
    qualidade_de_estrategia: f64,
) -> PlanoDeParada {
    if duracao_min < MINUTOS_MINIMOS_PARA_PARADA || total_laps < 8 {
        return PlanoDeParada::sem_parada();
    }

    let h = semente(&[team_id, driver_id], &[track_id as i64, total_laps as i64]);
    // Erro da chamada: equipe de qualidade 100 acerta o meio da janela; equipe de qualidade 0
    // erra até a borda.
    let acerto = (qualidade_de_estrategia.clamp(0.0, 100.0) / 100.0).clamp(0.0, 1.0);
    let desvio = (uniforme(h) * 2.0 - 1.0) * (1.0 - acerto);

    let (inicio, fim) = JANELA_DE_PARADA;
    let meio = (inicio + fim) / 2.0;
    let meia_largura = (fim - inicio) / 2.0;
    let fracao = (meio + desvio * meia_largura).clamp(inicio, fim);
    let volta = ((total_laps as f64 * fracao).round() as u32).max(2);

    // Rótulo pelo terço da janela em que a chamada caiu — é o que o harness conta.
    let terco = (fracao - inicio) / (fim - inicio);
    let rotulo = if terco < 0.33 {
        "1-parada-cedo"
    } else if terco < 0.67 {
        "1-parada-ideal"
    } else {
        "1-parada-tarde"
    };

    PlanoDeParada {
        estrategia_id: rotulo.to_string(),
        voltas_planejadas: vec![volta],
    }
}

/// O que aconteceu com as paradas de um carro. Dado CRU — o par
/// (`posicao_antes_da_parada`, `posicao_depois`) é o que torna o undercut mensurável sem
/// reconstruir a corrida.
#[derive(Debug, Clone, Default)]
pub struct ParadasDoCarro {
    pub estrategia_id: String,
    pub voltas: Vec<u32>,
    pub posicao_antes: Vec<i32>,
    pub posicao_depois: Vec<i32>,
    /// Voltas que ainda faltam cumprir do plano.
    pub pendentes: Vec<u32>,
    /// Índice, em `voltas`, das paradas cujo `posicao_depois` ainda não foi preenchido.
    pub aguardando_fechamento: Vec<usize>,
}

#[cfg(test)]
#[path = "estrategia/tests.rs"]
mod tests;
