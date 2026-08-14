//! A casca `#[tauri::command]`: resolve base_dir/config, cacheia e chama o servidor.

use super::*;

/// O pedágio que os três comandos de IA pagavam palavra por palavra antes de fazer
/// qualquer coisa: `app_data_dir` → config → `install_id` → pasta da carreira → banco.
/// Cada endpoint novo recopiava o ritual e escrevia a SUA versão da mensagem de erro,
/// então o mesmo `app_data_dir` ausente aparecia com três textos diferentes no log.
struct PreparoIa {
    /// Identificador da instalação — é por ele que o servidor aplica cooldown e teto de
    /// gasto. O `get_or_create` persiste no config, então o app inteiro conta como um só.
    install_id: String,
    lang: String,
    /// `.../saves/<career_id>`. O debrief precisa dela para achar a tela salva da corrida.
    career_dir: std::path::PathBuf,
    db: Database,
}

/// O config do app, que é onde moram o idioma, o `install_id` e a pasta de saves.
fn config_do_app(app: &tauri::AppHandle) -> Result<AppConfig, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    Ok(AppConfig::load_or_default(&base_dir))
}

/// Abre o banco de uma carreira e devolve também a pasta dela.
fn abrir_carreira(
    config: &AppConfig,
    career_id: &str,
) -> Result<(std::path::PathBuf, Database), String> {
    let career_dir = config.saves_dir().join(career_id);
    let db = Database::open_existing(&career_dir.join("career.db"))
        .map_err(|e| format!("Falha ao abrir banco: {e}"))?;
    Ok((career_dir, db))
}

/// A preparação completa de quem VAI falar com o servidor. Os comandos que só leem o
/// banco (engajamento, resolução de `news_id`) usam `abrir_carreira` direto: criar
/// `install_id` grava no config, e isso é efeito colateral que não cabe a eles.
fn preparar_ia(app: &tauri::AppHandle, career_id: &str) -> Result<PreparoIa, String> {
    let mut config = config_do_app(app)?;
    let install_id = config.get_or_create_install_id();
    let lang = config.language.clone();
    let (career_dir, db) = abrir_carreira(&config, career_id)?;
    Ok(PreparoIa {
        install_id,
        lang,
        career_dir,
        db,
    })
}

/// O que sobrou de uma consulta ao cache protegida pela trava de geração.
enum Vez<T> {
    /// Alguém já gerou (antes, ou enquanto esperávamos a vez).
    Cacheado(T),
    /// Ninguém gerou: o passe é o direito exclusivo de gerar esta chave, e vale
    /// enquanto ele estiver vivo.
    Gerar(crate::narrative::em_voo::Passe),
}

/// O contrato do `narrative::em_voo` numa chamada só: lê o cache; se não tiver, pega o
/// passe e RELÊ — a geração que acabou de sair enquanto esperávamos já gravou o texto.
/// Sem a releitura a trava não serve para nada, e ela estava recopiada em cada comando.
///
/// `usar_cache == false` é o reroll de debug: regenerar é o pedido, então nenhuma das
/// duas leituras acontece e o passe só serializa contra quem estiver gerando agora.
fn cache_ou_passe<T>(
    chave: String,
    usar_cache: bool,
    mut ler_cache: impl FnMut() -> Option<T>,
) -> Vez<T> {
    if usar_cache {
        if let Some(pronto) = ler_cache() {
            return Vez::Cacheado(pronto);
        }
    }
    let passe = crate::narrative::em_voo::aguardar_vez(chave);
    if usar_cache {
        if let Some(pronto) = ler_cache() {
            return Vez::Cacheado(pronto);
        }
    }
    Vez::Gerar(passe)
}

