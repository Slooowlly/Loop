//! **A máquina de busca** — calibração por descida coordenada contra as faixas-alvo.
//!
//! Construída ANTES do pacote D de propósito, para rodar sobre o espaço de parâmetros atual, que
//! é o banco de testes perfeito: a varredura já provou que ele **não tem alavanca** — nenhum knob,
//! em nenhum valor, chega perto do alvo. É um espaço onde o fracasso é garantido e conhecido.
//!
//! Por isso o critério de aceitação aqui é invertido: **a busca tem que falhar, e falhar bem.**
//! Se ela devolver um ponto ótimo com ar de sucesso, o defeito é dela. Descobrir isso agora é de
//! graça; descobrir depois do D, quando um falso "convergiu" custaria uma calibração inteira,
//! não é.
//!
//! O plano completo está em `CAMPANHA.md`. Esta é a parte que dava para adiantar.
//!
//! ## O problema do gradiente, e por que a função-objetivo mudou
//!
//! A especificação original media a distância à faixa na escala CRUA da métrica. Ela tem
//! gradiente fora da faixa — é linear na distância, não um degrau. Mas há um buraco mais sutil,
//! e ele é fatal exatamente na situação inicial de qualquer calibração real (tudo fora da faixa):
//!
//! **métricas limitadas saturam perto do limite.** A correlação entre etapas consecutivas está em
//! 0,976 contra um alvo de 0,55. Como ρ é limitado por 1, mexer um knob de 1,4 para 10 move ρ em
//! 0,13 — e perto de 1 cada passo de parâmetro produz um passo de métrica cada vez menor. Na
//! escala crua, o gradiente **existe mas encolhe** justamente onde a busca começa, e afunda abaixo
//! do ruído de amostragem. Aí sim a busca vira caminhada aleatória.
//!
//! A correção é medir a distância numa escala que ESTIQUE a região saturada:
//!
//! - correlações → `atanh` (Fisher). ρ = 0,976 vira z = 2,19; ρ = 0,55 vira z = 0,62. A distância
//!   passa de 0,43 para 1,57, e o mesmo passo de parâmetro produz um passo de objetivo bem maior.
//! - frações limitadas em (0, 1) → `logit`, pela mesma razão.
//! - contagens e posições, que não têm teto → lineares, sem transformação.
//!
//! Isso não muda ONDE está o alvo (a faixa transformada tem as mesmas bordas), só a métrica de
//! distância entre o ponto atual e ele. Ver [`Escala`] e o teste
//! `escala_de_correlacao_restaura_gradiente_na_saturacao`.

use std::collections::BTreeMap;

use super::alvos::{Alvos, Faixa};
use super::arena::{self, AjustesCtx, ConfigTemporada};
use super::metricas::MetricasAgregadas;
use super::varredura::Knob;

// ---------------------------------------------------------------------------
// Escala e distância
// ---------------------------------------------------------------------------

/// Em que espaço a distância à faixa é medida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escala {
    /// Sem transformação. Para métricas sem teto (contagens, posições).
    Linear,
    /// `atanh` — para correlações em (−1, 1). Estica a região perto de ±1, onde o gradiente
    /// bruto colapsa.
    Correlacao,
    /// `logit` — para frações em (0, 1). Mesma razão, perto de 0 e de 1.
    Fracao,
}

/// Limite de segurança: quanto se pode chegar do limite antes de a transformação explodir.
const MARGEM_DE_SATURACAO: f64 = 1e-4;

impl Escala {
    pub fn transformar(&self, x: f64) -> f64 {
        match self {
            Self::Linear => x,
            Self::Correlacao => {
                let c = x.clamp(-1.0 + MARGEM_DE_SATURACAO, 1.0 - MARGEM_DE_SATURACAO);
                0.5 * ((1.0 + c) / (1.0 - c)).ln()
            }
            Self::Fracao => {
                let p = x.clamp(MARGEM_DE_SATURACAO, 1.0 - MARGEM_DE_SATURACAO);
                (p / (1.0 - p)).ln()
            }
        }
    }
}

/// Uma métrica sob calibração: como extraí-la, qual a faixa e em que escala medir a distância.
pub struct Metrica {
    pub nome: &'static str,
    pub extrair: fn(&MetricasAgregadas) -> f64,
    pub faixa: Faixa,
    pub escala: Escala,
}

impl Metrica {
    /// Distância normalizada à faixa, na escala da métrica. 0 = dentro.
    ///
    /// A normalização é pela LARGURA DA FAIXA TRANSFORMADA, e é ela que impede uma métrica de
    /// escala grande de dominar uma de escala pequena. Não há pesos de propósito: peso é
    /// julgamento disfarçado de matemática — se uma métrica precisa valer mais, isso vai na
    /// largura da faixa, onde a razão fica escrita no comentário do alvo.
    pub fn distancia(&self, valor: f64) -> f64 {
        if !valor.is_finite() {
            return f64::INFINITY;
        }
        let x = self.escala.transformar(valor);
        let min = self.escala.transformar(self.faixa.min);
        let max = self.escala.transformar(self.faixa.max);
        let largura = (max - min).abs().max(f64::EPSILON);

        if x < min {
            (min - x) / largura
        } else if x > max {
            (x - max) / largura
        } else {
            0.0
        }
    }
}

