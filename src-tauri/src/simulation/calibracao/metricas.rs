//! As métricas de distribuição: o que se extrai de uma temporada inteira para responder
//! "isto parece corrida ou parece planilha ordenada?".
//!
//! Nenhuma métrica olha para dentro da simulação — todas saem do que o [`RaceResult`] já
//! devolve. É de propósito: a régua tem que continuar valendo depois que o motor mudar.
//!
//! Convenção de DNF: as correlações e o desvio de posição são medidos **só sobre quem
//! terminou**. Abandono é ruído de outra natureza (quebra, batida) e entra na conta como
//! `dnfs_por_etapa`, separado. Medir junto seria fácil demais: um motor totalmente determinístico
//! pareceria caótico só porque três carros quebraram.

use std::collections::{HashMap, HashSet};

use crate::constants::scoring::{get_points_for_position, BONUS_FASTEST_LAP};
use crate::simulation::context::SimDriver;
use crate::simulation::race::RaceResult;

// ---------------------------------------------------------------------------
// Estatística de apoio
// ---------------------------------------------------------------------------

/// Ranks 1..n de um vetor de valores (ordem crescente), com média nos empates.
fn ranks(valores: &[f64]) -> Vec<f64> {
    let n = valores.len();
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&a, &b| {
        valores[a]
            .partial_cmp(&valores[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut saida = vec![0.0_f64; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && (valores[indices[j + 1]] - valores[indices[i]]).abs() < f64::EPSILON {
            j += 1;
        }
        let rank_medio = ((i + j) as f64) / 2.0 + 1.0;
        for &idx in &indices[i..=j] {
            saida[idx] = rank_medio;
        }
        i = j + 1;
    }
    saida
}

fn pearson(a: &[f64], b: &[f64]) -> Option<f64> {
    if a.len() != b.len() || a.len() < 3 {
        return None;
    }
    let n = a.len() as f64;
    let ma = a.iter().sum::<f64>() / n;
    let mb = b.iter().sum::<f64>() / n;
    let mut cov = 0.0;
    let mut va = 0.0;
    let mut vb = 0.0;
    for (x, y) in a.iter().zip(b.iter()) {
        let dx = x - ma;
        let dy = y - mb;
        cov += dx * dy;
        va += dx * dx;
        vb += dy * dy;
    }
    if va <= f64::EPSILON || vb <= f64::EPSILON {
        return None;
    }
    Some(cov / (va.sqrt() * vb.sqrt()))
}

/// Correlação de postos de Spearman.
pub fn spearman(a: &[f64], b: &[f64]) -> Option<f64> {
    pearson(&ranks(a), &ranks(b))
}

fn media(valores: &[f64]) -> f64 {
    if valores.is_empty() {
        return f64::NAN;
    }
    valores.iter().sum::<f64>() / valores.len() as f64
}

fn desvio_padrao(valores: &[f64]) -> f64 {
    if valores.len() < 2 {
        return 0.0;
    }
    let m = media(valores);
    let soma: f64 = valores.iter().map(|v| (v - m).powi(2)).sum();
    (soma / (valores.len() - 1) as f64).sqrt()
}

// ---------------------------------------------------------------------------
// Métricas
// ---------------------------------------------------------------------------

/// O retrato de uma temporada. Uma linha da tabela do relatório.
#[derive(Debug, Clone)]
pub struct MetricasTemporada {
    /// Spearman entre a posição de grid e a de chegada, média sobre as etapas.
    /// Alto = a classificação já entregou o resultado.
    pub spearman_grid_chegada: f64,
    /// Fração de etapas vencidas por quem largou na pole.
    pub pct_vitorias_do_pole: f64,
    /// Vencedores distintos na temporada.
    pub vencedores_distintos: usize,
    /// Desvio-padrão da posição de chegada de um piloto ao longo da temporada, média do grid.
    /// Baixo = todo mundo termina sempre no mesmo lugar.
    pub desvio_posicao: f64,
    /// Probabilidade do melhor piloto do grid terminar fora do top 5 numa etapa.
    pub p_melhor_fora_top5: f64,
    /// Fração da temporada percorrida quando o campeonato ficou matematicamente decidido.
    /// 1.0 = foi decidido só na última (ou não foi decidido antes).
    ///
    /// **CONTEXTO, NÃO SINAL DE SAÚDE.** Esta métrica passa numa simulação completamente travada,
    /// por aritmética da tabela de pontos: com 12 etapas e 26/18 pontos, quem vence tudo abre 8
    /// por corrida sobre um segundo constante, e 8k > (12−k)·26 só a partir de k = 10 — ~83% da
    /// temporada, exatamente o que se mede num campeonato de um vencedor só. Ela foi aposentada
    /// como alvo; quem responde por saúde de campeonato são as duas abaixo.
    pub fracao_decisao_campeonato: f64,
    /// Quantas vezes a LIDERANÇA do campeonato trocou de mão ao longo da temporada. Imune ao
    /// problema acima: numa simulação travada isto é zero, ponto final.
    pub trocas_de_lideranca: f64,
    /// Margem do campeão sobre o segundo, em fração dos pontos que UM piloto poderia somar na
    /// temporada inteira. Numa simulação travada é gigante; numa disputa de verdade é pequena.
    pub margem_do_campeao: f64,
    /// Spearman entre a ordem de chegada de etapas CONSECUTIVAS, média sobre os pares.
    /// É a métrica que mede diretamente o sintoma relatado ("mesma ordem toda etapa").
    pub spearman_etapas_consecutivas: f64,
    /// Abandonos por etapa (contexto — não entra nas correlações).
    pub dnfs_por_etapa: f64,
    /// **Safety cars por etapa** (pacote G). É uma SAÍDA DIFERENTE das correlações, e por isso
    /// entra aqui: um knob pode ter alavanca forte na frequência de SC e nenhuma em ρ.
    pub scs_por_etapa: f64,
    /// ρ(ordem no momento em que o SC entrou × chegada), média sobre os SCs da temporada. Mede o
    /// quanto o SC de fato embaralha. `NaN` quando não houve SC.
    pub rho_pre_sc_chegada: f64,
    pub etapas: usize,
    pub pilotos: usize,
}

/// Extrai (pilot_id, posição) de quem terminou a corrida.
fn chegadas(corrida: &RaceResult) -> HashMap<&str, f64> {
    corrida
        .race_results
        .iter()
        .filter(|r| !r.is_dnf)
        .map(|r| (r.pilot_id.as_str(), r.finish_position as f64))
        .collect()
}

pub fn medir_temporada(grid: &[SimDriver], corridas: &[RaceResult]) -> MetricasTemporada {
    let etapas = corridas.len();
    let melhor = super::campo::melhor_do_grid(grid);

    // --- Spearman grid × chegada, por etapa ---
    let mut correlacoes_grid = Vec::new();
    for corrida in corridas {
        let (grids, finais): (Vec<f64>, Vec<f64>) = corrida
            .race_results
            .iter()
            .filter(|r| !r.is_dnf)
            .map(|r| (r.grid_position as f64, r.finish_position as f64))
            .unzip();
        if let Some(rho) = spearman(&grids, &finais) {
            correlacoes_grid.push(rho);
        }
    }

    // --- Pole vira vitória? ---
    let vitorias_do_pole = corridas
        .iter()
        .filter(|c| !c.winner_id.is_empty() && c.winner_id == c.pole_sitter_id)
        .count();

    // --- Vencedores distintos ---
    let vencedores: HashSet<&str> = corridas
        .iter()
        .map(|c| c.winner_id.as_str())
        .filter(|id| !id.is_empty())
        .collect();

    // --- Desvio da posição de chegada por piloto ---
    let mut posicoes_por_piloto: HashMap<&str, Vec<f64>> = HashMap::new();
    for corrida in corridas {
        for r in corrida.race_results.iter().filter(|r| !r.is_dnf) {
            posicoes_por_piloto
                .entry(r.pilot_id.as_str())
                .or_default()
                .push(r.finish_position as f64);
        }
    }
    let desvios: Vec<f64> = posicoes_por_piloto
        .values()
        .filter(|v| v.len() >= 2)
        .map(|v| desvio_padrao(v))
        .collect();

    // --- Melhor piloto do grid fora do top 5 (DNF conta como fora) ---
    let mut aparicoes = 0_usize;
    let mut fora_top5 = 0_usize;
    for corrida in corridas {
        if let Some(r) = corrida.race_results.iter().find(|r| r.pilot_id == melhor) {
            aparicoes += 1;
            if r.is_dnf || r.finish_position > 5 {
                fora_top5 += 1;
            }
        }
    }

    // --- Spearman entre etapas consecutivas ---
    let mut correlacoes_consecutivas = Vec::new();
    for par in corridas.windows(2) {
        let a = chegadas(&par[0]);
        let b = chegadas(&par[1]);
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for (id, pos_a) in &a {
            if let Some(pos_b) = b.get(id) {
                xs.push(*pos_a);
                ys.push(*pos_b);
            }
        }
        if let Some(rho) = spearman(&xs, &ys) {
            correlacoes_consecutivas.push(rho);
        }
    }

    // --- Total de abandonos ---
    let dnfs: i32 = corridas.iter().map(|c| c.total_dnfs).sum();

    // --- Safety car: frequência e embaralhamento ---
    let scs: usize = corridas.iter().map(|c| c.safety_cars.len()).sum();
    let mut rhos_pre_sc = Vec::new();
    for corrida in corridas {
        let chegada: HashMap<&str, f64> = corrida
            .race_results
            .iter()
            .filter(|r| !r.is_dnf)
            .map(|r| (r.pilot_id.as_str(), r.finish_position as f64))
            .collect();
        for ordem in &corrida.ordem_pre_safety_car {
            // A ordem pré-SC é uma lista posicional: índice 0 = líder no momento da entrada.
            let mut antes = Vec::new();
            let mut depois = Vec::new();
            for (indice, id) in ordem.iter().enumerate() {
                if let Some(pos) = chegada.get(id.as_str()) {
                    antes.push(indice as f64 + 1.0);
                    depois.push(*pos);
                }
            }
            if let Some(rho) = spearman(&antes, &depois) {
                rhos_pre_sc.push(rho);
            }
        }
    }

    let campeonato = medir_campeonato(corridas);

    MetricasTemporada {
        spearman_grid_chegada: media(&correlacoes_grid),
        pct_vitorias_do_pole: if etapas == 0 {
            f64::NAN
        } else {
            vitorias_do_pole as f64 / etapas as f64
        },
        vencedores_distintos: vencedores.len(),
        desvio_posicao: media(&desvios),
        p_melhor_fora_top5: if aparicoes == 0 {
            f64::NAN
        } else {
            fora_top5 as f64 / aparicoes as f64
        },
        fracao_decisao_campeonato: campeonato.fracao_decisao,
        trocas_de_lideranca: campeonato.trocas_de_lideranca,
        margem_do_campeao: campeonato.margem_do_campeao,
        spearman_etapas_consecutivas: media(&correlacoes_consecutivas),
        dnfs_por_etapa: if etapas == 0 {
            f64::NAN
        } else {
            dnfs as f64 / etapas as f64
        },
        scs_por_etapa: if etapas == 0 {
            f64::NAN
        } else {
            scs as f64 / etapas as f64
        },
        rho_pre_sc_chegada: media(&rhos_pre_sc),
        etapas,
        pilotos: grid.len(),
    }
}

/// As três leituras de campeonato, num único passe sobre a tabela acumulada.
struct RetratoCampeonato {
    fracao_decisao: f64,
    trocas_de_lideranca: f64,
    margem_do_campeao: f64,
}

fn medir_campeonato(corridas: &[RaceResult]) -> RetratoCampeonato {
    let etapas = corridas.len();
    if etapas == 0 {
        return RetratoCampeonato {
            fracao_decisao: f64::NAN,
            trocas_de_lideranca: f64::NAN,
            margem_do_campeao: f64::NAN,
        };
    }
    let maximo_por_etapa = (get_points_for_position(1, false) + BONUS_FASTEST_LAP) as f64;
    let teto_da_temporada = maximo_por_etapa * etapas as f64;

    let mut acumulado: HashMap<String, f64> = HashMap::new();
    let mut fracao_decisao = 1.0;
    let mut decidido = false;
    let mut lider_anterior: Option<String> = None;
    let mut trocas = 0.0;
    let mut margem = 0.0;

    for (indice, corrida) in corridas.iter().enumerate() {
        for r in &corrida.race_results {
            *acumulado.entry(r.pilot_id.clone()).or_insert(0.0) += r.points_earned as f64;
        }

        // Classificação da vez. Desempate por id só para a liderança não oscilar por sorte de
        // ordenação instável quando dois pilotos empatam em pontos.
        let mut tabela: Vec<(&String, &f64)> = acumulado.iter().collect();
        tabela.sort_by(|a, b| {
            b.1.partial_cmp(a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
        });

        if let Some((lider, _)) = tabela.first() {
            if let Some(anterior) = &lider_anterior {
                if anterior != *lider {
                    trocas += 1.0;
                }
            }
            lider_anterior = Some((*lider).clone());
        }

        if tabela.len() >= 2 {
            let vantagem = tabela[0].1 - tabela[1].1;
            if !decidido {
                let restantes = (etapas - indice - 1) as f64;
                if vantagem > restantes * maximo_por_etapa {
                    fracao_decisao = (indice + 1) as f64 / etapas as f64;
                    decidido = true;
                }
            }
            margem = vantagem;
        }
    }

    RetratoCampeonato {
        fracao_decisao,
        trocas_de_lideranca: trocas,
        margem_do_campeao: if teto_da_temporada > 0.0 {
            margem / teto_da_temporada
        } else {
            f64::NAN
        },
    }
}

// ---------------------------------------------------------------------------
// Agregação sobre muitas temporadas
// ---------------------------------------------------------------------------

/// Média das métricas sobre N temporadas, mais o intervalo observado das duas mais sensíveis.
/// É o que as asserções olham: uma temporada solta pode ser azarada, o agregado não.
#[derive(Debug, Clone)]
pub struct MetricasAgregadas {
    pub rotulo: String,
    pub temporadas: usize,
    pub corridas_totais: usize,
    pub spearman_grid_chegada: f64,
    pub pct_vitorias_do_pole: f64,
    pub vencedores_distintos: f64,
    pub desvio_posicao: f64,
    pub p_melhor_fora_top5: f64,
    pub fracao_decisao_campeonato: f64,
    pub trocas_de_lideranca: f64,
    pub margem_do_campeao: f64,
    pub spearman_etapas_consecutivas: f64,
    pub scs_por_etapa: f64,
    pub rho_pre_sc_chegada: f64,
    pub dnfs_por_etapa: f64,
    /// Menor e maior número de vencedores distintos visto entre as temporadas.
    pub vencedores_min: usize,
    pub vencedores_max: usize,
    /// Fração de temporadas em que a liderança do campeonato NUNCA trocou de mão.
    pub temporadas_sem_troca_de_lideranca: f64,
}

pub fn agregar(rotulo: &str, temporadas: &[MetricasTemporada]) -> MetricasAgregadas {
    let campo = |f: fn(&MetricasTemporada) -> f64| -> f64 {
        let valores: Vec<f64> = temporadas.iter().map(f).filter(|v| v.is_finite()).collect();
        media(&valores)
    };

    MetricasAgregadas {
        rotulo: rotulo.to_string(),
        temporadas: temporadas.len(),
        corridas_totais: temporadas.iter().map(|m| m.etapas).sum(),
        spearman_grid_chegada: campo(|m| m.spearman_grid_chegada),
        pct_vitorias_do_pole: campo(|m| m.pct_vitorias_do_pole),
        vencedores_distintos: campo(|m| m.vencedores_distintos as f64),
        desvio_posicao: campo(|m| m.desvio_posicao),
        p_melhor_fora_top5: campo(|m| m.p_melhor_fora_top5),
        fracao_decisao_campeonato: campo(|m| m.fracao_decisao_campeonato),
        trocas_de_lideranca: campo(|m| m.trocas_de_lideranca),
        margem_do_campeao: campo(|m| m.margem_do_campeao),
        spearman_etapas_consecutivas: campo(|m| m.spearman_etapas_consecutivas),
        dnfs_por_etapa: campo(|m| m.dnfs_por_etapa),
        scs_por_etapa: campo(|m| m.scs_por_etapa),
        rho_pre_sc_chegada: campo(|m| m.rho_pre_sc_chegada),
        temporadas_sem_troca_de_lideranca: if temporadas.is_empty() {
            f64::NAN
        } else {
            temporadas
                .iter()
                .filter(|m| m.trocas_de_lideranca == 0.0)
                .count() as f64
                / temporadas.len() as f64
        },
        vencedores_min: temporadas
            .iter()
            .map(|m| m.vencedores_distintos)
            .min()
            .unwrap_or(0),
        vencedores_max: temporadas
            .iter()
            .map(|m| m.vencedores_distintos)
            .max()
            .unwrap_or(0),
    }
}
