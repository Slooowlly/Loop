//! Entidade de calendário (extraída de `calendar/mod.rs`).

use serde::{Deserialize, Serialize};

use crate::models::enums::{RaceStatus, SeasonPhase, ThematicSlot, WeatherCondition};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarEntry {
    pub id: String,
    pub season_id: String,
    pub categoria: String,
    pub rodada: i32,
    pub nome: String,
    pub track_id: u32,
    pub track_name: String,
    pub track_config: String,
    pub clima: WeatherCondition,
    pub temperatura: f64,
    pub voltas: i32,
    pub duracao_corrida_min: i32,
    pub duracao_classificacao_min: i32,
    pub status: RaceStatus,
    pub horario: String,
    /// Semana do ano (1–52) — unidade temporal interna do sistema.
    /// A ordenação e toda lógica temporal baseiam-se neste campo.
    pub week_of_year: i32,
    /// Fase da temporada em que o evento ocorre (BlocoRegular ou BlocoEspecial).
    pub season_phase: SeasonPhase,
    /// Data visual derivada de week_of_year — para UI, notícias e narrativa.
    /// Não é a base lógica do sistema; use season_week para ordenação 9D.
    pub display_date: String,
    /// Papel narrativo fixo desta corrida dentro da temporada.
    /// Determinado no momento da geração — imutável após persistência.
    /// `NaoClassificado` para saves pré-v12 ou caminho legado.
    pub thematic_slot: ThematicSlot,
    /// Posição monotônica na régua 9D (1–51). None para saves pré-v33.
    /// Adicionado à coluna DB na migração v33 (Etapa 3).
    #[serde(default)]
    pub season_week: Option<u32>,
}

/// Duração de sprint assumida quando nem a etapa nem a categoria sabem dizer quanto
/// dura a prova. Fica abaixo do gate de enduro de propósito: na dúvida, sprint.
const DURACAO_SPRINT_PADRAO_MIN: u16 = 30;

impl CalendarEntry {
    /// Duração REAL desta prova, em minutos, pronta para alimentar o gate de enduro
    /// ([`crate::car::breakdown::is_enduro_duration`]) e o desgaste da etapa.
    ///
    /// Existe porque `get_category_config(...).duracao_corrida_min` é uma SENTINELA no
    /// Endurance: lá a constante vale 0 e quem sorteia entre 120, 180, 240 e 360 minutos
    /// por etapa é `calendar::montagem::resolve_race_duration`. Quem lia a constante
    /// recebia `is_enduro_duration(0) == false` e tratava uma prova de 6 horas como
    /// sprint. A duração de verdade mora aqui, no `CalendarEntry`.
    ///
    /// Cascata: a duração da etapa; se ela for 0 (save antigo, gravado antes de o campo
    /// existir), a constante da categoria; se ela também for 0 ou a categoria for
    /// desconhecida, [`DURACAO_SPRINT_PADRAO_MIN`].
    pub fn duracao_efetiva_min(&self) -> u16 {
        duracao_efetiva_min(self.duracao_corrida_min, &self.categoria)
    }
}

/// A mesma cascata de [`CalendarEntry::duracao_efetiva_min`] para quem tem os dois
/// campos soltos em vez da etapa inteira.
pub fn duracao_efetiva_min(duracao_da_etapa_min: i32, categoria: &str) -> u16 {
    u16::try_from(duracao_da_etapa_min)
        .ok()
        .filter(|&d| d > 0)
        .or_else(|| {
            crate::constants::categories::get_category_config(categoria)
                .map(|c| c.duracao_corrida_min)
                .filter(|&d| d > 0)
        })
        .unwrap_or(DURACAO_SPRINT_PADRAO_MIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duracao_da_etapa_manda_quando_existe() {
        assert_eq!(duracao_efetiva_min(360, "endurance"), 360);
        assert_eq!(duracao_efetiva_min(25, "gt3"), 25);
    }

    #[test]
    fn endurance_com_etapa_zerada_nao_vira_sprint_de_zero_minuto() {
        // A sentinela da categoria (0) não pode escapar: cai no padrão de sprint,
        // que é honesto, em vez de virar `is_enduro_duration(0) == false` disfarçado.
        assert_eq!(
            duracao_efetiva_min(0, "endurance"),
            DURACAO_SPRINT_PADRAO_MIN
        );
    }

    #[test]
    fn save_antigo_sem_duracao_cai_na_constante_da_categoria() {
        let cat = crate::constants::categories::get_category_config("gt3")
            .expect("gt3 existe no catálogo");
        assert!(cat.duracao_corrida_min > 0, "gt3 tem duração declarada");
        assert_eq!(duracao_efetiva_min(0, "gt3"), cat.duracao_corrida_min);
    }

    #[test]
    fn categoria_desconhecida_cai_no_padrao() {
        assert_eq!(
            duracao_efetiva_min(0, "categoria_que_nao_existe"),
            DURACAO_SPRINT_PADRAO_MIN
        );
        assert_eq!(
            duracao_efetiva_min(-5, "categoria_que_nao_existe"),
            DURACAO_SPRINT_PADRAO_MIN
        );
    }

    #[test]
    fn a_prova_longa_de_endurance_passa_no_gate_de_enduro() {
        // O bug em uma linha: com a constante da categoria o gate dava falso.
        for minutos in [120, 180, 240, 360] {
            assert!(
                crate::car::breakdown::is_enduro_duration(duracao_efetiva_min(
                    minutos,
                    "endurance"
                )),
                "{minutos} min deveria ser enduro"
            );
        }
    }
}
