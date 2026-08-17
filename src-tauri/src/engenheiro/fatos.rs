//! O dossiê: o [`EstadoAgora`] virado em linhas de fato que um modelo consegue redigir.
//!
//! ## A regra que governa este arquivo: nada de valor sentinela
//!
//! O `EstadoAgora` usa `-1` e `NaN` para "não sei" porque é um tipo de dados. Um dossiê
//! não pode fazer o mesmo. Uma linha `Combustível: -1 L` chega ao modelo como um fato, e
//! ele vai redigir em cima dela — o piloto ouve "menos um litro" no meio de uma corrida,
//! ou pior, ouve uma frase confiante construída sobre lixo.
//!
//! Por isso toda função aqui devolve `Option` e toda linha desconhecida **some**. Um
//! dossiê curto é uma resposta curta; um dossiê com sentinela é uma mentira. O silêncio
//! sobre um dado é informação honesta: quem redige recebe a instrução de dizer que não
//! sabe quando o fato não está na lista.
//!
//! ## O núcleo vai em toda resposta
//!
//! Independente da pergunta, o dossiê carrega posição, volta, quanto falta e bandeira.
//! Não é desperdício de contexto — é o que um engenheiro de verdade sabe de cor a corrida
//! inteira. Sem isso, a resposta a "quanto de combustível?" sai sem saber se faltam duas
//! ou vinte voltas, que é justamente o que torna o número útil.
//!
//! ## Por que as linhas são em PT cru, e não `rust_i18n`
//!
//! Decisão registrada em 11/08/2026, depois de a vistoria apontar a diferença: em
//! `commands::ai_news::fatos` todo fato passa por `rust_i18n::t!`, e aqui não passa
//! nenhum. É deliberado, por três motivos que valem só para o rádio:
//!
//! 1. **Estas linhas não são texto de UI.** Ninguém as lê na tela. Elas são o INSUMO que
//!    sobe ao servidor em `ptt_responder`, junto com o `lang` do jogador — quem escolhe o
//!    idioma da fala é o prompt de lá, sobre fatos que ele só precisa copiar. Traduzir o
//!    insumo não muda o que o piloto ouve; muda apenas em que língua o modelo lê "P4".
//! 2. **O acervo gravado é monolíngue.** As peças `.opus` do caminho gravado (o que toca
//!    quando o catálogo cobre a pergunta) foram sintetizadas numa voz PT-BR única. Não
//!    existe acervo en-US, então o rádio já é PT por construção no caminho principal.
//! 3. **Aqui a forma do número É a fala.** `n1` escreve `1,2` com vírgula e `voltas`
//!    resolve o singular porque o modelo lê em voz alta o que estiver escrito. Essas
//!    regras são de português falado e não sobrevivem a uma tabela de tradução.
//!
//! Se um dia existir acervo de voz em outro idioma, a mudança começa pelo acervo, não por
//! aqui — e aí este bloco é o lugar de registrar a virada.

use crate::iracing_sdk::race_monitor::{EstadoAgora, Vizinho};
use crate::iracing_sdk::tire_strategy::Compound;

use super::Intencao;

/// Número com uma casa e vírgula decimal — o português escreve `1,2`, e o modelo lê em
/// voz alta o que estiver escrito.
fn n1(x: f64) -> String {
    format!("{x:.1}").replace('.', ",")
}

/// Singular ou plural de "volta". Um `{n} voltas` cru produz "uma voltas" no caso mais
/// urgente que este arquivo tem — o de faltar combustível para exatamente uma volta.
fn voltas(n: i32) -> &'static str {
    if n == 1 {
        "volta"
    } else {
        "voltas"
    }
}

