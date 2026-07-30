//! **Decomposição de variância** — de onde vem a variação da posição de chegada.
//!
//! As sete métricas de resultado dizem SE a distribuição melhorou. Elas não dizem O QUE ajustar.
//! Esta peça diz: divide a variação da posição de chegada num orçamento percentual entre piloto
//! (permanente), equipe/carro, evento (pista, clima, acerto) e corrida (ruído puro).
//!
//! ## O desenho experimental
//!
//! Um grid fixo, `E` eventos distintos, `R` réplicas de cada evento (mesmo evento, RNG diferente).
//! Isso dá uma matriz `P[piloto][evento][réplica]` de posições de chegada, que é um ANOVA de
//! efeitos aleatórios cruzado:
//!
//! ```text
//! Var_total = Var_entre_pilotos  +  Var_piloto×evento  +  Var_residual
//!                 ^ permanente         ^ evento             ^ corrida
//! ```
//!
//! O efeito principal de evento é ZERO por construção: a posição é um posto de 1..N, então a
//! média por evento é sempre a mesma. Toda a assinatura da pista/clima aparece na INTERAÇÃO —
//! "este piloto rende melhor aqui do que costuma render" —, que é exatamente o que se quer medir.
//!
//! A parte permanente é então quebrada em piloto vs carro por congelamento seletivo: a mesma
//! campanha rodada com [`nivelar_carros`](super::campo::nivelar_carros) mantém só o piloto, e a
//! diferença para a campanha normal é o carro. Análogo para [`nivelar_pilotos`].
//!
//! ## A segunda via, independente
//!
//! A razão entre as duas correlações Spearman do baseline mede a mesma coisa por outro caminho.
//! Grid e chegada são duas observações do MESMO fim de semana: compartilham o permanente **e** a
//! camada de evento. Etapas consecutivas compartilham só o permanente. Logo:
//!
//! ```text
//! ρ(etapas consecutivas)  ≈  fração permanente
//! ρ(grid × chegada)       ≈  fração permanente + fração de evento
//! ```
//!
//! As duas vias são calculadas aqui lado a lado. Divergência entre elas é achado, não erro de
//! arredondamento — significa que alguma fonte não está onde o modelo supõe.

use crate::simulation::context::SimDriver;
use crate::simulation::race::RaceResult;

use super::arena::{self, ConfigTemporada, Evento};
use super::campo::{gerar_campo, nivelar_carros, nivelar_pilotos};
use super::metricas::spearman;

/// Parâmetros do experimento de decomposição.
#[derive(Debug, Clone)]
pub struct ConfigDecomposicao {
    pub base: ConfigTemporada,
    /// Eventos distintos (fins de semana diferentes) por grid.
    pub eventos: usize,
    /// Réplicas do MESMO evento, variando só o RNG da corrida.
    pub replicas: usize,
    /// Grids independentes; as frações são a média sobre eles.
    pub grids: usize,
}

impl ConfigDecomposicao {
    pub fn padrao(base: ConfigTemporada) -> Self {
        Self {
            base,
            eventos: 12,
            replicas: 8,
            grids: 10,
        }
    }

    /// Corridas totais deste experimento (por variante de congelamento).
    pub fn corridas(&self) -> usize {
        self.grids * self.eventos * self.replicas
    }
}

/// Como o grid foi congelado numa rodada do experimento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Congelamento {
    /// Grid completo, do jeito que o gerador entrega.
    Nenhum,
    /// Todos com o mesmo carro — sobra só o piloto.
    CarroNivelado,
    /// Todos com os mesmos atributos — sobra só o carro.
    PilotoNivelado,
}

/// Os três termos crus do ANOVA, na escala de posição² (não normalizados).
#[derive(Debug, Clone, Copy, Default)]
pub struct TermosAnova {
    pub entre_pilotos: f64,
    pub interacao_evento: f64,
    pub residual: f64,
}

impl TermosAnova {
    pub fn total(&self) -> f64 {
        self.entre_pilotos + self.interacao_evento + self.residual
    }
}

/// O orçamento final, em fração da variância total da posição de chegada.
#[derive(Debug, Clone)]
pub struct OrcamentoVariancia {
    pub rotulo: String,
    pub corridas: usize,
    /// Variância total da posição de chegada (posição²). Para N pilotos sem DNF ela tende ao
    /// valor teórico de uma distribuição uniforme de postos, (N²−1)/12 — serve de aferição.
    pub var_total: f64,
    pub var_total_teorica: f64,