/// O conjunto de métricas calibradas. Sem pesos, por desenho.
pub fn objetivo(alvos: &Alvos) -> Vec<Metrica> {
    vec![
        Metrica {
            nome: "spearman_etapas_consecutivas",
            extrair: |m| m.spearman_etapas_consecutivas,
            faixa: alvos.spearman_etapas_consecutivas,
            escala: Escala::Correlacao,
        },
        Metrica {
            nome: "spearman_grid_chegada",
            extrair: |m| m.spearman_grid_chegada,
            faixa: alvos.spearman_grid_chegada,
            escala: Escala::Correlacao,
        },
        Metrica {
            nome: "desvio_posicao",
            extrair: |m| m.desvio_posicao,
            faixa: alvos.desvio_posicao,
            escala: Escala::Linear,
        },
        Metrica {
            nome: "vencedores_distintos",
            extrair: |m| m.vencedores_distintos,
            faixa: alvos.vencedores_distintos,
            escala: Escala::Linear,
        },
        Metrica {
            nome: "pct_vitorias_do_pole",
            extrair: |m| m.pct_vitorias_do_pole,
            faixa: alvos.pct_vitorias_do_pole,
            escala: Escala::Fracao,
        },
        Metrica {
            nome: "p_melhor_fora_top5",
            extrair: |m| m.p_melhor_fora_top5,
            faixa: alvos.p_melhor_fora_top5,
            escala: Escala::Fracao,
        },
        Metrica {
            nome: "trocas_de_lideranca",
            extrair: |m| m.trocas_de_lideranca,
            faixa: alvos.trocas_de_lideranca,
            escala: Escala::Linear,
        },
        Metrica {
            nome: "margem_do_campeao",
            extrair: |m| m.margem_do_campeao,
            faixa: alvos.margem_do_campeao,
            escala: Escala::Fracao,
        },
    ]
}

// ---------------------------------------------------------------------------
// Níveis de triagem
// ---------------------------------------------------------------------------

/// Os três níveis de triagem. O gargalo não é CPU — é quantos pontos um humano consegue auditar.
/// T1 existe porque gastar medição boa em ponto grosseiramente errado polui a leitura do resto.
///
/// # A forma do nível depende da simulação medida — e isso derrubou uma generalização
///
/// A primeira medição, feita contra a simulação PRÉ-REFORMA, mostrou que o ruído do objetivo
/// quase não caía de T2 (1,89) para T3 (1,83) apesar de quase 3× mais corridas. A conclusão que
/// eu tirei — "a incerteza dominante é qual grid foi sorteado, não quantas corridas rodaram;
/// portanto corte etapas, não temporadas" — **estava certa como medição e errada como
/// generalização.**
///
/// Remedida na árvore reformada (com C, D e G), a mesma tabela inverteu:
///
/// | Nível | corridas | ruído — pré-reforma | ruído — pós-reforma |
/// |---|---|---|---|
/// | T1 (72) | 72 | 0,60 | 2,76 |
/// | T2 | 360 | 1,89 | 0,80 |
/// | T3 | 1008 | 1,83 | 0,55 |
///
/// Agora o ruído CAI com o volume, como ruído de amostragem deve cair. O comportamento anterior
/// era artefato de uma simulação quase determinística: sem variância por corrida, a única
/// variação que sobrava era estrutural (grid a grid), e essa de fato não some com mais corridas.
/// Assim que a corrida passou a ter física — tráfego, estratégia, forma —, mais corridas voltaram
/// a comprar precisão.
///
/// Duas lições que sobrevivem: comparar dois pontos exige a MESMA semente (grids diferentes não
/// são comparáveis), e **a forma do nível tem que ser remedida a cada mudança grande do motor**.
/// Por isso [`Nivel`] é struct e não enum: a forma é parâmetro, não constante.
///
/// A forma atual de T1 (15 × 10) foi escolhida por medição de concordância de ordenação contra
/// T2 — ver `triagem_t1_preserva_a_ordem_do_nivel_caro`. A anterior (12 × 6) dava 0,71, abaixo do
/// mínimo de 0,80.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nivel {
    pub temporadas: usize,
    pub etapas: usize,
}

impl Nivel {
    /// Peneira. A forma foi remedida na árvore reformada: 12 × 6 = 72 corridas dava concordância
    /// de ordenação de apenas 0,71 com o T2 (média sobre 5 eixos), abaixo do mínimo de 0,80.
    pub const T1: Nivel = Nivel {
        temporadas: 15,
        etapas: 10,
    };
    /// Trabalho: 360 corridas. É onde a descida coordenada vive.
    pub const T2: Nivel = Nivel {
        temporadas: 30,
        etapas: 12,
    };
    /// Veredito: 1008 corridas. Só no finalista.
    pub const T3: Nivel = Nivel {
        temporadas: 84,
        etapas: 12,
    };

    pub const fn nova(temporadas: usize, etapas: usize) -> Self {
        Self { temporadas, etapas }
    }

