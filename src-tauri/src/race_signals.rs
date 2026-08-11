//! Os SINAIS da corrida: a definição ÚNICA de cada conceito que os motores de
//! narrativa leem de um resultado — e **um limiar por conceito**.
//!
//! Existiam três leituras paralelas dos mesmos fatos (o debrief do jogador em
//! `commands/ai_news/`, o boletim do grid em `narrative/`, e a manchete de
//! `race_eval`). Cada uma redescobria "remontada", "colapso" e "abandono
//! mecânico" com o seu próprio critério — "remontada" chegou a ter QUATRO
//! limiares (>0, 4, 5, 8) e o mesmo DNF era batida para um motor e pane para o
//! outro, porque um lia o `IncidentType` e o outro rodava regex no motivo.
//!
//! Aqui mora o FATO. O que continua de fora, de propósito, é a VOZ: a ordem de
//! eleição da tese, os textos e os pesos editoriais seguem em cada motor — o
//! debrief fala em 1ª pessoa, o boletim tem voz de revista, e é certo que eles
//! escolham protagonistas diferentes a partir das mesmas flags.
//!
//! Módulo PURO (sem `rusqlite`), consumível pelo `narrative`.

use crate::race_eval::Assessment;
use crate::simulation::incidents::{IncidentResult, IncidentType};

// ───────────────────────── Limiares (um por conceito) ─────────────────────────

/// Ganho de posições que já conta como REMONTADA.
///
/// **Nota de calibração (fechada em 11/08/2026).** Os quatro motores usavam quatro
/// limiares (>0, 4, 5, 8) e a unificação teve de escolher um. Ficou **4**, que era o do
/// boletim, e não o 5 do debrief — o debrief perde uma posição de exigência. A escolha é
/// deliberada: `remontada` é o predicado GROSSO, o que decide se o assunto entra na
/// conversa, e o conceito de manchete tem constante própria
/// ([`REMONTADA_EPICA_MIN`], 8, com chegada no top-6). Com 5, um grid de 12 exigia
/// atravessar quase metade do pelotão para a corrida ser "de recuperação", e a
/// diferença entre subir 4 e subir 5 lugares não é a diferença entre ter havido e não
/// ter havido remontada. Não é medição — é onde a régua foi cravada, e mexer aqui é
/// decisão de design.
pub const REMONTADA_MIN: i32 = 4;
/// Remontada que vira MANCHETE: ganho grande E terminando na frente. É um
/// conceito próprio (não um segundo limiar de [`remontada`]) — a revista só
/// promove a eixo da matéria quem subiu muito e chegou onde se vê.
pub const REMONTADA_EPICA_MIN: i32 = 8;
/// Chegada máxima para a remontada ser épica.
pub const REMONTADA_EPICA_MAX_FINISH: i32 = 6;
/// Perda de posições que já conta como COLAPSO (negativo).
pub const COLAPSO_MAX: i32 = -4;
/// Chegada a partir da qual a pole que não venceu conta como AFUNDADA.
pub const POLE_FRUSTRADA_MIN_FINISH: i32 = 5;
/// Largada a partir da qual vencer conta como VITÓRIA IMPROVÁVEL.
pub const VITORIA_IMPROVAVEL_MIN_GRID: i32 = 6;
/// Piso absoluto de abandonos para a corrida ser CAÓTICA.
pub const CAOS_MIN_DNFS: i32 = 4;
/// ...e a fração do grid (1/N) que também qualifica, em grids grandes.
pub const CAOS_FIELD_DIVISOR: i32 = 4;

// ──────────────────────────── Natureza do abandono ────────────────────────────

/// Por que o carro parou. A pergunta que o jogador faz primeiro ("o carro me
/// traiu ou eu errei?") e que os dois motores respondiam de formas diferentes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnfKind {
    /// O carro falhou (peça, motor, câmbio...). Ninguém bateu.
    Mecanico,
    /// Contato entre carros.
    Contato,
    /// Erro de pilotagem sem outro carro envolvido.
    Erro,
    /// Sem evidência suficiente para dizer.
    Desconhecido,
}

impl DnfKind {
    pub fn is_mecanico(self) -> bool {
        self == DnfKind::Mecanico
    }

