//! **Guarda de consumo de knob** — parâmetros que o perfil calcula e a simulação nunca lê.
//!
//! Generalização do achado que `overtaking_difficulty_multiplier` produziu na varredura: ele
//! marcou alavanca 0,000 *exata* nas duas categorias, e a razão não era saturação — é que
//! ninguém o lê. Um knob assim é pior que inútil: ele aparece na configuração, é ajustado por
//! quem acha que está calibrando, e não faz nada. Foi exatamente esse tipo de coisa que produziu
//! "cinco corridas com o mesmo resultado".
//!
//! Esta guarda varre o código-fonte de `simulation/` procurando USOS de cada campo do
//! [`SimulationContext`](crate::simulation::context::SimulationContext) e compara com a
//! classificação declarada abaixo. Ela falha quando a realidade muda — nos dois sentidos:
//!
//! - um knob declarado NÃO CONSUMIDO passa a ser lido → alguém do pacote D/C conectou o fio, e a
//!   varredura de sensibilidade precisa passar a levá-lo a sério;
//! - um knob declarado CONSUMIDO deixa de ser lido → uma refatoração o desligou em silêncio.
//!
//! Ler o fonte como texto é grosseiro de propósito. O objetivo não é análise estática, é uma
//! cerca barata que dispara quando alguém mexe no fio — e que não custa nada manter.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Campos multiplicadores do contexto e o que se espera de cada um hoje.
///
/// `true` = a simulação lê. `false` = calculado e jogado fora.
pub const CLASSIFICACAO: &[(&str, bool)] = &[
    ("race_variance_multiplier", true),
    ("race_pace_spread_multiplier", true),
    ("start_chaos_multiplier", true),
    ("qualifying_variance_multiplier", true),
    ("pack_density_factor", true),
    ("incident_rate_multiplier", true),
    ("tire_degradation_rate", true),
    ("physical_degradation_rate", true),
    // RESSUSCITADO pelo pacote D. Era o órfão original — calculado pelo perfil, guardado no
    // contexto, nunca lido — e a varredura media alavanca 0,000 EXATA por isso. Agora é lido em
    // `race/motor.rs` e `race/trafego.rs`: o modelo de ultrapassagem finalmente o consome. Esta
    // guarda pegou a ligação do fio no momento em que ela aconteceu, sem ninguém avisar.
    ("overtaking_difficulty_multiplier", true),
    // RESSUSCITADO pelo pacote G. Achado desta guarda na PRIMEIRA execução, não previsto no
    // achado original: a cadeia era órfã inteira — o perfil calculava, `context.rs` tinha teste
    // asseverando que chuva o elevava acima de 1.0, e o único consumidor possível
    // (`math::adjusted_weather_multiplier`) não era chamado por lugar nenhum. Hoje é lido em
    // `qualifying.rs` e `race/pontuacao.rs`, e a modulação de chuva por pista e por perfil de
    // categoria passou a existir de fato.
    ("rain_sensitivity", true),
    // --- O último morto, e o mais insidioso ---
    //
    // `track_difficulty_multiplier` É LIDO por `race/pontuacao.rs` — esta guarda não tem o que
    // reclamar dele. Mas o efeito é `adaptabilidade/100 × (mult−1) × 0,05`: décimos de ponto num
    // score que vive na casa dos 60–70. Morto por MAGNITUDE, não por inexistência.
    //
    // É o caso que nenhuma guarda de fonte pega, porque o fio está conectado; quem o pega é a
    // varredura, medindo alavanca. Fica declarado `true` de propósito — mentir aqui para
    // "sinalizar" o problema estragaria a guarda que funciona.
    ("track_difficulty_multiplier", true),
    // --- O TERCEIRO órfão por inexistência, e o mais caro de todos ---
    //
    // `EscalasDeForma::peso_animo` foi adicionado ao struct, documentado com a razão certa ("rodar
    // com `peso_animo = 0` e comparar é o que separa as duas partes"), e **nunca ligado**: a
    // esteira chama `forma::proxima_forma_com_rho`, que usa a const `FORMA_PESO_ANIMO`, em vez de
    // `proxima_forma_com_escalas`, que recebe o parâmetro.
    //
    // Por que é o mais caro: os outros dois órfãos só desperdiçavam um knob. Este faz uma MEDIÇÃO
    // devolver o resultado errado sem parecer errada. Rodei a decomposição pareada com
    // `peso_animo` no padrão e em 0, e as quatro tabelas saíram idênticas até a primeira decimal —
    // o que se leria como "a contaminação do ânimo é zero" quando o certo é "o parâmetro não
    // chegou". Um knob órfão numa varredura dá alavanca 0,000 e denuncia a si mesmo; num
    // instrumento de medição pareada ele produz um zero que parece resposta.
    ("peso_animo", true),
    // --- As constantes de POSIÇÃO NA PISTA (A1.1) ---
    //
    // Não são campo do contexto no sentido antigo: moram em `ParametrosDeTrafego`, que o
    // contexto CARREGA. Entraram nesta classificação porque entraram na varredura, e a guarda
    // `a_varredura_de_knobs_esta_sincronizada_com_a_de_consumo` exige o par. A heurística de
    // `.<nome>` funciona igual — o acesso é `ctx.trafego.janela_ar_sujo_ms` em `race/motor.rs` e
    // `par.prob_base_ultrapassagem` em `race/trafego.rs`.
    ("janela_ar_sujo_ms", true),
    ("perda_maxima_ar_sujo_pontos", true),
    ("gap_minimo_entre_carros_ms", true),
    ("janela_de_ataque_ms", true),
    ("prob_base_ultrapassagem", true),
    ("delta_de_ritmo_que_satura", true),
    ("peso_da_habilidade_na_ultrapassagem", true),
    ("peso_da_agressividade_na_ultrapassagem", true),
    ("custo_tentativa_falha_atacante_ms", true),
    ("custo_tentativa_falha_defensor_ms", true),
];

