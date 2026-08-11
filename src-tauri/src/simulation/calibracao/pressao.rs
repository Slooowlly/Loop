//! **A régua da camada de PRESSÃO** — a que faltava para as vinte e tantas constantes de
//! [`crate::simulation::pressure`] saírem de "fixadas em design" para "medidas".
//!
//! ## O que estava travado, e o que destravou
//!
//! `PACE_K = 3.0`, o neutro em 0,55, o `×2` do líder e o `×3` da última corrida foram
//! escolhidos antes de existir esteira de medição. Não era descuido: **não havia como medir.**
//! A pressão de campeonato precisa da CLASSIFICAÇÃO para existir — sem saber quem lidera e por
//! quanto, `title_context` não tem entrada —, e o arena roda temporadas sem realimentar pontos
//! de uma etapa para a seguinte.
//!
//! Duas coisas mudaram desde então: a esteira virou função pura
//! ([`crate::simulation::esteira::aplicar_esteira`]), chamável de fora do `#[tauri::command]`, e
//! o arena passou a expor [`arena::aplicar_esteira_com_contextos`], que aceita
//! `EntradaDePressao` por piloto. Faltava só o laço que fecha o ciclo: rodar etapa, apurar
//! pontos, derivar a situação de título de cada um, e devolver isso para a esteira da etapa
//! seguinte. É o que este módulo faz.
//!
//! ## A medição é PAREADA, e isso não é detalhe
//!
//! Um número de "margem do campeão com pressão" isolado não diz nada — não há com o que
//! comparar. A medição roda a MESMA temporada duas vezes, com as mesmas sementes e o mesmo
//! grid, mudando só se a pressão é aplicada. Tudo que diferir entre as duas veio da camada, e
//! nada mais: o mesmo desenho da decomposição de variância do módulo `variancia`.
//!
//! ## O que ela NÃO mede, e por quê
//!
//! - **Pressão de evento** (casa cheia) e **duelo** (Nemesis) ficam de fora. A primeira depende
//!   do `venue_score` do interesse do evento, que o grid sintético não tem; o segundo depende
//!   de rivalidade acumulada, que exige histórico. Ambas entram por `EntradaDePressao` quando o
//!   harness tiver de onde tirá-las — a plumbing já aceita as três.
//! - **O erro** (`error_mult`) entra na medição, mas só se `incidentes` estiver ligado na
//!   config: sem incidente ligado, `pressure_error_mult` multiplica uma chance que ninguém rola.

use crate::simulation::context::SimDriver;
use crate::simulation::esteira::{ContextoDoPiloto, EntradaDePressao};
use crate::simulation::pressure::{self, PressureEffect};
use crate::simulation::race::RaceResult;

use super::arena::{self, ConfigTemporada};

/// Pontos do vencedor, INCLUINDO o bônus de volta mais rápida — é o teto do que uma etapa
/// entrega. É o `max_points_per_race` que `title_context` usa para saber quanto ainda dá para
/// recuperar, e sai da tabela oficial em vez de ser um número escolhido aqui.
fn pontos_do_vencedor(is_endurance: bool) -> f64 {
    use crate::constants::scoring::{BONUS_FASTEST_LAP, POINTS_ENDURANCE, POINTS_STANDARD};
    let tabela = if is_endurance {
        POINTS_ENDURANCE
    } else {
        POINTS_STANDARD
    };
    (tabela[0] + BONUS_FASTEST_LAP) as f64
}

/// O resultado de uma temporada medida, no recorte que a pressão pode mover.
#[derive(Debug, Clone, Copy, Default)]
pub struct RetratoDaTemporada {
    /// Margem do campeão sobre o vice, em fração dos pontos do campeão.
    pub margem_do_campeao: f64,
    /// Quantas vezes o líder do campeonato mudou de mão ao longo do ano.
    pub trocas_de_lideranca: f64,
    /// Posição média de chegada de quem estava em briga de título nas últimas etapas.
    /// É a leitura mais direta do clutch/choke: se a camada faz alguma coisa, ela aparece aqui.
    pub chegada_media_dos_candidatos: f64,
    /// Incidentes por piloto-corrida nas etapas dentro da janela de pressão.
    pub incidentes_na_reta_final: f64,
}

