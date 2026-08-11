//! Perfil adaptativo de dificuldade do jogador (por custid) e o processamento da última corrida.

use super::*;

/// Perfil ADAPTATIVO de dificuldade do JOGADOR (não do save): nível geral + por
/// pista, vs a baseline universal (os offsets por pista). Guardado por `custid`
/// (conta do iRacing) em `app_data/iracing_adaptive/<custid>.json`, então uma
/// carreira NOVA já nasce calibrada ao jogador — ele não recalibra do zero.
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct AdaptiveProfile {
    /// Delta global: nível geral do jogador vs a baseline.
    pub global: i64,
    /// Delta por pista (`track_id` como string → delta): aptidão por circuito.
    pub tracks: std::collections::HashMap<String, i64>,
}

impl AdaptiveProfile {
    /// Delta acumulado do jogador para uma pista (0 se nunca correu nela).
    pub fn track_delta(&self, track_id: i64) -> i64 {
        self.tracks.get(&track_id.to_string()).copied().unwrap_or(0)
    }
}

pub(crate) fn adaptive_profile_path(base_dir: &std::path::Path, custid: i64) -> std::path::PathBuf {
    base_dir
        .join("iracing_adaptive")
        .join(format!("{custid}.json"))
}

/// Carrega o perfil adaptativo do jogador (vazio se ainda não existe).
pub(crate) fn load_adaptive_profile(base_dir: &std::path::Path, custid: i64) -> AdaptiveProfile {
    std::fs::read_to_string(adaptive_profile_path(base_dir, custid))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persiste o perfil adaptativo do jogador.
pub(crate) fn save_adaptive_profile(
    base_dir: &std::path::Path,
    custid: i64,
    profile: &AdaptiveProfile,
) -> Result<(), String> {
    let path = adaptive_profile_path(base_dir, custid);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(profile).map_err(|e| format!("Falha ao serializar: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Falha ao gravar: {e}"))
}

/// Contexto de carro do ÚLTIMO export (por custid), pro mecanismo 2 (adaptativo cego ao
/// carro). O export sabe os carros e os NÚMEROS; o pós-corrida casa a frente
/// (`car_idx`→número via `cars_meta`) e desconta do ritmo o que o carro explica. Persistido
/// junto do perfil adaptativo. Ver [`crate::iracing_sdk::car_difficulty`].
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct CarDifficultyContext {
    /// Pista alvo do export (o pós-corrida só usa se casar com a pista corrida).
    pub(crate) track_id: i64,
    /// Vantagem de carro do jogador (car-perf) na pista.
    pub(crate) player_advantage: f64,
    /// número do carro (string) → vantagem de carro (car-perf) na pista.
    pub(crate) by_number: std::collections::HashMap<String, f64>,
    /// Quando o export gravou este bilhete (segundos desde a época). `0` em arquivo
    /// gravado antes deste campo existir. Ver [`postit_esta_fresco`].
    #[serde(default)]
    pub(crate) gravado_em_unix: i64,
}

/// Validade de um "post-it" do export (contexto de carro, faixa de skill).
///
/// Estes arquivos são bilhetes que uma etapa do fluxo deixa para a próxima: o roster
/// escreve, a temporada e o pós-corrida leem. A validade era conferida só por
/// categoria + pista, sem carimbo de tempo — então um bilhete deixado por um fluxo
/// INTERROMPIDO dias atrás, na mesma pista e categoria, era lido como se fosse de agora e
/// produzia banda de skill errada sem nenhum aviso.
///
/// Um dia é o teto: um fim de semana de corrida no Loop (exportar, correr, importar) cabe
/// numa sessão, inclusive um enduro de seis horas. Bilhete de ontem é de outro fluxo.
const VALIDADE_DO_POSTIT_SEGS: i64 = 24 * 60 * 60;

/// Agora, em segundos desde a época. `0` quando o relógio do sistema está antes de 1970 —
/// caso em que o carimbo simplesmente não filtra nada (ver [`postit_esta_fresco`]).
pub(crate) fn agora_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// O carimbo ainda vale? `0` (post-it gravado antes de este campo existir) passa: recusar
/// um bilhete legítimo é pior do que aceitar um velho, e o log diz qual é o caso.
pub(crate) fn postit_esta_fresco(gravado_em_unix: i64, rotulo: &str) -> bool {
    if gravado_em_unix <= 0 {
        return true;
    }
    let idade = agora_unix() - gravado_em_unix;
    if idade > VALIDADE_DO_POSTIT_SEGS {
        crate::diagnostico::linha(
            "iracing",
            &format!(
                "{rotulo}: registro do export tem {} h — velho demais, ignorado \
                 (exporte a etapa de novo)",
                idade / 3600
            ),
        );
        return false;
    }
    true
}

pub(crate) fn car_difficulty_context_path(
    base_dir: &std::path::Path,
    custid: i64,
) -> std::path::PathBuf {
    base_dir
        .join("iracing_adaptive")
        .join(format!("{custid}_car.json"))
}

/// Persiste o contexto de carro do export (best-effort; erro só é logado pelo chamador).
pub(crate) fn save_car_difficulty_context(
    base_dir: &std::path::Path,
    custid: i64,
    ctx: &CarDifficultyContext,
) -> Result<(), String> {
    let path = car_difficulty_context_path(base_dir, custid);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(ctx).map_err(|e| format!("Falha ao serializar: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Falha ao gravar: {e}"))
}

/// Lê o contexto de carro do último export. `None` se não existe ou se o bilhete é velho
/// demais para ser deste fluxo (ver [`postit_esta_fresco`]).
pub(crate) fn load_car_difficulty_context(
    base_dir: &std::path::Path,
    custid: i64,
) -> Option<CarDifficultyContext> {
    std::fs::read_to_string(car_difficulty_context_path(base_dir, custid))
        .ok()
        .and_then(|s| serde_json::from_str::<CarDifficultyContext>(&s).ok())
        .filter(|c| postit_esta_fresco(c.gravado_em_unix, "contexto de carro"))
}

/// Faixa de skill EFETIVA do último roster exportado — o post-it que fecha o ciclo entre
/// os dois arquivos do iRacing.
///
/// O roster grava `driverSkill` normalizado em 0–100 e o sim ESTICA esse roster para
/// preencher o `minSkill`/`maxSkill` da temporada. Se a temporada calcular a faixa por
/// conta própria (a partir das skills CRUAS), tudo o que só existe no roster (pressão,
/// forma, acerto do dia, chuva, carro, rivalidade) é distorcido pelo esticão e apagado de
/// vez em quem está na ponta e no fundo do grid. Com a faixa vindo daqui, o esticão vira a
/// identidade. O roster é sempre exportado ANTES da temporada no fluxo do botão Correr.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ExportSkillBand {
    /// Categoria do export (a temporada só usa se casar com a dela).
    pub(crate) categoria: String,
    /// Pista alvo do export (idem — post-it de outra pista é post-it velho).
    pub(crate) track_id: i64,
    /// Menor e maior skill PRETENDIDA do grid, na escala efetiva (vai até 125).
    pub(crate) min: f64,
    pub(crate) max: f64,
    /// Quando o roster gravou este bilhete (segundos desde a época). `0` em arquivo
    /// gravado antes deste campo existir. Ver [`postit_esta_fresco`].
    #[serde(default)]
    pub(crate) gravado_em_unix: i64,
}

