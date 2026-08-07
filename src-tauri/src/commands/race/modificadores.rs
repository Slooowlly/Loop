//! **Prévia dos modificadores do fim de semana**, piloto a piloto.
//!
//! A esteira de `simulation::esteira` cobra (ou dá) pontos de skill a cada piloto do grid antes
//! da largada: conhecimento de pista, adaptação à categoria, lesão, as três camadas de forma,
//! motivação e pressão. Até aqui isso acontecia inteiramente por baixo do pano — o jogador via
//! o resultado da corrida e nunca soube que o rival tinha chegado ao domingo três pontos abaixo
//! do próprio número.
//!
//! Este comando abre a caixa. Ele **não simula nada**: chama a MESMA
//! [`super::simulacao::preparar_grid_da_corrida`] que a corrida chama, e devolve o relatório de
//! [`DeltasDoPiloto`](crate::simulation::esteira::DeltasDoPiloto) que a esteira já produzia e
//! ninguém lia. Como a esteira é determinística (as três camadas de forma saem de hash de
//! temporada/etapa/piloto, e as demais de estado já gravado), o que a prévia mostra é
//! exatamente o que a etapa vai rodar — não uma estimativa.
//!
//! **Nada é gravado.** A forma que a esteira avança volta dentro do resultado e é descartada
//! aqui; quem persiste é o comando da corrida, e só ele.

use super::*;

use crate::simulation::esteira::{Canal, EloDaEsteira};

/// Um elo da esteira agindo sobre um piloto, nos dois canais.
///
/// Os canais vêm separados porque eles **divergem de verdade**: a afinidade de pista entra na
/// classificação multiplicada por `forma::MULT_AFINIDADE_QUALI`, e o acerto de fim de semana é
/// sorteado por trim (volta única ≠ distância de prova). Colapsar num número só esconderia
/// justamente o piloto que está bem no sábado e mal no domingo.
#[derive(Debug, Clone, Serialize)]
pub struct WeekendModifier {
    /// Chave estável do elo, para a UI traduzir. Ver [`chave_do_elo`].
    pub key: String,
    /// Pontos de skill no canal de CORRIDA (positivo ajuda).
    pub race: f64,
    /// Pontos de skill no canal de CLASSIFICAÇÃO.
    pub qualifying: f64,
}

/// **A chuva, em bloco separado — e separado por um motivo, não por estética.**
///
/// Ela não é um elo da esteira: a esteira desconta pontos de `skill` antes da largada, e a
/// chuva é cobrada direto no score de segmento (`race/pontuacao.rs`), numa escala diferente.
/// Somar as duas no mesmo total faria o "saldo do dia" mentir.
///
/// E o número que importa aqui não é o absoluto. **Na chuva o pelotão inteiro cai** — quem é
/// bom de chuva não fica mais rápido, fica menos lento. Só o delta contra a média do grid diz
/// se o piloto ganha ou perde posições quando molha, e é por isso que ele vem calculado do
/// lado de cá em vez de a UI tentar adivinhar.
#[derive(Debug, Clone, Serialize)]
pub struct WeekendRain {
    /// Chave do clima da etapa: "dry" | "damp" | "wet" | "heavy". Seco → o resto é zero.
    pub weather: String,
    /// `fator_chuva` do piloto (0–100) — o quanto ele rende no molhado.
    pub rain_skill: f64,
    /// Pontos de ritmo que ESTE piloto perde, já escalados pela sensibilidade da pista.
    pub penalty: f64,
    /// Quanto ele perde a MENOS que a média do grid. Positivo = sobe quando molha.
    pub vs_field: f64,
}

/// O dia de um piloto: os modificadores que estão pegando nele e a soma deles.
#[derive(Debug, Clone, Serialize)]
pub struct WeekendModifiers {
    pub driver_id: String,
    /// Soma dos modificadores no canal de corrida — o "saldo do dia" em pontos de skill.
    pub total_race: f64,
    pub total_qualifying: f64,
    /// **Os oito elos, sempre, na ordem de [`ELOS`]** — inclusive os que valem zero neste fim
    /// de semana. Filtrar os inativos deixava a lista mudando de tamanho e de ordem a cada
    /// piloto, e uma lista que muda de forma se lê do zero toda vez; a UI apaga os zeros com
    /// opacidade, o que custa nada e mantém cada elo sempre na mesma altura do balão.
    pub modifiers: Vec<WeekendModifier>,
    /// Fora do total, e fora da lista: outra unidade. Ver [`WeekendRain`].
    pub rain: WeekendRain,
}

/// Chave estável de cada elo. É contrato com o i18n do frontend — renomear aqui quebra a
/// tradução, não o build.
fn chave_do_elo(elo: EloDaEsteira) -> &'static str {
    match elo {
        EloDaEsteira::ConhecimentoDePista => "trackKnowledge",
        EloDaEsteira::AdaptacaoDeCategoria => "categoryAdaptation",
        EloDaEsteira::Lesao => "injury",
        EloDaEsteira::AfinidadeDePista => "trackAffinity",
        EloDaEsteira::FormaDoMomento => "form",
        EloDaEsteira::AcertoDeFimDeSemana => "setup",
        EloDaEsteira::Motivacao => "motivation",
        EloDaEsteira::Pressao => "pressure",
    }
}