#[tauri::command]
pub async fn enrich_race_news_ai(
    app: tauri::AppHandle,
    career_id: String,
    news_id: String,
    reading_seconds: Option<f64>,
) -> Result<AiNewsResult, String> {
    // async + spawn_blocking: o fetch de IA é bloqueante (até 45s) e travaria a THREAD
    // PRINCIPAL do Tauri se rodasse síncrono — a janela inteira congela ("não está
    // respondendo") enquanto o servidor gera.
    tauri::async_runtime::spawn_blocking(move || {
        let preparo = preparar_ia(&app, &career_id)?;
        let db = &preparo.db;

        let row = ai_story::get_story(&db.conn, &news_id)
            .map_err(|e| format!("Falha ao ler cache do boletim: {e:?}"))?;

        // Sem fatos guardados → não há boletim de IA para esta notícia (ex.: corrida
        // antiga, ou notícia que não é a do jogador). Front usa o template.
        let Some(row) = row else {
            return Ok(AiNewsResult {
                story: None,
                status: AiStatus::Unavailable,
                teams: None,
            });
        };

        // Cores das equipes da corrida (mapa nome→cor). Acompanha story em todo retorno.
        let teams = row
            .teams_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

        // Cache vazio na 1ª leitura NÃO quer dizer que ninguém está gerando: o
        // pré-aquecimento do fim da corrida pode estar no servidor agora mesmo com esta
        // notícia. Espera a vez e RELÊ — ver `narrative::em_voo`.
        let _passe = match cache_ou_passe(
            crate::narrative::em_voo::chave_boletim(&career_id, &news_id),
            true,
            || {
                ai_story::get_story(&db.conn, &news_id)
                    .ok()
                    .flatten()
                    .and_then(|r| r.story)
            },
        ) {
            Vez::Cacheado(story) => {
                return Ok(AiNewsResult {
                    story: Some(story),
                    status: AiStatus::Cached,
                    teams,
                })
            }
            Vez::Gerar(passe) => passe,
        };

        // 1ª vez: gera no servidor e cacheia.
        match client::fetch_story(
            &row.facts,
            &preparo.lang,
            &preparo.install_id,
            reading_seconds,
        ) {
            Ok(story) => {
                if let Err(e) = ai_story::set_story(&db.conn, &news_id, &story) {
                    crate::diagnostico::linha(
                        "narrative",
                        &format!("falha ao cachear o boletim: {e:?}"),
                    );
                }
                Ok(AiNewsResult {
                    story: Some(story),
                    status: AiStatus::Ok,
                    teams,
                })
            }
            Err(StoryError::RateLimited) => Ok(AiNewsResult {
                story: None,
                status: AiStatus::RateLimited,
                teams,
            }),
            // O motivo vinha embrulhado em `StoryError::{Server,Network}` e era
            // descartado aqui — um 5xx (os DOIS provedores caídos) e uma queda de rede
            // chegavam ao jogador como o mesmo "erro" mudo, sem rastro no loop.log.
            Err(err) => {
                client::registrar_falha("boletim de IA", &err);
                Ok(AiNewsResult {
                    story: None,
                    status: AiStatus::Error,
                    teams,
                })
            }
        }
    })
    .await
    .map_err(|e| format!("Falha ao executar boletim de IA: {e}"))?
}

