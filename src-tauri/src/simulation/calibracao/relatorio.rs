//! Formatação dos relatórios em texto puro: resultado, orçamento de variância, processo e
//! varredura de sensibilidade.
//!
//! Existe para que o número medido seja copiável para dentro de um documento sem ninguém ter que
//! reler o código para saber o que cada coluna significa. Os testes `#[ignore]` de geração
//! chamam isto.

use super::alvos::{Alvos, Faixa};
use super::metricas::MetricasAgregadas;
use super::processo::{MetricasProcessoAgregadas, PoderDaLargada};
use super::variancia::OrcamentoVariancia;
use super::varredura::Varredura;

fn linha(nome: &str, valor: f64, faixa: Faixa, formato_pct: bool) -> String {
    let (valor_txt, alvo_txt) = if formato_pct {
        (
            format!("{:.1}%", valor * 100.0),
            format!("{:.0}%–{:.0}%", faixa.min * 100.0, faixa.max * 100.0),
        )
    } else {
        (
            format!("{valor:.2}"),
            format!("{:.2}–{:.2}", faixa.min, faixa.max),
        )
    };
    format!(
        "| {:<44} | {:>9} | {:>13} | {:<8} |",
        nome,
        valor_txt,
        alvo_txt,
        faixa.veredito(valor)
    )
}

fn cabecalho(colunas: [&str; 4], larguras: [usize; 4]) -> String {
    let mut s = format!(
        "| {:<w0$} | {:>w1$} | {:>w2$} | {:<w3$} |\n",
        colunas[0],
        colunas[1],
        colunas[2],
        colunas[3],
        w0 = larguras[0],
        w1 = larguras[1],
        w2 = larguras[2],
        w3 = larguras[3],
    );
    s.push_str(&format!(
        "|{:-<w0$}|{:-<w1$}|{:-<w2$}|{:-<w3$}|\n",
        "",
        "",
        "",
        "",
        w0 = larguras[0] + 2,
        w1 = larguras[1] + 2,
        w2 = larguras[2] + 2,
        w3 = larguras[3] + 2,
    ));
    s
}

// ---------------------------------------------------------------------------
// Resultado
// ---------------------------------------------------------------------------

/// Tabela de métricas de resultado de uma categoria.
pub fn tabela(m: &MetricasAgregadas, alvos: &Alvos) -> String {
    let mut saida = format!(
        "\n### {} — {} temporadas, {} corridas\n\n",
        m.rotulo, m.temporadas, m.corridas_totais
    );
    saida.push_str(&cabecalho(
        ["Métrica", "Medido", "Alvo", "Status"],
        [44, 9, 13, 8],
    ));

    for l in [
        linha(
            "Spearman grid × chegada",
            m.spearman_grid_chegada,
            alvos.spearman_grid_chegada,
            false,
        ),
        linha(
            "Vitórias do pole",
            m.pct_vitorias_do_pole,
            alvos.pct_vitorias_do_pole,
            true,
        ),
        linha(
            "Vencedores distintos por temporada",
            m.vencedores_distintos,
            alvos.vencedores_distintos,
            false,
        ),
        linha(
            "Desvio-padrão da posição de chegada",
            m.desvio_posicao,
            alvos.desvio_posicao,
            false,
        ),
        linha(
            "P(melhor do grid fora do top 5)",
            m.p_melhor_fora_top5,
            alvos.p_melhor_fora_top5,
            true,
        ),
        linha(
            "Spearman etapa N × etapa N+1",
            m.spearman_etapas_consecutivas,
            alvos.spearman_etapas_consecutivas,
            false,
        ),
        linha(
            "Trocas de liderança no campeonato",
            m.trocas_de_lideranca,
            alvos.trocas_de_lideranca,
            false,
        ),
        linha(
            "Margem do campeão (% dos disponíveis)",
            m.margem_do_campeao,
            alvos.margem_do_campeao,
            true,
        ),
    ] {
        saida.push_str(&l);
        saida.push('\n');
    }

    saida.push_str(&format!(
        "\nContexto (não é sinal de saúde): título decidido a {:.0}% da temporada; \
         {:.0}% das temporadas sem NENHUMA troca de liderança; vencedores distintos de {} a {}; \
         {:.2} abandonos por etapa.\n",
        m.fracao_decisao_campeonato * 100.0,
        m.temporadas_sem_troca_de_lideranca * 100.0,
        m.vencedores_min,
        m.vencedores_max,
        m.dnfs_por_etapa
    ));
    saida
}

