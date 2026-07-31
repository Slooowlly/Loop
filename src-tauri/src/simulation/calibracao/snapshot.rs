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

/// **Âncora: `a905ca2`** — o primeiro ponto da história em que a reforma da simulação existe de
/// forma identificável (pacotes B, C, D, F, G + este harness), com 2043 testes verdes.
///
/// A âncora anterior era `d4c55e8`, medida com a régua ANTIGA e antes de qualquer conserto. Ela
/// deixou de servir por dois motivos somados, e é importante que os dois estejam registrados:
///
/// 1. **O motor mudou** — moeda em tempo, tráfego, forma, safety car, quali reescrita.
/// 2. **A régua estava errada** — o `campo.rs` derivava do talento sete atributos que o jogo
///    sorteia livres, e espalhava o skill da gt3 38% além do real. Números medidos com ela não são
///    comparáveis aos de agora, nem para melhor nem para pior.
///
/// Comparar contra a âncora velha mediria as duas mudanças somadas sem conseguir separá-las. Este
/// recongelamento é o novo zero.
///
/// ## RECONGELADO com a esteira ligada — a terceira e última correção de régua
///
/// Os números anteriores mediam o caminho `campo → run_full_race_with_breakdowns`, que **não passa
/// pela esteira de modificadores de fim de semana**. As três camadas de `forma` (afinidade, forma,
/// acerto) eram invisíveis para toda medição deste harness.
///
/// A esteira agora é [`crate::simulation::esteira::aplicar_esteira`] — pura, chamável, com as
/// escalas por parâmetro. O harness chama a função do jogo, `esteira_de_forma` é `true` por padrão,
/// e estes números medem o caminho que o jogo roda.
///
/// O que muda em relação ao congelado anterior é **efeito de mecanismo, não de conserto de régua**:
/// as camadas sempre estiveram lá no jogo; só não estavam aqui. O diff entre os dois congelados é o
/// tamanho do pacote B visto pela primeira vez.
///
/// | Métrica | antes (sem esteira) | agora | alvo rookie / gt3 |
/// |---|---|---|---|
/// | ρ(N,N+1) rookie | 0,828 | **0,776** | 0,20–0,55 |
/// | ρ(N,N+1) gt3 | 0,919 | **0,859** | 0,35–0,70 |
/// | desvio rookie | 2,19 | **2,57** | 3,5–6,5 |
/// | desvio gt3 | 1,48 | **2,03** | 2,5–5,0 |
/// | vencedores rookie | 2,13 | **2,83** | 5–10 |
/// | trocas de liderança gt3 | 0,93 | **1,50** ✅ | 1–5 |
///
/// **Este é o último recongelamento previsto antes da campanha.** As três correções de régua
/// (atributos do `campo.rs`, espalhamento do skill da gt3, esteira ausente) estão fechadas; o que
/// mover daqui em diante move por mudança de mecanismo ou de calibração, que é o que o diff deve
/// medir.
///
/// 1008 corridas por linha.
pub const CONGELADO: &[LinhaCongelada] = &[
    LinhaCongelada {
        rotulo: "mazda_rookie (sem incidentes)",
        spearman_grid_chegada: 0.8941,
        pct_vitorias_do_pole: 0.7163,
        vencedores_distintos: 2.8333,
        desvio_posicao: 2.5696,
        p_melhor_fora_top5: 0.0288,
        spearman_etapas_consecutivas: 0.7756,
        trocas_de_lideranca: 0.9762,
        margem_do_campeao: 0.2101,
    },
    LinhaCongelada {
        rotulo: "mazda_rookie (com incidentes)",
        spearman_grid_chegada: 0.8898,
        pct_vitorias_do_pole: 0.7044,
        vencedores_distintos: 2.7738,
        desvio_posicao: 2.5567,
        p_melhor_fora_top5: 0.0595,
        spearman_etapas_consecutivas: 0.7733,
        trocas_de_lideranca: 1.0952,
        margem_do_campeao: 0.2023,
    },
    LinhaCongelada {
        rotulo: "gt3 (sem incidentes)",
        spearman_grid_chegada: 0.9598,
        pct_vitorias_do_pole: 0.8056,
        vencedores_distintos: 2.6310,
        desvio_posicao: 2.0300,
        p_melhor_fora_top5: 0.0456,
        spearman_etapas_consecutivas: 0.8591,
        trocas_de_lideranca: 1.5000,
        margem_do_campeao: 0.1388,
    },
    LinhaCongelada {
        rotulo: "gt3 (com incidentes)",
        spearman_grid_chegada: 0.9584,
        pct_vitorias_do_pole: 0.7748,
        vencedores_distintos: 2.5595,
        desvio_posicao: 2.0299,
        p_melhor_fora_top5: 0.0615,
        spearman_etapas_consecutivas: 0.8588,
        trocas_de_lideranca: 1.2024,
        margem_do_campeao: 0.1530,
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
