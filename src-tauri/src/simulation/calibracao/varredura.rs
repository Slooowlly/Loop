//! **Varredura de sensibilidade** — quais knobs têm alavanca e quais são decorativos.
//!
//! Varre um multiplicador do [`SimulationContext`] numa faixa e reporta como as métricas
//! respondem. A pergunta prática: antes de passar semanas ajustando constante, saber quais delas
//! sequer movem o resultado.
//!
//! O veredito de cada knob sai de uma medida simples e honesta: a **amplitude** que a métrica de
//! interesse percorre entre o menor e o maior valor varrido. Se um knob varia 20× e a correlação
//! entre etapas anda 0,01, ele é decorativo — e nenhuma calibração futura vai extrair dele o que
//! ele não tem.
//!
//! Os knobs são campos públicos do contexto, então a varredura sobrescreve depois que o perfil da
//! categoria resolveu ([`AjustesCtx`]). Nada em `profile/**` é tocado.

use crate::simulation::race::trafego::ParametrosDeTrafego;

use super::arena::{self, AjustesCtx, ConfigTemporada};
use super::metricas::MetricasAgregadas;

/// Os multiplicadores varridos. São exatamente os que já existem hoje no contexto e que o pacote
/// E vai querer separar por categoria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    RaceVariance,
    RacePaceSpread,
    StartChaos,
    QualifyingVariance,
    PackDensity,
    IncidentRate,
    /// Calculado pelo perfil e **nunca lido** pela simulação hoje. Está na varredura de
    /// propósito: o resultado esperado é alavanca exatamente zero, e isso é a demonstração
    /// mecânica de que o pacote D tem trabalho a fazer, não uma constante a ajustar.
    OvertakingDifficulty,
    TrackDifficulty,
    /// Segundo knob órfão, encontrado pela guarda de [`super::consumo`]: o perfil calcula, o
    /// contexto guarda, e o único consumidor possível (`math::adjusted_weather_multiplier`) não
    /// é chamado por ninguém. Mesma expectativa de alavanca zero exata.
    RainSensitivity,

    // ── As constantes de POSIÇÃO NA PISTA ────────────────────────────────────────────────
    //
    // Nasceram `const` dentro de `race::trafego` e por isso ficavam INVISÍVEIS para esta
    // varredura, que só enxergava campo de contexto. Isso é o pior arranjo possível: a busca
    // fecha os knobs externos por cima de um conjunto interno que ninguém mediu, e o erro de
    // baixo fica travado debaixo de uma calibração com cara de boa.
    //
    // Elas são as que decidem QUANTO VALE LARGAR NA FRENTE, então a saída de interesse aqui
    // não é só ρ(N,N+1) — é `ρ(grid × chegada)`, medida por
    // [`Saida::RhoGridChegada`]. As cinco constantes de rivalidade e o
    // `RISCO_DE_CONTATO_NA_TENTATIVA_FALHA` continuam fora de propósito: a primeira é decisão
    // de desenho, o segundo está medido contra a tabela de órfãos de 26 temporadas.
    /// Janela de ar sujo, em ms.
    JanelaArSujo,
    /// Perda máxima de ritmo no ar sujo, em pontos.
    PerdaMaximaArSujo,
    /// Espaçamento mínimo entre dois carros em fila, em ms — o "não dá para atravessar".
    GapMinimoEntreCarros,
    /// Janela dentro da qual dá para TENTAR passar, em ms.
    JanelaDeAtaque,
    /// Chance base de a tentativa dar certo em condições neutras.
    ProbBaseUltrapassagem,
    /// Delta de ritmo que satura a chance de passar.
    DeltaDeRitmoQueSatura,
    /// Peso de `racecraft − defesa` na chance.
    PesoDaHabilidadeNaUltrapassagem,
    /// Peso da agressividade na chance.
    PesoDaAgressividadeNaUltrapassagem,
    /// Tempo perdido pelo atacante numa tentativa falha, em ms.
    CustoTentativaFalhaAtacante,
    /// Tempo perdido pelo defensor numa tentativa falha, em ms.
    CustoTentativaFalhaDefensor,
}