pub(crate) fn export_skill_band_path(
    base_dir: &std::path::Path,
    custid: i64,
) -> std::path::PathBuf {
    base_dir
        .join("iracing_adaptive")
        .join(format!("{custid}_band.json"))
}

/// Persiste a faixa efetiva do roster (best-effort; erro só é logado pelo chamador).
pub(crate) fn save_export_skill_band(
    base_dir: &std::path::Path,
    custid: i64,
    band: &ExportSkillBand,
) -> Result<(), String> {
    let path = export_skill_band_path(base_dir, custid);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(band).map_err(|e| format!("Falha ao serializar: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Falha ao gravar: {e}"))
}

/// Lê a faixa efetiva do último roster exportado. `None` se não existe ou se o bilhete é
/// velho demais para ser deste fluxo (ver [`postit_esta_fresco`]).
pub(crate) fn load_export_skill_band(
    base_dir: &std::path::Path,
    custid: i64,
) -> Option<ExportSkillBand> {
    std::fs::read_to_string(export_skill_band_path(base_dir, custid))
        .ok()
        .and_then(|s| serde_json::from_str::<ExportSkillBand>(&s).ok())
        .filter(|b| postit_esta_fresco(b.gravado_em_unix, "faixa de skill do roster"))
}

/// Resultado do processamento adaptativo pós-corrida (para a UI).
#[derive(serde::Serialize)]
pub struct AdaptiveResult {
    /// Se a corrida foi válida para adaptar (false = DNF/dados insuficientes).
    pub applied: bool,
    /// Explicação legível ("Dominou → sobe", "Trânsito → mantém", etc.).
    pub verdict: String,
    pub d_global: i64,
    pub d_track: i64,
    /// Deltas resultantes do jogador (já com piso/teto).
    pub global: i64,
    pub track: i64,
    pub track_id: i64,
    pub track_name: Option<String>,
}

