//! Os NOMES PRÓPRIOS que o rádio fala: sobrenome de piloto e nome de equipe.
//!
//! Uma fala de quebra é montada por peças — `[abertura] [nome] [aposto] [trecho]` — e as duas
//! peças variáveis são estas. Sem elas o engenheiro só consegue dizer "um carro quebrou", que
//! é a informação que o jogador já tem olhando a tela.
//!
//! ## Por que uma TERCEIRA forma do nome da equipe
//!
//! [`TeamTemplate`](crate::constants::teams) já carrega duas: `nome` ("Grid Start Racing
//! School") e `nome_curto` ("GSR", a sigla da torre de tempos). Nenhuma das duas se fala. A
//! sigla vira soletração, e o nome inteiro gasta 1,4 s de rádio repetindo "Racing School"
//! num aviso que dura 3 s.
//!
//! O nome falado é o nome sem o sufixo genérico do automobilismo. Uma regra de sufixos acerta
//! 96 dos 102 e erra os 6 em que o sufixo genérico leva junto uma palavra que era do nome
//! ("Apex Academy Racing" → "Apex Academy", quando a equipe é a Apex). Como a regra precisa
//! de exceção mesmo, virou tabela: 102 pares revisados uma vez, sem regra para reconstruir
//! depois. O guard `equipes_faladas_cobrem_o_catalogo` prende as duas listas.
//!
//! ## Os sobrenomes vêm do pool, não daqui
//!
//! 355 sobrenomes únicos em 23 nacionalidades, e eles JÁ existem em
//! [`get_all_name_pools`](crate::generators::names::get_all_name_pools). Copiá-los para cá
//! criaria a segunda lista que diverge na primeira nacionalidade nova — e a divergência seria
//! silenciosa, porque um piloto com sobrenome sem áudio simplesmente não seria nomeado.
//!
//! Por isso a lista é derivada em tempo de execução e ordenada: o catálogo precisa sair igual
//! a cada chamada, senão o gerador de voz acha que mudou tudo.

use std::collections::BTreeSet;

use crate::generators::names::get_all_name_pools;

/// Prefixo das chaves de sobrenome no catálogo do engenheiro (`nm_wisniewski`).
pub const PREFIXO_SOBRENOME: &str = "nm_";
/// Prefixo das chaves de equipe no catálogo do engenheiro (`eq_kitsune`).
pub const PREFIXO_EQUIPE: &str = "eq_";