pub fn relatorio(blocos: &[(MetricasAgregadas, Alvos)]) -> String {
    let mut saida = String::from("\n== BASELINE DE CALIBRAÇÃO — MÉTRICAS DE RESULTADO ==\n");
    for (m, a) in blocos {
        saida.push_str(&tabela(m, a));
    }
    saida
}

// ---------------------------------------------------------------------------
// Orçamento de variância
// ---------------------------------------------------------------------------

pub fn tabela_variancia(o: &OrcamentoVariancia) -> String {
    let mut saida = format!(
        "\n### Orçamento de variância — {} ({} corridas)\n\n",
        o.rotulo, o.corridas
    );
    saida.push_str(&format!(
        "| {:<40} | {:>9} |\n|{:-<42}|{:-<11}|\n",
        "Fonte", "% da var.", "", ""
    ));

    for (nome, frac) in [
        ("Piloto (permanente)", o.frac_piloto),
        ("Equipe / carro (permanente)", o.frac_carro),
        ("Evento — pista (afinidade)", o.frac_evento_pista),
        ("Evento — clima + forma + acerto", o.frac_evento_clima),
        ("Corrida (ruído puro)", o.frac_corrida),
    ] {
        saida.push_str(&format!("| {nome:<40} | {:>8.1}% |\n", frac * 100.0));
    }

    saida.push_str(
        "\nA divisão piloto:carro dentro do permanente é medida num grid de encaixe INDEPENDENTE\n\
         (bom piloto NÃO vai preferencialmente para o bom carro). No grid realista os dois são\n\
         correlacionados e nenhum congelamento isolado separa a covariância.\n\
         A linha `clima + forma + acerto` é o que SOBRA ao fixar a pista, e é um agregado de três\n\
         fontes — quebrá-lo exige que as escalas de `simulation::forma` sejam injetáveis.\n",
    );

    saida.push_str(&format!(
        "\nVariância total medida: {:.2} posição² (teórica p/ {} postos uniformes: {:.2}).\n",
        o.var_total,
        (o.var_total_teorica * 12.0 + 1.0).sqrt().round() as i64,
        o.var_total_teorica
    ));
    saida.push_str(&format!(
        "\nDuas vias para a fatia PERMANENTE:\n\
         - via ANOVA (variância entre pilotos):        {:.3}\n\
         - via ρ (chegadas de eventos diferentes):     {:.3}\n\
         - divergência entre as vias:                  {:.3}\n\
         Referência do mesmo fim de semana, ρ(grid × chegada) = {:.3} \
         — carrega permanente + evento, e por isso TEM que ser maior.\n",
        o.permanente_via_anova,
        o.permanente_via_rho,
        o.divergencia_das_vias(),
        o.permanente_mais_evento_via_rho
    ));

    // O diagnóstico da classificação: eixo diferente ou loteria?
    let reprodutibilidade_relativa = if o.permanente_via_rho.abs() > f64::EPSILON {
        o.reprodutibilidade_do_grid / o.permanente_via_rho
    } else {
        f64::NAN
    };
    saida.push_str(&format!(
        "\nClassificação:\n\
         - reprodutibilidade do GRID entre eventos:    {:.3}\n\
         - reprodutibilidade da CHEGADA entre eventos: {:.3}\n\
         - razão (grid / chegada):                     {:.3}  → {}\n",
        o.reprodutibilidade_do_grid,
        o.permanente_via_rho,
        reprodutibilidade_relativa,
        if !reprodutibilidade_relativa.is_finite() {
            "?"
        } else if reprodutibilidade_relativa >= 0.90 {
            "quali é um EIXO estável (só mede outra coisa)"
        } else if reprodutibilidade_relativa >= 0.75 {
            "quali é eixo próprio com ruído relevante"
        } else {
            "quali virou LOTERIA — o grid muda sem que o piloto mude"
        }
    ));
    saida
}

// ---------------------------------------------------------------------------
// Processo
// ---------------------------------------------------------------------------

pub fn tabela_processo(p: &MetricasProcessoAgregadas) -> String {
    let mut saida = format!(
        "\n### Processo — {} ({} corridas)\n\n",
        p.rotulo, p.corridas
    );
    saida.push_str(&format!(
        "| {:<44} | {:>9} |\n|{:-<46}|{:-<11}|\n",
        "Métrica", "Medido", "", ""
    ));
    for (nome, valor) in [
        ("Trocas de posição (dist. de Kendall)", p.trocas),
        (
            "Trocas normalizadas (0=nenhuma, .5=aleatório)",
            p.trocas_normalizadas,
        ),
        ("Posições ganhas/perdidas — média |Δ|", p.ganho_medio_abs),
        ("Posições ganhas/perdidas — p90", p.ganho_p90),
        ("Maior ganho da corrida", p.maior_ganho),
        (
            "CV dos gaps entre carros consecutivos",
            p.cv_gaps_sucessivos,
        ),
        ("Maior buraco / buraco mediano", p.maior_buraco_relativo),
        ("Pelotões formados", p.pelotoes),
    ] {
        saida.push_str(&format!("| {nome:<44} | {valor:>9.2} |\n"));
    }
    saida
}