impl Knob {
    pub fn nome(&self) -> &'static str {
        match self {
            Self::RaceVariance => "race_variance_multiplier",
            Self::RacePaceSpread => "race_pace_spread_multiplier",
            Self::StartChaos => "start_chaos_multiplier",
            Self::QualifyingVariance => "qualifying_variance_multiplier",
            Self::PackDensity => "pack_density_factor",
            Self::IncidentRate => "incident_rate_multiplier",
            Self::OvertakingDifficulty => "overtaking_difficulty_multiplier",
            Self::TrackDifficulty => "track_difficulty_multiplier",
            Self::RainSensitivity => "rain_sensitivity",
            Self::JanelaArSujo => "janela_ar_sujo_ms",
            Self::PerdaMaximaArSujo => "perda_maxima_ar_sujo_pontos",
            Self::GapMinimoEntreCarros => "gap_minimo_entre_carros_ms",
            Self::JanelaDeAtaque => "janela_de_ataque_ms",
            Self::ProbBaseUltrapassagem => "prob_base_ultrapassagem",
            Self::DeltaDeRitmoQueSatura => "delta_de_ritmo_que_satura",
            Self::PesoDaHabilidadeNaUltrapassagem => "peso_da_habilidade_na_ultrapassagem",
            Self::PesoDaAgressividadeNaUltrapassagem => "peso_da_agressividade_na_ultrapassagem",
            Self::CustoTentativaFalhaAtacante => "custo_tentativa_falha_atacante_ms",
            Self::CustoTentativaFalhaDefensor => "custo_tentativa_falha_defensor_ms",
        }
    }

    /// Alguns knobs só fazem sentido com incidentes ligados.
    pub fn exige_incidentes(&self) -> bool {
        matches!(self, Self::IncidentRate)
    }

    /// Knob de POSIÇÃO NA PISTA — vive em [`ParametrosDeTrafego`], não no contexto.
    ///
    /// A diferença importa na faixa varrida: os do contexto são multiplicadores adimensionais
    /// que giram em torno de 1,0, e estes são valores ABSOLUTOS na unidade da constante (ms,
    /// pontos, fração). Varrer os dois com a mesma lista `[0 … 10]` daria janela de ar sujo de
    /// 10 ms, que é o mesmo que desligá-la.
    pub fn e_de_trafego(&self) -> bool {
        matches!(
            self,
            Self::JanelaArSujo
                | Self::PerdaMaximaArSujo
                | Self::GapMinimoEntreCarros
                | Self::JanelaDeAtaque
                | Self::ProbBaseUltrapassagem
                | Self::DeltaDeRitmoQueSatura
                | Self::PesoDaHabilidadeNaUltrapassagem
                | Self::PesoDaAgressividadeNaUltrapassagem
                | Self::CustoTentativaFalhaAtacante
                | Self::CustoTentativaFalhaDefensor
        )
    }

    /// O valor que o jogo roda hoje. Só existe para knob de tráfego, que é onde a faixa de
    /// varredura precisa ser relativa ao valor atual em vez de absoluta.
    pub fn valor_de_hoje(&self) -> Option<f64> {
        let p = ParametrosDeTrafego::PADRAO;
        Some(match self {
            Self::JanelaArSujo => p.janela_ar_sujo_ms,
            Self::PerdaMaximaArSujo => p.perda_maxima_ar_sujo_pontos,
            Self::GapMinimoEntreCarros => p.gap_minimo_entre_carros_ms,
            Self::JanelaDeAtaque => p.janela_de_ataque_ms,
            Self::ProbBaseUltrapassagem => p.prob_base_ultrapassagem,
            Self::DeltaDeRitmoQueSatura => p.delta_de_ritmo_que_satura,
            Self::PesoDaHabilidadeNaUltrapassagem => p.peso_da_habilidade_na_ultrapassagem,
            Self::PesoDaAgressividadeNaUltrapassagem => p.peso_da_agressividade_na_ultrapassagem,
            Self::CustoTentativaFalhaAtacante => p.custo_tentativa_falha_atacante_ms,
            Self::CustoTentativaFalhaDefensor => p.custo_tentativa_falha_defensor_ms,
            _ => return None,
        })
    }

    pub(super) fn aplicar(&self, valor: f64) -> AjustesCtx {
        let mut a = AjustesCtx::default();
        match self {
            Self::RaceVariance => a.race_variance_multiplier = Some(valor),
            Self::RacePaceSpread => a.race_pace_spread_multiplier = Some(valor),
            Self::StartChaos => a.start_chaos_multiplier = Some(valor),
            Self::QualifyingVariance => a.qualifying_variance_multiplier = Some(valor),
            Self::PackDensity => a.pack_density_factor = Some(valor),
            Self::IncidentRate => a.incident_rate_multiplier = Some(valor),
            Self::OvertakingDifficulty => a.overtaking_difficulty_multiplier = Some(valor),
            Self::TrackDifficulty => a.track_difficulty_multiplier = Some(valor),
            Self::RainSensitivity => a.rain_sensitivity = Some(valor),
            Self::JanelaArSujo => a.trafego.janela_ar_sujo_ms = Some(valor),
            Self::PerdaMaximaArSujo => a.trafego.perda_maxima_ar_sujo_pontos = Some(valor),
            Self::GapMinimoEntreCarros => a.trafego.gap_minimo_entre_carros_ms = Some(valor),
            Self::JanelaDeAtaque => a.trafego.janela_de_ataque_ms = Some(valor),
            Self::ProbBaseUltrapassagem => a.trafego.prob_base_ultrapassagem = Some(valor),
            Self::DeltaDeRitmoQueSatura => a.trafego.delta_de_ritmo_que_satura = Some(valor),
            Self::PesoDaHabilidadeNaUltrapassagem => {
                a.trafego.peso_da_habilidade_na_ultrapassagem = Some(valor)
            }
            Self::PesoDaAgressividadeNaUltrapassagem => {
                a.trafego.peso_da_agressividade_na_ultrapassagem = Some(valor)
            }
            Self::CustoTentativaFalhaAtacante => {
                a.trafego.custo_tentativa_falha_atacante_ms = Some(valor)
            }
            Self::CustoTentativaFalhaDefensor => {
                a.trafego.custo_tentativa_falha_defensor_ms = Some(valor)
            }
        }
        a
    }

    /// Todos, na ordem em que entram no relatório.
    pub fn todos() -> [Knob; 19] {
        [
            Self::RaceVariance,
            Self::RacePaceSpread,
            Self::StartChaos,
            Self::QualifyingVariance,
            Self::PackDensity,
            Self::IncidentRate,
            Self::OvertakingDifficulty,
            Self::TrackDifficulty,
            Self::RainSensitivity,
            Self::JanelaArSujo,
            Self::PerdaMaximaArSujo,
            Self::GapMinimoEntreCarros,
            Self::JanelaDeAtaque,
            Self::ProbBaseUltrapassagem,
            Self::DeltaDeRitmoQueSatura,
            Self::PesoDaHabilidadeNaUltrapassagem,
            Self::PesoDaAgressividadeNaUltrapassagem,
            Self::CustoTentativaFalhaAtacante,
            Self::CustoTentativaFalhaDefensor,
        ]
    }

    /// Só os de POSIÇÃO NA PISTA. É o recorte que a medição de A1.1 varre: os outros nove já
    /// têm varredura publicada e rodá-los de novo só gasta CPU.
    pub fn de_trafego() -> Vec<Knob> {
        Self::todos()
            .into_iter()
            .filter(Knob::e_de_trafego)
            .collect()
    }
}

