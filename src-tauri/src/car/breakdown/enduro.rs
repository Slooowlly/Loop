//! **As regras de ENDURO** — o terceiro papel de `car::breakdown`.
//!
//! Corrida longa não é corrida curta com mais voltas. O objetivo, do design, é que o grid
//! **não esvazie** (DNF raro) e que a peça **custe muito mais** — e que esse custo escale da
//! metade para o fim da prova. As três alavancas moram aqui:
//!
//! 1. **A severidade abrandada** ([`ENDURO_DNF_SCALE`]): a maior parte do que seria DNF vira
//!    parada longa. Os carros pingam no box mas cruzam a bandeira.
//! 2. **A rampa de fim** ([`enduro_late_ramp`]): desgaste por volta que sobe da metade da
//!    corrida em diante — o carro se arrastando nas últimas horas.
//! 3. **O sobrecusto de peça** ([`enduro_economy_wear_mult`]): o desgaste que PERSISTE na
//!    economia, aliviado por parada real e limitado por [`ENDURO_SURCHARGE_CAP`].
//!
//! Ver o design em `docs/superpowers/specs/2026-07-18-car-breakdown-system.md` (enduro).

// Objetivo: em corrida longa o grid NÃO esvazia (DNF raro), mas a peça CUSTA muito mais —
// e o custo escala da metade pro fim da corrida. Uma parada REAL (pneu/combustível) alivia
// o gasto. Ver design: docs/superpowers/specs/2026-07-18-car-breakdown-system.md (enduro).

/// Corrida acima desta duração (min) é ENDURO — ativa a severidade abrandada, a rampa de
/// desgaste no fim e o custo/parada. Abaixo disto, sprint: tudo se comporta como antes.
pub const ENDURO_DURATION_GATE_MIN: f64 = 40.0;
/// Fração da fatia de DNF que PERMANECE DNF no enduro (o resto vira Grave = pit longo). Com
/// 0.25, o motor cai de ~38% → ~9,5% de DNF: os carros pingam no box mas cruzam a bandeira.
pub(super) const ENDURO_DNF_SCALE: f64 = 0.25;
/// Desgaste EXTRA por volta no clímax do enduro (progresso→1): +80% no fim (rampa 1.8×). Começa
/// na METADE da corrida (progresso 0.5) e sobe linear — é o carro se arrastando nas últimas horas.
pub(super) const ENDURO_LATE_RAMP_EXTRA: f64 = 0.80;
/// Inclinação do sobrecusto de peça do enduro por unidade de `(duração−gate)/gate`. Com 2.0,
/// uma corrida de 60 min (over=0.5) custa 2.0× o desgaste-base de peças; 80 min → 3.0×.
pub(super) const ENDURO_COST_K: f64 = 2.0;
/// Teto do sobrecusto ([`enduro_surcharge`]), ANTES do alívio de parada.
///
/// **Não é um número escolhido, é um invariante:** uma prova única não pode consumir mais
/// que a vida inteira de uma peça. O multiplicador de desgaste da etapa é `1 + sobrecusto`
/// e o desgaste de uma etapa é `car::wear::wear_per_race = 1/durabilidade`; a peça mais
/// frágil do carro tem durabilidade 3 ([`crate::car::parts::PartType::durability`] — motor,
/// câmbio, freios, suspensão, asa dianteira), ou seja `1/3` de vida por etapa. O maior
/// multiplicador que respeita o invariante é 3,0, e daí o teto de 2,0 no sobrecusto.
/// `o_teto_do_sobrecusto_sai_da_peca_mais_fragil` trava a derivação: baixar a durabilidade
/// mínima quebra o teste em vez de reabrir o buraco em silêncio.
///
/// Acima do invariante o excedente é desperdiçado — a peça já morreu no meio da prova e a
/// decisão de manutenção seguinte deixa de existir. Sem teto, medido, uma prova de 360 min
/// consumia 4,07 vidas de motor e o desgaste médio de fim de temporada ia de 0,46 para 5,32
/// (escala 0..1, onde 1,0 é fim de vida).
///
/// O teto morde em 80 min (`over = 1,0`), o que preserva EXATAMENTE os dois pontos que a
/// calibração original fixou (60 min → 2,0×, 80 min → 3,0×) e achata tudo acima disso no
/// mesmo 3,0×. O achatamento é consequência do invariante, não uma escolha: a MENOR duração
/// real do Endurance (120 min) já pedia 3,8×. Voltar a diferenciar 120 de 360 entre si é
/// decisão de balanceamento e pede uma rodada do harness de economia; o que não pode voltar
/// é o desgaste sem teto.
pub(super) const ENDURO_SURCHARGE_CAP: f64 = 2.0;
/// Alívio de gasto de peça por PARADA REAL (pneu/combustível) no enduro.
pub(super) const ENDURO_PIT_RELIEF: f64 = 0.10;
/// Teto do alívio acumulado (3+ paradas → −30% do sobrecusto).
pub(super) const ENDURO_RELIEF_CAP: f64 = 0.30;
/// Duração média de um stint (min) pra MODELAR as paradas da IA (o jogador usa o pit REAL).
pub(super) const ENDURO_STINT_MIN: f64 = 30.0;

/// Duração de uma prova, em minutos, **garantidamente conhecida** — nunca a sentinela.
///
/// Existe por causa de uma armadilha medida: `constants::categories` guarda
/// `duracao_corrida_min = 0` como SENTINELA de "quem sorteia a duração é o calendário", e a
/// categoria sentinelada é justamente o **Endurance**. Quem lia a constante e passava o zero
/// para [`is_enduro_duration`] recebia `false` e tratava uma prova de 6 horas como sprint. Os
/// call sites que erravam foram consertados um a um, mas a assinatura `u16` continuava
/// aceitando o zero de braços abertos — o bug tinha conserto, a armadilha não.
///
/// A cascata que RESOLVE a sentinela mora em `calendar::entry::duracao_efetiva_min` (etapa →
/// categoria → padrão de sprint). Este tipo é o outro lado da mesma pinça: ele se recusa a
/// existir com zero, então o que a cascata não resolveu não atravessa até o gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DuracaoDeProva(u16);

