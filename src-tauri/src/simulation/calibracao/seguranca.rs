//! **Safety car: frequência, embaralhamento, e o diagnóstico do gatilho.**
//!
//! O pacote G entregou o SC com consequência (zera os gaps) e a frequência saiu 6–10× abaixo do
//! alvo: 0,045 e 0,025 contra 0,25–0,60 e 0,15–0,40. A pergunta que decide o conserto é qual das
//! três coisas está errada, e são consertos completamente diferentes:
//!
//! 1. **Faltam batidas grandes** — a distribuição de severidade dos incidentes não produz eventos
//!    graves em volume. Conserto: mexer na severidade (ou na taxa) dos incidentes.
//! 2. **Batida grande não vira SC** — os eventos graves existem, mas o predicado do gatilho é
//!    estreito demais. Conserto: **uma linha** em `race/estrategia.rs::traz_bandeira_amarela`, e
//!    calibração nenhuma.
//! 3. **Falta mecanismo** — nem forçando a taxa dá para chegar ao alvo.
//!
//! As três se distinguem por medição, e tudo o que ela precisa já sai do [`RaceResult`]: os
//! incidentes vêm com `incident_type`, `severity` e `is_dnf`, e o SC vem em
//! `RaceResult::safety_cars`. O predicado é reusado de `race::estrategia::traz_bandeira_amarela` —
//! reimplementá-lo aqui mediria a diferença entre duas cópias.
//!
//! O gatilho atual, para referência:
//!
//! | tipo × severidade | vira SC? |
//! |---|---|
//! | Collision × Critical | sempre |
//! | Collision × Major | só se DNF |
//! | DriverError × Critical | só se DNF |
//! | qualquer outro | nunca |
//!
//! Duas das três linhas exigem DNF, e é aí que a hipótese do gatilho estreito mora.

use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};
use crate::simulation::race::estrategia::traz_bandeira_amarela;
use crate::simulation::race::RaceResult;

use super::arena::{self, ConfigTemporada};

/// Predicado ALARGADO: o mesmo do jogo, mas sem exigir DNF nas duas linhas que o exigem.
///
/// É o contrafactual do diagnóstico: se soltar o `is_dnf` multiplicar os SCs, o problema é o
/// gatilho; se quase não mudar, os eventos graves é que não existem.
pub fn traz_bandeira_amarela_alargado(inc: &IncidentResult) -> bool {
    match (inc.incident_type, inc.severity) {
        (IncidentType::Collision, IncidentSeverity::Critical) => true,
        (IncidentType::Collision, IncidentSeverity::Major) => true,
        (IncidentType::DriverError, IncidentSeverity::Critical) => true,
        _ => false,
    }
}

/// Quantos incidentes "graves" existem, independentemente de virarem SC: qualquer `Critical`, mais
/// colisão `Major`. É o numerador da hipótese 1.
pub fn e_grave(inc: &IncidentResult) -> bool {
    matches!(inc.severity, IncidentSeverity::Critical)
        || matches!(
            (inc.incident_type, inc.severity),
            (IncidentType::Collision, IncidentSeverity::Major)
        )
}

#[derive(Debug, Clone)]
pub struct DiagnosticoDoGatilho {
    pub rotulo: String,
    pub corridas: usize,

    /// Incidentes de qualquer natureza por corrida.
    pub incidentes_por_corrida: f64,
    /// Incidentes GRAVES por corrida (`Critical` de qualquer tipo, ou colisão `Major`).
    pub graves_por_corrida: f64,
    /// Graves que de fato satisfazem o gatilho ATUAL.
    pub qualificam_atual: f64,
    /// Graves que satisfariam o gatilho ALARGADO (sem exigir DNF).
    pub qualificam_alargado: f64,
    /// Safety cars por corrida, como o motor produziu.
    pub scs_por_corrida: f64,

    /// `qualificam_atual / graves_por_corrida` — quanto da gravidade o gatilho aproveita.
    pub aproveitamento_do_gatilho: f64,
    /// `qualificam_alargado / qualificam_atual` — o multiplicador que alargar o gatilho daria.
    pub ganho_de_alargar: f64,
    /// `scs_por_corrida / qualificam_atual` — o motor converte o que qualifica? Deveria ser ~1 por
    /// segmento; abaixo disso há qualificação que não virou SC (dois no mesmo segmento contam uma
    /// vez, então < 1 é esperado e não é defeito).
    pub conversao: f64,
}

impl DiagnosticoDoGatilho {
    /// Frequência de SC projetada se o gatilho fosse alargado, mantida a conversão medida. É a
    /// resposta quantitativa: alargar chega ao alvo, ou só parte do caminho?
    pub fn sc_projetado_alargando(&self) -> f64 {
        self.qualificam_alargado * self.conversao
    }