    /// Fatia do piloto dentro do permanente. A razão piloto:carro vem de um grid com encaixe
    /// INDEPENDENTE, porque no grid realista os dois são correlacionados e nenhum congelamento
    /// isolado separa a covariância.
    pub frac_piloto: f64,
    /// Fatia da equipe/carro dentro do permanente, pela mesma via.
    pub frac_carro: f64,
    pub frac_evento: f64,
    pub frac_corrida: f64,

    /// Quebra da camada de evento entre pista e clima (soma = `frac_evento`).
    pub frac_evento_pista: f64,
    pub frac_evento_clima: f64,

    /// Via ANOVA: `frac_piloto + frac_carro`.
    pub permanente_via_anova: f64,
    /// Via ρ: correlação média entre a chegada de dois eventos DIFERENTES.
    pub permanente_via_rho: f64,
    /// Via ρ: correlação média entre grid e chegada do MESMO evento — permanente + evento.
    pub permanente_mais_evento_via_rho: f64,

    /// **Reprodutibilidade da classificação**: ρ entre a ordem de GRID de dois eventos
    /// diferentes. É o que separa as duas explicações possíveis para um ρ(grid × chegada) baixo:
    ///
    /// - alta (perto da reprodutibilidade da chegada) → a quali mede um EIXO DIFERENTE, mas
    ///   estável. O piloto rápido de uma volta é sempre o mesmo, só não é o mais rápido de
    ///   corrida. É o que o pacote F quer.
    /// - baixa → a quali virou LOTERIA. O grid muda de evento para evento sem que nada no piloto
    ///   tenha mudado, e a variância da sessão está alta demais.
    ///
    /// Nenhuma métrica de resultado consegue distinguir esses dois casos; esta consegue.
    pub reprodutibilidade_do_grid: f64,
}

impl OrcamentoVariancia {
    /// Divergência absoluta entre as duas vias de estimar a fatia permanente.
    pub fn divergencia_das_vias(&self) -> f64 {
        (self.permanente_via_anova - self.permanente_via_rho).abs()
    }
}

// ---------------------------------------------------------------------------
// A repartição-alvo do orçamento
// ---------------------------------------------------------------------------

/// **Como o orçamento de variância DEVE ficar** depois de B, C, D e G.
///
/// É a pergunta de design mais importante que sobrou, e a decomposição é a única ferramenta capaz
/// de respondê-la. Cada faixa vem com o argumento; discordar é editar o número e o comentário
/// juntos, como em [`super::alvos`].
///
/// A âncora empírica é a correlação entre chegadas de corridas consecutivas em séries reais:
/// numa monomarca competitiva ela fica na casa de 0,40–0,60, e essa correlação É a fração
/// permanente. Hoje o Loop mede 0,97.
#[derive(Debug, Clone)]
pub struct OrcamentoAlvo {
    pub piloto: super::alvos::Faixa,
    pub carro: super::alvos::Faixa,
    pub evento: super::alvos::Faixa,
    pub corrida: super::alvos::Faixa,
    /// Teto da variância NÃO ATRIBUÍVEL a qualidade — safety car na hora errada, batida de
    /// terceiro, azar de trânsito. Não é uma fonte separada no ANOVA (mora dentro de `evento` e
    /// `corrida`); é um limite de design que atravessa as duas.
    pub teto_de_azar: f64,
}

impl OrcamentoAlvo {
    /// Monomarca de entrada: carro spec, então o permanente é quase todo piloto.
    pub fn entrada() -> Self {
        use super::alvos::Faixa;
        Self {
            // Permanente total 40–55%. Abaixo disso o jogador não sente que evoluir importa;
            // acima, a temporada é decidida na primeira etapa (que é o defeito de hoje).
            piloto: Faixa::nova(0.38, 0.52),
            // Spec de verdade: o que sobra é a moral da equipe e nada mais.
            carro: Faixa::nova(0.0, 0.05),
            // Afinidade com a pista + forma + acerto (B) + estratégia e safety car (G).
            evento: Faixa::nova(0.20, 0.32),
            // Incidente, trânsito, o erro do dia.
            corrida: Faixa::nova(0.22, 0.35),
            teto_de_azar: 0.25,
        }
    }

