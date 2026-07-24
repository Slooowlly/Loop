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

use serde::Serialize;

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
/// TETO do mult combinado de condições (pista × clima) por peça/volta. Sem isto, dia quente
/// e úmido em pista peaked chega a ~2× e consome ~65% da vida do motor numa corrida só —
/// esvaziando a grade. O teto poda só a CAUDA brutal: o neutro (~1.0) não muda, e a economia
/// continua respondendo às condições. Aplicado igual no risco ao vivo E na economia.
const CONDITIONS_MAX_MULT: f64 = 1.5;

/// Mult combinado de condições (pista × clima) de uma peça, já com o [`CONDITIONS_MAX_MULT`].
/// Fonte ÚNICA da poda — usado pelo risco ao vivo ([`LiveBreakdown::advance_lap_at`]) e pela
/// economia ([`conditions_wear_mults`]) pra os dois ficarem sincronizados.
fn conditions_mult(pt: PartType, track_pha: (f64, f64, f64), mean_align: f64, weather: Weather) -> f64 {
    (track_wear_mult(pt, track_pha, mean_align) * weather_wear_mult(pt, weather)).min(CONDITIONS_MAX_MULT)
}

/// Numa manutenção em box (enduro), troca a peça acima deste desgaste. Ver [`roll_race_breakdowns`].
const SERVICE_WEAR_FLOOR: f64 = 0.60;

// ───────────────────────── Enduro (corridas longas) ─────────────────────────
// Objetivo: em corrida longa o grid NÃO esvazia (DNF raro), mas a peça CUSTA muito mais —
// e o custo escala da metade pro fim da corrida. Uma parada REAL (pneu/combustível) alivia
// o gasto. Ver design: docs/superpowers/specs/2026-07-18-car-breakdown-system.md (enduro).

/// Corrida acima desta duração (min) é ENDURO — ativa a severidade abrandada, a rampa de
/// desgaste no fim e o custo/parada. Abaixo disto, sprint: tudo se comporta como antes.
pub const ENDURO_DURATION_GATE_MIN: f64 = 40.0;
/// Fração da fatia de DNF que PERMANECE DNF no enduro (o resto vira Grave = pit longo). Com
/// 0.25, o motor cai de ~38% → ~9,5% de DNF: os carros pingam no box mas cruzam a bandeira.
const ENDURO_DNF_SCALE: f64 = 0.25;
/// Desgaste EXTRA por volta no clímax do enduro (progresso→1): +80% no fim (rampa 1.8×). Começa
/// na METADE da corrida (progresso 0.5) e sobe linear — é o carro se arrastando nas últimas horas.
const ENDURO_LATE_RAMP_EXTRA: f64 = 0.80;
/// Inclinação do sobrecusto de peça do enduro por unidade de `(duração−gate)/gate`. Com 2.0,
/// uma corrida de 60 min (over=0.5) custa 2.0× o desgaste-base de peças; 80 min → 3.0×.
const ENDURO_COST_K: f64 = 2.0;
/// Alívio de gasto de peça por PARADA REAL (pneu/combustível) no enduro.
const ENDURO_PIT_RELIEF: f64 = 0.10;
/// Teto do alívio acumulado (3+ paradas → −30% do sobrecusto).
const ENDURO_RELIEF_CAP: f64 = 0.30;
/// Duração média de um stint (min) pra MODELAR as paradas da IA (o jogador usa o pit REAL).
const ENDURO_STINT_MIN: f64 = 30.0;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

// ───────────────────────── Influência da pista (qual peça sofre) ─────────────────────────

/// Força "média" do efeito da pista (~±35% nas pistas mais peaked). Calibrável.
const TRACK_STRESS_K: f64 = 1.4;

