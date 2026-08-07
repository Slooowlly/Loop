//! Os CINCO CANAIS de receita (seção 3.5 do redesign).
//!
//! Substitui o lado da receita de `commands::race::financas`, onde quatro canais são
//! frações do `round_operating_base` (`operacional_anual / rodadas`) e o quinto — o prêmio
//! de construtores — mora em `finance::prize`. Nada aqui é chamado pela simulação ainda: o
//! harness de economia dirige os dois modelos lado a lado.
//!
//! ## O que muda, e por quê
//!
//! **1. O calendário deixa de ser multiplicador escondido.** Hoje todo canal é fração de
//! `operacional_anual / rodadas`, então categoria de calendário curto tem cada rodada
//! inflada. Aqui o nível é declarado como TOTAL DE TEMPORADA e o calendário entra por um
//! expoente explícito ([`ParametrosDeReceita::expoente_do_calendario`]), que é o único lugar
//! onde o número de etapas mexe no dinheiro.
//!
//! **2. Sem realimentação de riqueza.** O patrocínio de hoje soma
//! `budget_index × base × 0,002`, e `budget_index` é derivado do dinheiro
//! (`derive_budget_index_from_money`): equipe rica capta mais patrocínio por ser rica. É o
//! laço de reforço positivo da seção 2.4, e ele não existe neste módulo. O patrocínio olha
//! reputação e fama do lineup — nada mais.
//!
//! **3. Volta mais rápida vira dinheiro.** `BONUS_FASTEST_LAP` continua sendo +1 ponto de
//! campeonato; o que nasce aqui é o prêmio em dinheiro, que hoje não existe.
//!
//! **4. A bilheteria é bolo de EVENTO.** Hoje o "bolo" é fração do custo de UMA equipe e
//! depois é dividido pelo grid inteiro, o que o torna minúsculo (0,3–0,6% da receita). Aqui
//! o bolo é do evento e a cota é que reparte.
//!
//! ## O requisito derivado do critério 6
//!
//! A medição do harness achou um corte limpo: **toda categoria com 8 ou mais etapas passa no
//! critério 6 (campeão ÷ lanterna ≥ 3×) e toda categoria abaixo falha**, sem exceção nos dois
//! sentidos — 5 etapas dão 1,67×–1,72×, 6 etapas dão 2,24×, 8 etapas dão 3,16×–3,40×.
//!
//! A causa não é o tamanho do grid: com a classe fixa em 6 equipes, 5 etapas dão 1,72×,
//! 6 dão 2,24× e 10 dão 3,03×. É o número de etapas mesmo. Com poucas corridas o campeonato
//! é dominado por ruído — quem termina em 1º não é muito melhor que quem termina em último,
//! então a diferença de dinheiro entre eles encolhe contra a parte plana da receita.
//!
//! Daí `expoente_do_calendario`: com 1,0 o prêmio total de temporada é o mesmo em toda a
//! escada; **acima de 1,0 o calendário curto recebe mais**, que é o que empurra a parte
//! variável da receita para cima onde o ruído a comprime.

/// Calendário de referência do parameterização. Com `expoente_do_calendario = 1,0`, o prêmio
/// total de temporada é o mesmo em qualquer categoria e este número não aparece em lugar
/// nenhum do resultado — ele só existe para o expoente girar em torno de um pivô.
///
/// 10 etapas é o meio da escada do Loop (5, 5, 8, 8, 8, 10, 10, 14, 6).
pub const ETAPAS_DE_REFERENCIA: f64 = 10.0;

/// Prestígio de evento que vale fator 1,0 na bilheteria. É o mesmo pivô que
/// `finance::cashflow` usa (`score / 60`), mantido para os dois modelos serem comparáveis.
pub const PRESTIGIO_NEUTRO: f64 = 60.0;

