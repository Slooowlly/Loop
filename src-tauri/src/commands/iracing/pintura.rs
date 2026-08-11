//! Pintura do carro do jogador: exibição do esquema, aplicação automática e vínculo do custid.

use super::*;

/// Pintura que o JOGADOR deve aplicar na garagem do iRacing para ficar na cor do
/// time (igual à IA). A pintura embutida do jogador é account-side, então só dá
/// para MOSTRAR o esquema certo — o usuário aplica uma vez.
#[derive(serde::Serialize)]
pub struct PlayerPaint {
    pub team_name: String,
    pub pattern: String,
    pub color1: String,
    pub color2: String,
    pub color3: String,
    pub spec: String,
}

/// Lê o time do jogador na carreira e devolve o esquema de pintura a aplicar.
#[tauri::command]
pub fn iracing_player_paint(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<PlayerPaint, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
    use crate::iracing_sdk::roster_gen;
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let player =
        dq::get_player_driver(&db.conn).map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let team = cq::get_active_contract_for_pilot(&db.conn, &player.id)
        .ok()
        .flatten()
        .and_then(|contract| {
            tq::get_team_by_id(&db.conn, &contract.equipe_id)
                .ok()
                .flatten()
        })
        .ok_or("Você não tem contrato/time ativo nesta carreira.")?;

    let hex = roster_gen::normalize_hex(&team.cor_primaria);
    Ok(PlayerPaint {
        team_name: team.nome,
        pattern: roster_gen::DESIGN_PATTERN.to_string(),
        color1: format!("#{hex}"),
        color2: format!("#{}", roster_gen::DESIGN_COLOR2),
        color3: format!("#{}", roster_gen::DESIGN_COLOR3),
        spec: format!(
            "{},{hex},{},{}",
            roster_gen::DESIGN_PATTERN,
            roster_gen::DESIGN_COLOR2,
            roster_gen::DESIGN_COLOR3
        ),
    })
}

/// Resultado da pintura automática do carro do jogador.
#[derive(serde::Serialize)]
pub struct ApplyPaintResult {
    pub path: String,
    pub custid: i64,
    pub color: String,
}

/// Escreve a pintura (cor sólida do time) do carro do jogador como custom paint
/// do iRacing: `paint/<carro>/car_<custid>.tga`. Usa o custid já capturado.
/// Disparo MANUAL do painel de diagnóstico, então ignora o interruptor das
/// Configurações de propósito: quem clicou pediu a pintura agora.
#[tauri::command]
pub fn iracing_apply_player_paint(
    app: tauri::AppHandle,
    career_id: String,
    car_key: String,
) -> Result<ApplyPaintResult, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
    use tauri::Manager;

    let custid = iracing_sdk::cached_custid()
        .ok_or("Ainda não capturei seu custid — abra o iRacing e entre numa sessão uma vez.")?;

    // Cor do time do jogador.
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;
    let player =
        dq::get_player_driver(&db.conn).map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let team = cq::get_active_contract_for_pilot(&db.conn, &player.id)
        .ok()
        .flatten()
        .and_then(|c| tq::get_team_by_id(&db.conn, &c.equipe_id).ok().flatten())
        .ok_or("Você não tem contrato/time ativo nesta carreira.")?;

    let (path, color) = write_player_car_tga(&car_key, &team.cor_primaria, custid)?;
    Ok(ApplyPaintResult {
        path,
        custid,
        color,
    })
}

/// Chave da tabela `meta` (career.db) que guarda o custid do iRacing do jogador
/// VINCULADO a este save. Capturado uma vez (popup da 1ª corrida) e reutilizado
/// para repintar o carro automaticamente a cada troca de equipe no mercado.
const PLAYER_CUSTID_META_KEY: &str = "player_iracing_custid";

/// Mapeia a categoria da carreira no carro do iRacing (mesma regra do export).
fn car_key_for_category(categoria: &str) -> &'static str {
    let c = categoria.to_lowercase();
    if c.contains("gr86") || c.contains("toyota") {
        "gr86"
    } else if c.contains("bmw") || c.contains("m2") {
        "bmwm2"
    } else {
        "mx5" // mazda mx-5 e padrão
    }
}

/// Sufixo do backup da pintura que já existia na pasta do iRacing. Guardado UMA
/// vez, antes da primeira escrita nossa: aquele `.tga` pode ser arquivo do
/// jogador (pintura baixada do Trading Paints ou feita à mão) e sobrescrever sem
/// preservar apagaria trabalho dele de forma definitiva. Existindo o backup, ele
/// é o original — as repinturas seguintes são todas nossas e não o substituem.
const PAINT_BACKUP_SUFFIX: &str = "tga.loop-bak";

