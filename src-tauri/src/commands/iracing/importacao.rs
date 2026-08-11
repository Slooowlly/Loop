//! Import automático da corrida do iRacing para a carreira.

use super::*;

/// Resultado de um import automático: o `RaceResult` (para a TELA de resultado) +
/// o resumo (para o pop-up de conserto).
#[derive(serde::Serialize)]
pub struct AutoImportResult {
    pub race_result: crate::simulation::race::RaceResult,
    pub summary: crate::commands::race::ImportedRaceSummary,
    /// Avaliação de carreira (expectativa vs resultado, nota, frases). `None` se
    /// não der para avaliar — a tela trata e nunca quebra.
    pub evaluation: Option<crate::race_eval::RaceEvaluation>,
    /// Análise de telemetria (ritmo, consistência, rival). Vazia se não houve
    /// telemetria (jogador saiu cedo / não monitorado).
    pub telemetry: crate::iracing_sdk::telemetry_analysis::TelemetryAnalysis,
}

/// GATILHO AUTOMÁTICO: chamado em loop pelo front. Se o iRacing já gravou o
/// resultado da próxima corrida pendente (jogador terminou/saiu da corrida),
/// IMPORTA para a carreira e devolve o resultado + resumo para a tela abrir
/// sozinha. Se ainda não há resultado pronto (ou nada a importar), devolve `None`
/// — SEM erro, para o poller não fazer barulho. Idempotente: após importar, a
/// corrida vira Concluída e a próxima pendente ainda não terá resultado.
#[tauri::command]
pub fn iracing_auto_import_if_ready(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Option<AutoImportResult>, String> {
    // "Não está pronto / nada a importar" não é erro: o resultado só existe depois
    // que o jogador termina/sai da corrida no iRacing. Qualquer falha de "ainda
    // não" vira None silencioso; o poller tenta de novo no próximo tick.
    let (
        mut db,
        career_dir,
        track_id,
        player_crash,
        result,
        telemetry,
        history,
        by_number,
        player_impact_dir,
        player_style,
        player_quali_crash,
    ) = match build_session_race_result(&app, &career_id) {
        Ok(v) => v,
        Err(e) => {
            // O poller engole este Err de propósito (ainda-não-pronto não é erro),
            // mas "corrida terminou e nada aconteceu" era invisível: o motivo real
            // (post-it do export sumiu, resultado ainda não gravado, pista não bate)
            // morria aqui. `linha_unica` registra só a TRANSIÇÃO de motivo — uma
            // linha por estado, não uma por tick de poll.
            crate::diagnostico::linha_unica("import", &format!("aguardando: {e}"));
            return Ok(None);
        }
    };
    // Peça 3: drena os desfechos de quebra (one-shot, só aqui) e resolve para driver_id.
    let breakdowns = resolve_breakdown_rows(&db.conn, &history, &by_number);
    let (summary, race_result) = crate::commands::race::import_iracing_race_result(
        &mut db,
        &career_dir,
        track_id,
        &player_crash,
        &player_impact_dir,
        result,
        &telemetry,
        &history,
        // Estilo neutro (sem sinal capturado) → None, pra não pagar a query de time à toa.
        (!player_style.is_neutral()).then_some(player_style),
        breakdowns,
        (&player_quali_crash.severidade, &player_quali_crash.direcao),
    )?;
    // Fecha o par com o "aguardando" acima: import concluído fica registrado com a
    // corrida e a pista, e é a linha que antecede o rastro do adaptativo logo abaixo.
    // Pelo MESMO canal do "aguardando" (linha_unica) de propósito: é o que renova a
    // memória da categoria e deixa o próximo "aguardando" idêntico ser logado de novo.
    crate::diagnostico::linha_unica(
        "import",
        &format!("Corrida importada: {} (pista {track_id})", summary.race_id),
    );

    // Ponte de rivalidade de pista: aplica no motor as rivalidades percebidas do SDK
    // nesta corrida (só o jogador). Atrás da flag IRACER_TRACK_RIVALRY e best-effort —
    // nunca desfaz o import. Idempotente por construção: só roda após um import
    // bem-sucedido (a corrida deixa de ser a pendente e não é reimportada).
    if std::env::var("IRACER_TRACK_RIVALRY").is_ok() {
        if let Ok(Some(entry)) =
            crate::db::queries::calendar::get_calendar_entry_by_id(&db.conn, &summary.race_id)
        {
            apply_track_rivalries(
                &db.conn,
                &history,
                &by_number,
                &race_result,
                entry.rodada,
                &entry.categoria,
            );
        }
    }

    // Clima da corrida importada: resolve+persiste pela fonte única (mesmo do export).
    if let Some(track) = crate::constants::tracks::get_track(track_id as u32) {
        if let Ok(Some(entry)) =
            crate::db::queries::calendar::get_calendar_entry_by_id(&db.conn, &summary.race_id)
        {
            let _ = resolve_and_persist_race_weather(
                &db.conn,
                &career_id,
                track,
                entry.week_of_year,
                &summary.race_id,
                false,
                1.0, // corrida do JOGADOR: sem viés de chuva
            );
        }
    }
    // DIFICULDADE ADAPTATIVA: calibra o nível da IA ao jogador (perfil por custid, global
    // + por pista) que o `ai_sweet_spot` lê na PRÓXIMA geração de roster/temporada. Roda
    // aqui, e não na UI, porque o ajuste tem de acontecer sempre que uma corrida entra na
    // carreira — e em SILÊNCIO: o jogador não deve perceber, senão duvida dos próprios
    // resultados. Idempotente pelo mesmo motivo do resto deste bloco (só corre depois de
    // um import bem-sucedido). Best-effort: corrida sem dados suficientes devolve Err e é
    // engolido — nunca desfaz o import.
    // O Err é engolido de propósito, mas vai pro log: a causa mais provável de "a
    // dificuldade não mexeu" é o monitor não ter o histórico vivo (app reaberto entre
    // correr e importar), e sem esta linha isso é invisível.
    if let Err(e) = iracing_process_race_result(app.clone()) {
        crate::diagnostico::linha("adaptativo", &format!("Sem ajuste: {e}"));
    }

    // Telemetria de produto: fecha o bloco de leitura da rodada anterior e abre o
    // desta. O `race_end` desta corrida já saiu lá do `race_monitor`, na bandeirada;
    // aqui é só a janela de leitura em volta dela. Best-effort como o resto do bloco.
    if let Ok(Some(entry)) =
        crate::db::queries::calendar::get_calendar_entry_by_id(&db.conn, &summary.race_id)
    {
        if let Ok(Some(temporada)) = crate::db::queries::seasons::get_active_season(&db.conn) {
            crate::telemetry::uso_virar_rodada(
                temporada.numero as i32,
                entry.rodada as i32,
                &entry.categoria,
            );
        }
    }

    let evaluation = crate::commands::race::compute_race_evaluation(&db.conn, &race_result);

    // Persiste a tela completa (resultado + avaliação + telemetria/gráficos) para
    // o jogador reabrir a classificação final depois pela Home.
    crate::commands::race::save_race_screen(
        &career_dir,
        &summary.race_id,
        &serde_json::json!({
            "race_result": &race_result,
            "evaluation": &evaluation,
            "telemetry": &telemetry,
            "maintenance": &summary.maintenance,
            "event_repercussion": &summary.event_repercussion,
        }),
    );

    Ok(Some(AutoImportResult {
        race_result,
        summary,
        evaluation,
        telemetry,
    }))
}
