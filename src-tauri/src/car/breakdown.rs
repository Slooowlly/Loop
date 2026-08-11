//! Cérebro do Sistema de Quebra — puro, determinístico-com-sorte, testável.
//!
//! Dado o carro (com o desgaste que cada peça carrega da economia), o nº de voltas da
//! corrida, a semente e a qualidade do pit crew da equipe, faz o **pré-roll** da corrida
//! inteira: simula o desgaste volta a volta, joga a sorte na janela de perigo (95%→105%),
//! e devolve os EVENTOS de quebra (qual peça, em que volta, severidade, e quantos segundos
//! de penalidade ou DNF). O mesmo pré-roll alimenta o disparo ao vivo (`!black`/`!dq` na
//! volta-alvo) E o aviso pré-corrida. Não toca DB, SDK nem economia — é wiring da Fase 3.
//!
//! **Cada peça** define sua chance de leve/grave/DNF (§11) E as faixas de tempo de conserto
//! de leve e de grave (câmbio demora mais que uma asa); a **qualidade do pit crew** modula
//! esse tempo (equipe boa perde menos). Params calibrados (Rota B): ver
//! `docs/superpowers/specs/2026-07-18-car-breakdown-system.md` §3, §4, §7, §11.
#![allow(dead_code)] // Fase 2: cérebro puro; o wiring ao vivo (disparo + aviso) vem na Fase 3.

use serde::{Deserialize, Serialize};

use crate::car::wear::wear_per_lap;
use crate::car::{Car, PartType};

// ───────────────────────── Parâmetros calibrados (Rota B) ─────────────────────────

// ── Curva de hazard em DOIS REGIMES (redesign 2026-07-22 §4.3) ──
// Em SERVIÇO [RISK_OPEN, OVERUSE): risco BAIXO ("azar" do carro bem-mantido/rico). SOBREUSO
// [OVERUSE, HARD_WALL): risco que sobe QUADRÁTICO (consequência do pobre que estica/degrada).
// Assim time bom raramente quebra; time que empurra a peça além de 100% quebra crescente/certo.

/// Desgaste em que o regime EM-SERVIÇO abre (abaixo disto a peça é confiável, risco 0).
const RISK_OPEN: f64 = 0.90;
/// Fim da vida NOMINAL — fronteira serviço→sobreuso. Acima daqui a peça está sendo forçada.
const OVERUSE: f64 = 1.00;
/// A PAREDE: ao atingir/passar, a peça acabou (falha forçada). Subiu (1.13→1.20) pra dar
/// corredor ao regime de sobreuso antes da morte súbita.
const HARD_WALL: f64 = 1.20;
/// Hazard/volta na BORDA de baixo do regime em-serviço (em RISK_OPEN). Baixo = o "azar" raro.
const HAZARD_SERVICE_LO: f64 = 0.006;
/// Hazard/volta no FIM da vida nominal (em OVERUSE). Ainda modesto — o carro bem-mantido que
/// roda sua corrida final passa por aqui e quase sempre sobrevive.
const HAZARD_SERVICE_HI: f64 = 0.030;
/// Hazard/volta junto à PAREDE (em HARD_WALL). Alto: a peça em sobreuso profundo morre logo.
const HAZARD_WALL: f64 = 0.45;
/// Ruído de sorte no desgaste por volta (±fração): volta "puxada" gasta mais.
const WEAR_NOISE: f64 = 0.30;
/// Botão global da taxa do grid (análogo ao `IRACER_SALARY_SHARE`).
const GLOBAL: f64 = 1.0;

// O TETO das condições e o multiplicador combinado moram em `condicoes` — ver o cabeçalho
// de lá. Ficam importados mais abaixo, junto da declaração do módulo.

/// Numa manutenção em box (enduro), troca a peça acima deste desgaste. Ver [`roll_race_breakdowns_cfg`].
const SERVICE_WEAR_FLOOR: f64 = 0.60;

// ──────────── Enduro (corridas longas): um arquivo só das regras dele ────────────
mod enduro;
// `DuracaoDeProva` e `ENDURO_DURATION_GATE_MIN` ainda não têm consumidor fora do módulo: o
// tipo existe para os call sites de `commands/` migrarem para ele (ver o doc dele), e o gate
// é referência de documentação. Ficam re-exportados porque a fronteira pública do sistema de
// quebra é esta, não o submódulo.
#[allow(unused_imports)]
pub use enduro::{
    enduro_economy_wear_mult, enduro_pit_relief, is_enduro_duration, modeled_ai_pits,
    DuracaoDeProva, ENDURO_DURATION_GATE_MIN,
};
use enduro::{enduro_late_ramp, ENDURO_DNF_SCALE};
// Os testes de enduro seguem em `breakdown/tests`, com os do resto do sistema.
#[cfg(test)]
use enduro::ENDURO_LATE_RAMP_EXTRA;

/// Canais da RNG determinística (descorrelaciona as rolagens do mesmo `(peça, volta)`).
const CH_NOISE: u64 = 1;
const CH_HAZARD: u64 = 2;
const CH_SEVERITY: u64 = 3;
const CH_TIME: u64 = 4;
const CH_PROBLEM: u64 = 5;

/// Modos de falha concretos por peça (o "qual problema"). Cada peça tem [`FAILURE_MODES`]
/// modos; a severidade (leve/grave/DNF) escolhe a frase. Ver [`problem_text`].
const FAILURE_MODES: u8 = 3;

// ───────────────────────── Tipos de saída ─────────────────────────