/// **As SAÍDAS medidas.** Um knob não é "morto" ou "alavanca" em abstrato — é morto *para uma
/// saída*. A primeira versão desta varredura media só correlação de resultado e desvio de posição,
/// e chamou `incident_rate_multiplier` de fraco; mas frequência de safety car é outra saída, e um
/// knob pode ter alavanca forte lá e nenhuma em ρ.
///
/// O veredito virou por PAR (knob × saída) por causa disso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saida {
    /// ρ entre etapas consecutivas — o sintoma central.
    RhoConsecutivas,
    /// Desvio-padrão da posição de chegada.
    DesvioPosicao,
    /// Vencedores distintos por temporada.
    VencedoresDistintos,
    /// **Safety cars por etapa** — a saída que a primeira versão não media.
    FrequenciaDeSc,
    /// ρ(ordem pré-SC × chegada) — o quanto o safety car embaralha.
    EmbaralhamentoDoSc,
    /// Abandonos por etapa.
    DnfsPorEtapa,
    /// **ρ(grid × chegada)** — o quanto largar na frente decide a chegada.
    ///
    /// Entrou junto com os knobs de posição na pista, e sem ela a varredura deles seria cega:
    /// as constantes de ar sujo, trem e ultrapassagem existem exatamente para mover ESTA saída,
    /// e nenhuma das cinco anteriores a media. Um knob que empurra ρ(grid) de 0,18 para 0,55 e
    /// deixa ρ(N,N+1) parado sairia como "morto" na tabela antiga.
    RhoGridChegada,
}

