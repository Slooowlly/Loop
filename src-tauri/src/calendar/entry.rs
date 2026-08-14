//! Entidade de calendário (extraída de `calendar/mod.rs`).

use serde::{Deserialize, Serialize};

use crate::car::breakdown::DuracaoDeProva;
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
const DURACAO_SPRINT_PADRAO: DuracaoDeProva = DuracaoDeProva::constante(30);

impl CalendarEntry {
    /// Duração REAL desta prova, pronta para alimentar o gate de enduro
    /// ([`DuracaoDeProva::e_enduro`]) e o desgaste da etapa.
    ///
    /// Existe porque `get_category_config(...).duracao_corrida_min` é uma SENTINELA no
    /// Endurance: lá a constante vale 0 e quem sorteia entre 120, 180, 240 e 360 minutos
    /// por etapa é `calendar::montagem::resolve_race_duration`. Quem lia a constante
    /// tratava uma prova de 6 horas como sprint. A duração de verdade mora aqui, no
    /// `CalendarEntry`.
    ///
    /// Devolve [`DuracaoDeProva`], não `u16`: esta é a única cascata que sabe resolver a
    /// sentinela, então é aqui que ela morre. Quem recebe o retorno já não tem como passar
    /// zero adiante.
    ///
    /// Cascata: a duração da etapa; se ela for 0 (save antigo, gravado antes de o campo
    /// existir), a constante da categoria; se ela também for 0 ou a categoria for
    /// desconhecida, [`DURACAO_SPRINT_PADRAO`].
    pub fn duracao_efetiva(&self) -> DuracaoDeProva {
        duracao_efetiva(self.duracao_corrida_min, &self.categoria)
    }
}

/// A mesma cascata de [`CalendarEntry::duracao_efetiva`] para quem tem os dois
/// campos soltos em vez da etapa inteira.
pub fn duracao_efetiva(duracao_da_etapa_min: i32, categoria: &str) -> DuracaoDeProva {
    u16::try_from(duracao_da_etapa_min)
        .ok()
        .and_then(DuracaoDeProva::nova)
        .or_else(|| {
            crate::constants::categories::get_category_config(categoria)
                .and_then(|c| DuracaoDeProva::nova(c.duracao_corrida_min))
        })
        .unwrap_or(DURACAO_SPRINT_PADRAO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duracao_da_etapa_manda_quando_existe() {
        assert_eq!(duracao_efetiva(360, "endurance").minutos(), 360);
        assert_eq!(duracao_efetiva(25, "gt3").minutos(), 25);
    }

    #[test]
    fn endurance_com_etapa_zerada_nao_vira_sprint_de_zero_minuto() {
        // A sentinela da categoria (0) não pode escapar: cai no padrão de sprint,
        // que é honesto, em vez de virar uma prova de zero minuto disfarçada.
        assert_eq!(duracao_efetiva(0, "endurance"), DURACAO_SPRINT_PADRAO);
    }

    #[test]
    fn save_antigo_sem_duracao_cai_na_constante_da_categoria() {
        let cat = crate::constants::categories::get_category_config("gt3")
            .expect("gt3 existe no catálogo");
        assert!(cat.duracao_corrida_min > 0, "gt3 tem duração declarada");
        assert_eq!(duracao_efetiva(0, "gt3").minutos(), cat.duracao_corrida_min);
    }

    #[test]
    fn categoria_desconhecida_cai_no_padrao() {
        assert_eq!(
            duracao_efetiva(0, "categoria_que_nao_existe"),
            DURACAO_SPRINT_PADRAO
        );
        assert_eq!(
            duracao_efetiva(-5, "categoria_que_nao_existe"),
            DURACAO_SPRINT_PADRAO
        );
    }

    /// A cascata NUNCA devolve a sentinela — é o invariante que sustenta o tipo do retorno.
    /// Varre a entrada crua inteira que o banco pode gravar (inclusive negativa) contra todas
    /// as categorias do catálogo, mais uma desconhecida.
    #[test]
    fn a_cascata_nunca_devolve_zero() {
        let mut categorias: Vec<&str> = crate::constants::categories::get_all_categories()
            .iter()
            .map(|c| c.id)
            .collect();
        categorias.push("categoria_que_nao_existe");
        for categoria in categorias {
            for bruto in [-5, 0, 1, 25, 120, 360] {
                assert!(
                    duracao_efetiva(bruto, categoria).minutos() > 0,
                    "{categoria} com bruto {bruto} devolveu a sentinela"
                );
            }
        }
    }

    #[test]
    fn a_prova_longa_de_endurance_passa_no_gate_de_enduro() {
        // O bug em uma linha: com a constante da categoria o gate dava falso.
        for minutos in [120, 180, 240, 360] {
            assert!(
                duracao_efetiva(minutos, "endurance").e_enduro(),
                "{minutos} min deveria ser enduro"
            );
        }
    }
}
