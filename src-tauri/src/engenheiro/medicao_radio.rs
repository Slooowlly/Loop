//! LINHA DO TEMPO do rádio — a metade que só o Rust sabe produzir.
//!
//! As quatro famílias que abrem o canal sozinhas (spotter, quebra, peça própria, ritmo) foram
//! medidas uma a uma, cada uma na sua linha do tempo. Ninguém somou. E somar é a única pergunta
//! que importa, porque o jogador tem **um par de ouvidos**: duas famílias calmas separadas podem
//! ser um rádio insuportável juntas.
//!
//! Este arreio produz a parte que depende do modelo de desgaste — quebra na grade e peça do
//! nosso carro —, num JSON por VOLTA. Quem junta com o spotter (que vem de captura real do
//! iRacing) e roda a fila de voz é [`scripts/analise-radio.mjs`]; a fila é JS, e um espelho dela
//! em Rust mediria outra coisa.
//!
//! ```text
//! cargo test --lib -p iracer despeja_linha_do_tempo -- --ignored --nocapture
//! node scripts/analise-radio.mjs <captura.jsonl.gz>
//! ```
//!
//! A montagem das falas é a MESMA de `commands::overlay::radio` — inclusive a fusão de duas
//! quebras na mesma volta. Reescrevê-la aqui mediria um rádio que não existe.

use std::io::Write;

use crate::car::breakdown::{BreakdownDirector, LiveBreakdown, Severity, Weather};
use crate::car::Car;
use crate::engenheiro::{nomes, peca_propria as pp, quebra};

const CORRIDAS: u32 = 30;

/// Tamanho da grade e da prova. Vêm de env porque **é a grade que decide a taxa**: a quebra é
/// por carro por volta, então medir com 24 e correr com 12 é medir o dobro da conversa. Não há
/// padrão "certo" — há o tamanho do grid que o jogador exporta, e ele varia por categoria.
///
///   LOOP_RADIO_CARROS=12 LOOP_RADIO_VOLTAS=3 cargo test ...
fn env_u32(nome: &str, padrao: u32) -> u32 {
    std::env::var(nome)
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(padrao)
}
/// O carro do jogador. Entra na grade como qualquer outro — em produção a quebra DELE também cai
/// no `breakdown_log`, e omiti-la aqui esconderia uma fala que o jogador vai ouvir.
const EU: u32 = 1;

