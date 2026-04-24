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
    pub tag_text: &'static str,
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

fn tag_text_for(attribute_name: &str, index: usize) -> Option<&'static str> {
    let tags = match attribute_name {
        "skill" => ["Lento", "Abaixo do Ritmo", "Veloz", "Super Veloz", "Alien"],
        "consistencia" => [
            "Totalmente Imprevisível",
            "Inconsistente",
            "Consistente",
            "Muito Consistente",
            "Máquina de Regularidade",
        ],
        "racecraft" => [
            "Perigo nas Rodas",
            "Roda-a-roda Fraco",
            "Bom Disputador",
            "Mestre em Disputas",
            "Racecraft de Elite",
        ],
        "defesa" => [
            "Porta Aberta",
            "Defesa Fraca",
            "Bom Defensor",
            "Muro na Pista",
            "Inultrapassável",
        ],
        "ritmo_classificacao" => [
            "Péssimo em Quali",
            "Lento na Classificação",
            "Forte na Classificação",
            "Especialista em Quali",
            "Rei da Pole",
        ],
        "gestao_pneus" => [
            "Destruidor de Pneus",
            "Gestão de Pneus Fraca",
            "Bom com Pneus",
            "Excelente Gestão",
            "Smooth Operator",
        ],
        "habilidade_largada" => [
            "Péssimo nas Largadas",
            "Ruim de Largada",
            "Boas Largadas",
            "Excelente nas Largadas",
            "Foguete na Largada",
        ],
        "adaptabilidade" => [
            "Inflexível",
            "Lento para Adaptar",
            "Adaptável",
            "Muito Adaptável",
            "Camaleão",
        ],
        "fator_chuva" => [
            "Terrível na Chuva",
            "Dificuldade na Chuva",
            "Bom na Chuva",
            "Especialista de Chuva",
            "Mestre da Chuva",
        ],
        "fitness" => [
            "Doente",
            "Fora de Forma",
            "Boa Forma Física",
            "Atleta",
            "Forma Física de Elite",
        ],
        "experiencia" => [
            "Calouro",
            "Inexperiente",
            "Experiente",
            "Muito Experiente",
            "Veterano Sábio",
        ],
        "desenvolvimento" => [
            "Estagnado",
            "Desenvolvimento Lento",
            "Em Ascensão",
            "Evolução Rápida",
            "Prodígio",
        ],
        "aggression" => [
            "Passivo Demais",
            "Muito Cauteloso",
            "Agressivo",
            "Muito Agressivo",
            "Kamikaze",
        ],
        "smoothness" => [
            "Pilotagem Bruta",
            "Pouco Suave",
            "Pilotagem Suave",
            "Muito Suave",
            "Pilotagem de Seda",
        ],
        "midia" => [
            "Invisível",
            "Discreto",
            "Carismático",
            "Queridinho da Mídia",
            "Estrela",
        ],
        "mentalidade" => [
            "Frágil sob Pressão",
            "Mentalidade Fraca",
            "Boa Mentalidade",
            "Mentalidade de Campeão",
            "Gelo nas Veias",
        ],
        "confianca" => [
            "Sem Confiança",
            "Inseguro",
            "Confiante",
            "Muito Confiante",
            "Inabalável",
        ],
        _ => return None,
    };

    Some(tags[index])
}
