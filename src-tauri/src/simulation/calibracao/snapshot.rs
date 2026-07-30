//! **Baseline congelado** — os números de referência, versionados em código, para diff antes/depois.
//!
//! Todo pacote daqui pra frente (B, C, D, E, F, G) vai ser julgado contra este retrato. Ele está
//! aqui como constante literal, e não como arquivo gerado, por três razões: entra no diff do git
//! junto com a mudança que o alterou, não depende de I/O dentro de teste, e não tem como
//! silenciosamente "atualizar sozinho" numa rodada.
//!
//! **Regra**: não edite [`CONGELADO`] para fazer um teste passar. Ele só muda quando alguém
//! decide, conscientemente, que o novo comportamento é o novo normal — e aí a linha do diff é a
//! evidência dessa decisão.
//!
//! Uso:
//!
//! ```text
//! cargo test --release --manifest-path src-tauri/Cargo.toml calibracao::tests::compara_com_congelado -- --ignored --nocapture
//! ```

use super::metricas::MetricasAgregadas;

/// Uma linha do baseline congelado: rótulo do cenário + as sete métricas de resultado + as duas
/// de campeonato que substituíram a etapa-de-decisão.
#[derive(Debug, Clone, Copy)]
pub struct LinhaCongelada {
    pub rotulo: &'static str,
    pub spearman_grid_chegada: f64,
    pub pct_vitorias_do_pole: f64,
    pub vencedores_distintos: f64,
    pub desvio_posicao: f64,
    pub p_melhor_fora_top5: f64,
    pub spearman_etapas_consecutivas: f64,
    pub trocas_de_lideranca: f64,
    pub margem_do_campeao: f64,
}

/// Configuração que produziu [`CONGELADO`]. Qualquer comparação tem que usar exatamente estes
/// números, senão o diff não significa nada.
pub const TEMPORADAS: usize = 84;
pub const PILOTOS: usize = 20;
pub const ETAPAS: usize = 12;
pub const SEMENTE_ROOKIE: u64 = 2026;
pub const SEMENTE_GT3: u64 = 2027;

/// Medido em 2026-07-29, antes dos pacotes B/C/D/E/F/G. 1008 corridas por linha.
pub const CONGELADO: &[LinhaCongelada] = &[
    LinhaCongelada {
        rotulo: "mazda_rookie (sem incidentes)",
        spearman_grid_chegada: 0.9358,
        pct_vitorias_do_pole: 0.7560,
        vencedores_distintos: 1.2976,
        desvio_posicao: 0.7139,
        p_melhor_fora_top5: 0.0129,
        spearman_etapas_consecutivas: 0.9763,
        trocas_de_lideranca: 0.2262,
        margem_do_campeao: 0.2652,
    },
    LinhaCongelada {
        rotulo: "mazda_rookie (com incidentes)",
        spearman_grid_chegada: 0.9360,
        pct_vitorias_do_pole: 0.7421,
        vencedores_distintos: 1.4286,
        desvio_posicao: 0.8927,
        p_melhor_fora_top5: 0.0288,
        spearman_etapas_consecutivas: 0.9749,
        trocas_de_lideranca: 0.4643,
        margem_do_campeao: 0.2576,
    },
    LinhaCongelada {
        rotulo: "gt3 (sem incidentes)",
        spearman_grid_chegada: 0.9857,
        pct_vitorias_do_pole: 0.8879,
        vencedores_distintos: 1.2976,
        desvio_posicao: 0.4485,
        p_melhor_fora_top5: 0.0397,
        spearman_etapas_consecutivas: 0.9891,
        trocas_de_lideranca: 0.2500,
        margem_do_campeao: 0.2500,
    },
    LinhaCongelada {
        rotulo: "gt3 (com incidentes)",
        spearman_grid_chegada: 0.9855,
        pct_vitorias_do_pole: 0.8819,
        vencedores_distintos: 1.3810,
        desvio_posicao: 0.5966,
        p_melhor_fora_top5: 0.0526,
        spearman_etapas_consecutivas: 0.9887,
        trocas_de_lideranca: 0.2976,
        margem_do_campeao: 0.2479,
    },
];

pub fn buscar(rotulo: &str) -> Option<&'static LinhaCongelada> {
    CONGELADO.iter().find(|l| l.rotulo == rotulo)
}

/// Emite a medição atual como literal Rust, pronto para colar em [`CONGELADO`].
///
/// Existe para que recongelar seja copiar-e-colar em vez de transcrever número a mão — transcrever
/// à mão introduz erro de arredondamento que depois aparece como falso delta no diff.
pub fn literal(m: &MetricasAgregadas) -> String {
    format!(
        "    LinhaCongelada {{\n\
         \x20       rotulo: {:?},\n\
         \x20       spearman_grid_chegada: {:.4},\n\
         \x20       pct_vitorias_do_pole: {:.4},\n\
         \x20       vencedores_distintos: {:.4},\n\
         \x20       desvio_posicao: {:.4},\n\
         \x20       p_melhor_fora_top5: {:.4},\n\
         \x20       spearman_etapas_consecutivas: {:.4},\n\
         \x20       trocas_de_lideranca: {:.4},\n\
         \x20       margem_do_campeao: {:.4},\n\
         \x20   }},\n",
        m.rotulo,
        m.spearman_grid_chegada,
        m.pct_vitorias_do_pole,
        m.vencedores_distintos,
        m.desvio_posicao,
        m.p_melhor_fora_top5,
        m.spearman_etapas_consecutivas,
        m.trocas_de_lideranca,
        m.margem_do_campeao,
    )
}

/// Tabela de diff: congelado → atual → delta, por métrica.
pub fn diff(atual: &MetricasAgregadas) -> String {
    let Some(base) = buscar(&atual.rotulo) else {
        return format!(
            "\n### {}\n(sem linha congelada com este rótulo — nada a comparar)\n",
            atual.rotulo
        );
    };

    let mut saida = format!("\n### {}\n\n", atual.rotulo);
    saida.push_str(&format!(
        "| {:<36} | {:>10} | {:>10} | {:>9} |\n",
        "Métrica", "Congelado", "Atual", "Δ"
    ));
    saida.push_str(&format!(
        "|{:-<38}|{:-<12}|{:-<12}|{:-<11}|\n",
        "", "", "", ""
    ));

    for (nome, antes, agora) in [
        (
            "Spearman grid × chegada",
            base.spearman_grid_chegada,
            atual.spearman_grid_chegada,
        ),
        (
            "Vitórias do pole",
            base.pct_vitorias_do_pole,
            atual.pct_vitorias_do_pole,
        ),
        (
            "Vencedores distintos",
            base.vencedores_distintos,
            atual.vencedores_distintos,
        ),
        (
            "Desvio-padrão da posição",
            base.desvio_posicao,
            atual.desvio_posicao,
        ),
        (
            "P(melhor fora do top 5)",
            base.p_melhor_fora_top5,
            atual.p_melhor_fora_top5,
        ),
        (
            "Spearman etapa N × N+1",
            base.spearman_etapas_consecutivas,
            atual.spearman_etapas_consecutivas,
        ),
        (
            "Trocas de liderança",
            base.trocas_de_lideranca,
            atual.trocas_de_lideranca,
        ),
        (
            "Margem do campeão",
            base.margem_do_campeao,
            atual.margem_do_campeao,
        ),
    ] {
        let delta = agora - antes;
        let marca = if delta.abs() < 0.005 { "" } else { " *" };
        saida.push_str(&format!(
            "| {nome:<36} | {antes:>10.3} | {agora:>10.3} | {delta:>+8.3}{marca} |\n"
        ));
    }
    saida
}
