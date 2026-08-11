use rand::Rng;

static NATIONALITIES: [NationalityInfo; 23] = [
    NationalityInfo {
        id: "gb",
        nome_pt: "Britânico",
        nome_en: "British",
        nome_fem_pt: "Britânica",
        nome_fem_en: "British",
        emoji: "🇬🇧",
        peso: 15,
    },
    NationalityInfo {
        id: "de",
        nome_pt: "Alemão",
        nome_en: "German",
        nome_fem_pt: "Alemã",
        nome_fem_en: "German",
        emoji: "🇩🇪",
        peso: 12,
    },
    NationalityInfo {
        id: "fr",
        nome_pt: "Francês",
        nome_en: "French",
        nome_fem_pt: "Francesa",
        nome_fem_en: "French",
        emoji: "🇫🇷",
        peso: 10,
    },
    NationalityInfo {
        id: "it",
        nome_pt: "Italiano",
        nome_en: "Italian",
        nome_fem_pt: "Italiana",
        nome_fem_en: "Italian",
        emoji: "🇮🇹",
        peso: 10,
    },
    NationalityInfo {
        id: "es",
        nome_pt: "Espanhol",
        nome_en: "Spanish",
        nome_fem_pt: "Espanhola",
        nome_fem_en: "Spanish",
        emoji: "🇪🇸",
        peso: 8,
    },
    NationalityInfo {
        id: "br",
        nome_pt: "Brasileiro",
        nome_en: "Brazilian",
        nome_fem_pt: "Brasileira",
        nome_fem_en: "Brazilian",
        emoji: "🇧🇷",
        peso: 8,
    },
    NationalityInfo {
        id: "nl",
        nome_pt: "Holandês",
        nome_en: "Dutch",
        nome_fem_pt: "Holandesa",
        nome_fem_en: "Dutch",
        emoji: "🇳🇱",
        peso: 6,
    },
    NationalityInfo {
        id: "au",
        nome_pt: "Australiano",
        nome_en: "Australian",
        nome_fem_pt: "Australiana",
        nome_fem_en: "Australian",
        emoji: "🇦🇺",
        peso: 5,
    },
    NationalityInfo {
        id: "jp",
        nome_pt: "Japonês",
        nome_en: "Japanese",
        nome_fem_pt: "Japonesa",
        nome_fem_en: "Japanese",
        emoji: "🇯🇵",
        peso: 5,
    },
    NationalityInfo {
        id: "us",
        nome_pt: "Americano",
        nome_en: "American",
        nome_fem_pt: "Americana",
        nome_fem_en: "American",
        emoji: "🇺🇸",
        peso: 5,
    },
    NationalityInfo {
        id: "mx",
        nome_pt: "Mexicano",
        nome_en: "Mexican",
        nome_fem_pt: "Mexicana",
        nome_fem_en: "Mexican",
        emoji: "🇲🇽",
        peso: 4,
    },
    NationalityInfo {
        id: "ar",
        nome_pt: "Argentino",
        nome_en: "Argentine",
        nome_fem_pt: "Argentina",
        nome_fem_en: "Argentine",
        emoji: "🇦🇷",
        peso: 4,
    },
    NationalityInfo {
        id: "fi",
        nome_pt: "Finlandês",
        nome_en: "Finnish",
        nome_fem_pt: "Finlandesa",
        nome_fem_en: "Finnish",
        emoji: "🇫🇮",
        peso: 3,
    },
    NationalityInfo {
        id: "be",
        nome_pt: "Belga",
        nome_en: "Belgian",
        nome_fem_pt: "Belga",
        nome_fem_en: "Belgian",
        emoji: "🇧🇪",
        peso: 3,
    },
    NationalityInfo {
        id: "pt",
        nome_pt: "Português",
        nome_en: "Portuguese",
        nome_fem_pt: "Portuguesa",
        nome_fem_en: "Portuguese",
        emoji: "🇵🇹",
        peso: 3,
    },
    NationalityInfo {
        id: "ca",
        nome_pt: "Canadense",
        nome_en: "Canadian",
        nome_fem_pt: "Canadense",
        nome_fem_en: "Canadian",
        emoji: "🇨🇦",
        peso: 3,
    },
    NationalityInfo {
        id: "at",
        nome_pt: "Austríaco",
        nome_en: "Austrian",
        nome_fem_pt: "Austríaca",
        nome_fem_en: "Austrian",
        emoji: "🇦🇹",
        peso: 2,
    },
    NationalityInfo {
        id: "ch",
        nome_pt: "Suíço",
        nome_en: "Swiss",
        nome_fem_pt: "Suíça",
        nome_fem_en: "Swiss",
        emoji: "🇨🇭",
        peso: 2,
    },
    NationalityInfo {
        id: "dk",
        nome_pt: "Dinamarquês",
        nome_en: "Danish",
        nome_fem_pt: "Dinamarquesa",
        nome_fem_en: "Danish",
        emoji: "🇩🇰",
        peso: 2,
    },
    NationalityInfo {
        id: "se",
        nome_pt: "Sueco",
        nome_en: "Swedish",
        nome_fem_pt: "Sueca",
        nome_fem_en: "Swedish",
        emoji: "🇸🇪",
        peso: 2,
    },
    NationalityInfo {
        id: "no",
        nome_pt: "Norueguês",
        nome_en: "Norwegian",
        nome_fem_pt: "Norueguesa",
        nome_fem_en: "Norwegian",
        emoji: "🇳🇴",
        peso: 2,
    },
    NationalityInfo {
        id: "pl",
        nome_pt: "Polonês",
        nome_en: "Polish",
        nome_fem_pt: "Polonesa",
        nome_fem_en: "Polish",
        emoji: "🇵🇱",
        peso: 2,
    },
    NationalityInfo {
        id: "cn",
        nome_pt: "Chinês",
        nome_en: "Chinese",
        nome_fem_pt: "Chinesa",
        nome_fem_en: "Chinese",
        emoji: "🇨🇳",
        peso: 2,
    },
];