/// Tempo de volta no formato de rádio: `1:32,4`. Décimos bastam — o milésimo não muda
/// nenhuma decisão dentro do carro e alonga a fala.
fn tempo_volta(s: f64) -> Option<String> {
    if !(s.is_finite() && s > 0.0) {
        return None;
    }
    let min = (s / 60.0).floor();
    let resto = s - min * 60.0;
    if min >= 1.0 {
        Some(format!("{min:.0}:{:04.1}", resto).replace('.', ","))
    } else {
        Some(format!("{resto:.1}").replace('.', ","))
    }
}

/// Uma linha de vizinho. `curto` é o modo do núcleo — nome e gap, nada mais; o modo longo
/// só aparece quando a pergunta É sobre ele, para não gastar a atenção do modelo com
/// ritmo e tendência de quem não foi perguntado.
fn linha_vizinho(v: &Vizinho, rotulo: &str, curto: bool) -> Option<String> {
    if v.gap_s < 0.0 {
        return None;
    }
    // O NÚMERO DO CARRO não entra no dossiê, e a ausência custou uma medição para ser
    // descoberta. Com a linha escrita como `Silva (#12), 0,8 s`, o Gemini respondeu "a
    // oitocentos metros de Silva, o décimo segundo": leu o `#12` como POSIÇÃO e o `0,8 s`
    // como DISTÂNCIA. Os dois erros somem quando a linha deixa de ser telegráfica.
    //
    // O número existe para a ponte com o `driver_id` da carreira, não para ser falado — quem
    // precisar dele tem o `Vizinho.numero` no `EstadoAgora`.
    let quem = if v.nome.is_empty() {
        format!("o carro {}", if v.numero > 0 { v.numero } else { v.idx })
    } else {
        v.nome.clone()
    };
    // O gap vai JÁ FALADO — "um e dois", "oito décimos" —, pela mesma função que nomeia as
    // peças gravadas. Não é estética: entregando `0,8 segundos`, um modelo respondeu "a
    // oitocentos metros" e outro "um e oito". Os dois soam verdadeiros ditos em voz alta, e
    // o piloto não tem como conferir a 200 por hora.
    //
    // Converter aqui também é o que faz o engenheiro falar o MESMO idioma nos dois caminhos:
    // a peça gravada e a fala gerada dizem o gap com as mesmas palavras, e a costura entre
    // elas deixa de aparecer.
    let mut partes = vec![match super::fala::gap_falado(v.gap_s) {
        Some(falado) => format!("{rotulo}: {quem}, a {falado} de você"),
        // Fora da faixa falável o número cru volta, com a unidade por extenso — é pior que a
        // forma falada e melhor que calar sobre um adversário que está ali.
        None => format!("{rotulo}: {quem}, a {} segundos de você", n1(v.gap_s)),
    }];

    if v.no_box {
        partes.push("no box agora".to_string());
    }
    // Tráfego a dobrar não é adversário, e confundir os dois é o erro que faz um
    // engenheiro mandar atacar quem está uma volta atrás.
    if v.volta_a_parte {
        partes.push("uma volta à parte".to_string());
    }

    if !curto {
        if let Some(t) = tempo_volta(v.ritmo_s) {
            let comparacao = if v.delta_ritmo_s.is_finite() {
                // O sinal de `delta_ritmo_s` é "meu ritmo menos o dele", então negativo
                // sou eu mais rápido. Escrever isso por extenso aqui evita que o modelo
                // tenha de interpretar sinal — que é exatamente o tipo de conta que ele
                // erra com confiança.
                if v.delta_ritmo_s < 0.0 {
                    format!(
                        " · você é {} segundos por volta mais rápido",
                        n1(-v.delta_ritmo_s)
                    )
                } else {
                    format!(
                        " · ele é {} segundos por volta mais rápido",
                        n1(v.delta_ritmo_s)
                    )
                }
            } else {
                String::new()
            };
            partes.push(format!("ritmo {t}{comparacao}"));
        }
        // Meio décimo por volta é o piso do que vale dizer. Abaixo disso a linha sairia
        // como "aproximando 0,0 s por volta" — uma frase que afirma movimento e mostra
        // parado, que é pior que não falar de tendência nenhuma.
        const TENDENCIA_MIN: f64 = 0.05;
        if v.tendencia_s_por_volta <= -TENDENCIA_MIN {
            partes.push(format!(
                "aproximando {} segundos por volta",
                n1(-v.tendencia_s_por_volta)
            ));
        } else if v.tendencia_s_por_volta >= TENDENCIA_MIN {
            partes.push(format!(
                "afastando {} segundos por volta",
                n1(v.tendencia_s_por_volta)
            ));
        }
        if v.voltas_para_alcancar > 0.0 && v.voltas_para_alcancar.is_finite() {
            partes.push(format!(
                "encosta em {:.0} voltas no ritmo atual",
                v.voltas_para_alcancar.ceil()
            ));
        }
    }
    Some(partes.join(" · "))
}

