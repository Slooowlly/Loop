//! Camada de **comportamento por corrida** do export iRacing. **Lógica pura.**
//!
//! Cada corrida o piloto chega com uma "atitude do dia": os atributos secundários
//! (agressividade/otimismo/suavidade) variam MUITO conforme o contexto, mas o PACE
//! (driverSkill) quase não se move (±2, ±4 pra quem é muito afetado) — o pace é a
//! identidade. SÓ no export (a sim offline usa pace_delta + error_mult).
//!
//! Modelo: `atributo_final = base + Σ(sinais) × maleabilidade(mentalidade)`, clamp
//! 0–100. Sem teto artificial — a BASE é a inclinação, os sinais somam a partir dela;
//! um stack de sinais cautelosos vira a mão até do mais agressivo.
//!
//! A **mentalidade** age de DUAS formas (ambas contínuas — ninguém é 0 nem 100):
//! - GANHO: forte = estável (ganho baixo), fraca = volátil (ganho alto).
//! - COMPOSTURA: reduz o IMPACTO dos sinais ADVERSOS (choke, má fase, pista nova,
//!   medo de chuva, status baixo) por corrida, de forma GRADUAL — quanto mais forte,
//!   menor o impacto médio, com variação do dia (de um dia que blinda quase tudo a um
//!   dia ruim que sente quase tudo). Mental baixo leva o adverso quase cheio sempre.
//!   Sinais favoráveis e traços (idade, casa, domínio, calor, humor do dia) sempre valem.
//!
//! Tier 1 (dado já pronto). Tier 2/3 entram depois como +1 função somando aqui.

use crate::simulation::pressure::{self, TitleContext};
use crate::simulation::track_knowledge::TrackKnowledge;

// --- Ganho pela mentalidade (espinha dorsal) ------------------------------------
const MALL_MIN: f64 = 0.6; // mental forte (100) → sinais amortecidos
const MALL_MAX: f64 = 1.4; // mental fraco (0) → sinais amplificados
/// O quanto o mais forte mentalmente consegue blindar do adverso no melhor dia —
/// nunca 100% (ninguém é imune). Compostura efetiva = mentalidade × isto.
const MAX_COMPOSURE: f64 = 0.75;

/// Quanto os sinais conseguem deformar o piloto (0.6 forte … 1.4 fraco).
pub fn malleability(mentality: f64) -> f64 {
    MALL_MAX - mentality.clamp(0.0, 100.0) / 100.0 * (MALL_MAX - MALL_MIN)
}

/// Fração do impacto ADVERSO que o piloto leva nesta corrida (0 = blindou tudo,
/// 1 = levou cheio). GRANULAR: a mentalidade puxa a média pra baixo e um sorteio do
/// dia dá a variação. Mental 0 → sempre 1.0; quanto mais forte, menor e mais variável.
pub fn adverse_multiplier(mentality: f64, seed: u64) -> f64 {
    let composure = mentality.clamp(0.0, 100.0) / 100.0 * MAX_COMPOSURE;
    (1.0 - composure * composure_roll(seed)).clamp(0.0, 1.0)
}

// --- Magnitudes por sinal (tunáveis; calibrar com teste real) -------------------
const PRESS_AGG: f64 = 10.0;
const PRESS_OPT: f64 = 8.0;
const PRESS_SMO: f64 = 12.0;
const PRESS_SKILL: f64 = 1.9; // pequeno de propósito (pace ~ ±2..4 no extremo)
// Casa cheia (interesse do evento). Neutro mais baixo que o título (0.55) → palco =
// oportunidade, a maioria rende. GAIN leva stakes×forma a uma intensidade ~0..3
// comparável à do título. Reaproveita as magnitudes PRESS_* acima.
const EVENT_PRESS_NEUTRAL: f64 = 0.42;
const EVENT_PRESS_GAIN: f64 = 1.9;
const CRUISE_AGG: f64 = 10.0;
const CRUISE_SMO: f64 = 10.0;
const FORM_OPT: f64 = 16.0;
const FORM_AGG: f64 = 12.0;
const FORM_SMO: f64 = 14.0;
const SURVIVAL_AGG: f64 = 14.0;
const SURVIVAL_SMO: f64 = 12.0;
const AFF_AGG: f64 = 10.0;
const AFF_OPT: f64 = 12.0;
const AFF_SMO: f64 = 12.0;
const RAIN_AGG: f64 = 14.0;
const RAIN_OPT: f64 = 12.0;
const RAIN_SMO: f64 = 16.0;
const HEAT_THRESHOLD: f64 = 28.0;
const HEAT_AGG: f64 = 10.0;
const HEAT_SMO: f64 = 12.0;
const AGE_AGG: f64 = 12.0;
const AGE_OPT: f64 = 10.0;
const AGE_SMO: f64 = 12.0;
const STATUS_OPT: f64 = 12.0;
const STATUS_SMO: f64 = 8.0;
const STATUS_GRID_WEIGHT: f64 = 0.6; // domínio no grid pesa mais que a fama global
const HOME_AGG: f64 = 8.0;
const HOME_OPT: f64 = 12.0;
const WOBBLE: f64 = 6.0;
// Tier 2 (mesmos dados de Tier 1: resultados recentes / idade / fim de temporada).
const STREAK_OPT: f64 = 6.0; // por vitória na sequência (cap)
const STREAK_AGG: f64 = 4.0;
const STREAK_CAP: u32 = 3;
const NEARMISS_AGG: f64 = 10.0;
const NEARMISS_OPT: f64 = 8.0;
const FATIGUE_OPT: f64 = 9.0;
const FATIGUE_SMO: f64 = 7.0;
const PRODIGY_OPT: f64 = 10.0;
const PRODIGY_AGG: f64 = 8.0;
// Tier 2 Batch B (sourcing no comando: carreira / contrato / time / companheiro).
const MILESTONE_AGG: f64 = 8.0;
const MILESTONE_OPT: f64 = 8.0;
const CONTRACT_AGG: f64 = 10.0;
const CONTRACT_OPT: f64 = 6.0;
const CONTRACT_SMO: f64 = 12.0;
const TEAMMATE_AGG: f64 = 12.0;
const PROMO_AGG: f64 = 10.0;
const PROMO_SMO: f64 = 12.0;
const RELEG_AGG: f64 = 10.0;
const RELEG_OPT: f64 = 10.0;
const MORALE_OPT: f64 = 12.0;
const MORALE_AGG: f64 = 10.0;
const MORALE_SMO: f64 = 10.0;
const PRIZE_AGG: f64 = 9.0;
const PRIZE_SMO: f64 = 6.0;
const PRIZE_WINDOW: u32 = 3; // só nas últimas corridas
const INJURY_AGG: f64 = 10.0;
const INJURY_OPT: f64 = 8.0;
const INJURY_SMO: f64 = 12.0;
// Tier 3 (estado/derivado: contrato, DNFs recentes, crash na pista).
const HONEYMOON_AGG: f64 = 10.0;
const HONEYMOON_OPT: f64 = 10.0;
const HONEYMOON_SMO: f64 = 8.0;
const REVENGE_AGG: f64 = 16.0;
const BADLUCK_AGG: f64 = 8.0;
const BADLUCK_SMO: f64 = 10.0;
const TRAUMA_AGG: f64 = 12.0;
const TRAUMA_OPT: f64 = 8.0;
const TRAUMA_SMO: f64 = 14.0;
// Lote novo (rivalidade / história / carro / pista) — mesmo padrão de sourcing dos
// anteriores, sem schema novo.
const NEMESIS_AGG: f64 = 12.0;
const NEMESIS_SMO: f64 = 8.0;
const FORMER_TEAM_AGG: f64 = 10.0;
const FORMER_TEAM_OPT: f64 = 8.0;
const CHAMP_OPT: f64 = 10.0;
const CHAMP_SMO: f64 = 8.0;
const CHAMP_AGG: f64 = 8.0; // lado frágil, sob o peso de defender o título
const CHAMP_NEUTRAL: f64 = 0.5; // resiliência acima disto → autoridade; abaixo → peso
const DEBUT_AGG: f64 = 10.0;
const DEBUT_OPT: f64 = 12.0;
const DEBUT_SMO: f64 = 10.0;
const MECHDIST_AGG: f64 = 10.0;
const MECHDIST_SMO: f64 = 12.0;
const BOGEY_AGG: f64 = 8.0;
const BOGEY_OPT: f64 = 12.0;
const BOGEY_SMO: f64 = 8.0;
const BOGEY_MIN_STARTS: u32 = 3; // precisa ter corrido aqui várias vezes
const BOGEY_DEPTH: f64 = 0.45; // e o melhor resultado ainda pior que ~45% do grid