/// Gravidade da quebra → o comando de admin que ela vira no iRacing.
///
/// Vocabulário FECHADO, e o mesmo dos dois lados da ponte: as variantes serializam exatamente
/// como [`Severity::key`] ("light"/"heavy"/"dnf"), que é o que o React já lê e o que a coluna
/// `race_breakdowns.severity` já guarda. Era `String` comparada por literal (`== "dnf"`) na
/// borda inteira — monitor, importação, rádio e notícia —, onde um typo compilava, serializava
/// e falhava calado. Agora só o compilador escreve essas chaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Penalidade curta no box (`!black`), tempo próprio da peça.
    Light,
    /// Penalidade longa no box (`!black`), tempo próprio da peça.
    Heavy,
    /// Retirada (`!dq`) — encerra a corrida do carro.
    Dnf,
}

impl Severity {
    /// Chave estável ("light"/"heavy"/"dnf") pra persistência e frontend.
    pub fn key(self) -> &'static str {
        match self {
            Severity::Light => "light",
            Severity::Heavy => "heavy",
            Severity::Dnf => "dnf",
        }
    }

    /// Inverso de [`Severity::key`] — resolve a chave persistida ("light"/"heavy"/"dnf") de volta
    /// à severidade. `None` para chaves desconhecidas. Usado pelo feedback físico da quebra (§4.6).
    pub fn from_key(key: &str) -> Option<Severity> {
        match key {
            "light" => Some(Severity::Light),
            "heavy" => Some(Severity::Heavy),
            "dnf" => Some(Severity::Dnf),
            _ => None,
        }
    }

    /// A quebra tira o carro da corrida (`!dq`)? O predicado que a borda inteira comparava
    /// por literal.
    pub fn e_abandono(self) -> bool {
        self == Severity::Dnf
    }

    /// A quebra custou caro — abandono ou parada longa. Separa "o carro sofreu de verdade" de
    /// "perdeu rendimento", que é o corte que a notícia e o boletim usam.
    pub fn e_severa(self) -> bool {
        matches!(self, Severity::Heavy | Severity::Dnf)
    }
}

/// Um evento de quebra pré-rolado para um carro numa corrida.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BreakdownEvent {
    /// Peça culpada (nome legível via [`PartType::display_name`] na narrativa).
    pub part: PartType,
    /// Volta em que a peça larga (1-based). O disparo ao vivo acontece aqui.
    pub lap: u32,
    /// Gravidade da quebra.
    pub severity: Severity,
    /// Segundos de penalidade no box (`!black`); `None` = DNF (`!dq`).
    pub penalty_secs: Option<u32>,
    /// Desgaste da peça ao LARGAR a corrida (pra narrativa/aviso).
    pub entered_wear: f64,
    /// Desgaste no momento da falha (sempre ≥ 95%).
    pub wear_at_fail: f64,
    /// `true` se foi na parede (105%), `false` se foi por sorte na janela.
    pub forced: bool,
    /// Modo de falha concreto (0..[`FAILURE_MODES`]) — QUAL o problema na peça. Sorteado por
    /// `(seed, peça, volta)`; combinado com a severidade dá a frase (ver [`problem_text`]).
    pub problem: u8,
}

impl BreakdownEvent {
    /// A quebra tira o carro da corrida?
    pub fn is_dnf(&self) -> bool {
        self.severity == Severity::Dnf
    }

    /// O comando de admin do iRacing para este evento, dado o nº do carro na sessão.
    pub fn command(&self, car_number: u32) -> String {
        match self.penalty_secs {
            Some(secs) => format!("!black #{car_number} {secs}"),
            None => format!("!dq #{car_number}"),
        }
    }

    /// A frase do problema concreto (peça + modo + severidade) — o "o que" da quebra, pra
    /// narrativa (debrief/notícia). Ex.: "motor fundiu por superaquecimento".
    pub fn problem_label(&self) -> &'static str {
        problem_text(self.part, self.problem, self.severity)
    }
}