/// Os coeficientes dos cinco canais. **Todos são frações do custo operacional ANUAL da
/// categoria**, e todos os canais por-etapa são declarados como TOTAL DE TEMPORADA — é o que
/// tira o calendário de dentro de cada fórmula e o põe num lugar só.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParametrosDeReceita {
    // ── 1. Prêmio por etapa ──────────────────────────────────────────────────────────
    /// O que uma equipe MEDIANA arrecada em prêmio de corrida numa temporada inteira, em
    /// frações do custo operacional anual. É o canal principal.
    pub premio_de_corrida: f64,
    /// Como o prêmio de temporada responde ao tamanho do calendário.
    /// `1,0` = mesmo total em toda a escada. `> 1,0` = calendário curto recebe mais.
    /// Ver a discussão do critério 6 no topo do módulo.
    pub expoente_do_calendario: f64,
    /// Inclinação da curva por posição na classe: `peso(p) = (M + 1 − p)^γ`. `1,0` é linear
    /// (o 1º leva ~2× a média), `2,0` é convexo (o 1º leva ~3×). É o que converte uma
    /// vantagem pequena de ritmo numa vantagem grande de dinheiro.
    pub inclinacao_do_premio: f64,

    // ── 2. Volta mais rápida ─────────────────────────────────────────────────────────
    /// Total de temporada do prêmio de volta mais rápida, se a equipe levasse TODAS. Pequeno
    /// por desenho — é tempero, não canal.
    pub volta_mais_rapida: f64,

    // ── 3. Patrocínio ────────────────────────────────────────────────────────────────
    /// Parte fixa do contrato, por temporada. O piso que permite sobreviver.
    pub patrocinio_fixo: f64,
    /// Parte negociada por reputação da equipe (0–100).
    pub patrocinio_por_reputacao: f64,
    /// Parte negociada pela fama do lineup (0–100).
    pub patrocinio_por_fama: f64,

    // ── 4. Bilheteria ────────────────────────────────────────────────────────────────
    /// Bolo de bilheteria de uma temporada, por equipe do grid, num evento de prestígio
    /// neutro. O bolo do EVENTO é este número × o tamanho do grid.
    pub bilheteria: f64,
    /// Quanto do bolo é piso dividido igual (0) contra cota por atração de público (1).
    /// Com 0,0 a cota é 1/N para todos e o critério 11 é impossível por construção.
    pub bilheteria_piso: f64,
    /// Quanto o prestígio do evento escala o bolo: `(1 − w) + w × prestígio/neutro`.
    ///
    /// Existe porque o prestígio é fortemente **correlacionado com a categoria** — uma etapa
    /// de meio de Rookie pontua ~26 e uma final de Endurance ~89, medido. Com `w = 1,0` a
    /// bilheteria varia 3× ao longo da escada só por isso, e o critério 4 (10–20% em TODA
    /// categoria) fica impossível com um coeficiente só. `w` menor mantém o prestígio como
    /// diferenciador ENTRE etapas sem deixá-lo diferenciar categorias.
    pub bilheteria_por_prestigio: f64,

    // ── 5. Prêmio de fim de temporada ────────────────────────────────────────────────
    /// O que o ÚLTIMO da classe leva no fechamento, por temporada.
    pub fechamento_ao_ultimo: f64,
    /// O que o PRIMEIRO da classe leva no fechamento, por temporada.
    pub fechamento_ao_primeiro: f64,
}