/// Knobs que são LIDOS mas cuja alavanca medida é nula — morte por magnitude, o defeito que a
/// varredura pega e esta guarda não. Documentado aqui para que o catálogo dos mortos fique num só
/// lugar, agora que os dois órfãos por inexistência foram ressuscitados.
pub const MORTOS_POR_MAGNITUDE: &[&str] = &["track_difficulty_multiplier"];

/// Diretórios varridos. `profile/` e `context.rs` são EXCLUÍDOS: lá o campo é escrito, não lido,
/// e contá-los daria consumo falso para todo mundo. `calibracao/` também sai — nós mesmos
/// mencionamos todos os nomes aqui dentro.
fn arquivos_de_simulacao() -> Vec<PathBuf> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/simulation");
    let mut saida = Vec::new();
    coletar(&raiz, &mut saida);
    saida.retain(|p| {
        let s = p.to_string_lossy().replace('\\', "/");
        !s.contains("/simulation/profile")
            && !s.ends_with("/simulation/profile.rs")
            && !s.ends_with("/simulation/context.rs")
            && !s.contains("/simulation/calibracao")
            // ARQUIVOS DE TESTE NÃO CONTAM COMO CONSUMIDOR. Furo encontrado com o
            // `peso_animo`: ele é lido por `forma/tests.rs` (o teste de equivalência da própria
            // injetabilidade) e por mais ninguém, e a guarda o dava como vivo. Um parâmetro que só
            // o próprio teste consome é exatamente o caso que esta guarda existe para pegar — o
            // teste prova que a função sabe usá-lo, não que alguém a chama assim.
            && !s.contains("/tests/")
            && !s.ends_with("/tests.rs")
    });
    saida
}

fn coletar(dir: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.is_dir() {
            coletar(&caminho, saida);
        } else if caminho.extension().is_some_and(|e| e == "rs") {
            saida.push(caminho);
        }
    }
}

/// Um knob é considerado LIDO quando aparece como acesso de campo (`.<nome>`) em algum arquivo
/// de simulação fora dos excluídos. É uma heurística: pega `ctx.overtaking_difficulty_multiplier`
/// e não se confunde com a declaração do struct nem com a atribuição no `profile`.
pub fn consumo_real() -> BTreeMap<String, Vec<String>> {
    let mut mapa: BTreeMap<String, Vec<String>> = CLASSIFICACAO
        .iter()
        .map(|(nome, _)| ((*nome).to_string(), Vec::new()))
        .collect();

    for arquivo in arquivos_de_simulacao() {
        let Ok(conteudo) = std::fs::read_to_string(&arquivo) else {
            continue;
        };
        let curto = arquivo
            .to_string_lossy()
            .replace('\\', "/")
            .rsplit("/simulation/")
            .next()
            .unwrap_or("?")
            .to_string();

        for (nome, _) in CLASSIFICACAO {
            if conteudo.contains(&format!(".{nome}")) {
                mapa.get_mut(*nome).unwrap().push(curto.clone());
            }
        }
    }
    mapa
}

/// Divergências entre o declarado e o medido, já em texto pronto para mensagem de falha.
pub fn divergencias() -> Vec<String> {
    let real = consumo_real();
    let mut saida = Vec::new();

    for (nome, declarado_consumido) in CLASSIFICACAO {
        let sitios = real.get(*nome).map(|v| v.len()).unwrap_or(0);
        let de_fato_consumido = sitios > 0;

        if de_fato_consumido && !declarado_consumido {
            saida.push(format!(
                "`{nome}` passou a SER LIDO ({}). O fio foi conectado — mude a classificação para \
                 `true` em consumo.rs e reavalie a alavanca dele em varredura.rs, que até agora \
                 media 0,000 por inexistência.",
                real[*nome].join(", ")
            ));
        } else if !de_fato_consumido && *declarado_consumido {
            saida.push(format!(
                "`{nome}` DEIXOU de ser lido pela simulação. Ou uma refatoração o desligou em \
                 silêncio, ou ele mudou de nome — nos dois casos a calibração que dependia dele \
                 está medindo nada."
            ));
        }
    }
    saida
}

/// Relatório de onde cada knob é consumido. Útil ao calibrar: saber quem lê o quê é metade da
/// resposta sobre por que um ajuste não teve efeito.
pub fn relatorio_de_consumo() -> String {
    let real = consumo_real();
    let mut saida = String::from("\n### Onde cada knob do contexto é lido\n\n");
    for (nome, declarado) in CLASSIFICACAO {
        let sitios = &real[*nome];
        saida.push_str(&format!(
            "- `{nome}` — {} ({})\n",
            if sitios.is_empty() {
                "NUNCA LIDO".to_string()
            } else {
                sitios.join(", ")
            },
            if *declarado {
                "esperado"
            } else {
                "buraco conhecido"
            }
        ));
    }
    saida
}