/// Frase do problema concreto por `(peça, modo 0..`[`FAILURE_MODES`]`, severidade)`. 3 modos por
/// peça; a severidade escolhe a intensidade (leve = perdeu rendimento, grave = parada, DNF =
/// abandono). Voz de boletim, PT. Usado pela narrativa da Peça 3.
pub fn problem_text(pt: PartType, mode: u8, sev: Severity) -> &'static str {
    let m = (mode % FAILURE_MODES) as usize;
    use Severity::{Dnf, Heavy, Light};
    match pt {
        PartType::Engine => match (m, sev) {
            (0, Light) => "superaquecimento — perdeu potência",
            (0, Heavy) => "motor fervendo — parada pra baixar temperatura",
            (0, Dnf) => "motor fundiu por superaquecimento",
            (1, Light) => "queda na pressão de óleo — segurou o ritmo",
            (1, Heavy) => "vazamento de óleo — reparo demorado",
            (1, Dnf) => "quebra de biela por falta de óleo",
            (_, Light) => "falha de ignição — engasgando",
            (_, Heavy) => "falha na injeção — parada nos boxes",
            (_, Dnf) => "pane de ignição — motor morreu",
        },
        PartType::Gearbox => match (m, sev) {
            (0, Light) => "engate falhando — perdendo marchas",
            (0, Heavy) => "câmbio travando — parada longa",
            (0, Dnf) => "câmbio quebrou",
            (1, Light) => "embreagem patinando — largadas fracas",
            (1, Heavy) => "embreagem queimada — box",
            (1, Dnf) => "embreagem destruída — sem tração",
            (_, Light) => "diferencial nervoso — traseira solta",
            (_, Heavy) => "diferencial superaquecido — parada",
            (_, Dnf) => "diferencial travou",
        },
        PartType::Brakes => match (m, sev) {
            (0, Light) => "fadiga de freio — pedal longo",
            (0, Heavy) => "superaquecimento dos freios — parada",
            (0, Dnf) => "perda total de freio",
            (1, Light) => "pastilhas no fim — frenagens fracas",
            (1, Heavy) => "disco trincado — box",
            (1, Dnf) => "disco de freio estourou",
            (_, Light) => "ar na linha — pedal esponjoso",
            (_, Heavy) => "vazamento de fluido de freio — parada",
            (_, Dnf) => "pane hidráulica de freio",
        },
        PartType::Suspension => match (m, sev) {
            (0, Light) => "bandeja folgada — carro impreciso",
            (0, Heavy) => "braço de suspensão trincado — box",
            (0, Dnf) => "braço de suspensão quebrou",
            (1, Light) => "amortecedor vazando — carro saltando",
            (1, Heavy) => "amortecedor morto — parada",
            (1, Dnf) => "colapso da suspensão",
            (_, Light) => "alinhamento fugindo — comendo pneu",
            (_, Heavy) => "rolamento de roda gritando — box",
            (_, Dnf) => "rolamento travou a roda",
        },
        PartType::FrontWing => match (m, sev) {
            (0, Light) => "asa dianteira frouxa — perdeu downforce",
            (0, Heavy) => "flap dianteiro solto — parada pra trocar",
            (0, Dnf) => "asa dianteira arrancou",
            (1, Light) => "suporte da asa trincado — subvirando",
            (1, Heavy) => "asa desalinhada — reparo nos boxes",
            (1, Dnf) => "estrutura da asa dianteira cedeu",
            (_, Light) => "endplate danificado — leve perda aero",
            (_, Heavy) => "flap preso — pit pra ajustar",
            (_, Dnf) => "asa dianteira destruída",
        },
        PartType::RearWing => match (m, sev) {
            (0, Light) => "asa traseira frouxa — traseira solta",
            (0, Heavy) => "asa traseira desregulada — box",
            (0, Dnf) => "asa traseira arrancou",
            (1, Light) => "flap teimando — perda nas retas",
            (1, Heavy) => "mecanismo da asa travado — parada",
            (1, Dnf) => "colapso da asa traseira",
            (_, Light) => "suporte da asa vibrando",
            (_, Heavy) => "montante da asa cedendo — reparo",
            (_, Dnf) => "asa traseira desprendeu",
        },
        PartType::Cooling => match (m, sev) {
            (0, Light) => "radiador entupindo — temperatura subindo",
            (0, Heavy) => "radiador furado — parada pra completar água",
            (0, Dnf) => "perda total de líquido — motor cozinhou",
            (1, Light) => "bomba d'água fraca — aquecendo",
            (1, Heavy) => "falha na bomba d'água — box",
            (1, Dnf) => "bomba d'água travou — superaquecimento fatal",
            (_, Light) => "intercooler sujo — perda de potência",
            (_, Heavy) => "ventoinha falhando — parada",
            (_, Dnf) => "colapso do arrefecimento",
        },
        PartType::Sidepods => match (m, sev) {
            (0, Light) => "sidepod amassado — leve perda aero",
            (0, Heavy) => "lateral danificada — reparo",
            (0, Dnf) => "sidepod arrancou",
            (1, Light) => "entrada de ar obstruída — temperatura subindo",
            (1, Heavy) => "duto lateral rompido — parada",
            (1, Dnf) => "lateral comprometeu a refrigeração",
            (_, Light) => "painel lateral solto — vibração",
            (_, Heavy) => "lateral desprendendo — box",
            (_, Dnf) => "colapso da lateral",
        },
        PartType::Underbody => match (m, sev) {
            (0, Light) => "assoalho raspando — perda de rendimento",
            (0, Heavy) => "assoalho danificado — parada",
            (0, Dnf) => "assoalho destruído — carro ingável",
            (1, Light) => "difusor lascado — leve perda de carga",
            (1, Heavy) => "difusor quebrado — reparo",
            (1, Dnf) => "difusor arrancou — perda total de carga",
            (_, Light) => "skid desgastado — carro baixo demais",
            (_, Heavy) => "plank comprometido — box",
            (_, Dnf) => "assoalho rompeu",
        },
        PartType::Chassis => match (m, sev) {
            (0, Light) => "trinca no chassi — carro perdeu firmeza",
            (0, Heavy) => "trinca estrutural — carro difícil de guiar",
            (0, Dnf) => "chassi rompeu",
            (1, Light) => "fixação folgada — alinhamento fugindo",
            (1, Heavy) => "suporte estrutural cedendo — muito tempo perdido",
            (1, Dnf) => "estrutura do chassi colapsou",
            (_, Light) => "vibração estrutural — carro nervoso",
            (_, Heavy) => "ressonância forte — parada de emergência",
            (_, Dnf) => "fadiga do chassi — quebra total",
        },
        PartType::Electronics => match (m, sev) {
            (0, Light) => "sensor falhando — leituras erradas",
            (0, Heavy) => "chicote com mau contato — parada",
            (0, Dnf) => "pane elétrica geral",
            (1, Light) => "bateria fraca — eletrônica instável",
            (1, Heavy) => "alternador falhando — box",
            (1, Dnf) => "morte elétrica — carro desligou",
            (_, Light) => "ECU limitando potência — modo seguro",
            (_, Heavy) => "ECU reiniciando — parada",
            (_, Dnf) => "ECU travou — carro parou",
        },
    }
}

// ───────────────────────── RNG determinística (splitmix64) ─────────────────────────

/// Finalizador splitmix64 → f64 uniforme em [0, 1). Puro, sem estado, reproduzível.
fn hash_to_unit(mut x: u64) -> f64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 11) as f64 / ((1u64 << 53) as f64)
}

/// Rolagem determinística em [0,1) para `(semente, peça, volta, canal)`.
fn roll(seed: u64, part_idx: usize, lap: u32, channel: u64) -> f64 {
    const P: u64 = 0x0000_0100_0000_01B3; // FNV-ish
    let mut x = seed;
    x = x.wrapping_mul(P).wrapping_add(part_idx as u64 + 1);
    x = x.wrapping_mul(P).wrapping_add(lap as u64 + 1);
    x = x.wrapping_mul(P).wrapping_add(channel);
    hash_to_unit(x)
}

