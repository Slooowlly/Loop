//! Onde os países ficam — a geografia que a LOGÍSTICA precisa.
//!
//! ## Por que isto existe
//!
//! A fatura classificava a viagem em três faixas — casa (220 km), mesmo continente
//! (1.250 km), outro continente (8.500 km). Visto de dentro do modelo é uma
//! aproximação razoável; visto na tela é um defeito de credibilidade, e foi assim que
//! apareceu: **uma etapa em Snetterton (Reino Unido) e outra em Rudskogen (Noruega)
//! cobraram exatamente os mesmos 9.679,4 km de frete, ao centavo.** Um jogador atento vê
//! duas corridas em países diferentes cobrarem igual e conclui, com razão, que o número é
//! decorativo — que é exatamente a percepção que este redesign existe para corrigir.
//!
//! ## O que é, e o que não é
//!
//! É a distância de grande círculo entre um ponto de referência do país da equipe e um do
//! país do circuito. **Não** é a distância entre a sede e a pista: o jogo não guarda
//! endereço de sede, e inventar um seria falsa precisão do tipo que esta reforma remove.
//! O que se ganha é o que se pretendia: países diferentes deixam de cobrar o mesmo.
//!
//! Os pontos NÃO são centroides geográficos — são **onde a corrida acontece** naquele
//! país, que é o que a transportadora percorre. O centroide da Noruega fica em 64,5°N; o
//! automobilismo norueguês fica em Rudskogen, a 59,4°N, quase 600 km ao sul. Para um
//! modelo de frete, o centroide erraria mais que a escolha declarada.
//!
//! Nomes de país chegam de duas fontes com formatos diferentes — `constants/teams` e
//! `constants/tracks` usam bandeira emoji, variantes legadas vêm sem, e há entradas sem
//! acento (`Japao`, `Suica`, `Franca`). A normalização derruba emoji, acento e caixa.

/// Ponto de referência de um país: `(nome, latitude, longitude)`.
///
/// Um país fora desta lista não quebra nada — quem chama cai na faixa de referência
/// (ver `commands::race::despesa::distancia_da_sede_km`). Falta de dado nunca vira
/// distância inventada.
const PONTOS: &[(&str, f64, f64)] = &[
    // ── Europa ───────────────────────────────────────────────────────────────────
    ("reino unido", 52.5, -1.5),      // Silverstone/Donington/Brands, miolo inglês
    ("alemanha", 50.3, 8.0),          // Nürburgring/Hockenheim
    ("franca", 47.0, 3.0),            // Magny-Cours/Le Mans
    ("italia", 44.8, 10.5),           // Monza/Imola/Mugello
    ("espanha", 41.3, 2.0),           // Barcelona; Jerez puxa para o sul, Aragón para o norte
    ("holanda", 52.4, 4.6),           // Zandvoort
    ("belgica", 50.4, 5.9),           // Spa
    ("portugal", 38.8, -9.0),         // Estoril/Portimão
    ("austria", 47.2, 14.8),          // Red Bull Ring
    ("suica", 46.9, 7.4),             // sem circuito no jogo; sede de equipe
    ("noruega", 59.4, 11.4),          // Rudskogen
    ("hungria", 47.6, 19.2),          // Hungaroring
    ("finlandia", 60.5, 22.3),
    ("suecia", 58.4, 13.5),
    ("dinamarca", 55.7, 12.1),
    ("polonia", 52.0, 20.5),
    ("irlanda", 53.3, -6.3),
    ("republica tcheca", 49.4, 17.0),
    // ── Américas ────────────────────────────────────────────────────────────────
    ("eua", 39.5, -98.4),   // o país é largo; o centro é a melhor aposta única
    ("canada", 45.5, -74.5), // Montreal/Mosport, onde o automobilismo canadense mora
    ("mexico", 19.4, -99.1), // Hermanos Rodríguez
    ("brasil", -23.6, -46.7), // Interlagos
    ("argentina", -34.7, -58.5),
    // ── Ásia e Oceania ──────────────────────────────────────────────────────────
    ("japao", 35.4, 138.9),      // Suzuka/Fuji/Motegi
    ("taiwan", 23.7, 121.0),
    ("china", 31.3, 121.2),      // Xangai
    ("coreia do sul", 34.7, 126.4),
    ("emirados", 24.5, 54.6),    // Yas Marina
    ("bahrein", 26.0, 50.5),
    ("catar", 25.5, 51.5),
    ("india", 28.4, 77.5),
    ("australia", -34.0, 145.0), // Bathurst/Phillip Island/Sandown, sudeste
    ("nova zelandia", -37.2, 175.0),
    // ── África ──────────────────────────────────────────────────────────────────
    ("africa do sul", -25.9, 28.1), // Kyalami
];