/// As chaves de carro que o Loop sabe pintar (as do conteúdo grátis, ver
/// [`roster_gen::car_spec`]). O desfazer varre todas: uma carreira longa passa por
/// mais de uma categoria, e cada troca deixou um `.tga` numa pasta diferente.
const CARROS_PINTAVEIS: [&str; 3] = ["mx5", "gr86", "bmwm2"];

/// Escreve `car_<custid>.tga` (cor sólida `hex`) na pasta de pintura do carro.
/// Núcleo compartilhado por todos os caminhos de pintura. Recebe o `hex` já
/// normalizado pelo chamador.
///
/// Preserva a pintura anterior antes de escrever (ver [`PAINT_BACKUP_SUFFIX`]).
/// A falha ao preservar ABORTA a escrita: perder o arquivo do jogador é pior que
/// ficar sem a cor da equipe.
fn write_player_car_tga(car_key: &str, hex: &str, custid: i64) -> Result<(String, String), String> {
    use crate::iracing_sdk::{paint_gen, paths, roster_gen};
    let car =
        roster_gen::car_spec(car_key).ok_or_else(|| format!("Carro desconhecido: {car_key}"))?;
    let hex = roster_gen::normalize_hex(hex);
    let dir = paths::car_paint_dir(car.car_path)
        .ok_or("Não foi possível localizar a pasta de pintura do iRacing.")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar pasta: {e}"))?;
    let path = dir.join(format!("car_{custid}.tga"));
    if path.is_file() {
        let backup = path.with_extension(PAINT_BACKUP_SUFFIX);
        if !backup.exists() {
            std::fs::copy(&path, &backup)
                .map_err(|e| format!("Falha ao preservar a pintura que já estava lá: {e}"))?;
        }
    }
    paint_gen::write_solid_tga(&path, &hex).map_err(|e| format!("Falha ao gravar pintura: {e}"))?;
    Ok((path.display().to_string(), format!("#{hex}")))
}

// ---------------------------------------------------------------------------
// Desfazer
// ---------------------------------------------------------------------------
//
// A pintura entra sem perguntar (ver [`iracing_auto_paint_player`]), e isso só é
// defensável porque a volta existe e é um clique. O arquivo é do jogador, não do
// Loop: automatizar a ida sem oferecer a volta seria decidir por ele dentro da pasta
// dele. É o mesmo par de `iracing_sdk::modo_janela`, que aplica no boot e restaura
// pelo `iracing_modo_janela_restaurar`.
//
// **Com o iRacing aberto.** Diferente dos `renderer*.ini`, aqui NÃO se bloqueia. O sim
// não reescreve os `.tga` ao fechar: ele os LÊ ao carregar o carro numa sessão. Escrever
// com ele aberto é seguro e não se perde — o que muda é só quando a cor aparece, que é
// na próxima vez que o carro for carregado. Bloquear aqui cobraria do jogador um
// fechamento de simulador por uma diferença que ele veria sozinho na volta seguinte.

/// O que aconteceu com um arquivo de pintura no desfazer.
#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EstadoDoDesfazer {
    /// Havia backup: a pintura que estava lá antes voltou.
    Restaurada,
    /// Não havia backup e o arquivo era reconhecidamente nosso (cor chapada): saiu,
    /// que é exatamente o estado anterior — antes de o Loop escrever, não havia nada.
    Removida,
    /// Não havia backup e o arquivo NÃO parece nosso. Fica onde está: é mais provável
    /// ser pintura que o jogador pôs depois do que sobra da nossa.
    Preservada,
    /// Não há arquivo nenhum nesta pasta.
    Nada,
}

/// O resultado do desfazer, por carro.
#[derive(serde::Serialize)]
pub struct DesfazerPintura {
    pub car_key: String,
    pub caminho: String,
    pub estado: EstadoDoDesfazer,
}

/// Tamanho exato de um `.tga` escrito por [`crate::iracing_sdk::paint_gen`]: 18 bytes
/// de cabeçalho + 2048² pixels de 4 bytes.
const TAMANHO_TGA_DO_LOOP: usize = 18 + 2048 * 2048 * 4;