// ───────────────────────── Modelo de risco (quando quebra) ─────────────────────────

/// Fragilidade relativa: peça de vida curta falha mais (∝ 1/durabilidade, normalizada à
/// mais durável = 0.5). Motor/câmbio/asas/freios/suspensão (vida 3) = 1.0; eletrônica = 0.5.
fn fragility(pt: PartType) -> f64 {
    (3.0 / pt.durability() as f64).clamp(0.5, 1.0)
}

/// Risco POR VOLTA de a peça quebrar (redesign 2026-07-22 §4.3), em dois regimes:
/// - `[RISK_OPEN, OVERUSE)` EM SERVIÇO: sobe LINEAR de `HAZARD_SERVICE_LO` a `HAZARD_SERVICE_HI`
///   (risco baixo — o "azar" do carro bem-mantido).
/// - `[OVERUSE, HARD_WALL)` SOBREUSO: sobe QUADRÁTICO de `HAZARD_SERVICE_HI` a `HAZARD_WALL`
///   (a consequência crescente de forçar a peça além de 100%).
/// A parede (`≥ HARD_WALL`) é falha forçada, tratada no chamador.
fn per_lap_hazard(pt: PartType, wear: f64) -> f64 {
    if wear < RISK_OPEN {
        return 0.0;
    }
    let base = if wear < OVERUSE {
        let t = (wear - RISK_OPEN) / (OVERUSE - RISK_OPEN);
        HAZARD_SERVICE_LO + (HAZARD_SERVICE_HI - HAZARD_SERVICE_LO) * t
    } else {
        let t = ((wear - OVERUSE) / (HARD_WALL - OVERUSE)).clamp(0.0, 1.0);
        HAZARD_SERVICE_HI + (HAZARD_WALL - HAZARD_SERVICE_HI) * t * t
    };
    (base * fragility(pt) * GLOBAL).clamp(0.0, 1.0)
}

// ──────────── Condições da rodada (pista × clima): um arquivo só delas ────────────
mod condicoes;
use condicoes::{conditions_mult, track_alignment};
pub use condicoes::{conditions_wear_mults, Weather};
// Os testes de condições continuam morando em `breakdown/tests`, junto dos do resto do
// sistema, e alcançam os internos daqui por este caminho.
#[cfg(test)]
use condicoes::{track_wear_mult, weather_wear_mult, WIND_TYPICAL_KMH};

// ───────────────────────── Enduro: rampa de fim + economia ─────────────────────────

// ─────────────── Proteção do JOGADOR (aposentada, viva só em teste) ───────────────
//
// **Este mecanismo foi substituído e não roda mais em produção.** Quem faz o papel dele
// hoje é `car::wear::reliability_life_mult`: a "proteção via manutenção" do jogador virou a
// confiabilidade do pit crew do time, que vale igual para a IA (variedade no grid) e entra
// na vida efetiva da peça em vez de aliviar o desgaste de entrada. O próprio doc de
// `reliability_life_mult` registra a substituição.
//
// O que sobrou aqui é só o que os testes de `breakdown/tests` ainda exercitam, e o
// `#[cfg(test)]` é o que garante isso: um caller de produção novo passa a não compilar, em
// vez de ressuscitar o mecanismo em silêncio. A calibração que o comentário antigo prometia
// ("o número final se calibra contra o desgaste real de time pobre no wiring") deixou de
// fazer sentido junto com o mecanismo — não é medição pendente, é medição cancelada.

/// Alívio MÁXIMO de desgaste de entrada do carro do jogador (no time mais fraco possível).
/// Primeiro e único corte: com `0.05`, num teste de frota pobre, o jogador quebrava
/// ~8,6%/corrida contra ~15,8% do poor-AI.
#[cfg(test)]
const PLAYER_MAX_RELIEF: f64 = 0.05;

/// Fração de alívio no desgaste de entrada do carro do JOGADOR, escalando com a FRAQUEZA do
/// time (via `pit_crew_quality` 0-100): time forte → ~0; time pobre → alívio maior.
#[cfg(test)]
fn player_wear_relief(pit_crew_quality: f64) -> f64 {
    let weakness = 1.0 - pit_crew_quality.clamp(0.0, 100.0) / 100.0;
    PLAYER_MAX_RELIEF * weakness
}

/// Cópia do carro do JOGADOR com o desgaste aliviado pela proteção (ver [`player_wear_relief`]).
/// Sem consumidor em produção — ver o bloco acima.
#[cfg(test)]
pub fn player_protected_car(car: &Car, pit_crew_quality: f64) -> Car {
    let keep = 1.0 - player_wear_relief(pit_crew_quality);
    let mut c = car.clone();
    for p in c.parts.iter_mut() {
        p.wear *= keep;
    }
    c
}

// ───────────────────────── Consequência: severidade por peça ─────────────────────────

/// Distribuição de severidade por peça: `(prob. leve, prob. grave)`; o resto é DNF.
/// **Percentuais aprovados** — estrutural/mecânica tira você da corrida; aero/eletrônica
/// quase sempre é só penalidade. Ver spec §11.
fn severity_weights(pt: PartType) -> (f64, f64) {
    // DNF das ESTRUTURAIS cortada ~pela metade (era 0.38/0.34/0.30/0.25): mesmo com a parede
    // já não promovendo Grave→DNF, o peso-base sozinho ainda tirava metade do grid. Agora uma
    // peça estrutural gasta CUSTA TEMPO na maioria das vezes, mas só TIRA o carro numa fração.
    match pt {
        PartType::Engine => (0.25, 0.52),      // 0.23 DNF
        PartType::Gearbox => (0.27, 0.53),     // 0.20
        PartType::Suspension => (0.32, 0.53),  // 0.15
        PartType::Chassis => (0.30, 0.52),     // 0.18
        PartType::Brakes => (0.45, 0.45),      // 0.10
        PartType::Cooling => (0.50, 0.42),     // 0.08
        PartType::FrontWing => (0.60, 0.35),   // 0.05
        PartType::RearWing => (0.60, 0.35),    // 0.05
        PartType::Underbody => (0.72, 0.25),   // 0.03
        PartType::Electronics => (0.72, 0.25), // 0.03
        PartType::Sidepods => (0.78, 0.20),    // 0.02
    }
}

