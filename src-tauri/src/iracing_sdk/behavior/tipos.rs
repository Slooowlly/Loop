//! Tipos da camada de comportamento: o empurrão de um sinal (`Nudge`), o sinal em
//! si (`Signal`), os insumos completos (`BehaviorInputs`) e a saída (`BehaviorOutput`).

use crate::simulation::pressure::TitleContext;
use crate::simulation::track_knowledge::TrackKnowledge;

/// Empurrões de UM sinal (pontos crus, antes do ganho). Skill quase sempre 0.
#[derive(Clone, Copy, Debug, Default)]
pub struct Nudge {
    pub aggression: f64,
    pub optimism: f64,
    pub smoothness: f64,
    pub skill: f64,
}

impl Nudge {
    pub(super) fn add(self, o: Nudge) -> Nudge {
        Nudge {
            aggression: self.aggression + o.aggression,
            optimism: self.optimism + o.optimism,
            smoothness: self.smoothness + o.smoothness,
            skill: self.skill + o.skill,
        }
    }

    pub(super) fn scale(self, k: f64) -> Nudge {
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

pub(super) fn fav(nudge: Nudge) -> Signal {
    Signal {
        nudge,
        adverse: false,
    }
}

/// Atributos finais do piloto pra esta corrida (0–100).
#[derive(Clone, Copy, Debug)]
pub struct BehaviorOutput {
    pub aggression: f64,
    pub optimism: f64,
    pub smoothness: f64,
    pub skill: f64,
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
    /// Fama/estrelato do piloto (0–100), a "2ª moeda" (`midia`).
    pub fame: f64,
    /// Nível do vínculo com a equipe (1–6; ver [`crate::market::bond::bond_level`]).
    pub bond_level: u8,
    /// Handicap de lesão ATIVA: fração do pace perdida por uma lesão em recuperação
    /// (`skill_penalty × corridas_restantes/total`, 0–1). 0 = sem lesão. MESMA rampa da sim
    /// (`commands/race.rs`) — antes só o bool `injury_return` (piloto já sarado) cruzava.
    pub injury_active_penalty: f64,
    pub seed: u64,
}