/// Composto e idade de um jogo de pneus, na forma que se fala no rádio.
///
/// O iRacing tem exatamente **dois** compostos, seco e chuva, então o índice cru vira nome
/// sem chute — é o que [`Compound::from_indice`] resolve. A idade vem da última parada com
/// troca; não há canal de desgaste na telemetria, e não haverá.
fn pneu_de(composto: i32, voltas: i32) -> Option<String> {
    let nome = match Compound::from_indice(composto) {
        Compound::Unknown => None,
        c => Some(c.label()),
    };
    match (nome, voltas >= 0) {
        (Some(n), true) => Some(format!("{n}, com {voltas} voltas de uso")),
        (Some(n), false) => Some(n.to_string()),
        (None, true) => Some(format!("com {voltas} voltas de uso")),
        (None, false) => None,
    }
}

/// O bloco de pneu: o seu, o dos vizinhos, a comparação de idade e o descasamento com a
/// pista — que é o fato que antecipa a parada alheia.
fn bloco_pneu(e: &EstadoAgora) -> Vec<String> {
    let mut linhas = Vec::new();

    match pneu_de(e.meu_composto, e.meu_pneu_voltas) {
        Some(meu) => linhas.push(format!("Seu pneu: {meu}")),
        None => {
            linhas.push("Pneu: o iRacing não informa o composto nesta categoria".to_string());
            return linhas;
        }
    }

    for (viz, rotulo) in [(&e.frente, "à frente"), (&e.atras, "atrás")] {
        let Some(v) = viz else { continue };
        let Some(dele) = pneu_de(v.composto, v.pneu_voltas) else {
            continue;
        };
        let quem = if v.nome.is_empty() {
            format!("O carro {rotulo}")
        } else {
            v.nome.clone()
        };
        linhas.push(format!("{quem}: {dele}"));

        // A COMPARAÇÃO é o que responde a pergunta de verdade. "Ele está com 23 voltas" não
        // diz nada sozinho; "15 voltas mais velho que o seu" diz se vale atacar agora.
        if v.pneu_voltas >= 0 && e.meu_pneu_voltas >= 0 {
            let diferenca = v.pneu_voltas - e.meu_pneu_voltas;
            if diferenca > 0 {
                linhas.push(format!(
                    "Vantagem sua: o pneu dele é {diferenca} voltas MAIS VELHO que o seu"
                ));
            } else if diferenca < 0 {
                linhas.push(format!(
                    "Vantagem dele: o pneu dele é {} voltas MAIS NOVO que o seu",
                    -diferenca
                ));
            } else {
                linhas.push("Pneus da mesma idade — nenhum dos dois leva vantagem".to_string());
            }
        }

        // Descasamento pneu↔pista. É o fato mais acionável do bloco: quem está de seco com
        // a pista molhada (ou o contrário) vai ter de parar, e saber disso ANTES de ele
        // parar é o que decide se o piloto ataca ou economiza e espera.
        let dele_composto = Compound::from_indice(v.composto);
        if e.pista_molhada && dele_composto == Compound::Dry {
            linhas.push(format!(
                "{quem} está de SECO com a pista molhada — vai precisar parar"
            ));
        } else if !e.pista_molhada && dele_composto == Compound::Wet {
            linhas.push(format!(
                "{quem} está de CHUVA com a pista seca — vai perder ritmo e parar"
            ));
        }
    }

    // O mesmo descasamento, mas do lado de cá — e este vem por último porque é o que o
    // piloto tem de fazer, não o que os outros vão fazer.
    let meu = Compound::from_indice(e.meu_composto);
    if e.pista_molhada && meu == Compound::Dry {
        linhas.push("VOCÊ está de seco com a pista molhada".to_string());
    } else if !e.pista_molhada && meu == Compound::Wet {
        linhas.push("VOCÊ está de chuva com a pista seca".to_string());
    }

    if e.compostos_no_grid.len() > 1 {
        let resumo: Vec<String> = e
            .compostos_no_grid
            .iter()
            .map(|(c, n)| match Compound::from_indice(*c) {
                Compound::Unknown => format!("{n} no índice {c}"),
                comp => format!("{n} de {}", comp.label()),
            })
            .collect();
        linhas.push(format!("Grid dividido: {}", resumo.join(", ")));
    }
    linhas
}