pub fn tabela_largada(p: &PoderDaLargada) -> String {
    format!(
        "\n### Poder da largada — {} ({} corridas)\n\n\
         | {:<48} | {:>7} |\n|{:-<50}|{:-<9}|\n\
         | {:<48} | {:>7.3} |\n\
         | {:<48} | {:>7.3} |\n\
         | {:<48} | {:>7.3} |\n\
         | {:<48} | {:>7.3} |\n",
        p.rotulo,
        p.corridas,
        "Medida",
        "ρ",
        "",
        "",
        "ρ(grid × chegada), grid da classificação",
        p.rho_grid_normal,
        "ρ(grid × chegada), grid SORTEADO",
        p.rho_grid_sorteado,
        "ρ(skill × chegada), grid SORTEADO",
        p.rho_skill_com_grid_sorteado,
        "trocas normalizadas com grid sorteado",
        p.trocas_normalizadas_sorteado,
    )
}

// ---------------------------------------------------------------------------
// Varredura
// ---------------------------------------------------------------------------

/// **Matriz knob × saída.** É a forma certa do veredito: "morto" nunca é propriedade só do knob.
/// A tabela consolidada abaixo mede uma saída por vez e chamava `incident_rate_multiplier` de
/// fraco — mas frequência de safety car é outra saída, e é ali que ele pode ter alavanca.
pub fn matriz_de_alavanca(varreduras: &[Varredura]) -> String {
    use super::varredura::Saida;
    let saidas = Saida::todas();

    let mut saida = String::from("\n### Alavanca por par (knob × saída)\n\n");
    saida.push_str(&format!("| {:<34} |", "Knob"));
    for s in &saidas {
        saida.push_str(&format!(" {:>13} |", s.nome()));
    }
    saida.push_str(&format!("\n|{:-<36}|", ""));
    for _ in &saidas {
        saida.push_str(&format!("{:-<15}|", ""));
    }
    saida.push('\n');

    for v in varreduras {
        saida.push_str(&format!("| {:<34} |", v.knob.nome()));
        for s in &saidas {
            let a = v.alavanca(*s);
            let marca = match v.veredito_de(*s) {
                "ALAVANCA" => "**",
                "fraco" => " ",
                _ => " ",
            };
            if a.is_finite() {
                saida.push_str(&format!(" {a:>11.4}{marca} |"));
            } else {
                saida.push_str(&format!(" {:>13} |", "—"));
            }
        }
        saida.push('\n');
    }

    saida.push_str(
        "\n`**` = ALAVANCA para aquela saída. Um knob só é MORTO quando é morto em TODAS as \
         colunas.\nLimiar por saída (a escala importa): 0,02 em ρ e em SC/etapa; 0,30 em desvio de \
         posição e em vencedores.\n",
    );
    saida
}