impl Default for ParametrosDeReceita {
    /// **Calibrado no harness** (`varrer_receita`), sobre a âncora de fluxo nova e com o ralo
    /// ligado. Satisfaz simultaneamente os critérios 1, 3, 4, 5, 6 e 7 da Parte 4.
    ///
    /// Duas coisas que quem for mexer aqui precisa saber:
    ///
    /// **O nível está preso à despesa MEDIDA, não ao custo operacional declarado.** A
    /// simulação gasta 1,65–2,27× o `operating_cost_midpoint` da divisão (o lado da despesa
    /// ainda é o modelo antigo de `race/financas.rs`), então os coeficientes carregam esse
    /// fator ~1,96 embutido. Quando `economia::evento` e `economia::temporada` substituírem
    /// a despesa, **todo número deste bloco precisa ser dividido de novo** — o alvo volta a
    /// se mexer.
    ///
    /// **A inclinação 6,5 não é um exagero de balanceamento, é a amarra do critério 8.**
    /// Abaixo de 5 nenhuma equipe do mundo vai à falência (o critério 8 pede 0,5–5% do grid
    /// por temporada e mede 0,0%); acima de 8 o fundo do grid morre e ele estoura pela outra
    /// ponta. 6,5 é onde a curva de vendas tem o mínimo de categorias fora, e é também onde a
    /// crise fica em 6,7% com o colapso abaixo dela.
    fn default() -> Self {
        Self {
            premio_de_corrida: 0.981,
            expoente_do_calendario: 1.0,
            inclinacao_do_premio: 6.5,
            volta_mais_rapida: 0.020,
            patrocinio_fixo: 0.471,
            patrocinio_por_reputacao: 0.196,
            patrocinio_por_fama: 0.118,
            bilheteria: 0.294,
            bilheteria_piso: 1.0,
            bilheteria_por_prestigio: 0.30,
            fechamento_ao_ultimo: 0.010,
            fechamento_ao_primeiro: 0.118,
        }
    }
}

/// Como o tamanho do calendário mexe no prêmio de temporada.
///
/// Devolve 1,0 quando `etapas == ETAPAS_DE_REFERENCIA` ou quando o expoente é 1,0 — ou seja,
/// o expoente só faz alguma coisa quando o calendário foge do pivô.
pub fn fator_de_calendario(etapas: f64, expoente: f64) -> f64 {
    (ETAPAS_DE_REFERENCIA / etapas.max(1.0)).powf(expoente - 1.0)
}

/// Peso de uma posição na curva de prêmio. `posicao` é 1-based, dentro da CLASSE.
fn peso_da_posicao(posicao: u32, carros_na_classe: f64, inclinacao: f64) -> f64 {
    let m = carros_na_classe.max(1.0);
    let p = (posicao.max(1) as f64).min(m);
    (m + 1.0 - p).max(0.0).powf(inclinacao.max(0.0))
}

/// Soma dos pesos de todas as posições da classe — o normalizador da curva.
fn soma_dos_pesos(carros_na_classe: f64, inclinacao: f64) -> f64 {
    let m = carros_na_classe.max(1.0).round() as u32;
    (1..=m)
        .map(|p| peso_da_posicao(p, carros_na_classe, inclinacao))
        .sum::<f64>()
        .max(f64::EPSILON)
}

/// Tudo que os cinco canais precisam saber sobre uma etapa. Sem `Connection`, sem
/// `AppHandle`, sem tabela de categoria — o custo operacional anual entra como ARGUMENTO.
#[derive(Debug, Clone, PartialEq)]
pub struct EntradaDeReceitaDaEtapa {
    /// O custo de operar uma temporada nesta categoria. A unidade de todos os coeficientes.
    pub custo_operacional_anual: f64,
    /// Quantas etapas o calendário tem.
    pub etapas_na_temporada: f64,
    /// Quantos CARROS disputam a classe da equipe (não equipes — a posição é por carro).
    pub carros_na_classe: f64,
    /// Quantos carros a equipe inscreveu.
    pub carros_da_equipe: f64,
    /// Posição de chegada de cada carro da equipe DENTRO DA CLASSE (1 = vencedor da classe).
    /// Carro que abandonou entra com a posição que ocupou na classificação final.
    pub posicoes_na_classe: Vec<u32>,
    /// A equipe levou a volta mais rápida da corrida?
    pub volta_mais_rapida: bool,
    /// Reputação da equipe (0–100).
    pub reputacao: f64,
    /// Presença pública do lineup (0–100) — a fama dos pilotos. É o que o PATROCÍNIO
    /// negocia.
    pub presenca_publica: f64,
    /// Atração de público da equipe nesta etapa (0–100), de
    /// `public_presence::atracao::compute_team_audience_appeal`. É o que a BILHETERIA
    /// rateia, e é uma grandeza diferente da presença: ela carrega competitividade atual,
    /// vínculo local com a pista e história de títulos.
    ///
    /// A distinção não é preciosismo. A presença do lineup é comprimida por construção (a
    /// mídia vive numa faixa estreita); a atração espalha 2,7–5,1× dentro de um grid, medido
    /// em produção. Ratear bilheteria por presença é o que fazia a cota colapsar em 1/N.
    pub atracao_de_publico: f64,
    /// Soma da atração de TODAS as equipes do grid. O denominador da cota.
    pub atracao_total_do_grid: f64,
    /// Quantas equipes dividem a bilheteria do evento.
    pub equipes_no_grid: f64,
    /// Prestígio do evento (o `score` de `event_interest`, ~26 a ~89).
    pub prestigio_do_evento: f64,
}

