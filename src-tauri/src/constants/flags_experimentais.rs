//! Inventário das flags de EXPERIMENTO que mudam regra de jogo por variável de ambiente.
//!
//! Elas existem para o A/B do harness de Monte Carlo (`sim_stats`): ligar um mecanismo,
//! rodar N temporadas, comparar. O problema é que uma regra de jogo decidida pelo
//! ambiente é irreproduzível no relato de bug do jogador — duas máquinas com o mesmo
//! save podem simular temporadas diferentes, e nada na tela conta isso.
//!
//! Enquanto elas estiverem de pé, este módulo é o lugar ÚNICO onde estão declaradas:
//! nome, valor padrão do jogo, quem lê e o que muda. [`INVENTARIO`] é a lista, e o
//! teste no fim do arquivo cobra que nenhuma flag nova apareça no mercado ou na escada
//! sem passar por aqui.
//!
//! **Cada entrada carrega um [`Destino`]**: se a decisão de produto já foi tomada
//! (virar padrão definitivo e sumir com a flag) ou se ela ainda serve para um A/B em
//! aberto. Fechar as `Indefinido` é decisão de produto/calibração, não de código.

/// Como a flag lê o ambiente.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TipoDaFlag {
    /// Liga/desliga, com o padrão do jogo quando a env não está definida.
    Booleana { padrao: bool },
    /// Número de calibração (só tem efeito com a flag dona ligada).
    Numerica { padrao: f64 },
}

/// O que ainda falta decidir sobre a flag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Destino {
    /// O padrão já é a regra do jogo; a flag só sobrevive para o A/B poder desligá-la.
    /// Candidata a sumir junto com o experimento.
    PadraoDefinitivo,
    /// O experimento ainda não fechou: o padrão pode mudar conforme a medição.
    Indefinido,
}

/// Uma flag de experimento declarada.
#[derive(Debug, Clone, Copy)]
pub struct FlagExperimental {
    pub nome: &'static str,
    pub tipo: TipoDaFlag,
    /// Módulo que lê a flag (caminho no crate).
    pub dono: &'static str,
    /// O que muda no jogo quando ela sai do padrão.
    pub efeito: &'static str,
    pub destino: Destino,
}

/// TODAS as flags de regra de jogo lidas do ambiente pelo mercado e pela escada.
///
/// Ordem: mercado primeiro, escada depois — a mesma ordem em que a virada de temporada
/// as executa.
pub const INVENTARIO: &[FlagExperimental] = &[
    FlagExperimental {
        nome: "IRACER_ROOKIE_MERIT",
        tipo: TipoDaFlag::Booleana { padrao: false },
        dono: "market::pipeline::consolidacao",
        efeito: "força a troca do 1º do Rookie com o pior do Amador quando o Amador está cheio",
        destino: Destino::Indefinido,
    },
    FlagExperimental {
        nome: "IRACER_MARKET_AFFORDABILITY",
        tipo: TipoDaFlag::Booleana { padrao: true },
        dono: "market::pipeline::consolidacao",
        efeito: "ordem de escolha dos assentos por prestígio + penalidade de quem o time não pode pagar",
        destino: Destino::PadraoDefinitivo,
    },
    FlagExperimental {
        nome: "IRACER_PROMO_SOFT_LANDING",
        tipo: TipoDaFlag::Booleana { padrao: true },
        dono: "promotion::pipeline",
        efeito: "o promovido aterrissa no nível de peça do pior incumbente da categoria de destino",
        destino: Destino::PadraoDefinitivo,
    },
    FlagExperimental {
        nome: "IRACER_PROMO_DIMINISH",
        tipo: TipoDaFlag::Booleana { padrao: true },
        dono: "promotion::effects",
        efeito: "retorno decrescente do pacote econômico em promoções encadeadas (anti-snowball)",
        destino: Destino::PadraoDefinitivo,
    },
    FlagExperimental {
        nome: "IRACER_PROMO_DIMINISH_DECAY",
        tipo: TipoDaFlag::Numerica { padrao: 0.55 },
        dono: "promotion::effects",
        efeito: "fator de cada promoção encadeada dentro da janela (decay^(n-1))",
        destino: Destino::Indefinido,
    },
    FlagExperimental {
        nome: "IRACER_PROMO_DIMINISH_WINDOW",
        tipo: TipoDaFlag::Numerica { padrao: 3.0 },
        dono: "promotion::effects",
        efeito: "em quantas temporadas as promoções ainda contam como encadeadas",
        destino: Destino::Indefinido,
    },
    FlagExperimental {
        nome: "IRACER_PROMO_DIMINISH_FLOOR",
        tipo: TipoDaFlag::Numerica { padrao: 0.15 },
        dono: "promotion::effects",
        efeito: "piso do fator, para uma sequência longa nunca zerar o pacote",
        destino: Destino::Indefinido,
    },
];

/// Lê uma flag booleana do inventário. `1/true/on/sim/yes` liga, `0/false/off/nao/no`
/// desliga; qualquer outro valor (e a env ausente) devolve o padrão declarado.
///
/// Antes cada flag tinha o seu parser: as ligadas por padrão aceitavam `0/false/off` para
/// desligar, as desligadas só aceitavam `1/true` para ligar. Valor esquisito (`on` numa,
/// `talvez` noutra) caía num lado ou no outro sem regra. Aqui é uma leitura só, e o que
/// não é reconhecido vale o padrão do jogo — o desfecho seguro.
///
/// **Pânico em debug** se `nome` não estiver no [`INVENTARIO`]: flag lida sem declarar é
/// exatamente o que este módulo existe para impedir.
pub fn booleana(nome: &str) -> bool {
    let padrao = match declarada(nome).map(|flag| flag.tipo) {
        Some(TipoDaFlag::Booleana { padrao }) => padrao,
        Some(TipoDaFlag::Numerica { .. }) => {
            debug_assert!(false, "flag '{nome}' esta declarada como numerica");
            false
        }
        None => {
            debug_assert!(false, "flag '{nome}' nao esta no INVENTARIO");
            false
        }
    };
    std::env::var(nome)
        .ok()
        .and_then(|valor| interpretar_booleana(&valor))
        .unwrap_or(padrao)
}

