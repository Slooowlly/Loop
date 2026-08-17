//! Inventário das flags de EXPERIMENTO que mudam regra de jogo por variável de ambiente.
//!
//! Elas existem para o A/B do harness de Monte Carlo (`sim_stats`): ligar um mecanismo,
//! rodar N temporadas, comparar. O problema é que uma regra de jogo decidida pelo
//! ambiente é irreproduzível no relato de bug do jogador — duas máquinas com o mesmo
//! save podem simular temporadas diferentes, e nada na tela conta isso.
//!
//! Enquanto elas estiverem de pé, este módulo é o lugar ÚNICO onde estão declaradas:
//! nome, valor padrão do jogo, quem lê e o que muda. [`INVENTARIO`] é a lista, e o
//! teste no fim do arquivo **varre a árvore inteira do crate** cobrando que nenhuma env
//! de regra apareça em lugar nenhum sem passar por aqui.
//!
//! Antes o teste vigiava três arquivos escritos à mão, e por isso deu verde por anos com
//! `IRACER_GATE_SHARE`, `IRACER_SALARY_SHARE`, `IRACER_TRACK_RIVALRY` e `IRACER_QUALI_WRECK`
//! soltos fora do inventário — a vistoria V5.1 achou os três primeiros. A vigilância agora
//! é por descoberta: qualquer `.rs` sob `src-tauri/src`, e o que não é regra de jogo precisa
//! estar na allowlist `ENVS_DE_AMBIENTE` do módulo de teste, nomeado e justificado.
//!
//! **Cada entrada carrega um [`Destino`]**: se a decisão de produto já foi tomada
//! (virar padrão definitivo e sumir com a flag) ou se ela ainda serve para um A/B em
//! aberto. Fechar as `Indefinido` é decisão de produto/calibração, não de código.
//!
//! Declarar aqui **não** obriga o dono a ler pelos helpers [`booleana`]/[`numerica`]: quatro
//! entradas têm parser próprio (`LazyLock` com `filter`, ou `is_ok()` de presença) e foram
//! declaradas sem mexer no leitor. O que o inventário promete nesses casos é só o **padrão
//! sem env** — e o teste `o_padrao_declarado_bate_com_o_do_dono` cobra isso onde o número
//! está escrito duas vezes.

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

