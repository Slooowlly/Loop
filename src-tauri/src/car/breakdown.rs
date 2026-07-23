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
mod tests {
    use super::*;

    /// Qualidade neutra de pit crew para os testes que não medem esse eixo.
    const PIT_NEUTRO: f64 = 50.0;
    /// Pista equilibrada (sem influência: todos os multiplicadores = 1.0).
    const TRACK_NEUTRO: (f64, f64, f64) = (1.0, 1.0, 1.0);
    /// Pistas peaked para os testes de influência.
    const TRACK_POWER: (f64, f64, f64) = (0.70, 0.15, 0.15);
    const TRACK_HANDLING: (f64, f64, f64) = (0.15, 0.70, 0.15);
    /// Clima: neutro (25°C, seco), aguaceiro, e dia quente máx (32°C — a faixa real do iRacing).
    const WEATHER_NEUTRO: Weather = Weather::NEUTRAL;
    const WEATHER_RAIN: Weather = Weather {
        wetness: 1.0,
        temperature: 20.0,
        humidity: 90.0,
        wind_kmh: 25.0,
    };
    const WEATHER_HOT: Weather = Weather {
        wetness: 0.0,
        temperature: 32.0,
        humidity: 45.0,
        wind_kmh: 18.0,
    };

    fn car_with(part: PartType, wear: f64) -> Car {
        let mut car = Car::uniform(3); // demais peças em wear 0.0
        car.set_wear(part, wear);
        car
    }

    // -------- Primitivo ao vivo (LiveBreakdown) --------

