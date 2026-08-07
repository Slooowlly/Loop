use serde::{Deserialize, Serialize};

use crate::models::driver::DriverAttributes;

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

/// Teto do `Defeito`, e o motivo dele ser 32 e nao 25.
///
/// O gerador de rookie marca 8 dos 12 pilotos de um grid como `Flawed` e joga
/// 2-3 atributos deles em `U{25..=32}` ([`crate::models::driver_generation`]).
/// O limiar era 25. Os dois numeros paravam a sete pontos um do outro, entao o
/// gerador criava o defeito e a ficha nao contava: a tag so disparava quando o
/// dado caia exatamente em 25, 12,5% das vezes. Alinhar pelo lado do LIMIAR e
/// nao pelo lado do gerador porque este nao mexe em simulacao — 30 de gestao de
/// pneus continua sendo 30 na corrida, so passa a ter nome na tela.
const DEFEITO_MAX: u8 = 32;

/// Quanto o piloto precisa se afastar da mediana do grid para o eixo virar traco
/// mesmo sem cruzar limiar absoluto nenhum. Ver [`visible_tags_in_grid`].
const DESVIO_DO_GRID: f64 = 12.0;

/// Minimo de pilotos para a mediana do grid valer alguma coisa. Com dois, "a
/// mediana" e so o outro piloto com nome de estatistica — mesma regra da regua
/// da leitura tecnica, e pela mesma razao.
const GRID_MINIMO: usize = 3;

pub(crate) fn get_attribute_tag(attribute_name: &'static str, value: f64) -> Option<AttributeTag> {
    monta_tag(attribute_name, nivel_absoluto(value)?)
}

fn nivel_absoluto(value: f64) -> Option<TagLevel> {
    let rounded = value.round() as u8;
    if rounded <= 10 {
        Some(TagLevel::DefeitoGrave)
    } else if rounded <= DEFEITO_MAX {
        Some(TagLevel::Defeito)
    } else if rounded <= 74 {
        None
    } else if rounded <= 84 {
        Some(TagLevel::Qualidade)
    } else if rounded <= 94 {
        Some(TagLevel::QualidadeAlta)
    } else {
        Some(TagLevel::Elite)
    }
}

fn monta_tag(attribute_name: &'static str, level: TagLevel) -> Option<AttributeTag> {
    let index = match level {
        TagLevel::DefeitoGrave => 0,
        TagLevel::Defeito => 1,
        TagLevel::Qualidade => 2,
        TagLevel::QualidadeAlta => 3,
        TagLevel::Elite => 4,
    };
    let tag_text = tag_text_for(attribute_name, index)?;
    Some(AttributeTag {
        attribute_name,
        tag_text,
        level,
    })
}

/// Os tracos do piloto LIDOS CONTRA O GRID em que ele corre.
///
/// A escala absoluta e a da escada inteira, e e ela que da sentido as palavras
/// altas: "Alien" tem de querer dizer alien em qualquer categoria, senao a
/// piramide de nove degraus vira nove jogos separados. Mas na base da escada ela
/// emudece a ficha por construcao — um grid de mazda vive entre 36 e 75, a faixa
/// 26..74 nao tem tag, e nenhum eixo correlacionado a skill consegue os 75 que a
/// primeira qualidade pede. O piloto tem forcas e fraquezas, e a tela mostra
/// nada.
///
/// A saida e a mesma ancora que a leitura tecnica ja usa na regua: a MEDIANA DO
/// GRID. Quem esta [`DESVIO_DO_GRID`] pontos acima ou abaixo dos vizinhos de
/// garagem ganha tag mesmo dentro da faixa muda.
///
/// O relativo so PROMOVE quem nao tinha tag, e so ate o degrau mais baixo de
/// cada lado (`Qualidade` / `Defeito`). Ele nunca contradiz nem eleva o
/// absoluto, porque as duas escalas respondem perguntas diferentes e deixar a
/// relativa vencer produziria mentira nos dois sentidos: um 76 num grid de 90
/// viraria "Porta Aberta", e o melhor de um grid ruim viraria "Alien".
pub(crate) fn visible_tags_in_grid(
    atributos: &DriverAttributes,
    grid: &[DriverAttributes],
) -> Vec<AttributeTag> {
    let medianas = medianas_do_grid(grid);

    atributos
        .entries()
        .into_iter()
        .filter_map(|(attribute_name, value)| {
            if let Some(level) = nivel_absoluto(value) {
                return monta_tag(attribute_name, level);
            }
            let mediana = medianas
                .iter()
                .find(|(nome, _)| *nome == attribute_name)
                .map(|(_, valor)| *valor)?;
            let delta = value - mediana;
            let level = if delta >= DESVIO_DO_GRID {
                TagLevel::Qualidade
            } else if delta <= -DESVIO_DO_GRID {
                TagLevel::Defeito
            } else {
                return None;
            };
            monta_tag(attribute_name, level)
        })
        .collect()
}