/// `(nome no catálogo de equipes, nome FALADO)`. Ver o cabeçalho do módulo para o critério.
pub static EQUIPES_FALADAS: &[(&str, &str)] = &[
    ("Racing Academy Red", "Racing Academy Red"),
    ("Thunderline Academy", "Thunderline"),
    ("Grid Start Racing School", "Grid Start"),
    ("First Gear Motorsport", "First Gear"),
    ("Apex Academy Racing", "Apex"),
    ("Velocity Prep Motorsport", "Velocity Prep"),
    ("Sakura Driver Academy", "Sakura"),
    ("Kanzen Racing School", "Kanzen"),
    ("Open Road Academy", "Open Road"),
    ("Speed Lab Rookies", "Speed Lab"),
    ("Rising Stars Motorsport", "Rising Stars"),
    ("Fresh Start Racing", "Fresh Start"),
    ("Roadster Touring Club", "Roadster"),
    ("Weekend Warriors Racing", "Weekend Warriors"),
    ("Club Racer Motorsport", "Club Racer"),
    ("Dual Exit Racing", "Dual Exit"),
    ("Sunday Speed Club", "Sunday Speed"),
    ("Grassroots Racing Team", "Grassroots"),
    ("Petrolhead United", "Petrolhead United"),
    ("Track Day Heroes", "Track Day Heroes"),
    ("Late Apex Contenders", "Late Apex Contenders"),
    ("Amateur Hour Racing", "Amateur Hour"),
    ("Eastline Cup Racing", "Eastline"),
    ("Street to Track Team", "Street to Track"),
    ("Flat Six Motorsport", "Flat Six"),
    // A regra deixaria "Gentleman", que sozinho não nomeia equipe nenhuma.
    ("Gentleman Driver Club", "Gentleman Driver"),
    ("Akagi Touring Challenge", "Akagi"),
    ("Rev Happy Racing", "Rev Happy"),
    ("Corner Workers Racing", "Corner Workers"),
    ("Daily Driver Racing", "Daily Driver"),
    ("Smooth Line Motorsport", "Smooth Line"),
    ("Over the Limit Racing", "Over the Limit"),
    ("Bayern Division", "Bayern"),
    ("M Power", "M Power"),
    ("Blue Propeller", "Blue Propeller"),
    // "Works" é sufixo genérico, "Speed" não é: a equipe é a Munich Speed.
    ("Munich Speed Works", "Munich Speed"),
    ("Isar Track", "Isar"),
    ("Eifel Sprint", "Eifel Sprint"),
    ("Corporate Express", "Corporate Express"),
    ("Roundel", "Roundel"),
    ("Southern Cross", "Southern Cross"),
    ("Black Forest Works", "Black Forest"),
    ("Aperture", "Aperture"),
    ("Backmesa", "Backmesa"),
    ("Northgate", "Northgate"),
    ("Kestrel", "Kestrel"),
    ("Overland", "Overland"),
    ("Rookfield", "Rookfield"),
    ("Komorebi", "Komorebi"),
    ("Nakatomi", "Nakatomi"),
    ("Hikari", "Hikari"),
    ("Redwell", "Redwell"),
    ("Ashford", "Ashford"),
    ("Tetsu", "Tetsu"),
    ("Nachtwerk", "Nachtwerk"),
    ("Adler", "Adler"),
    ("Eisen", "Eisen"),
    ("Kronstadt", "Kronstadt"),
    ("Vektor", "Vektor"),
    ("Lindenhaus", "Lindenhaus"),
    ("Rahal Letterman Racing", "Rahal Letterman"),
    ("Toksport World Racing", "Toksport"),
    // A regra come "Academy" e devolve "Stuttgart Racing"; "Racing" também é genérico aqui.
    ("Stuttgart Racing Academy", "Stuttgart"),
    ("Grove Drive Racing", "Grove Drive"),
    ("Formosa Corsa", "Formosa Corsa"),
    ("Silver Peak Performance", "Silver Peak"),
    // Tirar "Racing" deixaria "Heart of" — preposição solta. Fica inteiro.
    ("Heart of Racing", "Heart of Racing"),
    ("North Sea Motorsport", "North Sea"),
    ("Aures Racing", "Aures"),
    ("Aichi Works", "Aichi"),
    ("Waypoint", "Waypoint"),
    ("Farpoint", "Farpoint"),
    ("Northstar", "Northstar"),
    ("Mammoth", "Mammoth"),
    ("Atlas", "Atlas"),
    ("Outpost", "Outpost"),
    ("Mercedes-AMG", "Mercedes-AMG"),
    ("Porsche", "Porsche"),
    ("Ferrari", "Ferrari"),
    ("McLaren", "McLaren"),
    ("Lamborghini", "Lamborghini"),
    ("BMW", "BMW"),
    ("Audi", "Audi"),
    ("Aston Martin", "Aston Martin"),
    ("Chevrolet", "Chevrolet"),
    ("Ford Mustang", "Ford Mustang"),
    ("Acura", "Acura"),
    ("Obsidian", "Obsidian"),
    ("Kitsune", "Kitsune"),
    ("Valkyrie", "Valkyrie"),
    ("Solaris", "Solaris"),
    ("Peregrine", "Peregrine"),
    ("Arclight", "Arclight"),
    ("Blackwell", "Blackwell"),
    ("Stratos", "Stratos"),
    ("Helion", "Helion"),
    ("United Autosports", "United"),
    ("Jota Sport", "Jota"),
    // "Belgian" sozinho é gentílico, não nome de equipe.
    ("Belgian Racing Team", "Belgian Racing"),
    ("Prema Powerteam", "Prema"),
    // "Cool" sozinho vira adjetivo no meio da frase.
    ("Cool Racing", "Cool Racing"),
    ("Meridian", "Meridian"),
];

