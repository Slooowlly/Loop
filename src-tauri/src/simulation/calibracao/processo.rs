//! **Métricas de processo** — não o que a corrida entregou, mas como ela aconteceu.
//!
//! As sete métricas de resultado podem ficar verdes com um processo errado: dá para produzir a
//! distribuição final certa embaralhando tudo na largada e congelando depois, ou distribuindo
//! posições por sorteio sem nenhuma disputa. Estas medem o caminho.
//!
//! ## O que dá para medir com o que `RaceResult` expõe — e o que não dá
//!
//! Derivável: `grid_position`, `finish_position`, `positions_gained`, `gap_to_winner_ms`,
//! `is_dnf`, `dnf_segment`, e os `incidents` com o segmento de cada um. Disso saem as trocas de
//! posição (distância de Kendall entre grid e chegada), a cauda de ganho/perda e a forma da
//! distribuição de gaps.
//!
//! **Não derivável: posição por segmento.** `RaceDriverResult` guarda só a posição FINAL; o
//! `current_position` de cada segmento vive no `RaceState`, que é interno ao laço da corrida e
//! não sai de lá. Então as duas métricas do briefing que dependem disso — "trocas por segmento" e
//! "em que segmento a posição final estabiliza" — não têm como ser calculadas sem expor o dado.
//! Conforme combinado, isto é reportado em vez de contornado com mudança fora da fronteira.
//! Ver [`LACUNA_SEGMENTO`].
//!
//! No lugar delas, [`medir_poder_da_largada`] responde a pergunta de fundo ("a corrida acaba na
//! largada?") por um caminho que só usa API pública: rodar o mesmo evento com um grid de largada
//! SORTEADO, sem relação nenhuma com o ritmo, e ver o quanto a chegada segue o grid mesmo assim.
//! É argumentavelmente uma resposta melhor que a original — mede a consequência causal da
//! posição inicial, não só a correlação temporal.

use crate::simulation::race::RaceResult;

use super::arena::{self, ConfigTemporada};
use super::campo::gerar_campo;
use super::metricas::spearman;

/// O que falta em `RaceResult` para fechar as métricas de processo por segmento. Constante e não
/// função de propósito: é texto de relatório, não lógica.
pub const LACUNA_SEGMENTO: &str = "\
`RaceDriverResult` não guarda a posição por segmento — só `finish_position`. As posições \
intermediárias existem em `RaceState::current_position` dentro de `race/motor.rs`, mas são \
descartadas ao montar o resultado. Para medir \"trocas por segmento\" e \"em que segmento a \
posição estabiliza\" bastaria um campo `posicoes_por_segmento: Vec<i32>` (5 entradas) em \
`RaceDriverResult`, preenchido no laço que já calcula `state.current_position`. Fora da \
fronteira deste pacote — reportado, não alterado.";

// ---------------------------------------------------------------------------
// Métricas por corrida
// ---------------------------------------------------------------------------

/// Retrato de UMA corrida do ponto de vista do processo.
#[derive(Debug, Clone, Default)]
pub struct MetricasProcesso {
    /// Trocas de posição entre o grid e a chegada, medidas como distância de Kendall: o número
    /// MÍNIMO de ultrapassagens entre carros vizinhos capaz de levar a ordem de largada à ordem
    /// de chegada. É um piso do que aconteceu na pista, nunca um teto.
    pub trocas: f64,
    /// A mesma coisa normalizada pelo máximo possível, `n(n−1)/2`. 0 = chegou na ordem que
    /// largou; 0,5 = ordem de chegada independente do grid; 1 = ordem exatamente invertida.
    pub trocas_normalizadas: f64,
    /// Média de |posições ganhas ou perdidas| da largada à chegada.
    pub ganho_medio_abs: f64,
    /// Percentil 90 do mesmo — é a CAUDA que faz a corrida ter história.
    pub ganho_p90: f64,
    /// Maior ganho de posição da corrida (a "recuperação do dia").
    pub maior_ganho: f64,
    /// Coeficiente de variação dos gaps ENTRE CARROS CONSECUTIVOS. Uma escada regular tende a 0;
    /// corrida de verdade tem pelotões colados e buracos, então fica bem acima.
    pub cv_gaps_sucessivos: f64,
    /// Maior buraco entre dois carros consecutivos, em múltiplos do buraco mediano.
    pub maior_buraco_relativo: f64,
    /// Quantos pelotões o pelotão formou: grupos separados por um buraco > 2× a mediana.
    pub pelotoes: f64,
}

fn percentil(ordenado: &[f64], p: f64) -> f64 {
    if ordenado.is_empty() {
        return f64::NAN;
    }
    let idx = ((ordenado.len() - 1) as f64 * p).round() as usize;
    ordenado[idx.min(ordenado.len() - 1)]
}