/// O front monta os `facts` do briefing e chama isto ao abrir a tela. Cacheia por
/// `race_id` (reentrar não regenera). Em cooldown/rede/cota → textos `None` e o front
/// usa o template. O cooldown de 10 min entre prévias é imposto pelo servidor
/// (escopo "pre-race", separado do boletim pós-corrida).
#[tauri::command]
pub async fn pre_race_briefing_ai(
    app: tauri::AppHandle,
    career_id: String,
    race_id: String,
    facts: String,
    force: Option<bool>,
) -> Result<PreRaceAiResult, String> {
    // async + spawn_blocking: ver enrich_race_news_ai — fetch bloqueante fora da main.
    tauri::async_runtime::spawn_blocking(move || {
        let force = force.unwrap_or(false);
        let preparo = preparar_ia(&app, &career_id)?;
        let db = &preparo.db;

        // Cache por etapa → reentrar na tela não regenera (sem custo, sem cooldown).
        // No reroll de debug (force) ignoramos o cache e regeramos pelo servidor.
        if !force {
            if let Ok(Some(row)) = ai_pre_race::get_pre_race(&db.conn, &race_id) {
                return Ok(PreRaceAiResult {
                    headline: Some(row.headline),
                    narrative: Some(row.narrative),
                    team_voice: Some(row.team_voice),
                    status: AiStatus::Cached,
                });
            }

            // Gate de engajamento: se o jogador não vem lendo a prévia, segura/alterna no
            // template para não gastar IA (sem tocar no servidor). O reroll (force) pula.
            let streak = meta::get_meta_value(&db.conn, PRE_RACE_STREAK_KEY)
                .ok()
                .flatten()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            if !pre_race_use_ai(streak) {
                return Ok(PreRaceAiResult {
                    headline: None,
                    narrative: None,
                    team_voice: None,
                    status: AiStatus::EngagementTemplate,
                });
            }
        }

        // O prefetch da animação de avanço e a Sala de Estratégia pedem esta MESMA etapa,
        // e a Sala dispara justamente quando o prefetch ainda não voltou (servidor frio).
        // Espera a vez e relê — ver `narrative::em_voo`. No reroll (`force`) o passe só
        // serializa: regenerar é o pedido, então não relemos o cache.
        let _passe = match cache_ou_passe(
            crate::narrative::em_voo::chave_pre_corrida(&career_id, &race_id),
            !force,
            || ai_pre_race::get_pre_race(&db.conn, &race_id).ok().flatten(),
        ) {
            Vez::Cacheado(row) => {
                return Ok(PreRaceAiResult {
                    headline: Some(row.headline),
                    narrative: Some(row.narrative),
                    team_voice: Some(row.team_voice),
                    status: AiStatus::Cached,
                })
            }
            Vez::Gerar(passe) => passe,
        };

        if facts.trim().is_empty() {
            return Ok(PreRaceAiResult {
                headline: None,
                narrative: None,
                team_voice: None,
                status: AiStatus::Unavailable,
            });
        }

        // Arco narrativo entre etapas: anexa a memória das últimas corridas (chegada +
        // manchete do debrief, e o corpo do debrief anterior) para o engenheiro retomar de
        // onde parou. Vazio na 1ª corrida da carreira/categoria → briefing inalterado.
        let facts = {
            let arc = build_recent_arc_facts(&db.conn, &race_id);
            if arc.trim().is_empty() {
                facts
            } else {
                format!("{facts}\n{arc}")
            }
        };

        match client::fetch_pre_race_briefing(&facts, &preparo.lang, &preparo.install_id, force) {
            Ok(b) => {
                // O corpo cinematográfico é guardado na coluna `narrative` do cache.
                if let Err(e) = ai_pre_race::set_pre_race(
                    &db.conn,
                    &race_id,
                    &b.headline,
                    &b.body,
                    &b.team_voice,
                ) {
                    crate::diagnostico::linha(
                        "narrative",
                        &format!("falha ao cachear a prévia pré-corrida: {e:?}"),
                    );
                }
                Ok(PreRaceAiResult {
                    headline: Some(b.headline),
                    narrative: Some(b.body),
                    team_voice: Some(b.team_voice),
                    status: AiStatus::Ok,
                })
            }
            Err(StoryError::RateLimited) => Ok(PreRaceAiResult {
                headline: None,
                narrative: None,
                team_voice: None,
                status: AiStatus::RateLimited,
            }),
            Err(err) => {
                client::registrar_falha("prévia pré-corrida", &err);
                Ok(PreRaceAiResult {
                    headline: None,
                    narrative: None,
                    team_voice: None,
                    status: AiStatus::Error,
                })
            }
        }
    })
    .await
    .map_err(|e| format!("Falha ao executar prévia pré-corrida: {e}"))?
}

/// Reporta se o jogador LEU a prévia pré-corrida (ficou tempo suficiente na Sala de
/// Estratégia) e atualiza a sequência de "não-leu" no `meta`. Leu → zera; não leu →
/// +1 (limitado). O front chama isto ao simular/sair da tela. Devolve a sequência
/// nova (útil só para debug). Falha de IO vira erro string, mas o front ignora.
#[tauri::command]
pub fn report_pre_race_engagement(
    app: tauri::AppHandle,
    career_id: String,
    read: bool,
) -> Result<i64, String> {
    let (_dir, db) = abrir_carreira(&config_do_app(&app)?, &career_id)?;

    let streak = meta::get_meta_value(&db.conn, PRE_RACE_STREAK_KEY)
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let next = if read { 0 } else { (streak + 1).min(10) };
    meta::put_meta_value(&db.conn, PRE_RACE_STREAK_KEY, &next.to_string())
        .map_err(|e| format!("Falha ao gravar engajamento: {e:?}"))?;
    Ok(next)
}