    /// Topo: o carro é metade do permanente por design (`car_weight_scale("gt3") == 1.30`) e as
    /// dinastias são um objetivo declarado, não um efeito colateral.
    pub fn topo() -> Self {
        use super::alvos::Faixa;
        Self {
            piloto: Faixa::nova(0.22, 0.35),
            carro: Faixa::nova(0.22, 0.38),
            // Corrida longa, parada obrigatória: a estratégia pesa MAIS aqui que na entrada.
            evento: Faixa::nova(0.22, 0.34),
            // Pelotão profissional erra menos — o ruído puro é menor que na rookie.
            corrida: Faixa::nova(0.12, 0.24),
            teto_de_azar: 0.20,
        }
    }

    /// Permanente total implícito na repartição.
    pub fn permanente(&self) -> (f64, f64) {
        (
            self.piloto.min + self.carro.min,
            self.piloto.max + self.carro.max,
        )
    }

    /// Divergências entre um orçamento medido e este alvo, em texto pronto para relatório.
    pub fn conferir(&self, o: &OrcamentoVariancia) -> Vec<String> {
        let mut falhas = Vec::new();
        for (nome, valor, faixa) in [
            ("piloto", o.frac_piloto, self.piloto),
            ("carro", o.frac_carro, self.carro),
            ("evento", o.frac_evento, self.evento),
            ("corrida", o.frac_corrida, self.corrida),
        ] {
            if !faixa.contem(valor) {
                falhas.push(format!(
                    "{nome}: {:.1}% (alvo {:.0}–{:.0}%, {})",
                    valor * 100.0,
                    faixa.min * 100.0,
                    faixa.max * 100.0,
                    faixa.veredito(valor)
                ));
            }
        }
        falhas
    }
}

// ---------------------------------------------------------------------------
// ANOVA sobre a matriz P[piloto][evento][réplica]
// ---------------------------------------------------------------------------

/// `matriz[p][e][r]` = posição de chegada. DNF entra pela posição classificada, que é o que o
/// campeonato enxerga.
///
/// ANOVA de efeitos aleatórios cruzado, com os **quadrados médios esperados** corretos — não é
/// preciosismo: dividir a soma de quadrados residual por `R` em vez de `R−1` subestima o ruído em
/// ~1/R e joga essa fatia na interação, que é justamente a fração que interessa distinguir.
///
/// ```text
/// E[MS_piloto]   = σ²  +  R·σ²_interacao  +  R·E·σ²_piloto
/// E[MS_interacao]= σ²  +  R·σ²_interacao
/// E[MS_residual] = σ²
/// ```
fn decompor(matriz: &[Vec<Vec<f64>>]) -> TermosAnova {
    let pilotos = matriz.len();
    if pilotos < 2 {
        return TermosAnova::default();
    }
    let eventos = matriz[0].len();
    let replicas = matriz[0].first().map(|v| v.len()).unwrap_or(0);
    if eventos < 2 || replicas < 2 {
        return TermosAnova::default();
    }

    let (p, e, r) = (pilotos as f64, eventos as f64, replicas as f64);

    let media_celula = |pi: usize, ei: usize| -> f64 {
        matriz[pi][ei].iter().sum::<f64>() / matriz[pi][ei].len().max(1) as f64
    };
    let media_piloto =
        |pi: usize| -> f64 { (0..eventos).map(|ei| media_celula(pi, ei)).sum::<f64>() / e };
    let media_evento =
        |ei: usize| -> f64 { (0..pilotos).map(|pi| media_celula(pi, ei)).sum::<f64>() / p };
    let media_geral = (0..pilotos).map(media_piloto).sum::<f64>() / p;

    let mut ss_piloto = 0.0;
    let mut ss_interacao = 0.0;
    let mut ss_residual = 0.0;

    for pi in 0..pilotos {
        let mp = media_piloto(pi);
        ss_piloto += (mp - media_geral).powi(2);
        for ei in 0..eventos {
            let mc = media_celula(pi, ei);
            ss_interacao += (mc - mp - media_evento(ei) + media_geral).powi(2);
            for v in &matriz[pi][ei] {
                ss_residual += (v - mc).powi(2);
            }
        }
    }
    ss_piloto *= r * e;
    ss_interacao *= r;

    let ms_piloto = ss_piloto / (p - 1.0);
    let ms_interacao = ss_interacao / ((p - 1.0) * (e - 1.0));
    let ms_residual = ss_residual / (p * e * (r - 1.0));

    // Componentes de variância. O clamp em 0 é padrão em ANOVA de efeitos aleatórios: uma
    // estimativa negativa significa "indistinguível de zero", não variância negativa.
    TermosAnova {
        entre_pilotos: ((ms_piloto - ms_interacao) / (r * e)).max(0.0),
        interacao_evento: ((ms_interacao - ms_residual) / r).max(0.0),
        residual: ms_residual.max(0.0),
    }
}