    /// O loop manual do `LiveBreakdown` (mesmo clima toda volta) é IDÊNTICO ao pré-roll —
    /// o disparo ao vivo e o pré-roll compartilham o mesmo cérebro.
    #[test]
    fn live_step_equivale_ao_preroll() {
        for seed in 0..400u64 {
            // Frota realista: desgaste de entrada variado por peça.
            let mut car = Car::uniform(3);
            for (i, &pt) in PartType::ALL.iter().enumerate() {
                let u = roll(seed ^ 0xBEEF, i, 1, 7);
                car.set_wear(pt, 0.30 + 0.67 * u * u);
            }
            let batch =
                roll_race_breakdowns(&car, 22, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
            let mut state = LiveBreakdown::new(&car, seed, PIT_NEUTRO, TRACK_NEUTRO);
            let mut live = Vec::new();
            for lap in 1..=22 {
                live.extend(state.advance_lap(lap, WEATHER_NEUTRO));
                if state.is_out() {
                    break;
                }
            }
            assert_eq!(batch, live, "seed {seed}: live != pré-roll");
        }
    }

    // -------- Análise: taxa de quebra por corrida (diagnóstico, não regressão) --------

    /// Clima BRUTAL pro motor/arrefecimento: dia quente MÁX + úmido (amplifica a térmica) +
    /// vento forte (estressa suspensão/asas). O pior cenário realista da faixa do iRacing.
    const WEATHER_BRUTAL: Weather = Weather {
        wetness: 0.0,
        temperature: 32.0,
        humidity: 95.0,
        wind_kmh: 42.0,
    };

    /// Monta um carro com um PERFIL de desgaste de entrada que reflete o que o cérebro de
    /// manutenção deixa cada tipo de time levar pra pista (ver `car_maintenance::needs_decision`,
    /// que troca a peça quando `wear + wear_per_race >= 1.0`).
    fn perfil(kind: &str) -> Car {
        use crate::car::wear::wear_per_race;
        let mut car = Car::uniform(3);
        // Limiar em que o time DECIDE trocar (peça cruzaria 100% na próxima corrida).
        let limiar = |pt: PartType| 1.0 - wear_per_race(pt);
        for &pt in &PartType::ALL {
            let frag = pt.durability() <= 3; // motor/câmbio/freios/asas/suspensão
            let w = match kind {
                // Time rico: repõe no limiar → entra, em média, na METADE da vida útil.
                "rico_saudavel" => limiar(pt) / 2.0,
                // Time rico no PIOR momento do ciclo: logo antes de repor.
                "rico_limitrofe" => (limiar(pt) - 0.02).max(0.0),
                // Time pobre: não repôs as frágeis → esticou até o limiar; resto na metade.
                "pobre_esticando" => if frag { limiar(pt) } else { limiar(pt) / 2.0 },
                // Time quebrado: frágeis DEGRADADAS além da vida; resto no limiar.
                "pobre_degradado" => if frag { 0.98 } else { limiar(pt) - 0.02 },
                _ => 0.0,
            };
            car.set_wear(pt, w);
        }
        car
    }

    /// Roda o MC real (`roll_race_breakdowns`) e mede, POR CORRIDA:
    /// (P(≥1 quebra qualquer), P(≥1 quebra que custa tempo), P(DNF do carro)).
    fn medir(car: &Car, laps: u32, track: (f64, f64, f64), weather: Weather, samples: u32) -> (f64, f64, f64) {
        let base = 0x00C0_FFEE_u64;
        let (mut any, mut costly, mut dnf) = (0u32, 0u32, 0u32);
        for i in 0..samples {
            let seed = splitmix64(base ^ splitmix64(i as u64));
            let evs = roll_race_breakdowns(car, laps, seed, PIT_NEUTRO, track, weather, &[]);
            if !evs.is_empty() {
                any += 1;
            }
            if evs.iter().any(|e| e.severity == Severity::Heavy || e.is_dnf()) {
                costly += 1;
            }
            if evs.iter().any(|e| e.is_dnf()) {
                dnf += 1;
            }
        }
        let n = samples as f64;
        (any as f64 / n, costly as f64 / n, dnf as f64 / n)
    }

    /// "1 a cada N corridas" a partir da probabilidade por corrida (∞ se ~0).
    fn cada(p: f64) -> String {
        if p < 1e-4 {
            "     —".to_string()
        } else {
            format!("1/{:>4.1}", 1.0 / p)
        }
    }

    /// DIAGNÓSTICO (rode com: `cargo test analise_taxa_quebra -- --ignored --nocapture`).
    /// Não é regressão — só imprime a taxa esperada de quebra por corrida pra calibração.
    #[test]
    #[ignore]
    fn analise_taxa_quebra() {
        const N: u32 = 20_000;
        let perfis = ["rico_saudavel", "rico_limitrofe", "pobre_esticando", "pobre_degradado"];
        let cenarios: [(&str, (f64, f64, f64), Weather); 2] = [
            ("NEUTRO  (pista equilibrada, 25C seco)", TRACK_NEUTRO, WEATHER_NEUTRO),
            ("BRUTAL  (pista de potencia, 32C umido)", TRACK_POWER, WEATHER_BRUTAL),
        ];

        for laps in [18u32, 30] {
            println!("\n================ CORRIDA DE {laps} VOLTAS ================");
            for (nome_cen, track, weather) in cenarios {
                println!("\n  -- {nome_cen} --");
                println!(
                    "  {:<18} {:>10} {:>12} {:>10}",
                    "perfil do time", "qq quebra", "custa tempo", "DNF"
                );
                for kind in perfis {
                    let car = perfil(kind);
                    let (a, c, d) = medir(&car, laps, track, weather, N);
                    println!(
                        "  {:<18} {:>5.1}% {:>6}  {:>5.1}% {:>6}  {:>5.1}% {}",
                        kind,
                        a * 100.0,
                        cada(a),
                        c * 100.0,
                        cada(c),
                        d * 100.0,
                        cada(d),
                    );
                }
            }
        }

        // Curva: risco de UMA peça frágil (motor, durab 3) sozinha, por desgaste de entrada.
        println!("\n================ CURVA: MOTOR (durab 3) SOZINHO, 18 voltas ================");
        println!("  desgaste_entrada   qq quebra    custa tempo    DNF   (NEUTRO / BRUTAL)");
        for pct in [50, 60, 67, 75, 85, 95, 100, 103] {
            let w = pct as f64 / 100.0;
            let car = car_with(PartType::Engine, w);
            let (an, cn, dn) = medir(&car, 18, TRACK_NEUTRO, WEATHER_NEUTRO, N);
            let (ab, cb, db) = medir(&car, 18, TRACK_POWER, WEATHER_BRUTAL, N);
            println!(
                "  {:>3}%   N:{:>5.1}%/{:>5.1}%/{:>5.1}%   B:{:>5.1}%/{:>5.1}%/{:>5.1}%",
                pct,
                an * 100.0, cn * 100.0, dn * 100.0,
                ab * 100.0, cb * 100.0, db * 100.0,
            );
        }
        println!();
    }

    /// Métricas profundas de UMA corrida (por peça-que-falha), agregadas no Monte Carlo.
    struct DistQuebra {
        /// P(≥1 quebra qualquer), P(quebra que custa tempo), P(DNF do carro).
        any: f64,
        costly: f64,
        dnf: f64,
        /// Distribuição do nº de peças que falharam numa corrida: 0, 1, 2, ≥3.
        p0: f64,
        p1: f64,
        p2: f64,
        p3plus: f64,
        /// P(≥2 quebras | ≥1 quebra) — "erros SEGUIDOS na MESMA corrida".
        cond_2plus: f64,
        /// Média de peças que falham por corrida.
        mean: f64,
    }

    /// Roda o MC real e agrega a DISTRIBUIÇÃO do nº de quebras por corrida (não só "teve/não
    /// teve"). `roll_race_breakdowns` já para no 1º DNF, então o nº de eventos é o nº de peças
    /// que largaram antes (ou até) o carro sair.
    fn medir_profundo(
        car: &Car,
        laps: u32,
        track: (f64, f64, f64),
        weather: Weather,
        samples: u32,
    ) -> DistQuebra {
        let base = 0x00C0_FFEE_u64;
        let (mut any, mut costly, mut dnf) = (0u32, 0u32, 0u32);
        let (mut c0, mut c1, mut c2, mut c3) = (0u32, 0u32, 0u32, 0u32);
        let mut total_events = 0u64;
        for i in 0..samples {
            let seed = splitmix64(base ^ splitmix64(i as u64));
            let evs = roll_race_breakdowns(car, laps, seed, PIT_NEUTRO, track, weather, &[]);
            let n = evs.len();
            total_events += n as u64;
            match n {
                0 => c0 += 1,
                1 => c1 += 1,
                2 => c2 += 1,
                _ => c3 += 1,
            }
            if n >= 1 {
                any += 1;
            }
            if evs.iter().any(|e| e.severity == Severity::Heavy || e.is_dnf()) {
                costly += 1;
            }
            if evs.iter().any(|e| e.is_dnf()) {
                dnf += 1;
            }
        }
        let s = samples as f64;
        let two_plus = (c2 + c3) as f64;
        DistQuebra {
            any: any as f64 / s,
            costly: costly as f64 / s,
            dnf: dnf as f64 / s,
            p0: c0 as f64 / s,
            p1: c1 as f64 / s,
            p2: c2 as f64 / s,
            p3plus: c3 as f64 / s,
            cond_2plus: if any > 0 { two_plus / any as f64 } else { 0.0 },
            mean: total_events as f64 / s,
        }
    }

    /// DIAGNÓSTICO PROFUNDO — Perguntas 1 e 3 (rode com:
    /// `cargo test analise_profunda_quebras -- --ignored --nocapture`).
    /// (1) QUÃO FREQUENTES são as quebras por corrida, por perfil de time e cenário.
    /// (3) Com que frequência caem QUEBRAS SEGUIDAS na MESMA corrida (≥2 peças) — a coluna
    ///     `P(≥2|≥1)` é "dado que quebrou, a chance de ter sido MAIS de uma peça".
    /// A recorrência ENTRE corridas (mesma peça na próxima) está em
    /// `car_maintenance::tests::analise_recorrencia_entre_corridas`.
    #[test]
    #[ignore]
    fn analise_profunda_quebras() {
        const N: u32 = 40_000;
        let perfis = ["rico_saudavel", "rico_limitrofe", "pobre_esticando", "pobre_degradado"];
        let cenarios: [(&str, (f64, f64, f64), Weather); 2] = [
            ("NEUTRO (pista equilibrada, 25C seco)", TRACK_NEUTRO, WEATHER_NEUTRO),
            ("BRUTAL (pista de potencia, 32C umido+vento)", TRACK_POWER, WEATHER_BRUTAL),
        ];

        for laps in [18u32, 30] {
            println!("\n================ CORRIDA DE {laps} VOLTAS ================");
            for (cen, track, weather) in cenarios {
                println!("\n  -- cenário {cen} --");
                println!(
                    "  {:<18} {:>6} {:>6} {:>6}  |  {:>6} {:>6} {:>6} {:>6}  {:>9} {:>6}",
                    "perfil", "≥1qb", "custa", "DNF", "0qb", "1qb", "2qb", "≥3qb", "P(≥2|≥1)", "média"
                );
                for kind in perfis {
                    let car = perfil(kind);
                    let m = medir_profundo(&car, laps, track, weather, N);
                    println!(
                        "  {:<18} {:>5.1}% {:>5.1}% {:>5.1}%  |  {:>5.1}% {:>5.1}% {:>5.1}% {:>5.1}%  {:>8.1}% {:>6.2}",
                        kind,
                        m.any * 100.0,
                        m.costly * 100.0,
                        m.dnf * 100.0,
                        m.p0 * 100.0,
                        m.p1 * 100.0,
                        m.p2 * 100.0,
                        m.p3plus * 100.0,
                        m.cond_2plus * 100.0,
                        m.mean,
                    );
                }
            }
        }
        println!();
    }

    // -------- Diretor do disparo ao vivo --------

    #[test]
    fn diretor_dispara_peca_no_limite() {
        let mut car = Car::uniform(3);
        car.set_wear(PartType::Engine, 1.22); // além da parede (1.20) → falha forçada na volta 1
        let live = LiveBreakdown::new(&car, 42, PIT_NEUTRO, TRACK_NEUTRO);
        let mut dir = BreakdownDirector::new();
        dir.add_car(7, live, vec![]);
        let evs = dir.on_lap(7, 1, WEATHER_NEUTRO);
        assert_eq!(evs.len(), 1, "deveria disparar 1 evento na volta 1");
        assert_eq!(evs[0].part, PartType::Engine);
        let cmd = evs[0].command(7);
        assert!(cmd.starts_with("!black #7") || cmd == "!dq #7", "comando inesperado: {cmd}");
    }

    #[test]
    fn diretor_ignora_carro_fora_do_grid() {
        let mut dir = BreakdownDirector::new();
        assert!(dir.is_empty());
        assert!(dir.on_lap(99, 5, WEATHER_NEUTRO).is_empty(), "carro não montado → nada");
    }

    #[test]
    fn diretor_nao_redispara_a_mesma_volta() {
        let mut car = Car::uniform(3);
        car.set_wear(PartType::Brakes, 1.22); // além da parede (HARD_WALL 1.20) → falha forçada garantida
        let live = LiveBreakdown::new(&car, 1, PIT_NEUTRO, TRACK_NEUTRO);
        let mut dir = BreakdownDirector::new();
        dir.add_car(3, live, vec![]);
        assert!(!dir.on_lap(3, 1, WEATHER_NEUTRO).is_empty(), "1ª chamada deveria disparar");
        assert!(dir.on_lap(3, 1, WEATHER_NEUTRO).is_empty(), "reprocessar a volta 1 não redispara");
    }

    #[test]
    fn diretor_avanca_multiplas_voltas_sem_duplicar() {
        // Se o monitor "pula" pra volta 40, avança tudo de uma vez; reprocessar não faz nada.
        let car = Car::uniform(3);
        let live = LiveBreakdown::new(&car, 5, PIT_NEUTRO, TRACK_NEUTRO);
        let mut dir = BreakdownDirector::new();
        dir.add_car(1, live, vec![]);
        let _ = dir.on_lap(1, 40, WEATHER_NEUTRO);
        assert!(dir.on_lap(1, 40, WEATHER_NEUTRO).is_empty(), "reprocessar não redispara");
    }

    #[test]
    fn diretor_para_apos_dnf() {
        // Acha um seed que dá DNF numa peça na parede; confirma que o carro para de disparar.
        let seed = (0..500u64)
            .find(|&s| {
                let mut c = Car::uniform(3);
                c.set_wear(PartType::Engine, 1.22);
                let mut lb = LiveBreakdown::new(&c, s, PIT_NEUTRO, TRACK_NEUTRO);
                lb.advance_lap(1, WEATHER_NEUTRO)
                    .iter()
                    .any(|e| e.severity == Severity::Dnf)
            })
            .expect("algum seed deveria dar DNF no motor na parede");
        let mut car = Car::uniform(3);
        car.set_wear(PartType::Engine, 1.22);
        let live = LiveBreakdown::new(&car, seed, PIT_NEUTRO, TRACK_NEUTRO);
        let mut dir = BreakdownDirector::new();
        dir.add_car(2, live, vec![]);
        let first = dir.on_lap(2, 1, WEATHER_NEUTRO);
        assert!(first.iter().any(|e| e.severity == Severity::Dnf), "volta 1 deveria dar DNF");
        assert!(dir.on_lap(2, 30, WEATHER_NEUTRO).is_empty(), "após DNF o carro não dispara mais");
    }

    // -------- Confiabilidade da peça sadia --------

    #[test]
    fn peca_sadia_nunca_quebra_num_sprint() {
        let car = Car::uniform(5);
        for seed in 0..1000 {
            let ev = roll_race_breakdowns(&car, 20, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
            assert!(ev.is_empty(), "carro novo não deveria quebrar em 20 voltas (seed {seed})");
        }
    }

    #[test]
    fn nenhuma_quebra_abaixo_de_95_por_cento() {
        for w in [0.0, 0.5, 0.80, 0.90, 0.94] {
            let car = car_with(PartType::Engine, w);
            for seed in 0..200 {
                for ev in roll_race_breakdowns(&car, 18, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]) {
                    assert!(
                        ev.wear_at_fail >= RISK_OPEN,
                        "quebra a {:.3} < 95% (peça entrou {w})",
                        ev.wear_at_fail
                    );
                }
            }
        }
    }

    // -------- Peça no limite quebra por sorte --------

    #[test]
    fn peca_no_limite_quebra_com_frequencia() {
        let car = car_with(PartType::Engine, 0.97);
        let quebrou = (0..1000)
            .filter(|&s| !roll_race_breakdowns(&car, 18, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]).is_empty())
            .count();
        assert!(quebrou > 700, "esperado muitas quebras, deu {quebrou}/1000");
    }