impl Saida {
    pub fn nome(&self) -> &'static str {
        match self {
            Self::RhoConsecutivas => "ρ(N,N+1)",
            Self::DesvioPosicao => "desvio pos.",
            Self::VencedoresDistintos => "vencedores",
            Self::FrequenciaDeSc => "SC/etapa",
            Self::EmbaralhamentoDoSc => "ρ(pré-SC)",
            Self::DnfsPorEtapa => "DNF/etapa",
            Self::RhoGridChegada => "ρ(grid)",
        }
    }

    pub fn extrair(&self, m: &MetricasAgregadas) -> f64 {
        match self {
            Self::RhoConsecutivas => m.spearman_etapas_consecutivas,
            Self::DesvioPosicao => m.desvio_posicao,
            Self::VencedoresDistintos => m.vencedores_distintos,
            Self::FrequenciaDeSc => m.scs_por_etapa,
            Self::EmbaralhamentoDoSc => m.rho_pre_sc_chegada,
            Self::DnfsPorEtapa => m.dnfs_por_etapa,
            Self::RhoGridChegada => m.spearman_grid_chegada,
        }
    }

    /// Amplitude a partir da qual a saída é considerada movida de verdade. Escala por saída: 0,02
    /// de ρ é invisível, 0,02 de SC/etapa é uma mudança de 8% num valor que vive perto de 0,25.
    pub fn limiar_de_alavanca(&self) -> (f64, f64) {
        match self {
            Self::RhoConsecutivas | Self::EmbaralhamentoDoSc | Self::RhoGridChegada => (0.02, 0.10),
            Self::DesvioPosicao => (0.30, 1.00),
            Self::VencedoresDistintos => (0.30, 1.00),
            Self::FrequenciaDeSc => (0.02, 0.10),
            Self::DnfsPorEtapa => (0.10, 0.50),
        }
    }

    pub fn todas() -> [Saida; 7] {
        [
            Self::RhoConsecutivas,
            Self::DesvioPosicao,
            Self::VencedoresDistintos,
            Self::FrequenciaDeSc,
            Self::EmbaralhamentoDoSc,
            Self::DnfsPorEtapa,
            Self::RhoGridChegada,
        ]
    }
}

/// Um ponto da varredura.
#[derive(Debug, Clone)]
pub struct PontoVarredura {
    pub valor: f64,
    pub metricas: MetricasAgregadas,
}

/// O resultado completo de varrer um knob.
#[derive(Debug, Clone)]
pub struct Varredura {
    pub knob: Knob,
    pub categoria: String,
    pub pontos: Vec<PontoVarredura>,
}

impl Varredura {
    fn extremos(&self, f: impl Fn(&MetricasAgregadas) -> f64) -> Option<(f64, f64)> {
        let valores: Vec<f64> = self
            .pontos
            .iter()
            .map(|p| f(&p.metricas))
            .filter(|v| v.is_finite())
            .collect();
        if valores.len() < 2 {
            return None;
        }
        Some((
            valores.iter().cloned().fold(f64::INFINITY, f64::min),
            valores.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        ))
    }

    /// Amplitude percorrida pela correlação entre etapas consecutivas — a métrica que mede o
    /// sintoma. É a alavanca principal de um knob.
    pub fn alavanca_consecutivas(&self) -> f64 {
        self.extremos(|m| m.spearman_etapas_consecutivas)
            .map(|(min, max)| max - min)
            .unwrap_or(f64::NAN)
    }

    /// Amplitude percorrida pelo desvio-padrão da posição de chegada, em posições.
    pub fn alavanca_desvio_posicao(&self) -> f64 {
        self.extremos(|m| m.desvio_posicao)
            .map(|(min, max)| max - min)
            .unwrap_or(f64::NAN)
    }