/// Empurrões de UM sinal (pontos crus, antes do ganho). Skill quase sempre 0.
#[derive(Clone, Copy, Debug, Default)]
pub struct Nudge {
    pub aggression: f64,
    pub optimism: f64,
    pub smoothness: f64,
    pub skill: f64,
}

impl Nudge {
    fn add(self, o: Nudge) -> Nudge {
        Nudge {
            aggression: self.aggression + o.aggression,
            optimism: self.optimism + o.optimism,
            smoothness: self.smoothness + o.smoothness,
            skill: self.skill + o.skill,
        }
    }

    fn scale(self, k: f64) -> Nudge {
        Nudge {
            aggression: self.aggression * k,
            optimism: self.optimism * k,
            smoothness: self.smoothness * k,
            skill: self.skill * k,
        }
    }
}

/// Saída de um sinal: o empurrão + se ele é ADVERSO (adversidade psicológica que um
/// mental forte pode blindar numa corrida). Favorável/traço → `adverse: false`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Signal {
    pub nudge: Nudge,
    pub adverse: bool,
}

fn fav(nudge: Nudge) -> Signal {
    Signal {
        nudge,
        adverse: false,
    }
}

// --- Sinais do Tier 1 -----------------------------------------------------------

/// Pressão de campeonato (do topo) + modo cruzeiro. `resilience` (0–1) decide choke
/// (ADVERSO) vs clutch/cruzeiro (favorável).
pub fn pressure_title(title: &TitleContext, races_left: u32, resilience: f64) -> Signal {
    // Cruzeiro: líder com título praticamente decidido → relaxa e administra.
    if title.title_decided && title.is_leader {
        return fav(Nudge {
            aggression: -CRUISE_AGG,
            smoothness: CRUISE_SMO,
            ..Default::default()
        });
    }
    let intensity = pressure::pressure_intensity(title, races_left); // 0..3
    if intensity <= 0.0 {
        return Signal::default();
    }
    let dir = 0.55 - resilience; // >0 = choke (frágil), <0 = clutch (resiliente)
    let nudge = Nudge {
        aggression: intensity * dir * PRESS_AGG, // choke → +agressivo
        optimism: intensity * dir * PRESS_OPT,
        smoothness: -intensity * dir * PRESS_SMO, // choke → −suave (bruto)
        skill: -intensity * dir * PRESS_SKILL,    // choke → pace cai um pouco
    };
    Signal {
        nudge,
        adverse: dir > 0.0,
    } // só o choke é adverso
}

/// Pressão de "casa cheia" (interesse do evento) — UNIVERSAL, ao contrário da de
/// título (que só pega quem briga). Palco = oportunidade: neutro mais baixo (0.42),
/// a maioria rende e só o frágil trava. Pesa MAIS em quem vem em má fase ("algo a
/// provar"). `event_stakes` 0..1 = quanto o evento desperta público. Mesma semântica
/// da sim (simulation/pressure.rs event_pressure_*). Choke = ADVERSO.
pub fn pressure_event(
    event_stakes: f64,
    recent_positions: &[u32],
    field_size: u32,
    resilience: f64,
) -> Signal {
    let stakes = event_stakes.clamp(0.0, 1.0);
    if stakes <= 0.0 {
        return Signal::default();
    }
    // "Algo a provar": quem vem no fundo do grid sente mais (0.7 na frente .. 1.6 no fundo).
    let form_weight = if recent_positions.is_empty() || field_size <= 1 {
        1.0
    } else {
        let avg =
            recent_positions.iter().map(|&p| p as f64).sum::<f64>() / recent_positions.len() as f64;
        let depth = ((avg - 1.0) / (field_size as f64 - 1.0)).clamp(0.0, 1.0);
        0.7 + depth * 0.9
    };
    let intensity = stakes * form_weight * EVENT_PRESS_GAIN; // ~0..3
    let dir = EVENT_PRESS_NEUTRAL - resilience; // >0 choke (só frágil <0.42), <0 clutch
    let nudge = Nudge {
        aggression: intensity * dir * PRESS_AGG,
        optimism: intensity * dir * PRESS_OPT,
        smoothness: -intensity * dir * PRESS_SMO,
        skill: -intensity * dir * PRESS_SKILL,
    };
    Signal {
        nudge,
        adverse: dir > 0.0, // só o choke é adverso (blindável pela compostura)
    }
}

/// Forma/embalo pelos resultados recentes + "seca de resultados" (pressão de baixo).
/// `recent_positions`: posições finais recentes (1 = vitória). Má fase = ADVERSO.
pub fn form(recent_positions: &[u32], field_size: u32, resilience: f64) -> Signal {
    if recent_positions.is_empty() || field_size <= 1 {
        return Signal::default();
    }
    let avg =
        recent_positions.iter().map(|&p| p as f64).sum::<f64>() / recent_positions.len() as f64;
    let depth = ((avg - 1.0) / (field_size as f64 - 1.0)).clamp(0.0, 1.0); // 0 frente .. 1 fundo
    let hot = 0.5 - depth; // >0 em alta, <0 em baixa

    let mut n = Nudge {
        optimism: hot * FORM_OPT,
        aggression: hot.max(0.0) * FORM_AGG, // em alta → mais agressivo
        smoothness: hot.min(0.0) * FORM_SMO, // em baixa → menos suave (abalado)
        skill: 0.0,
    };
    // Seca de resultados (zona de baixo) = pressão de entregar → desespero se frágil.
    if depth > 0.7 {
        let despair = (depth - 0.7) / 0.3; // 0..1
        let choke = (0.55 - resilience).max(0.0); // só o lado frágil
        n.aggression += despair * choke * SURVIVAL_AGG;
        n.smoothness -= despair * choke * SURVIVAL_SMO;
    }
    Signal {
        nudge: n,
        adverse: hot < 0.0,
    } // má fase = adverso
}

/// Afinidade com a pista. Pista nova = ADVERSO (cauteloso); domínio = favorável.
pub fn track_affinity(k: &TrackKnowledge) -> Signal {
    if k.starts == 0 {
        return Signal {
            nudge: Nudge {
                aggression: -AFF_AGG,
                optimism: -AFF_OPT,
                smoothness: AFF_SMO,
                skill: 0.0,
            },
            adverse: true,
        };
    }
    // Domínio = pódio aqui, OU experiente (≥4 largadas) COM um resultado ao menos
    // decente (≤P8). Experiência sem resultado não é mais domínio — abre espaço p/ a
    // pista-fantasma (bogey_track) tratar o experiente-mas-ruim.
    let masters =
        k.best_finish.is_some_and(|b| b <= 3) || (k.starts >= 4 && k.best_finish.is_some_and(|b| b <= 8));
    if masters {
        return fav(Nudge {
            aggression: AFF_AGG,
            optimism: AFF_OPT,
            ..Default::default()
        });
    }
    Signal::default()
}

/// Clima: medo da chuva = ADVERSO (recolhe); mestre = favorável (ataca). `intensity` 0–1.
pub fn weather(is_wet: bool, fator_chuva: f64, intensity: f64) -> Signal {
    if !is_wet {
        return Signal::default();
    }
    let mastery = (fator_chuva.clamp(0.0, 100.0) - 50.0) / 50.0; // -1 teme .. 1 mestre
    let k = intensity.clamp(0.0, 1.0);
    let nudge = Nudge {
        aggression: mastery * k * RAIN_AGG,
        optimism: mastery * k * RAIN_OPT,
        smoothness: -mastery * k * RAIN_SMO, // teme (mastery<0) → suavidade↑
        skill: 0.0,
    };
    Signal {
        nudge,
        adverse: mastery < 0.0,
    } // teme a chuva = adverso
}

/// Calor extremo → gestão de pneu: mais suave, menos agressivo. Traço (não adverso).
pub fn heat(temp_c: f64) -> Signal {
    if temp_c <= HEAT_THRESHOLD {
        return Signal::default();
    }
    let h = ((temp_c - HEAT_THRESHOLD) / 12.0).clamp(0.0, 1.0); // 28..40 °C
    fav(Nudge {
        aggression: -h * HEAT_AGG,
        smoothness: h * HEAT_SMO,
        ..Default::default()
    })
}

/// Idade/fase: jovem cru e afoito; veterano calculista e suave. Traço (não adverso).
pub fn age_phase(age: u32) -> Signal {
    let a = age as f64;
    if a <= 22.0 {
        let y = ((23.0 - a) / 7.0).clamp(0.0, 1.0); // ~16..22
        return fav(Nudge {
            aggression: y * AGE_AGG,
            optimism: y * AGE_OPT,
            smoothness: -y * AGE_SMO,
            skill: 0.0,
        });
    }
    if a >= 33.0 {
        let v = ((a - 32.0) / 8.0).clamp(0.0, 1.0); // 33..40
        return fav(Nudge {
            aggression: -v * AGE_AGG,
            smoothness: v * AGE_SMO,
            ..Default::default()
        });
    }
    Signal::default()
}