    pub fn corridas(&self) -> usize {
        self.temporadas * self.etapas
    }
}

// ---------------------------------------------------------------------------
// Ponto do espaço e avaliação
// ---------------------------------------------------------------------------

/// Um ponto do espaço de parâmetros: um valor por knob.
pub type Ponto = BTreeMap<&'static str, f64>;

/// Um eixo do espaço: o knob e os valores testáveis.
#[derive(Debug, Clone)]
pub struct Eixo {
    pub knob: Knob,
    pub valores: Vec<f64>,
}

/// Converte um ponto do espaço nas sobrescritas de contexto. Público para que a guarda de teste
/// consiga verificar que o ajuste de fato CHEGA ao `SimulationContext` — se não chegasse, a busca
/// mediria o mesmo ponto N vezes e reportaria "sem alavanca" por bug em vez de por achado.
pub fn ajustes_de_ponto(ponto: &Ponto) -> AjustesCtx {
    ajustes_de(ponto)
}

fn ajustes_de(ponto: &Ponto) -> AjustesCtx {
    let mut a = AjustesCtx::default();
    for (nome, valor) in ponto {
        for knob in Knob::todos() {
            if knob.nome() == *nome {
                let parcial = knob_para_ajuste(knob, *valor);
                mesclar(&mut a, parcial);
            }
        }
    }
    a
}

fn knob_para_ajuste(knob: Knob, valor: f64) -> AjustesCtx {
    let mut a = AjustesCtx::default();
    match knob.nome() {
        "race_variance_multiplier" => a.race_variance_multiplier = Some(valor),
        "race_pace_spread_multiplier" => a.race_pace_spread_multiplier = Some(valor),
        "start_chaos_multiplier" => a.start_chaos_multiplier = Some(valor),
        "qualifying_variance_multiplier" => a.qualifying_variance_multiplier = Some(valor),
        "pack_density_factor" => a.pack_density_factor = Some(valor),
        "incident_rate_multiplier" => a.incident_rate_multiplier = Some(valor),
        "overtaking_difficulty_multiplier" => a.overtaking_difficulty_multiplier = Some(valor),
        "track_difficulty_multiplier" => a.track_difficulty_multiplier = Some(valor),
        "rain_sensitivity" => a.rain_sensitivity = Some(valor),
        _ => {}
    }
    a
}

fn mesclar(destino: &mut AjustesCtx, origem: AjustesCtx) {
    macro_rules! juntar {
        ($($campo:ident),+ $(,)?) => {
            $(if origem.$campo.is_some() { destino.$campo = origem.$campo; })+
        };
    }
    juntar!(
        race_variance_multiplier,
        race_pace_spread_multiplier,
        start_chaos_multiplier,
        qualifying_variance_multiplier,
        pack_density_factor,
        incident_rate_multiplier,
        overtaking_difficulty_multiplier,
        track_difficulty_multiplier,
        rain_sensitivity,
    );
}

/// O resultado de medir um ponto.
#[derive(Debug, Clone)]
pub struct Avaliacao {
    pub ponto: Ponto,
    pub nivel: Nivel,
    /// Distância por métrica, na ordem de [`objetivo`].
    pub distancias: Vec<f64>,
    pub valores: Vec<f64>,
    /// Soma das distâncias. Somar, e não tirar média, mantém o custo de deixar uma métrica fora.
    pub total: f64,
}

pub struct Avaliador {
    pub base: ConfigTemporada,
    pub alvos: Alvos,
    pub semente: u64,
    pub gastas: usize,
    pub teto: usize,
    /// Melhor (menor) valor de distância que CADA métrica atingiu em QUALQUER ponto já medido,
    /// junto com o valor bruto que a produziu. É o requisito 4 do relatório de fracasso — responde
    /// "esta métrica é alcançável de todo?", que é pergunta diferente de "o melhor ponto a
    /// atinge?".
    pub melhor_por_metrica: Vec<(f64, f64)>,
}

impl Avaliador {
    pub fn novo(base: ConfigTemporada, alvos: Alvos, semente: u64, teto: usize) -> Self {
        let n = objetivo(&alvos).len();
        Self {
            base,
            alvos,
            semente,
            gastas: 0,
            teto,
            melhor_por_metrica: vec![(f64::INFINITY, f64::NAN); n],
        }
    }

    pub fn esgotado(&self) -> bool {
        self.gastas >= self.teto
    }

    pub fn avaliar(&mut self, ponto: &Ponto, nivel: Nivel) -> Option<Avaliacao> {
        if self.esgotado() {
            return None;
        }
        self.gastas += 1;

        let config = ConfigTemporada {
            etapas: nivel.etapas,
            ..self.base.clone()
        }
        .com_ajustes(ajustes_de(ponto));

        let m = arena::medir("busca", &config, nivel.temporadas, self.semente);
        let metricas = objetivo(&self.alvos);

        let mut distancias = Vec::with_capacity(metricas.len());
        let mut valores = Vec::with_capacity(metricas.len());
        for (i, metrica) in metricas.iter().enumerate() {
            let valor = (metrica.extrair)(&m);
            let d = metrica.distancia(valor);
            if d < self.melhor_por_metrica[i].0 {
                self.melhor_por_metrica[i] = (d, valor);
            }
            distancias.push(d);
            valores.push(valor);
        }

        Some(Avaliacao {
            ponto: ponto.clone(),
            nivel,
            total: distancias.iter().sum(),
            distancias,
            valores,
        })
    }
}