/// Alinhamento (centrado) da peça com a demanda PHA da pista: positivo = a peça puxa PARA o
/// atributo que a pista cobra (é estressada ali); negativo = puxa pra longe. Ambos os vetores
/// em frações centradas em 1/3, então pista equilibrada → ~0 para todas as peças.
fn track_alignment(pt: PartType, track_pha: (f64, f64, f64)) -> f64 {
    let dir = |(p, h, a): (f64, f64, f64)| {
        let t = p + h + a;
        if t > 0.0 {
            (p / t, h / t, a / t)
        } else {
            (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0)
        }
    };
    let (pp, ph, pa) = dir(pt.pha_per_level());
    let (dp, dh, da) = dir(track_pha);
    let t = 1.0 / 3.0;
    (pp - t) * (dp - t) + (ph - t) * (dh - t) + (pa - t) * (da - t)
}

/// Multiplicador de desgaste POR VOLTA da peça nesta pista, **centrado em ~1.0** (subtraindo
/// o `mean_align` das 11 peças) pra não inflar a taxa total do grid: peça alinhada com a
/// pista gasta mais (chega na zona de perigo antes), a contrária gasta menos. Clampado pra
/// não explodir em pistas extremas. É assim que a pista decide QUAL peça tende a quebrar.
fn track_wear_mult(pt: PartType, track_pha: (f64, f64, f64), mean_align: f64) -> f64 {
    let a = track_alignment(pt, track_pha) - mean_align;
    (1.0 + TRACK_STRESS_K * a).clamp(0.6, 1.5)
}

// ────────────────── Influência do CLIMA (chuva/calor/umidade/vento) ──────────────────

/// Condições de clima da corrida que modulam o desgaste. Faixas REAIS do iRacing: temp
/// [18,32] °C, umidade [0,100] %, vento [2,48] km/h. Fonte única = a `WeatherStory` (mesma
/// que o iRacing roda), resolvida por corrida.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weather {
    /// Molhado da pista, 0..1 (da chuva).
    pub wetness: f64,
    /// Temperatura do ar, °C (18..32).
    pub temperature: f64,
    /// Umidade relativa, 0..100 %.
    pub humidity: f64,
    /// Velocidade do vento, km/h (2..48).
    pub wind_kmh: f64,
}

impl Weather {
    /// Clima NEUTRO → todos os mults = 1.0 (temp no meio da faixa, vento típico, seco).
    pub const NEUTRAL: Weather = Weather {
        wetness: 0.0,
        temperature: TEMP_MID_C,
        humidity: 45.0,
        wind_kmh: WIND_TYPICAL_KMH,
    };
}

/// Chuva: +% de risco na ELETRÔNICA (curto/sensores). Cheio no aguaceiro (wetness=1).
const RAIN_ELEC_STRESS: f64 = 0.60;
/// Chuva ALIVIA a térmica: −% no motor/arrefecimento (o carro esfria).
const RAIN_THERMAL_RELIEF: f64 = 0.25;
/// Faixa REAL de temperatura do iRacing (°C). O modelo é CENTRADO no MEIO: dia frio alivia
/// a térmica, dia quente a agrava — a média da temporada fica ~neutra (não infla a economia).
const TEMP_MIN_C: f64 = 18.0;
const TEMP_MID_C: f64 = 25.0;
const TEMP_MAX_C: f64 = 32.0;
/// Estresse térmico no motor/arrefecimento no dia mais QUENTE (32 °C) — agora ALCANÇÁVEL.
const HEAT_THERMAL_STRESS: f64 = 0.40;
/// Alívio térmico no dia mais FRIO (18 °C).
const COOL_THERMAL_RELIEF: f64 = 0.20;
/// A umidade AMPLIFICA o estresse do calor (ar úmido refrigera pior). A 100 %, o estresse
/// térmico fica ×(1 + este fator). Só morde o lado QUENTE — dia frio úmido não vira alívio.
const HUMIDITY_THERMAL_BOOST: f64 = 0.50;
/// Vento: estressa suspensão + asas (instabilidade + carga aero flutuante). Centrado na
/// MÉDIA do vento sorteado (`generate_wind` seco ~25 km/h) pra não inflar a economia na média;
/// acima estressa, abaixo alivia de leve.
const WIND_TYPICAL_KMH: f64 = 25.0;
const WIND_MAX_KMH: f64 = 48.0;
const WIND_STRESS: f64 = 0.15;