/// Processa o resultado da ÚLTIMA corrida e atualiza o perfil adaptativo do
/// jogador (por `custid`). Só aplica em corrida limpa do jogador.
///
/// Chamado de dentro do import automático ([`super::iracing_auto_import_if_ready`]),
/// não pelo frontend: o ajuste tem de acontecer sempre que uma corrida entra na
/// carreira, e o jogador NÃO deve percebê-lo — se perceber, passa a duvidar dos
/// próprios resultados. Ficou anos preso a um painel desligado; o perfil nunca era
/// escrito e o `ai_sweet_spot` lia sempre zero (ver `docs/iracing-escopo.md` §4).
#[tauri::command]
pub fn iracing_process_race_result(app: tauri::AppHandle) -> Result<AdaptiveResult, String> {
    use crate::constants::tracks::get_track;
    use crate::iracing_sdk::{adaptive, race_monitor};
    use tauri::Manager;

    let history = race_monitor::get_history();
    if !history.finished {
        return Err("A corrida ainda não encerrou.".to_string());
    }
    let track_id = history.track_id;
    if track_id <= 0 {
        return Err("Pista da corrida não identificada (sem TrackID na sessão).".to_string());
    }
    // Idempotência: a MESMA tentativa nunca ajusta duas vezes. O histórico fica
    // `finished` na memória até a próxima sessão começar, então qualquer segunda
    // chamada nesse intervalo (invocação manual, código futuro religando um painel)
    // reaplicaria o passo inteiro — +5 virava +10 sem ninguém perceber. A chave
    // (subsession, tentativa) é única dentro de uma execução do app, que é
    // exatamente o tempo de vida do histórico em memória.
    static ULTIMA_PROCESSADA: std::sync::Mutex<Option<(i64, i32)>> = std::sync::Mutex::new(None);
    let chave = (history.subsession_id, history.attempt_number);
    if ULTIMA_PROCESSADA.lock().ok().and_then(|u| *u) == Some(chave) {
        return Err(format!(
            "Corrida já processada (tentativa {}): o ajuste não se aplica duas vezes.",
            history.attempt_number
        ));
    }
    let custid = iracing_sdk::cached_custid().unwrap_or(0);
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let mut profile = load_adaptive_profile(&base_dir, custid);

    let current = adaptive::Deltas {
        global: profile.global,
        track: profile.track_delta(track_id),
    };
    // Regime RÁPIDO (posição+gravidade+RITMO LIMPO) em TODOS os tiers. Reusa
    // build_adaptive_result (voltas por carro) → fast_result_from descarta voltas de erro,
    // então uma rodada não baixa a dificuldade. O regime por ritmo completo
    // (compute_adaptive_update) segue dormente — mantido no código pra revisitar.
    let race = race_monitor::build_adaptive_result(&history, track_id);
    // Mecanismo 2 (cego ao carro): carrega o contexto de carro do último export e casa a
    // frente (car_idx→número via cars_meta → vantagem). Só usa se a pista bater. Sem contexto
    // ou pista diferente → None → comportamento antigo (adaptativo puro por ritmo).
    let car_ctx = load_car_difficulty_context(&base_dir, custid)
        .filter(|c| c.track_id == track_id)
        .map(|c| {
            let by_idx = history
                .cars_meta
                .iter()
                .filter_map(|m| {
                    c.by_number
                        .get(&m.car_number.to_string())
                        .map(|adv| (m.idx, *adv))
                })
                .collect();
            adaptive::CarContext {
                player_advantage: c.player_advantage,
                by_idx,
            }
        });
    let summary = adaptive::fast_result_from(&race, car_ctx.as_ref());
    let update = adaptive::compute_fast_update(&summary, &current);
    // Rastro no log de diagnóstico. O jogador não vê o ajuste (é de propósito), então sem
    // esta linha não há COMO saber se ele rodou, o que mediu e por que decidiu — o perfil só
    // é gravado quando a agulha se move, e "arquivo ausente" é ambíguo entre "nunca rodou" e
    // "rodou e ficou no escudo". Registra também o recorte de CLASSE, que é o que responde
    // se o multiclasse comparou os carros certos.
    let classe = race
        .race
        .iter()
        .find(|d| d.is_player)
        .map(|d| d.car_class_id)
        .unwrap_or(0);
    let ias_na_classe = race
        .race
        .iter()
        .filter(|d| d.is_ai && d.car_class_id == classe)
        .count();
    let ritmo = match summary.pace_vs_front {
        Some(g) => format!("{:+.2}%/volta", g * 100.0),
        None => "sem amostra".to_string(),
    };
    crate::diagnostico::linha(
        "adaptativo",
        &format!(
            "Pista {track_id} · classe {classe}: {ias_na_classe} IA de {} carros · carro {} · ritmo vs frente {ritmo} · {} · global {}{:+}={} · pista {}{:+}={} · {}",
            race.race.len(),
            if car_ctx.is_some() { "sim" } else { "não" },
            update.verdict,
            current.global,
            update.d_global,
            update.new.global,
            current.track,
            update.d_track,
            update.new.track,
            if update.applied {
                "gravado"
            } else {
                "sem mudança"
            }
        ),
    );
    if update.applied {
        profile.global = update.new.global;
        profile
            .tracks
            .insert(track_id.to_string(), update.new.track);
        save_adaptive_profile(&base_dir, custid, &profile)?;
    }
    // Só marca DEPOIS do save: se a gravação falhar, a corrida continua elegível
    // para uma nova tentativa de processamento (o Err sobe e o import loga).
    if let Ok(mut ultima) = ULTIMA_PROCESSADA.lock() {
        *ultima = Some(chave);
    }
    Ok(AdaptiveResult {
        applied: update.applied,
        verdict: update.verdict,
        d_global: update.d_global,
        d_track: update.d_track,
        global: profile.global,
        track: profile.track_delta(track_id),
        track_id,
        track_name: get_track(track_id as u32).map(|t| t.nome.to_string()),
    })
}