    /// "Batida" no sentido do boletim: alguém se meteu numa — por contato ou por
    /// erro próprio. Pane mecânica não conta.
    ///
    /// **Não olha a SEVERIDADE, e isso está resolvido, não esquecido.** A regra antiga
    /// tirava de "batida" o `DriverError` de severidade `Minor`; a investigação (fechada
    /// em 11/08/2026) mostrou que esse caso não existe no motor: `roll_driver_error`
    /// devolve `Minor ⇒ segue na prova` / `Major ⇒ abandona`, e a escalada por stall
    /// sobe severidade e abandono JUNTOS. Erro de pilotagem que abandona é sempre
    /// `Major` ou pior, então filtrar por severidade não removeria caso nenhum.
    ///
    /// O acoplamento é do motor, não deste módulo — e por isso o guard mora lá:
    /// `simulation::incidents::tests::erro_de_pilotagem_com_abandono_nunca_sai_minor`
    /// varre 1500 segmentos e quebra se alguém desacoplar as duas coisas. A colisão fica
    /// de fora de propósito: lá severidade e consequência são sorteadas em separado, e
    /// contato é batida em qualquer severidade.
    pub fn is_crash(self) -> bool {
        matches!(self, DnfKind::Contato | DnfKind::Erro)
    }
}

/// Palavras que denunciam falha do CARRO no motivo textual do abandono. Saves
/// feitos no locale inglês guardam o motivo em inglês, por isso as duas listas.
const PALAVRAS_MECANICAS: &[&str] = &[
    // PT
    "motor",
    "câmbio",
    "cambio",
    "mecân",
    "mecan",
    "suspens",
    "freio",
    "transmiss",
    "embreagem",
    "turbo",
    "óleo",
    "oleo",
    "superaquec",
    "pane",
    "elétric",
    "eletric",
    "diferencial",
    // "bateria" precisa estar AQUI, e não só por completude: `PALAVRAS_BATIDA` carrega
    // "bate", que é prefixo dela. As mecânicas são testadas primeiro, então a precedência é
    // o que impede "bateria descarregou" de virar batida.
    "bateria",
    // EN
    "engine",
    "gearbox",
    "mechanic",
    "suspension",
    "brake",
    "clutch",
    "oil",
    "overheat",
    "electric",
    "differential",
    "failure",
    "battery",
];

/// ...e as que denunciam BATIDA.
const PALAVRAS_BATIDA: &[&str] = &[
    // PT
    "batida",
    "bateu",
    // Presente no tempo presente: é a redação do contato de disputa que o motor gera
    // (`race.incident.contact_attacker`). Faltava, e a frase caía em `Desconhecido`.
    "bate",
    "colis",
    "acidente",
    "contato",
    "rodada",
    "rodou",
    "toque",
    // EN
    "crash",
    "collision",
    "accident",
    "contact",
    "spin",
];

fn contem(reason: Option<&str>, palavras: &[&str]) -> bool {
    match reason {
        Some(r) => {
            let r = r.to_lowercase();
            palavras.iter().any(|k| r.contains(k))
        }
        None => false,
    }
}

/// Classifica um abandono a partir do que o chamador tiver em mãos, na ordem de
/// confiabilidade das fontes:
///
/// 1. o **incidente** que tirou o carro da prova (o motor da simulação sabe o
///    tipo — é a fonte forte, e só o boletim a tem em memória);
/// 2. uma **peça grave quebrada** registrada no banco (só o debrief a tem);
/// 3. o **motivo textual**, por palavra-chave (o que sobra depois de salvo).
///
/// Os dois motores chamam ESTA função: mesmo com insumos diferentes, a regra é
/// uma só — antes, o boletim podia chamar de batida o mesmo DNF que o debrief
/// chamava de mecânico.
pub fn dnf_kind(inc: Option<&IncidentResult>, mech_break: bool, reason: Option<&str>) -> DnfKind {
    if let Some(i) = inc {
        // DANO LATENTE de colisão: o motor o registra como `Mechanical` (o tipo decide a
        // bandeira amarela em `estrategia::traz_bandeira_amarela`, e trocá-lo mexeria no
        // safety car), mas a natureza dele é CONTATO — o carro morreu da batida de dois
        // trechos atrás, e o texto gerado diz exatamente isso. `damage_origin_segment` é a
        // marca dessa origem e sobrevive ao save.
        //
        // Sem este ramo os dois motores divergiam no mesmo abandono: o boletim, que tem o
        // incidente em mãos, lia `Mecanico`; o debrief, que só tem o motivo salvo ("dano de
        // colisão anterior"), lia `Contato` pela palavra-chave. Era o desencontro que este
        // módulo existe para não deixar acontecer.
        if i.incident_type == IncidentType::Mechanical && i.damage_origin_segment.is_some() {
            return DnfKind::Contato;
        }
        return match i.incident_type {
            IncidentType::Collision => DnfKind::Contato,
            IncidentType::DriverError => DnfKind::Erro,
            IncidentType::Mechanical => DnfKind::Mecanico,
        };
    }
    if mech_break || contem(reason, PALAVRAS_MECANICAS) {
        return DnfKind::Mecanico;
    }
    if contem(reason, PALAVRAS_BATIDA) {
        return DnfKind::Contato;
    }
    DnfKind::Desconhecido
}