/// Roda UMA temporada com o ciclo de pontos fechado, aplicando (ou não) a pressão.
///
/// `com_pressao = false` roda exatamente o mesmo caminho com `EntradaDePressao` neutra — não é
/// um atalho que pula a esteira. Isso importa: pular a esteira mudaria o consumo de RNG e as
/// duas temporadas deixariam de ser comparáveis.
pub fn rodar_temporada_com_pressao(
    config: &ConfigTemporada,
    grid: &[SimDriver],
    catalogo: &crate::simulation::catalog::IncidentCatalog,
    semente: u64,
    temporada: i32,
    com_pressao: bool,
) -> Vec<RaceResult> {
    let eventos = arena::sortear_eventos(config, semente);
    let total = eventos.len();
    let mut estado = arena::estado_de_forma_inicial(grid);
    let mut pontos: Vec<f64> = vec![0.0; grid.len()];
    let max_por_corrida = pontos_do_vencedor(config.is_endurance());
    let mut corridas = Vec::with_capacity(total);

    for (indice, evento) in eventos.iter().enumerate() {
        let rodada = indice + 1;
        let restantes = (total - indice) as u32;

        // A situação de título de cada piloto, da classificação ATUAL — que é o que a torna
        // realimentação e não parâmetro.
        let contextos: Vec<ContextoDoPiloto> = grid
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let pressao = if com_pressao {
                    let tc =
                        pressure::title_context(pontos[i], &pontos, restantes, max_por_corrida);
                    let is_chaser = tc.in_contention && !tc.is_leader;
                    let campeonato = pressure::pressure_effect(
                        pressure::pressure_intensity(&tc, restantes),
                        pressure::pressure_resilience(d.mentalidade as f64, d.experiencia as f64),
                        is_chaser,
                    );
                    EntradaDePressao {
                        campeonato,
                        evento: PressureEffect::NONE,
                        duelo: None,
                        mentalidade: d.mentalidade as f64,
                        experiencia: d.experiencia as f64,
                    }
                } else {
                    EntradaDePressao::default()
                };
                ContextoDoPiloto {
                    pressao: Some(pressao),
                    ..Default::default()
                }
            })
            .collect();

        let (usado, _) = arena::aplicar_esteira_com_contextos(
            grid,
            &contextos,
            temporada,
            rodada as i32,
            evento.pista.track_id,
            &mut estado,
            &config.escalas_da_previa.unwrap_or_default(),
        );

        let corrida = arena::rodar_evento(
            config,
            &usado,
            evento,
            rodada,
            catalogo,
            semente
                .wrapping_mul(31)
                .wrapping_add(indice as u64 * 1_000_003),
        );

        for (i, d) in grid.iter().enumerate() {
            if let Some(r) = corrida.race_results.iter().find(|r| r.pilot_id == d.id) {
                pontos[i] += r.points_earned as f64;
            }
        }
        corridas.push(corrida);
    }

    corridas
}

/// Extrai o retrato de uma temporada já rodada.
pub fn retrato(grid: &[SimDriver], corridas: &[RaceResult], janela: u32) -> RetratoDaTemporada {
    let total = corridas.len();
    if total == 0 || grid.is_empty() {
        return RetratoDaTemporada::default();
    }

    let mut pontos: Vec<f64> = vec![0.0; grid.len()];
    let mut lider_anterior: Option<usize> = None;
    let mut trocas = 0.0;
    let mut chegadas_de_candidato: Vec<f64> = Vec::new();
    let mut incidentes_finais: Vec<f64> = Vec::new();

    for (indice, corrida) in corridas.iter().enumerate() {
        let restantes = (total - indice) as u32;
        // "Candidato" é lido ANTES da etapa, com a classificação com que ele entrou nela — é a
        // mesma foto que a pressão usou.
        let max_atual = pontos.iter().cloned().fold(f64::MIN, f64::max);
        let na_janela = restantes <= janela && indice > 0;

        for (i, d) in grid.iter().enumerate() {
            let Some(r) = corrida.race_results.iter().find(|r| r.pilot_id == d.id) else {
                continue;
            };
            if na_janela && pontos[i] >= max_atual - 1e-6 * max_atual.abs() - 25.0 {
                if !r.is_dnf {
                    chegadas_de_candidato.push(r.finish_position as f64);
                }
                incidentes_finais.push(r.incidents_count as f64);
            }
            pontos[i] += r.points_earned as f64;
        }

        let lider = pontos
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i);
        if lider_anterior.is_some() && lider != lider_anterior {
            trocas += 1.0;
        }
        lider_anterior = lider;
    }

    let mut ordenados = pontos.clone();
    ordenados.sort_by(|a, b| b.total_cmp(a));
    let campeao = ordenados.first().copied().unwrap_or(0.0);
    let vice = ordenados.get(1).copied().unwrap_or(0.0);
    let media = |v: &[f64]| {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };

    RetratoDaTemporada {
        margem_do_campeao: if campeao > 0.0 {
            (campeao - vice) / campeao
        } else {
            0.0
        },
        trocas_de_lideranca: trocas,
        chegada_media_dos_candidatos: media(&chegadas_de_candidato),
        incidentes_na_reta_final: media(&incidentes_finais),
    }
}