/// Roda a grade eventos × réplicas para um grid e devolve a matriz de posições, mais as corridas
/// cruas (que as vias-ρ reaproveitam).
fn rodar_grade(
    config: &ConfigDecomposicao,
    grid: &[SimDriver],
    eventos: &[Evento],
    catalogo: &crate::simulation::catalog::IncidentCatalog,
    semente: u64,
) -> (Vec<Vec<Vec<f64>>>, Vec<Vec<RaceResult>>) {
    let mut matriz: Vec<Vec<Vec<f64>>> =
        vec![vec![Vec::with_capacity(config.replicas); eventos.len()]; grid.len()];
    let mut corridas: Vec<Vec<RaceResult>> = Vec::with_capacity(eventos.len());

    let indice_do_piloto = |id: &str| grid.iter().position(|d| d.id == id);

    for (e, evento) in eventos.iter().enumerate() {
        let mut do_evento = Vec::with_capacity(config.replicas);
        for r in 0..config.replicas {
            let s = semente
                .wrapping_mul(1_000_003)
                .wrapping_add((e * 1_009 + r) as u64);
            let resultado = arena::rodar_evento(&config.base, grid, evento, e + 1, catalogo, s);
            for linha in &resultado.race_results {
                if let Some(p) = indice_do_piloto(&linha.pilot_id) {
                    matriz[p][e].push(linha.finish_position as f64);
                }
            }
            do_evento.push(resultado);
        }
        corridas.push(do_evento);
    }

    (matriz, corridas)
}

/// Fração permanente pela via-ρ: correlação da chegada entre dois eventos DIFERENTES (réplicas
/// distintas, para não compartilhar nem o RNG). O que sobrevive a uma troca de pista e de
/// semente só pode ser o que o piloto carrega consigo.
fn rho_entre_eventos(corridas: &[Vec<RaceResult>]) -> f64 {
    let mut valores = Vec::new();
    for e1 in 0..corridas.len() {
        for e2 in (e1 + 1)..corridas.len() {
            let a = &corridas[e1][0];
            let b = corridas[e2].get(1).unwrap_or(&corridas[e2][0]);
            if let Some(rho) = rho_de_duas_corridas(a, b) {
                valores.push(rho);
            }
        }
    }
    media(&valores)
}

/// Reprodutibilidade do GRID: mesma lógica de [`rho_entre_eventos`], mas comparando a ordem de
/// largada em vez da de chegada. Compará-las lado a lado é o que diz se a classificação virou
/// outro eixo ou virou ruído.
fn rho_entre_grids(corridas: &[Vec<RaceResult>]) -> f64 {
    let mut valores = Vec::new();
    for e1 in 0..corridas.len() {
        for e2 in (e1 + 1)..corridas.len() {
            let a = &corridas[e1][0];
            let b = corridas[e2].get(1).unwrap_or(&corridas[e2][0]);
            let mapa: std::collections::HashMap<&str, f64> = b
                .race_results
                .iter()
                .map(|r| (r.pilot_id.as_str(), r.grid_position as f64))
                .collect();
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for r in &a.race_results {
                if let Some(pos) = mapa.get(r.pilot_id.as_str()) {
                    xs.push(r.grid_position as f64);
                    ys.push(*pos);
                }
            }
            if let Some(rho) = spearman(&xs, &ys) {
                valores.push(rho);
            }
        }
    }
    media(&valores)
}

fn rho_de_duas_corridas(a: &RaceResult, b: &RaceResult) -> Option<f64> {
    let mapa_b: std::collections::HashMap<&str, f64> = b
        .race_results
        .iter()
        .map(|r| (r.pilot_id.as_str(), r.finish_position as f64))
        .collect();
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for r in &a.race_results {
        if let Some(pos) = mapa_b.get(r.pilot_id.as_str()) {
            xs.push(r.finish_position as f64);
            ys.push(*pos);
        }
    }
    spearman(&xs, &ys)
}

