#![allow(dead_code)]

use rand::Rng;

static NATIONALITIES: [NationalityInfo; 23] = [
    NationalityInfo {
        id: "gb",
        nome_pt: "Britanico",
        nome_en: "British",
        nome_fem_pt: "Britanica",
        nome_fem_en: "British",
        emoji: "🇬🇧",
        peso: 15,
    },
    NationalityInfo {
        id: "de",
        nome_pt: "Alemao",
        nome_en: "German",
        nome_fem_pt: "Alema",
        nome_fem_en: "German",
        emoji: "🇩🇪",
        peso: 12,
    },
    NationalityInfo {
        id: "fr",
        nome_pt: "Frances",
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
        nome_pt: "Holandes",
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
        nome_pt: "Japones",
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
        nome_pt: "Finlandes",
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
        nome_pt: "Portugues",
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
        nome_pt: "Austriaco",
        nome_en: "Austrian",
        nome_fem_pt: "Austriaca",
        nome_fem_en: "Austrian",
        emoji: "🇦🇹",
        peso: 2,
    },
    NationalityInfo {
        id: "ch",
        nome_pt: "Suico",
        nome_en: "Swiss",
        nome_fem_pt: "Suica",
        nome_fem_en: "Swiss",
        emoji: "🇨🇭",
        peso: 2,
    },
    NationalityInfo {
        id: "dk",
        nome_pt: "Dinamarques",
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
        nome_pt: "Noruegues",
        nome_en: "Norwegian",
        nome_fem_pt: "Norueguesa",
        nome_fem_en: "Norwegian",
        emoji: "🇳🇴",
        peso: 2,
    },
    NationalityInfo {
        id: "pl",
        nome_pt: "Polones",
        nome_en: "Polish",
        nome_fem_pt: "Polonesa",
        nome_fem_en: "Polish",
        emoji: "🇵🇱",
        peso: 2,
    },
    NationalityInfo {
        id: "cn",
        nome_pt: "Chines",
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
}