/// Raio médio da Terra, em km.
const RAIO_DA_TERRA_KM: f64 = 6_371.0;

/// Coordenada de referência de um país, se ele estiver no mapa.
pub fn coordenadas(pais: &str) -> Option<(f64, f64)> {
    let alvo = normalizar(pais);
    if alvo.is_empty() {
        return None;
    }
    PONTOS
        .iter()
        .find(|(nome, _, _)| *nome == alvo)
        .map(|(_, lat, lon)| (*lat, *lon))
}

/// Distância de grande círculo entre dois países, em km. `None` se qualquer um dos dois
/// estiver fora do mapa — quem chama decide o que fazer com a falta de dado, e a resposta
/// certa nunca é chutar um número.
pub fn distancia_entre_paises_km(origem: &str, destino: &str) -> Option<f64> {
    let (lat_a, lon_a) = coordenadas(origem)?;
    let (lat_b, lon_b) = coordenadas(destino)?;
    Some(haversine_km(lat_a, lon_a, lat_b, lon_b))
}

/// Os dois países são o mesmo? Tolera bandeira, acento e caixa.
pub fn mesmo_pais(a: &str, b: &str) -> bool {
    let (a, b) = (normalizar(a), normalizar(b));
    !a.is_empty() && a == b
}

/// Continente do país (formato "🇧🇷 Brasil", com ou sem bandeira). `None` quando o país não
/// está no mapa — quem chama trata a falta de dado como "mesmo continente", nunca como
/// penalidade.
///
/// É a classificação GROSSA, e existe só como queda de [`distancia_entre_paises_km`]: quando
/// um dos dois países não tem coordenada, a única coisa que ainda dá para dizer é se a viagem
/// atravessa oceano. Compara por `contains` sobre o nome cru, com as variantes acentuadas e
/// sem acento lado a lado, porque é assim que o dado real vem — e porque este era o critério
/// exato do modelo de despesa antigo, que continua rodando no harness como base de comparação.
pub fn continente(pais: &str) -> Option<&'static str> {
    const EUROPA: &[&str] = &[
        "Áustria",
        "Austria",
        "Bélgica",
        "Belgica",
        "Alemanha",
        "Espanha",
        "França",
        "Franca",
        "Reino Unido",
        "Hungria",
        "Itália",
        "Italia",
        "Holanda",
        "Noruega",
        "Portugal",
        "Suíça",
        "Suica",
        "Finlândia",
        "Finlandia",
        "Suécia",
        "Suecia",
        "Dinamarca",
        "Polônia",
        "Polonia",
        "Irlanda",
        "República Tcheca",
        "Republica Tcheca",
    ];
    const AMERICA_NORTE: &[&str] = &["EUA", "Canadá", "Canada", "México", "Mexico"];
    const AMERICA_SUL: &[&str] = &["Brasil", "Argentina"];
    const ASIA: &[&str] = &[
        "Japão",
        "Japao",
        "Taiwan",
        "China",
        "Coreia do Sul",
        "Emirados",
        "Bahrein",
        "Catar",
        "Índia",
        "India",
    ];
    const OCEANIA: &[&str] = &["Austrália", "Australia", "Nova Zelândia", "Nova Zelandia"];
    const AFRICA: &[&str] = &["África do Sul", "Africa do Sul"];

    // A Oceania vem primeiro de propósito: "Austrália" contém "Austria" sem acento, e na
    // ordem inversa a Austrália cairia na Europa.
    for (nomes, nome_continente) in [
        (OCEANIA, "oceania"),
        (EUROPA, "europa"),
        (AMERICA_NORTE, "america_norte"),
        (AMERICA_SUL, "america_sul"),
        (ASIA, "asia"),
        (AFRICA, "africa"),
    ] {
        if nomes.iter().any(|n| pais.contains(n)) {
            return Some(nome_continente);
        }
    }
    None
}

fn haversine_km(lat_a: f64, lon_a: f64, lat_b: f64, lon_b: f64) -> f64 {
    let (fi_a, fi_b) = (lat_a.to_radians(), lat_b.to_radians());
    let d_fi = (lat_b - lat_a).to_radians();
    let d_lambda = (lon_b - lon_a).to_radians();
    let h = (d_fi / 2.0).sin().powi(2) + fi_a.cos() * fi_b.cos() * (d_lambda / 2.0).sin().powi(2);
    2.0 * RAIO_DA_TERRA_KM * h.sqrt().clamp(0.0, 1.0).asin()
}