/// Lê uma flag numérica do inventário; valor ilegível (ou env ausente) devolve o padrão
/// declarado. O `clamp` de cada uso continua no dono — o inventário guarda o padrão, não
/// a faixa válida.
pub fn numerica(nome: &str) -> f64 {
    let padrao = match declarada(nome).map(|flag| flag.tipo) {
        Some(TipoDaFlag::Numerica { padrao }) => padrao,
        Some(TipoDaFlag::Booleana { .. }) => {
            debug_assert!(false, "flag '{nome}' esta declarada como booleana");
            0.0
        }
        None => {
            debug_assert!(false, "flag '{nome}' nao esta no INVENTARIO");
            0.0
        }
    };
    std::env::var(nome)
        .ok()
        .and_then(|valor| valor.trim().parse::<f64>().ok())
        .unwrap_or(padrao)
}

/// A declaração da flag, se ela existir no inventário.
pub fn declarada(nome: &str) -> Option<&'static FlagExperimental> {
    INVENTARIO.iter().find(|flag| flag.nome == nome)
}

fn interpretar_booleana(valor: &str) -> Option<bool> {
    let valor = valor.trim();
    if valor.eq_ignore_ascii_case("1")
        || valor.eq_ignore_ascii_case("true")
        || valor.eq_ignore_ascii_case("on")
        || valor.eq_ignore_ascii_case("sim")
        || valor.eq_ignore_ascii_case("yes")
    {
        Some(true)
    } else if valor.eq_ignore_ascii_case("0")
        || valor.eq_ignore_ascii_case("false")
        || valor.eq_ignore_ascii_case("off")
        || valor.eq_ignore_ascii_case("nao")
        || valor.eq_ignore_ascii_case("não")
        || valor.eq_ignore_ascii_case("no")
    {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fontes que têm permissão de ler flag de regra de jogo do ambiente. Qualquer
    /// `env::var("IRACER_…")` aqui dentro precisa estar no [`INVENTARIO`].
    const FONTES_VIGIADAS: &[(&str, &str)] = &[
        (
            "market/pipeline/consolidacao.rs",
            include_str!("../market/pipeline/consolidacao.rs"),
        ),
        (
            "promotion/pipeline.rs",
            include_str!("../promotion/pipeline.rs"),
        ),
        (
            "promotion/effects.rs",
            include_str!("../promotion/effects.rs"),
        ),
    ];

    /// Nomes passados a `env::var("…")` no texto de um arquivo.
    fn envs_lidas(fonte: &str) -> Vec<String> {
        let mut nomes = Vec::new();
        for pedaco in fonte.split("env::var(\"").skip(1) {
            if let Some(fim) = pedaco.find('"') {
                nomes.push(pedaco[..fim].to_string());
            }
        }
        nomes
    }

    #[test]
    fn nenhuma_flag_de_regra_escapa_do_inventario() {
        for (arquivo, fonte) in FONTES_VIGIADAS {
            for nome in envs_lidas(fonte) {
                assert!(
                    declarada(&nome).is_some(),
                    "{arquivo} lê a env '{nome}' e ela não está no INVENTARIO de \
                     constants::flags_experimentais — regra de jogo decidida por ambiente \
                     precisa estar declarada num lugar só"
                );
            }
        }
    }

    #[test]
    fn o_inventario_nao_tem_nome_repetido() {
        for (i, flag) in INVENTARIO.iter().enumerate() {
            assert!(
                INVENTARIO[..i].iter().all(|outra| outra.nome != flag.nome),
                "flag '{}' declarada duas vezes",
                flag.nome
            );
            assert!(
                flag.nome.starts_with("IRACER_"),
                "flag '{}' foge do prefixo IRACER_",
                flag.nome
            );
        }
    }

    #[test]
    fn valor_irreconhecivel_cai_no_padrao_do_jogo() {
        assert_eq!(interpretar_booleana("1"), Some(true));
        assert_eq!(interpretar_booleana("TRUE"), Some(true));
        assert_eq!(interpretar_booleana("on"), Some(true));
        assert_eq!(interpretar_booleana("0"), Some(false));
        assert_eq!(interpretar_booleana("Off"), Some(false));
        assert_eq!(interpretar_booleana("talvez"), None);
        assert_eq!(interpretar_booleana(""), None);
    }

    /// Sem env definida, toda flag booleana lê exatamente o padrão declarado — é este o
    /// jogo que o jogador roda. O teste NÃO mexe em `std::env` (é global do processo e
    /// contaminaria as suítes paralelas): só cobra que a leitura sem env bate.
    #[test]
    fn sem_env_a_leitura_devolve_o_padrao_declarado() {
        for flag in INVENTARIO {
            if std::env::var(flag.nome).is_ok() {
                continue; // o harness pode estar rodando com a flag ligada
            }
            match flag.tipo {
                TipoDaFlag::Booleana { padrao } => assert_eq!(
                    booleana(flag.nome),
                    padrao,
                    "flag '{}' não devolve o padrão declarado",
                    flag.nome
                ),
                TipoDaFlag::Numerica { padrao } => assert!(
                    (numerica(flag.nome) - padrao).abs() < f64::EPSILON,
                    "flag '{}' não devolve o padrão declarado",
                    flag.nome
                ),
            }
        }
    }
}