/// A receita de UMA etapa, canal a canal. `total()` é a soma — nenhum campo é derivado de
/// outro, então nada pode divergir.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReceitaDaEtapa {
    pub premio_de_corrida: f64,
    pub volta_mais_rapida: f64,
    pub patrocinio: f64,
    pub bilheteria: f64,
}

impl ReceitaDaEtapa {
    pub fn total(&self) -> f64 {
        self.premio_de_corrida + self.volta_mais_rapida + self.patrocinio + self.bilheteria
    }
}

/// Os quatro canais por-etapa. O quinto (fechamento) é de temporada e mora em
/// [`premio_de_fim_de_temporada`].
pub fn receita_da_etapa(
    entrada: &EntradaDeReceitaDaEtapa,
    p: &ParametrosDeReceita,
) -> ReceitaDaEtapa {
    let op = entrada.custo_operacional_anual.max(0.0);
    let etapas = entrada.etapas_na_temporada.max(1.0);
    let fator = fator_de_calendario(etapas, p.expoente_do_calendario);

    // ── 1. Prêmio por etapa ──────────────────────────────────────────────────────────
    // `premio_de_corrida` é o total de temporada de uma equipe MEDIANA. O que um carro
    // mediano leva por corrida é isso dividido pelas etapas e pelos carros da equipe; a
    // curva por posição redistribui em torno dessa média sem mudar o total pago à classe.
    let carros_da_equipe = entrada.carros_da_equipe.max(1.0);
    let media_por_carro = op * p.premio_de_corrida * fator / etapas / carros_da_equipe;
    let m = entrada.carros_na_classe.max(1.0);
    let soma = soma_dos_pesos(m, p.inclinacao_do_premio);
    let premio: f64 = entrada
        .posicoes_na_classe
        .iter()
        .map(|&pos| media_por_carro * m * peso_da_posicao(pos, m, p.inclinacao_do_premio) / soma)
        .sum();

    // ── 2. Volta mais rápida ─────────────────────────────────────────────────────────
    let volta = if entrada.volta_mais_rapida {
        op * p.volta_mais_rapida * fator / etapas
    } else {
        0.0
    };

    // ── 3. Patrocínio ────────────────────────────────────────────────────────────────
    // Contrato: parte fixa + reputação + fama. NENHUM termo lê caixa, dívida ou índice
    // derivado de dinheiro — é o corte do laço da seção 2.4.
    let patrocinio_anual = op
        * (p.patrocinio_fixo
            + p.patrocinio_por_reputacao * entrada.reputacao.clamp(0.0, 100.0) / 100.0
            + p.patrocinio_por_fama * entrada.presenca_publica.clamp(0.0, 100.0) / 100.0);

    // ── 4. Bilheteria ────────────────────────────────────────────────────────────────
    // O bolo é do EVENTO: o que uma equipe média levaria numa temporada, multiplicado pelo
    // grid. A cota reparte entre piso igual e atração de público.
    let n = entrada.equipes_no_grid.max(1.0);
    let w = p.bilheteria_por_prestigio.clamp(0.0, 1.0);
    let fator_prestigio =
        ((1.0 - w) + w * (entrada.prestigio_do_evento / PRESTIGIO_NEUTRO)).max(0.0);
    let bolo = op * p.bilheteria * n * fator_prestigio / etapas;
    let piso = p.bilheteria_piso.clamp(0.0, 1.0);
    let fatia_por_atracao = if entrada.atracao_total_do_grid > 0.0 {
        entrada.atracao_de_publico.max(0.0) / entrada.atracao_total_do_grid
    } else {
        1.0 / n
    };
    let cota = (1.0 - piso) / n + piso * fatia_por_atracao;

    ReceitaDaEtapa {
        premio_de_corrida: premio,
        volta_mais_rapida: volta,
        patrocinio: patrocinio_anual / etapas,
        bilheteria: bolo * cota,
    }
}

