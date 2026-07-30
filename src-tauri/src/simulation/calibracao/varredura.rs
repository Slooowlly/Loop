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
        }
    }

    /// Alguns knobs só fazem sentido com incidentes ligados.
    pub fn exige_incidentes(&self) -> bool {
        matches!(self, Self::IncidentRate)
    }

    fn aplicar(&self, valor: f64) -> AjustesCtx {
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
        }
        a
    }

    /// Todos, na ordem em que entram no relatório.
    pub fn todos() -> [Knob; 9] {
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
        ]
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
        }
    }

    /// Amplitude a partir da qual a saída é considerada movida de verdade. Escala por saída: 0,02
    /// de ρ é invisível, 0,02 de SC/etapa é uma mudança de 8% num valor que vive perto de 0,25.
    pub fn limiar_de_alavanca(&self) -> (f64, f64) {
        match self {
            Self::RhoConsecutivas | Self::EmbaralhamentoDoSc => (0.02, 0.10),
            Self::DesvioPosicao => (0.30, 1.00),
            Self::VencedoresDistintos => (0.30, 1.00),
            Self::FrequenciaDeSc => (0.02, 0.10),
            Self::DnfsPorEtapa => (0.10, 0.50),
        }
    }

    pub fn todas() -> [Saida; 6] {
        [
            Self::RhoConsecutivas,
            Self::DesvioPosicao,
            Self::VencedoresDistintos,
            Self::FrequenciaDeSc,
            Self::EmbaralhamentoDoSc,
            Self::DnfsPorEtapa,
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
