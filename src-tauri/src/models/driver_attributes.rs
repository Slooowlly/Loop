#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverAttributeKey {
    Skill,
    Consistencia,
    Racecraft,
    Defesa,
    RitmoClassificacao,
    GestaoPneus,
    HabilidadeLargada,
    Adaptabilidade,
    FatorChuva,
    Fitness,
    Experiencia,
    Desenvolvimento,
    Aggression,
    Smoothness,
    Midia,
    Mentalidade,
    Confianca,
}

impl DriverAttributeKey {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Consistencia => "consistencia",
            Self::Racecraft => "racecraft",
            Self::Defesa => "defesa",
            Self::RitmoClassificacao => "ritmo_classificacao",
            Self::GestaoPneus => "gestao_pneus",
            Self::HabilidadeLargada => "habilidade_largada",
            Self::Adaptabilidade => "adaptabilidade",
            Self::FatorChuva => "fator_chuva",
            Self::Fitness => "fitness",
            Self::Experiencia => "experiencia",
            Self::Desenvolvimento => "desenvolvimento",
            Self::Aggression => "aggression",
            Self::Smoothness => "smoothness",
            Self::Midia => "midia",
            Self::Mentalidade => "mentalidade",
            Self::Confianca => "confianca",
        }
    }
}