/// Peça ESTRUTURAL: mesmo no enduro pode MATAR o carro (motor/câmbio). O resto, na parede do
/// enduro, trava em Grave (pit longo) — é o que mantém o grid cheio na corrida longa.
fn is_structural(pt: PartType) -> bool {
    matches!(pt, PartType::Engine | PartType::Gearbox)
}

/// Sorteia a severidade. Falha FORÇADA na parede (105%) sobe um degrau (a peça foi ao limite).
///
/// No **enduro** (`is_enduro`), o DNF fica RARO: a maior parte da fatia de DNF é rebaixada a
/// Grave (só [`ENDURO_DNF_SCALE`] permanece DNF), e a parede não empurra peça não-estrutural
/// pra DNF — trava em Grave. Assim a corrida longa enche o box de reparos sem esvaziar o grid;
/// só motor/câmbio ([`is_structural`]) ainda tiram o carro.
fn sample_severity(pt: PartType, forced: bool, r: f64, is_enduro: bool) -> Severity {
    let (light, heavy) = severity_weights(pt);
    let base = if r < light {
        Severity::Light
    } else if r < light + heavy {
        Severity::Heavy
    } else {
        Severity::Dnf
    };
    // Parede sobe UM degrau, mas só até Grave: `Leve→Grave`. Antes `Grave→DNF` também subia, e
    // era isso que esvaziava a grade (falha forçada de motor dava ~80% DNF). A DNF forçada agora
    // vem só da fatia de DNF NATURAL da peça (`severity_weights`), não de um bônus da parede.
    let sev = if !forced {
        base
    } else {
        match base {
            Severity::Light => Severity::Heavy,
            Severity::Heavy => Severity::Heavy,
            Severity::Dnf => Severity::Dnf,
        }
    };
    // Enduro: o DNF vira RARO como FILTRO FINAL — vale igual pra falha por sorte ou por parede.
    // Peça não-estrutural NUNCA tira o carro; estrutural (motor/câmbio) mantém só a fração
    // [`ENDURO_DNF_SCALE`], via uma rolagem decorrelacionada de `r` (determinística, sem canal novo).
    if !is_enduro || sev != Severity::Dnf {
        return sev;
    }
    if !is_structural(pt) {
        return Severity::Heavy;
    }
    let keep = hash_to_unit(r.to_bits() ^ 0xE711_D0F5_A5A5_1234);
    if keep < ENDURO_DNF_SCALE {
        Severity::Dnf
    } else {
        Severity::Heavy
    }
}

// ───────────────────────── Consequência: tempo por peça × pit crew ─────────────────────────

/// Faixa de tempo de conserto (segundos), **por peça e por severidade** — condizente com o
/// tamanho do serviço: câmbio/motor demoram; asa/eletrônica são rápidos; grave > leve. Ainda
/// SEM o efeito do pit crew (aplicado em [`repair_secs`]). Não usada para DNF.
fn repair_secs_range(pt: PartType, sev: Severity) -> (u32, u32) {
    match sev {
        Severity::Light => match pt {
            PartType::Gearbox => (6, 9),
            PartType::Engine => (6, 9),
            PartType::Chassis => (5, 8),
            PartType::Suspension => (5, 8),
            PartType::Cooling => (4, 7),
            PartType::Brakes => (3, 6),
            PartType::RearWing => (3, 5),
            PartType::FrontWing => (2, 5),
            PartType::Underbody => (2, 4),
            PartType::Sidepods => (2, 4),
            PartType::Electronics => (2, 3),
        },
        Severity::Heavy => match pt {
            PartType::Gearbox => (14, 20),
            PartType::Engine => (13, 19),
            PartType::Chassis => (11, 17),
            PartType::Suspension => (10, 15),
            PartType::Cooling => (9, 13),
            PartType::Brakes => (7, 11),
            PartType::RearWing => (6, 9),
            PartType::FrontWing => (5, 8),
            PartType::Underbody => (5, 8),
            PartType::Sidepods => (4, 6),
            PartType::Electronics => (3, 5),
        },
        Severity::Dnf => (0, 0), // não usado
    }
}

/// Fator de tempo do pit pela qualidade da equipe (0-100). Boa equipe perde menos tempo:
/// qualidade 0 → 1.20× (lenta), 50 → 1.00× (neutra), 100 → 0.80× (rápida).
fn pit_time_factor(pit_crew_quality: f64) -> f64 {
    let q = pit_crew_quality.clamp(0.0, 100.0);
    1.20 - 0.40 * (q / 100.0)
}

/// Segundos de conserto: faixa da peça/severidade (com sorte) escalada pela qualidade do pit.
fn repair_secs(pt: PartType, sev: Severity, pit_crew_quality: f64, r: f64) -> u32 {
    let (lo, hi) = repair_secs_range(pt, sev);
    let span = (hi - lo + 1) as f64;
    let raw = lo + ((r * span) as u32).min(hi - lo); // lo..=hi
    ((raw as f64) * pit_time_factor(pit_crew_quality))
        .round()
        .max(1.0) as u32
}

