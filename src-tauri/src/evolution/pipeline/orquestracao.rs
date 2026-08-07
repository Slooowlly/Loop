//! Orquestração de alto nível da virada de temporada: monta o contexto, roda
//! standings/licenças, evolução dos pilotos, arquivamento, finanças, promoção e
//! abre a próxima temporada — tudo dentro de uma única transação.

use super::*;

pub fn run_end_of_season(
    conn: &mut Connection,
    season: &Season,
    save_path: &Path,
) -> Result<EndOfSeasonResult, String> {
    run_end_of_season_with_mode(conn, season, save_path, EndOfSeasonMode::Playable)
}

pub(crate) fn run_historical_end_of_season(
    conn: &mut Connection,
    season: &Season,
    save_path: &Path,
) -> Result<EndOfSeasonResult, String> {
    run_end_of_season_with_mode(conn, season, save_path, EndOfSeasonMode::HistoricalDraft)
}

pub(super) fn run_end_of_season_with_mode(
    conn: &mut Connection,
    season: &Season,
    save_path: &Path,
    mode: EndOfSeasonMode,
) -> Result<EndOfSeasonResult, String> {
    let mut rng = StdRng::seed_from_u64(((season.numero as u64) << 32) | season.ano as u64);
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao iniciar transacao de fim de temporada: {e}"))?;

    let (teams_by_id, contracts_by_driver) = build_context(&tx)?;

    let standings = build_and_persist_standings(&tx, season, &contracts_by_driver)?;
    // Pilotos de lmp2 aparecem em dois standings (lmp2 regular + classe lmp2 da
    // Endurance); o campeonato regular é o de referência para evolução/arquivo.
    let mut standings_by_driver: HashMap<String, StandingEntry> = HashMap::new();
    for entry in standings
        .iter()
        .filter(|entry| crate::constants::categories::is_multiclass_category(&entry.category))
    {
        standings_by_driver.insert(entry.driver_id.clone(), entry.clone());
    }
    for entry in standings
        .iter()
        .filter(|entry| !crate::constants::categories::is_multiclass_category(&entry.category))
    {
        standings_by_driver.insert(entry.driver_id.clone(), entry.clone());
    }

    let licenses_earned = persist_licenses(&tx, &standings, &standings_by_driver)
        .map_err(|e| format!("Falha ao persistir licencas: {e}"))?;

    season_queries::finalize_season(&tx, &season.id)
        .map_err(|e| format!("Falha ao finalizar temporada: {e}"))?;

    // Títulos contam por campeonato vencido: nas categorias especiais cada classe
    // (mazda/toyota/bmw, gt4/gt3/lmp2) tem o próprio campeão — não há título geral.
    let mut titles_by_driver: HashMap<String, i32> = HashMap::new();
    for entry in standings.iter().filter(|entry| entry.position == 1) {
        *titles_by_driver.entry(entry.driver_id.clone()).or_insert(0) += 1;
    }

    // Cura lesões de pilotos sem assento (passo 1 de 2). O tick por corrida
    // (`process_injury_recovery`) só enxerga quem tem `categoria_atual`; um piloto
    // que perde o assento enquanto lesionado nunca cicatriza e fica preso fora da
    // grade para sempre. Esta 1ª passagem cobre os órfãos que JÁ entraram na virada
    // sem assento (passivo de saves antigas): roda antes da evolução para que o
    // recuperado volte a `Ativo` e dispute já o mercado da pré-temporada desta virada.
    // A 2ª passagem (após o mercado, abaixo) cobre quem VIRA órfão agora — ver lá.
    crate::evolution::injury::process_injury_recovery_without_seat(&tx)
        .map_err(|e| format!("Falha ao recuperar lesões de pilotos sem assento: {e}"))?;

    // Onde cada piloto estava ANTES da virada. É a referência do segundo passe da
    // motivação, lá embaixo: promoção e mercado ainda não rodaram nesta altura,
    // então "promovido/rebaixado/renovado/dispensado" só se conhece comparando
    // este retrato com o de depois da pré-temporada.
    let seats_before = seat_map(&tx)?;

    let (growth_reports, mut motivation_reports, retirements, _existing_names) =
        process_driver_evolution(
            &tx,
            season,
            &standings_by_driver,
            &titles_by_driver,
            &contracts_by_driver,
            &teams_by_id,
            &mut rng,
        )?;

    archive_driver_season(&tx, season, &standings_by_driver)
        .map_err(|e| format!("Falha ao arquivar temporada dos pilotos: {e}"))?;
    archive_team_season(&tx, season)
        .map_err(|e| format!("Falha ao arquivar temporada das equipes: {e}"))?;

    // Reputação viva: com as posições já arquivadas, cada equipe tem a reputação
    // ajustada pelo resultado (título sobe, fiasco desce, decai pro meio). Roda
    // ANTES da promoção/rebaixamento, que somam o próprio degrau por cima.
    crate::finance::reputation::update_team_reputations_from_season(&tx, season)
        .map_err(|e| format!("Falha ao atualizar reputação das equipes: {e}"))?;

    // Histórico de carreira vivo (ideia 2): consolida os arquivos de temporada nos
    // campos `historico_*` do time. Faz o termo `titulos_construtores*2` do
    // elite_score das dinastias deixar de ser 0 → campeões passados ficam elite por
    // legado. Recompute idempotente; roda com o archive já gravado.
    crate::world::team_archive::roll_up_team_career_history(&tx)
        .map_err(|e| format!("Falha ao consolidar histórico das equipes: {e}"))?;

    // Moral viva (ideia 3): move a moral de cada equipe pelo resultado da temporada
    // + a treta interna N1/N2. A moral entra na simulação (efeito sutil, simétrico
    // jogador+IA) e na eficiência de desenvolvimento do carro. Promoção/rebaixamento
    // aplicam os próprios multiplicadores de moral depois.
    crate::finance::morale::update_team_morale_from_season(&tx, season)
        .map_err(|e| format!("Falha ao atualizar moral das equipes: {e}"))?;

    // Vínculo piloto-equipe (ideia 4, Fase 1): pares que ficaram juntos acumulam
    // (bônus por título); os demais decaem. É o motor de "segurar um piloto pra
    // fazer história" — as consequências (renovação leal, segurar-vs-vender) vêm
    // depois. Roda com o archive já gravado (fonte de quem esteve com quem).
    crate::market::bond::update_bonds_from_season(&tx, season)
        .map_err(|e| format!("Falha ao atualizar vínculos piloto-equipe: {e}"))?;

    // Rivalidade entre EQUIPES — Fonte 1 (briga de construtores): lê o `team_season_archive`
    // recém-gravado e reforça rivalidades entre construtores que brigaram apertado nesta
    // temporada. É a espinha dorsal do sistema; o decay logo abaixo esfria o recente (igual
    // ao piloto). Best-effort semântico — não desfaz o resto do offseason em caso de dado
    // parcial (usa os mesmos guards do motor de piloto).
    crate::rivalry::team::process_constructor_battle_rivalry(&tx, season.numero)
        .map_err(|e| format!("Falha na rivalidade de construtores: {e}"))?;

    // Prêmio de fim de temporada por posição no campeonato de construtores.
    // Creditado após o arquivamento (que define posicao_campeonato) e antes da
    // promoção/rebaixamento, para que a equipe receba referente à categoria em
    // que de fato competiu nesta temporada.
    award_constructor_prizes(&tx, season)
        .map_err(|e| format!("Falha ao pagar prêmios de construtores: {e}"))?;

    // Ciclo de colapso → venda: equipes que fecham a temporada em colapso têm o
    // contador incrementado; ao chegar à 2ª temporada consecutiva em colapso (a
    // 2ª já em all-in), a equipe é vendida e renovada por uma nova diretoria.
    process_collapse_lifecycle(&tx, season, &mut rng)
        .map_err(|e| format!("Falha no ciclo de colapso/venda de equipes: {e}"))?;

    // Modelo fechado: nada de pré-geração de rookies aqui (era fonte de órfãos —
    // os excedentes não contratados viravam agentes livres eternos). Rookies nascem
    // sob demanda no mercado/cascata quando abre uma vaga de categoria de estreia.
    let rookies_generated: Vec<RookieInfo> = Vec::new();

    let promotion_result =
        run_promotion_relegation_for_year(&tx, season.numero, season.ano, &mut rng)
            .map_err(|e| format!("Erro na promocao/rebaixamento: {e}"))?;

    apply_season_end_rivalry_decay(&tx, season.numero)
        .map_err(|e| format!("Erro no decaimento de rivalidades: {e}"))?;

    // OS DOIS VEREDITOS DE TEMPORADA VÊM DEPOIS DO DECAIMENTO, e a ordem não é detalhe.
    // O decaimento esfria o que a temporada acumulou corrida a corrida; estes dois
    // gatilhos são o RESUMO dessa mesma temporada, calculado de uma vez no fim. Passá-los
    // pelo decaimento seria contar o resfriamento duas vezes — e não é teoria: com eles
    // rodando antes, a rivalidade de pista da faixa de entrada nascia em percebida 6.6,
    // saía do decaimento em 3.9 e era APAGADA no mesmo instante (o limiar de extinção é
    // 5.0). O harness mostrou `pista = 0` em dez das doze temporadas por causa disso.
    //
    // Companheiros: placar do duelo interno N1/N2 do ano, lido dos contadores de
    // hierarquia antes da pré-temporada zerá-los. É o caminho que de fato produz treta de
    // dupla — o eixo de tensão mora no piso, porque o N2 leva ~29% dos duelos e o eixo
    // exige 40% só para parar de cair.
    crate::rivalry::process_teammate_season_rivalry(&tx, season.numero)
        .map_err(|e| format!("Falha na rivalidade entre companheiros de equipe: {e}"))?;

    // Pista: placar de chegadas coladas do ano, lido de `race_results`. Mesmo motivo do
    // gatilho acima — o tipo `Pista` só era aplicado na importação de corrida real do
    // iRacing, então em mundo simulado ele nunca existia.
    crate::rivalry::process_track_season_rivalry(&tx, season.numero)
        .map_err(|e| format!("Falha na rivalidade de pista: {e}"))?;

    // Decaimento anual das rivalidades entre EQUIPES (mesma regra do piloto): clássicos
    // ativos persistem e crescem no histórico; brigas pontuais esfriam e somem sozinhas.
    crate::rivalry::team::apply_season_end_team_rivalry_decay(&tx, season.numero)
        .map_err(|e| format!("Erro no decaimento de rivalidades de equipe: {e}"))?;

    let new_season = create_next_season_phase(&tx, season, &mut rng, mode)?;

    let (preseason_initialized, preseason_total_weeks) =
        initialize_preseason_phase(&tx, &new_season, save_path, &mut rng, mode)?;

    // Cura lesões de pilotos sem assento (passo 2 de 2). O mercado da pré-temporada
    // (dentro de `initialize_preseason_phase`) larga o piloto que se lesionou na
    // última corrida e teve o contrato expirando: a renovação e o leilão pulam quem
    // não está `Ativo`, e o `sync` zera a categoria dele. Sem esta 2ª passagem ele só
    // seria curado na PRÓXIMA virada (a 1ª passagem já rodou antes de virar órfão),
    // ficando uma temporada inteira na bancada. Curando-o aqui, entra a temporada que
    // vem como agente livre `Ativo` e a janela de transferências semanal o recontrata.
    crate::evolution::injury::process_injury_recovery_without_seat(&tx)
        .map_err(|e| format!("Falha ao recuperar lesões de pilotos recém-liberados: {e}"))?;

    // SEGUNDO PASSE DA MOTIVAÇÃO — a última coisa antes do commit, porque só aqui
    // o offseason inteiro já aconteceu: promoção/rebaixamento (acima) e o mercado
    // de pré-temporada (dentro de `initialize_preseason_phase`). É o passe que faz
    // "perdeu a vaga" e "foi rebaixado" existirem de fato; antes disso os dois
    // chegavam ao modelo como `false` fixo e nunca puxavam a motivação para baixo.
    let offseason_reports = apply_offseason_motivation(&tx, &seats_before)?;
    merge_motivation_reports(&mut motivation_reports, offseason_reports);

    tx.commit().map_err(|e| {
        let _ = std::fs::remove_file(save_path.join("preseason_plan.json"));
        format!("Falha ao confirmar fim de temporada: {e}")
    })?;

    Ok(EndOfSeasonResult {
        growth_reports,
        motivation_reports,
        retirements,
        rookies_generated,
        new_season_id: new_season.id,
        new_year: new_season.ano,
        licenses_earned,
        promotion_result,
        preseason_initialized,
        preseason_total_weeks,
    })
}