/// Mediana de cada eixo no grid. Eixo a eixo e nao um piloto fantasma com a
/// mediana de tudo: esse piloto nao existe no grid, e guardar um so convidaria a
/// lê-lo como se existisse.
fn medianas_do_grid(grid: &[DriverAttributes]) -> Vec<(&'static str, f64)> {
    if grid.len() < GRID_MINIMO {
        return Vec::new();
    }

    let eixos = grid[0].entries();
    eixos
        .iter()
        .enumerate()
        .map(|(indice, (nome, _))| {
            let mut valores: Vec<f64> = grid
                .iter()
                .map(|atributos| atributos.entries()[indice].1.clamp(0.0, 100.0))
                .collect();
            valores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            (*nome, valores[valores.len() / 2])
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::driver::Driver;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use std::collections::HashSet;

    fn atributos(gestao_pneus: f64) -> DriverAttributes {
        DriverAttributes {
            gestao_pneus,
            ..Default::default()
        }
    }

    /// O relativo so fala onde o absoluto se cala. Um piloto de 76 num grid de 90
    /// e fraco PARA AQUELE GRID, mas chama-lo de "Destruidor de Pneus" seria
    /// mentira: 76 e bom em qualquer lugar da escada, e a palavra alta tem de
    /// continuar querendo dizer o que diz.
    #[test]
    fn o_absoluto_manda_quando_existe() {
        let grid: Vec<DriverAttributes> = [90.0, 90.0, 90.0, 90.0, 90.0]
            .into_iter()
            .map(atributos)
            .collect();
        let tags = visible_tags_in_grid(&atributos(76.0), &grid);
        let pneus = tags
            .iter()
            .find(|tag| tag.attribute_name == "gestao_pneus")
            .expect("76 cruza o limiar absoluto de Qualidade");

        assert_eq!(pneus.level, TagLevel::Qualidade);
    }

    /// E o caso que motivou tudo: a faixa 26..74 nao tem tag absoluta, entao um
    /// grid inteiro de categoria de base saia mudo. Contra a mediana, quem se
    /// afasta doze pontos ganha nome.
    #[test]
    fn o_relativo_fala_dentro_da_faixa_muda() {
        let grid: Vec<DriverAttributes> = [40.0, 44.0, 46.0, 48.0, 52.0]
            .into_iter()
            .map(atributos)
            .collect();

        let bom = visible_tags_in_grid(&atributos(62.0), &grid);
        assert_eq!(
            bom.iter()
                .find(|tag| tag.attribute_name == "gestao_pneus")
                .map(|tag| tag.level.clone()),
            Some(TagLevel::Qualidade),
            "62 contra mediana 46 e +16: o eixo separa ele do grid"
        );

        let ruim = visible_tags_in_grid(&atributos(33.0), &grid);
        assert_eq!(
            ruim.iter()
                .find(|tag| tag.attribute_name == "gestao_pneus")
                .map(|tag| tag.level.clone()),
            Some(TagLevel::Defeito),
            "33 contra mediana 46 e -13"
        );

        let medio = visible_tags_in_grid(&atributos(50.0), &grid);
        assert!(
            !medio.iter().any(|tag| tag.attribute_name == "gestao_pneus"),
            "quem esta no meio do grid nao tem o que anunciar"
        );
    }

    /// Com menos de tres pilotos "a mediana" e so o outro piloto com nome de
    /// estatistica — mesma regra da regua da leitura tecnica.
    #[test]
    fn grid_pequeno_nao_inventa_mediana() {
        let grid: Vec<DriverAttributes> = [40.0, 44.0].into_iter().map(atributos).collect();
        let tags = visible_tags_in_grid(&atributos(62.0), &grid);

        assert!(!tags.iter().any(|tag| tag.attribute_name == "gestao_pneus"));
    }

    /// O defeito comum do rookie nasce em `U{25..=32}`, e o limiar de `Defeito`
    /// era 25: o gerador criava a fraqueza e a ficha nao contava. Com o teto em
    /// 32 a faixa inteira tem nome.
    #[test]
    fn o_defeito_do_rookie_cabe_no_limiar() {
        for valor in 25..=32 {
            let tag = get_attribute_tag("gestao_pneus", f64::from(valor))
                .unwrap_or_else(|| panic!("{valor} devia ser Defeito"));
            assert_eq!(tag.level, TagLevel::Defeito, "valor {valor}");
        }
        assert!(get_attribute_tag("gestao_pneus", 33.0).is_none());
    }

    /// O teste que faltava e que teria pego tudo isto: um grid de mazda inteiro
    /// sem um traco sequer. Nada media distribuicao de TAG — o guard mais proximo
    /// contava o atributo baixo, nao a tag que ele deveria virar.
    #[test]
    fn um_grid_de_rookie_nao_nasce_mudo() {
        let mut rng = StdRng::seed_from_u64(4242);
        let mut nomes = HashSet::new();
        let pilotos =
            Driver::generate_for_category("mazda_rookie", 0, "medio", 12, &mut nomes, &mut rng);
        let grid: Vec<DriverAttributes> = pilotos
            .iter()
            .map(|piloto| piloto.atributos.clone())
            .collect();

        let com_traco = pilotos
            .iter()
            .filter(|piloto| !visible_tags_in_grid(&piloto.atributos, &grid).is_empty())
            .count();

        assert_eq!(
            com_traco, 12,
            "todo piloto de um grid de base tem alguma coisa que o separa do vizinho"
        );
    }
}
