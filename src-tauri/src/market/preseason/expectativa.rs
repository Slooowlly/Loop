//! A expectativa de procura mostrada ao jogador nas semanas de abertura da janela.
//!
//! Antes de o mercado contratar, o jogador não recebe proposta nenhuma — ficaria duas
//! semanas olhando uma tela muda. No lugar das propostas ele vê uma FAIXA: quantas
//! equipes devem cortejá-lo quando a janela abrir.
//!
//! A faixa é honesta por construção: ela não é um teaser calibrado no olho, e sim a
//! MESMA conta que vai gerar as propostas (`count_interested_teams`), com a incerteza
//! que cada semana ainda carrega. Semana 1 o grid está cheio e não há vaga nenhuma pra
//! avaliar, então o único número verdadeiro é o teto — quantos assentos do nível dele
//! vencem contrato. Semana 2 as vagas já são reais e a conta roda de verdade; sobra só
//! a margem de que a escada mexe no tabuleiro antes da semana 3.
//!
//! Da semana 3 em diante a expectativa NÃO some mais (até 17/08/2026 ela sumia): a
//! proposta formal só nasce quando o jogador é a PRIMEIRA escolha da vaga, então "tem
//! equipe de olho, mas há gente na frente" virou o estado comum — mais da metade das
//! janelas fecha sem proposta nenhuma, medido no Monte Carlo. Sem a expectativa no ar,
//! essas semanas seriam uma tela muda com interesse real acontecendo por trás. A conta
//! passa a ser exata (min == max), porque as vagas já existem e a margem morreu.
//!
//! O piso subindo (0 → conta real − 1 → exata) é o que o jogador lê como "está
//! esquentando".

use super::*;

/// Margem da semana 2: a conta já é a real, mas entre ela e a abertura a escada ainda
/// preenche vagas e o jogador pode segurar assentos — o número final oscila por pouco.
const MARGEM_SEMANA_2: i32 = 1;

/// Recalcula (ou limpa) a expectativa no estado do plano.
///
/// `contracts_snapshot` são os contratos da FOTO — os que valiam antes de as pré-passes
/// caírem. Só é lido na semana 1, quando ainda não existe vaga pra contar.
pub(super) fn refresh_player_interest_forecast(
    conn: &Connection,
    plan: &mut PreSeasonPlan,
    season: i32,
    contracts_snapshot: &[Contract],
) -> Result<(), String> {
    let week = plan.state.current_week;
    if week >= plan.state.signings_start_week {
        // Da abertura em diante a conta é exata: quantas equipes colocariam o jogador na
        // shortlist AGORA. A proposta continua sendo outra coisa (ser a PRIMEIRA
        // escolha) — este número é o interesse que ainda não virou proposta, e é ele que
        // impede a tela de ficar muda nas semanas em que há gente na frente.
        // `count_interested_teams` usa semente própria e não escreve nada, então não
        // desloca o RNG do mercado.
        plan.state.player_interest_forecast =
            crate::market::pipeline::count_interested_teams(conn, season, week)?.map(|count| {
                PlayerInterestForecast {
                    min: count,
                    max: count,
                }
            });
        return Ok(());
    }

    plan.state.player_interest_forecast = if week <= 1 {
        forecast_from_expiring_seats(conn, season, contracts_snapshot)?
    } else {
        forecast_from_open_seats(conn, season, week)?
    };
    Ok(())
}

/// Semana 1 — a foto. Nenhum assento está vago ainda, então contar equipes interessadas
/// devolveria zero por construção. O que se sabe de verdade é o TETO: quantos assentos
/// do nível do jogador têm contrato vencendo e portanto PODEM abrir. Faixa `0..=teto`.
/// `None` quando ele não vai ao mercado (contrato plurianual em dia).
fn forecast_from_expiring_seats(
    conn: &Connection,
    season: i32,
    contracts_snapshot: &[Contract],
) -> Result<Option<PlayerInterestForecast>, String> {
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return Ok(None);
    };
    // Só o aposentado está fora do mercado — lesão e suspensão são temporárias, e o
    // scan das propostas (`build_player_market_scan`) usa o mesmo critério.
    if player.status == crate::models::enums::DriverStatus::Aposentado {
        return Ok(None);
    }

    // Vai ao mercado quem não tem contrato ou tem um que termina nesta virada.
    let player_contract = contracts_snapshot.iter().find(|c| c.piloto_id == player.id);
    if player_contract.is_some_and(|c| c.temporada_fim >= season) {
        return Ok(None);
    }

    let player_tier = tier_of(player.categoria_atual.as_deref())
        .or_else(|| player_contract.and_then(|c| tier_of(Some(c.categoria.as_str()))));
    let Some(player_tier) = player_tier else {
        return Ok(None);
    };

    let teto = contracts_snapshot
        .iter()
        .filter(|c| c.piloto_id != player.id)
        .filter(|c| c.temporada_fim < season)
        .filter(|c| tier_of(Some(c.categoria.as_str())) == Some(player_tier))
        .count() as i32;

    Ok(Some(PlayerInterestForecast { min: 0, max: teto }))
}

/// Semana 2 — as pré-passes caíram e as vagas existem. Roda a conta de verdade e abre
/// só a margem do que ainda pode mudar até a abertura.
fn forecast_from_open_seats(
    conn: &Connection,
    season: i32,
    week: i32,
) -> Result<Option<PlayerInterestForecast>, String> {
    // `None` aqui é o jogador que renovou e não vai ao mercado — sem expectativa. Já
    // `Some(0)` é agente livre que ninguém quer, e isso ele precisa saber.
    let Some(interessadas) = crate::market::pipeline::count_interested_teams(conn, season, week)?
    else {
        return Ok(None);
    };
    Ok(Some(PlayerInterestForecast {
        min: (interessadas - MARGEM_SEMANA_2).max(0),
        max: interessadas + MARGEM_SEMANA_2,
    }))
}

fn tier_of(categoria: Option<&str>) -> Option<u8> {
    categoria
        .and_then(crate::constants::categories::get_category_config)
        .map(|c| c.tier)
}