/// Estes bytes são uma pintura que NÓS escrevemos?
///
/// A pergunta importa porque, sem backup, o desfazer teria de escolher entre apagar um
/// arquivo que pode ser do jogador e deixar a cor da equipe para sempre. O que resolve é
/// que a nossa pintura é uma cor CHAPADA no quadro inteiro — nenhuma pintura de verdade
/// é. Somado ao tamanho e ao cabeçalho exatos, é reconhecimento seguro.
fn e_pintura_do_loop(bytes: &[u8]) -> bool {
    if bytes.len() != TAMANHO_TGA_DO_LOOP {
        return false;
    }
    // Tipo 2 (truecolor sem compressão), 32 bits por pixel.
    if bytes[2] != 2 || bytes[16] != 32 {
        return false;
    }
    let pixels = &bytes[18..];
    let primeiro = &pixels[..4];
    pixels.chunks_exact(4).all(|p| p == primeiro)
}

/// Devolve UM arquivo de pintura ao estado anterior à primeira escrita nossa.
///
/// Escreve o conteúdo do backup em vez de renomear pelo mesmo motivo do modo janela: um
/// `rename` que falhe no meio deixaria o jogador sem os dois arquivos.
fn desfazer_pintura(tga: &std::path::Path) -> Result<EstadoDoDesfazer, String> {
    let backup = tga.with_extension(PAINT_BACKUP_SUFFIX);
    if backup.is_file() {
        let original = std::fs::read(&backup)
            .map_err(|e| format!("Falha ao ler a pintura preservada: {e}"))?;
        std::fs::write(tga, original)
            .map_err(|e| format!("Falha ao devolver a pintura original: {e}"))?;
        let _ = std::fs::remove_file(&backup);
        return Ok(EstadoDoDesfazer::Restaurada);
    }
    if !tga.is_file() {
        return Ok(EstadoDoDesfazer::Nada);
    }
    let bytes = std::fs::read(tga).map_err(|e| format!("Falha ao ler a pintura: {e}"))?;
    if !e_pintura_do_loop(&bytes) {
        return Ok(EstadoDoDesfazer::Preservada);
    }
    std::fs::remove_file(tga).map_err(|e| format!("Falha ao remover a pintura: {e}"))?;
    Ok(EstadoDoDesfazer::Removida)
}

/// Desfaz a pintura automática em TODOS os carros que o Loop sabe pintar.
///
/// A contrapartida de pintar sem perguntar. Devolve uma linha por carro para a tela
/// poder dizer o que foi feito — inclusive o caso em que nada foi tocado, que é o mais
/// importante de não esconder.
#[tauri::command]
pub fn iracing_desfazer_pinturas(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Vec<DesfazerPintura>, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::iracing_sdk::{paths, roster_gen};
    use tauri::Manager;

    // O custid do save vem primeiro: é o que foi usado para NOMEAR os arquivos que
    // escrevemos. O cache da sessão é a segunda opção, para quem desfaz numa carreira
    // que nunca chegou a exportar.
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let vinculado = if db_path.exists() {
        Database::open_existing(&db_path)
            .ok()
            .and_then(|db| {
                crate::db::queries::meta::get_meta_value(&db.conn, PLAYER_CUSTID_META_KEY).ok()
            })
            .flatten()
            .and_then(|s| s.trim().parse::<i64>().ok())
    } else {
        None
    };
    let custid = vinculado
        .or_else(capture_player_custid)
        .ok_or("Ainda não sei o seu ID do iRacing, então nunca pintei nada para desfazer.")?;

    let mut saida = Vec::new();
    for car_key in CARROS_PINTAVEIS {
        let car = match roster_gen::car_spec(car_key) {
            Some(c) => c,
            None => continue,
        };
        let dir = match paths::car_paint_dir(car.car_path) {
            Some(d) => d,
            None => continue,
        };
        let tga = dir.join(format!("car_{custid}.tga"));
        let estado = desfazer_pintura(&tga)?;
        saida.push(DesfazerPintura {
            car_key: car_key.to_string(),
            caminho: tga.display().to_string(),
            estado,
        });
    }
    Ok(saida)
}

/// A pintura automática está ligada nas Configurações? Na dúvida (não deu para
/// resolver o `app_data_dir`), responde o padrão: ligada.
fn auto_paint_enabled(app: &tauri::AppHandle) -> bool {
    use crate::config::app_config::AppConfig;
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .map(|dir| AppConfig::load_or_default(&dir).auto_paint_car)
        .unwrap_or(true)
}

/// Lê o custid já capturado (sampler) ou tenta ler a sessão atual agora.
fn capture_player_custid() -> Option<i64> {
    if let Some(id) = iracing_sdk::cached_custid() {
        return Some(id);
    }
    if let Ok(session) = iracing_sdk::read_session() {
        iracing_sdk::note_session_custid(&session.session_yaml);
    }
    iracing_sdk::cached_custid()
}

