use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TagLevel {
    DefeitoGrave,
    Defeito,
    Qualidade,
    QualidadeAlta,
    Elite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeTag {
    pub attribute_name: &'static str,
    // Texto de EXIBIÇÃO (i18n, runtime) → String, não &'static. A LÓGICA deve usar
    // `level` (locale-independente), nunca comparar `tag_text`.
    pub tag_text: String,
    pub level: TagLevel,
}

pub(crate) fn get_attribute_tag(attribute_name: &'static str, value: f64) -> Option<AttributeTag> {
    let rounded = value.round() as u8;
    let (level, index) = if rounded <= 10 {
        (TagLevel::DefeitoGrave, 0)
    } else if rounded <= 25 {
        (TagLevel::Defeito, 1)
    } else if rounded <= 74 {
        return None;
    } else if rounded <= 84 {
        (TagLevel::Qualidade, 2)
    } else if rounded <= 94 {
        (TagLevel::QualidadeAlta, 3)
    } else {
        (TagLevel::Elite, 4)
    };

    let tag_text = tag_text_for(attribute_name, index)?;
    Some(AttributeTag {
        attribute_name,
        tag_text,
        level,
    })
}

// Atributos que geram tag. O texto vem do i18n (`driver_tags.<attr>.<index>`),
// resolvido em runtime no idioma ativo.
const TAGGED_ATTRS: &[&str] = &[
    "skill",
    "consistencia",
    "racecraft",
    "defesa",
    "ritmo_classificacao",
    "gestao_pneus",
    "habilidade_largada",
    "adaptabilidade",
    "fator_chuva",
    "fitness",
    "experiencia",
    "desenvolvimento",
    "aggression",
    "smoothness",
    "midia",
    "mentalidade",
    "confianca",
];

fn tag_text_for(attribute_name: &str, index: usize) -> Option<String> {
    if !TAGGED_ATTRS.contains(&attribute_name) {
        return None;
    }
    let key = format!("driver_tags.{attribute_name}.{index}");
    Some(rust_i18n::t!(&key).to_string())
}