fn mediana(valores: &[f64]) -> f64 {
    if valores.is_empty() {
        return f64::NAN;
    }
    let mut v = valores.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    percentil(&v, 0.5)
}

/// Distância de Kendall: pares (i, j) cuja ordem relativa no grid difere da ordem na chegada.
fn inversoes(pares: &[(f64, f64)]) -> f64 {
    let mut total = 0.0;
    for i in 0..pares.len() {
        for j in (i + 1)..pares.len() {
            let (gi, fi) = pares[i];
            let (gj, fj) = pares[j];
            if (gi - gj) * (fi - fj) < 0.0 {
                total += 1.0;
            }
        }
    }
    total
}

pub fn medir_processo(corrida: &RaceResult) -> MetricasProcesso {
    let terminaram: Vec<&crate::simulation::race::RaceDriverResult> =
        corrida.race_results.iter().filter(|r| !r.is_dnf).collect();
    if terminaram.len() < 3 {
        return MetricasProcesso::default();
    }

    // --- Trocas de posição ---
    let pares: Vec<(f64, f64)> = terminaram
        .iter()
        .map(|r| (r.grid_position as f64, r.finish_position as f64))
        .collect();
    let n = pares.len() as f64;
    let maximo = n * (n - 1.0) / 2.0;
    let trocas = inversoes(&pares);

    // --- Ganho / perda de posição ---
    let mut ganhos: Vec<f64> = terminaram
        .iter()
        .map(|r| (r.positions_gained as f64).abs())
        .collect();
    ganhos.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let ganho_medio_abs = ganhos.iter().sum::<f64>() / n;
    let maior_ganho = terminaram
        .iter()
        .map(|r| r.positions_gained)
        .max()
        .unwrap_or(0) as f64;

    // --- Forma da distribuição de gaps ---
    let mut por_posicao: Vec<&&crate::simulation::race::RaceDriverResult> =
        terminaram.iter().collect();
    por_posicao.sort_by_key(|r| r.finish_position);
    let gaps: Vec<f64> = por_posicao
        .windows(2)
        .map(|w| (w[1].gap_to_winner_ms - w[0].gap_to_winner_ms).max(0.0))
        .collect();

    let (cv, maior_relativo, pelotoes) = if gaps.len() < 2 {
        (f64::NAN, f64::NAN, 1.0)
    } else {
        let m = gaps.iter().sum::<f64>() / gaps.len() as f64;
        let dp =
            (gaps.iter().map(|g| (g - m).powi(2)).sum::<f64>() / (gaps.len() - 1) as f64).sqrt();
        let med = mediana(&gaps);
        let maior = gaps.iter().cloned().fold(0.0_f64, f64::max);
        let quebras = gaps.iter().filter(|g| **g > med * 2.0).count() as f64;
        (
            if m > f64::EPSILON { dp / m } else { f64::NAN },
            if med > f64::EPSILON {
                maior / med
            } else {
                f64::NAN
            },
            quebras + 1.0,
        )
    };

    MetricasProcesso {
        trocas,
        trocas_normalizadas: if maximo > 0.0 {
            trocas / maximo
        } else {
            f64::NAN
        },
        ganho_medio_abs,
        ganho_p90: percentil(&ganhos, 0.90),
        maior_ganho,
        cv_gaps_sucessivos: cv,
        maior_buraco_relativo: maior_relativo,
        pelotoes,
    }
}

// ---------------------------------------------------------------------------
// Agregação
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MetricasProcessoAgregadas {
    pub rotulo: String,
    pub corridas: usize,
    pub trocas: f64,
    pub trocas_normalizadas: f64,
    pub ganho_medio_abs: f64,
    pub ganho_p90: f64,
    pub maior_ganho: f64,
    pub cv_gaps_sucessivos: f64,
    pub maior_buraco_relativo: f64,
    pub pelotoes: f64,
}

fn media(v: &[f64]) -> f64 {
    let finitos: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if finitos.is_empty() {
        return f64::NAN;
    }
    finitos.iter().sum::<f64>() / finitos.len() as f64
}

pub fn agregar_processo(rotulo: &str, amostras: &[MetricasProcesso]) -> MetricasProcessoAgregadas {
    let campo =
        |f: fn(&MetricasProcesso) -> f64| media(&amostras.iter().map(f).collect::<Vec<_>>());
    MetricasProcessoAgregadas {
        rotulo: rotulo.to_string(),
        corridas: amostras.len(),
        trocas: campo(|m| m.trocas),
        trocas_normalizadas: campo(|m| m.trocas_normalizadas),
        ganho_medio_abs: campo(|m| m.ganho_medio_abs),
        ganho_p90: campo(|m| m.ganho_p90),
        maior_ganho: campo(|m| m.maior_ganho),
        cv_gaps_sucessivos: campo(|m| m.cv_gaps_sucessivos),
        maior_buraco_relativo: campo(|m| m.maior_buraco_relativo),
        pelotoes: campo(|m| m.pelotoes),
    }
}