    /// Quanto mais gravidade ainda faltaria DEPOIS de alargar, para bater o piso do alvo.
    /// `<= 1.0` significa que alargar sozinho resolve.
    pub fn fator_de_gravidade_faltante(&self, alvo: super::alvos::Faixa) -> f64 {
        let projetado = self.sc_projetado_alargando();
        if projetado <= f64::EPSILON {
            return f64::INFINITY;
        }
        (alvo.min / projetado).max(1.0)
    }

    /// A leitura quantitativa. A versão categórica desta função era pior: com um corte em
    /// "0,10 graves por corrida" ela chamava gt3 (0,093) de "faltam batidas grandes" e rookie
    /// (0,242) de "gatilho estreito", quando na verdade **as duas coisas contribuem nas duas
    /// categorias**, em proporções diferentes. Um veredito de faca no fio esconde isso.
    pub fn veredito(&self, alvo: super::alvos::Faixa) -> String {
        let projetado = self.sc_projetado_alargando();
        let faltante = self.fator_de_gravidade_faltante(alvo);

        let parte_do_gatilho = format!(
            "Alargar o gatilho (soltar o `is_dnf` das duas linhas que o exigem) multiplica os \
             eventos qualificados por {:.1}× e levaria a frequência de {:.3} para ~{:.3} SC/corrida.",
            self.ganho_de_alargar, self.scs_por_corrida, projetado
        );

        if faltante <= 1.0 {
            format!(
                "GATILHO ESTREITO, E SÓ. {parte_do_gatilho} Isso já entra no alvo ({:.2}–{:.2}). \
                 Conserto: uma linha em `traz_bandeira_amarela`, calibração nenhuma.",
                alvo.min, alvo.max
            )
        } else if faltante <= 2.0 {
            format!(
                "GATILHO ESTREITO EM PRIMEIRO LUGAR. {parte_do_gatilho} Ainda ficaria {:.1}× abaixo \
                 do piso do alvo ({:.2}), então depois de alargar sobra um ajuste modesto de \
                 gravidade — mas o alargamento é o passo grande e é grátis.",
                faltante, alvo.min
            )
        } else {
            format!(
                "OS DOIS, COM A GRAVIDADE DOMINANDO. {parte_do_gatilho} Mesmo assim ficaria {:.1}× \
                 abaixo do piso ({:.2}): há só {:.3} incidentes graves por corrida, e alargar o \
                 predicado não cria gravidade que não existe. Ordem: alargar primeiro (é grátis), \
                 depois subir severidade/taxa de incidente pelo fator que sobrar.",
                faltante, alvo.min, self.graves_por_corrida
            )
        }
    }
}

fn incidentes(corrida: &RaceResult) -> impl Iterator<Item = &IncidentResult> {
    corrida.race_results.iter().flat_map(|r| r.incidents.iter())
}

/// Roda uma campanha e diagnostica o gatilho. Exige `incidentes: true` na config — sem incidentes
/// não há o que neutralizar e o diagnóstico não significa nada.
pub fn diagnosticar_gatilho(
    rotulo: &str,
    config: &ConfigTemporada,
    temporadas: usize,
    semente: u64,
) -> DiagnosticoDoGatilho {
    let campanha = arena::rodar_campanha_crua(config, temporadas, semente);
    let corridas: Vec<&RaceResult> = campanha.iter().flat_map(|(_, c)| c.iter()).collect();
    let n = corridas.len().max(1) as f64;

    let mut total = 0.0;
    let mut graves = 0.0;
    let mut atual = 0.0;
    let mut alargado = 0.0;
    let mut scs = 0.0;

    for corrida in &corridas {
        scs += corrida.safety_cars.len() as f64;
        for inc in incidentes(corrida) {
            total += 1.0;
            if e_grave(inc) {
                graves += 1.0;
            }
            if traz_bandeira_amarela(inc) {
                atual += 1.0;
            }
            if traz_bandeira_amarela_alargado(inc) {
                alargado += 1.0;
            }
        }
    }

    let razao = |a: f64, b: f64| if b > 0.0 { a / b } else { f64::NAN };

    DiagnosticoDoGatilho {
        rotulo: rotulo.to_string(),
        corridas: corridas.len(),
        incidentes_por_corrida: total / n,
        graves_por_corrida: graves / n,
        qualificam_atual: atual / n,
        qualificam_alargado: alargado / n,
        scs_por_corrida: scs / n,
        aproveitamento_do_gatilho: razao(atual, graves),
        ganho_de_alargar: razao(alargado, atual),
        conversao: razao(scs, atual),
    }
}

/// Alvos do safety car, por categoria (do briefing do pacote G).
pub fn alvo_de_frequencia(category_id: &str) -> super::alvos::Faixa {
    if category_id.starts_with("mazda") || category_id.starts_with("toyota") {
        super::alvos::Faixa::nova(0.25, 0.60)
    } else {
        super::alvos::Faixa::nova(0.15, 0.40)
    }
}