/// Carga térmica ASSINADA da temperatura: +1 no calor máx (32°), 0 no meio (25°), −1 no
/// frio máx (18°). Multiplicada por STRESS (calor) ou RELIEF (frio).
fn thermal_load(temperature: f64) -> f64 {
    if temperature >= TEMP_MID_C {
        ((temperature - TEMP_MID_C) / (TEMP_MAX_C - TEMP_MID_C)).clamp(0.0, 1.0)
    } else {
        -((TEMP_MID_C - temperature) / (TEMP_MID_C - TEMP_MIN_C)).clamp(0.0, 1.0)
    }
}

/// Carga de vento CENTRADA no vento típico: >0 acima (estressa), <0 abaixo (alivia leve).
fn wind_load(wind_kmh: f64) -> f64 {
    ((wind_kmh - WIND_TYPICAL_KMH) / (WIND_MAX_KMH - WIND_TYPICAL_KMH)).clamp(-1.0, 1.0)
}

/// Multiplicador de desgaste por volta pelo CLIMA. Chuva: eletrônica ↑, motor/arrefecimento ↓
/// (resfria). Temperatura CENTRADA: dia quente agrava a térmica, frio alivia; a **umidade
/// amplifica o calor** (o dia quente+úmido é o matador de motor). Vento estressa suspensão +
/// asas. Centrado → não infla a economia na média; só redistribui e cria variância real.
fn weather_wear_mult(pt: PartType, weather: Weather) -> f64 {
    let w = weather.wetness.clamp(0.0, 1.0);
    let hum = (weather.humidity / 100.0).clamp(0.0, 1.0);
    let thermal = thermal_load(weather.temperature);
    // Calor (thermal ≥ 0) é amplificado pela umidade; frio (thermal < 0) é alívio puro.
    let thermal_term = if thermal >= 0.0 {
        thermal * HEAT_THERMAL_STRESS * (1.0 + hum * HUMIDITY_THERMAL_BOOST)
    } else {
        thermal * COOL_THERMAL_RELIEF
    };
    let wind = wind_load(weather.wind_kmh);
    match pt {
        PartType::Electronics => 1.0 + w * RAIN_ELEC_STRESS,
        PartType::Engine | PartType::Cooling => {
            (1.0 - w * RAIN_THERMAL_RELIEF) * (1.0 + thermal_term)
        }
        PartType::Suspension | PartType::FrontWing | PartType::RearWing => {
            1.0 + wind * WIND_STRESS
        }
        _ => 1.0,
    }
}

/// Multiplicador de desgaste POR CORRIDA de cada peça pelas condições REAIS da rodada
/// (pista × clima), a MESMA física do risco de quebra ao vivo. Alimenta a **economia**: o
/// desgaste que PERSISTE no carro passa a responder à pista (qual peça sofre) e ao clima
/// (chuva → eletrônica; calor → térmica) daquela corrida — pra **grade toda**, de modo que o
/// cérebro de manutenção reaja a corridas brutais. O estilo de pilotagem (só o jogador)
/// multiplica isto por fora. Corrida neutra (pista equilibrada, seco, ≤25 °C) → todos ~1.0,
/// e a economia calibrada não muda. `mean_align` é subtraído dentro de [`track_wear_mult`]
/// pra a pista só REDISTRIBUIR (não inflar o total); só o calor infla.
pub fn conditions_wear_mults(
    track_pha: (f64, f64, f64),
    weather: Weather,
) -> std::collections::HashMap<PartType, f64> {
    let mean_align = PartType::ALL
        .iter()
        .map(|&p| track_alignment(p, track_pha))
        .sum::<f64>()
        / PartType::ALL.len() as f64;
    PartType::ALL
        .iter()
        .map(|&pt| (pt, conditions_mult(pt, track_pha, mean_align, weather)))
        .collect()
}

