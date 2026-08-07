//! Posicionamento do piloto entre pares: campeonato da categoria, companheiro de equipe e rankings de carreira.
//!
//! Rivais saíram daqui para `rivais.rs` quando o bloco deixou de ser três números
//! do motor de rivalidade e passou a cruzar `race_results` atrás do confronto
//! direto — virou consulta de peso próprio.

use super::*;

pub(super) fn build_career_rank_block(
    conn: &Connection,
    driver: &Driver,
) -> Result<DriverCareerRankBlock, String> {
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos para rankings de carreira: {e}"))?;

    Ok(DriverCareerRankBlock {
        corridas: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.corridas),
        vitorias: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.vitorias),
        podios: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.podios),
        titulos: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.titulos),
        // O mesmo conjunto que `rank_driver_by` ordena — se o denominador saisse
        // de outra consulta, "12º de 240" poderia mentir nas duas pontas.
        total: (!drivers.is_empty()).then_some(drivers.len() as i32),
    })
}

/// As mesmas quatro posicoes de carreira, contadas so dentro do grid atual.
///
/// O conjunto e o MESMO que disputa o campeonato da categoria — e, em categoria
/// multiclasse, so a classe dele: quem corre de LMP2 nao divide grid com o GT4,
/// entao contar os dois juntos daria um denominador que nao existe em pista
/// nenhuma. Reaproveita a regra de `grid_da_categoria`, que ja e a mesma usada
/// pela leitura tecnica.
///
/// `None` quando o piloto nao esta no grid resultante (sem contrato, aposentado,
/// ou categoria que nao carregou): a ficha some com o seletor em vez de mostrar
/// uma posicao entre pares que nao sao os dele.
pub(super) fn build_grid_rank_block(
    conn: &Connection,
    driver: &Driver,
    category: Option<&str>,
    team: Option<&Team>,
) -> Option<DriverCareerRankBlock> {
    let drivers = pilotos_do_grid(conn, category?, team);
    if !drivers.iter().any(|value| value.id == driver.id) {
        return None;
    }

    Some(DriverCareerRankBlock {
        corridas: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.corridas),
        vitorias: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.vitorias),
        podios: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.podios),
        titulos: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.titulos),
        total: Some(drivers.len() as i32),
    })
}

/// Os pilotos sentados nas equipes do grid — categoria e, quando ela e
/// multiclasse, a classe do piloto.
pub(super) fn pilotos_do_grid(
    conn: &Connection,
    category: &str,
    team: Option<&Team>,
) -> Vec<Driver> {
    let Ok(equipes) = team_queries::get_teams_by_category(conn, base_category_of(category)) else {
        return Vec::new();
    };
    let classe = team.and_then(|value| value.classe.as_deref());

    equipes
        .iter()
        .filter(|rival| classe.is_none() || rival.classe.as_deref() == classe)
        .flat_map(|rival| [rival.piloto_1_id.as_deref(), rival.piloto_2_id.as_deref()])
        .flatten()
        .filter_map(|id| driver_queries::get_driver(conn, id).ok())
        .collect()
}

pub(super) fn rank_driver_by<F>(drivers: &[Driver], driver_id: &str, metric: F) -> Option<i32>
where
    F: Fn(&Driver) -> u32,
{
    let mut ranked: Vec<(&str, u32)> = drivers
        .iter()
        .map(|driver| (driver.id.as_str(), metric(driver)))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    ranked
        .iter()
        .position(|(id, _)| *id == driver_id)
        .map(|index| index as i32 + 1)
}

/// Onde o piloto esta NA TABELA, e nao so em que degrau dela.
///
/// "P3" sozinho nao diz se o campeonato esta a um podio de distancia ou ja
/// perdido — o mesmo P3 pode estar 4 pontos do lider ou 180. `gap_lider` e
/// quanto falta para o topo (0 para quem lidera) e `gap_proximo` a vantagem
/// sobre quem vem logo atras (None para o lanterna, que nao tem ninguem atras).
pub(super) struct ChampionshipContext {
    pub posicao: i32,
    /// Quantos pilotos disputam este campeonato — o denominador da posicao acima.
    /// Sai da MESMA lista ordenada, pelo mesmo motivo dos gaps: um total vindo de
    /// outra consulta poderia dizer "P14 de 12".
    pub total: i32,
    pub gap_lider: i32,
    pub gap_proximo: Option<i32>,
}