/// Evento de quebra GARANTIDO e NÃO-DNF — a "vitrine" da primeira corrida de um save novo: a
/// peça larga com severidade GRAVE (parada de reparo no box via `!black`), com o tempo próprio da
/// peça × pit crew. NÃO passa pela sorte da janela/parede; é determinístico por `(part, seed)`,
/// que só varia a peça/tempo/frase entre saves. Serve pra MOSTRAR o sistema logo de cara (um
/// backmarker para pra arrumar), sem tirar o carro da corrida.
pub fn showcase_repair_event(
    part: PartType,
    lap: u32,
    pit_crew_quality: f64,
    seed: u64,
) -> BreakdownEvent {
    let severity = Severity::Heavy;
    let penalty = repair_secs(
        part,
        severity,
        pit_crew_quality,
        hash_to_unit(seed ^ 0x5340_57CA_5E00_0001),
    );
    let problem = (hash_to_unit(seed ^ 0x0B0B_0B0B_0B0B_0B0B) * FAILURE_MODES as f64) as u8;
    BreakdownEvent {
        part,
        lap,
        severity,
        penalty_secs: Some(penalty),
        entered_wear: HARD_WALL,
        wear_at_fail: HARD_WALL,
        forced: true,
        problem,
    }
}

// ───────────────────────── Passo por-volta (ao vivo) ─────────────────────────

/// Estado do risco de quebra de UM carro, avançado **volta a volta**. É o primitivo do
/// disparo AO VIVO (o monitor chama [`LiveBreakdown::advance_lap`] com o clima REAL de cada
/// volta) E do pré-roll ([`roll_race_breakdowns_cfg`] é só um loop disto). Determinístico dado
/// `(wear de entrada, seed, pista)` + a sequência de climas por volta — a SORTE é a semente.
#[derive(Debug, Clone)]
pub struct LiveBreakdown {
    wear: [f64; 11],
    broken: [bool; 11],
    entered: [f64; 11],
    /// Vida INDIVIDUAL de cada peça (redesign 2026-07-22 §4.1) — divisor do desgaste por volta,
    /// pra a sim viva dessincronizar peças de mesma durabilidade igual à economia.
    life_scale: [f64; 11],
    /// Multiplicador de vida pela TENDA de nível (§4.8) por peça — aplicado só se `is_tent`.
    level_mult: [f64; 11],
    seed: u64,
    pit_crew_quality: f64,
    track_pha: (f64, f64, f64),
    mean_align: f64,
    out: bool,
    /// Corrida longa: severidade abrandada (DNF raro) + rampa de desgaste no fim. Default false.
    is_enduro: bool,
    /// Aplica a TENDA de durabilidade por nível (§4.8). Default `true`; o wiring passa `false`
    /// em categoria spec (teto ≤ 2), onde nível baixo é a norma.
    is_tent: bool,
}

impl LiveBreakdown {
    /// Estado inicial a partir do desgaste de ENTRADA do carro (já com a proteção do jogador
    /// aplicada, se for o dele). `track_pha` fixa a pista; o clima entra por volta.
    pub fn new(car: &Car, seed: u64, pit_crew_quality: f64, track_pha: (f64, f64, f64)) -> Self {
        let mut wear = [0.0; 11];
        let mut life_scale = [1.0; 11];
        let mut level_mult = [1.0; 11];
        for (i, &pt) in PartType::ALL.iter().enumerate() {
            if let Some(p) = car.part(pt) {
                wear[i] = p.wear;
                life_scale[i] = crate::car::wear::part_life_scale(p);
                level_mult[i] = crate::car::wear::level_durability_mult(p.level);
            }
        }
        // Média do alinhamento das 11 peças — subtraída em cada peça pra CENTRAR os mults em
        // ~1.0 (redistribui sem inflar a taxa total).
        let mean_align = PartType::ALL
            .iter()
            .map(|&pt| track_alignment(pt, track_pha))
            .sum::<f64>()
            / 11.0;
        Self {
            wear,
            broken: [false; 11],
            entered: wear,
            life_scale,
            level_mult,
            seed,
            pit_crew_quality,
            track_pha,
            mean_align,
            out: false,
            is_enduro: false,
            is_tent: true,
        }
    }

    /// Marca este carro como ENDURO (severidade abrandada + rampa de desgaste no fim). O wiring
    /// chama isto quando a corrida passa de [`ENDURO_DURATION_GATE_MIN`]. Encadeável.
    pub fn with_enduro(mut self, is_enduro: bool) -> Self {
        self.is_enduro = is_enduro;
        self
    }

    /// Liga/desliga a TENDA de durabilidade por nível (§4.8). O wiring passa `false` em categoria
    /// spec (teto ≤ 2). Default `true`. Encadeável.
    pub fn with_tent(mut self, is_tent: bool) -> Self {
        self.is_tent = is_tent;
        self
    }

    /// O carro já saiu da corrida (DNF)? Depois disso `advance_lap` não faz nada.
    pub fn is_out(&self) -> bool {
        self.out
    }

    /// TROCA A SORTE deste carro, sem tocar no desgaste nem no que já quebrou.
    ///
    /// Existe para o REINÍCIO de sessão: a tentativa nova devolve o carro ao desgaste de
    /// entrada, mas com a mesma semente ele quebraria a mesma peça na mesma volta, e a corrida
    /// refeita viraria a repetição exata da anterior. Com um `salt` novo por tentativa, o carro
    /// volta a ter a MESMA chance de quebrar e um desfecho próprio. `salt = 0` não mexe em nada
    /// (a primeira tentativa mantém a sorte com que o diretor foi instalado).
    pub fn reroll_luck(&mut self, salt: u64) {
        if salt == 0 {
            return;
        }
        self.seed = splitmix64(self.seed ^ splitmix64(salt));
    }