// ---------------------------------------------------------------------------
// Veredito
// ---------------------------------------------------------------------------

/// O veredito de uma métrica. A taxonomia é o que separa três defeitos com três consertos
/// diferentes — e a primeira versão desta busca errou exatamente aqui, tratando "alguém atingiu
/// em algum ponto" como sucesso.
///
/// A distinção que importa: uma métrica pode ser atingível SOZINHA e impossível JUNTO com as
/// outras. Isso não é "falta mecanismo", é "as métricas brigam entre si" — e o conserto é outro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VereditoMetrica {
    /// Dentro da faixa **no ponto ótimo**, junto com todas as outras. É o único sucesso.
    Atingido,
    /// Fora no ótimo, mas perto (≤ 1,5× a largura da faixa). Falta ajuste fino.
    Parcial,
    /// **Atingível isoladamente, impossível em conjunto**: existe ponto no espaço que a coloca
    /// dentro, mas lá as outras métricas desabam. O espaço TEM alavanca; ela é que está sendo
    /// gasta contra outra métrica. Conserto: rever alvos ou acrescentar mecanismo que desacople
    /// as duas — não mexer mais nas constantes.
    Conflito,
    /// **Nenhum ponto do espaço** chegou a 1,5×, nem isoladamente. Não é "não achamos o ponto
    /// certo" — é "o mecanismo não existe". Conserto: construir mecanismo.
    Inalcancavel,
}

impl VereditoMetrica {
    /// `no_otimo` é a distância no ponto de melhor agregado; `melhor_isolado`, a menor distância
    /// que a métrica atingiu em QUALQUER ponto avaliado.
    fn classificar(no_otimo: f64, melhor_isolado: f64) -> Self {
        if no_otimo <= 0.0 {
            Self::Atingido
        } else if melhor_isolado > 1.5 {
            // Nem sozinha ela chega perto.
            Self::Inalcancavel
        } else if melhor_isolado <= 0.0 {
            // Sozinha ela entra; junto, não.
            Self::Conflito
        } else if no_otimo <= 1.5 {
            Self::Parcial
        } else {
            Self::Conflito
        }
    }

    /// Vereditos que impedem a busca de reportar sucesso.
    pub fn e_falha(&self) -> bool {
        matches!(self, Self::Inalcancavel | Self::Conflito)
    }

    pub fn rotulo(&self) -> &'static str {
        match self {
            Self::Atingido => "ATINGIDO",
            Self::Parcial => "PARCIAL",
            Self::Conflito => "CONFLITO",
            Self::Inalcancavel => "INALCANÇÁVEL",
        }
    }
}