/// O nome falado da equipe. `None` quando a equipe não está no catálogo — o que acontece com
/// save antigo e com equipe de teste, e é o sinal para o rádio cair na forma sem vínculo.
pub fn equipe_falada(nome: &str) -> Option<&'static str> {
    EQUIPES_FALADAS
        .iter()
        .find(|(catalogo, _)| *catalogo == nome)
        .map(|(_, falado)| *falado)
}

/// Os 355 sobrenomes únicos dos pools, ordenados. A ordem importa: o catálogo tem que sair
/// idêntico a cada chamada, senão o gerador de voz vê a lista inteira como nova.
pub fn sobrenomes() -> Vec<&'static str> {
    get_all_name_pools()
        .iter()
        .flat_map(|p| p.sobrenomes.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Texto → chave de arquivo. Minúsculas, sem acento, o resto vira `_`.
///
/// Os pools guardam os sobrenomes sem acento de propósito (`Hamalainen`, não `Hämäläinen`),
/// mas a dobra existe aqui mesmo assim: o nome do jogador entra por este caminho e ele digita
/// o que quiser. Sem a dobra, "Gonçalves" viraria `gon_alves` — chave válida, arquivo que
/// ninguém gravou.
pub fn slug(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    let mut ultimo_underscore = false;
    for c in texto.chars() {
        let base = dobrar(c);
        if base.is_ascii_alphanumeric() {
            s.push(base.to_ascii_lowercase());
            ultimo_underscore = false;
        } else if !ultimo_underscore && !s.is_empty() {
            s.push('_');
            ultimo_underscore = true;
        }
    }
    while s.ends_with('_') {
        s.pop();
    }
    s
}

/// Latin-1 acentuado → a letra sem acento. Fora dessa faixa devolve o próprio caractere, que
/// o `slug` descarta como separador.
fn dobrar(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        'ý' | 'ÿ' => 'y',
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'O',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'Ç' => 'C',
        'Ñ' => 'N',
        'ß' => 's',
        outro => outro,
    }
}

/// A chave de áudio do sobrenome, ou `None` se não houver gravação para ele.
///
/// A checagem contra o pool não é zelo: o nome do JOGADOR passa por aqui, e ele não sai de
/// pool nenhum. Sem esta porta o rádio pediria `nm_magnossilva.wav`, não acharia, e a fala
/// inteira morreria — em vez de cair na forma pela equipe, que existe justamente para isto.
pub fn chave_sobrenome(sobrenome: &str) -> Option<String> {
    let alvo = slug(sobrenome);
    sobrenomes()
        .iter()
        .any(|s| slug(s) == alvo)
        .then(|| format!("{PREFIXO_SOBRENOME}{alvo}"))
}

/// A chave de áudio do nome da equipe, ou `None` se a equipe não estiver no catálogo.
pub fn chave_equipe(nome: &str) -> Option<String> {
    equipe_falada(nome).map(|falado| format!("{PREFIXO_EQUIPE}{}", slug(falado)))
}