pub struct NationalityInfo {
    pub id: &'static str,
    pub nome_pt: &'static str,
    pub nome_en: &'static str,
    pub nome_fem_pt: &'static str,
    pub nome_fem_en: &'static str,
    pub emoji: &'static str,
    pub peso: u8,
}

pub fn get_all_nationalities() -> &'static [NationalityInfo] {
    &NATIONALITIES
}

pub fn random_nationality(rng: &mut impl Rng) -> &'static NationalityInfo {
    let total_weight: u32 = NATIONALITIES
        .iter()
        .map(|nationality| nationality.peso as u32)
        .sum();
    let mut roll = rng.gen_range(0..total_weight);

    for nationality in &NATIONALITIES {
        let weight = nationality.peso as u32;
        if roll < weight {
            return nationality;
        }
        roll -= weight;
    }

    &NATIONALITIES[0]
}

pub fn get_nationality(id: &str) -> Option<&'static NationalityInfo> {
    NATIONALITIES
        .iter()
        .find(|nationality| nationality.id == id)
}

pub fn format_nationality(id: &str, genero: &str, lang: &str) -> String {
    let Some(nationality) = get_nationality(id) else {
        return id.to_string();
    };

    let label = if lang.eq_ignore_ascii_case("pt-BR") || lang.eq_ignore_ascii_case("pt") {
        if genero.eq_ignore_ascii_case("F") {
            nationality.nome_fem_pt
        } else {
            nationality.nome_pt
        }
    } else if genero.eq_ignore_ascii_case("F") {
        nationality.nome_fem_en
    } else {
        nationality.nome_en
    };

    format!("{} {}", nationality.emoji, label)
}

/// O id da nacionalidade a partir do RÓTULO GRAVADO no piloto (`drivers.nacionalidade`).
///
/// O save guarda o rótulo pronto, não o id, e ao longo das versões guardou várias formas
/// da mesma coisa: com e sem bandeira, em PT e em EN, no masculino e no feminino, e — em
/// saves anteriores à acentuação desta tabela — sem acento (`Britanico`, `Suico`,
/// `Japones`). Todas apontam para o mesmo país, e é isso que esta função devolve.
///
/// Compara sobre a forma dobrada (sem bandeira, sem acento, minúscula), pela mesma razão
/// que [`crate::constants::geografia`] dobra o nome de país: sem isso `Japonês` e
/// `Japones` seriam nacionalidades diferentes, e o dado real tem as duas.
pub fn nationality_id_from_label(label: &str) -> Option<&'static str> {
    let alvo = dobrar(label);
    if alvo.is_empty() {
        return None;
    }
    NATIONALITIES
        .iter()
        .find(|n| {
            dobrar(n.id) == alvo
                || dobrar(n.nome_pt) == alvo
                || dobrar(n.nome_fem_pt) == alvo
                || dobrar(n.nome_en) == alvo
                || dobrar(n.nome_fem_en) == alvo
        })
        .map(|n| n.id)
}