// ───────────────────────── Enduro: rampa de fim + economia ─────────────────────────

/// A corrida é ENDURO pela duração (min)? Gate único usado no risco ao vivo E na economia.
pub fn is_enduro_duration(duracao_min: u8) -> bool {
    duracao_min as f64 > ENDURO_DURATION_GATE_MIN
}

/// Multiplicador de desgaste POR VOLTA pela fase da corrida no enduro: 1.0 até a metade
/// (`progress ≤ 0.5`) e sobe linear até `1 + ENDURO_LATE_RAMP_EXTRA` no fim (`progress = 1`).
/// Fora do enduro o chamador não aplica isto. É o "principalmente da metade pro fim": as peças
/// se cansam no clímax → mais reparos no box no fim da corrida longa.
fn enduro_late_ramp(progress: f64) -> f64 {
    let t = ((progress - 0.5) * 2.0).clamp(0.0, 1.0);
    1.0 + ENDURO_LATE_RAMP_EXTRA * t
}

/// Sobrecusto de desgaste de peça do enduro pela duração (acima do gate), ANTES do alívio de
/// parada. Contínuo no gate (40 min → 0). Ex.: 60 min → 1.0 (over 0.5 × K 2.0); 80 min → 3.0.
fn enduro_surcharge(duracao_min: u8) -> f64 {
    let over = ((duracao_min as f64) - ENDURO_DURATION_GATE_MIN).max(0.0) / ENDURO_DURATION_GATE_MIN;
    ENDURO_COST_K * over
}

/// Alívio de gasto de peça por PARADA REAL no enduro: 10%/parada, teto 30% (3+ paradas).
pub fn enduro_pit_relief(genuine_pits: u32) -> f64 {
    (ENDURO_PIT_RELIEF * genuine_pits as f64).min(ENDURO_RELIEF_CAP)
}

/// Paradas MODELADAS da IA pela duração (o jogador usa o pit REAL do SDK). Um stint dura
/// ~[`ENDURO_STINT_MIN`] min → uma corrida de 60 min ≈ 2 paradas. 0 fora do enduro.
pub fn modeled_ai_pits(duracao_min: u8) -> u32 {
    if !is_enduro_duration(duracao_min) {
        return 0;
    }
    ((duracao_min as f64) / ENDURO_STINT_MIN).floor().max(0.0) as u32
}

/// Multiplicador de desgaste (→ CUSTO) da corrida na ECONOMIA da grade toda: 1.0 no sprint;
/// no enduro sobe com a duração ([`enduro_surcharge`]) e é aliviado por paradas reais
/// ([`enduro_pit_relief`]). O sobrecusto persiste no desgaste → as peças chegam ao fim da vida
/// mais cedo → mais trocas ao longo da temporada de enduro (a conta que o usuário pediu). Uma
/// parada nunca leva abaixo de 1.0 (o alívio só corta o sobrecusto, não o desgaste-base).
pub fn enduro_economy_wear_mult(duracao_min: u8, genuine_pits: u32) -> f64 {
    if !is_enduro_duration(duracao_min) {
        return 1.0;
    }
    1.0 + enduro_surcharge(duracao_min) * (1.0 - enduro_pit_relief(genuine_pits))
}

// ───────────────────────── Proteção do JOGADOR (só o jogador) ─────────────────────────

/// Alívio MÁXIMO de desgaste de entrada do carro do jogador (no time mais fraco possível).
/// Botão da proteção — a IA NUNCA recebe isto. Primeiro corte: com `0.05`, num teste de
/// frota pobre, o jogador quebra ~8.6%/corrida vs ~15.8% do poor-AI (≈metade). O número
/// FINAL se calibra contra o desgaste REAL de time pobre no wiring (frota sintética é
/// hipersensível à distribuição de entrada).
const PLAYER_MAX_RELIEF: f64 = 0.05;

