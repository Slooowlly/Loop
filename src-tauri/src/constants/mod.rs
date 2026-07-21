pub mod cars;
pub mod categories;
pub mod historical_timeline;
pub mod scoring;
pub mod skill_ranges;
pub mod teams;
pub mod timeline;
pub mod tracks;

/// Rótulo de display (i18n) do tier de categoria a partir do `nivel` (token da fonte,
/// `categories.rs`). Só o display usa isto; a fonte/lógica seguem com o token cru.
pub fn category_tier_label(nivel: &str) -> String {
    let key = match nivel {
        "Amador" => "amador",
        "Rookie" => "rookie",
        "Pro" => "pro",
        "Especial" => "especial",
        "Super Pro" => "super_pro",
        "Master" => "master",
        "Elite" => "elite",
        _ => return nivel.to_string(),
    };
    let full = format!("category_tier.{key}");
    rust_i18n::t!(&full).to_string()
}

/// Rótulo de display de país (i18n, com bandeira) a partir do `pais` cru de
/// `tracks.rs`/`teams.rs`. Cobre as variantes legadas do dado (com/sem emoji e acento).
/// A fonte fica como token: `track_hemisphere` casa o `pais` cru. Fallback = valor cru.
pub fn country_label(pais: &str) -> String {
    let key = match pais.trim() {
        "🇦🇹 Áustria" | "Áustria" | "Austria" => "austria",
        "🇦🇺 Austrália" | "Austrália" | "Australia" => "australia",
        "🇧🇪 Bélgica" | "Bélgica" => "belgium",
        "🇧🇷 Brasil" | "Brasil" => "brazil",
        "🇨🇦 Canadá" | "Canadá" | "Canada" => "canada",
        "🇩🇪 Alemanha" | "Alemanha" => "germany",
        "🇪🇸 Espanha" | "Espanha" => "spain",
        "🇫🇷 França" | "França" | "Franca" => "france",
        "🇬🇧 Reino Unido" | "Reino Unido" => "uk",
        "🇭🇺 Hungria" | "Hungria" => "hungary",
        "🇮🇹 Itália" | "Itália" | "Italia" => "italy",
        "🇯🇵 Japão" | "Japão" | "Japao" => "japan",
        "🇲🇽 México" | "México" => "mexico",
        "🇳🇱 Holanda" | "Holanda" => "netherlands",
        "🇳🇴 Noruega" | "Noruega" => "norway",
        "🇵🇹 Portugal" | "Portugal" => "portugal",
        "🇺🇸 EUA" | "EUA" => "usa",
        "🇨🇭 Suíça" | "Suíça" | "Suica" => "switzerland",
        "🇹🇼 Taiwan" | "Taiwan" => "taiwan",
        _ => return pais.to_string(),
    };
    let full = format!("country.{key}");
    rust_i18n::t!(&full).to_string()
}

#[cfg(test)]
mod i18n_label_tests {
    use super::*;

    /// Tier de categoria + país resolvem nos dois locales, cobrem as variantes legadas
    /// do dado de país e preservam a bandeira. `#[serial]` (troca o locale global).
    #[test]
    #[serial_test::serial]
    fn tier_e_pais_resolvem_nos_dois_locales() {
        rust_i18n::set_locale("pt-BR");
        assert_eq!(category_tier_label("Especial"), "Especial");
        assert_eq!(category_tier_label("Amador"), "Amador");
        assert_eq!(country_label("🇧🇷 Brasil"), "🇧🇷 Brasil");
        assert_eq!(country_label("Franca"), "🇫🇷 França"); // legado sem emoji/acento
        assert_eq!(country_label("Suica"), "🇨🇭 Suíça");

        rust_i18n::set_locale("en-US");
        assert_eq!(category_tier_label("Especial"), "Special");
        assert_eq!(category_tier_label("Amador"), "Amateur");
        assert_eq!(country_label("🇧🇷 Brasil"), "🇧🇷 Brazil");
        assert_eq!(country_label("Franca"), "🇫🇷 France");
        assert_eq!(country_label("Alemanha"), "🇩🇪 Germany");
        assert_eq!(country_label("Desconhecido"), "Desconhecido"); // fallback = valor cru

        rust_i18n::set_locale("pt-BR"); // restaura
    }
}