/// O relatório da busca. Os cinco requisitos do plano estão todos aqui — a estrutura é o que
/// garante que eles não se percam numa impressão descuidada.
#[derive(Debug, Clone)]
pub struct RelatorioBusca {
    pub rotulo: String,
    pub avaliacoes: usize,
    pub teto: usize,
    /// **Falha quando qualquer métrica sai `Inalcancavel`.** Não existe "melhor ponto encontrado"
    /// como desfecho de sucesso.
    pub fracassou: bool,
    /// (1) distância do melhor ponto POR MÉTRICA — não só a soma, que pode esconder uma métrica
    /// catastroficamente fora.
    pub melhor_ponto: Ponto,
    pub distancias_do_melhor: Vec<(&'static str, f64, f64)>,
    /// (2) veredito por métrica.
    pub vereditos: Vec<(&'static str, VereditoMetrica)>,
    /// (3) eixos cujo ótimo ficou na BORDA da faixa varrida — o valor devolvido é suspeito.
    pub otimos_na_borda: Vec<&'static str>,
    /// (4) melhor valor que cada métrica atingiu em QUALQUER ponto, mesmo pontos que perderam no
    /// agregado.
    pub melhor_por_metrica: Vec<(&'static str, f64, f64)>,
    /// (5) o que falta, quando falta.
    pub diagnostico: Vec<String>,
    /// Ponto ótimo de uma segunda descida, partindo de um lugar distante. Se os dois convergirem
    /// para lugares diferentes, o espaço tem vales múltiplos e o relatório TEM que dizer isso em
    /// vez de apresentar um vencedor.
    pub segundo_otimo: Option<Ponto>,
    pub partidas_divergem: bool,
    /// Divergências do ORÇAMENTO DE VARIÂNCIA no ponto final, contra
    /// [`OrcamentoAlvo`](super::variancia::OrcamentoAlvo). Vazio = a distribuição está certa pelo
    /// motivo certo.
    pub falhas_de_orcamento: Vec<String>,
}

impl RelatorioBusca {
    pub fn inalcancaveis(&self) -> Vec<&'static str> {
        self.vereditos
            .iter()
            .filter(|(_, v)| *v == VereditoMetrica::Inalcancavel)
            .map(|(n, _)| *n)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Descida coordenada
// ---------------------------------------------------------------------------

/// Ponto inicial: o valor de cada eixo mais próximo de 1,0 (o neutro do perfil).
fn ponto_inicial(espaco: &[Eixo]) -> Ponto {
    espaco
        .iter()
        .map(|e| {
            let valor = e
                .valores
                .iter()
                .cloned()
                .min_by(|a, b| {
                    (a - 1.0)
                        .abs()
                        .partial_cmp(&(b - 1.0).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(1.0);
            (e.knob.nome(), valor)
        })
        .collect()
}

/// Descida coordenada com triagem: T1 peneira, T2 ordena, T3 dá o veredito nos finalistas.
///
/// Duas passadas sobre as coordenadas. Não é sofisticado de propósito — é auditável: cada passo
/// tem um "por quê" de uma linha, e num espaço de calibração isso vale mais que convergência
/// rápida para um ponto que ninguém sabe explicar.
pub fn buscar(
    rotulo: &str,
    base: ConfigTemporada,
    alvos: Alvos,
    espaco: &[Eixo],
    teto_de_avaliacoes: usize,
    semente: u64,
) -> RelatorioBusca {
    let metricas = objetivo(&alvos);
    let base_para_orcamento = base.clone();
    let mut avaliador = Avaliador::novo(base, alvos.clone(), semente, teto_de_avaliacoes);

    let mut atual = ponto_inicial(espaco);
    let mut melhor = avaliador
        .avaliar(&atual, Nivel::T2)
        .expect("primeira avaliação cabe no orçamento");

    for _passada in 0..2 {
        for eixo in espaco {
            if avaliador.esgotado() {
                break;
            }
            // T1: peneira. Descarta o grosseiramente errado antes de gastar T2.
            //
            // A promoção é RELATIVA, não absoluta: passa quem está entre os melhores da própria
            // varredura do eixo. Um corte absoluto ("nenhuma métrica além de 3×") descarta o eixo
            // inteiro quando TODO o espaço está longe do alvo — que é exatamente a situação
            // inicial de qualquer calibração real, e foi o que a primeira versão fez aqui.
            let mut medidos: Vec<(f64, f64)> = Vec::new();
            for &valor in &eixo.valores {
                if avaliador.esgotado() {
                    break;
                }
                let mut p = atual.clone();
                p.insert(eixo.knob.nome(), valor);
                if let Some(a) = avaliador.avaliar(&p, Nivel::T1) {
                    medidos.push((valor, a.total));
                }
            }
            medidos.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let quantos = (medidos.len() / 2).max(3).min(medidos.len());
            let candidatos: Vec<f64> = if medidos.is_empty() {
                eixo.valores.clone()
            } else {
                medidos[..quantos].iter().map(|(v, _)| *v).collect()
            };

            // T2: ordena os sobreviventes.
            for valor in candidatos {
                if avaliador.esgotado() {
                    break;
                }
                let mut p = atual.clone();
                p.insert(eixo.knob.nome(), valor);
                if let Some(a) = avaliador.avaliar(&p, Nivel::T2) {
                    if a.total < melhor.total {
                        melhor = a;
                        atual = p;
                    }
                }
            }
        }
    }

    // T3: veredito no finalista.
    if let Some(a) = avaliador.avaliar(&atual, Nivel::T3) {
        melhor = a;
    }

    // O PORTÃO FINAL, obrigatório (regra 6 de CAMPANHA.md): a distribuição pode estar certa pelo
    // motivo errado. Métrica de resultado não distingue "o campeonato ficou disputado" de "o
    // campeonato virou sorteio" — as duas produzem dispersão alta e vencedores variados. Só a
    // decomposição de variância separa as duas, e por isso ela roda SEMPRE no ponto final.
    let orcamento_alvo = orcamento_alvo_de(&alvos);
    let falhas_de_orcamento = {
        let config = ConfigTemporada {
            etapas: 12,
            ..base_para_orcamento.clone()
        }
        .com_ajustes(ajustes_de(&atual));
        let decomposicao = super::variancia::decompor_variancia(
            "ponto final",
            &super::variancia::ConfigDecomposicao {
                base: config,
                eventos: 8,
                replicas: 5,
                grids: 4,
            },
            semente,
        );
        orcamento_alvo.conferir(&decomposicao)
    };

    // --- Montagem do relatório ---
    let vereditos: Vec<(&'static str, VereditoMetrica)> = metricas
        .iter()
        .enumerate()
        .map(|(i, m)| {
            // O veredito cruza as DUAS leituras. Usar só o "melhor de qualquer ponto" foi o
            // defeito da primeira versão desta busca: os ATINGIDO vinham de pontos diferentes e
            // mutuamente incompatíveis, e somados davam ar de sucesso a um espaço que não
            // entrega nada junto.
            (
                m.nome,
                VereditoMetrica::classificar(
                    melhor.distancias[i],
                    avaliador.melhor_por_metrica[i].0,
                ),
            )
        })
        .collect();

    let otimos_na_borda: Vec<&'static str> = espaco
        .iter()
        .filter(|e| {
            let v = atual.get(e.knob.nome()).copied().unwrap_or(f64::NAN);
            e.valores.first() == Some(&v) || e.valores.last() == Some(&v)
        })
        .map(|e| e.knob.nome())
        .collect();

    let inalcancaveis: Vec<&'static str> = vereditos
        .iter()
        .filter(|(_, v)| *v == VereditoMetrica::Inalcancavel)
        .map(|(n, _)| *n)
        .collect();
    let conflitos: Vec<&'static str> = vereditos
        .iter()
        .filter(|(_, v)| *v == VereditoMetrica::Conflito)
        .map(|(n, _)| *n)
        .collect();
    // Sucesso exige TODAS dentro da faixa no MESMO ponto **e** o orçamento de variância certo.
    // Nem inalcançável, nem conflito, nem distribuição-certa-pelo-motivo-errado passam.
    let fracassou = vereditos.iter().any(|(_, v)| v.e_falha()) || !falhas_de_orcamento.is_empty();

    let mut diagnostico = Vec::new();
    if !inalcancaveis.is_empty() {
        diagnostico.push(format!(
            "FALHA — INALCANÇÁVEL: {}. Nenhum ponto do espaço varrido chegou a 1,5× da faixa, nem \
             isoladamente. Isto não é 'a busca não achou o ponto': é 'o mecanismo não existe no \
             espaço'. Não há valor de constante que produza o alvo; falta mecanismo.",
            inalcancaveis.join(", ")
        ));
    }
    if !conflitos.is_empty() {
        diagnostico.push(format!(
            "FALHA — CONFLITO: {}. Existe ponto no espaço que coloca cada uma dentro da faixa, mas \
             não o MESMO ponto: a alavanca existe e está sendo gasta contra outra métrica. \
             Conserto: rever alvos ou acrescentar mecanismo que desacople — mexer mais nas \
             constantes não resolve.",
            conflitos.join(", ")
        ));
    }
    if !falhas_de_orcamento.is_empty() {
        diagnostico.push(format!(
            "FALHA — DISTRIBUIÇÃO CERTA PELO MOTIVO ERRADO: as métricas de resultado passam, mas o \
             orçamento de variância no ponto final está fora do alvo em {}. Métrica de resultado \
             não distingue 'campeonato disputado' de 'campeonato sorteado' — as duas dão dispersão \
             alta e vencedores variados. Um ponto assim NÃO é calibração: é ruído dimensionado \
             para imitar disputa.",
            falhas_de_orcamento.join("; ")
        ));
    }
    if atual == ponto_inicial(espaco) {
        diagnostico.push(
            "A busca NÃO SAIU DO PONTO INICIAL: nenhum movimento de coordenada reduziu o \
             agregado. Ou o ponto de partida é ótimo local, ou cada eixo melhora uma métrica \
             piorando outras — ver a coluna CONFLITO."
                .to_string(),
        );
    }
    if !otimos_na_borda.is_empty() {
        diagnostico.push(format!(
            "SUSPEITO: o ótimo ficou na BORDA da faixa varrida em {}. Ou a faixa foi apertada \
             demais, ou o parâmetro está saturando — nos dois casos o valor devolvido não é \
             confiável.",
            otimos_na_borda.join(", ")
        ));
    }

    RelatorioBusca {
        rotulo: rotulo.to_string(),
        avaliacoes: avaliador.gastas,
        teto: teto_de_avaliacoes,
        fracassou,
        melhor_ponto: atual,
        distancias_do_melhor: metricas
            .iter()
            .enumerate()
            .map(|(i, m)| (m.nome, melhor.distancias[i], melhor.valores[i]))
            .collect(),
        vereditos,
        otimos_na_borda,
        melhor_por_metrica: metricas
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let (d, v) = avaliador.melhor_por_metrica[i];
                (m.nome, d, v)
            })
            .collect(),
        diagnostico,
        segundo_otimo: None,
        partidas_divergem: false,
        falhas_de_orcamento,
    }
}

/// Qual repartição-alvo de orçamento vale para este conjunto de alvos. A heurística é a faixa de
/// vencedores distintos, que é o que separa uma categoria de entrada de uma de topo nos alvos.
fn orcamento_alvo_de(alvos: &Alvos) -> super::variancia::OrcamentoAlvo {
    if alvos.vencedores_distintos.min >= 5.0 {
        super::variancia::OrcamentoAlvo::entrada()
    } else {
        super::variancia::OrcamentoAlvo::topo()
    }
}

/// Duas descidas de pontos de partida distantes, comparadas.
///
/// Descida coordenada depende do ponto inicial. Rodar de dois lugares e comparar é o mínimo para
/// saber se o resultado é uma propriedade do espaço ou do palpite inicial — e a divergência é
/// informação, não erro: significa vales múltiplos, e aí não existe "o" ponto ótimo a devolver.
pub fn buscar_com_duas_partidas(
    rotulo: &str,
    base: ConfigTemporada,
    alvos: Alvos,
    espaco: &[Eixo],
    teto_de_avaliacoes: usize,
    semente: u64,
) -> RelatorioBusca {
    let mut principal = buscar(
        rotulo,
        base.clone(),
        alvos.clone(),
        espaco,
        teto_de_avaliacoes / 2,
        semente,
    );

    // Segunda partida: o extremo oposto de cada eixo em relação ao neutro.
    let espaco_invertido: Vec<Eixo> = espaco
        .iter()
        .map(|e| {
            let mut valores = e.valores.clone();
            valores.reverse();
            Eixo {
                knob: e.knob,
                valores,
            }
        })
        .collect();
    let alternativa = buscar(
        rotulo,
        base,
        alvos,
        &espaco_invertido,
        teto_de_avaliacoes / 2,
        semente ^ 0xD15,
    );

    let divergem = principal.melhor_ponto != alternativa.melhor_ponto;
    if divergem {
        principal.diagnostico.push(format!(
            "PARTIDAS DIVERGEM: duas descidas de pontos iniciais distantes pararam em lugares \
             diferentes ({:?} vs {:?}). O espaço tem vales múltiplos — não existe 'o' ponto ótimo \
             a devolver, e qualquer um dos dois lido isolado engana.",
            principal.melhor_ponto, alternativa.melhor_ponto
        ));
    }
    principal.avaliacoes += alternativa.avaliacoes;
    // O teto reportado tem que ser o das DUAS descidas, senão a linha "N de M" mente.
    principal.teto = teto_de_avaliacoes;
    principal.segundo_otimo = Some(alternativa.melhor_ponto);
    principal.partidas_divergem = divergem;
    principal
}

/// O espaço de parâmetros de HOJE: só os knobs que a varredura classificou como tendo alguma
/// alavanca. Incluir os mortos apenas gastaria orçamento provando de novo o que já se sabe.
pub fn espaco_atual() -> Vec<Eixo> {
    [
        Knob::RaceVariance,
        Knob::PackDensity,
        Knob::StartChaos,
        Knob::RacePaceSpread,
        Knob::QualifyingVariance,
    ]
    .into_iter()
    .map(|knob| Eixo {
        knob,
        valores: super::varredura::faixa_padrao(knob),
    })
    .collect()
}

// ---------------------------------------------------------------------------
// Diagnóstico do próprio método
// ---------------------------------------------------------------------------

/// Ruído de amostragem do objetivo num nível: desvio-padrão do total ao repetir a MESMA
/// configuração com sementes diferentes.
///
/// É o número que responde "o T1 tem sinal suficiente para triagem?". Se o ruído for da ordem da
/// amplitude que o objetivo percorre ao longo de um eixo, o T1 está descartando ponto bom por
/// sorteio.
pub fn ruido_de_amostragem(
    base: &ConfigTemporada,
    alvos: &Alvos,
    nivel: Nivel,
    repeticoes: usize,
) -> (f64, f64) {
    let metricas = objetivo(alvos);
    let totais: Vec<f64> = (0..repeticoes)
        .map(|i| {
            let config = ConfigTemporada {
                etapas: nivel.etapas,
                ..base.clone()
            };
            let m = arena::medir("ruido", &config, nivel.temporadas, 9_000 + i as u64);
            metricas
                .iter()
                .map(|met| met.distancia((met.extrair)(&m)))
                .sum()
        })
        .collect();

    let media = totais.iter().sum::<f64>() / totais.len() as f64;
    let dp = if totais.len() > 1 {
        (totais.iter().map(|t| (t - media).powi(2)).sum::<f64>() / (totais.len() - 1) as f64).sqrt()
    } else {
        0.0
    };
    (media, dp)
}

/// **A validação certa da triagem: ARREPENDIMENTO, não concordância de ordenação.**
///
/// A guarda anterior media Spearman entre os totais do nível barato e do caro ao longo do eixo, e
/// ela é enganosa: **Spearman entre duas medições de uma função PLANA é ~0 por construção**,
/// qualquer que seja a qualidade das duas. Medido na árvore reformada, nenhuma forma de T1 passava
/// de 0,75 — e o culpado era um eixo só, `race_pace_spread_multiplier`, com ρ de 0,29 (uma semente
/// em −0,26). Ali o objetivo praticamente não responde: as duas medições estavam ordenando ruído,
/// e isso não é defeito da peneira.
///
/// O que a triagem precisa garantir não é ordenar bem — é **não jogar fora o ponto que o nível
/// caro escolheria**. Essa é a quantidade certa:
///
/// ```text
/// arrependimento = (melhor T2 entre os PROMOVIDOS − melhor T2 do eixo inteiro) / amplitude T2
/// ```
///
/// Zero = a peneira não custou nada. E num eixo plano ele é zero automaticamente, porque qualquer
/// ponto serve — que é o comportamento correto.
/// `None` quando o eixo é **plano dentro do ruído**: ali nenhum ponto é distinguivelmente melhor
/// que outro, então a peneira não pode errar e arrependimento não é quantidade definida. Dizer
/// "zero" seria mentira conveniente; dizer "não definido" é o que é.
pub fn arrependimento_da_triagem(
    base: &ConfigTemporada,
    alvos: &Alvos,
    eixo: &Eixo,
    barato: Nivel,
    caro: Nivel,
    semente: u64,
) -> Option<f64> {
    let medir_com = |nivel: Nivel, s: u64| -> Vec<f64> {
        let mut avaliador = Avaliador::novo(base.clone(), alvos.clone(), s, usize::MAX);
        eixo.valores
            .iter()
            .filter_map(|&valor| {
                let mut p: Ponto = BTreeMap::new();
                p.insert(eixo.knob.nome(), valor);
                avaliador.avaliar(&p, nivel).map(|a| a.total)
            })
            .collect()
    };
    let medir = |nivel: Nivel| medir_com(nivel, semente);
    let t1 = medir(barato);
    let t2 = medir(caro);
    if t1.len() != t2.len() || t1.len() < 3 {
        return None;
    }

    // Quem a peneira promove: a metade melhor segundo o nível barato (mesma regra de `buscar`).
    let mut indices: Vec<usize> = (0..t1.len()).collect();
    indices.sort_by(|&a, &b| {
        t1[a]
            .partial_cmp(&t1[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let quantos = (t1.len() / 2).max(3).min(t1.len());
    let promovidos = &indices[..quantos];

    let melhor_promovido = promovidos
        .iter()
        .map(|&i| t2[i])
        .fold(f64::INFINITY, f64::min);
    let melhor_global = t2.iter().cloned().fold(f64::INFINITY, f64::min);
    let pior_global = t2.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let amplitude = pior_global - melhor_global;

    // Eixo PLANO DENTRO DO RUÍDO. O limiar não é chutado: mede-se o ruído do nível caro repetindo
    // o MESMO eixo com outras sementes e comparando a amplitude contra ele. Uma primeira versão
    // usava limiar relativo à magnitude do objetivo e não funcionou — `track_difficulty`, cuja
    // alavanca medida é 0,0036, devolvia arrependimento de 0,66, que era ruído dividido por
    // quase-zero.
    let ruido = {
        let a = medir_com(caro, semente ^ 0xA1);
        let b = medir_com(caro, semente ^ 0xB2);
        if a.len() == t2.len() && b.len() == t2.len() {
            let difs: Vec<f64> = a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).collect();
            difs.iter().sum::<f64>() / difs.len() as f64
        } else {
            0.0
        }
    };

    if amplitude <= f64::EPSILON || amplitude < 2.0 * ruido {
        return None;
    }
    Some((melhor_promovido - melhor_global) / amplitude)
}

/// Concordância de ORDENAÇÃO entre dois níveis ao longo de um eixo.
///
/// Mantida como diagnóstico — ela diz o quanto o nível barato reproduz a CURVA do caro, o que é
/// informativo — mas **não serve de guarda**: num eixo plano ela vai a zero por construção. Quem
/// guarda é [`arrependimento_da_triagem`].
///
/// Sinal/ruído é um proxy e pode enganar — uma temporada curta reduz o ruído em parte porque
/// COMPRIME as métricas (com 6 etapas há no máximo 6 vencedores distintos e poucas trocas de
/// liderança), não porque meça melhor. O que a triagem precisa de fato é preservar a ORDEM: os
/// pontos que o nível caro prefere têm que estar entre os que o nível barato promove.
///
/// Devolve o Spearman entre os totais do nível barato e os do nível caro sobre os mesmos valores
/// do eixo. Acima de ~0,8 a peneira é fiel.
pub fn concordancia_de_triagem(
    base: &ConfigTemporada,
    alvos: &Alvos,
    eixo: &Eixo,
    barato: Nivel,
    caro: Nivel,
    semente: u64,
) -> f64 {
    let totais = |nivel: Nivel| -> Vec<f64> {
        let mut avaliador = Avaliador::novo(base.clone(), alvos.clone(), semente, usize::MAX);
        eixo.valores
            .iter()
            .filter_map(|&valor| {
                let mut p: Ponto = BTreeMap::new();
                p.insert(eixo.knob.nome(), valor);
                avaliador.avaliar(&p, nivel).map(|a| a.total)
            })
            .collect()
    };
    super::metricas::spearman(&totais(barato), &totais(caro)).unwrap_or(f64::NAN)
}

/// Amplitude que o objetivo percorre ao varrer UM eixo, num nível. Comparada com
/// [`ruido_de_amostragem`], dá a razão sinal/ruído da triagem.
pub fn amplitude_do_eixo(
    base: &ConfigTemporada,
    alvos: &Alvos,
    eixo: &Eixo,
    nivel: Nivel,
    semente: u64,
) -> f64 {
    let mut avaliador = Avaliador::novo(base.clone(), alvos.clone(), semente, usize::MAX);
    let mut totais = Vec::new();
    for &valor in &eixo.valores {
        let mut p: Ponto = BTreeMap::new();
        p.insert(eixo.knob.nome(), valor);
        if let Some(a) = avaliador.avaliar(&p, nivel) {
            totais.push(a.total);
        }
    }
    if totais.len() < 2 {
        return f64::NAN;
    }
    let max = totais.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min = totais.iter().cloned().fold(f64::INFINITY, f64::min);
    max - min
}