/// O quinto canal: o prêmio de fim de temporada, pago uma vez, por posição no campeonato da
/// CLASSE. Bônus, não muleta — o alvo da Parte 4 é ≤ 10% da receita.
///
/// `colocacao` é 1-based; `equipes_na_classe` é quantas disputam aquele campeonato.
pub fn premio_de_fim_de_temporada(
    custo_operacional_anual: f64,
    colocacao: u32,
    equipes_na_classe: u32,
    p: &ParametrosDeReceita,
) -> f64 {
    let n = equipes_na_classe.max(1) as f64;
    let pos = (colocacao.max(1) as f64).min(n);
    // 1,0 para o primeiro, 0,0 para o último. Classe de uma equipe só leva o topo.
    let relativo = if n <= 1.0 { 1.0 } else { (n - pos) / (n - 1.0) };
    custo_operacional_anual.max(0.0)
        * (p.fechamento_ao_ultimo + (p.fechamento_ao_primeiro - p.fechamento_ao_ultimo) * relativo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entrada(posicoes: Vec<u32>) -> EntradaDeReceitaDaEtapa {
        EntradaDeReceitaDaEtapa {
            custo_operacional_anual: 1_000_000.0,
            etapas_na_temporada: 10.0,
            carros_na_classe: 20.0,
            carros_da_equipe: 2.0,
            posicoes_na_classe: posicoes,
            volta_mais_rapida: false,
            reputacao: 50.0,
            presenca_publica: 45.0,
            atracao_de_publico: 45.0,
            atracao_total_do_grid: 450.0,
            equipes_no_grid: 10.0,
            prestigio_do_evento: PRESTIGIO_NEUTRO,
        }
    }

    /// O nível é declarado como total de TEMPORADA por equipe MÉDIA, e com
    /// `expoente_do_calendario = 1,0` o calendário não se mete: uma classe de 10 equipes paga
    /// 10 × o coeficiente por ano, tenha ela 5 ou 14 etapas.
    ///
    /// Note que "equipe média" é média do BOLO, não a equipe na posição do meio: com curva
    /// convexa quem termina no meio da classe arrecada MENOS que a média, porque o dinheiro
    /// está concentrado na frente. É o efeito pretendido, e é o que o critério 6 pede.
    #[test]
    fn a_classe_arrecada_o_coeficiente_no_ano() {
        let p = ParametrosDeReceita::default();
        for etapas in [5.0, 8.0, 10.0, 14.0] {
            let por_etapa: f64 = (1..=20)
                .map(|pos| {
                    let mut e = entrada(vec![pos]);
                    e.etapas_na_temporada = etapas;
                    receita_da_etapa(&e, &p).premio_de_corrida
                })
                .sum();
            let ano = por_etapa * etapas;
            // 20 carros ÷ 2 por equipe = 10 equipes.
            let alvo = 10.0 * entrada(vec![1]).custo_operacional_anual * p.premio_de_corrida;
            assert!(
                (ano - alvo).abs() / alvo < 1e-9,
                "{etapas} etapas: ano {ano:.0} vs alvo {alvo:.0}"
            );
        }
    }

    /// A consequência da convexidade, medida: quem termina no meio da classe fica abaixo da
    /// média do bolo. Existe como aviso — quem ler a tabela de coeficientes precisa saber
    /// que `premio_de_corrida` é a média, não a mediana.
    #[test]
    fn o_meio_da_classe_fica_abaixo_da_media() {
        let p = ParametrosDeReceita::default();
        let media = entrada(vec![1]).custo_operacional_anual * p.premio_de_corrida
            / entrada(vec![1]).etapas_na_temporada
            / 2.0;
        let meio = receita_da_etapa(&entrada(vec![10]), &p).premio_de_corrida;
        assert!(meio < media, "meio {meio:.0} vs média {media:.0}");
    }

    /// O expoente do calendário é o ÚNICO lugar em que o número de etapas mexe no prêmio.
    /// Com 1,0 ele não faz nada; acima de 1,0 calendário curto recebe mais.
    #[test]
    fn expoente_inclina_a_favor_do_calendario_curto() {
        assert_eq!(fator_de_calendario(5.0, 1.0), 1.0);
        assert_eq!(fator_de_calendario(14.0, 1.0), 1.0);
        assert!(fator_de_calendario(5.0, 1.5) > 1.0);
        assert!(fator_de_calendario(14.0, 1.5) < 1.0);
        // E o pivô nunca se move.
        assert!((fator_de_calendario(ETAPAS_DE_REFERENCIA, 2.0) - 1.0).abs() < 1e-12);
    }

    /// A curva por posição redistribui sem criar nem destruir dinheiro: o que a CLASSE
    /// inteira recebe não depende da inclinação.
    #[test]
    fn a_curva_redistribui_sem_mudar_o_bolo() {
        let mut total = Vec::new();
        for inclinacao in [0.5, 1.0, 2.0, 3.0] {
            let p = ParametrosDeReceita {
                inclinacao_do_premio: inclinacao,
                ..Default::default()
            };
            let pago: f64 = (1..=20)
                .map(|pos| receita_da_etapa(&entrada(vec![pos]), &p).premio_de_corrida)
                .sum();
            total.push(pago);
        }
        let primeiro = total[0];
        for t in &total {
            assert!(
                (t - primeiro).abs() / primeiro < 1e-9,
                "bolo mudou com a inclinação: {total:?}"
            );
        }
    }

    /// Inclinação maior significa mais dinheiro para quem vence e menos para quem é último.
    #[test]
    fn inclinacao_separa_o_primeiro_do_ultimo() {
        let plano = ParametrosDeReceita {
            inclinacao_do_premio: 1.0,
            ..Default::default()
        };
        let convexo = ParametrosDeReceita {
            inclinacao_do_premio: 2.5,
            ..Default::default()
        };
        let v_plano = receita_da_etapa(&entrada(vec![1]), &plano).premio_de_corrida;
        let v_convexo = receita_da_etapa(&entrada(vec![1]), &convexo).premio_de_corrida;
        assert!(v_convexo > v_plano);
        let u_plano = receita_da_etapa(&entrada(vec![20]), &plano).premio_de_corrida;
        let u_convexo = receita_da_etapa(&entrada(vec![20]), &convexo).premio_de_corrida;
        assert!(u_convexo < u_plano);
    }

    /// O corte da seção 2.4: nada na receita lê dinheiro. A entrada nem tem campo de caixa —
    /// este teste existe para que acrescentar um seja uma decisão consciente, não um
    /// descuido.
    #[test]
    fn patrocinio_nao_le_riqueza() {
        let p = ParametrosDeReceita::default();
        let pobre = receita_da_etapa(&entrada(vec![18, 19]), &p).patrocinio;
        let rica = receita_da_etapa(&entrada(vec![1, 2]), &p).patrocinio;
        // Mesmo com resultados opostos, o contrato é o mesmo: ele não olha resultado nem
        // caixa, só reputação e fama, que aqui são iguais.
        assert_eq!(pobre, rica);
    }

    /// Sem piso, a cota é pura fama; com piso 1,0... o inverso. O parâmetro precisa mesmo
    /// mover a bilheteria, senão o critério 11 é impossível por construção.
    #[test]
    fn o_piso_controla_o_espalhamento_da_bilheteria() {
        let so_piso = ParametrosDeReceita {
            bilheteria_piso: 0.0,
            ..Default::default()
        };
        let so_fama = ParametrosDeReceita {
            bilheteria_piso: 1.0,
            ..Default::default()
        };
        let mut famosa = entrada(vec![1, 2]);
        famosa.atracao_de_publico = 90.0;
        let mut anonima = entrada(vec![1, 2]);
        anonima.atracao_de_publico = 10.0;

        let igual = receita_da_etapa(&famosa, &so_piso).bilheteria
            / receita_da_etapa(&anonima, &so_piso).bilheteria;
        assert!((igual - 1.0).abs() < 1e-9, "com piso 0 todos recebem igual");

        let separado = receita_da_etapa(&famosa, &so_fama).bilheteria
            / receita_da_etapa(&anonima, &so_fama).bilheteria;
        assert!(separado > 8.0, "com piso 1 a fama manda: {separado:.2}×");
    }

    /// O fechamento é monotônico e limitado pelos dois coeficientes.
    #[test]
    fn fechamento_vai_do_ultimo_ao_primeiro() {
        let p = ParametrosDeReceita::default();
        let op = 1_000_000.0;
        let primeiro = premio_de_fim_de_temporada(op, 1, 14, &p);
        let meio = premio_de_fim_de_temporada(op, 7, 14, &p);
        let ultimo = premio_de_fim_de_temporada(op, 14, 14, &p);
        assert!(primeiro > meio && meio > ultimo);
        assert!((primeiro - op * p.fechamento_ao_primeiro).abs() < 1e-6);
        assert!((ultimo - op * p.fechamento_ao_ultimo).abs() < 1e-6);
    }

    /// Prestígio do evento escala a bilheteria e nada mais — e o peso controla QUANTO.
    ///
    /// Com peso 1,0 a bilheteria é proporcional ao prestígio (3× de prestígio, 3× de
    /// dinheiro). O padrão calibrado usa 0,30 porque o prestígio é correlacionado com a
    /// CATEGORIA, e a proporcionalidade cheia sozinha põe o critério 4 fora do alvo em quatro
    /// categorias. Amortecido, ele continua diferenciando etapa de etapa sem diferenciar
    /// degrau de degrau.
    #[test]
    fn prestigio_move_so_a_bilheteria() {
        let mut fraca = entrada(vec![5, 6]);
        fraca.prestigio_do_evento = 30.0;
        let mut forte = entrada(vec![5, 6]);
        forte.prestigio_do_evento = 90.0;

        let cheio = ParametrosDeReceita {
            bilheteria_por_prestigio: 1.0,
            ..Default::default()
        };
        let razao_cheia = receita_da_etapa(&forte, &cheio).bilheteria
            / receita_da_etapa(&fraca, &cheio).bilheteria;
        assert!((razao_cheia - 3.0).abs() < 1e-9, "{razao_cheia}");

        let p = ParametrosDeReceita::default();
        let a = receita_da_etapa(&fraca, &p);
        let b = receita_da_etapa(&forte, &p);
        let razao = b.bilheteria / a.bilheteria;
        assert!(
            razao > 1.0 && razao < razao_cheia,
            "amortecido devia ficar entre 1× e o proporcional: {razao}"
        );
        // E nenhum outro canal se mexe.
        assert_eq!(a.premio_de_corrida, b.premio_de_corrida);
        assert_eq!(a.patrocinio, b.patrocinio);
    }
}