/// Status/reputação: fama GLOBAL (40%) + domínio no GRID atual (60%, contexto
/// imediato — alfa de grid fraco > craque anônimo no pelotão). Status baixo = ADVERSO
/// (titubeia); topo = favorável (autoridade). Percentis 0–1 (1 = topo).
pub fn status(global_percentile: f64, grid_percentile: f64) -> Signal {
    let blend = grid_percentile.clamp(0.0, 1.0) * STATUS_GRID_WEIGHT
        + global_percentile.clamp(0.0, 1.0) * (1.0 - STATUS_GRID_WEIGHT);
    let s = (blend - 0.5) * 2.0; // -1 fundo .. 1 topo
    let nudge = Nudge {
        optimism: s * STATUS_OPT,
        smoothness: s.max(0.0) * STATUS_SMO,
        ..Default::default()
    };
    Signal {
        nudge,
        adverse: s < 0.0,
    } // fundo = titubeia (adverso)
}

/// Corrida em casa (país natal == país da pista): motivação extra. Favorável.
pub fn home_race(is_home: bool) -> Signal {
    if !is_home {
        return Signal::default();
    }
    fav(Nudge {
        aggression: HOME_AGG,
        optimism: HOME_OPT,
        ..Default::default()
    })
}

/// Sequência de vitórias (das mais recentes) → swagger. Favorável.
/// `recent_positions` em ordem [mais recente … mais antiga].
pub fn win_streak(recent_positions: &[u32]) -> Signal {
    let streak = recent_positions.iter().take_while(|&&p| p == 1).count() as u32;
    if streak < 2 {
        return Signal::default();
    }
    let n = streak.min(STREAK_CAP) as f64;
    fav(Nudge {
        optimism: n * STREAK_OPT,
        aggression: n * STREAK_AGG,
        ..Default::default()
    })
}

/// Vários pódios SEM vitória → coceira de risco pra finalmente vencer. Favorável (drive).
pub fn near_miss(recent_positions: &[u32]) -> Signal {
    if recent_positions.is_empty() {
        return Signal::default();
    }
    let podiums = recent_positions.iter().filter(|&&p| p <= 3).count();
    let wins = recent_positions.iter().filter(|&&p| p == 1).count();
    if wins > 0 || podiums < 2 {
        return Signal::default();
    }
    let itch = (podiums as f64 / recent_positions.len() as f64).min(1.0);
    fav(Nudge {
        aggression: itch * NEARMISS_AGG,
        optimism: itch * NEARMISS_OPT,
        ..Default::default()
    })
}

