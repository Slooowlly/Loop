pub mod cars;
pub mod categories;
pub mod flags_experimentais;
pub mod geografia;
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
/// `tracks.rs`/`teams.rs`. A fonte fica como token: `track_hemisphere` casa o `pais` cru.
/// Fallback = valor cru — país fora da tabela nunca vira rótulo inventado.
///
/// A entrada passa por [`geografia::normalizar_pais`], a MESMA dobra que resolve
/// coordenada e continente. Antes esta função mantinha a própria lista de variantes
/// ("🇫🇷 França" | "França" | "Franca"), o que dava dois lugares para lembrar a cada país
/// novo e um modo de falha silencioso: o país sumia do i18n e voltava cru na tela sem
/// ninguém notar. Agora a chave é uma por país, e o guard
/// `todo_pais_do_catalogo_tem_rotulo_i18n` cobra a cobertura contra o catálogo real.
pub fn country_label(pais: &str) -> String {
    match country_key(pais) {
        Some(key) => {
            let full = format!("country.{key}");
            rust_i18n::t!(&full).to_string()
        }
        None => pais.to_string(),
    }
}

/// A chave i18n do país, ou `None` se ele não estiver na tabela. Separada de
/// [`country_label`] para o guard conseguir distinguir "não tem chave" de "a tradução é
/// igual ao valor cru" — que é o caso de Taiwan e Portugal nos dois idiomas.
fn country_key(pais: &str) -> Option<&'static str> {
    let key = match geografia::normalizar_pais(pais).as_str() {
        "austria" => "austria",
        "australia" => "australia",
        "belgica" => "belgium",
        "brasil" => "brazil",
        "canada" => "canada",
        "alemanha" => "germany",
        "espanha" => "spain",
        "franca" => "france",
        "reino unido" => "uk",
        "hungria" => "hungary",
        "italia" => "italy",
        "japao" => "japan",
        "mexico" => "mexico",
        "holanda" => "netherlands",
        "noruega" => "norway",
        "portugal" => "portugal",
        "eua" => "usa",
        "suica" => "switzerland",
        "taiwan" => "taiwan",
        _ => return None,
    };
    Some(key)
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

    /// GUARD irmão do `todo_pais_do_catalogo_esta_no_mapa` de `geografia`: todo país que
    /// aparece em `constants/tracks` ou `constants/teams` precisa ter chave i18n. Sem
    /// isto, país novo no catálogo volta cru na tela — em português, para quem joga em
    /// inglês — e ninguém percebe, porque o fallback não faz barulho.
    #[test]
    fn todo_pais_do_catalogo_tem_rotulo_i18n() {
        let mut faltando: Vec<String> = Vec::new();
        for track in tracks::get_all_tracks() {
            if country_key(track.pais).is_none() {
                faltando.push(format!("pista: {}", track.pais));
            }
        }
        for equipe in teams::get_all_team_templates() {
            if country_key(equipe.pais_sede).is_none() {
                faltando.push(format!("equipe: {}", equipe.pais_sede));
            }
        }
        faltando.sort();
        faltando.dedup();
        assert!(faltando.is_empty(), "países sem chave i18n: {faltando:?}");
    }
}