/// Derruba bandeira emoji, acento e caixa. Sem o dobramento de acento "Japão" e "Japao"
/// seriam países diferentes, e o dado real tem os dois.
fn normalizar(pais: &str) -> String {
    pais.chars()
        .filter_map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => Some('a'),
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => Some('e'),
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => Some('i'),
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => Some('o'),
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => Some('u'),
            'ç' | 'Ç' => Some('c'),
            c if c.is_alphabetic() => Some(c.to_ascii_lowercase()),
            c if c.is_whitespace() => Some(' '),
            _ => None,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_normalizacao_derruba_bandeira_acento_e_caixa() {
        for variante in ["🇯🇵 Japão", "Japao", "japão", " JAPÃO "] {
            assert!(
                coordenadas(variante).is_some(),
                "variante não resolveu: {variante:?}"
            );
        }
        assert!(mesmo_pais("🇫🇷 França", "Franca"));
        assert!(mesmo_pais("Suíça", "🇨🇭 Suica"));
        assert!(!mesmo_pais("Brasil", "Portugal"));
        assert!(!mesmo_pais("", ""));
    }

    /// **O caso que originou o módulo.** Reino Unido e Noruega tinham a mesma distância
    /// porque caíam na mesma faixa. Agora não têm.
    #[test]
    fn paises_diferentes_deixam_de_empatar() {
        let ao_reino = distancia_entre_paises_km("Brasil", "🇬🇧 Reino Unido").expect("mapa");
        let a_noruega = distancia_entre_paises_km("Brasil", "🇳🇴 Noruega").expect("mapa");
        assert!(
            (ao_reino - a_noruega).abs() > 500.0,
            "Reino Unido {ao_reino:.0} km e Noruega {a_noruega:.0} km continuam empatados"
        );
    }

    /// Ordens de grandeza conferidas contra distâncias conhecidas — se a fórmula ou uma
    /// coordenada saírem do lugar, é aqui que aparece.
    #[test]
    fn as_distancias_batem_com_a_realidade() {
        let casos: [(&str, &str, f64); 5] = [
            ("Reino Unido", "Alemanha", 700.0),
            ("Brasil", "Reino Unido", 9_400.0),
            ("EUA", "Japão", 9_700.0),
            ("Itália", "Espanha", 900.0),
            ("Austrália", "Reino Unido", 17_000.0),
        ];
        for (a, b, esperado) in casos {
            let medido = distancia_entre_paises_km(a, b).expect("mapa");
            let desvio = (medido - esperado).abs() / esperado;
            assert!(
                desvio < 0.25,
                "{a} → {b}: esperado ~{esperado:.0} km, medido {medido:.0} km"
            );
        }
    }

    #[test]
    fn a_distancia_e_simetrica_e_zero_no_mesmo_pais() {
        let ida = distancia_entre_paises_km("Itália", "Japão").expect("mapa");
        let volta = distancia_entre_paises_km("Japão", "Itália").expect("mapa");
        assert!((ida - volta).abs() < 1e-6);
        assert!(distancia_entre_paises_km("EUA", "EUA").expect("mapa") < 1e-6);
    }

    #[test]
    fn pais_fora_do_mapa_devolve_nada_em_vez_de_chutar() {
        assert!(coordenadas("Atlântida").is_none());
        assert!(distancia_entre_paises_km("Brasil", "Atlântida").is_none());
        assert!(distancia_entre_paises_km("", "Brasil").is_none());
    }

    /// GUARD: todo país que aparece em `constants/tracks` ou `constants/teams` tem que
    /// estar no mapa. Um país novo no catálogo sem coordenada volta silenciosamente para
    /// a faixa de referência, e o empate que este módulo removeu reaparece só para ele.
    #[test]
    fn todo_pais_do_catalogo_esta_no_mapa() {
        let mut faltando: Vec<String> = Vec::new();
        for track in crate::constants::tracks::get_all_tracks() {
            if coordenadas(track.pais).is_none() {
                faltando.push(format!("pista: {}", track.pais));
            }
        }
        for equipe in crate::constants::teams::get_all_team_templates() {
            if coordenadas(equipe.pais_sede).is_none() {
                faltando.push(format!("equipe: {}", equipe.pais_sede));
            }
        }
        faltando.sort();
        faltando.dedup();
        assert!(faltando.is_empty(), "países sem coordenada: {faltando:?}");
    }
}