/// Desgaste de fim de temporada longa → menos afiado. ADVERSO.
pub fn end_season_fatigue(races_left: u32, season_length: u32) -> Signal {
    if season_length < 8 || races_left > 2 {
        return Signal::default();
    }
    Signal {
        nudge: Nudge {
            optimism: -FATIGUE_OPT,
            smoothness: -FATIGUE_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Prodígio em ascensão: jovem E indo bem → bola de neve de confiança. Favorável.
pub fn rising_prodigy(age: u32, recent_positions: &[u32], field_size: u32) -> Signal {
    if age > 23 || recent_positions.is_empty() || field_size <= 1 {
        return Signal::default();
    }
    let avg =
        recent_positions.iter().map(|&p| p as f64).sum::<f64>() / recent_positions.len() as f64;
    let depth = ((avg - 1.0) / (field_size as f64 - 1.0)).clamp(0.0, 1.0);
    if depth > 0.4 {
        return Signal::default(); // precisa estar indo BEM
    }
    let youth = ((24.0 - age as f64) / 8.0).clamp(0.0, 1.0);
    let hot = (0.4 - depth) / 0.4; // 0..1
    let k = youth * hot;
    fav(Nudge {
        optimism: k * PRODIGY_OPT,
        aggression: k * PRODIGY_AGG,
        ..Default::default()
    })
}

/// Caça a marco: a PRÓXIMA vitória é um número redondo → fogo extra. Favorável.
pub fn milestone_chase(career_wins: u32) -> Signal {
    let next = career_wins + 1;
    let milestone = next == 10 || next == 25 || (next >= 50 && next % 25 == 0);
    if !milestone {
        return Signal::default();
    }
    fav(Nudge {
        aggression: MILESTONE_AGG,
        optimism: MILESTONE_OPT,
        ..Default::default()
    })
}

/// Último ano de contrato → pressão de impressionar (showboating); frágil fica impreciso.
/// ADVERSO.
pub fn contract_year(last_year: bool, resilience: f64) -> Signal {
    if !last_year {
        return Signal::default();
    }
    let fragile = (0.55 - resilience).max(0.0);
    Signal {
        nudge: Nudge {
            aggression: CONTRACT_AGG, // sempre tenta aparecer
            optimism: CONTRACT_OPT,
            smoothness: -fragile * CONTRACT_SMO, // frágil → ragged
            ..Default::default()
        },
        adverse: true,
    }
}

/// Duelo interno: apanhando do companheiro de equipe → orgulho, responde forte. Favorável.
pub fn teammate_duel(my_points: f64, teammate_points: Option<f64>) -> Signal {
    let Some(tp) = teammate_points else {
        return Signal::default();
    };
    if tp <= my_points {
        return Signal::default();
    }
    let deficit = ((tp - my_points) / 50.0).clamp(0.0, 1.0); // déficit em pontos (cap)
    fav(Nudge {
        aggression: deficit * TEAMMATE_AGG,
        ..Default::default()
    })
}

/// Mudança de categoria: promovido (subiu) → respeito/cautela (ADVERSO); rebaixado
/// (caiu) → swagger (favorável). `mv`: +1 subiu, -1 caiu, 0 nada.
pub fn category_move(mv: i32) -> Signal {
    if mv > 0 {
        Signal {
            nudge: Nudge {
                aggression: -PROMO_AGG,
                smoothness: PROMO_SMO,
                ..Default::default()
            },
            adverse: true,
        }
    } else if mv < 0 {
        fav(Nudge {
            aggression: RELEG_AGG,
            optimism: RELEG_OPT,
            ..Default::default()
        })
    } else {
        Signal::default()
    }
}

/// Moral no time/carro: feliz → calmo e confiante; insatisfeito → frustrado (ADVERSO).
/// `morale` é multiplicador (~0.5 infeliz … 1.5 feliz; 1.0 neutro).
pub fn team_morale(morale: f64) -> Signal {
    let happy = (morale - 1.0).clamp(-1.0, 1.0);
    if happy.abs() < 0.05 {
        return Signal::default();
    }
    if happy > 0.0 {
        fav(Nudge {
            optimism: happy * MORALE_OPT,
            smoothness: happy * MORALE_SMO,
            ..Default::default()
        })
    } else {
        Signal {
            nudge: Nudge {
                aggression: -happy * MORALE_AGG, // infeliz → frustrado/agressivo
                smoothness: happy * MORALE_SMO,  // e mais bruto
                ..Default::default()
            },
            adverse: true,
        }
    }
}

/// Briga por posição/grana no fim: FORA da briga de título, mas com um rival colado
/// numa posição que vale dinheiro → tensão competitiva. ADVERSO.
pub fn prize_fight(
    my_points: f64,
    all_points: &[f64],
    races_left: u32,
    max_points: f64,
    in_title_fight: bool,
) -> Signal {
    if in_title_fight || races_left > PRIZE_WINDOW || all_points.len() < 2 || max_points <= 0.0 {
        return Signal::default();
    }
    let nearest = all_points
        .iter()
        .map(|&p| (p - my_points).abs())
        .filter(|&d| d > 1e-6)
        .fold(f64::MAX, f64::min);
    let reach = races_left as f64 * max_points;
    if nearest == f64::MAX || nearest > reach {
        return Signal::default();
    }
    let tension = (1.0 - nearest / reach).clamp(0.0, 1.0); // rival colado → tensão alta
    Signal {
        nudge: Nudge {
            aggression: tension * PRIZE_AGG,
            smoothness: -tension * PRIZE_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Retorno de lesão: voltou há poucas corridas → cauteloso, ainda se recompondo.
/// ADVERSO.
pub fn injury_return(recently_returned: bool) -> Signal {
    if !recently_returned {
        return Signal::default();
    }
    Signal {
        nudge: Nudge {
            aggression: -INJURY_AGG,
            optimism: -INJURY_OPT,
            smoothness: INJURY_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Lua de mel: chegou no time ESTA temporada → ansioso pra impressionar, sobra-pista.
/// Favorável.
pub fn honeymoon(joined_this_season: bool) -> Signal {
    if !joined_this_season {
        return Signal::default();
    }
    fav(Nudge {
        aggression: HONEYMOON_AGG,
        optimism: HONEYMOON_OPT,
        smoothness: -HONEYMOON_SMO,
        ..Default::default()
    })
}

/// Vingança: foi tirado de corrida numa colisão na ÚLTIMA corrida → corre furioso.
/// ADVERSO (raiva que a cabeça fria controla).
pub fn revenge(crashed_out_last_race: bool) -> Signal {
    if !crashed_out_last_race {
        return Signal::default();
    }
    Signal {
        nudge: Nudge {
            aggression: REVENGE_AGG,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Azar acumulado: vários DNFs SEM culpa recentes → frustração, pavio curto. ADVERSO.
pub fn bad_luck(not_at_fault_dnfs: u32) -> Signal {
    if not_at_fault_dnfs < 2 {
        return Signal::default();
    }
    let k = not_at_fault_dnfs.min(3) as f64 / 3.0;
    Signal {
        nudge: Nudge {
            aggression: k * BADLUCK_AGG,
            smoothness: -k * BADLUCK_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Trauma de pista: já bateu feio NESTA pista → cautela psicológica ali. ADVERSO.
pub fn track_trauma(crashed_here_before: bool) -> Signal {
    if !crashed_here_before {
        return Signal::default();
    }
    Signal {
        nudge: Nudge {
            aggression: -TRAUMA_AGG,
            optimism: -TRAUMA_OPT,
            smoothness: TRAUMA_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Nêmesis: cruzou a linha lado a lado (±1 posição) com o MESMO rival em ≥2 das últimas
/// corridas → rivalidade pessoal, corre no limite contra ele. ADVERSO (emoção que a
/// cabeça fria controla). O rival pode ser o próprio jogador.
pub fn nemesis(has_nemesis: bool) -> Signal {
    if !has_nemesis {
        return Signal::default();
    }
    Signal {
        nudge: Nudge {
            aggression: NEMESIS_AGG,
            smoothness: -NEMESIS_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Contra a ex-equipe: trocou de time NESTA virada de temporada e tinha OUTRO time antes
/// → algo a provar pra quem o deixou ir. ADVERSO (chip no ombro, pode sobre-pilotar).
/// Distinto da lua de mel (que todo recém-chegado sente, favorável): aqui é a rivalidade
/// com o passado.
pub fn former_team_grudge(switched_teams: bool) -> Signal {
    if !switched_teams {
        return Signal::default();
    }
    Signal {
        nudge: Nudge {
            aggression: FORMER_TEAM_AGG,
            optimism: FORMER_TEAM_OPT,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Campeão reinante: venceu o título da categoria na temporada passada → alvo nas costas
/// + expectativa. Resiliente → autoridade serena (favorável, calmo/confiante); frágil →
/// peso de defender (ADVERSO, tenso/bruto).
pub fn reigning_champion(is_champion: bool, resilience: f64) -> Signal {
    if !is_champion {
        return Signal::default();
    }
    let dir = (CHAMP_NEUTRAL - resilience) * 2.0; // -1 autoridade .. +1 peso
    if dir <= 0.0 {
        fav(Nudge {
            optimism: -dir * CHAMP_OPT,
            smoothness: -dir * CHAMP_SMO,
            ..Default::default()
        })
    } else {
        Signal {
            nudge: Nudge {
                aggression: dir * CHAMP_AGG,
                smoothness: -dir * CHAMP_SMO,
                ..Default::default()
            },
            adverse: true,
        }
    }
}

/// Estreia absoluta: a PRIMEIRA corrida da carreira → nervo cru, tateando (recolhido).
/// ADVERSO (mental forte segura melhor o frio na barriga). Distinto de idade jovem (traço
/// permanente) e lua de mel (chegou no time, não na carreira).
pub fn career_debut(is_debut: bool) -> Signal {
    if !is_debut {
        return Signal::default();
    }
    Signal {
        nudge: Nudge {
            aggression: -DEBUT_AGG,
            optimism: -DEBUT_OPT,
            smoothness: DEBUT_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Desconfiança mecânica: o carro quebrou (Mechanical/Operational) nas últimas corridas →
/// passa a poupar a máquina, mais cauteloso. ADVERSO. Contraste proposital com o azar
/// (`bad_luck`, que é frustração → agressivo): aqui a resposta é recolher, não atacar.
pub fn mechanical_distrust(mech_dnfs: u32) -> Signal {
    if mech_dnfs == 0 {
        return Signal::default();
    }
    let k = mech_dnfs.min(3) as f64 / 3.0;
    Signal {
        nudge: Nudge {
            aggression: -k * MECHDIST_AGG,
            smoothness: k * MECHDIST_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Pista-fantasma: já correu bastante aqui (≥3 largadas) mas o melhor resultado ainda é
/// ruim (pior que ~60% do grid) e não é trauma de batida → a pista simplesmente não
/// combina, leve resignação. ADVERSO. Não colide com `track_affinity` (cujo "domínio"
/// agora exige um resultado decente, não só experiência).
pub fn bogey_track(k: &TrackKnowledge, field_size: u32) -> Signal {
    if k.starts < BOGEY_MIN_STARTS || field_size <= 1 {
        return Signal::default();
    }
    let Some(best) = k.best_finish else {
        return Signal::default();
    };
    if best <= 3 {
        return Signal::default(); // pódio aqui = não é bogey
    }
    let depth = (best as f64 - 1.0) / (field_size as f64 - 1.0); // 0 frente .. 1 fundo
    if depth < BOGEY_DEPTH {
        return Signal::default();
    }
    let severity = ((depth - BOGEY_DEPTH) / (1.0 - BOGEY_DEPTH)).clamp(0.0, 1.0);
    Signal {
        nudge: Nudge {
            aggression: -severity * BOGEY_AGG,
            optimism: -severity * BOGEY_OPT,
            smoothness: severity * BOGEY_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// "Humor do dia" — ruído pequeno determinístico por (evento, piloto). Sempre vale.
pub fn wobble(seed: u64) -> Signal {
    let r = splitmix(seed);
    let pick = |shift: u32| (((r >> shift) & 0xff) as f64 / 255.0 - 0.5) * 2.0 * WOBBLE;
    fav(Nudge {
        aggression: pick(0),
        optimism: pick(8),
        smoothness: pick(16),
        skill: 0.0,
    })
}

/// Sorteio determinístico [0,1) do seed (descorrelacionado do wobble).
fn composure_roll(seed: u64) -> f64 {
    (splitmix(seed ^ 0xA5A5_A5A5_A5A5_A5A5) >> 11) as f64 / (1u64 << 53) as f64
}

// --- Composição -----------------------------------------------------------------

/// Atributos finais do piloto pra esta corrida (0–100).
#[derive(Clone, Copy, Debug)]
pub struct BehaviorOutput {
    pub aggression: f64,
    pub optimism: f64,
    pub smoothness: f64,
    pub skill: f64,
}

/// Soma os nudges a partir da base, escala pela maleabilidade e clampa 0–100.
pub fn compose(
    base_aggression: f64,
    base_optimism: f64,
    base_smoothness: f64,
    base_skill: f64,
    mentality: f64,
    nudges: &[Nudge],
) -> BehaviorOutput {
    let gain = malleability(mentality);
    let sum = nudges.iter().copied().fold(Nudge::default(), Nudge::add);
    // Headroom: o nudge de pace (essencialmente da pressão) vira pontos de skill
    // conforme onde o piloto está na curva — subir tem teto, cair tem chão. Mesma
    // curva da sim (simulation/pressure.rs).
    let skill_nudge = sum.skill * gain;
    let hr = pressure::headroom_pace_mult(base_skill, skill_nudge >= 0.0);
    BehaviorOutput {
        aggression: (base_aggression + sum.aggression * gain).clamp(0.0, 100.0),
        optimism: (base_optimism + sum.optimism * gain).clamp(0.0, 100.0),
        smoothness: (base_smoothness + sum.smoothness * gain).clamp(0.0, 100.0),
        // Pace: a base já entra com a penalidade de pista; aqui só o nudge, com headroom.
        skill: (base_skill + skill_nudge * hr).clamp(0.0, 100.0),
    }
}

/// Insumos completos (o comando preenche do banco; o módulo é puro).
pub struct BehaviorInputs {
    pub base_aggression: f64,
    pub base_optimism: f64,
    pub base_smoothness: f64,
    /// Pace base JÁ com a penalidade de conhecimento de pista aplicada.
    pub base_skill: f64,
    pub mentality: f64,
    pub resilience: f64,
    pub title: TitleContext,
    pub races_left: u32,
    /// Interesse "de local" do evento (0..1) — pressão de casa cheia (universal).
    pub event_stakes: f64,
    pub recent_positions: Vec<u32>,
    pub field_size: u32,
    /// Total de corridas da temporada (p/ desgaste de fim de temporada).
    pub season_length: u32,
    pub track: TrackKnowledge,
    pub is_wet: bool,
    pub fator_chuva: f64,
    pub rain_intensity: f64,
    pub temp_c: f64,
    pub age: u32,
    /// Percentil no ranking mundial (0–1, 1 = topo).
    pub global_rank_percentile: f64,
    /// Percentil de skill DENTRO do grid atual (0–1, 1 = melhor do grid).
    pub grid_rank_percentile: f64,
    pub home_race: bool,
    // Tier 2 Batch B.
    pub career_wins: u32,
    pub season_points: f64,
    pub contract_last_year: bool,
    pub teammate_points: Option<f64>,
    /// +1 promovido (subiu de categoria), -1 rebaixado (caiu), 0 nada.
    pub category_move: i32,
    /// Multiplicador de moral do time (~0.5 infeliz … 1.5 feliz; 1.0 neutro).
    pub team_morale: f64,
    /// Pontos de TODOS da categoria (p/ briga por posição/grana no fim).
    pub all_points: Vec<f64>,
    /// Pontos do vencedor (P1 + volta rápida) — alcance por corrida.
    pub max_points: f64,
    /// Voltou de lesão há poucas corridas.
    pub injury_return: bool,
    // Tier 3.
    pub honeymoon: bool,
    pub crashed_out_last_race: bool,
    pub not_at_fault_dnfs: u32,
    pub track_crash: bool,
    // Lote novo.
    /// Cruzou a linha lado a lado com o mesmo rival em ≥2 das últimas corridas.
    pub nemesis: bool,
    /// Trocou de equipe nesta virada de temporada (tinha outro time antes).
    pub switched_teams: bool,
    /// Campeão da categoria na temporada passada.
    pub reigning_champion: bool,
    /// Primeira corrida da carreira.
    pub career_debut: bool,
    /// DNFs mecânicos (Mechanical/Operational) nas últimas corridas.
    pub mechanical_dnfs: u32,
    pub seed: u64,
}

/// Entrada única do export: monta os sinais do Tier 1, blinda o adverso se o piloto
/// passar no teste de compostura, e compõe.
pub fn compute(i: &BehaviorInputs) -> BehaviorOutput {
    let amult = adverse_multiplier(i.mentality, i.seed);
    let signals = [
        pressure_title(&i.title, i.races_left, i.resilience),
        pressure_event(i.event_stakes, &i.recent_positions, i.field_size, i.resilience),
        form(&i.recent_positions, i.field_size, i.resilience),
        track_affinity(&i.track),
        weather(i.is_wet, i.fator_chuva, i.rain_intensity),
        heat(i.temp_c),
        age_phase(i.age),
        status(i.global_rank_percentile, i.grid_rank_percentile),
        home_race(i.home_race),
        win_streak(&i.recent_positions),
        near_miss(&i.recent_positions),
        end_season_fatigue(i.races_left, i.season_length),
        rising_prodigy(i.age, &i.recent_positions, i.field_size),
        milestone_chase(i.career_wins),
        contract_year(i.contract_last_year, i.resilience),
        teammate_duel(i.season_points, i.teammate_points),
        category_move(i.category_move),
        team_morale(i.team_morale),
        prize_fight(
            i.season_points,
            &i.all_points,
            i.races_left,
            i.max_points,
            i.title.in_contention,
        ),
        injury_return(i.injury_return),
        honeymoon(i.honeymoon),
        revenge(i.crashed_out_last_race),
        bad_luck(i.not_at_fault_dnfs),
        track_trauma(i.track_crash),
        nemesis(i.nemesis),
        former_team_grudge(i.switched_teams),
        reigning_champion(i.reigning_champion, i.resilience),
        career_debut(i.career_debut),
        mechanical_distrust(i.mechanical_dnfs),
        bogey_track(&i.track, i.field_size),
        wobble(i.seed),
    ];
    let nudges: Vec<Nudge> = signals
        .iter()
        .map(|s| {
            if s.adverse {
                s.nudge.scale(amult)
            } else {
                s.nudge
            }
        })
        .collect();
    compose(
        i.base_aggression,
        i.base_optimism,
        i.base_smoothness,
        i.base_skill,
        i.mentality,
        &nudges,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tc(in_contention: bool, is_leader: bool, decided: bool) -> TitleContext {
        TitleContext {
            in_contention,
            is_leader,
            title_decided: decided,
        }
    }

    fn neutral_inputs() -> BehaviorInputs {
        BehaviorInputs {
            base_aggression: 50.0,
            base_optimism: 50.0,
            base_smoothness: 50.0,
            base_skill: 60.0,
            mentality: 50.0,
            resilience: 0.5,
            title: tc(false, false, false), // fora de briga → sem pressão
            races_left: 8,
            event_stakes: 0.0, // sem casa cheia por padrão nos testes neutros
            recent_positions: Vec::new(),
            field_size: 20,
            season_length: 10,
            track: TrackKnowledge {
                starts: 5,
                best_finish: Some(6),
                last_season: Some(1),
            },
            is_wet: false,
            fator_chuva: 50.0,
            rain_intensity: 0.0,
            temp_c: 20.0,
            age: 27,
            global_rank_percentile: 0.5,
            grid_rank_percentile: 0.5,
            home_race: false,
            career_wins: 3,
            season_points: 50.0,
            contract_last_year: false,
            teammate_points: None,
            category_move: 0,
            team_morale: 1.0,
            all_points: vec![10.0, 20.0, 30.0],
            max_points: 26.0,
            injury_return: false,
            honeymoon: false,
            crashed_out_last_race: false,
            not_at_fault_dnfs: 0,
            track_crash: false,
            nemesis: false,
            switched_teams: false,
            reigning_champion: false,
            career_debut: false,
            mechanical_dnfs: 0,
            seed: 1,
        }
    }

    #[test]
    fn mentalidade_e_o_ganho() {
        assert!(malleability(100.0) < malleability(0.0));
        assert!((malleability(50.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn pressao_choke_vs_clutch() {
        let title = tc(true, false, false);
        let choke = pressure_title(&title, 1, 0.15); // frágil
        let clutch = pressure_title(&title, 1, 0.9); // resiliente
        assert!(choke.nudge.aggression > 0.0 && choke.nudge.smoothness < 0.0 && choke.adverse);
        assert!(clutch.nudge.aggression < 0.0 && clutch.nudge.smoothness > 0.0 && !clutch.adverse);
    }

    #[test]
    fn pace_export_respeita_headroom() {
        let title = tc(true, false, false);
        // Choke forte (frágil, última corrida): craque (skill alto, espaço p/ cair)
        // desaba MAIS que o azarão (perto do chão, quase não sente).
        let choke = pressure_title(&title, 1, 0.05).nudge;
        let drop_craque = 90.0 - compose(50.0, 50.0, 50.0, 90.0, 0.0, &[choke]).skill;
        let drop_azarao = 55.0 - compose(50.0, 50.0, 50.0, 55.0, 0.0, &[choke]).skill;
        assert!(
            drop_craque > drop_azarao,
            "craque desaba mais: {drop_craque} vs {drop_azarao}"
        );
        assert!(drop_craque <= 10.0, "não pode explodir: {drop_craque}");
        // Clutch (resiliente): azarão sobe MAIS que o craque (espaço até o teto).
        let clutch = pressure_title(&title, 1, 0.95).nudge;
        let up_craque = compose(50.0, 50.0, 50.0, 90.0, 0.0, &[clutch]).skill - 90.0;
        let up_azarao = compose(50.0, 50.0, 50.0, 55.0, 0.0, &[clutch]).skill - 55.0;
        assert!(
            up_azarao > up_craque,
            "azarão salta mais: {up_azarao} vs {up_craque}"
        );
    }

    #[test]
    fn casa_cheia_export_universal_e_adversa_no_fragil() {
        // Frágil sob casa cheia (sem briga de título) → choke: mais agressivo, adverso.
        let fragil = pressure_event(1.0, &[18, 20, 19], 24, 0.20);
        assert!(fragil.nudge.aggression > 0.0 && fragil.nudge.smoothness < 0.0 && fragil.adverse);
        // Resiliente no mesmo palco → clutch: mais suave, não adverso.
        let firme = pressure_event(1.0, &[18, 20, 19], 24, 0.85);
        assert!(firme.nudge.aggression < 0.0 && firme.nudge.smoothness > 0.0 && !firme.adverse);
        // Evento sem público → nada.
        assert!(pressure_event(0.0, &[10], 24, 0.5).nudge.aggression == 0.0);
    }

    #[test]
    fn cruzeiro_relaxa() {
        let s = pressure_title(&tc(true, true, true), 1, 0.5);
        assert!(s.nudge.aggression < 0.0 && s.nudge.smoothness > 0.0 && !s.adverse);
    }

    #[test]
    fn forma_alta_vs_seca() {
        let hot = form(&[1, 2, 1], 20, 0.5); // pódios
        let cold = form(&[18, 19, 20], 20, 0.15); // fundo + frágil
        assert!(hot.nudge.optimism > 0.0 && hot.nudge.aggression > 0.0 && !hot.adverse);
        assert!(cold.nudge.optimism < 0.0 && cold.nudge.smoothness < 0.0 && cold.adverse);
        assert!(cold.nudge.aggression > 0.0, "seca → desespero agressivo");
    }

    #[test]
    fn pista_nova_cautelosa_dominio_ataca() {
        let nova = track_affinity(&TrackKnowledge {
            starts: 0,
            best_finish: None,
            last_season: None,
        });
        let dom = track_affinity(&TrackKnowledge {
            starts: 5,
            best_finish: Some(2),
            last_season: Some(3),
        });
        assert!(nova.nudge.aggression < 0.0 && nova.nudge.smoothness > 0.0 && nova.adverse);
        assert!(dom.nudge.aggression > 0.0 && dom.nudge.optimism > 0.0 && !dom.adverse);
    }

    #[test]
    fn chuva_teme_vs_mestre() {
        let teme = weather(true, 10.0, 1.0);
        let mestre = weather(true, 95.0, 1.0);
        assert!(teme.nudge.aggression < 0.0 && teme.nudge.smoothness > 0.0 && teme.adverse);
        assert!(mestre.nudge.aggression > 0.0 && mestre.nudge.smoothness < 0.0 && !mestre.adverse);
        assert_eq!(weather(false, 10.0, 1.0).nudge.aggression, 0.0); // seco = nada
    }

    #[test]
    fn jovem_vs_veterano() {
        let jovem = age_phase(18);
        let vet = age_phase(38);
        assert!(jovem.nudge.aggression > 0.0 && jovem.nudge.smoothness < 0.0);
        assert!(vet.nudge.aggression < 0.0 && vet.nudge.smoothness > 0.0);
        assert_eq!(age_phase(27).nudge.aggression, 0.0); // auge = neutro
    }

    #[test]
    fn status_grid_pesa_mais_que_fama() {
        let alfa_local = status(0.5, 1.0);
        let craque_anonimo = status(1.0, 0.4);
        assert!(alfa_local.nudge.optimism > craque_anonimo.nudge.optimism);
        assert!(status(0.1, 0.1).nudge.optimism < 0.0 && status(0.1, 0.1).adverse);
    }

    #[test]
    fn stack_cauteloso_vira_a_mao_do_agressivo() {
        let cautelosos = [
            track_affinity(&TrackKnowledge {
                starts: 0,
                best_finish: None,
                last_season: None,
            })
            .nudge,
            weather(true, 5.0, 1.0).nudge,
            age_phase(39).nudge,
            heat(38.0).nudge,
        ];
        let fraco = compose(92.0, 50.0, 50.0, 70.0, 10.0, &cautelosos);
        let forte = compose(92.0, 50.0, 50.0, 70.0, 95.0, &cautelosos);
        assert!(
            fraco.aggression < 60.0,
            "fraco devia cair muito: {}",
            fraco.aggression
        );
        assert!(forte.aggression > fraco.aggression, "forte resiste mais");
    }

    #[test]
    fn wobble_deterministico() {
        assert_eq!(wobble(42).nudge.aggression, wobble(42).nudge.aggression);
        assert!(wobble(1).nudge.aggression != wobble(2).nudge.aggression);
    }

    #[test]
    fn tier2_sinais() {
        // Streak: 3 vitórias seguidas → swagger; 1 só → nada.
        assert!(win_streak(&[1, 1, 1]).nudge.optimism > 0.0);
        assert_eq!(win_streak(&[1, 4, 1]).nudge.optimism, 0.0);
        // Quase lá: pódios sem vitória → coceira de risco; com vitória → nada.
        assert!(near_miss(&[2, 3, 2]).nudge.aggression > 0.0);
        assert_eq!(near_miss(&[1, 3, 2]).nudge.aggression, 0.0);
        // Desgaste: fim de temporada longa = adverso; meio de temporada = nada.
        let f = end_season_fatigue(1, 14);
        assert!(f.nudge.optimism < 0.0 && f.adverse);
        assert_eq!(end_season_fatigue(8, 14).nudge.optimism, 0.0);
        // Prodígio: jovem indo bem → confiança; jovem indo mal ou veterano → nada.
        assert!(rising_prodigy(18, &[1, 2, 1], 20).nudge.optimism > 0.0);
        assert_eq!(rising_prodigy(18, &[18, 19], 20).nudge.optimism, 0.0);
        assert_eq!(rising_prodigy(30, &[1, 1], 20).nudge.optimism, 0.0);
    }

    #[test]
    fn tier2b_sinais() {
        // Caça a marco: 99→100ª = fogo; 12→13 = nada.
        assert!(milestone_chase(99).nudge.aggression > 0.0);
        assert_eq!(milestone_chase(12).nudge.aggression, 0.0);
        // Contrato último ano: showboat + adverso; frágil fica mais bruto que resiliente.
        let frag = contract_year(true, 0.1);
        let res = contract_year(true, 0.9);
        assert!(frag.nudge.aggression > 0.0 && frag.adverse);
        assert!(frag.nudge.smoothness < res.nudge.smoothness);
        assert_eq!(contract_year(false, 0.5).nudge.aggression, 0.0);
        // Duelo interno: apanhando do companheiro → agressivo; à frente → nada.
        assert!(teammate_duel(30.0, Some(80.0)).nudge.aggression > 0.0);
        assert_eq!(teammate_duel(80.0, Some(30.0)).nudge.aggression, 0.0);
        assert_eq!(teammate_duel(50.0, None).nudge.aggression, 0.0);
        // Categoria: promovido = cauteloso/adverso; rebaixado = swagger.
        assert!(category_move(1).nudge.smoothness > 0.0 && category_move(1).adverse);
        assert!(category_move(-1).nudge.aggression > 0.0 && !category_move(-1).adverse);
        assert_eq!(category_move(0).nudge.aggression, 0.0);
        // Moral: feliz → otimismo/suave; infeliz → frustrado/adverso.
        assert!(team_morale(1.4).nudge.optimism > 0.0 && !team_morale(1.4).adverse);
        let unhappy = team_morale(0.6);
        assert!(
            unhappy.nudge.aggression > 0.0 && unhappy.nudge.smoothness < 0.0 && unhappy.adverse
        );
    }

    #[test]
    fn tier3_sinais() {
        // Lua de mel: chegou agora → afoito (agressivo, menos suave).
        let hm = honeymoon(true);
        assert!(hm.nudge.aggression > 0.0 && hm.nudge.smoothness < 0.0 && !hm.adverse);
        assert_eq!(honeymoon(false).nudge.aggression, 0.0);
        // Vingança: tirado na última → furioso, adverso.
        assert!(revenge(true).nudge.aggression > 0.0 && revenge(true).adverse);
        assert_eq!(revenge(false).nudge.aggression, 0.0);
        // Azar: 2+ DNFs sem culpa → frustrado/adverso; 1 só = nada.
        assert!(bad_luck(3).nudge.aggression > 0.0 && bad_luck(3).nudge.smoothness < 0.0);
        assert!(bad_luck(3).adverse && bad_luck(3).nudge.aggression > bad_luck(2).nudge.aggression);
        assert_eq!(bad_luck(1).nudge.aggression, 0.0);
        // Trauma de pista: bateu aqui antes → cauteloso/adverso.
        let tr = track_trauma(true);
        assert!(tr.nudge.aggression < 0.0 && tr.nudge.smoothness > 0.0 && tr.adverse);
        assert_eq!(track_trauma(false).nudge.smoothness, 0.0);
    }

    #[test]
    fn lote_novo_sinais() {
        // Nêmesis: rivalidade → agressivo e menos suave, adverso; sem rival = nada.
        let nem = nemesis(true);
        assert!(nem.nudge.aggression > 0.0 && nem.nudge.smoothness < 0.0 && nem.adverse);
        assert_eq!(nemesis(false).nudge.aggression, 0.0);
        // Ex-equipe: algo a provar → agressivo/otimista, adverso; sem troca = nada.
        let ex = former_team_grudge(true);
        assert!(ex.nudge.aggression > 0.0 && ex.nudge.optimism > 0.0 && ex.adverse);
        assert_eq!(former_team_grudge(false).nudge.aggression, 0.0);
        // Campeão reinante: resiliente = autoridade serena (favorável); frágil = peso
        // tenso (adverso). Não-campeão = nada.
        let autoridade = reigning_champion(true, 0.95);
        let peso = reigning_champion(true, 0.05);
        assert!(autoridade.nudge.optimism > 0.0 && autoridade.nudge.smoothness > 0.0 && !autoridade.adverse);
        assert!(peso.nudge.aggression > 0.0 && peso.nudge.smoothness < 0.0 && peso.adverse);
        assert_eq!(reigning_champion(false, 0.5).nudge.optimism, 0.0);
        // Estreia: nervo → recolhido (agg↓ opt↓ suav↑), adverso; veterano = nada.
        let deb = career_debut(true);
        assert!(deb.nudge.aggression < 0.0 && deb.nudge.optimism < 0.0 && deb.nudge.smoothness > 0.0 && deb.adverse);
        assert_eq!(career_debut(false).nudge.smoothness, 0.0);
        // Desconfiança mecânica: poupa o carro (agg↓ suav↑), adverso, escala com DNFs;
        // 0 = nada. Contraste com bad_luck (agg↑).
        let md = mechanical_distrust(3);
        assert!(md.nudge.aggression < 0.0 && md.nudge.smoothness > 0.0 && md.adverse);
        assert!(mechanical_distrust(3).nudge.smoothness > mechanical_distrust(1).nudge.smoothness);
        assert_eq!(mechanical_distrust(0).nudge.aggression, 0.0);
        assert!(mechanical_distrust(2).nudge.aggression < 0.0 && bad_luck(2).nudge.aggression > 0.0);
        // Pista-fantasma: experiente mas ruim aqui (best P15 em 20) → resignação, adverso.
        let bogey = bogey_track(&TrackKnowledge { starts: 5, best_finish: Some(15), last_season: Some(2) }, 20);
        assert!(bogey.nudge.aggression < 0.0 && bogey.nudge.optimism < 0.0 && bogey.nudge.smoothness > 0.0 && bogey.adverse);
        // Não é bogey: pouca experiência, ou pódio aqui, ou resultado decente.
        assert_eq!(bogey_track(&TrackKnowledge { starts: 2, best_finish: Some(18), last_season: None }, 20).nudge.aggression, 0.0);
        assert_eq!(bogey_track(&TrackKnowledge { starts: 6, best_finish: Some(2), last_season: None }, 20).nudge.aggression, 0.0);
        assert_eq!(bogey_track(&TrackKnowledge { starts: 6, best_finish: Some(6), last_season: None }, 20).nudge.aggression, 0.0);
        // E o experiente-mas-ruim NÃO é mais tratado como domínio pelo track_affinity.
        let aff = track_affinity(&TrackKnowledge { starts: 6, best_finish: Some(15), last_season: Some(2) });
        assert_eq!(aff.nudge.aggression, 0.0, "experiência sem resultado ≠ domínio");
    }

    #[test]
    fn prize_e_lesao() {
        // Briga por grana: fim de temporada, fora do título, rival colado → tensão.
        let pf = prize_fight(40.0, &[100.0, 42.0, 40.0, 10.0], 2, 26.0, false);
        assert!(pf.nudge.aggression > 0.0 && pf.adverse);
        // Em briga de título não conta (a pressão de título cobre).
        assert_eq!(
            prize_fight(40.0, &[42.0, 40.0], 2, 26.0, true)
                .nudge
                .aggression,
            0.0
        );
        // Longe do fim não conta.
        assert_eq!(
            prize_fight(40.0, &[42.0, 40.0], 8, 26.0, false)
                .nudge
                .aggression,
            0.0
        );
        // Retorno de lesão: cauteloso + adverso.
        let inj = injury_return(true);
        assert!(inj.nudge.aggression < 0.0 && inj.nudge.smoothness > 0.0 && inj.adverse);
        assert_eq!(injury_return(false).nudge.smoothness, 0.0);
    }

    #[test]
    fn impacto_adverso_granular() {
        // Fraco SEMPRE leva o adverso cheio (1.0). Quanto mais forte, MENOR o impacto
        // adverso médio (granular, sem extremos binários).
        for s in 0..80 {
            assert_eq!(
                adverse_multiplier(0.0, s),
                1.0,
                "fraco leva cheio (seed {s})"
            );
        }
        let avg = |m: f64| (0..500u64).map(|s| adverse_multiplier(m, s)).sum::<f64>() / 500.0;
        assert!((avg(0.0) - 1.0).abs() < 1e-9);
        assert!(avg(100.0) < avg(70.0));
        assert!(avg(70.0) < avg(40.0));
        assert!(avg(40.0) < avg(0.0));
    }

    #[test]
    fn fraco_sempre_afetado_forte_resiste_mais() {
        // Input adverso (pista nova + medo de chuva → agressividade despenca).
        let aggression_for = |mentality: f64, seed: u64| {
            let mut i = neutral_inputs();
            i.mentality = mentality;
            i.seed = seed;
            i.track = TrackKnowledge {
                starts: 0,
                best_finish: None,
                last_season: None,
            };
            i.is_wet = true;
            i.fator_chuva = 5.0;
            i.rain_intensity = 1.0;
            compute(&i).aggression
        };
        let max_fraco = (0..300u64)
            .map(|s| aggression_for(0.0, s))
            .fold(f64::MIN, f64::max);
        let max_forte = (0..300u64)
            .map(|s| aggression_for(100.0, s))
            .fold(f64::MIN, f64::max);
        // Fraco nunca blinda → mesmo no melhor dia fica bem abaixo da base (50).
        assert!(max_fraco < 42.0, "fraco max {max_fraco}");
        // Forte, num bom dia, reduz muito o adverso → chega perto da base.
        assert!(max_forte > 45.0, "forte max {max_forte}");
    }

    // ─── HARNESS DE CALIBRAÇÃO ───────────────────────────────────────────────────
    // Não valida nada: IMPRIME tabelas pra calibrar as magnitudes vendo números.
    //   cargo test --lib behavior::tests::calibracao -- --nocapture --test-threads=1
    // (rode com CARGO_TARGET_DIR fora do OneDrive)

    fn row(label: &str, s: Signal) {
        let n = s.nudge;
        println!(
            "{:<26} agg {:>6.1}  opt {:>6.1}  suav {:>6.1}  pace {:>5.1}   {}",
            label,
            n.aggression,
            n.optimism,
            n.smoothness,
            n.skill,
            if s.adverse { "ADVERSO" } else { "favor." }
        );
    }

    /// TABELA A: peso CRU de cada sinal (antes de ganho×mentalidade e da blindagem do
    /// adverso). É o "quanto cada lever vale" — a referência principal pra calibrar.
    #[test]
    fn calibracao_magnitudes() {
        println!("\n=== TABELA A · magnitude CRUA por sinal (gain=1, sem blindagem) ===");
        let title = tc(true, false, false);
        row("pressao_titulo choke", pressure_title(&title, 1, 0.15));
        row("pressao_titulo clutch", pressure_title(&title, 1, 0.90));
        row("cruzeiro (titulo ganho)", pressure_title(&tc(true, true, true), 1, 0.5));
        row("casa_cheia choke", pressure_event(1.0, &[18, 20, 19], 24, 0.15));
        row("casa_cheia clutch", pressure_event(1.0, &[18, 20, 19], 24, 0.90));
        row("forma_alta", form(&[1, 2, 1], 20, 0.5));
        row("forma_seca (fragil)", form(&[18, 19, 20], 20, 0.15));
        row(
            "pista_nova",
            track_affinity(&TrackKnowledge { starts: 0, best_finish: None, last_season: None }),
        );
        row(
            "pista_dominio",
            track_affinity(&TrackKnowledge { starts: 5, best_finish: Some(2), last_season: Some(1) }),
        );
        row("chuva_teme", weather(true, 10.0, 1.0));
        row("chuva_mestre", weather(true, 95.0, 1.0));
        row("calor_extremo (38C)", heat(38.0));
        row("jovem (18)", age_phase(18));
        row("veterano (38)", age_phase(38));
        row("status_alto (alfa grid)", status(0.5, 1.0));
        row("status_baixo (fundo)", status(0.1, 0.1));
        row("casa (pais natal)", home_race(true));
        row("win_streak (3)", win_streak(&[1, 1, 1]));
        row("near_miss (podios)", near_miss(&[2, 3, 2]));
        row("fim_temporada_fadiga", end_season_fatigue(1, 14));
        row("prodigio_ascensao", rising_prodigy(18, &[1, 2, 1], 20));
        row("caca_marco (99->100)", milestone_chase(99));
        row("contrato_ult_ano (frag)", contract_year(true, 0.15));
        row("duelo_companheiro", teammate_duel(30.0, Some(80.0)));
        row("promovido", category_move(1));
        row("rebaixado", category_move(-1));
        row("moral_feliz", team_morale(1.4));
        row("moral_infeliz", team_morale(0.6));
        row(
            "briga_grana (prize)",
            prize_fight(40.0, &[100.0, 42.0, 40.0, 10.0], 2, 26.0, false),
        );
        row("retorno_lesao", injury_return(true));
        row("lua_de_mel", honeymoon(true));
        row("vinganca", revenge(true));
        row("azar (3 dnf)", bad_luck(3));
        row("trauma_pista", track_trauma(true));
        row("NEMESIS", nemesis(true));
        row("EX-EQUIPE", former_team_grudge(true));
        row("CAMPEAO autoridade", reigning_champion(true, 0.90));
        row("CAMPEAO peso (fragil)", reigning_champion(true, 0.15));
        row("ESTREIA", career_debut(true));
        row("DESCONFIANCA_MEC (3)", mechanical_distrust(3));
        row(
            "PISTA-FANTASMA (P15/20)",
            bogey_track(&TrackKnowledge { starts: 5, best_finish: Some(15), last_season: Some(2) }, 20),
        );
        row("wobble (humor, seed 7)", wobble(7));
        println!("(crus; no jogo cada um leva ×gain[0.6..1.4] e o ADVERSO ainda é blindado pela compostura)\n");
    }

    /// Roda 200 "dias" (seeds) e imprime a média final vs base — o adverso não depende
    /// mais do sorteio do dia.
    fn profile(label: &str, mut base: BehaviorInputs) {
        let n = 200u64;
        let (ba, bo, bs, bk) = (
            base.base_aggression,
            base.base_optimism,
            base.base_smoothness,
            base.base_skill,
        );
        let (mut a, mut o, mut s, mut k) = (0.0, 0.0, 0.0, 0.0);
        for seed in 0..n {
            base.seed = seed;
            let out = compute(&base);
            a += out.aggression;
            o += out.optimism;
            s += out.smoothness;
            k += out.skill;
        }
        let f = n as f64;
        println!(
            "{:<34} agg {:>5.1} (b{:>3.0})  opt {:>5.1} (b{:>3.0})  suav {:>5.1} (b{:>3.0})  pace {:>5.1} (b{:>3.0})",
            label, a / f, ba, o / f, bo, s / f, bs, k / f, bk,
        );
    }

    /// TABELA B: perfis realistas end-to-end. Mostra onde os stacks aterrissam e como a
    /// mentalidade blinda o adverso.
    #[test]
    fn calibracao_perfis() {
        println!("\n=== TABELA B · perfis realistas (média de 200 dias) ===");

        // 1. Rookie estreante, pista nova, chuva forte, mental fraco.
        let mut p = neutral_inputs();
        p.base_aggression = 55.0;
        p.base_optimism = 50.0;
        p.base_smoothness = 48.0;
        p.base_skill = 45.0;
        p.mentality = 30.0;
        p.resilience = pressure::pressure_resilience(30.0, 10.0);
        p.age = 18;
        p.track = TrackKnowledge { starts: 0, best_finish: None, last_season: None };
        p.is_wet = true;
        p.fator_chuva = 20.0;
        p.rain_intensity = 1.0;
        p.career_debut = true;
        profile("1. rookie estreante·pista nova·chuva", p);

        // 2. Campeão reinante frágil, defendendo, última corrida, título em jogo.
        let mut p = neutral_inputs();
        p.base_aggression = 52.0;
        p.base_optimism = 60.0;
        p.base_smoothness = 55.0;
        p.base_skill = 88.0;
        p.mentality = 35.0;
        p.resilience = pressure::pressure_resilience(35.0, 60.0);
        p.title = tc(true, false, false);
        p.races_left = 1;
        p.reigning_champion = true;
        profile("2. campeao reinante fragil·defesa", p);

        // 3. Veterano, fim de temporada, azar mecânico, moral baixa.
        let mut p = neutral_inputs();
        p.base_aggression = 48.0;
        p.base_optimism = 50.0;
        p.base_smoothness = 62.0;
        p.base_skill = 72.0;
        p.mentality = 55.0;
        p.resilience = pressure::pressure_resilience(55.0, 90.0);
        p.age = 38;
        p.races_left = 1;
        p.season_length = 14;
        p.mechanical_dnfs = 2;
        p.team_morale = 0.7;
        profile("3. veterano·fim season·azar mec", p);

        // 4. Jovem prodígio em alta, casa, win streak, mental forte.
        let mut p = neutral_inputs();
        p.base_aggression = 60.0;
        p.base_optimism = 58.0;
        p.base_smoothness = 50.0;
        p.base_skill = 78.0;
        p.mentality = 85.0;
        p.resilience = pressure::pressure_resilience(85.0, 40.0);
        p.age = 19;
        p.recent_positions = vec![1, 1, 1];
        p.home_race = true;
        profile("4. jovem prodigio·alta·casa", p);

        // 5. Rivalidade carregada: nêmesis + ex-equipe + trauma de pista.
        let mut p = neutral_inputs();
        p.base_aggression = 58.0;
        p.base_optimism = 52.0;
        p.base_smoothness = 54.0;
        p.base_skill = 74.0;
        p.mentality = 50.0;
        p.resilience = pressure::pressure_resilience(50.0, 50.0);
        p.nemesis = true;
        p.switched_teams = true;
        p.track_crash = true;
        profile("5. nemesis+ex-equipe+trauma", p);

        println!("\n--- combos EXTREMOS (tudo alinhado) ---");
        // 6. COMBO FAVORÁVEL MÁXIMO (mental fraco = gain 1.4 amplifica o favorável):
        // jovem prodígio, tri-vitória, casa, status topo, domínio de pista, mestre de
        // chuva, rebaixado (swagger), lua de mel, caça-marco, moral alta. Empurra
        // agg/opt pro TETO 100.
        let mut p = neutral_inputs();
        p.base_aggression = 55.0;
        p.base_optimism = 55.0;
        p.base_smoothness = 50.0;
        p.base_skill = 78.0;
        p.mentality = 20.0; // fraco → gain alto → favorável amplificado
        p.resilience = pressure::pressure_resilience(20.0, 20.0);
        p.age = 18;
        p.recent_positions = vec![1, 1, 1];
        p.field_size = 20;
        p.home_race = true;
        p.global_rank_percentile = 1.0;
        p.grid_rank_percentile = 1.0;
        p.track = TrackKnowledge { starts: 6, best_finish: Some(1), last_season: Some(1) };
        p.is_wet = true;
        p.fator_chuva = 95.0;
        p.rain_intensity = 1.0;
        p.category_move = -1; // rebaixado → swagger
        p.honeymoon = true;
        p.career_wins = 99; // próxima = marco 100
        p.team_morale = 1.5;
        profile("6. COMBO favoravel max (fraco)", p);

        // 7. COMBO ADVERSO MÁXIMO (mental fraco, tudo contra, sinais que RECOLHEM):
        // pista nova, teme a chuva, veterano, calor, retorno de lesão, promovido,
        // desconfiança mecânica, trauma de pista. Empurra agg/opt pro CHÃO 0 e suav pro
        // teto. (Sem choke/vingança/nêmesis, que SOBEM a agressividade.)
        let mut p = neutral_inputs();
        p.base_aggression = 45.0;
        p.base_optimism = 45.0;
        p.base_smoothness = 55.0;
        p.base_skill = 60.0;
        p.mentality = 5.0; // fraco → não blinda nada, gain máximo
        p.resilience = pressure::pressure_resilience(5.0, 5.0);
        p.age = 38; // veterano
        p.field_size = 20;
        p.track = TrackKnowledge { starts: 0, best_finish: None, last_season: None }; // nova
        p.is_wet = true;
        p.fator_chuva = 5.0; // teme a chuva
        p.rain_intensity = 1.0;
        p.temp_c = 39.0; // calor extremo
        p.injury_return = true;
        p.category_move = 1; // promovido → cautela
        p.mechanical_dnfs = 3;
        p.track_crash = true; // trauma
        profile("7. COMBO adverso max (fraco)", p);
        println!();
    }
}