/// **A ordem de leitura, e ela é a da própria esteira** — a sequência em que os elos são
/// aplicados ao piloto em `simulation::esteira::aplicar_esteira`. Não é a ordem por magnitude:
/// essa muda de piloto para piloto, e a lista precisa ser sempre a mesma lista para o olho
/// aprender onde cada coisa mora. De quebra, ela conta a história certa de cima para baixo —
/// primeiro o que o piloto traz da carreira (pista, categoria, corpo), depois o que este fim de
/// semana produziu (afinidade, forma, acerto), e por último a cabeça (motivação, pressão).
const ELOS: [EloDaEsteira; 8] = [
    EloDaEsteira::ConhecimentoDePista,
    EloDaEsteira::AdaptacaoDeCategoria,
    EloDaEsteira::Lesao,
    EloDaEsteira::AfinidadeDePista,
    EloDaEsteira::FormaDoMomento,
    EloDaEsteira::AcertoDeFimDeSemana,
    EloDaEsteira::Motivacao,
    EloDaEsteira::Pressao,
];

/// PRÉVIA DOS MODIFICADORES: como cada piloto da categoria do jogador chega à próxima etapa.
///
/// Alimenta o tooltip da tabela do campeonato na Sala de Estratégia. Devolve lista vazia (e não
/// erro) quando simplesmente não há o que mostrar — sem carreira aberta, sem contrato, sem etapa
/// pendente —, porque um tooltip ausente é melhor que um toast de erro no meio da tela.
#[tauri::command]
pub fn get_weekend_modifiers(
    app: AppHandle,
    career_id: String,
) -> Result<Vec<WeekendModifiers>, String> {
    use crate::db::queries::{
        calendar as calq, contracts as cq, drivers as dq, seasons as sq, teams as tq,
    };

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    // A tabela mostrada é a da categoria do jogador — é essa que precisa dos modificadores.
    let Some(team_id) = dq::get_player_driver(&db.conn)
        .ok()
        .and_then(|player| {
            cq::get_active_contract_for_pilot(&db.conn, &player.id)
                .ok()
                .flatten()
        })
        .map(|contract| contract.equipe_id)
    else {
        return Ok(Vec::new());
    };
    let Some(team) = tq::get_team_by_id(&db.conn, &team_id).ok().flatten() else {
        return Ok(Vec::new());
    };
    let Some(season) = sq::get_active_season(&db.conn).ok().flatten() else {
        return Ok(Vec::new());
    };
    let Some(race) = calq::get_next_race(&db.conn, &season.id, &team.categoria)
        .ok()
        .flatten()
    else {
        return Ok(Vec::new());
    };

    let grid = preparar_grid_da_corrida(&db, &race, RacePersistenceMode::Playable)?;

    // A chuva sai do MESMO contexto que a corrida vai montar — inclusive a `rain_sensitivity`
    // da pista e da categoria, que a curva de chuva escala.
    let ctx = SimulationContext::from_calendar_entry(
        &race,
        grid.category.tier,
        race.rodada >= grid.category.corridas_por_temporada as i32,
    );
    let clima = match ctx.weather {
        crate::models::enums::WeatherCondition::Dry => "dry",
        crate::models::enums::WeatherCondition::Damp => "damp",
        crate::models::enums::WeatherCondition::Wet => "wet",
        crate::models::enums::WeatherCondition::HeavyRain => "heavy",
    };
    let penalidade_de = |fator_chuva: f64| {
        crate::simulation::math::rain_penalty_escalada(ctx.weather, fator_chuva, ctx.rain_sensitivity)
    };
    // A média do grid é a régua: na chuva todo mundo cai, e o que decide posição é cair menos.
    let media_do_grid = if grid.esteira.grid.is_empty() {
        0.0
    } else {
        grid.esteira
            .grid
            .iter()
            .map(|sd| penalidade_de(sd.fator_chuva as f64))
            .sum::<f64>()
            / grid.esteira.grid.len() as f64
    };
    // `deltas` e `grid` saem da esteira na mesma ordem, piloto a piloto.
    let fator_chuva_por_piloto: HashMap<&str, f64> = grid
        .esteira
        .grid
        .iter()
        .map(|sd| (sd.id.as_str(), sd.fator_chuva as f64))
        .collect();

    Ok(grid
        .esteira
        .deltas
        .iter()
        .map(|delta| {
            let modifiers: Vec<WeekendModifier> = ELOS
                .iter()
                .map(|&elo| WeekendModifier {
                    key: chave_do_elo(elo).to_string(),
                    race: delta.pretendido_de(elo, Canal::Corrida),
                    qualifying: delta.pretendido_de(elo, Canal::Classificacao),
                })
                .collect();

            let fator_chuva = fator_chuva_por_piloto
                .get(delta.driver_id.as_str())
                .copied()
                .unwrap_or(50.0);
            let penalidade = penalidade_de(fator_chuva);

            WeekendModifiers {
                total_race: modifiers.iter().map(|m| m.race).sum(),
                total_qualifying: modifiers.iter().map(|m| m.qualifying).sum(),
                driver_id: delta.driver_id.clone(),
                modifiers,
                rain: WeekendRain {
                    weather: clima.to_string(),
                    rain_skill: fator_chuva,
                    penalty: penalidade,
                    vs_field: media_do_grid - penalidade,
                },
            }
        })
        .collect())
}