/// Via-ρ do mesmo fim de semana: grid × chegada. Carrega permanente + evento.
fn rho_grid_chegada(corridas: &[Vec<RaceResult>]) -> f64 {
    let mut valores = Vec::new();
    for do_evento in corridas {
        for corrida in do_evento {
            let (g, f): (Vec<f64>, Vec<f64>) = corrida
                .race_results
                .iter()
                .filter(|r| !r.is_dnf)
                .map(|r| (r.grid_position as f64, r.finish_position as f64))
                .unzip();
            if let Some(rho) = spearman(&g, &f) {
                valores.push(rho);
            }
        }
    }
    media(&valores)
}

fn media(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.iter().sum::<f64>() / v.len() as f64
}

/// Roda a grade para uma variante de congelamento e devolve os termos médios sobre os grids.
fn termos_da_variante(
    config: &ConfigDecomposicao,
    congelamento: Congelamento,
    eventos_por_grid: &dyn Fn(u64) -> Vec<Evento>,
    semente_base: u64,
) -> (TermosAnova, f64, f64, f64) {
    let catalogo = arena::catalogo_para(&config.base);
    let mut acumulado = TermosAnova::default();
    let mut rhos_entre = Vec::new();
    let mut rhos_grid = Vec::new();
    let mut rhos_grid_entre = Vec::new();

    for g in 0..config.grids {
        let semente = arena::semente_da_temporada(semente_base, g);
        let mut grid = gerar_campo(&config.base.perfil, config.base.pilotos, semente);
        match congelamento {
            Congelamento::Nenhum => {}
            Congelamento::CarroNivelado => nivelar_carros(&mut grid),
            Congelamento::PilotoNivelado => nivelar_pilotos(&mut grid),
        }

        let eventos = eventos_por_grid(semente);
        let (matriz, corridas) =
            rodar_grade(config, &grid, &eventos, &catalogo, semente ^ 0xC0FFEE);
        let termos = decompor(&matriz);

        acumulado.entre_pilotos += termos.entre_pilotos;
        acumulado.interacao_evento += termos.interacao_evento;
        acumulado.residual += termos.residual;
        rhos_entre.push(rho_entre_eventos(&corridas));
        rhos_grid.push(rho_grid_chegada(&corridas));
        rhos_grid_entre.push(rho_entre_grids(&corridas));
    }

    let n = config.grids.max(1) as f64;
    (
        TermosAnova {
            entre_pilotos: acumulado.entre_pilotos / n,
            interacao_evento: acumulado.interacao_evento / n,
            residual: acumulado.residual / n,
        },
        media(&rhos_entre),
        media(&rhos_grid),
        media(&rhos_grid_entre),
    )
}