/// Onde o JSON sai. Sobrescrevível por env para o arreio de JS não depender do `%TEMP%` de quem
/// rodou — mas o padrão é o que ele procura sozinho.
fn saida() -> std::path::PathBuf {
    std::env::var_os("LOOP_RADIO_SAIDA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("loop_radio_timeline.json"))
}

/// O elenco sintético. Um sobrenome DO POOL gravado por carro (senão a fala sairia muda por um
/// motivo que não é o que se quer medir) e duas cadeiras por equipe.
struct Elenco {
    sobrenomes: Vec<String>,
    equipes: Vec<String>,
}

impl Elenco {
    fn novo(carros: u32) -> Elenco {
        let pool = nomes::sobrenomes();
        // Espalhados pelo pool em vez dos N primeiros: assim o comprimento dos nomes (que vira
        // duração de áudio) é uma amostra do acervo, não do começo do alfabeto.
        let passo = (pool.len() / carros as usize).max(1);
        let sobrenomes = (0..carros as usize)
            .map(|i| pool[(i * passo) % pool.len()].to_string())
            .collect();
        let equipes = nomes::EQUIPES_FALADAS
            .iter()
            .take((carros as usize / 2).max(1))
            .map(|(catalogo, _)| catalogo.to_string())
            .collect();
        Elenco {
            sobrenomes,
            equipes,
        }
    }

    fn nome(&self, carro: u32) -> String {
        // Nome completo, que é o que o banco devolve — `montar` extrai o sobrenome sozinho.
        format!("Piloto {}", self.sobrenomes[(carro - 1) as usize])
    }

    fn equipe(&self, carro: u32) -> String {
        self.equipes[((carro - 1) / 2) as usize % self.equipes.len()].clone()
    }

    fn assento(&self, carro: u32) -> u8 {
        if carro % 2 == 1 {
            1
        } else {
            2
        }
    }
}

/// Os vínculos com o jogador. Um de cada, que é o retrato de uma temporada em curso: nêmesis é
/// um só por definição, o companheiro é a outra cadeira, e rival é punhado.
///
/// Isto é SUPOSIÇÃO, e é a maior fonte de erro do número — vínculo decide o enquadramento, e o
/// enquadramento decide o comprimento da fala. Está aqui, num lugar só, para quem quiser apertar.
const NEMESIS: u32 = 7;
const RIVAIS: [u32; 2] = [9, 12];
const LIDER: u32 = 3;

/// Pontos do carro MENOS os do jogador. Metade da grade longe, uns poucos na vizinhança dos 15
/// pontos que ligam o enquadramento "está alguns pontos à sua frente".
fn delta_pontos(carro: u32) -> Option<i32> {
    let d = match carro {
        2 => 8,
        3 => 120,
        4 => -11,
        5 => 4,
        6 => -60,
        7 => -9,
        _ => (carro as i32 - 12) * 23,
    };
    Some(d)
}

fn contexto(
    e: &Elenco,
    carro: u32,
    peca: &str,
    severidade: &str,
    variante: usize,
    abandonos: u32,
) -> quebra::Contexto {
    quebra::Contexto {
        // O JOGADOR não sai de pool nenhum — o nome dele é dele. Sem sobrenome gravado, a fala
        // cai na forma pela equipe, que é exatamente o que produção faz hoje.
        nome_completo: if carro == EU {
            "Você Mesmo".to_string()
        } else {
            e.nome(carro)
        },
        equipe: Some(e.equipe(carro)),
        assento: e.assento(carro),
        e_nemesis: carro == NEMESIS,
        e_rival: RIVAIS.contains(&carro),
        // A outra cadeira da equipe do jogador.
        e_companheiro: carro == EU + 1,
        lidera_campeonato: carro == LIDER,
        delta_pontos: if carro == EU {
            None
        } else {
            delta_pontos(carro)
        },
        peca: peca.to_string(),
        severidade: severidade.to_string(),
        variante,
        abandonos_ate_aqui: abandonos,
    }
}

/// Uma corrida: a grade inteira volta a volta, com a fusão de simultâneas do rádio de verdade.
fn uma_corrida(corrida: u32, e: &Elenco, carros: u32, voltas: u32) -> serde_json::Value {
    let mut dir = BreakdownDirector::new();
    for c in 1..=carros {
        // A MESMA distribuição de entrada de `car::breakdown::medicao` — uniforme em [0,1) da
        // vida nominal. Trocá-la aqui daria dois números que não conversam.
        let mut car = Car::uniform(5);
        for (i, peca) in car.parts.iter_mut().enumerate() {
            peca.wear = f64::from((c * 37 + i as u32 * 61 + corrida * 7) % 100) / 100.0;
        }
        let semente = u64::from(corrida) * 1_000 + u64::from(c);
        dir.add_car(
            c,
            LiveBreakdown::new(&car, semente, 60.0, (0.4, 0.3, 0.3)).with_tent(true),
            Vec::new(),
        );
    }

    let mut quebras = Vec::new();
    let mut proprias = Vec::new();
    let mut abandonos = 0u32;
    let mut ordem = 0usize;
    let mut avisado = [false; 11];
    let mut poupar_avisado = false;

    for volta in 1..=voltas {
        let progresso = f64::from(volta) / f64::from(voltas);
        // O log da volta, em ordem de grade — a mesma ordem que o monitor produz e que a fusão
        // consome.
        let mut da_volta: Vec<(u32, String, String)> = Vec::new();
        for c in 1..=carros {
            for ev in dir.on_lap_at(c, volta, Weather::NEUTRAL, progresso) {
                if ev.severity == Severity::Dnf {
                    abandonos += 1;
                }
                da_volta.push((
                    c,
                    ev.part.as_str().to_string(),
                    ev.severity.key().to_string(),
                ));
            }
        }

        // Contextos e fusão, iguais a `get_breakdown_feed`.
        let ctxs: Vec<quebra::Contexto> = da_volta
            .iter()
            .map(|(c, peca, sev)| {
                let variante = (*c as usize).wrapping_add(ordem) % 3;
                ordem += 1;
                contexto(e, *c, peca, sev, variante, abandonos)
            })
            .collect();

        let mut i = 0usize;
        while i < ctxs.len() {
            let par = (i + 1 < ctxs.len())
                .then(|| quebra::montar_duplo(&ctxs[i], &ctxs[i + 1]))
                .flatten();
            match par {
                Some(fala) => {
                    quebras.push(serde_json::json!({
                        "volta": volta,
                        // O carro é o que espalha a fala DENTRO da volta. Duas quebras na mesma
                        // volta não acontecem no mesmo instante: cada carro cruza a linha na sua
                        // vez, e colapsá-las no mesmo tique inventaria uma rajada que a corrida
                        // não tem. Quem converte volta+carro em segundo é o arreio de JS, que é
                        // quem tem a captura.
                        "carro": da_volta[i].0,
                        "severidade": da_volta[i].2,
                        "fundida": true,
                        "pecas": fala.pecas,
                        "texto": fala.texto,
                    }));
                    i += 2;
                }
                None => {
                    let fala = quebra::montar(&ctxs[i]);
                    quebras.push(serde_json::json!({
                        "volta": volta,
                        // O carro é o que espalha a fala DENTRO da volta. Duas quebras na mesma
                        // volta não acontecem no mesmo instante: cada carro cruza a linha na sua
                        // vez, e colapsá-las no mesmo tique inventaria uma rajada que a corrida
                        // não tem. Quem converte volta+carro em segundo é o arreio de JS, que é
                        // quem tem a captura.
                        "carro": da_volta[i].0,
                        "severidade": da_volta[i].2,
                        "fundida": false,
                        "pecas": fala.pecas,
                        "texto": fala.texto,
                    }));
                    i += 1;
                }
            }
        }

        // O NOSSO carro. Mesma regra do `race_monitor`: avisa a peça uma vez ao entrar na zona de
        // risco, rearma quando ela sai, e o conselho de poupar é latch no cruzamento dos dois
        // sinais.
        let perigo = dir.car_parts_in_danger(EU);
        let mut agora = [false; 11];
        for (i, pt, _) in &perigo {
            agora[*i] = true;
            if !avisado[*i] {
                avisado[*i] = true;
                let variante = proprias.len();
                proprias.push(serde_json::json!({
                    "volta": volta,
                    "tipo": "peca",
                    "pecas": [pp::chave_aviso(pt.as_str(), variante)],
                    "texto": pp::aviso_frase(pt.as_str(), variante),
                }));
            }
        }
        for i in 0..11 {
            if !agora[i] {
                avisado[i] = false;
            }
        }
        if !poupar_avisado && !perigo.is_empty() && abandonos > quebra::ABANDONOS_PARA_COMENTAR {
            poupar_avisado = true;
            let (chave, texto) = pp::poupar_frase(proprias.len());
            proprias.push(serde_json::json!({
                "volta": volta,
                "tipo": "poupar",
                "pecas": [chave],
                "texto": texto,
            }));
        }
    }

    serde_json::json!({ "quebras": quebras, "proprias": proprias })
}

#[test]
#[ignore = "medição, não asserção — rode com --ignored --nocapture"]
fn despeja_linha_do_tempo() {
    let carros = env_u32("LOOP_RADIO_CARROS", 24);
    let voltas = env_u32("LOOP_RADIO_VOLTAS", 18);
    let e = Elenco::novo(carros);
    let corridas: Vec<serde_json::Value> = (0..CORRIDAS)
        .map(|c| uma_corrida(c, &e, carros, voltas))
        .collect();

    let total_q: usize = corridas
        .iter()
        .map(|c| c["quebras"].as_array().map_or(0, Vec::len))
        .sum();
    let total_p: usize = corridas
        .iter()
        .map(|c| c["proprias"].as_array().map_or(0, Vec::len))
        .sum();

    let doc = serde_json::json!({
        "voltas": voltas,
        "carros": carros,
        "corridas": corridas,
    });
    let caminho = saida();
    let mut f = std::fs::File::create(&caminho).expect("criar o JSON da linha do tempo");
    f.write_all(doc.to_string().as_bytes()).expect("escrever");

    let n = f64::from(CORRIDAS);
    println!(
        "\n── linha do tempo do rádio ({CORRIDAS} corridas × {carros} carros × {voltas} voltas) ──"
    );
    println!(
        "  quebras (já fundidas)  {:>6.2} falas por corrida",
        total_q as f64 / n
    );
    println!(
        "  peça própria + poupar  {:>6.2} falas por corrida",
        total_p as f64 / n
    );
    println!("  → {}", caminho.display());
}