    /// Peças ATUALMENTE na janela de risco: desgaste em [`RISK_OPEN`, `HARD_WALL`), não
    /// quebradas — já têm chance de falhar a cada volta. Devolve `(índice em PartType::ALL,
    /// peça, desgaste)`. É a base do AVISO pessoal ao jogador quando uma peça cruza o limiar.
    pub fn parts_in_danger(&self) -> Vec<(usize, PartType, f64)> {
        PartType::ALL
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                !self.broken[*i] && self.wear[*i] >= RISK_OPEN && self.wear[*i] < HARD_WALL
            })
            .map(|(i, &pt)| (i, pt, self.wear[i]))
            .collect()
    }

    /// Parada no box (enduro): troca as peças acima de [`SERVICE_WEAR_FLOOR`].
    pub fn service_pit(&mut self) {
        for (i, w) in self.wear.iter_mut().enumerate() {
            if !self.broken[i] && *w >= SERVICE_WEAR_FLOOR {
                *w = 0.0;
            }
        }
    }

    /// Avança UMA volta (sprint: sem rampa de fim). Ver [`LiveBreakdown::advance_lap_at`].
    pub fn advance_lap(&mut self, lap: u32, weather: Weather) -> Vec<BreakdownEvent> {
        self.advance_lap_at(lap, weather, 0.0)
    }

    /// Avança UMA volta com o clima daquela volta e o `progress` (0..1) da corrida, devolvendo os
    /// eventos de quebra que aconteceram (0 ou 1 no caso comum; para no 1º DNF). Cada peça acumula
    /// desgaste (pista × clima × ruído × — só no enduro — rampa de fim); na janela [95%,105%] a
    /// sorte decide, na parede é forçado. `progress` só importa no enduro (ver [`enduro_late_ramp`]).
    pub fn advance_lap_at(
        &mut self,
        lap: u32,
        weather: Weather,
        progress: f64,
    ) -> Vec<BreakdownEvent> {
        self.advance_lap_at_cfg(lap, weather, progress, true)
    }

    /// Igual a [`LiveBreakdown::advance_lap_at`], mas com o interruptor da FALHA.
    ///
    /// `allow_break = false` é a volta em que a quebra **não pode existir** — a classificatória
    /// (onde uma penalidade de tempo não tem o que punir) e a carência do início da corrida. O
    /// desgaste da volta continua sendo acumulado igual: o que fica de fora é só o SORTEIO da
    /// falha (e a parede, que é a mesma falha sem sorte). A peça segue envelhecendo e volta a
    /// correr risco assim que o interruptor abre — a restrição é do evento, não do desgaste.
    ///
    /// Pular o sorteio não desloca a sorte das voltas seguintes: cada rolagem é um hash puro de
    /// `(semente, peça, volta, canal)`, não um gerador com estado.
    pub fn advance_lap_at_cfg(
        &mut self,
        lap: u32,
        weather: Weather,
        progress: f64,
        allow_break: bool,
    ) -> Vec<BreakdownEvent> {
        let mut out = Vec::new();
        if self.out {
            return out;
        }
        let ramp = if self.is_enduro {
            enduro_late_ramp(progress)
        } else {
            1.0
        };
        for (i, &pt) in PartType::ALL.iter().enumerate() {
            if self.broken[i] {
                continue;
            }
            let noise = 1.0 + (roll(self.seed, i, lap, CH_NOISE) * 2.0 - 1.0) * WEAR_NOISE;
            let life = self.life_scale[i]
                * if self.is_tent {
                    self.level_mult[i]
                } else {
                    1.0
                };
            self.wear[i] += wear_per_lap(pt)
                * noise
                * conditions_mult(pt, self.track_pha, self.mean_align, weather)
                * ramp
                / life;

            let (failed, forced) = if !allow_break {
                (false, false)
            } else if self.wear[i] >= HARD_WALL {
                (true, true)
            } else if self.wear[i] >= RISK_OPEN
                && roll(self.seed, i, lap, CH_HAZARD) < per_lap_hazard(pt, self.wear[i])
            {
                (true, false)
            } else {
                (false, false)
            };

            if failed {
                self.broken[i] = true;
                let severity = sample_severity(
                    pt,
                    forced,
                    roll(self.seed, i, lap, CH_SEVERITY),
                    self.is_enduro,
                );
                let problem = (roll(self.seed, i, lap, CH_PROBLEM) * FAILURE_MODES as f64) as u8;
                let penalty = match severity {
                    Severity::Dnf => None,
                    _ => Some(repair_secs(
                        pt,
                        severity,
                        self.pit_crew_quality,
                        roll(self.seed, i, lap, CH_TIME),
                    )),
                };
                out.push(BreakdownEvent {
                    part: pt,
                    lap,
                    severity,
                    penalty_secs: penalty,
                    entered_wear: self.entered[i],
                    wear_at_fail: self.wear[i],
                    forced,
                    problem,
                });
                self.wear[i] = 0.0; // a peça é trocada dali em diante
                if severity == Severity::Dnf {
                    self.out = true;
                    return out; // carro fora — encerra
                }
            }
        }
        out
    }
}

// ───────────────────────── Pré-roll da corrida ─────────────────────────

/// Pré-rola a corrida inteira para UM carro e devolve os eventos de quebra, em ordem de
/// volta. É só um loop do [`LiveBreakdown`] com o MESMO clima toda volta — o disparo ao vivo
/// usa o mesmo primitivo com o clima real de cada volta. Determinístico dado
/// `(car.wear, laps, seed, pit_crew_quality, service_laps)`. A SORTE é a semente.
///
/// - `track_pha`: demanda P/H/A da pista (inclina QUAL peça sofre, sem inflar o total).
/// - `weather` ([`Weather`]): chuva → eletrônica; temperatura centrada; umidade amplifica o
///   calor; vento → suspensão/asas.
/// - `service_laps`: voltas de parada no box (enduro) — vazio para sprints.
/// O flag `is_enduro`: a corrida longa abranda a severidade (DNF raro) e aplica a rampa de
/// desgaste no fim (progresso = volta/total). O forecast do enduro passa por aqui pra refletir
/// o risco REAL (menos DNF, mais Grave no fim).
pub fn roll_race_breakdowns_cfg(
    car: &Car,
    laps: u32,
    seed: u64,
    pit_crew_quality: f64,
    track_pha: (f64, f64, f64),
    weather: Weather,
    service_laps: &[u32],
    is_enduro: bool,
    apply_tent: bool,
) -> Vec<BreakdownEvent> {
    let mut state = LiveBreakdown::new(car, seed, pit_crew_quality, track_pha)
        .with_enduro(is_enduro)
        .with_tent(apply_tent);
    let mut events = Vec::new();
    let denom = laps.max(1) as f64;
    for lap in 1..=laps {
        if service_laps.contains(&lap) {
            state.service_pit();
        }
        let progress = lap as f64 / denom;
        events.extend(state.advance_lap_at(lap, weather, progress));
        if state.is_out() {
            break;
        }
    }
    events
}