/// O último token do nome completo — o sobrenome, como o rádio o diz.
///
/// "James Cooper" → "Cooper". Nome de uma palavra devolve ele mesmo. É deliberadamente burro:
/// partícula ("van Dijk") não é tratada porque os pools guardam essas grafias CONCATENADAS
/// (`vanDijk`), então o token único já é o nome inteiro.
pub fn sobrenome_de(nome_completo: &str) -> &str {
    nome_completo.rsplit(' ').next().unwrap_or(nome_completo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equipes_faladas_cobrem_o_catalogo() {
        // O acoplamento que ninguém percebe quebrar: uma equipe nova em `constants/teams`
        // compila, entra no mundo e fica MUDA no rádio. Sem este teste o sintoma é silêncio.
        let catalogo: Vec<&str> = crate::constants::teams::get_all_team_templates()
            .iter()
            .map(|t| t.nome)
            .collect();
        let faladas: Vec<&str> = EQUIPES_FALADAS.iter().map(|(n, _)| *n).collect();
        let faltando: Vec<_> = catalogo.iter().filter(|n| !faladas.contains(n)).collect();
        assert!(faltando.is_empty(), "equipes sem nome falado: {faltando:?}");
        let sobrando: Vec<_> = faladas.iter().filter(|n| !catalogo.contains(n)).collect();
        assert!(
            sobrando.is_empty(),
            "nome falado de equipe inexistente: {sobrando:?}"
        );
    }

    #[test]
    fn nome_falado_nunca_termina_em_preposicao_nem_fica_vazio() {
        for (nome, falado) in EQUIPES_FALADAS {
            assert!(!falado.trim().is_empty(), "{nome} ficou sem nome falado");
            let ultimo = falado.rsplit(' ').next().unwrap_or(falado).to_lowercase();
            assert!(
                !["of", "the", "to", "de", "da", "and"].contains(&ultimo.as_str()),
                "{nome} → \"{falado}\" termina em palavra solta",
            );
        }
    }

    #[test]
    fn as_chaves_de_equipe_nao_colidem() {
        // Duas equipes com o mesmo nome falado gerariam UM arquivo e duas equipes com a mesma
        // voz — o jogador ouviria a equipe errada sem nada quebrar em lugar nenhum.
        let mut chaves: Vec<String> = EQUIPES_FALADAS
            .iter()
            .filter_map(|(n, _)| chave_equipe(n))
            .collect();
        let total = chaves.len();
        chaves.sort();
        chaves.dedup();
        assert_eq!(total, chaves.len(), "chaves de equipe repetidas");
        assert_eq!(total, EQUIPES_FALADAS.len());
    }

    #[test]
    fn as_chaves_de_sobrenome_nao_colidem() {
        let nomes = sobrenomes();
        let mut chaves: Vec<String> = nomes.iter().map(|s| slug(s)).collect();
        let total = chaves.len();
        chaves.sort();
        chaves.dedup();
        assert_eq!(total, chaves.len(), "dois sobrenomes com a mesma chave");
    }

    #[test]
    fn a_lista_de_sobrenomes_e_estavel_e_do_tamanho_medido() {
        let a = sobrenomes();
        let b = sobrenomes();
        assert_eq!(a, b, "a ordem mudou entre duas chamadas");
        // 435 entradas nos 23 pools, 355 únicas. Se este número mudar foi porque alguém mexeu
        // nos pools — e aí há sobrenome novo para GRAVAR, não só para compilar.
        assert_eq!(a.len(), 355, "o pool de sobrenomes mudou de tamanho");
    }

    #[test]
    fn o_slug_dobra_acento_e_nao_deixa_buraco() {
        assert_eq!(slug("Gonçalves"), "goncalves");
        assert_eq!(slug("Hämäläinen"), "hamalainen");
        assert_eq!(slug("Mercedes-AMG"), "mercedes_amg");
        assert_eq!(slug("Racing Academy Red"), "racing_academy_red");
        assert_eq!(slug("  Over the Limit  "), "over_the_limit");
    }

    #[test]
    fn sobrenome_fora_do_pool_nao_ganha_chave() {
        // O caso do JOGADOR. Ele digita o nome; não há gravação. A porta tem que fechar aqui,
        // e não no `fetch` do arquivo.
        assert!(chave_sobrenome("Cooper").is_some());
        assert!(chave_sobrenome("Magnossilva").is_none());
    }

    #[test]
    fn o_sobrenome_e_o_ultimo_token() {
        assert_eq!(sobrenome_de("James Cooper"), "Cooper");
        assert_eq!(sobrenome_de("Cooper"), "Cooper");
        assert_eq!(sobrenome_de("Jean Pierre Dubois"), "Dubois");
    }
}