// ────────────────────────── Predicados por piloto ──────────────────────────

/// Subiu o suficiente para a corrida dele ser "uma remontada".
pub fn remontada(positions_gained: i32) -> bool {
    positions_gained >= REMONTADA_MIN
}

/// Remontada de manchete: subiu muito E terminou na frente.
pub fn remontada_epica(positions_gained: i32, finish_position: i32) -> bool {
    positions_gained >= REMONTADA_EPICA_MIN && finish_position <= REMONTADA_EPICA_MAX_FINISH
}

/// Afundou o suficiente para a corrida dele ser "um colapso".
pub fn colapso(positions_gained: i32) -> bool {
    positions_gained <= COLAPSO_MAX
}

/// A pole que não venceu e ainda afundou. `is_winner` corta o caso trivial.
pub fn pole_frustrada(is_winner: bool, is_dnf: bool, finish_position: i32) -> bool {
    !is_winner && !is_dnf && finish_position >= POLE_FRUSTRADA_MIN_FINISH
}

/// Entregou acima do que o conjunto prometia.
pub fn overperf(assessment: Option<Assessment>) -> bool {
    matches!(
        assessment,
        Some(Assessment::Acima) | Some(Assessment::MuitoAcima)
    )
}

/// Ficou aquém do que o conjunto prometia.
pub fn underperf(assessment: Option<Assessment>) -> bool {
    matches!(
        assessment,
        Some(Assessment::Abaixo) | Some(Assessment::MuitoAbaixo)
    )
}

// ────────────────────────── Predicados da corrida ──────────────────────────

/// O vencedor veio lá de trás.
pub fn vitoria_improvavel(winner_grid: i32) -> bool {
    winner_grid >= VITORIA_IMPROVAVEL_MIN_GRID
}