/// Média dos retratos de uma campanha PAREADA: as mesmas sementes e os mesmos grids, com e sem
/// pressão. Devolve `(sem, com)`.
pub fn campanha_pareada(
    config: &ConfigTemporada,
    temporadas: usize,
    semente_base: u64,
    janela: u32,
) -> (RetratoDaTemporada, RetratoDaTemporada) {
    let catalogo = arena::catalogo_para(config);
    let mut sem = Vec::with_capacity(temporadas);
    let mut com = Vec::with_capacity(temporadas);

    for t in 0..temporadas {
        let semente = arena::semente_da_temporada(semente_base, t);
        let grid = super::campo::gerar_campo(&config.perfil, config.pilotos, semente);
        for (ligado, destino) in [(false, &mut sem), (true, &mut com)] {
            let corridas = rodar_temporada_com_pressao(
                config,
                &grid,
                &catalogo,
                semente ^ 0x5EED,
                t as i32 + 1,
                ligado,
            );
            destino.push(retrato(&grid, &corridas, janela));
        }
    }

    let agregar = |v: &[RetratoDaTemporada]| {
        let n = v.len().max(1) as f64;
        RetratoDaTemporada {
            margem_do_campeao: v.iter().map(|r| r.margem_do_campeao).sum::<f64>() / n,
            trocas_de_lideranca: v.iter().map(|r| r.trocas_de_lideranca).sum::<f64>() / n,
            chegada_media_dos_candidatos: v
                .iter()
                .map(|r| r.chegada_media_dos_candidatos)
                .sum::<f64>()
                / n,
            incidentes_na_reta_final: v.iter().map(|r| r.incidentes_na_reta_final).sum::<f64>() / n,
        }
    };
    (agregar(&sem), agregar(&com))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A camada de pressão faz alguma coisa?** É a primeira pergunta, e até 11/08/2026 ela não
    /// tinha resposta medida — só o argumento de desenho.
    ///
    /// O teste não assevera um valor: assevera que a camada NÃO É INERTE. Um valor asseverado
    /// aqui congelaria uma calibração que ainda não foi feita; um "não é inerte" pega o modo de
    /// falha que a guarda de `consumo` já pegou três vezes neste módulo — o parâmetro que é
    /// calculado, guardado e nunca chega ao resultado.
    ///
    /// Rode com `--nocapture` para ler a tabela: é dela que sai a decisão sobre `PACE_K`.
    #[test]
    #[ignore = "pesado; harness de calibração da camada de pressão"]
    fn pressao_de_campeonato_move_o_resultado() {
        for (rotulo, base) in [
            ("mazda_rookie", ConfigTemporada::rookie()),
            ("gt3", ConfigTemporada::gt3()),
        ] {
            let config = ConfigTemporada {
                pilotos: 20,
                etapas: 12,
                esteira_de_forma: true,
                ..base
            }
            .com_incidentes(true);

            let (sem, com) = campanha_pareada(&config, 24, 90_210, 5);
            println!(
                "\n== {rotulo} — pressão de campeonato (24 temporadas pareadas) ==\n\
                 {:<34} {:>10} {:>10} {:>10}\n\
                 {:<34} {:>10.4} {:>10.4} {:>10.4}\n\
                 {:<34} {:>10.4} {:>10.4} {:>10.4}\n\
                 {:<34} {:>10.4} {:>10.4} {:>10.4}\n\
                 {:<34} {:>10.4} {:>10.4} {:>10.4}",
                "métrica",
                "sem",
                "com",
                "delta",
                "margem do campeão",
                sem.margem_do_campeao,
                com.margem_do_campeao,
                com.margem_do_campeao - sem.margem_do_campeao,
                "trocas de liderança",
                sem.trocas_de_lideranca,
                com.trocas_de_lideranca,
                com.trocas_de_lideranca - sem.trocas_de_lideranca,
                "chegada média dos candidatos",
                sem.chegada_media_dos_candidatos,
                com.chegada_media_dos_candidatos,
                com.chegada_media_dos_candidatos - sem.chegada_media_dos_candidatos,
                "incidentes na reta final",
                sem.incidentes_na_reta_final,
                com.incidentes_na_reta_final,
                com.incidentes_na_reta_final - sem.incidentes_na_reta_final,
            );

            let mexeu = (com.margem_do_campeao - sem.margem_do_campeao).abs() > 1e-9
                || (com.trocas_de_lideranca - sem.trocas_de_lideranca).abs() > 1e-9
                || (com.chegada_media_dos_candidatos - sem.chegada_media_dos_candidatos).abs()
                    > 1e-9;
            assert!(
                mexeu,
                "{rotulo}: a camada de pressão não moveu NADA — ou ela é inerte, ou o \
                 `EntradaDePressao` não está chegando à esteira"
            );
        }
    }
}