/// O que vale em toda resposta: onde estou, em que volta, quanto falta, e se há bandeira.
fn nucleo(e: &EstadoAgora) -> Vec<String> {
    let mut linhas = Vec::new();

    if !e.conectado {
        linhas.push("Situação: sem telemetria do iRacing agora.".to_string());
        return linhas;
    }
    // QUE SESSÃO É ESTA, sempre e como primeira linha do bloco.
    //
    // Antes daqui só saía uma negativa ("a sessão não está em corrida"), e ela nem chegava a
    // sair na classificatória porque `em_corrida` vinha do `session_state`, que é `Racing`
    // também no treino e na quali. O modelo recebia um punhado de fatos sem contexto de
    // sessão e escrevia o que era plausível: no fim de uma classificatória em que o jogador
    // não marcou tempo, "Novato, que corrida" (medido em 17/08/2026).
    //
    // A linha é afirmativa e vem sempre porque a negativa não ensina o que dizer. "Não é
    // corrida" deixa o modelo escolher entre treino e classificatória, e ele escolhe errado
    // metade das vezes; "estamos na classificatória" fecha a porta.
    match e.tipo_sessao {
        "corrida" => linhas.push("Sessão: CORRIDA.".to_string()),
        "classificacao" => linhas.push(
            "Sessão: CLASSIFICATÓRIA. Não é corrida: aqui se busca UMA volta rápida para \
             definir o grid, não há disputa de posição na pista e não existe resultado de \
             corrida para comentar."
                .to_string(),
        ),
        _ => linhas.push(
            "Sessão: TREINO LIVRE. Não é corrida nem classificatória: nada aqui vale \
             posição, grid ou pontos."
                .to_string(),
        ),
    }
    if e.em_formacao {
        linhas.push("Situação: volta de formação, a corrida ainda não largou.".to_string());
    }

    if e.posicao > 0 {
        let mut l = format!("Posição: {}º", e.posicao);
        if e.total_carros > 0 {
            l.push_str(&format!(" de {}", e.total_carros));
        }
        // A posição na classe só entra quando é OUTRO número. Em prova de classe única
        // ela repete a geral, e repetir um fato em outras palavras faz o modelo achar que
        // são dois fatos — e comentar os dois.
        if e.posicao_classe > 0 && e.posicao_classe != e.posicao {
            l.push_str(&format!(" ({}º na sua classe)", e.posicao_classe));
        }
        linhas.push(l);
    }

    if e.volta > 0 {
        linhas.push(if e.voltas_totais > 0 {
            format!("Volta: {} de {}", e.volta, e.voltas_totais)
        } else {
            format!("Volta: {}", e.volta)
        });
    }

    if e.voltas_restantes >= 0 {
        linhas.push(if e.voltas_restantes_estimadas {
            // A marca de estimativa viaja NA LINHA, não num campo à parte, porque é a
            // linha que chega ao modelo. Prova por tempo não tem número exato de voltas,
            // e um engenheiro que anuncia "faltam 12" numa prova por tempo está mentindo
            // com precisão falsa.
            format!(
                "Faltam: por volta de {} voltas (prova por TEMPO — o número é estimativa, diga-o como aproximação)",
                e.voltas_restantes
            )
        } else {
            format!("Faltam: {} voltas", e.voltas_restantes)
        });
    }

    if !e.bandeira.is_empty() {
        linhas.push(format!("Bandeira: {}", e.bandeira));
    }
    linhas
}