/// Executa a decomposição completa: quatro variantes do experimento, uma para cada fonte que
/// precisa ser isolada.
///
/// 1. **completa** — eventos variados, grid inteiro. Dá o total, a camada de evento e o ruído.
/// 2. **carro nivelado** — isola a fatia do piloto dentro do permanente.
/// 3. **pista fixa, clima variado** — isola a fatia de clima dentro do evento.
/// 4. **pistas variadas, clima seco** — isola a fatia de pista dentro do evento.
pub fn decompor_variancia(
    rotulo: &str,
    config: &ConfigDecomposicao,
    semente_base: u64,
) -> OrcamentoVariancia {
    // (1) Completa — eventos variados, grid inteiro.
    let cfg_variado = ConfigDecomposicao {
        base: ConfigTemporada {
            etapas: config.eventos,
            ..config.base.clone()
        },
        ..config.clone()
    };
    let gerar_variados = {
        let c = cfg_variado.base.clone();
        move |semente: u64| arena::sortear_eventos(&c, semente)
    };
    let (completa, rho_entre, rho_grid, rho_grid_entre) = termos_da_variante(
        &cfg_variado,
        Congelamento::Nenhum,
        &gerar_variados,
        semente_base,
    );

    // (2) Piloto vs carro. Rodado sobre um grid de encaixe INDEPENDENTE (`ruido_encaixe = 1.0`),
    // porque no grid realista os dois são correlacionados e o congelamento isolado não separa a
    // covariância: nivelar carros deixa a ordem quase igual, e nivelar pilotos também. Com o
    // encaixe ortogonal o split fica identificado, ao custo de o cenário não ser o do jogo —
    // por isso ele NÃO é usado para o total, só para a razão piloto:carro dentro do permanente.
    let cfg_ortogonal = ConfigDecomposicao {
        base: ConfigTemporada {
            perfil: super::campo::PerfilCampo {
                ruido_encaixe: 1.0,
                ..cfg_variado.base.perfil.clone()
            },
            ..cfg_variado.base.clone()
        },
        ..cfg_variado.clone()
    };
    let gerar_ortogonais = {
        let c = cfg_ortogonal.base.clone();
        move |semente: u64| arena::sortear_eventos(&c, semente)
    };
    let (orto_completa, _, _, _) = termos_da_variante(
        &cfg_ortogonal,
        Congelamento::Nenhum,
        &gerar_ortogonais,
        semente_base,
    );
    let (orto_carro_nivelado, _, _, _) = termos_da_variante(
        &cfg_ortogonal,
        Congelamento::CarroNivelado,
        &gerar_ortogonais,
        semente_base,
    );
    let (orto_piloto_nivelado, _, _, _) = termos_da_variante(
        &cfg_ortogonal,
        Congelamento::PilotoNivelado,
        &gerar_ortogonais,
        semente_base,
    );

    // (3) Pista FIXA, clima variado — a interação que sobra é só clima.
    let gerar_pista_fixa = {
        let c = cfg_variado.base.clone();
        let n = config.eventos;
        move |semente: u64| {
            let base = arena::evento_unico(semente);
            let variados = arena::sortear_eventos(&c, semente);
            (0..n)
                .map(|i| Evento {
                    pista: base.pista,
                    clima: variados.get(i).map(|e| e.clima).unwrap_or(base.clima),
                    temperatura: variados
                        .get(i)
                        .map(|e| e.temperatura)
                        .unwrap_or(base.temperatura),
                    decisivo: false,
                })
                .collect()
        }
    };
    let (pista_fixa, _, _, _) = termos_da_variante(
        &cfg_variado,
        Congelamento::Nenhum,
        &gerar_pista_fixa,
        semente_base,
    );

    // --- Montagem do orçamento ---
    let total = completa.total();
    let seguro = |x: f64| {
        if total > f64::EPSILON {
            x / total
        } else {
            f64::NAN
        }
    };

    let frac_permanente = seguro(completa.entre_pilotos);

    // Razão piloto:carro medida no grid ortogonal, aplicada ao permanente do grid realista.
    let orto_permanente = orto_completa.entre_pilotos;
    let (peso_piloto, peso_carro) = if orto_permanente > f64::EPSILON {
        let p = (orto_carro_nivelado.entre_pilotos / orto_permanente).clamp(0.0, 1.0);
        let c = (orto_piloto_nivelado.entre_pilotos / orto_permanente).clamp(0.0, 1.0);
        let soma = p + c;
        if soma > f64::EPSILON {
            (p / soma, c / soma)
        } else {
            (1.0, 0.0)
        }
    } else {
        (1.0, 0.0)
    };
    let frac_piloto = frac_permanente * peso_piloto;
    let frac_carro = frac_permanente * peso_carro;

    let frac_evento = seguro(completa.interacao_evento);
    // Com a pista congelada, a interação remanescente só pode vir de clima/temperatura.
    let frac_evento_clima = seguro(pista_fixa.interacao_evento).min(frac_evento);
    let frac_evento_pista = (frac_evento - frac_evento_clima).max(0.0);

    let n = config.base.pilotos as f64;

    OrcamentoVariancia {
        rotulo: rotulo.to_string(),
        corridas: cfg_variado.corridas() * 4,
        var_total: total,
        var_total_teorica: (n * n - 1.0) / 12.0,
        frac_piloto,
        frac_carro,
        frac_evento,
        frac_corrida: seguro(completa.residual),
        frac_evento_pista,
        frac_evento_clima,
        permanente_via_anova: frac_permanente,
        permanente_via_rho: rho_entre,
        permanente_mais_evento_via_rho: rho_grid,
        reprodutibilidade_do_grid: rho_grid_entre,
    }
}