/// Comando lazy do debrief pós-corrida: o front chama quando o jogador abre a aba
/// Debrief. Monta os fatos no Rust, manda ao servidor (voz única do engenheiro) e
/// cacheia por `race_id` — reabrir não regenera. Sem gate de engajamento (o jogador
/// sempre olha o resultado). Em qualquer falha devolve `None` e o front usa o
/// texto determinístico do cérebro (nunca quebra).
#[tauri::command]
pub async fn post_race_debrief_ai(
    app: tauri::AppHandle,
    career_id: String,
    race_id: String,
    force: Option<bool>,
) -> Result<PostRaceAiResult, String> {
    // async + spawn_blocking: ver enrich_race_news_ai. Este é o pior caso — a tela de
    // resultado chama assim que abre, e com o engenheiro "no rádio" o jogador ficava
    // olhando a janela congelada até o servidor responder.
    tauri::async_runtime::spawn_blocking(move || {
        let force = force.unwrap_or(false);
        let preparo = preparar_ia(&app, &career_id)?;
        let db = &preparo.db;

        // Antes de montar os fatos (que varrem o resultado inteiro): espera uma geração
        // desta etapa que já esteja em voo e relê — ver `narrative::em_voo`.
        let _passe = match cache_ou_passe(
            crate::narrative::em_voo::chave_pos_corrida(&career_id, &race_id),
            !force,
            || {
                ai_post_race::get_post_race(&db.conn, &race_id)
                    .ok()
                    .flatten()
            },
        ) {
            Vez::Cacheado(row) => {
                return Ok(PostRaceAiResult {
                    headline: Some(row.headline),
                    body: Some(row.body),
                    status: AiStatus::Cached,
                })
            }
            Vez::Gerar(passe) => passe,
        };

        let facts = build_post_race_facts(&db.conn, &preparo.career_dir, &race_id);
        if facts.trim().is_empty() {
            return Ok(PostRaceAiResult {
                headline: None,
                body: None,
                status: AiStatus::Unavailable,
            });
        }

        match client::fetch_post_race_debrief(&facts, &preparo.lang, &preparo.install_id, force) {
            Ok(d) => {
                if let Err(e) =
                    ai_post_race::set_post_race(&db.conn, &race_id, &d.headline, &d.body)
                {
                    crate::diagnostico::linha(
                        "narrative",
                        &format!("falha ao cachear o debrief pós-corrida: {e:?}"),
                    );
                }
                Ok(PostRaceAiResult {
                    headline: Some(d.headline),
                    body: Some(d.body),
                    status: AiStatus::Ok,
                })
            }
            Err(StoryError::RateLimited) => Ok(PostRaceAiResult {
                headline: None,
                body: None,
                status: AiStatus::RateLimited,
            }),
            Err(err) => {
                client::registrar_falha("debrief pós-corrida", &err);
                Ok(PostRaceAiResult {
                    headline: None,
                    body: None,
                    status: AiStatus::Error,
                })
            }
        }
    })
    .await
    .map_err(|e| format!("Falha ao executar debrief pós-corrida: {e}"))?
}

/// Resolve o `news_id` da notícia de Corrida do JOGADOR para uma (temporada, rodada).
///
/// A revista (NewsMagazineTab) monta as edições a partir do calendário; para puxar o
/// boletim de IA de cada etapa ela precisa do `news_id` correspondente. Como os fatos
/// de IA só são guardados para a corrida do jogador (uma por rodada), basta cruzar
/// `ai_race_story` com `news` pela rodada da temporada. Devolve `None` quando não há
/// boletim para a etapa (ex.: corrida simulada antes do recurso existir) → o front
/// usa o texto-placeholder.
#[tauri::command]
pub fn player_race_news_id(
    app: tauri::AppHandle,
    career_id: String,
    season_id: String,
    rodada: i32,
) -> Result<Option<String>, String> {
    let (_dir, db) = abrir_carreira(&config_do_app(&app)?, &career_id)?;

    // Se a tabela ai_race_story ainda não existe (save sem nenhum boletim), a query
    // falha — tratamos como "sem boletim" em vez de erro.
    let id = db
        .conn
        .query_row(
            "SELECT n.id
               FROM news n
               JOIN ai_race_story a ON a.news_id = n.id
              WHERE n.temporada_id = ?1 AND n.rodada = ?2
              LIMIT 1",
            rusqlite::params![season_id, rodada],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .unwrap_or(None);

    Ok(id)
}