/// Todos os blocos de uma vez, sem repetir linha.
///
/// Existe por causa de uma restrição de arquitetura, não de gosto: a transcrição nasce no
/// **servidor** (é lá que está a chave do Scribe), e a classificação de intenção acontece
/// **aqui**. Escolher o bloco pela pergunta exige, portanto, uma segunda ida e volta —
/// áudio sobe, transcrição desce, dossiê sobe.
///
/// Quando esse ida-e-volta não se paga, este é o caminho: manda tudo numa tacada só. São
/// ~20 linhas, e o que importa continua valendo — **os fatos já vêm calculados**, que era o
/// ponto. O que se perde é foco: o modelo recebe informação que ninguém pediu e precisa da
/// instrução de responder só o que foi perguntado.
///
/// ## A ordem é prioridade, e isso foi aprendido medindo
///
/// Na primeira versão as linhas saíam na ordem em que os blocos eram montados, e o resultado
/// apareceu na medição do Gemini: num estado com **combustível para seis voltas e oito por
/// correr**, a resposta a "como estamos?" falava de posição e gap e **não mencionava
/// combustível**. O fato estava lá — na linha 11 de 19.
///
/// Um modelo lendo vinte linhas dá peso às primeiras, como qualquer leitor. Então o que é
/// urgente sobe: falta de combustível, bandeira preta, desclassificação, reparo obrigatório e
/// descasamento de pneu com a pista passam à frente do núcleo. É o que um engenheiro de
/// verdade faz — ele interrompe a resposta educada para dizer que o tanque não dá.
pub fn dossie_completo(e: &EstadoAgora) -> Vec<String> {
    const TODAS: [Intencao; 7] = [
        Intencao::Frente,
        Intencao::Atras,
        Intencao::Combustivel,
        Intencao::Ritmo,
        Intencao::Carro,
        Intencao::Bandeira,
        Intencao::Pista,
    ];
    let mut linhas = dossie(e, Intencao::Pneu);
    if !e.conectado {
        return linhas;
    }
    for intencao in TODAS {
        for l in dossie(e, intencao) {
            // O núcleo repete em todo bloco por construção; a deduplicação por conteúdo
            // resolve isso sem que os blocos precisem saber uns dos outros.
            if !linhas.contains(&l) {
                linhas.push(l);
            }
        }
    }

    // A igualdade exata não basta. O vizinho aparece em DUAS formas — a curta do núcleo e a
    // longa do bloco dele — e as duas sobreviviam, mostrando o mesmo carro duas vezes com
    // números diferentes de detalhe. Como a curta é prefixo literal da longa, descartar toda
    // linha que seja prefixo de outra resolve sem que nenhum bloco precise saber do outro.
    let completas: Vec<String> = linhas.clone();
    linhas.retain(|l| {
        !completas
            .iter()
            .any(|outra| outra != l && outra.starts_with(l.as_str()))
    });

    // O urgente sobe. Ver a nota de ordem na documentação desta função.
    const URGENTES: [&str; 5] = [
        "FALTA combustível",
        "BANDEIRA PRETA",
        "DESCLASSIFICADO",
        "Reparo OBRIGATÓRIO",
        "com a pista molhada",
    ];
    let urgente = |l: &String| URGENTES.iter().any(|m| l.contains(m));
    let (primeiro, resto): (Vec<String>, Vec<String>) = linhas.into_iter().partition(urgente);
    linhas = primeiro;
    linhas.extend(resto);
    linhas
}

