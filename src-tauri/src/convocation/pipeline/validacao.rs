//! Validação em memória dos grids montados: roda antes de qualquer escrita e
//! devolve a lista de erros (duplicatas intra-grid e entre classes).

use super::*;

pub(super) fn validar_grids(grids: &[GridClasse]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut global_driver_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for grid in grids {
        // Sem duplicatas intra-grid
        let mut ids_neste: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for a in &grid.assignments {
            if !ids_neste.insert(a.driver_id.as_str()) {
                errors.push(format!(
                    "[{}] driver_id duplicado no grid: {}",
                    grid.class_name, a.driver_id
                ));
            }
            if !global_driver_ids.insert(a.driver_id.as_str()) {
                errors.push(format!(
                    "[{}] driver {} já foi alocado em outra classe",
                    grid.class_name, a.driver_id
                ));
            }
        }
    }

    errors
}