/// Exportação da etapa: pinta o carro do jogador na cor da equipe atual e VINCULA
/// o custid a este save. Roda sozinho, sem perguntar nada — o arquivo é local
/// (ninguém mais na sessão vê a nossa cor), a cor é a da carreira, e o `.tga` que
/// já estava lá fica preservado em `.tga.loop-bak`.
///
/// Não perguntar tem duas contrapartidas, e as duas precisam existir: o interruptor
/// `auto_paint_car` das Configurações, que impede as próximas, e
/// [`iracing_desfazer_pinturas`], que devolve o que já foi escrito.
///
/// Silencioso por contrato: devolve `None` quando a pintura está desligada nas
/// Configurações ou quando ainda não temos o custid (o jogador nunca abriu o
/// iRacing). Nos dois casos não há nada a mostrar, e a exportação segue.
#[tauri::command]
pub fn iracing_auto_paint_player(
    app: tauri::AppHandle,
    career_id: String,
    car_key: String,
) -> Result<Option<ApplyPaintResult>, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
    use crate::iracing_sdk::roster_gen;
    use tauri::Manager;

    if !auto_paint_enabled(&app) {
        return Ok(None);
    }
    let custid = match capture_player_custid() {
        Some(id) => id,
        None => return Ok(None), // nunca abriu o iRacing → pinta na próxima vez
    };

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;
    let player =
        dq::get_player_driver(&db.conn).map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let team = cq::get_active_contract_for_pilot(&db.conn, &player.id)
        .ok()
        .flatten()
        .and_then(|c| tq::get_team_by_id(&db.conn, &c.equipe_id).ok().flatten())
        .ok_or("Você não tem contrato/time ativo nesta carreira.")?;

    let (path, color) = write_player_car_tga(
        &car_key,
        &roster_gen::normalize_hex(&team.cor_primaria),
        custid,
    )?;

    crate::db::queries::meta::put_meta_value(&db.conn, PLAYER_CUSTID_META_KEY, &custid.to_string())
        .map_err(|e| format!("Falha ao vincular o ID ao save: {e}"))?;

    Ok(Some(ApplyPaintResult {
        path,
        custid,
        color,
    }))
}