// ───────────────────────── Previsão de risco (aviso pré-corrida) ─────────────────────────

/// Finalizador splitmix64 → u64 (descorrelaciona sementes vizinhas nas amostras do MC).
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Risco de uma peça numa corrida (probabilidades agregadas do MC).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PartRisk {
    pub part: PartType,
    /// Probabilidade de a peça LARGAR (qualquer severidade) nesta corrida.
    pub any_prob: f64,
    /// Probabilidade de a peça custar TEMPO DE VERDADE (penalidade pesada ou DNF) nesta corrida.
    /// Separa o desgaste trivial (`Light`, quase certo) do que realmente machuca a corrida —
    /// é a base do rótulo "custa tempo" vs "confiável" no aviso pré-corrida.
    pub costly_prob: f64,
    /// Probabilidade de a peça causar DNF nesta corrida.
    pub dnf_prob: f64,
}

/// PREVISÃO de risco de quebra de UM carro numa corrida — a base do aviso pré-corrida. Roda o
/// pré-roll [`roll_race_breakdowns_cfg`] MUITAS vezes variando a semente (Monte Carlo) e agrega, por
/// peça, a probabilidade de largar e de DNF, + o risco geral de DNF. É RISCO, não o desfecho de
/// uma semente específica: não revela QUAL peça/volta vai quebrar na corrida real (essa usa uma
/// única semente), só a chance. Determinístico dado `(car, laps, base_seed, pista, clima)`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BreakdownForecast {
    /// Risco por peça que participa (any_prob > 0), a mais arriscada primeiro.
    pub parts: Vec<PartRisk>,
    /// Risco geral de o carro ABANDONAR por quebra nesta corrida.
    pub dnf_prob: f64,
    /// Amostras do MC (transparência da precisão).
    pub samples: u32,
}

/// Roda o MC e agrega o risco. `samples` ~200-500 dá um % estável. `base_seed` fixa a previsão
/// por corrida (ex.: `event_seed(career, race)` — a mesma família da semente do disparo vivo).
pub fn forecast_breakdown_risk(
    car: &Car,
    laps: u32,
    base_seed: u64,
    pit_crew_quality: f64,
    track_pha: (f64, f64, f64),
    weather: Weather,
    service_laps: &[u32],
    samples: u32,
    is_enduro: bool,
    apply_tent: bool,
) -> BreakdownForecast {
    let mut any = [0u32; 11];
    let mut costly = [0u32; 11];
    let mut dnf = [0u32; 11];
    let mut any_dnf = 0u32;
    let n = samples.max(1);
    for i in 0..n {
        let seed = splitmix64(base_seed ^ splitmix64(i as u64));
        let evs = roll_race_breakdowns_cfg(
            car,
            laps,
            seed,
            pit_crew_quality,
            track_pha,
            weather,
            service_laps,
            is_enduro,
            apply_tent,
        );
        let mut part_failed = [false; 11];
        let mut part_costly = [false; 11];
        let mut part_dnf = [false; 11];
        let mut car_dnf = false;
        for ev in evs {
            if let Some(idx) = PartType::ALL.iter().position(|&p| p == ev.part) {
                part_failed[idx] = true;
                // "Custa tempo": penalidade PESADA ou DNF — o desgaste trivial (Light) não conta,
                // senão quase toda peça acenderia (Light é quase certo numa corrida).
                if ev.severity == Severity::Heavy || ev.is_dnf() {
                    part_costly[idx] = true;
                }
                if ev.is_dnf() {
                    part_dnf[idx] = true;
                    car_dnf = true;
                }
            }
        }
        for k in 0..11 {
            if part_failed[k] {
                any[k] += 1;
            }
            if part_costly[k] {
                costly[k] += 1;
            }
            if part_dnf[k] {
                dnf[k] += 1;
            }
        }
        if car_dnf {
            any_dnf += 1;
        }
    }
    let denom = n as f64;
    let mut parts: Vec<PartRisk> = PartType::ALL
        .iter()
        .enumerate()
        .map(|(k, &pt)| PartRisk {
            part: pt,
            any_prob: any[k] as f64 / denom,
            costly_prob: costly[k] as f64 / denom,
            dnf_prob: dnf[k] as f64 / denom,
        })
        .filter(|r| r.any_prob > 0.0)
        .collect();
    parts.sort_by(|a, b| {
        b.any_prob
            .partial_cmp(&a.any_prob)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    BreakdownForecast {
        parts,
        dnf_prob: any_dnf as f64 / denom,
        samples: n,
    }
}

// ───────────────────────── Diretor do disparo ao vivo ─────────────────────────
// O papel com estado (a grade inteira, volta a volta) mora num arquivo só dele.
mod diretor;
pub use diretor::BreakdownDirector;

#[cfg(test)]
#[path = "breakdown/tests/mod.rs"]
mod tests;

#[cfg(test)]
#[path = "breakdown/medicao.rs"]
mod medicao;