/// Roda uma campanha e agrega o processo de todas as corridas.
pub fn medir_campanha_processo(
    rotulo: &str,
    config: &ConfigTemporada,
    temporadas: usize,
    semente_base: u64,
) -> MetricasProcessoAgregadas {
    let amostras: Vec<MetricasProcesso> =
        arena::rodar_campanha_crua(config, temporadas, semente_base)
            .iter()
            .flat_map(|(_, corridas)| corridas.iter().map(medir_processo))
            .collect();
    agregar_processo(rotulo, &amostras)
}

// ---------------------------------------------------------------------------
// O poder da largada
// ---------------------------------------------------------------------------

/// Quanto a posição de largada, POR SI SÓ, determina a chegada.
///
/// O experimento: o mesmo evento, o mesmo grid de pilotos, rodado duas vezes —
/// uma com o grid de largada saído da classificação (o normal), outra com o grid **sorteado**,
/// sem nenhuma relação com o ritmo. No segundo caso, qualquer correlação entre largada e chegada
/// só pode ser herança da posição inicial: o ritmo está distribuído aleatoriamente pelo grid.
///
/// - `rho_grid_normal` alto e `rho_grid_sorteado` baixo → a ordem vem do ritmo; a largada é
///   decoração e a classificação é que está determinando tudo.
/// - `rho_grid_sorteado` também alto → **a corrida acaba na largada**: quem larga na frente fica
///   na frente independentemente de quem é.
#[derive(Debug, Clone)]
pub struct PoderDaLargada {
    pub rotulo: String,
    pub corridas: usize,
    /// Spearman grid × chegada com o grid vindo da classificação.
    pub rho_grid_normal: f64,
    /// Spearman grid × chegada com o grid SORTEADO.
    pub rho_grid_sorteado: f64,
    /// Spearman skill × chegada com o grid sorteado — o ritmo conseguiu se impor ao grid ruim?
    pub rho_skill_com_grid_sorteado: f64,
    /// Trocas normalizadas com grid sorteado. Se a largada não manda, tem que explodir para ~0,5.
    pub trocas_normalizadas_sorteado: f64,
}

pub fn medir_poder_da_largada(
    rotulo: &str,
    config: &ConfigTemporada,
    repeticoes: usize,
    semente_base: u64,
) -> PoderDaLargada {
    let catalogo = arena::catalogo_para(config);
    let mut normais = Vec::new();
    let mut sorteados = Vec::new();
    let mut skills = Vec::new();
    let mut trocas = Vec::new();

    for i in 0..repeticoes {
        let semente = arena::semente_da_temporada(semente_base, i);
        let grid = gerar_campo(&config.perfil, config.pilotos, semente);
        let eventos = arena::sortear_eventos(config, semente);
        let Some(evento) = eventos.first() else {
            continue;
        };

        for (embaralhar, destino) in [(false, &mut normais), (true, &mut sorteados)] {
            let resultado = arena::rodar_evento_com_grid_imposto(
                config,
                &grid,
                evento,
                1,
                &catalogo,
                semente ^ 0xA11CE,
                embaralhar,
            );
            let (g, f): (Vec<f64>, Vec<f64>) = resultado
                .race_results
                .iter()
                .filter(|r| !r.is_dnf)
                .map(|r| (r.grid_position as f64, r.finish_position as f64))
                .unzip();
            if let Some(rho) = spearman(&g, &f) {
                destino.push(rho);
            }

            if embaralhar {
                // Com o grid sorteado, o ritmo é a única coisa que pode reordenar o pelotão.
                let mut xs = Vec::new();
                let mut ys = Vec::new();
                for r in resultado.race_results.iter().filter(|r| !r.is_dnf) {
                    if let Some(d) = grid.iter().find(|d| d.id == r.pilot_id) {
                        xs.push(-(d.skill as f64)); // skill alto = posição baixa
                        ys.push(r.finish_position as f64);
                    }
                }
                if let Some(rho) = spearman(&xs, &ys) {
                    skills.push(rho);
                }
                trocas.push(medir_processo(&resultado).trocas_normalizadas);
            }
        }
    }

    PoderDaLargada {
        rotulo: rotulo.to_string(),
        corridas: repeticoes * 2,
        rho_grid_normal: media(&normais),
        rho_grid_sorteado: media(&sorteados),
        rho_skill_com_grid_sorteado: media(&skills),
        trocas_normalizadas_sorteado: media(&trocas),
    }
}