impl DuracaoDeProva {
    /// `None` para a sentinela. Quem tem só a constante da categoria em mãos deve passar
    /// antes por `calendar::entry::duracao_efetiva_min`, que é quem sabe a cascata.
    pub const fn nova(minutos: u16) -> Option<Self> {
        if minutos == 0 {
            None
        } else {
            Some(Self(minutos))
        }
    }

    pub const fn minutos(self) -> u16 {
        self.0
    }

    /// A prova é ENDURO? Gate único usado no risco ao vivo E na economia.
    pub fn e_enduro(self) -> bool {
        self.0 as f64 > ENDURO_DURATION_GATE_MIN
    }

    /// Paradas MODELADAS da IA por esta duração — ver [`modeled_ai_pits`].
    pub fn paradas_modeladas_da_ia(self) -> u32 {
        if !self.e_enduro() {
            return 0;
        }
        ((self.0 as f64) / ENDURO_STINT_MIN).floor().max(0.0) as u32
    }

    /// Multiplicador de desgaste da etapa na economia — ver [`enduro_economy_wear_mult`].
    pub fn mult_de_desgaste_na_economia(self, genuine_pits: u32) -> f64 {
        if !self.e_enduro() {
            return 1.0;
        }
        1.0 + enduro_surcharge(self.0) * (1.0 - enduro_pit_relief(genuine_pits))
    }
}

/// A corrida é ENDURO pela duração (min)? Gate único usado no risco ao vivo E na economia.
///
/// **Prefira [`DuracaoDeProva::e_enduro`].** Esta forma aceita a sentinela `0` e responde
/// `false` para ela, que é exatamente o modo como o Endurance já foi tratado como sprint uma
/// vez. Ela segue viva só enquanto os call sites de `commands/` não migram para o tipo.
pub fn is_enduro_duration(duracao_min: u16) -> bool {
    DuracaoDeProva::nova(duracao_min).is_some_and(DuracaoDeProva::e_enduro)
}

/// Multiplicador de desgaste POR VOLTA pela fase da corrida no enduro: 1.0 até a metade
/// (`progress ≤ 0.5`) e sobe linear até `1 + ENDURO_LATE_RAMP_EXTRA` no fim (`progress = 1`).
/// Fora do enduro o chamador não aplica isto. É o "principalmente da metade pro fim": as peças
/// se cansam no clímax → mais reparos no box no fim da corrida longa.
pub(super) fn enduro_late_ramp(progress: f64) -> f64 {
    let t = ((progress - 0.5) * 2.0).clamp(0.0, 1.0);
    1.0 + ENDURO_LATE_RAMP_EXTRA * t
}

/// Sobrecusto de desgaste de peça do enduro pela duração (acima do gate), ANTES do alívio de
/// parada. Contínuo no gate (40 min → 0), linear até [`ENDURO_SURCHARGE_CAP`] e constante daí
/// em diante. Ex.: 60 min → 1.0 (over 0.5 × K 2.0); 80 min → 2.0, que já é o teto; 120 e
/// 360 min → 2.0 também.
fn enduro_surcharge(duracao_min: u16) -> f64 {
    let over =
        ((duracao_min as f64) - ENDURO_DURATION_GATE_MIN).max(0.0) / ENDURO_DURATION_GATE_MIN;
    (ENDURO_COST_K * over).min(ENDURO_SURCHARGE_CAP)
}

/// Alívio de gasto de peça por PARADA REAL no enduro: 10%/parada, teto 30% (3+ paradas).
pub fn enduro_pit_relief(genuine_pits: u32) -> f64 {
    (ENDURO_PIT_RELIEF * genuine_pits as f64).min(ENDURO_RELIEF_CAP)
}

/// Paradas MODELADAS da IA pela duração (o jogador usa o pit REAL do SDK). Um stint dura
/// ~[`ENDURO_STINT_MIN`] min → uma corrida de 60 min ≈ 2 paradas. 0 fora do enduro.
///
/// **Prefira [`DuracaoDeProva::paradas_modeladas_da_ia`]** — ver [`DuracaoDeProva`].
pub fn modeled_ai_pits(duracao_min: u16) -> u32 {
    DuracaoDeProva::nova(duracao_min)
        .map(DuracaoDeProva::paradas_modeladas_da_ia)
        .unwrap_or(0)
}

/// Multiplicador de desgaste (→ CUSTO) da corrida na ECONOMIA da grade toda: 1.0 no sprint;
/// no enduro sobe com a duração ([`enduro_surcharge`]) e é aliviado por paradas reais
/// ([`enduro_pit_relief`]). O sobrecusto persiste no desgaste → as peças chegam ao fim da vida
/// mais cedo → mais trocas ao longo da temporada de enduro (a conta que o usuário pediu). Uma
/// parada nunca leva abaixo de 1.0 (o alívio só corta o sobrecusto, não o desgaste-base).
///
/// **Prefira [`DuracaoDeProva::mult_de_desgaste_na_economia`]** — ver [`DuracaoDeProva`].
pub fn enduro_economy_wear_mult(duracao_min: u16, genuine_pits: u32) -> f64 {
    DuracaoDeProva::nova(duracao_min)
        .map(|d| d.mult_de_desgaste_na_economia(genuine_pits))
        .unwrap_or(1.0)
}