pub(super) fn find_championship_context(
    conn: &Connection,
    category: &str,
    driver_id: &str,
) -> Result<Option<ChampionshipContext>, String> {
    // `category` aqui e a CHAVE DE DIVISAO ("endurance:lmp2"), e `categoria_atual`
    // do piloto guarda so a categoria base ("endurance"). Consultar a chave inteira
    // nao trazia linha nenhuma, e o piloto sumia da propria classificacao: sem
    // posicao, sem gap e sem delta contra o esperado — um terco do grid (as duas
    // categorias multiclasse) via a faixa vazia por isso.
    let (base_category, class_name) = match category.split_once(':') {
        Some((base, class_name)) => (base, Some(class_name)),
        None => (category, None),
    };
    let mut drivers = driver_queries::get_drivers_by_category(conn, base_category)
        .map_err(|e| format!("Falha ao carregar classificacao da categoria: {e}"))?;
    // Numa categoria multiclasse cada classe corre o SEU campeonato: o LMP2 nao
    // disputa pontos com o GT4. Quem diz a classe de cada piloto e o assento na
    // equipe, que e onde a classe esta persistida.
    if let Some(class_name) = class_name {
        let seats = class_seat_ids(conn, base_category, class_name)?;
        drivers.retain(|driver| seats.contains(driver.id.as_str()));
    }
    drivers.sort_by(|a, b| {
        b.stats_temporada
            .pontos
            .partial_cmp(&a.stats_temporada.pontos)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.stats_temporada.vitorias.cmp(&a.stats_temporada.vitorias))
            .then_with(|| b.stats_temporada.podios.cmp(&a.stats_temporada.podios))
            .then_with(|| a.nome.cmp(&b.nome))
    });

    let Some(index) = drivers.iter().position(|driver| driver.id == driver_id) else {
        return Ok(None);
    };
    // A mesma lista ordenada que deu a posicao da os dois gaps: se o denominador
    // saisse de outra consulta, "8 do lider" poderia discordar do proprio "P3".
    let pontos = |driver: &Driver| driver.stats_temporada.pontos.round() as i32;
    let atual = pontos(&drivers[index]);

    Ok(Some(ChampionshipContext {
        posicao: index as i32 + 1,
        total: drivers.len() as i32,
        gap_lider: pontos(&drivers[0]) - atual,
        gap_proximo: drivers
            .get(index + 1)
            .map(|proximo| atual - pontos(proximo)),
    }))
}

/// IDs dos pilotos sentados nas equipes de UMA classe da categoria.
fn class_seat_ids(
    conn: &Connection,
    base_category: &str,
    class_name: &str,
) -> Result<HashSet<String>, String> {
    let teams = team_queries::get_teams_by_category(conn, base_category)
        .map_err(|e| format!("Falha ao carregar equipes da classe: {e}"))?;

    Ok(teams
        .into_iter()
        .filter(|team| team.classe.as_deref() == Some(class_name))
        .flat_map(|team| [team.piloto_1_id, team.piloto_2_id])
        .flatten()
        .collect())
}

pub(super) fn find_teammate(
    conn: &Connection,
    driver: &Driver,
    team: Option<&Team>,
) -> Result<Option<Driver>, String> {
    let Some(team) = team else {
        return Ok(None);
    };
    let teammate_id = [team.piloto_1_id.as_ref(), team.piloto_2_id.as_ref()]
        .into_iter()
        .flatten()
        .find(|id| id.as_str() != driver.id);
    let Some(teammate_id) = teammate_id else {
        return Ok(None);
    };

    driver_queries::get_driver(conn, teammate_id)
        .map(Some)
        .map_err(|e| format!("Falha ao carregar companheiro de equipe: {e}"))
}