/// Mercado: repinta o carro do jogador na cor da NOVA equipe ao aceitar um contrato.
/// Usa o custid vinculado ao save (ou o capturado na sessão, persistindo-o). Devolve
/// `None` silenciosamente se a pintura está desligada nas Configurações ou se ainda
/// não há custid (jamais abriu o iRacing) — o front simplesmente não mostra o toast
/// nesses casos.
#[tauri::command]
pub fn iracing_apply_market_paint(
    app: tauri::AppHandle,
    career_id: String,
    team_color: String,
    category: String,
) -> Result<Option<ApplyPaintResult>, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use tauri::Manager;

    if !auto_paint_enabled(&app) {
        return Ok(None);
    }

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let linked = crate::db::queries::meta::get_meta_value(&db.conn, PLAYER_CUSTID_META_KEY)
        .map_err(|e| format!("Falha ao ler meta: {e}"))?
        .and_then(|s| s.trim().parse::<i64>().ok());
    let custid = match linked.or_else(capture_player_custid) {
        Some(id) => id,
        None => return Ok(None), // sem ID ainda → nada a fazer (silencioso)
    };
    if linked.is_none() {
        let _ = crate::db::queries::meta::put_meta_value(
            &db.conn,
            PLAYER_CUSTID_META_KEY,
            &custid.to_string(),
        );
    }

    let car_key = car_key_for_category(&category);
    let (path, color) = write_player_car_tga(car_key, &team_color, custid)?;
    Ok(Some(ApplyPaintResult {
        path,
        custid,
        color,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iracing_sdk::paint_gen;
    use std::path::PathBuf;

    /// Pasta temporária só deste caso — o `.tga` de teste tem 16 MB e não pode
    /// esbarrar no de outro.
    fn pasta(rotulo: &str) -> PathBuf {
        let unico = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("loop_pintura_{rotulo}_{unico}"));
        std::fs::create_dir_all(&dir).expect("criar pasta temporária");
        dir
    }

    #[test]
    fn reconhece_a_propria_pintura() {
        let dir = pasta("reconhece");
        let tga = dir.join("car_1.tga");
        paint_gen::write_solid_tga(&tga, "3A86FF").unwrap();
        assert!(e_pintura_do_loop(&std::fs::read(&tga).unwrap()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// O reconhecimento é o que autoriza APAGAR um arquivo quando não há backup.
    /// Errar para o lado do "é nossa" apagaria pintura do jogador, então tudo que
    /// não bate exatamente precisa ser recusado.
    #[test]
    fn nao_confunde_arquivo_alheio_com_a_propria_pintura() {
        assert!(!e_pintura_do_loop(&[])); // vazio
        assert!(!e_pintura_do_loop(&[0u8; 64])); // curto demais

        let dir = pasta("alheio");
        let tga = dir.join("car_1.tga");
        paint_gen::write_solid_tga(&tga, "FF0000").unwrap();
        let mut bytes = std::fs::read(&tga).unwrap();
        // Um único pixel diferente já basta: pintura de verdade tem desenho.
        let meio = bytes.len() / 2;
        bytes[meio] ^= 0xFF;
        assert!(!e_pintura_do_loop(&bytes));

        // Tamanho certo, cabeçalho errado (outro formato salvo com o mesmo nome).
        let mut outro = std::fs::read(&tga).unwrap();
        outro[2] = 10; // TGA com RLE
        assert!(!e_pintura_do_loop(&outro));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn com_backup_a_pintura_do_jogador_volta() {
        let dir = pasta("restaura");
        let tga = dir.join("car_1.tga");
        let backup = tga.with_extension(PAINT_BACKUP_SUFFIX);
        std::fs::write(&backup, b"a pintura que o jogador baixou").unwrap();
        paint_gen::write_solid_tga(&tga, "00FF00").unwrap();

        assert_eq!(desfazer_pintura(&tga).unwrap(), EstadoDoDesfazer::Restaurada);
        assert_eq!(
            std::fs::read(&tga).unwrap(),
            b"a pintura que o jogador baixou"
        );
        assert!(!backup.exists(), "o backup precisa sair junto");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Sem backup, o `.tga` que está lá foi criado por nós do zero — antes não havia
    /// nada, e devolver o estado anterior é remover.
    #[test]
    fn sem_backup_a_nossa_pintura_e_removida() {
        let dir = pasta("remove");
        let tga = dir.join("car_1.tga");
        paint_gen::write_solid_tga(&tga, "112233").unwrap();
        assert_eq!(desfazer_pintura(&tga).unwrap(), EstadoDoDesfazer::Removida);
        assert!(!tga.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// O caso que o desfazer NÃO pode estragar: o jogador pôs uma pintura própria
    /// depois, sem o Loop ter passado por ali. Apagá-la seria perder trabalho dele de
    /// forma definitiva.
    #[test]
    fn sem_backup_a_pintura_alheia_fica_onde_esta() {
        let dir = pasta("preserva");
        let tga = dir.join("car_1.tga");
        std::fs::write(&tga, b"pintura feita a mao").unwrap();
        assert_eq!(desfazer_pintura(&tga).unwrap(), EstadoDoDesfazer::Preservada);
        assert!(tga.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sem_arquivo_nenhum_nao_e_erro() {
        let dir = pasta("vazio");
        assert_eq!(
            desfazer_pintura(&dir.join("car_1.tga")).unwrap(),
            EstadoDoDesfazer::Nada
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Desfazer duas vezes seguidas responde "não há nada" em vez de estourar — o
    /// botão fica clicável e o jogador clica de novo.
    #[test]
    fn desfazer_duas_vezes_e_seguro() {
        let dir = pasta("duas_vezes");
        let tga = dir.join("car_1.tga");
        std::fs::write(&tga.with_extension(PAINT_BACKUP_SUFFIX), b"original").unwrap();
        paint_gen::write_solid_tga(&tga, "445566").unwrap();
        assert_eq!(desfazer_pintura(&tga).unwrap(), EstadoDoDesfazer::Restaurada);
        // Na segunda, o que está lá é a pintura do jogador — e ela fica.
        assert_eq!(desfazer_pintura(&tga).unwrap(), EstadoDoDesfazer::Preservada);
        assert_eq!(std::fs::read(&tga).unwrap(), b"original");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A lista que o desfazer varre precisa bater com os carros que o Loop sabe
    /// pintar. Um carro novo no `car_spec` sem entrada aqui deixaria a cor da equipe
    /// para trás numa pasta que ninguém mais visita.
    #[test]
    fn todo_carro_pintavel_existe_no_catalogo() {
        for chave in CARROS_PINTAVEIS {
            assert!(
                crate::iracing_sdk::roster_gen::car_spec(chave).is_some(),
                "{chave} não está em roster_gen::car_spec"
            );
        }
        for chave in ["mx5", "gr86", "bmwm2"] {
            assert!(
                CARROS_PINTAVEIS.contains(&chave),
                "{chave} é pintável mas o desfazer não passa por ele"
            );
        }
    }
}
