use crate::config::app_config::AppConfig;
use tauri::AppHandle;
use tauri::Manager;

#[tauri::command]
pub fn get_config(app: AppHandle) -> Result<AppConfig, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    Ok(AppConfig::load_or_default(&base_dir))
}

/// Funde o config que veio da tela de Configurações com o que está no disco.
///
/// **A regra é invertida de propósito.** A versão anterior listava campo a campo o que
/// vinha do front (`language`, `autosave_enabled`, `auto_paint_car`, as chaves de overlay)
/// e tudo que não estivesse na lista era descartado em silêncio: uma preferência nova
/// aparecia na tela, o jogador mudava, o Loop dizia que salvou e ao reabrir estava como
/// antes. O esquecimento era invisível.
///
/// Aqui a preferência do jogador vale por padrão e a lista é a dos campos que o front
/// NÃO manda: identidade da instalação, estado de janela e as chaves com dono próprio.
/// Esquecer de listar um campo novo agora significa deixá-lo passar, que é o erro barato.
///
/// O front sempre devolve o objeto inteiro que leu do `get_config` (`{...config, campo}`),
/// então nenhum campo chega ausente por acidente.
pub fn merge_config_do_front(atual: &AppConfig, do_front: AppConfig) -> AppConfig {
    let mut merged = do_front;

    // Identidade e metadados: a tela não tem o que dizer sobre eles.
    // `version` segue o binário; `install_id` é a chave do cooldown do servidor de IA e
    // recriá-la zeraria o cooldown; `last_career` e o estado de janela têm escritores
    // próprios; `base_dir` nem persiste no JSON.
    merged.version = atual.version.clone();
    merged.install_id = atual.install_id.clone();
    merged.last_career = atual.last_career;
    merged.window_width = atual.window_width;
    merged.window_height = atual.window_height;
    merged.window_maximized = atual.window_maximized;
    merged.base_dir = atual.base_dir.clone();

    // `spotter_takeover` tem dono: `iracing_spotter_set`, que também sincroniza o
    // `app.ini` do iRacing. Aceitar o valor aqui deixaria a preferência e o arquivo
    // divergirem sempre que a tela salvasse qualquer outra coisa.
    merged.spotter_takeover = atual.spotter_takeover;

    // Telemetria de produto: `None` vindo do front significa "não mexi nisso", e não "o
    // jogador recusou" — a diferença importa, porque `None` no disco é o que faz o aviso
    // de primeira execução aparecer.
    if merged.telemetry_enabled.is_none() {
        merged.telemetry_enabled = atual.telemetry_enabled;
    }

    merged
}