    /// Amplitude percorrida por UMA saída ao longo de toda a faixa varrida.
    pub fn alavanca(&self, saida: Saida) -> f64 {
        self.extremos(|m| saida.extrair(m))
            .map(|(min, max)| max - min)
            .unwrap_or(f64::NAN)
    }

    /// Veredito do PAR (este knob × esta saída). É a forma certa: "morto" nunca é propriedade só
    /// do knob.
    pub fn veredito_de(&self, saida: Saida) -> &'static str {
        let a = self.alavanca(saida);
        if !a.is_finite() {
            return "?";
        }
        let (morto, forte) = saida.limiar_de_alavanca();
        if a < morto {
            "MORTO"
        } else if a < forte {
            "fraco"
        } else {
            "ALAVANCA"
        }
    }

    /// Em quais saídas este knob tem alavanca de verdade.
    pub fn saidas_com_alavanca(&self) -> Vec<Saida> {
        Saida::todas()
            .into_iter()
            .filter(|s| self.veredito_de(*s) == "ALAVANCA")
            .collect()
    }

    /// Veredito CONSOLIDADO: o melhor que este knob consegue em qualquer saída. Um knob só é morto
    /// quando é morto em TODAS — que é a afirmação que a primeira versão fazia sem ter medido.
    pub fn veredito(&self) -> &'static str {
        let vereditos: Vec<&'static str> = Saida::todas()
            .iter()
            .map(|s| self.veredito_de(*s))
            .collect();
        if vereditos.iter().any(|v| *v == "ALAVANCA") {
            "ALAVANCA"
        } else if vereditos.iter().any(|v| *v == "fraco") {
            "fraco"
        } else if vereditos.iter().all(|v| *v == "?") {
            "?"
        } else {
            "MORTO"
        }
    }
}

/// Faixa padrão de varredura de um knob: de bem abaixo a bem acima do que qualquer categoria usa
/// hoje. Deliberadamente generosa — se um knob só produz efeito com valor absurdo, isso é um
/// achado sobre o desenho, não sobre a calibração.
pub fn faixa_padrao(knob: Knob) -> Vec<f64> {
    // Knob de tráfego tem UNIDADE (ms, pontos, fração), então a faixa é multiplicativa sobre o
    // valor de hoje. `0×` é o desligamento total da constante, e ele é o ponto mais informativo
    // da varredura: se apagar a janela de ar sujo não move nada, a constante é decorativa
    // independentemente do valor que se escolha para ela.
    if let Some(hoje) = knob.valor_de_hoje() {
        return [0.0, 0.25, 0.5, 1.0, 1.5, 2.0, 3.0]
            .iter()
            .map(|m| m * hoje)
            .collect();
    }
    match knob {
        Knob::RacePaceSpread => vec![0.25, 0.5, 0.85, 1.0, 1.3, 2.0, 3.0, 5.0],
        _ => vec![0.0, 0.25, 0.5, 1.0, 1.4, 2.5, 5.0, 10.0],
    }
}

/// Varre um knob e mede a resposta em cada ponto.
pub fn varrer(
    categoria: &str,
    base: &ConfigTemporada,
    knob: Knob,
    valores: &[f64],
    temporadas: usize,
    semente: u64,
) -> Varredura {
    let pontos = valores
        .iter()
        .map(|&valor| {
            let mut config = base.clone().com_ajustes(knob.aplicar(valor));
            if knob.exige_incidentes() {
                config.incidentes = true;
            }
            PontoVarredura {
                valor,
                metricas: arena::medir(
                    &format!("{}={valor}", knob.nome()),
                    &config,
                    temporadas,
                    semente,
                ),
            }
        })
        .collect();

    Varredura {
        knob,
        categoria: categoria.to_string(),
        pontos,
    }
}

/// Varre todos os knobs de uma categoria com a faixa padrão.
pub fn varrer_todos(
    categoria: &str,
    base: &ConfigTemporada,
    temporadas: usize,
    semente: u64,
) -> Vec<Varredura> {
    Knob::todos()
        .into_iter()
        .map(|knob| {
            varrer(
                categoria,
                base,
                knob,
                &faixa_padrao(knob),
                temporadas,
                semente,
            )
        })
        .collect()
}