/// Abandonos suficientes para terem redesenhado o grid. Em grid pequeno vale o
/// piso absoluto; em grid grande, a fração.
pub fn caos(total_dnfs: i32, field_size: i32) -> bool {
    total_dnfs >= CAOS_MIN_DNFS.max(field_size / CAOS_FIELD_DIVISOR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::incidents::IncidentSeverity;

    fn inc(t: IncidentType) -> IncidentResult {
        IncidentResult {
            pilot_id: "p".into(),
            incident_type: t,
            severity: IncidentSeverity::Critical,
            segment: String::new(),
            positions_lost: 0,
            is_dnf: true,
            description: String::new(),
            linked_pilot_id: None,
            is_two_car_incident: false,
            injury_risk_multiplier: 1.0,
            narrative_importance_hint: 0,
            catalog_id: None,
            damage_origin_segment: None,
        }
    }

    /// O incidente manda: era este o desencontro entre boletim e debrief.
    #[test]
    fn incidente_tem_precedencia_sobre_o_texto() {
        let colisao = inc(IncidentType::Collision);
        // Motivo textual "falha de motor" NÃO reclassifica um contato.
        assert_eq!(
            dnf_kind(Some(&colisao), true, Some("falha de motor")),
            DnfKind::Contato
        );
        assert!(dnf_kind(Some(&colisao), false, None).is_crash());
        assert!(dnf_kind(Some(&inc(IncidentType::Mechanical)), false, None).is_mecanico());
        assert_eq!(
            dnf_kind(Some(&inc(IncidentType::DriverError)), false, None),
            DnfKind::Erro
        );
    }

    /// Sem incidente em mãos (o caso do debrief), peça quebrada > texto > nada.
    #[test]
    fn sem_incidente_cai_para_peca_e_depois_texto() {
        assert!(dnf_kind(None, true, None).is_mecanico());
        assert!(dnf_kind(None, false, Some("Quebra de câmbio")).is_mecanico());
        assert!(dnf_kind(None, false, Some("Gearbox failure")).is_mecanico());
        assert_eq!(
            dnf_kind(None, false, Some("Batida na curva 3")),
            DnfKind::Contato
        );
        assert_eq!(
            dnf_kind(None, false, Some("Abandono")),
            DnfKind::Desconhecido
        );
        assert!(!dnf_kind(None, false, None).is_crash());
    }

    /// O dano latente de colisão precisa dar a MESMA resposta pelos dois caminhos: com o
    /// incidente em mãos (boletim) e só com o texto salvo (debrief). É a divergência que o
    /// módulo existe para não deixar existir, e ela era real — o incidente vem como
    /// `Mechanical` e o texto fala em colisão.
    #[test]
    fn dano_latente_de_colisao_e_contato_pelos_dois_caminhos() {
        let mut latente = inc(IncidentType::Mechanical);
        latente.damage_origin_segment = Some("early".to_string());
        assert_eq!(dnf_kind(Some(&latente), false, None), DnfKind::Contato);
        // O mesmo abandono, visto pelo save (só o motivo textual sobrevive).
        assert_eq!(
            dnf_kind(
                None,
                false,
                Some("Fulano abandona por dano de colisão anterior")
            ),
            DnfKind::Contato
        );
        // Pane mecânica de verdade (sem origem de dano) segue mecânica.
        assert!(dnf_kind(Some(&inc(IncidentType::Mechanical)), false, None).is_mecanico());
    }

    /// As frases que o MOTOR gera (contato de disputa e fallback de dano latente) são
    /// classificadas por palavra-chave quando o incidente cru não sobrevive ao save. Elas
    /// moram no rust-i18n, então mudar a redação em qualquer um dos dois idiomas passaria a
    /// degradar `dnf_kind` para `Desconhecido` em silêncio. Este teste é o alarme.
    ///
    /// `#[serial]`: o locale é estado global do processo.
    #[test]
    #[serial_test::serial]
    fn textos_do_motor_sao_classificaveis_nos_dois_idiomas() {
        let anterior = rust_i18n::locale().to_string();
        for locale in ["pt-BR", "en-US"] {
            rust_i18n::set_locale(locale);
            let atacante = rust_i18n::t!(
                "race.incident.contact_attacker",
                atacante = "Ayrton",
                defensor = "Alain"
            )
            .to_string();
            let defensor = rust_i18n::t!(
                "race.incident.contact_defender",
                atacante = "Ayrton",
                defensor = "Alain"
            )
            .to_string();
            let dano =
                rust_i18n::t!("race.incident.latent_damage_dnf", name = "Ayrton").to_string();
            for texto in [&atacante, &defensor, &dano] {
                assert_eq!(
                    dnf_kind(None, false, Some(texto)),
                    DnfKind::Contato,
                    "[{locale}] \"{texto}\" deixou de ser lido como contato"
                );
            }
        }
        rust_i18n::set_locale(&anterior);
    }

    #[test]
    fn limiares_de_remontada_e_colapso() {
        assert!(!remontada(3));
        assert!(remontada(4));
        // Épica é conceito próprio: ganho grande NÃO basta se terminou no fundo.
        assert!(!remontada_epica(12, 9));
        assert!(remontada_epica(8, 6));
        assert!(!colapso(-3));
        assert!(colapso(-4));
    }

    #[test]
    fn caos_usa_o_maior_entre_piso_e_fracao() {
        assert!(!caos(3, 20)); // piso 5 (20/4) não batido
        assert!(caos(5, 20));
        assert!(!caos(3, 12)); // grid pequeno: vale o piso 4
        assert!(caos(4, 12));
    }

    #[test]
    fn pole_frustrada_exige_nao_vencer_terminar_e_afundar() {
        assert!(pole_frustrada(false, false, 5));
        assert!(!pole_frustrada(false, false, 4));
        assert!(!pole_frustrada(true, false, 8));
        assert!(!pole_frustrada(false, true, 8));
    }

    #[test]
    fn over_e_underperf_leem_o_assessment() {
        assert!(overperf(Some(Assessment::MuitoAcima)));
        assert!(!overperf(Some(Assessment::Dentro)));
        assert!(underperf(Some(Assessment::Abaixo)));
        assert!(!underperf(None));
    }
}