/// Rótulo de DISPLAY da nacionalidade gravada, resolvido no locale ATIVO.
///
/// É o mesmo desenho de [`crate::constants::country_label`]: o dado persistido vira
/// token, e quem escolhe o idioma é a tela. Sem isto, o rótulo congelava em pt-BR no
/// instante da geração do piloto — um jogador em en-US lia "Britânico" na ficha de um
/// piloto gerado meses antes, e trocar o idioma do jogo não mexia em nada já gravado.
///
/// `genero` é o do piloto ("F" flexiona); o rótulo cru gravado NÃO é fonte confiável de
/// gênero, porque em inglês as duas formas são a mesma palavra. Fallback = valor cru:
/// falta de correspondência nunca vira nacionalidade inventada.
pub fn nationality_display_label(rotulo_gravado: &str, genero: &str) -> String {
    match nationality_id_from_label(rotulo_gravado) {
        Some(id) => format_nationality(id, genero, &rust_i18n::locale()),
        None => rotulo_gravado.to_string(),
    }
}

/// Derruba bandeira emoji, acento, pontuação e caixa.
///
/// É A normalização do crate — a mesma de país, em
/// [`crate::constants::geografia::normalizar_pais`]. Gentílico e nome de país chegam do
/// dado com exatamente os mesmos três formatos, então dobrar de dois jeitos diferentes
/// só criaria uma segunda tabela de variantes para alguém esquecer.
fn dobrar(valor: &str) -> String {
    crate::constants::geografia::normalizar_pais(valor)
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_random_nationality_returns_valid() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..100 {
            let nationality = random_nationality(&mut rng);
            assert!(!nationality.id.is_empty());
            assert!(!nationality.emoji.is_empty());
        }
    }

    #[test]
    fn test_nationality_weights_sum() {
        let total: u32 = get_all_nationalities()
            .iter()
            .map(|nationality| nationality.peso as u32)
            .sum();
        assert_eq!(total, 118);
    }

    #[test]
    fn test_format_nationality_pt() {
        assert_eq!(format_nationality("br", "M", "pt-BR"), "🇧🇷 Brasileiro");
    }

    #[test]
    fn test_format_nationality_fem() {
        assert_eq!(format_nationality("br", "F", "pt-BR"), "🇧🇷 Brasileira");
    }

    /// Todas as formas que o save já gravou para a MESMA nacionalidade resolvem no mesmo
    /// id — inclusive as sem acento, que é o que ficou nos saves anteriores à acentuação
    /// desta tabela.
    #[test]
    fn o_rotulo_gravado_resolve_em_qualquer_das_formas_ja_persistidas() {
        for variante in [
            "🇬🇧 Britânico",
            "Britânico",
            "Britanico",    // legado sem acento
            "🇬🇧 Britânica", // feminino
            "British",      // save feito no locale inglês
            "gb",
        ] {
            assert_eq!(
                nationality_id_from_label(variante),
                Some("gb"),
                "variante não resolveu: {variante:?}"
            );
        }
        assert_eq!(nationality_id_from_label("Suico"), Some("ch"));
        assert_eq!(nationality_id_from_label("Japones"), Some("jp"));
        // Sem correspondência não vira palpite.
        assert_eq!(nationality_id_from_label("Atlante"), None);
        assert_eq!(nationality_id_from_label(""), None);
    }

    /// O rótulo de display segue o locale ATIVO, e não o que estava em vigor quando o
    /// piloto foi gerado. `#[serial]`: o locale é estado global do processo.
    #[test]
    #[serial_test::serial]
    fn o_display_da_nacionalidade_segue_o_locale_ativo() {
        let anterior = rust_i18n::locale().to_string();

        rust_i18n::set_locale("pt-BR");
        assert_eq!(nationality_display_label("Britanico", "M"), "🇬🇧 Britânico");
        assert_eq!(nationality_display_label("British", "F"), "🇬🇧 Britânica");
        assert_eq!(
            nationality_display_label("🇧🇷 Brasileiro", "F"),
            "🇧🇷 Brasileira"
        );

        rust_i18n::set_locale("en-US");
        assert_eq!(nationality_display_label("Britanico", "M"), "🇬🇧 British");
        assert_eq!(
            nationality_display_label("🇧🇷 Brasileiro", "M"),
            "🇧🇷 Brazilian"
        );
        // Fallback: o que não está na tabela sai como veio.
        assert_eq!(nationality_display_label("Atlante", "M"), "Atlante");

        rust_i18n::set_locale(&anterior);
    }
}