/// TODAS as flags de regra de jogo lidas do ambiente, em qualquer canto do crate.
///
/// Ordem: mercado primeiro, escada depois — a mesma ordem em que a virada de temporada
/// as executa —, e no fim as que não estão na virada (economia, importação, pista).
pub const INVENTARIO: &[FlagExperimental] = &[
    FlagExperimental {
        nome: "IRACER_CONTRATO_PLURIANUAL",
        tipo: TipoDaFlag::Booleana { padrao: true },
        dono: "market::team_ai",
        efeito: "contratacao nova da IA usa a duracao sorteada por tier; desligada, volta ao 1 ano cravado",
        destino: Destino::PadraoDefinitivo,
    },
    FlagExperimental {
        nome: "IRACER_FUNDO_OLHA_PARA_BAIXO",
        tipo: TipoDaFlag::Booleana { padrao: true },
        dono: "market::pipeline::consolidacao",
        efeito: "a metade de baixo da categoria tenta o campeao do feeder antes do pool, paga multa por contrato vivo e assina por 1 ano",
        destino: Destino::PadraoDefinitivo,
    },
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
        nome: "IRACER_MERCADO_EM_ONDAS",
        tipo: TipoDaFlag::Booleana { padrao: true },
        dono: "market::pipeline::consolidacao",
        efeito: "a equipe so contrata a partir da semana de liberacao (metade de cima na abertura, fundo dos tiers altos na semana 5, fundo da base na 7); desligada, toda equipe contrata desde a abertura",
        destino: Destino::Indefinido,
    },
    FlagExperimental {
        nome: "IRACER_FUNDO_ESTREIA_NOVATO",
        tipo: TipoDaFlag::Booleana { padrao: true },
        dono: "market::pipeline::consolidacao",
        efeito: "70% das equipes do fundo das categorias de estreia preferem um novato gerado a sobra do pool, por equipe-temporada; desligada, o fundo disputa o pool como qualquer equipe",
        destino: Destino::Indefinido,
    },
    FlagExperimental {
        nome: "IRACER_ROOKIE_NA_VITRINE",
        tipo: TipoDaFlag::Booleana { padrao: true },
        dono: "market::visibility",
        efeito: "a categoria rookie troca o teto 3.0 de visibilidade por escala 0.5: o fora da curva passa do limiar de shortlist (4.0); desligada, nenhum piloto do tier 0 entra em shortlist alguma",
        destino: Destino::Indefinido,
    },
    FlagExperimental {
        nome: "IRACER_PROPOSTA_PRIMEIRA_ESCOLHA",
        tipo: TipoDaFlag::Booleana { padrao: true },
        dono: "market::pipeline::jogador",
        efeito: "a proposta formal ao jogador só nasce quando ele é o MELHOR candidato ainda livre da vaga; desligada, volta a nascer com ele em qualquer lugar do top-3",
        destino: Destino::Indefinido,
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
    // ── Fora da virada de temporada ───────────────────────────────────────────────────
    // As quatro abaixo já existiam soltas quando o guard só olhava mercado e escada. Foram
    // declaradas em 11/08/2026 SEM tocar no valor nem no leitor: cada uma continua sendo
    // lida pelo seu próprio dono, do jeito que era. O destino é `Indefinido` porque nada
    // foi decidido sobre elas — declarar não é fechar experimento.
    FlagExperimental {
        nome: "IRACER_GATE_SHARE",
        tipo: TipoDaFlag::Numerica { padrao: 0.12 },
        dono: "finance::cashflow",
        // Lida uma vez por `gate_pot_coeff()` (LazyLock), que só aceita 0 < v < 1.
        efeito: "fração do custo operacional da rodada que vira o bolo de bilheteria",
        destino: Destino::Indefinido,
    },
    FlagExperimental {
        nome: "IRACER_SALARY_SHARE",
        tipo: TipoDaFlag::Numerica { padrao: 0.15 },
        dono: "finance::salary",
        // Lida uma vez por `team_salary_share_of_operating()` (LazyLock), 0 < v < 1.
        efeito: "fração do custo operacional que a folha de uma equipe (2 pilotos) representa",
        destino: Destino::Indefinido,
    },
    FlagExperimental {
        nome: "IRACER_TRACK_RIVALRY",
        tipo: TipoDaFlag::Booleana { padrao: false },
        dono: "commands::iracing::importacao",
        // Lida por PRESENÇA (`is_ok()`): qualquer valor liga, inclusive "0". O padrão
        // declarado descreve só o caso sem env, que é o que o jogador roda.
        efeito: "aplica no motor as rivalidades de pista que o SDK percebeu na corrida importada",
        destino: Destino::Indefinido,
    },
    FlagExperimental {
        nome: "IRACER_QUALI_WRECK",
        tipo: TipoDaFlag::Booleana { padrao: false },
        dono: "iracing_sdk::race_monitor",
        // Também por presença, e por cima do interruptor das Configurações — é o atalho
        // de teste da regra. Está aqui, e não na allowlist de ambiente, porque ligá-la
        // muda o que acontece na classificação.
        efeito: "arma a penalidade do carro destruído na classificação, vencendo a preferência do jogador",
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

    use std::path::{Path, PathBuf};

    /// Envs que a varredura acha e que NÃO são regra de jogo: ambiente, bancada e
    /// integração. Cada uma precisa de um motivo escrito — a allowlist é a única saída do
    /// inventário, e uma saída sem justificativa é o buraco que o guard antigo tinha.
    const ENVS_DE_AMBIENTE: &[(&str, &str)] = &[
        (
            "IRACER_MC_RUNS",
            "tamanho do harness de Monte Carlo (quantas carreiras); não muda regra, muda a amostra",
        ),
        (
            "IRACER_MC_SEASONS",
            "temporadas por run do mesmo harness",
        ),
        (
            "IRACER_OVERLAY_DEMO",
            "modo demo do overlay de rádio (retrocompat do botão das Configurações); é tela, não jogo",
        ),
        (
            "IRACER_OVERLAY_DISABLE",
            "`disable_environment` do manifesto OpenXR — convenção do loader para desligar a API layer",
        ),
        (
            "LOOP_BASE_DIR",
            "diretório de save que a bancada de fatura aponta; caminho, não regra",
        ),
        ("LOOP_BENCH_DB", "banco do bench de overlay"),
        ("LOOP_BENCH_YAML", "YAML de sessão do bench de overlay"),
        (
            "LOOP_RADIO_SAIDA",
            "onde a medição de rádio escreve o JSON, para o arreio de JS não depender do %TEMP%",
        ),
        (
            "LOOP_SEM_IA",
            "corta a chamada HTTP de enriquecimento por IA; a notícia determinística é a mesma",
        ),
        (
            "USERPROFILE",
            "perfil do Windows, para achar a pasta do iRacing e a do diagnóstico",
        ),
    ];

    fn e_de_ambiente(nome: &str) -> bool {
        ENVS_DE_AMBIENTE.iter().any(|(n, _)| *n == nome)
    }

    fn raiz_do_crate() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
    }

    /// Todo `.rs` sob `raiz`, em qualquer profundidade. É esta recursão que substitui a
    /// lista de três arquivos escrita à mão.
    fn arquivos_rs(raiz: &Path) -> Vec<PathBuf> {
        let mut achados = Vec::new();
        let Ok(itens) = std::fs::read_dir(raiz) else {
            return achados;
        };
        for item in itens.flatten() {
            let caminho = item.path();
            if caminho.is_dir() {
                achados.extend(arquivos_rs(&caminho));
            } else if caminho.extension().is_some_and(|e| e == "rs") {
                achados.push(caminho);
            }
        }
        achados.sort();
        achados
    }

    /// Um nome de env plausível? Serve para descartar o lixo que a varredura por texto
    /// pega em doc-comment (`env::var("IRACER_…")` do topo deste arquivo, por exemplo).
    fn nome_plausivel(nome: &str) -> bool {
        nome.len() >= 4
            && nome
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '(' || c == ')')
    }

    /// O nome entre aspas que começa em `inicio` (índice do caractere logo após a aspa).
    fn ate_a_aspa(fonte: &str, inicio: usize) -> Option<&str> {
        let resto = &fonte[inicio..];
        resto.find('"').map(|fim| &resto[..fim])
    }

    /// Nomes de env candidatos no texto de um arquivo, por duas redes:
    ///
    ///   1. **todo literal `"IRACER_*"`**, esteja ele num `env::var` ou numa `const` que
    ///      alguém lê depois. É esta rede que enxerga o `QUALI_WRECK_ENV` de
    ///      `race_monitor/constantes.rs`, invisível para qualquer busca por `env::var`;
    ///   2. **todo nome literal passado a `env::var`/`env::var_os`**, para que uma regra
    ///      nova batizada fora do prefixo `IRACER_` também caia no guard.
    fn envs_no_texto(fonte: &str) -> Vec<String> {
        let mut nomes = Vec::new();
        let mut empurrar = |nome: &str| {
            if nome_plausivel(nome) && !nomes.iter().any(|n: &String| n == nome) {
                nomes.push(nome.to_string());
            }
        };

        for (i, _) in fonte.match_indices("\"IRACER_") {
            if let Some(nome) = ate_a_aspa(fonte, i + 1) {
                // `"IRACER_"` cru é o prefixo do teste de nomenclatura, não uma flag.
                if nome.len() > "IRACER_".len() {
                    empurrar(nome);
                }
            }
        }
        for marcador in ["env::var(\"", "env::var_os(\""] {
            for (i, _) in fonte.match_indices(marcador) {
                if let Some(nome) = ate_a_aspa(fonte, i + marcador.len()) {
                    empurrar(nome);
                }
            }
        }
        nomes
    }

    struct Varredura {
        arquivos: usize,
        nomes: usize,
        /// (caminho relativo, nome) das que não estão nem no INVENTARIO nem na allowlist.
        fora_do_inventario: Vec<(String, String)>,
    }

    fn varrer(raiz: &Path) -> Varredura {
        let arquivos = arquivos_rs(raiz);
        let mut nomes = 0;
        let mut fora_do_inventario = Vec::new();
        for caminho in &arquivos {
            let Ok(fonte) = std::fs::read_to_string(caminho) else {
                continue;
            };
            let rel = caminho
                .strip_prefix(raiz)
                .unwrap_or(caminho)
                .to_string_lossy()
                .replace('\\', "/");
            for nome in envs_no_texto(&fonte) {
                nomes += 1;
                if declarada(&nome).is_none() && !e_de_ambiente(&nome) {
                    fora_do_inventario.push((rel.clone(), nome));
                }
            }
        }
        Varredura {
            arquivos: arquivos.len(),
            nomes,
            fora_do_inventario,
        }
    }

    #[test]
    fn nenhuma_flag_de_regra_escapa_do_inventario() {
        let varredura = varrer(&raiz_do_crate());
        // Um guard que não acha o que procura precisa gritar em vez de passar vazio.
        assert!(
            varredura.arquivos >= 400,
            "só {} arquivos .rs varridos — a descoberta furou",
            varredura.arquivos
        );
        assert!(
            varredura.nomes >= 20,
            "só {} nomes de env extraídos — a extração furou",
            varredura.nomes
        );

        let escapadas: Vec<String> = varredura
            .fora_do_inventario
            .iter()
            .map(|(arquivo, nome)| format!("  {arquivo}: {nome}"))
            .collect();
        assert!(
            escapadas.is_empty(),
            "env de regra fora do INVENTARIO de constants::flags_experimentais:\n{}\n\n\
             Regra de jogo decidida por ambiente precisa estar declarada num lugar só. \
             Se a variável for de ambiente/bancada e não de regra, some-a a \
             ENVS_DE_AMBIENTE com o motivo.",
            escapadas.join("\n")
        );
    }

    /// A allowlist é a saída do inventário: ela não pode conter nada que já esteja
    /// declarado (duas verdades sobre a mesma flag) nem apodrecer com nome que sumiu do
    /// crate — uma entrada morta ali é permissão em branco esperando um nome reciclado.
    #[test]
    fn a_allowlist_de_ambiente_nao_se_sobrepoe_nem_apodrece() {
        let mut vivas = std::collections::HashSet::new();
        for caminho in arquivos_rs(&raiz_do_crate()) {
            if let Ok(fonte) = std::fs::read_to_string(&caminho) {
                vivas.extend(envs_no_texto(&fonte));
            }
        }
        for (nome, motivo) in ENVS_DE_AMBIENTE {
            assert!(
                declarada(nome).is_none(),
                "'{nome}' está no INVENTARIO e na allowlist ao mesmo tempo"
            );
            assert!(
                !motivo.trim().is_empty(),
                "'{nome}' está na allowlist sem motivo escrito"
            );
            assert!(
                vivas.contains(*nome),
                "'{nome}' está na allowlist e não é mais lida em lugar nenhum — apague a entrada"
            );
        }
    }

    /// Os nomes das fixtures são montados por `concat!` porque **este arquivo também é
    /// varrido**: um literal `"IRACER_…"` inteiro aqui faria o guard acusar a própria
    /// fixture — e essa foi a primeira coisa que ele fez, o que é a melhor prova de que a
    /// varredura enxerga a árvore de verdade em vez de uma lista escrita à mão.
    const FLAG_FICTICIA: &str = concat!("IRACER", "_REGRA_NAO_DECLARADA");
    const FLAG_EM_CONST: &str = concat!("IRACER", "_ESCONDIDA");

    /// MUTAÇÃO: uma flag de regra nova, num arquivo em subpasta que a antiga lista fixa
    /// (`market/pipeline/consolidacao.rs`, `promotion/pipeline.rs`, `promotion/effects.rs`)
    /// jamais cobriria, precisa fazer o guard falhar.
    #[test]
    fn flag_nova_em_arquivo_novo_derruba_o_guard() {
        // O pid isola a fixture de outra sessão rodando `cargo test` ao mesmo tempo, e o
        // `remove_dir_all` de entrada limpa o resto de uma execução interrompida.
        let raiz = std::env::temp_dir().join(format!("loop_flags_guard_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let fundo = raiz.join("dominio").join("fundo").join("do").join("poco");
        std::fs::create_dir_all(&fundo).expect("criar a árvore de fixture");
        std::fs::write(
            fundo.join("regra_nova.rs"),
            format!("pub fn ligada() -> bool {{ std::env::var(\"{FLAG_FICTICIA}\").is_ok() }}\n"),
        )
        .expect("escrever a fixture");
        // Uma env de infra na mesma árvore não pode virar falso positivo.
        std::fs::write(
            raiz.join("infra.rs"),
            "fn perfil() -> Option<String> { std::env::var(\"USERPROFILE\").ok() }\n",
        )
        .expect("escrever a fixture de infra");

        let varredura = varrer(&raiz);
        let _ = std::fs::remove_dir_all(&raiz);

        assert_eq!(
            varredura.arquivos, 2,
            "a recursão não achou os dois arquivos"
        );
        assert_eq!(
            varredura.fora_do_inventario,
            vec![(
                "dominio/fundo/do/poco/regra_nova.rs".to_string(),
                FLAG_FICTICIA.to_string()
            )],
            "a flag nova no fundo da árvore tinha de ser a única acusada"
        );
    }

    /// A rede 1 existe porque a flag pode estar numa `const` e ser lida por variável —
    /// exatamente o caso do `IRACER_QUALI_WRECK`, que passou anos invisível.
    #[test]
    fn a_varredura_enxerga_flag_escondida_em_const() {
        let nomes = envs_no_texto(&format!(
            "pub(super) const ALGO_ENV: &str = \"{FLAG_EM_CONST}\";\n\
             fn ligada() -> bool {{ std::env::var(ALGO_ENV).is_ok() }}\n"
        ));
        assert_eq!(nomes, vec![FLAG_EM_CONST.to_string()]);

        // E uma regra batizada fora do prefixo também é vista, pela rede do `env::var`.
        let nomes = envs_no_texto("std::env::var_os(\"LOOP_REGRA_NOVA\")");
        assert_eq!(nomes, vec!["LOOP_REGRA_NOVA".to_string()]);

        // Já o prefixo cru do teste de nomenclatura não é flag nenhuma.
        assert!(envs_no_texto("flag.nome.starts_with(\"IRACER_\")").is_empty());
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

    /// As flags de parser próprio guardam o padrão em DOIS lugares: a `const` do dono e a
    /// declaração daqui. O teste acima só compara o inventário consigo mesmo — este é o
    /// que impede a dupla de divergir em silêncio quando alguém recalibra a economia.
    #[test]
    fn o_padrao_declarado_bate_com_o_do_dono() {
        // Com a env definida o `LazyLock` do dono já resolveu com ela; comparar não diria nada.
        if std::env::var("IRACER_GATE_SHARE").is_err() {
            let dono = crate::finance::cashflow::gate_pot_coeff();
            assert!(
                (dono - numerica("IRACER_GATE_SHARE")).abs() < f64::EPSILON,
                "finance::cashflow usa {dono} e o INVENTARIO declara {}",
                numerica("IRACER_GATE_SHARE")
            );
        }
        if std::env::var("IRACER_SALARY_SHARE").is_err() {
            let dono = crate::finance::salary::team_salary_share_of_operating();
            assert!(
                (dono - numerica("IRACER_SALARY_SHARE")).abs() < f64::EPSILON,
                "finance::salary usa {dono} e o INVENTARIO declara {}",
                numerica("IRACER_SALARY_SHARE")
            );
        }
    }
}