/// O dossiê completo para uma pergunta: núcleo mais o bloco da intenção.
///
/// A ordem importa. O núcleo vem primeiro para o modelo se situar, e o bloco específico
/// por último — é o que ele lê por último e o que a resposta deve ser sobre.
pub fn dossie(e: &EstadoAgora, intencao: Intencao) -> Vec<String> {
    let mut linhas = nucleo(e);
    if !e.conectado {
        return linhas;
    }

    // Os vizinhos entram no núcleo em modo curto, EXCETO quando a pergunta é sobre eles —
    // aí o bloco abaixo os traz por inteiro, e repetir aqui seria dizer duas vezes.
    let sobre_vizinho = matches!(intencao, Intencao::Frente | Intencao::Atras);
    if !sobre_vizinho {
        if let Some(v) = e
            .frente
            .as_ref()
            .and_then(|v| linha_vizinho(v, "À frente", true))
        {
            linhas.push(v);
        }
        if let Some(v) = e
            .atras
            .as_ref()
            .and_then(|v| linha_vizinho(v, "Atrás", true))
        {
            linhas.push(v);
        }
    }

    match intencao {
        Intencao::Frente => {
            match e
                .frente
                .as_ref()
                .and_then(|v| linha_vizinho(v, "À frente", false))
            {
                Some(l) => linhas.push(l),
                None => linhas.push("À frente: ninguém — você lidera.".to_string()),
            }
        }
        Intencao::Atras => {
            match e
                .atras
                .as_ref()
                .and_then(|v| linha_vizinho(v, "Atrás", false))
            {
                Some(l) => linhas.push(l),
                // Vizinho ausente NÃO é o mesmo que ser o último. O carro de trás some do
                // `cars[]` quando está no box, na garagem ou fora do mundo, e afirmar "você é
                // o último" a quem está em quinto de vinte e quatro é uma mentira que o
                // piloto desmente olhando pelo retrovisor — e que derruba a confiança em tudo
                // o mais que o engenheiro disser depois.
                None if e.posicao > 0 && e.posicao == e.total_carros => {
                    linhas.push("Atrás: ninguém — você é o último na pista.".to_string());
                }
                None => linhas.push("Atrás: não estou vendo o carro de trás agora.".to_string()),
            }
        }
        Intencao::Combustivel => {
            if e.combustivel_l >= 0.0 {
                linhas.push(format!(
                    "Combustível: {} litros no tanque",
                    n1(e.combustivel_l)
                ));
            }
            if e.consumo_por_volta_l > 0.0 {
                linhas.push(format!(
                    "Consumo: {} litros por volta",
                    n1(e.consumo_por_volta_l)
                ));
            }
            if e.autonomia_voltas >= 0.0 {
                let n = e.autonomia_voltas.floor() as i32;
                linhas.push(format!("Autonomia: {n} {}", voltas(n)));
            }
            // O saldo é a única forma útil do dado. "Dá para 14 voltas" não responde nada
            // sem saber quantas faltam — é o cruzamento que manda o piloto ao box ou o
            // deixa correr em paz. Desconhecido aqui é `NaN`, e o `is_finite` o descarta.
            if e.saldo_combustivel_voltas.is_finite() {
                linhas.push(if e.saldo_combustivel_voltas >= 0.0 {
                    let n = e.saldo_combustivel_voltas.floor() as i32;
                    format!(
                        "Saldo: sobra para {n} {} além do fim — dá para terminar sem parar",
                        voltas(n)
                    )
                } else {
                    let n = (-e.saldo_combustivel_voltas).ceil() as i32;
                    format!(
                        "Saldo: FALTA combustível para {n} {} — vai precisar parar",
                        voltas(n)
                    )
                });
            }
        }
        Intencao::Ritmo => {
            if let Some(t) = tempo_volta(e.ultima_volta_s) {
                linhas.push(format!("Última volta: {t}"));
            }
            if let Some(t) = tempo_volta(e.melhor_volta_s) {
                linhas.push(format!("Melhor volta: {t}"));
            }
            if e.delta_melhor_s.is_finite() {
                linhas.push(if e.delta_melhor_s <= 0.0 {
                    "Comparação: esta foi a sua melhor volta".to_string()
                } else {
                    format!(
                        "Comparação: {} segundos acima da sua melhor",
                        n1(e.delta_melhor_s)
                    )
                });
            }
        }
        Intencao::Carro => {
            // Reparo obrigatório e opcional são coisas diferentes e a diferença muda a
            // decisão: o obrigatório o piloto vai pagar de qualquer jeito, o opcional é
            // escolha dele. Juntar os dois num "dano" só apagaria a escolha.
            if e.reparo_obrigatorio_s > 0.0 {
                linhas.push(format!(
                    "Reparo OBRIGATÓRIO pendente: {} s de box",
                    n1(e.reparo_obrigatorio_s)
                ));
            }
            if e.reparo_opcional_s > 0.0 {
                linhas.push(format!(
                    "Reparo opcional disponível: {} s de box",
                    n1(e.reparo_opcional_s)
                ));
            }
            if e.reparo_obrigatorio_s <= 0.0 && e.reparo_opcional_s <= 0.0 {
                linhas.push("Carro: sem reparo pendente".to_string());
            }
            if e.reparos_rapidos_usados > 0 {
                linhas.push(format!(
                    "Reparos rápidos já usados: {}",
                    e.reparos_rapidos_usados
                ));
            }
        }
        Intencao::Bandeira => {
            if e.bandeira.is_empty() {
                linhas.push("Bandeira: nenhuma bandeira ativa".to_string());
            }
            // A preta NÃO entra no dossiê: no Loop ela é o mecanismo de quebra de peça
            // (`car::breakdown`), e mandá-la ao modelo faria o engenheiro dizer que o
            // piloto foi punido quando o motor quebrou. Ver `engenheiro::fala`.
            if e.desclassificado {
                linhas.push("Atenção: você foi DESCLASSIFICADO".to_string());
            }
            linhas.push(format!("Incidentes: {} pontos na sessão", e.incidentes));
        }
        Intencao::Pista => {
            linhas.push(format!(
                "Pista: {}",
                if e.pista_molhada { "molhada" } else { "seca" }
            ));
            // Chuva caindo AGORA é outra coisa que pista molhada: é o que antecipa a
            // decisão em vez de descrevê-la depois de tomada.
            if e.chuva_agora > 0.0 {
                linhas.push("Chuva: está chovendo neste momento".to_string());
            }
            if e.declarada_molhada {
                linhas.push("Direção de prova declarou a corrida molhada".to_string());
            }
            if e.temp_pista_c > 0.0 {
                linhas.push(format!(
                    "Temperatura: pista {} °C, ar {} °C",
                    n1(e.temp_pista_c),
                    n1(e.temp_ar_c)
                ));
            }
            if !e.boxes_abertos {
                linhas.push("Boxes: FECHADOS agora".to_string());
            }
        }
        Intencao::Pneu => {
            linhas.extend(bloco_pneu(e));
        }
        // O núcleo já respondeu; não há bloco extra a acrescentar. `Campeonato` está aqui
        // porque o bloco dele não é de telemetria — ele vem do save, por
        // [`campeonato::linhas`](super::campeonato::linhas), e quem os junta é
        // [`super::responder`](super::responder).
        Intencao::Posicao | Intencao::Restante | Intencao::Geral | Intencao::Campeonato => {}
    }

    linhas
}