    #[test]
    fn a_culpada_e_a_peca_no_limite() {
        let car = car_with(PartType::Gearbox, 0.98);
        for seed in 0..300 {
            for ev in roll_race_breakdowns(&car, 18, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]) {
                assert_eq!(ev.part, PartType::Gearbox, "só o câmbio deveria quebrar aqui");
            }
        }
    }

    // -------- Determinismo e sorte --------

    #[test]
    fn e_deterministico() {
        let car = car_with(PartType::Engine, 0.96);
        for seed in [0u64, 7, 42, 1000, 999_999] {
            let a = roll_race_breakdowns(&car, 22, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
            let b = roll_race_breakdowns(&car, 22, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
            assert_eq!(a, b, "mesmos inputs deveriam dar o mesmo resultado (seed {seed})");
        }
    }

    #[test]
    fn a_sorte_varia_o_desfecho() {
        let car = car_with(PartType::Engine, 0.96);
        let mut voltas = std::collections::HashSet::new();
        for seed in 0..200 {
            if let Some(ev) = roll_race_breakdowns(&car, 22, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]).first() {
                voltas.insert(ev.lap);
            }
        }
        assert!(voltas.len() > 3, "a volta da quebra deveria variar com a sorte, deu {voltas:?}");
    }

    // -------- Parede vs sorte --------

    #[test]
    fn parede_forca_a_falha() {
        let car = car_with(PartType::Engine, 1.04);
        for seed in 0..300 {
            let ev = roll_race_breakdowns(&car, 18, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
            assert!(!ev.is_empty(), "peça a 104% deveria quebrar (seed {seed})");
        }
    }

    // -------- DNF encerra a corrida --------

    #[test]
    fn dnf_e_o_ultimo_e_unico() {
        let car = car_with(PartType::Engine, 1.04);
        for seed in 0..500 {
            let ev = roll_race_breakdowns(&car, 30, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]);
            let dnfs = ev.iter().filter(|e| e.is_dnf()).count();
            assert!(dnfs <= 1, "no máximo 1 DNF por corrida");
            if let Some(pos) = ev.iter().position(|e| e.is_dnf()) {
                assert_eq!(pos, ev.len() - 1, "DNF deveria ser o último evento");
            }
            // DNF não tem tempo de penalidade; leve/grave têm.
            for e in &ev {
                assert_eq!(e.is_dnf(), e.penalty_secs.is_none());
            }
        }
    }

    // -------- Severidade por peça --------

    #[test]
    fn motor_termina_em_dnf_mais_que_eletronica() {
        let dnf = |pt| {
            let (l, h) = severity_weights(pt);
            1.0 - l - h
        };
        assert!(
            dnf(PartType::Engine) > dnf(PartType::Electronics) + 0.15,
            "motor deveria dar muito mais DNF que eletrônica"
        );
    }

    #[test]
    fn parede_agrava_a_severidade() {
        let r = 0.05; // dentro da fatia "leve" do motor (light=0.20)
        assert_eq!(sample_severity(PartType::Engine, false, r, false), Severity::Light);
        assert_eq!(sample_severity(PartType::Engine, true, r, false), Severity::Heavy);
    }

    // -------- Tempo de conserto CONDIZENTE com a peça e a severidade --------

    #[test]
    fn grave_demora_mais_que_leve_na_mesma_peca() {
        let leve = repair_secs(PartType::Gearbox, Severity::Light, PIT_NEUTRO, 0.5);
        let grave = repair_secs(PartType::Gearbox, Severity::Heavy, PIT_NEUTRO, 0.5);
        assert!(grave > leve, "câmbio grave ({grave}s) deveria demorar mais que leve ({leve}s)");
    }

    #[test]
    fn peca_grande_demora_mais_que_pequena() {
        let g = |pt| repair_secs(pt, Severity::Heavy, PIT_NEUTRO, 0.5);
        assert!(g(PartType::Gearbox) > g(PartType::Suspension));
        assert!(g(PartType::Suspension) > g(PartType::Brakes));
        assert!(g(PartType::Brakes) > g(PartType::FrontWing));
        assert!(g(PartType::FrontWing) >= g(PartType::Electronics));
    }

    #[test]
    fn cambio_leve_e_grave_nos_intervalos_esperados() {
        // Pit neutro (fator 1.0) → tempo cru dentro da faixa da tabela.
        for r in [0.0, 0.3, 0.6, 0.99] {
            let leve = repair_secs(PartType::Gearbox, Severity::Light, PIT_NEUTRO, r);
            assert!((6..=9).contains(&leve), "câmbio leve fora de 6-9: {leve}");
            let grave = repair_secs(PartType::Gearbox, Severity::Heavy, PIT_NEUTRO, r);
            assert!((14..=20).contains(&grave), "câmbio grave fora de 14-20: {grave}");
        }
    }

    // -------- Variância pela qualidade do pit crew --------

    #[test]
    fn pit_crew_melhor_conserta_mais_rapido() {
        let ruim = repair_secs(PartType::Gearbox, Severity::Heavy, 20.0, 0.5);
        let bom = repair_secs(PartType::Gearbox, Severity::Heavy, 95.0, 0.5);
        assert!(bom < ruim, "pit bom ({bom}s) deveria ser mais rápido que ruim ({ruim}s)");
    }

    #[test]
    fn fator_de_pit_neutro_em_50() {
        assert!((pit_time_factor(50.0) - 1.0).abs() < 1e-9);
        assert!(pit_time_factor(0.0) > 1.0);
        assert!(pit_time_factor(100.0) < 1.0);
    }

    // -------- Comandos --------

    #[test]
    fn comando_black_para_penalidade_dq_para_dnf() {
        let pen = BreakdownEvent {
            part: PartType::Brakes,
            lap: 5,
            severity: Severity::Heavy,
            penalty_secs: Some(9),
            entered_wear: 0.96,
            wear_at_fail: 0.99,
            forced: false,
            problem: 0,
        };
        assert_eq!(pen.command(7), "!black #7 9");
        assert!(!pen.is_dnf());
        let dnf = BreakdownEvent { severity: Severity::Dnf, penalty_secs: None, ..pen };
        assert_eq!(dnf.command(7), "!dq #7");
        assert!(dnf.is_dnf());
    }

    #[test]
    fn catalogo_de_problemas_cobre_todas_as_pecas_e_severidades() {
        // Toda combinação (peça × modo × severidade) devolve uma frase não-vazia e o modo
        // além do range faz wrap (nunca entra no braço impossível de panic).
        for &pt in &PartType::ALL {
            for mode in 0..(FAILURE_MODES + 2) {
                for sev in [Severity::Light, Severity::Heavy, Severity::Dnf] {
                    assert!(!problem_text(pt, mode, sev).is_empty());
                }
            }
        }
        // O rótulo do evento combina peça + modo + severidade (DNF do motor cita "fundiu").
        let ev = BreakdownEvent {
            part: PartType::Engine,
            lap: 3,
            severity: Severity::Dnf,
            penalty_secs: None,
            entered_wear: 0.98,
            wear_at_fail: 1.06,
            forced: true,
            problem: 0,
        };
        assert_eq!(ev.problem_label(), "motor fundiu por superaquecimento");
    }

    #[test]
    fn previsao_de_risco_reflete_o_desgaste_de_entrada() {
        // Carro novo (desgaste baixo) → risco ~zero num sprint.
        let sadio = Car::uniform(3);
        let f0 = forecast_breakdown_risk(&sadio, 18, 42, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], 300, false, true);
        assert!(f0.dnf_prob < 0.02, "carro novo não deveria ter risco de DNF: {}", f0.dnf_prob);

        // Motor entrando na zona de perigo → motor vira a peça de MAIOR risco e o risco sobe.
        let mut gasto = Car::uniform(3);
        gasto.set_wear(PartType::Engine, 0.98);
        let f1 = forecast_breakdown_risk(&gasto, 18, 42, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], 300, false, true);
        assert!(f1.dnf_prob > f0.dnf_prob, "carro gasto deveria ter mais risco");
        assert!(!f1.parts.is_empty());
        assert_eq!(f1.parts[0].part, PartType::Engine, "o motor no fio deveria liderar o risco");
        assert!(f1.parts[0].any_prob > 0.3, "motor a 98% deveria largar com frequência: {}", f1.parts[0].any_prob);

        // Determinístico: mesma entrada → mesma previsão.
        let f2 = forecast_breakdown_risk(&gasto, 18, 42, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], 300, false, true);
        assert_eq!(f1, f2);
    }

    // -------- Influência da pista (qual peça sofre) --------

    #[test]
    fn pista_neutra_nao_influencia() {
        // Pista equilibrada → multiplicador 1.0 para toda peça.
        let mean = PartType::ALL
            .iter()
            .map(|&pt| track_alignment(pt, TRACK_NEUTRO))
            .sum::<f64>()
            / 11.0;
        for &pt in &PartType::ALL {
            assert!((track_wear_mult(pt, TRACK_NEUTRO, mean) - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn pista_de_potencia_estressa_o_motor_nao_os_freios() {
        let mean = PartType::ALL
            .iter()
            .map(|&pt| track_alignment(pt, TRACK_POWER))
            .sum::<f64>()
            / 11.0;
        let motor = track_wear_mult(PartType::Engine, TRACK_POWER, mean);
        let freios = track_wear_mult(PartType::Brakes, TRACK_POWER, mean);
        assert!(motor > 1.0, "motor deveria sofrer mais na pista de potência ({motor})");
        assert!(freios < 1.0, "freios deveriam sofrer menos ({freios})");
        assert!(motor > freios);
    }

    #[test]
    fn pista_de_handling_estressa_os_freios_nao_o_motor() {
        let mean = PartType::ALL
            .iter()
            .map(|&pt| track_alignment(pt, TRACK_HANDLING))
            .sum::<f64>()
            / 11.0;
        let motor = track_wear_mult(PartType::Engine, TRACK_HANDLING, mean);
        let freios = track_wear_mult(PartType::Brakes, TRACK_HANDLING, mean);
        assert!(freios > motor, "na pista técnica os freios ({freios}) deveriam sofrer mais que o motor ({motor})");
    }

    /// Roda uma frota REALISTA (desgaste de entrada variado por peça: maioria baixo, poucas
    /// perto da zona) e conta as quebras por peça.
    fn fleet_breaks(track: (f64, f64, f64), weather: Weather) -> ([u32; 11], u32) {
        let mut by_part = [0u32; 11];
        let mut total = 0;
        for seed in 0..8000u64 {
            let mut car = Car::uniform(3);
            for (i, &pt) in PartType::ALL.iter().enumerate() {
                // Desgaste de entrada determinístico e enviesado pra baixo (cauda até ~0.97).
                let u = roll(seed ^ 0xA5A5_5A5A, i, 1, 99);
                car.set_wear(pt, 0.30 + 0.67 * u * u);
            }
            let ev = roll_race_breakdowns(&car, 20, seed, PIT_NEUTRO, track, weather, &[]);
            if !ev.is_empty() {
                total += 1;
            }
            for e in &ev {
                let i = PartType::ALL.iter().position(|&x| x == e.part).unwrap();
                by_part[i] += 1;
            }
        }
        (by_part, total)
    }

    #[test]
    fn taxa_total_quase_invariante_e_distribuicao_inclina_por_pista() {
        let (neutro, tn) = fleet_breaks(TRACK_NEUTRO, WEATHER_NEUTRO);
        let (power, tp) = fleet_breaks(TRACK_POWER, WEATHER_NEUTRO);
        let (handling, th) = fleet_breaks(TRACK_HANDLING, WEATHER_NEUTRO);

        // Relatório (visível com --nocapture).
        let idx = |pt: PartType| PartType::ALL.iter().position(|&x| x == pt).unwrap();
        println!("\n── Influência da pista (frota com desgaste realista) ──");
        println!("Taxa total (carros com quebra): neutro {tn} · power {tp} · handling {th}");
        println!(
            "{:<12} {:>8} {:>8} {:>8}",
            "peça", "neutro", "power", "handling"
        );
        for &pt in &PartType::ALL {
            let i = idx(pt);
            println!("{:<12} {:>8} {:>8} {:>8}", pt.as_str(), neutro[i], power[i], handling[i]);
        }

        // (a) A taxa total quase não muda com o tipo de pista (redistribui, não infla).
        let max = tn.max(tp).max(th) as f64;
        let min = tn.min(tp).min(th) as f64;
        assert!((max - min) / max < 0.15, "taxa total variou demais: {tn}/{tp}/{th}");

        // (b) O motor sofre mais na pista de potência; os freios, na técnica.
        assert!(power[idx(PartType::Engine)] > handling[idx(PartType::Engine)],
            "motor deveria quebrar mais na pista de potência");
        assert!(handling[idx(PartType::Brakes)] > power[idx(PartType::Brakes)],
            "freios deveriam quebrar mais na pista técnica");
    }

    // -------- Influência do clima (chuva/calor) --------

    /// Clima só com wetness+temp movidos (umidade/vento neutros) — pra isolar um eixo.
    fn wx(wetness: f64, temperature: f64) -> Weather {
        Weather { wetness, temperature, humidity: 45.0, wind_kmh: WIND_TYPICAL_KMH }
    }

    #[test]
    fn clima_neutro_nao_influencia() {
        for &pt in &PartType::ALL {
            assert!((weather_wear_mult(pt, WEATHER_NEUTRO) - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn chuva_estressa_eletronica_e_alivia_a_termica() {
        let elec = weather_wear_mult(PartType::Electronics, wx(1.0, 20.0));
        let motor = weather_wear_mult(PartType::Engine, wx(1.0, 20.0));
        let cooling = weather_wear_mult(PartType::Cooling, wx(1.0, 20.0));
        assert!(elec > 1.0, "chuva deveria estressar a eletrônica ({elec})");
        assert!(motor < 1.0 && cooling < 1.0, "chuva deveria aliviar motor/arrefecimento");
        // Peça sem relação com chuva/vento não muda.
        assert!((weather_wear_mult(PartType::Brakes, wx(1.0, 20.0)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn calor_agrava_e_frio_alivia_o_motor() {
        // Modelo CENTRADO em 25°C: 32° (máx real) agrava, 18° (mín real) alivia.
        let motor_neutro = weather_wear_mult(PartType::Engine, wx(0.0, 25.0));
        let motor_quente = weather_wear_mult(PartType::Engine, wx(0.0, 32.0));
        let motor_frio = weather_wear_mult(PartType::Engine, wx(0.0, 18.0));
        assert!((motor_neutro - 1.0).abs() < 1e-9, "25° deveria ser neutro");
        assert!(motor_quente > 1.0, "dia quente (32°) deveria agravar o motor ({motor_quente})");
        assert!(motor_frio < 1.0, "dia frio (18°) deveria aliviar o motor ({motor_frio})");
        assert!((weather_wear_mult(PartType::Brakes, wx(0.0, 32.0)) - 1.0).abs() < 1e-9, "calor não mexe nos freios");
    }

    #[test]
    fn umidade_amplifica_o_calor() {
        let seco = Weather { wetness: 0.0, temperature: 32.0, humidity: 10.0, wind_kmh: WIND_TYPICAL_KMH };
        let umido = Weather { wetness: 0.0, temperature: 32.0, humidity: 95.0, wind_kmh: WIND_TYPICAL_KMH };
        let motor_seco = weather_wear_mult(PartType::Engine, seco);
        let motor_umido = weather_wear_mult(PartType::Engine, umido);
        assert!(motor_umido > motor_seco, "dia quente ÚMIDO deveria castigar mais o motor ({motor_umido} vs {motor_seco})");
        // Em dia FRIO a umidade não vira alívio extra (só amplifica o lado quente).
        let frio_seco = weather_wear_mult(PartType::Engine, Weather { wetness: 0.0, temperature: 18.0, humidity: 10.0, wind_kmh: WIND_TYPICAL_KMH });
        let frio_umido = weather_wear_mult(PartType::Engine, Weather { wetness: 0.0, temperature: 18.0, humidity: 95.0, wind_kmh: WIND_TYPICAL_KMH });
        assert!((frio_seco - frio_umido).abs() < 1e-9, "umidade não deveria mexer no dia frio");
    }

    #[test]
    fn vento_estressa_suspensao_e_asas() {
        let calmo = Weather { wetness: 0.0, temperature: 25.0, humidity: 45.0, wind_kmh: 2.0 };
        let vendaval = Weather { wetness: 0.0, temperature: 25.0, humidity: 45.0, wind_kmh: 48.0 };
        for pt in [PartType::Suspension, PartType::FrontWing, PartType::RearWing] {
            assert!(weather_wear_mult(pt, vendaval) > weather_wear_mult(pt, calmo),
                "{pt:?} deveria sofrer mais no vendaval");
        }
        // Motor não sente o vento.
        assert!((weather_wear_mult(PartType::Engine, vendaval) - weather_wear_mult(PartType::Engine, calmo)).abs() < 1e-9);
    }

    #[test]
    fn clima_redistribui_e_dia_quente_agrava_frio_alivia() {
        let idx = |pt: PartType| PartType::ALL.iter().position(|&x| x == pt).unwrap();
        const WEATHER_COOL: Weather = Weather { wetness: 0.0, temperature: 18.0, humidity: 45.0, wind_kmh: WIND_TYPICAL_KMH };
        let (seco, ts) = fleet_breaks(TRACK_NEUTRO, WEATHER_NEUTRO);
        let (chuva, _tc) = fleet_breaks(TRACK_NEUTRO, WEATHER_RAIN);
        let (forno, tf) = fleet_breaks(TRACK_NEUTRO, WEATHER_HOT);
        let (frio, tfr) = fleet_breaks(TRACK_NEUTRO, WEATHER_COOL);

        println!("\n── Influência do clima (frota realista) ──");
        println!("Taxa total: neutro {ts} · quente {tf} · frio {tfr}");

        // Chuva: eletrônica quebra mais; motor menos.
        assert!(chuva[idx(PartType::Electronics)] > seco[idx(PartType::Electronics)],
            "chuva deveria quebrar mais eletrônica");
        assert!(chuva[idx(PartType::Engine)] < seco[idx(PartType::Engine)],
            "chuva deveria quebrar menos motor");
        // Dia quente (32°) agrava o motor E sobe o total; dia frio (18°) alivia.
        assert!(forno[idx(PartType::Engine)] > seco[idx(PartType::Engine)],
            "dia quente deveria quebrar mais motor");
        assert!(tf > ts, "dia quente deveria subir o total: {tf} vs {ts}");
        assert!(tfr < ts, "dia frio deveria aliviar o total: {tfr} vs {ts}");
    }

    // -------- Mults por corrida que alimentam a ECONOMIA (persistência) --------

    /// Corrida neutra (pista equilibrada, clima neutro) → todos os mults = 1.0: a economia
    /// calibrada NÃO muda ao introduzir a modulação por condições.
    #[test]
    fn conditions_mults_neutro_sao_um() {
        let neutro = conditions_wear_mults((1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0), Weather::NEUTRAL);
        for &pt in &PartType::ALL {
            assert!(
                (neutro[&pt] - 1.0).abs() < 1e-9,
                "{pt:?} neutro deveria ser 1.0, deu {}",
                neutro[&pt]
            );
        }
    }

    /// O mult combinado leva o clima corretamente por peça: calor agrava motor/arrefecimento;
    /// chuva estressa a eletrônica e alivia o motor.
    #[test]
    fn conditions_mults_levam_o_clima_por_peca() {
        let bal = (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0);
        let quente = conditions_wear_mults(bal, wx(0.0, 32.0));
        assert!(quente[&PartType::Engine] > 1.0, "calor deveria agravar o motor");
        assert!(quente[&PartType::Cooling] > 1.0, "calor deveria agravar o arrefecimento");
        let chuva = conditions_wear_mults(bal, wx(1.0, 20.0));
        assert!(chuva[&PartType::Electronics] > 1.0, "chuva deveria estressar a eletrônica");
        assert!(chuva[&PartType::Engine] < 1.0, "chuva deveria aliviar o motor");
        assert!((chuva[&PartType::Brakes] - 1.0).abs() < 1e-9, "clima não mexe nos freios");
    }

    // -------- Proteção do jogador (só o jogador, via manutenção) --------

    #[test]
    fn time_forte_nao_da_protecao_ao_jogador() {
        // Crew no topo (100) → alívio 0 → carro idêntico → chances iguais à IA.
        let mut car = Car::uniform(4);
        car.set_wear(PartType::Engine, 0.90);
        let protegido = player_protected_car(&car, 100.0);
        assert_eq!(protegido, car, "time forte não deveria mudar o carro do jogador");
    }

    #[test]
    fn protecao_reduz_quebras_do_jogador_em_time_fraco() {
        // Frota pobre (desgaste alto). Sem proteção (crew 100) vs protegido (crew fraco 45).
        // Frota pobre NÃO-saturada (taxa moderada) pra o efeito da proteção aparecer.
        let n = 8000u64;
        let total = |crew: f64| -> usize {
            let mut c = 0;
            for seed in 0..n {
                let mut raw = Car::uniform(3);
                for (i, &pt) in PartType::ALL.iter().enumerate() {
                    let u = roll(seed ^ 0xBEEF, i, 1, 7);
                    raw.set_wear(pt, 0.20 + 0.48 * u); // 0.20–0.68 (poucas peças perto da zona)
                }
                let car = player_protected_car(&raw, crew);
                if !roll_race_breakdowns(&car, 18, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[])
                    .is_empty()
                {
                    c += 1;
                }
            }
            c
        };
        let sem = total(100.0); // crew máximo = sem proteção (poor-AI)
        let protegido = total(45.0); // time pobre do jogador = protegido
        let pct = |x: usize| x as f64 / n as f64 * 100.0;
        println!(
            "\n── Proteção do jogador ── quebra/corrida: poor-AI {:.1}% · jogador-protegido {:.1}% (razão {:.2})",
            pct(sem), pct(protegido), protegido as f64 / sem as f64
        );
        // Banda sã: protege de verdade, mas sem tornar o jogador quase-imune (a frota
        // sintética é arbitrária; o valor fino se calibra no wiring com desgaste real). Com a
        // janela de risco alargada (RISK_OPEN 0.87), o mesmo alívio de 5% cobre proporção menor
        // da zona → a redução relativa cai de ~20% pra ~19%; a banda acompanha.
        assert!(protegido < sem * 85 / 100, "proteção fraca demais: {protegido} vs {sem}");
        assert!(protegido > sem / 5, "proteção forte demais (quase imune): {protegido} vs {sem}");
    }

    // -------- Manutenção em box do enduro (gap 2) --------

    #[test]
    fn parada_de_box_reduz_quebras_no_enduro() {
        let car = car_with(PartType::Engine, 0.5);
        let sem: usize = (0..500)
            .filter(|&s| !roll_race_breakdowns(&car, 50, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[]).is_empty())
            .count();
        let com: usize = (0..500)
            .filter(|&s| !roll_race_breakdowns(&car, 50, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[16, 32]).is_empty())
            .count();
        assert!(com < sem, "a parada de box deveria reduzir quebras (sem={sem}, com={com})");
    }

    // ========== ENDURO: DNF raro + rampa de fim + economia ==========

    /// Eixo 1: no enduro, a MAIORIA dos DNFs de peça não-estrutural vira Grave. Numa parede
    /// (peça a 1.06) o motor ainda pode morrer, mas a asa/eletrônica travam em Grave.
    #[test]
    fn enduro_rebaixa_a_maioria_dos_dnfs_a_grave() {
        // Conta DNFs numa frota de motores no fio, sprint vs enduro.
        let dnf_rate = |is_enduro: bool| -> usize {
            (0..2000u64)
                .filter(|&s| {
                    let car = car_with(PartType::Engine, 0.98);
                    roll_race_breakdowns_cfg(&car, 40, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], is_enduro, true)
                        .iter()
                        .any(|e| e.is_dnf())
                })
                .count()
        };
        let sprint = dnf_rate(false);
        let enduro = dnf_rate(true);
        assert!(
            enduro * 2 < sprint,
            "enduro deveria dar bem menos DNF de motor (sprint={sprint}, enduro={enduro})"
        );
    }

    /// Peça não-estrutural (asa) NUNCA tira o carro no enduro, nem na parede.
    #[test]
    fn enduro_peca_nao_estrutural_nunca_da_dnf() {
        for seed in 0..1000u64 {
            let car = car_with(PartType::FrontWing, 1.08); // dentro da janela → a asa vai quebrar
            for ev in roll_race_breakdowns_cfg(&car, 40, seed, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], true, true) {
                // O invariante é sobre a ASA (não-estrutural). Numa corrida de 40 voltas, peças
                // frágeis frescas (motor/câmbio) também entram na zona e, por serem estruturais,
                // PODEM dar DNF no enduro — então filtramos só os eventos da asa.
                if ev.part == PartType::FrontWing {
                    assert!(!ev.is_dnf(), "asa não deveria dar DNF no enduro (seed {seed})");
                }
            }
        }
    }

    /// Peça ESTRUTURAL (motor) ainda pode dar DNF no enduro (só fica mais raro).
    #[test]
    fn enduro_estrutural_ainda_pode_dar_dnf() {
        let algum_dnf = (0..1000u64).any(|s| {
            let car = car_with(PartType::Engine, 1.06);
            roll_race_breakdowns_cfg(&car, 40, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], true, true)
                .iter()
                .any(|e| e.is_dnf())
        });
        assert!(algum_dnf, "motor deveria conseguir dar DNF mesmo no enduro");
    }

    /// Estrutural no enduro mantém só ~[`ENDURO_DNF_SCALE`] da fatia de DNF; não-estrutural
    /// nunca vira DNF. O filtro é determinístico e vale igual pra sorte ou parede.
    #[test]
    fn enduro_estrutural_mantem_so_a_fracao_de_dnf() {
        // Motor (estrutural), todos os `r` na fatia de DNF, sem parede: ~25% permanecem DNF.
        let (light, heavy) = severity_weights(PartType::Engine); // fonte única — não fica stale
        let n = 4000;
        let dnf = (0..n)
            .filter(|k| {
                let r = (light + heavy) + (1.0 - light - heavy) * (*k as f64 / n as f64);
                sample_severity(PartType::Engine, false, r, true) == Severity::Dnf
            })
            .count();
        let frac = dnf as f64 / n as f64;
        assert!((frac - ENDURO_DNF_SCALE).abs() < 0.08, "estrutural deveria manter ~25% de DNF, deu {frac}");
        // Não-estrutural NUNCA vira DNF no enduro, nem na fatia de DNF nem na parede.
        for k in 0..1000 {
            let r = 0.96 + 0.04 * (k as f64 / 1000.0);
            assert_ne!(sample_severity(PartType::FrontWing, false, r, true), Severity::Dnf);
            assert_ne!(sample_severity(PartType::FrontWing, true, r, true), Severity::Dnf);
        }
        // Sem enduro, a fatia inteira do motor é DNF.
        assert_eq!(sample_severity(PartType::Engine, false, 0.95, false), Severity::Dnf);
    }

    /// Eixo 2 (rampa de fim): a mesma peça no fio quebra MAIS no clímax (progress alto) que no
    /// começo (progress baixo) no enduro — a rampa de desgaste morde da metade pro fim.
    #[test]
    fn enduro_rampa_agrava_o_fim_da_corrida() {
        assert!(enduro_late_ramp(0.0) < enduro_late_ramp(1.0));
        assert!((enduro_late_ramp(0.25) - 1.0).abs() < 1e-9, "1ª metade não deveria ter rampa");
        assert!((enduro_late_ramp(0.5) - 1.0).abs() < 1e-9, "rampa começa na metade");
        assert!((enduro_late_ramp(1.0) - (1.0 + ENDURO_LATE_RAMP_EXTRA)).abs() < 1e-9);
    }

    // -------- Economia do enduro (custo + alívio de parada) --------

    #[test]
    fn gate_de_enduro_por_duracao() {
        assert!(!is_enduro_duration(30));
        assert!(!is_enduro_duration(40)); // no gate ainda é sprint
        assert!(is_enduro_duration(41));
        assert!(is_enduro_duration(60));
    }

    #[test]
    fn sprint_nao_tem_sobrecusto_de_peca() {
        for d in [0u8, 15, 25, 30, 40] {
            assert!((enduro_economy_wear_mult(d, 0) - 1.0).abs() < 1e-9, "sprint {d}min deveria ser 1.0");
        }
    }

    #[test]
    fn enduro_custa_mais_e_escala_com_a_duracao() {
        let m60 = enduro_economy_wear_mult(60, 0);
        let m80 = enduro_economy_wear_mult(80, 0);
        assert!((m60 - 2.0).abs() < 1e-9, "60min sem parada deveria ser 2.0×, deu {m60}");
        assert!(m80 > m60, "corrida mais longa deveria custar mais ({m80} vs {m60})");
    }

    #[test]
    fn parada_alivia_o_sobrecusto_com_teto_de_30() {
        let base = enduro_economy_wear_mult(60, 0); // 2.0
        let uma = enduro_economy_wear_mult(60, 1); // −10% do sobrecusto
        let tres = enduro_economy_wear_mult(60, 3); // −30% (teto)
        let cinco = enduro_economy_wear_mult(60, 5); // ainda −30% (teto)
        assert!(uma < base && tres < uma, "cada parada deveria aliviar ({base} → {uma} → {tres})");
        assert!((tres - cinco).abs() < 1e-9, "o alívio deveria travar em 30% (3+ paradas)");
        // O alívio nunca leva abaixo de 1.0 (só corta o sobrecusto).
        assert!(cinco > 1.0, "enduro com muitas paradas ainda custa mais que sprint ({cinco})");
        assert!((enduro_pit_relief(3) - 0.30).abs() < 1e-9);
        assert!((enduro_pit_relief(10) - 0.30).abs() < 1e-9);
    }

    #[test]
    fn paradas_da_ia_sao_modeladas_pela_duracao() {
        assert_eq!(modeled_ai_pits(30), 0, "sprint: sem parada modelada");
        assert_eq!(modeled_ai_pits(60), 2, "60min ≈ 2 stints");
        assert_eq!(modeled_ai_pits(90), 3);
    }

    // -------- Tenda de durabilidade por NÍVEL (§4.8) --------

    #[test]
    fn tenda_de_nivel_tem_pico_no_5_e_e_simetrica() {
        use crate::car::wear::level_durability_mult as m;
        assert!(m(5) > m(4) && m(5) > m(6), "pico de vida no nível 5");
        assert!((m(4) - m(6)).abs() < 1e-9, "4 e 6 = normal (iguais)");
        assert!(
            (m(3) - m(7)).abs() < 1e-9 && (m(2) - m(8)).abs() < 1e-9 && (m(1) - m(9)).abs() < 1e-9,
            "curva simétrica em torno do 5"
        );
        assert!(m(1) < m(2) && m(2) < m(3) && m(3) < m(4) && m(4) < m(5), "sobe até o 5");
        assert!(m(9) < m(8) && m(8) < m(7) && m(7) < m(6), "cai depois do 5");
    }

    #[test]
    fn peca_nivel_alto_quebra_mais_que_nivel_5() {
        // MESMA peça (motor), MESMO desgaste de entrada — só o NÍVEL muda. Nível 8 (de ponta,
        // frágil) quebra MAIS que o nível 5 (o ponto confiável): o tradeoff desempenho×confiab.
        let breaks = |level: u8| -> usize {
            (0..1500u64)
                .filter(|&s| {
                    let mut car = Car::uniform(level);
                    car.set_wear(PartType::Engine, 0.80);
                    !roll_race_breakdowns(&car, 18, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[])
                        .is_empty()
                })
                .count()
        };
        let n5 = breaks(5);
        let n8 = breaks(8);
        assert!(n8 > n5 + 50, "nível 8 deveria quebrar bem mais que o 5 (5={n5}, 8={n8})");
    }

    #[test]
    fn categoria_spec_ignora_a_tenda_de_nivel() {
        // Guard do §4.8: sem a tenda (`apply_tent=false`, categoria spec) o NÍVEL não muda a vida
        // — o carro do iniciante (tudo nível 1) NÃO é penalizado. Com a tenda, nível 1 quebra mais.
        let breaks = |level: u8, tent: bool| -> usize {
            (0..1500u64)
                .filter(|&s| {
                    let mut car = Car::uniform(level);
                    car.set_wear(PartType::Engine, 0.80);
                    !roll_race_breakdowns_cfg(
                        &car, 18, s, PIT_NEUTRO, TRACK_NEUTRO, WEATHER_NEUTRO, &[], false, tent,
                    )
                    .is_empty()
                })
                .count()
        };
        assert!(breaks(1, true) > breaks(5, true), "com tenda, nível 1 (0.60×) quebra mais que 5");
        assert_eq!(breaks(1, false), breaks(5, false), "sem tenda (spec), o nível é irrelevante");
    }
}
