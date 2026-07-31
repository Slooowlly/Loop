//! Sinais do **Tier 3** (estado/derivado: contrato, DNFs recentes, crash na pista) e
//! do lote novo — rivalidade, história, carro e pista — mais fama e vínculo com a equipe.

use crate::simulation::track_knowledge::TrackKnowledge;

use super::tipos::{fav, Nudge, Signal};

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
                               // Fama/estrelato (a "2ª moeda") + vínculo estrutural com a equipe. Empurrões MODESTOS —
                               // nudge, não dominância: são status/contexto de carreira, não drama da corrida.
const STARDOM_OPT: f64 = 6.0;
const STARDOM_AGG: f64 = 4.0;
const BOND_OPT: f64 = 6.0;
const BOND_SMO: f64 = 6.0;

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

/// Fama/estrelato (a "2ª moeda", `midia`): a estrela carrega presença — mais confiante e com
/// um algo a mais de ousadia; o anônimo fica com um traço mais apagado. TRAÇO (não adverso):
/// é status de carreira, não uma adversidade da corrida que a compostura blinde. `fame` 0–100.
pub fn stardom(fame: f64) -> Signal {
    let s = ((fame.clamp(0.0, 100.0) - 50.0) / 50.0).clamp(-1.0, 1.0); // -1 anônimo .. 1 astro
    if s.abs() < 1e-9 {
        return Signal::default();
    }
    fav(Nudge {
        optimism: s * STARDOM_OPT,
        aggression: s.max(0.0) * STARDOM_AGG, // só a estrela ganha swagger; o anônimo não perde agg
        ..Default::default()
    })
}

/// Vínculo com a equipe (selo de 6 níveis; ver [`crate::market::bond`]): a "casa" (nível
/// alto) dá conforto → sereno e confiante; o recém-chegado (nível baixo) ainda se ajusta →
/// leve insegurança. Nível 3 = neutro. TRAÇO (não adverso): relação estrutural de longo
/// prazo, não um estado emocional da corrida.
pub fn team_bond(level: u8) -> Signal {
    let b = (level as f64 - 3.0) / 3.0; // 1→-0.67 .. 3→0 .. 6→+1.0
    if b.abs() < 1e-9 {
        return Signal::default();
    }
    fav(Nudge {
        optimism: b * BOND_OPT,
        smoothness: b.max(0.0) * BOND_SMO, // a casa acomoda (mais suave); vínculo baixo não fica bruto
        ..Default::default()
    })
}