/// Fração de alívio no desgaste de entrada do carro do JOGADOR — a equipe dele cuida melhor
/// do carro. Escala com a FRAQUEZA do time (via `pit_crew_quality` 0-100): time forte
/// (crew alto) → ~0 (chances IDÊNTICAS à IA); time pobre (crew baixo) → alívio maior. É
/// "via manutenção", não desconto mágico no risco. Só o jogador chama isto.
fn player_wear_relief(pit_crew_quality: f64) -> f64 {
    let weakness = 1.0 - pit_crew_quality.clamp(0.0, 100.0) / 100.0;
    PLAYER_MAX_RELIEF * weakness
}

/// Cópia do carro do JOGADOR com o desgaste aliviado pela proteção (ver [`player_wear_relief`]).
/// O wiring aplica isto ANTES do pré-roll, só para o carro do jogador.
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

// ───────────────────────── Passo por-volta (ao vivo) ─────────────────────────

/// Estado do risco de quebra de UM carro, avançado **volta a volta**. É o primitivo do
/// disparo AO VIVO (o monitor chama [`LiveBreakdown::advance_lap`] com o clima REAL de cada
/// volta) E do pré-roll ([`roll_race_breakdowns`] é só um loop disto). Determinístico dado
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
        let mean_align =
            PartType::ALL.iter().map(|&pt| track_alignment(pt, track_pha)).sum::<f64>() / 11.0;
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
    pub fn advance_lap_at(&mut self, lap: u32, weather: Weather, progress: f64) -> Vec<BreakdownEvent> {
        let mut out = Vec::new();
        if self.out {
            return out;
        }
        let ramp = if self.is_enduro { enduro_late_ramp(progress) } else { 1.0 };
        for (i, &pt) in PartType::ALL.iter().enumerate() {
            if self.broken[i] {
                continue;
            }
            let noise = 1.0 + (roll(self.seed, i, lap, CH_NOISE) * 2.0 - 1.0) * WEAR_NOISE;
            let life = self.life_scale[i] * if self.is_tent { self.level_mult[i] } else { 1.0 };
            self.wear[i] += wear_per_lap(pt)
                * noise
                * conditions_mult(pt, self.track_pha, self.mean_align, weather)
                * ramp
                / life;

            let (failed, forced) = if self.wear[i] >= HARD_WALL {
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
                let severity =
                    sample_severity(pt, forced, roll(self.seed, i, lap, CH_SEVERITY), self.is_enduro);
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
pub fn roll_race_breakdowns(
    car: &Car,
    laps: u32,
    seed: u64,
    pit_crew_quality: f64,
    track_pha: (f64, f64, f64),
    weather: Weather,
    service_laps: &[u32],
) -> Vec<BreakdownEvent> {
    roll_race_breakdowns_cfg(car, laps, seed, pit_crew_quality, track_pha, weather, service_laps, false, true)
}

/// Igual a [`roll_race_breakdowns`], mas com o flag `is_enduro`: a corrida longa abranda a
/// severidade (DNF raro) e aplica a rampa de desgaste no fim (progresso = volta/total). O
/// forecast do enduro passa por aqui pra refletir o risco REAL (menos DNF, mais Grave no fim).
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
/// pré-roll [`roll_race_breakdowns`] MUITAS vezes variando a semente (Monte Carlo) e agrega, por
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
            car, laps, seed, pit_crew_quality, track_pha, weather, service_laps, is_enduro, apply_tent,
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
    parts.sort_by(|a, b| b.any_prob.partial_cmp(&a.any_prob).unwrap_or(std::cmp::Ordering::Equal));
    BreakdownForecast { parts, dnf_prob: any_dnf as f64 / denom, samples: n }
}

// ───────────────────────── Diretor do disparo ao vivo ─────────────────────────

/// Orquestra o disparo AO VIVO na grade toda: guarda um [`LiveBreakdown`] por carro (pelo
/// NÚMERO no iRacing) e, quando o monitor avisa que um carro completou uma volta, avança o
/// estado dele com o clima daquela volta e devolve os [`BreakdownEvent`] que aconteceram —
/// o monitor então dispara `ev.command(car_number)` (`!black`/`!dq`) e registra. Puro e
/// testável; a fiação viva (verde→montar a grade; volta→`on_lap`→`send_chat_text`) fica no
/// `race_monitor`. Idempotente por volta: só avança pra frente (reprocessar a mesma volta
/// não dispara de novo).
#[derive(Debug, Default)]
pub struct BreakdownDirector {
    cars: std::collections::HashMap<u32, DirectorCar>,
}

#[derive(Debug)]
struct DirectorCar {
    live: LiveBreakdown,
    /// Última volta já processada deste carro (dedupe).
    last_lap: u32,
    /// Voltas de parada no box (enduro) — troca de peças gastas.
    service_laps: Vec<u32>,
}

impl BreakdownDirector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Nenhum carro montado (grade não carregada) → o monitor não dispara nada.
    pub fn is_empty(&self) -> bool {
        self.cars.is_empty()
    }

    /// Registra um carro do grid pelo número no iRacing (o `LiveBreakdown` já vem com o
    /// desgaste de entrada + proteção do jogador aplicados). `service_laps` vazio em sprints.
    pub fn add_car(&mut self, car_number: u32, live: LiveBreakdown, service_laps: Vec<u32>) {
        self.cars.insert(
            car_number,
            DirectorCar { live, last_lap: 0, service_laps },
        );
    }

    /// Peças de `car_number` atualmente na janela de risco (ver [`LiveBreakdown::parts_in_danger`]).
    /// Vazio se o carro não está montado.
    pub fn car_parts_in_danger(&self, car_number: u32) -> Vec<(usize, PartType, f64)> {
        self.cars
            .get(&car_number)
            .map(|c| c.live.parts_in_danger())
            .unwrap_or_default()
    }

    /// Marca a volta de PARTIDA de um carro já montado (evita processar as voltas anteriores).
    /// Útil quando a grade é montada com a corrida já em andamento — a avaliação começa da
    /// PRÓXIMA volta cruzada, não retroage.
    pub fn prime_lap(&mut self, car_number: u32, lap: u32) {
        if let Some(car) = self.cars.get_mut(&car_number) {
            car.last_lap = lap;
        }
    }

    /// O carro `car_number` completou a volta `lap` (1-based). Avança o estado dele até essa
    /// volta com `weather` (o clima vivo do momento) e devolve os eventos de quebra que
    /// aconteceram. Volta ≤ já processada → nada (dedupe). Carro fora (DNF) → nada.
    pub fn on_lap(&mut self, car_number: u32, lap: u32, weather: Weather) -> Vec<BreakdownEvent> {
        self.on_lap_at(car_number, lap, weather, 0.0)
    }

    /// Igual a [`BreakdownDirector::on_lap`], com o `progress` (0..1) da corrida — o enduro usa
    /// pra a rampa de desgaste do fim. O monitor passa o progresso VIVO (tempo de sessão); as
    /// voltas em atraso reaproveitam o mesmo progresso (aproximação suave, a rampa é contínua).
    pub fn on_lap_at(
        &mut self,
        car_number: u32,
        lap: u32,
        weather: Weather,
        progress: f64,
    ) -> Vec<BreakdownEvent> {
        let mut out = Vec::new();
        if let Some(car) = self.cars.get_mut(&car_number) {
            while car.last_lap < lap && !car.live.is_out() {
                car.last_lap += 1;
                if car.service_laps.contains(&car.last_lap) {
                    car.live.service_pit();
                }
                out.extend(car.live.advance_lap_at(car.last_lap, weather, progress));
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "breakdown/tests/mod.rs"]
mod tests;