#[tauri::command]
pub fn update_config(app: AppHandle, new_config: AppConfig) -> Result<(), String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let current_config = AppConfig::load_or_default(&base_dir);
    let telemetria_veio_do_front = new_config.telemetry_enabled.is_some();
    let merged = merge_config_do_front(&current_config, new_config);

    // Aplicações a quente, para nada exigir restart.
    if telemetria_veio_do_front {
        crate::telemetry::set_enabled(merged.telemetry_enabled.unwrap_or(false));
    }
    // Chaves de overlay no espelho volátil: os escritores do front (que leem por poll)
    // param/voltam na hora.
    crate::commands::vr_overlay::set_vr_mode(&merged.vr_overlay_mode);
    crate::commands::vr_overlay::set_monitor_in_vr(merged.monitor_overlay_in_vr);
    // Idioma no locale do backend.
    rust_i18n::set_locale(&merged.language);
    // Punição por voltar destruído da classificação: o monitor lê de um estático, então
    // ligar a chave vale já no fim de semana em curso, sem reabrir o app.
    crate::iracing_sdk::race_monitor::set_quali_wreck_penalty(merged.quali_wreck_penalty);

    merged.save()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn config_no_disco() -> AppConfig {
        AppConfig {
            version: "9.9.9".to_string(),
            last_career: Some(3),
            language: "pt-BR".to_string(),
            autosave_enabled: true,
            install_id: Some("instalacao-antiga".to_string()),
            telemetry_enabled: Some(true),
            vr_overlay_mode: "auto".to_string(),
            monitor_overlay_in_vr: false,
            spotter_takeover: true,
            auto_paint_car: true,
            quali_wreck_penalty: false,
            window_width: 1600,
            window_height: 900,
            window_maximized: true,
            base_dir: PathBuf::from("C:/base"),
        }
    }

    /// O modo de falha que motivou a inversão: uma preferência que o front manda precisa
    /// chegar ao disco, mesmo sem ninguém ter lembrado de listá-la no merge.
    #[test]
    fn a_preferencia_do_jogador_vale_por_padrao() {
        let atual = config_no_disco();
        let mut do_front = config_no_disco();
        do_front.language = "en-US".to_string();
        do_front.autosave_enabled = false;
        do_front.auto_paint_car = false;
        do_front.vr_overlay_mode = "off".to_string();
        do_front.monitor_overlay_in_vr = true;
        do_front.quali_wreck_penalty = true;

        let merged = merge_config_do_front(&atual, do_front);

        assert_eq!(merged.language, "en-US");
        assert!(!merged.autosave_enabled);
        assert!(!merged.auto_paint_car);
        assert_eq!(merged.vr_overlay_mode, "off");
        assert!(merged.monitor_overlay_in_vr);
        assert!(
            merged.quali_wreck_penalty,
            "a punição da quali destruída saiu da variável de ambiente para a config; \
             se ela não atravessar o merge, o jogador liga e nada acontece"
        );
    }

    /// Identidade e estado de janela não vêm da tela, mesmo que o front os devolva sujos.
    #[test]
    fn identidade_e_estado_de_janela_ficam_com_o_disco() {
        let atual = config_no_disco();
        let mut do_front = config_no_disco();
        do_front.install_id = Some("instalacao-inventada".to_string());
        do_front.version = "0.0.1".to_string();
        do_front.last_career = Some(99);
        do_front.window_width = 100;
        do_front.window_height = 100;
        do_front.window_maximized = false;
        do_front.base_dir = PathBuf::from("C:/outro");

        let merged = merge_config_do_front(&atual, do_front);

        assert_eq!(merged.install_id.as_deref(), Some("instalacao-antiga"));
        assert_eq!(merged.version, "9.9.9");
        assert_eq!(merged.last_career, Some(3));
        assert_eq!((merged.window_width, merged.window_height), (1600, 900));
        assert!(merged.window_maximized);
        assert_eq!(merged.base_dir, PathBuf::from("C:/base"));
    }

    /// O spotter tem dono, e não é esta tela.
    #[test]
    fn spotter_takeover_nao_muda_por_aqui() {
        let atual = config_no_disco();
        let mut do_front = config_no_disco();
        do_front.spotter_takeover = false;

        let merged = merge_config_do_front(&atual, do_front);

        assert!(
            merged.spotter_takeover,
            "quem escreve essa chave é o iracing_spotter_set, que também mexe no app.ini"
        );
    }

    /// `None` do front é "não mexi", e não "recusei".
    #[test]
    fn telemetria_ausente_no_front_preserva_a_escolha_gravada() {
        let atual = config_no_disco();
        let mut do_front = config_no_disco();
        do_front.telemetry_enabled = None;

        let merged = merge_config_do_front(&atual, do_front);

        assert_eq!(merged.telemetry_enabled, Some(true));
    }

    /// E a recusa explícita passa.
    #[test]
    fn telemetria_recusada_no_front_chega_ao_disco() {
        let atual = config_no_disco();
        let mut do_front = config_no_disco();
        do_front.telemetry_enabled = Some(false);

        let merged = merge_config_do_front(&atual, do_front);

        assert_eq!(merged.telemetry_enabled, Some(false));
    }

    /// Consentimento ainda não dado no disco: o `None` continua `None` e o aviso de
    /// primeira execução segue aparecendo.
    #[test]
    fn consentimento_nunca_dado_continua_pendente() {
        let mut atual = config_no_disco();
        atual.telemetry_enabled = None;
        let mut do_front = config_no_disco();
        do_front.telemetry_enabled = None;

        let merged = merge_config_do_front(&atual, do_front);

        assert_eq!(merged.telemetry_enabled, None);
    }
}