/// Tabela consolidada: um knob por linha, com a amplitude que ele consegue percorrer.
///
/// A coluna **Onde** existe porque sem ela a tabela mente por omissão: o `incident_rate_multiplier`
/// aparece com Δρ = 0,022 ao lado de um veredito `ALAVANCA`, e quem lê não tem como saber que o
/// veredito foi ganho em `SC/etapa` e `DNF/etapa`, não em ρ. O veredito é consolidado sobre as seis
/// saídas; a amplitude mostrada é de UMA. Dizer em qual coluna ele vive fecha a lacuna.
pub fn tabela_varreduras(varreduras: &[Varredura]) -> String {
    let mut saida = String::from("\n### Alavanca dos knobs existentes\n\n");
    // Cabeçalho escrito à mão: o `cabecalho` é de aridade fixa em 4 e esta tabela tem 5 colunas.
    saida.push_str(&format!(
        "| {:<36} | {:>13} | {:>10} | {:<8} | {:<34} |\n|{:-<38}|{:-<15}|{:-<12}|{:-<10}|{:-<36}|\n",
        "Knob", "Faixa", "Δρ(N,N+1)", "Veredito", "Onde", "", "", "", "", ""
    ));

    for v in varreduras {
        let primeiro = v.pontos.first().map(|p| p.valor).unwrap_or(f64::NAN);
        let ultimo = v.pontos.last().map(|p| p.valor).unwrap_or(f64::NAN);
        let onde: Vec<&str> = v.saidas_com_alavanca().iter().map(|s| s.nome()).collect();
        let onde = if onde.is_empty() {
            "—".to_string()
        } else {
            onde.join(", ")
        };
        saida.push_str(&format!(
            "| {:<36} | {:>13} | {:>10.4} | {:<8} | {:<34} |\n",
            v.knob.nome(),
            format!("{primeiro:.2}–{ultimo:.2}"),
            v.alavanca_consecutivas(),
            v.veredito(),
            onde
        ));
    }
    saida.push_str(
        "\nΔρ(N,N+1) = amplitude percorrida pela correlação entre etapas consecutivas ao longo de \
         toda a faixa varrida — UMA das seis saídas.\nO veredito é consolidado sobre TODAS elas, e \
         `Onde` lista em quais ele é `ALAVANCA`. `MORTO` = abaixo do limiar nas seis: invisível \
         para o jogador em qualquer valor, em qualquer saída medida.\n",
    );
    saida
}

// ---------------------------------------------------------------------------
// Busca
// ---------------------------------------------------------------------------

/// O relatório da busca, com os cinco requisitos do caminho de fracasso na ordem em que importam.
/// O veredito vem PRIMEIRO, de propósito: quem lê tem que bater no "FALHOU" antes de bater no
/// melhor ponto, senão o melhor ponto é lido como resultado.
pub fn tabela_busca(r: &super::busca::RelatorioBusca) -> String {
    let mut saida = format!(
        "\n### Busca — {} ({} de {} avaliações)\n\n",
        r.rotulo, r.avaliacoes, r.teto
    );

    saida.push_str(if r.fracassou {
        ">>> VEREDITO: FALHOU. Não há ponto ótimo a devolver.\n\n"
    } else {
        ">>> VEREDITO: alvo atingível no espaço varrido.\n\n"
    });

    for linha in &r.diagnostico {
        saida.push_str(&format!("{linha}\n\n"));
    }

    saida.push_str(&cabecalho(
        ["Métrica", "no ótimo", "melhor de todos", "veredito"],
        [32, 10, 16, 13],
    ));
    for (i, (nome, dist, valor)) in r.distancias_do_melhor.iter().enumerate() {
        let (_, melhor_d, melhor_v) = r.melhor_por_metrica[i];
        let veredito = r.vereditos[i].1.rotulo();
        saida.push_str(&format!(
            "| {nome:<32} | {valor:>9.3} | {melhor_v:>8.3} (d={melhor_d:>4.2}) | {veredito:<13} |\n"
        ));
        let _ = dist;
    }

    if !r.falhas_de_orcamento.is_empty() {
        saida.push_str("\nOrçamento de variância no ponto final (portão obrigatório):\n");
        for f in &r.falhas_de_orcamento {
            saida.push_str(&format!("  FORA — {f}\n"));
        }
    }

    saida.push_str("\nPonto de melhor agregado:\n");
    for (knob, valor) in &r.melhor_ponto {
        let borda = if r.otimos_na_borda.contains(knob) {
            "  <- NA BORDA"
        } else {
            ""
        };
        saida.push_str(&format!("  {knob} = {valor:.2}{borda}\n"));
    }

    saida.push_str(
        "\n'melhor de todos' = o melhor valor que a métrica atingiu em QUALQUER ponto avaliado, \
         mesmo pontos\nque perderam no agregado. Responde 'é alcançável de todo?', que é pergunta \
         diferente de\n'o ótimo a atinge?'.\n",
    );
    saida
}

/// Detalhe ponto a ponto de uma varredura — para quando o veredito consolidado não basta.
pub fn detalhe_varredura(v: &Varredura) -> String {
    let mut saida = format!("\n#### {} ({})\n\n", v.knob.nome(), v.categoria);
    saida.push_str(&cabecalho(
        ["valor", "ρ(N,N+1)", "desvio pos.", "vencedores"],
        [10, 10, 12, 10],
    ));
    for p in &v.pontos {
        saida.push_str(&format!(
            "| {:<10.2} | {:>10.4} | {:>12.2} | {:>10.2} |\n",
            p.valor,
            p.metricas.spearman_etapas_consecutivas,
            p.metricas.desvio_posicao,
            p.metricas.vencedores_distintos
        ));
    }
    saida
}
