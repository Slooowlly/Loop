//! O RENDERIZADOR: a resposta montada de peças gravadas, sem passar pelo Gemini.
//!
//! A maioria das perguntas de rádio tem resposta de forma fixa e valor variável — "faltam
//! N voltas", "o carro da frente está a X". Mandar isso a um modelo é pagar dois serviços e
//! dois segundos para redigir uma frase que já se sabe de cor.
//!
//! Então o push-to-talk tem dois caminhos:
//!
//! ```text
//! áudio ─► Scribe ─► classificar ─┬─► renderizar OK  ─► toca peças gravadas   (~0,7 s, grátis)
//!                                 └─► renderizar None ─► Gemini + TTS          (~2,9 s)
//! ```
//!
//! ## O que decide o caminho não é a intenção, é se a fala RENDERIZA
//!
//! A intenção diz o assunto; quem manda é se **todas** as peças daquela resposta existem.
//! "Qual o gap?" com 1,2 s renderiza; com gap desconhecido, não. Por isso
//! [`renderizar`] devolve `Option` e é tudo-ou-nada: falhou **uma** peça, a resposta
//! inteira vai para o modelo. Meia fala gravada emendada com meia fala gerada seria o pior
//! dos dois mundos — a emenda apareceria, e a deriva de timbre junto.
//!
//! Isso importa mais aqui do que importava no dossiê. Enquanto o classificador só
//! escolhia o contexto do modelo, um erro dele dava uma resposta menos focada. Agora ele
//! dispara uma **fala pronta**, com a voz confiante do engenheiro, e um erro vira uma
//! resposta errada dita com convicção. Daí a regra ser estrita.
//!
//! ## Uma fonte para tocar e para gravar
//!
//! [`catalogo`] devolve TODA chave que este módulo pode emitir, com o texto exato a ser
//! gravado. O gerador de biblioteca lê daqui; o app toca daqui. São a mesma lista, e é o
//! [teste de cobertura](super::tests) que prova que são — ele varre um espaço grande de
//! estados, renderiza cada um, e falha se alguma chave emitida não estiver no catálogo.
//!
//! Sem esse teste, o modo de falha é silencioso e cruel: o renderizador diz "eu sei
//! responder", o app procura um `.wav` que nunca foi gerado, e o engenheiro simplesmente
//! não fala. Nenhum erro, nenhum log — só silêncio no meio de uma corrida.
//!
//! ## Fundir, não picar
//!
//! A POC de voz mediu três decomposições do mesmo tempo e a **fundida** ganhou por margem
//! larga na audição: `"um trinta e dois e quatro."` numa peça só soa melhor E sai mais
//! curta que a mesma frase picada em dígitos, porque a peça inteira já vem com a prosódia
//! certa e sobra uma emenda só para pagar.
//!
//! Aqui a regra vai até o fim: **toda** peça é uma frase completa. Até o gap, que tem dois
//! valores (de que lado e quanto), é gravado fundido — `O carro da frente está a um e
//! dois.` Isso dobra os arquivos daquela família e paga por si duas vezes: some a última
//! emenda do acervo, e o catálogo inteiro passa a obedecer a mesma regra de forma
//! (maiúscula no começo, ponto no fim), que é verificável por teste em vez de ser uma
//! coleção de exceções.
//!
//! O custo é o número de arquivos, e o número de arquivos continua barato. Medido em
//! 11/08/2026: o acervo tem **3.948 peças, ~110 mil caracteres e 373 MB** em disco — 11%
//! do milhão gratuito mensal da Cloud TTS, gerado uma vez. (O cabeçalho dizia 1.056 peças
//! e ~25 mil caracteres, número de quando o acervo nasceu; a família de tempo de volta
//! sozinha tem 2.101 peças hoje. Decisões de escala vinham sendo tomadas em cima do
//! número velho.)
//!
//! O que a medição nova muda: a conta da Cloud TTS segue folgada, então fundir continua
//! acessível. O que passou a doer é o DISCO e o clone do repositório — 373 MB de binário
//! versionado. Esse é o eixo a vigiar numa próxima leva, não o custo de síntese.
//!
//! ## A exceção: a família de QUEBRA é montada por peças
//!
//! [`quebra`](super::quebra) desobedece a regra de fundir, e por aritmética: a fala nomeia um
//! piloto do grid, e a frase inteira custaria 355 sobrenomes × 108 trechos × 8
//! enquadramentos. Por peça custa 576. É o único lugar do acervo onde o número de arquivos
//! deixaria de ser barato, e por isso o único onde vale pagar a emenda.
//!
//! Ela também não sai de [`renderizar`]: responde a um evento da corrida, não a uma pergunta
//! do jogador. Divide o catálogo porque divide a VOZ — mesma pessoa, mesma pasta, uma tomada
//! por frase.

use crate::iracing_sdk::race_monitor::EstadoAgora;
use crate::iracing_sdk::tire_strategy::Compound;

use super::Intencao;

/// Peças que o engenheiro TOCA mas não grava, porque já existem no pacote do spotter.
///
/// O spotter e o engenheiro são a mesma pessoa; o que os separa é o imediatismo — o spotter
/// alerta sem ser perguntado, o engenheiro responde. Quando os dois papéis precisam da mesma
/// frase, gravá-la duas vezes seria pedir duas TOMADAS distintas dela ao mesmo modelo não
/// determinístico, e o jogador ouviria a mesma pessoa dizendo a mesma coisa com timbres
/// sutilmente diferentes conforme quem falou.
///
/// Reaproveitar também ganha o rodízio de graça: o spotter tem `amarela`, `amarela_2` e
/// `amarela_3`, e o [`spotterVoice.js`](../../../src/lib/spotterVoice.js) alterna sozinho.
///
/// Estas chaves vivem em `src/assets/spotter/` e são geradas por `scripts/spotter-pack.mjs`.
/// O guard estrutural `scripts/tests/engenheiro-pecas-do-spotter.test.mjs` confere que elas
/// continuam existindo lá — sem ele, renomear uma fala do spotter emudeceria o engenheiro sem
/// erro nenhum aparecer.
pub const PECAS_DO_SPOTTER: [&str; 3] = ["ultima_volta", "amarela", "reparo"];

/// Peças que o RENDERIZADOR nunca emite, e que existem mesmo assim.
///
/// São faladas pela camada de cima — a que orquestra o push-to-talk —, não por este módulo.
/// A `nao_sei` é a desistência: entra quando o encanamento inteiro estoura o orçamento de
/// tempo ou o servidor não responde, situações que o renderizador não conhece porque elas
/// acontecem *depois* dele. A `nao_ouvi` é a irmã dela do outro lado do encanamento — a
/// pergunta que não chegou, e não a que não pôde ser respondida. As `espera*` são a frase que cobre o tempo do modelo, e só
/// existem no caminho em que ele foi chamado — que também é uma decisão de depois.
///
/// As `foco*` e a `radio_ruim` são o FREIO do rádio (`src/lib/pttFreio.js`): elas não
/// respondem a pergunta nenhuma, respondem à frequência delas. O renderizador nem tem
/// como conhecê-las — ele vê uma pergunta de cada vez, e o que as dispara é o histórico.
///
/// A lista existe para o teste de alcance poder distinguir "peça órfã" de "peça de outro
/// dono". Sem ela, a única saída seria afrouxar o teste — e um teste de alcance afrouxado
/// para de encontrar a coisa que ele existe para encontrar.
pub const PECAS_DE_OUTRA_CAMADA: [&str; 9] = [
    "nao_sei",
    "nao_ouvi",
    "espera",
    "espera_2",
    "espera_3",
    "foco",
    "foco_2",
    "foco_3",
    "radio_ruim",
];

/// Maior posição com fala gravada. Grid de iRacing raramente passa disso, e acima do teto
/// a pergunta cai no modelo em vez de inventar uma peça.
const MAX_POSICAO: i32 = 40;
/// Maior número de voltas com fala gravada (restantes, idade de pneu, autonomia).
const MAX_VOLTAS: i32 = 60;
/// Maior gap em segundos inteiros com fala gravada. Acima disso o adversário não é
/// adversário, e o número exato deixa de importar.
const MAX_GAP_S: i32 = 60;

// ─── Números por extenso ─────────────────────────────────────────────────────

/// Cardinal por extenso, 0–60. `None` fora da faixa — e o `None` é o ponto: ele sobe até o
/// [`renderizar`] e manda a pergunta para o modelo, em vez de produzir uma chave para um
/// arquivo que não existe.
pub(crate) fn cardinal(n: i32) -> Option<String> {
    const ATE_20: [&str; 21] = [
        "zero",
        "um",
        "dois",
        "três",
        "quatro",
        "cinco",
        "seis",
        "sete",
        "oito",
        "nove",
        "dez",
        "onze",
        "doze",
        "treze",
        "catorze",
        "quinze",
        "dezesseis",
        "dezessete",
        "dezoito",
        "dezenove",
        "vinte",
    ];
    const DEZENAS: [(i32, &str); 4] = [
        (20, "vinte"),
        (30, "trinta"),
        (40, "quarenta"),
        (50, "cinquenta"),
    ];
    match n {
        0..=20 => Some(ATE_20[n as usize].to_string()),
        21..=59 => {
            let dezena = n / 10 * 10;
            let unidade = n % 10;
            let nome = DEZENAS.iter().find(|(d, _)| *d == dezena)?.1;
            if unidade == 0 {
                Some(nome.to_string())
            } else {
                Some(format!("{nome} e {}", ATE_20[unidade as usize]))
            }
        }
        60 => Some("sessenta".to_string()),
        _ => None,
    }
}

/// Ordinal por extenso, 1–40. É como se anuncia posição no rádio.
///
/// `pub(crate)` porque a posição no CAMPEONATO se diz igual à posição na pista — mesma
/// pessoa, mesma boca. Ver [`super::campeonato`].
pub(crate) fn ordinal(n: i32) -> Option<String> {
    const ATE_10: [&str; 10] = [
        "primeiro", "segundo", "terceiro", "quarto", "quinto", "sexto", "sétimo", "oitavo", "nono",
        "décimo",
    ];
    const DEZENAS: [(i32, &str); 3] = [(10, "décimo"), (20, "vigésimo"), (30, "trigésimo")];
    match n {
        1..=10 => Some(ATE_10[(n - 1) as usize].to_string()),
        11..=39 => {
            let dezena = n / 10 * 10;
            let unidade = n % 10;
            let nome = DEZENAS.iter().find(|(d, _)| *d == dezena)?.1;
            if unidade == 0 {
                Some(nome.to_string())
            } else {
                Some(format!("{nome} {}", ATE_10[(unidade - 1) as usize]))
            }
        }
        40 => Some("quadragésimo".to_string()),
        _ => None,
    }
}

/// Cardinal no FEMININO, para contar voltas.
///
/// "Volta" é feminino e o cardinal padrão é masculino, então `{n} voltas` cru produz "um
/// volta" e "dois voltas" — e produzia, até a revisão do catálogo pegar. Num texto isso é um
/// bug de formatação; numa **gravação** é um erro permanente, que só some regerando o pacote.
///
/// A troca é por sufixo porque o problema é só das unidades 1 e 2, em qualquer dezena: "vinte
/// e um" vira "vinte e uma", "trinta e dois" vira "trinta e duas".
fn cardinal_f(n: i32) -> Option<String> {
    let masculino = cardinal(n)?;
    Some(if masculino.ends_with("dois") {
        format!("{}duas", &masculino[..masculino.len() - "dois".len()])
    } else if masculino.ends_with("um") {
        format!("{masculino}a")
    } else {
        masculino
    })
}

/// Plural de "volta" — porque cada frase é gravada inteira, e "Faltam uma voltas" seria
/// uma peça de áudio permanentemente errada, não um bug de formatação passageiro.
fn voltas(n: i32) -> String {
    if n == 1 {
        "volta".to_string()
    } else {
        "voltas".to_string()
    }
}

fn faltam(n: i32) -> &'static str {
    if n == 1 {
        "Falta"
    } else {
        "Faltam"
    }
}

// ─── Famílias de peças ───────────────────────────────────────────────────────
//
// Cada família tem DOIS lados que precisam concordar: uma função que devolve a chave de um
// valor (usada ao tocar) e uma que enumera todas as chaves com seus textos (usada ao
// gravar). Manter os dois no mesmo lugar é o que torna a divergência visível na revisão —
// e o teste de cobertura é o que a torna impossível de passar despercebida.

/// `Você está em quinto.` — uma peça por posição.
fn chave_posicao(n: i32) -> Option<String> {
    (1..=MAX_POSICAO).contains(&n).then(|| format!("pos_{n}"))
}

fn familia_posicao() -> Vec<(String, String)> {
    // Começa em DOIS. A primeira posição nunca chega aqui: quem lidera ouve "Você está na
    // liderança", que é o que se diz, e não "Você está em primeiro". O `pos_1` foi gravado na
    // primeira rodada e nunca teria tocado — o teste de alcance o encontrou.
    (2..=MAX_POSICAO)
        .filter_map(|n| {
            let o = ordinal(n)?;
            Some((format!("pos_{n}"), format!("Você está em {o}.")))
        })
        .collect()
}

/// `Faltam doze voltas.` e a forma aproximada da prova por tempo.
///
/// As duas formas são gravações SEPARADAS, e não um advérbio colado na frente. A POC
/// descobriu que a pontuação e a prosódia de uma peça são propriedade da frase inteira: um
/// "por volta de" gravado à parte entraria com entoação de começo de frase no meio de uma,
/// e a diferença entre um número exato e uma estimativa é exatamente o que essa entoação
/// carrega.
fn chave_restante(n: i32, estimado: bool) -> Option<String> {
    // Nas duas últimas voltas as duas formas COLAPSAM numa só, e é consequência de a versão
    // estimada ter perdido o hedge ali: "Última volta" e "Falta uma volta" são certezas, não
    // aproximações. Manter duas chaves para o mesmo texto geraria dois arquivos com a mesma
    // frase — e, como a Chirp 3 não é determinística, duas TOMADAS diferentes dela. O jogador
    // ouviria a mesma fala com timbres sutilmente distintos conforme o tipo de prova.
    if !(0..=MAX_VOLTAS).contains(&n) {
        return None;
    }
    // A última volta é do spotter — ele já a anuncia sem ninguém perguntar, e o engenheiro
    // perguntado responde com a mesma boca.
    if n == 0 {
        return Some("ultima_volta".to_string());
    }
    Some(if estimado && n >= 2 {
        format!("restam_aprox_{n}")
    } else {
        format!("restam_{n}")
    })
}

fn familia_restante() -> Vec<(String, String)> {
    let mut v = Vec::new();
    // Começa em 1: a última volta é `ultima_volta`, do pacote do spotter. Ver
    // [`PECAS_DO_SPOTTER`] — é a mesma pessoa dizendo a mesma frase, e gravá-la de novo daria
    // uma segunda tomada dela.
    for n in 1..=MAX_VOLTAS {
        let Some(c) = cardinal_f(n) else { continue };
        v.push((
            format!("restam_{n}"),
            format!("{} {c} {}.", faltam(n), voltas(n)),
        ));
        // A forma estimada usa "umas", e não "por volta de". Duas razões, as duas de leitura:
        // "por volta de doze voltas" repete a palavra e trava a frase, e "umas doze voltas" é
        // como se fala isso em português de verdade.
        //
        // E o hedge SOME quando resta uma volta só. "Falta por volta de uma volta" é absurdo
        // duas vezes — pela repetição e pelo sentido, porque na última volta não há o que
        // estimar: ou ela está acontecendo, ou a corrida acabou.
        // A forma estimada só existe de duas voltas para cima — abaixo disso ela colapsa na
        // exata (ver [`chave_restante`]), e gerar o arquivo assim mesmo criaria uma tomada
        // órfã: paga geração, ocupa disco e nunca toca.
        if n >= 2 {
            v.push((
                format!("restam_aprox_{n}"),
                format!("Faltam umas {c} voltas."),
            ));
        }
    }
    v
}

/// O gap, do jeito que se fala num rádio de corrida.
///
/// Três faixas, porque a FORMA da fala muda em cada uma e não só o número:
///
/// - **abaixo de 1 s** → `sete décimos` — ninguém diz "zero vírgula sete" dentro do carro
/// - **1 a 9,9 s** → `um e dois`, o desenho fundido que venceu a audição da POC
/// - **10 s ou mais** → `quinze segundos`, arredondado, porque o décimo já não decide nada
///
/// O arredondamento para o décimo acontece **uma vez, antes** de escolher a faixa. Decidir a
/// faixa primeiro foi o defeito da versão anterior: 9,97 caía na faixa do meio, arredondava
/// para "nove e dez" — que não é uma peça — e a resposta sumia num `None` inexplicável.
/// `pub(crate)` porque a família do vizinho NOMEADO precisa da mesma escolha de faixa —
/// ver [`super::vizinhanca`]. Duas derivações do mesmo gap divergiriam na borda de
/// arredondamento, e o sintoma seria a fala com nome sumir em valores em que a anônima sai.
pub(crate) fn sufixo_gap(s: f64) -> Option<String> {
    gap_partes(s).map(|(chave, _)| chave)
}

/// A GRADE de gaps faláveis: `(sufixo da chave, texto)`, do menor ao maior.
///
/// Existe como função, e não repetida em cada família, porque duas famílias a percorrem —
/// a anônima e a do vizinho nomeado. Com duas cópias, acrescentar uma resolução geraria
/// arquivo para uma e não para a outra, e a fala com nome ficaria muda naquele valor.
pub(crate) fn grade_de_gaps() -> Vec<(String, String)> {
    // Décimo a décimo até 1,9; de meio em meio até 9,5; de segundo em segundo daí para
    // cima. Enumerada em DÉCIMOS INTEIROS para não depender de aritmética de ponto
    // flutuante na geração dos nomes de arquivo, e passando pela mesma `gap_partes` que o
    // runtime usa — então a lista gravada é exatamente a lista tocável.
    (1..=19)
        .chain((20..=95).step_by(5))
        .chain((100..=MAX_GAP_S as i64 * 10).step_by(10))
        .filter_map(|decimos| gap_partes(decimos as f64 / 10.0))
        .collect()
}

/// O gap dito como um engenheiro de corrida diz: `um e dois`, `oito décimos`, `quinze
/// segundos`. `None` fora da faixa falável.
///
/// Serve aos DOIS caminhos, e é de propósito. O acervo gravado usa esta conversão para
/// nomear as peças; o [dossiê](super::fatos) usa a mesma para escrever o fato que vai ao
/// modelo. Com isso o modelo não precisa converter número nenhum — ele copia.
///
/// A necessidade veio de medição, não de gosto. Recebendo `a 0,8 segundos`, um modelo
/// respondeu "a oitocentos metros" e outro "um e oito"; os dois números soam perfeitamente
/// verdadeiros ditos em voz alta, e o piloto não tem como conferir dentro do carro. Também é
/// o que garante que o engenheiro fale o mesmo idioma nos dois caminhos — a costura entre a
/// peça gravada e a fala gerada é justamente onde a ilusão se quebra.
pub fn gap_falado(segundos: f64) -> Option<String> {
    gap_partes(segundos).map(|(_, texto)| texto)
}

/// Chave e texto do gap, sempre juntos.
///
/// As duas coisas saem da MESMA função porque elas têm de concordar sempre: a chave nomeia o
/// arquivo que vai ser gravado e o texto é o que se grava dentro dele. Calculá-las em lugares
/// diferentes é convidar o dia em que `gap_2_5` contém "três segundos".
///
/// ## A resolução acompanha a distância
///
/// Décimo de segundo num carro a três segundos é precisão que não muda decisão nenhuma — e
/// custa arquivo, e alonga a fala. Então a grade afrouxa conforme o gap cresce:
///
/// | faixa | passo | exemplo |
/// |---|---|---|
/// | abaixo de 1 s | décimo | `sete décimos` |
/// | 1 s a 2 s | décimo | `um e dois` |
/// | 2 s a 10 s | **meio segundo** | `dois segundos e meio` |
/// | 10 s ou mais | segundo | `quinze segundos` |
///
/// O décimo sobrevive só onde ele decide alguma coisa: abaixo de dois segundos é briga, e a
/// diferença entre sete e nove décimos é a diferença entre dar o bote nesta volta ou não.
fn gap_partes(s: f64) -> Option<(String, String)> {
    if !s.is_finite() || s < 0.0 {
        return None;
    }

    // Abaixo de dois segundos: décimo a décimo.
    if s < 1.95 {
        let d = (s * 10.0).round() as i64;
        return match d {
            // Zero décimo não é "zero décimos": é encostado, e pede outra frase, não um
            // número. O acervo tem `frente_colado` para isso.
            0 => None,
            1 => Some(("gap_0_1".to_string(), "um décimo".to_string())),
            2..=9 => Some((
                format!("gap_0_{d}"),
                format!("{} décimos", cardinal(d as i32)?),
            )),
            10 => Some(("gap_1_0".to_string(), "um segundo".to_string())),
            _ => Some((
                format!("gap_1_{}", d % 10),
                format!("um e {}", cardinal((d % 10) as i32)?),
            )),
        };
    }

    // De dois a dez segundos: meio em meio.
    if s < 9.75 {
        let meios = (s * 2.0).round() as i64;
        let inteiro = (meios / 2) as i32;
        let metade = meios % 2 == 1;
        return Some((
            format!("gap_{inteiro}_{}", if metade { 5 } else { 0 }),
            format!(
                "{} segundos{}",
                cardinal(inteiro)?,
                if metade { " e meio" } else { "" }
            ),
        ));
    }

    // Daí para cima o segundo inteiro basta — quem está a vinte segundos não é disputa.
    let inteiro = s.round() as i32;
    (10..=MAX_GAP_S).contains(&inteiro).then(|| {
        (
            format!("gap_{inteiro}"),
            format!("{} segundos", cardinal(inteiro).unwrap_or_default()),
        )
    })
}

/// O gap é a ÚNICA resposta com dois valores (quem e quanto), e mesmo assim é gravada
/// fundida — `O carro da frente está a um e dois.` numa peça só.
///
/// Fundir dobra o número de arquivos (dois leads × ~130 valores = ~260 em vez de 132) e
/// vale a pena mesmo assim, por duas razões que se somam. A primeira é a da POC: peça
/// inteira já vem com a prosódia certa, e a colagem paga uma emenda que a fundida não tem.
/// A segunda é estrutural: sem esta, o acervo inteiro passa a ser feito de **frases
/// completas**, todas começando em maiúscula e terminando em ponto — e essa uniformidade é
/// verificável por teste, enquanto "esta peça começa em minúscula porque é continuação"
/// seria uma exceção que o teste teria de conhecer caso a caso.
fn familia_gap() -> Vec<(String, String)> {
    let mut v = Vec::new();
    for (sufixo, texto) in grade_de_gaps() {
        v.push((
            format!("frente_{sufixo}"),
            format!("O carro da frente está a {texto}."),
        ));
        v.push((
            format!("atras_{sufixo}"),
            format!("O carro de trás está a {texto}."),
        ));
    }
    v
}

/// Idade de pneu e diferença de idade — as duas em voltas, as duas fundidas.
fn chave_idade_pneu(n: i32) -> Option<String> {
    (0..=MAX_VOLTAS)
        .contains(&n)
        .then(|| format!("pneu_idade_{n}"))
}

/// Acima desta diferença de idade o número vira ruído, pela mesma razão do déficit de
/// combustível: vinte voltas de vantagem de pneu já é "muito", e trinta não muda o que o
/// piloto faz com a informação.
const PNEU_DIF_MAX: i32 = 20;

fn chave_diferenca_pneu(voltas_dif: i32) -> Option<String> {
    let d = voltas_dif.abs();
    if d < 1 {
        return None;
    }
    let velho = voltas_dif > 0;
    Some(match (velho, d <= PNEU_DIF_MAX) {
        (true, true) => format!("pneu_dele_velho_{d}"),
        (false, true) => format!("pneu_dele_novo_{d}"),
        (true, false) => "pneu_dele_muito_velho".to_string(),
        (false, false) => "pneu_dele_muito_novo".to_string(),
    })
}

fn familia_pneu() -> Vec<(String, String)> {
    let mut v = vec![
        (
            "pneu_seco".to_string(),
            "Você está de pneu seco.".to_string(),
        ),
        (
            "pneu_chuva".to_string(),
            "Você está de pneu de chuva.".to_string(),
        ),
        (
            "pneu_dele_mesma_idade".to_string(),
            "O pneu dele tem a mesma idade do seu.".to_string(),
        ),
        (
            "pneu_frente_seco_pista_molhada".to_string(),
            "O carro da frente ainda está de seco com a pista molhada. Vai ter que parar."
                .to_string(),
        ),
        (
            "pneu_frente_chuva_pista_seca".to_string(),
            "O carro da frente está de chuva com a pista seca. Vai perder ritmo.".to_string(),
        ),
        (
            "pneu_voce_seco_pista_molhada".to_string(),
            "Você está de seco com a pista molhada.".to_string(),
        ),
        (
            "pneu_voce_chuva_pista_seca".to_string(),
            "Você está de chuva com a pista seca.".to_string(),
        ),
    ];
    for n in 0..=MAX_VOLTAS {
        let Some(c) = cardinal_f(n) else { continue };
        v.push((
            format!("pneu_idade_{n}"),
            if n == 0 {
                "Seu pneu é novo.".to_string()
            } else {
                format!("Seu pneu tem {c} {}.", voltas(n))
            },
        ));
    }
    for n in 1..=PNEU_DIF_MAX {
        let Some(c) = cardinal_f(n) else { continue };
        v.push((
            format!("pneu_dele_velho_{n}"),
            format!("O pneu dele é {c} {} mais velho que o seu.", voltas(n)),
        ));
        v.push((
            format!("pneu_dele_novo_{n}"),
            format!("O pneu dele é {c} {} mais novo que o seu.", voltas(n)),
        ));
    }
    v.push((
        "pneu_dele_muito_velho".to_string(),
        "O pneu dele está muito mais velho que o seu.".to_string(),
    ));
    v.push((
        "pneu_dele_muito_novo".to_string(),
        "O pneu dele está muito mais novo que o seu.".to_string(),
    ));
    v
}

/// Combustível. A falta leva número; a sobra não precisa de nenhum — "dá até o fim" é a
/// resposta completa, e um número ali só daria ao piloto uma conta para fazer no carro.
/// Acima deste déficit o número exato para de informar.
///
/// "Vai faltar combustível para sessenta voltas" é uma frase que ninguém diz e que não muda
/// decisão nenhuma: a partir de um certo buraco, a informação já não é *quanto* falta, é que
/// **não dá nem perto** — e a resposta do piloto é a mesma seja o déficit vinte e um ou
/// cinquenta. Abaixo do teto o número decide se ele estica o stint ou entra agora; acima, não
/// decide nada.
const COMB_FALTA_MAX: i32 = 20;

fn chave_combustivel_falta(n: i32) -> Option<String> {
    if n < 1 {
        return None;
    }
    Some(if n <= COMB_FALTA_MAX {
        format!("comb_falta_{n}")
    } else {
        "comb_falta_muito".to_string()
    })
}

fn familia_combustivel() -> Vec<(String, String)> {
    let mut v = vec![
        (
            "comb_sobra".to_string(),
            "O combustível dá até o fim.".to_string(),
        ),
        (
            "comb_no_limite".to_string(),
            "O combustível está no limite. Vai dar justo.".to_string(),
        ),
        (
            "comb_falta_muito".to_string(),
            "O combustível não dá nem perto. Você vai ter que parar.".to_string(),
        ),
    ];
    for n in 1..=COMB_FALTA_MAX {
        let Some(c) = cardinal_f(n) else { continue };
        v.push((
            format!("comb_falta_{n}"),
            format!("Vai faltar combustível para {c} {}.", voltas(n)),
        ));
    }
    v
}

/// Frases sem valor variável: bandeiras, leads e as negativas.
fn familia_estatica() -> Vec<(String, String)> {
    [
        ("lidera", "Você está na liderança."),
        ("ultimo", "Você é o último na pista."),
        ("frente_no_box", "O carro da frente entrou no box."),
        ("atras_no_box", "O carro de trás entrou no box."),
        ("frente_colado", "Ele está colado em você."),
        ("atras_colado", "Ele está colado atrás."),
        ("band_nenhuma", "Sem bandeira. Pista livre."),
        ("band_vermelha", "Bandeira vermelha. Corrida parada."),
        ("band_azul", "Bandeira azul. Deixa passar."),
        // SEM bandeira preta, de propósito — a mesma decisão que o spotter já tinha
        // tomado (ver `a_bandeira_preta_nao_fala` em `spotter_bandeira.rs`). No Loop a
        // preta é o mecanismo de dano do PRÓPRIO jogo (`!black #N <segundos>`, em
        // `car::breakdown`, simulando quebra de peça). Anunciá-la diria "você foi
        // punido" quando o que houve foi o motor quebrando.
        ("band_dq", "Você foi desclassificado."),
        ("band_bandeirada", "Bandeirada. Corrida encerrada."),
        ("pista_seca", "A pista está seca."),
        ("pista_molhada", "A pista está molhada."),
        ("pista_chovendo", "Está chovendo agora."),
        ("nao_sei", "Não consegui ver isso agora."),
        // NÃO OUVIU é outra coisa que não SABER. A `nao_sei` responde à pergunta que
        // chegou e não pôde ser respondida; esta responde à pergunta que não chegou —
        // botão apertado sem falar, ou a voz coberta pelo motor. Enquanto as duas eram a
        // mesma fala, o engenheiro dizia "não consegui ver isso" sobre uma pergunta que
        // ninguém fez, e o piloto ficava esperando a resposta de outra coisa.
        // Reticências, e não vírgula. MEDIDO nas quatro redações: a vírgula sai colada
        // ("nãoteouvirepete"), o ponto dá 0,29 s, as reticências 0,42 s e o travessão
        // 0,52 s. A Chirp 3 não pausa em vírgula — ela pausa em fim de oração, e qual
        // sinal se usa muda o tamanho da respirada.
        ("nao_ouvi", "Não te ouvi... repete."),
        ("sem_telemetria", "Estou sem dados do carro."),
        // A ESPERA. Toca quando o roteamento decidiu pelo modelo, e só então: o caminho
        // gravado responde antes dela terminar, e anunciá-la ali faria a resposta pronta
        // esperar a espera. São três redações pelo mesmo motivo que o lembrete do spotter
        // tem três — a mesma frase quatro perguntas seguidas deixa de ser um engenheiro
        // pensando e vira um sinal de ocupado.
        ("espera", "Deixa eu ver aqui."),
        ("espera_2", "Só um segundo."),
        ("espera_3", "Deixa eu conferir."),
        // O FREIO DO RÁDIO. Quem decide quando estas tocam é o app (`pttFreio.js`), não o
        // renderizador — elas não respondem a pergunta nenhuma, respondem à FREQUÊNCIA
        // delas. Vivem no catálogo mesmo assim porque o gerador de pacote é um só.
        //
        // Três redações pelo mesmo motivo da espera: o empurrão pode sair mais de uma vez
        // numa corrida, e repetido ao pé da letra soa como gravação, não como alguém
        // perdendo a paciência.
        ("foco", "Depois a gente conversa. Olha a pista."),
        // Vírgula, e não ponto: o Chirp 3 respira em todo ponto final, e nas duas peças
        // longas essa respirada veio com 0,3 s — buraco que o auditor do gerador reprova
        // e que, no rádio, soa como transmissão cortando. Ver `buracoInterno`.
        ("foco_2", "Foca na corrida que o que importar eu te aviso."),
        ("foco_3", "Menos rádio e mais volante."),
        // O corte. Uma redação só: ela sai uma vez e é seguida de silêncio de verdade,
        // então não há repetição para cansar.
        (
            "radio_ruim",
            "Estou com problema no rádio, não vou te responder agora.",
        ),
    ]
    .into_iter()
    .map(|(k, t)| (k.to_string(), t.to_string()))
    .collect()
}

/// TODA chave que [`renderizar`] pode emitir, com o texto exato a gravar.
///
/// É a entrada do gerador de biblioteca e a referência do teste de cobertura. Acrescentar
/// uma resposta gravada é acrescentar aqui e no renderizador — e o teste reclama se só um
/// dos dois lados for feito.
pub fn catalogo() -> Vec<(String, String)> {
    let mut v = familia_estatica();
    v.extend(familia_posicao());
    v.extend(familia_restante());
    v.extend(familia_gap());
    v.extend(familia_pneu());
    v.extend(familia_combustivel());
    // A família de QUEBRA não sai de `renderizar` — ela é montada por
    // [`quebra::montar`](crate::engenheiro::quebra::montar), que responde a um evento da
    // corrida em vez de a uma pergunta do jogador. O catálogo é comum porque o pacote de voz
    // é comum: mesma pessoa, mesma pasta, uma tomada por frase.
    v.extend(crate::engenheiro::quebra::familia_quebra());
    v.extend(crate::engenheiro::peca_propria::familia_peca_propria());
    v.extend(crate::engenheiro::tempo_volta::familia_tempo_volta());
    v.extend(crate::engenheiro::classificacao::familia_classificacao());
    // A família do CAMPEONATO também não sai daqui: ela precisa do save, e este módulo só
    // conhece telemetria. Quem a emite é `commands::engenheiro`, que tem o banco em mãos.
    v.extend(crate::engenheiro::campeonato::catalogo());
    // A do VIZINHO NOMEADO também não sai de `renderizar`: ela precisa do vínculo, que vem
    // do save. Ver `engenheiro::vizinhanca`.
    v.extend(crate::engenheiro::vizinhanca::familia_vizinho());
    // E a da MEMÓRIA da conversa, que não sai daqui pelo motivo mais simples de todos:
    // este módulo vê uma pergunta de cada vez. Ver `engenheiro::memoria`.
    v.extend(crate::engenheiro::memoria::familia_memoria());
    // O VOCATIVO. Também não sai de `renderizar`: quem decide se você ainda é novato é o
    // save, e de quantas em quantas respostas ele te chama é o contador da camada de cima.
    // Ver `engenheiro::tratamento`.
    v.extend(crate::engenheiro::tratamento::familia_tratamento());
    v
}

// ─── Renderização ────────────────────────────────────────────────────────────

/// Encadeia peças abortando na primeira que faltar. É o `?` do módulo: qualquer peça
/// ausente derruba a fala inteira para o caminho do modelo.
macro_rules! pecas {
    ($($p:expr),+ $(,)?) => {{
        let mut v: Vec<String> = Vec::new();
        $( v.push($p?); )+
        Some(v)
    }};
}

/// A resposta em peças gravadas, ou `None` quando o acervo não cobre este caso.
///
/// `None` **não é erro** — é o roteamento funcionando. Significa "esta pergunta merece o
/// modelo", e é o que acontece com `Ritmo` (que precisa da biblioteca de tempos de volta,
/// ~1.200 peças ainda não geradas), com `Carro`, e com toda pergunta aberta.
/// **A corrida está de fato acontecendo?**
///
/// Fora dela a maioria dos fatos ou não existe ou não significa o que parece — uma posição na
/// volta de formação não é uma posição, e um gap na volta de formação é a fila andando devagar.
/// Nesses instantes o acervo se cala e quem explica é o modelo.
///
/// Vive aqui, e é `pub`, porque não é só [`renderizar`] que precisa dela: quem compõe a resposta
/// do vizinho NOMEADO (`super::responder`) tenta uma fala gravada ANTES de chegar aqui, e sem
/// consultar o mesmo portão ele entregava, na volta de formação, um gap com o sobrenome do
/// carro da frente — a única fala do acervo que atravessava a formação inteira.
pub fn em_corrida_de_verdade(e: &EstadoAgora) -> bool {
    e.conectado && e.em_corrida && !e.em_formacao
}

pub fn renderizar(e: &EstadoAgora, intencao: Intencao) -> Option<Vec<String>> {
    if !e.conectado {
        return Some(vec!["sem_telemetria".to_string()]);
    }
    if !em_corrida_de_verdade(e) {
        return None;
    }

    match intencao {
        Intencao::Posicao => {
            if e.posicao == 1 {
                return Some(vec!["lidera".to_string()]);
            }
            if e.posicao > 0 && e.posicao == e.total_carros {
                return Some(vec!["ultimo".to_string()]);
            }
            pecas![chave_posicao(e.posicao)]
        }

        Intencao::Restante => {
            chave_restante(e.voltas_restantes, e.voltas_restantes_estimadas).map(|c| vec![c])
        }

        Intencao::Frente | Intencao::Atras => {
            let frente = intencao == Intencao::Frente;
            let viz = if frente { &e.frente } else { &e.atras };
            let Some(v) = viz else {
                // Sem vizinho não é falta de dado: é liderança ou último lugar, e as duas
                // têm fala própria.
                return Some(vec![if frente { "lidera" } else { "ultimo" }.to_string()]);
            };
            if v.no_box {
                return Some(vec![if frente {
                    "frente_no_box"
                } else {
                    "atras_no_box"
                }
                .to_string()]);
            }
            // Uma volta à parte é tráfego, não disputa. Anunciar o gap dele como se fosse
            // briga é o erro que faz o piloto atacar quem não está na sua corrida — e o
            // acervo não tem fala para essa distinção, então vai para o modelo.
            if v.volta_a_parte {
                return None;
            }
            let lado = if frente { "frente" } else { "atras" };
            match sufixo_gap(v.gap_s) {
                Some(sufixo) => Some(vec![format!("{lado}_{sufixo}")]),
                // Abaixo de um décimo não há número que se diga — "zero décimos" é uma
                // não-frase. Mas também não é caso para o modelo: colado é a informação mais
                // clara que existe naquele instante, e o acervo tem a fala pronta.
                //
                // Esta ligação faltava e o teste de alcance a encontrou: as duas peças foram
                // geradas, empacotadas, e nenhuma situação as produzia.
                None if (0.0..0.1).contains(&v.gap_s) => Some(vec![format!("{lado}_colado")]),
                None => None,
            }
        }

        Intencao::Combustivel => {
            let saldo = e.saldo_combustivel_voltas;
            if !saldo.is_finite() {
                return None;
            }
            if saldo >= 1.0 {
                return Some(vec!["comb_sobra".to_string()]);
            }
            if saldo >= 0.0 {
                return Some(vec!["comb_no_limite".to_string()]);
            }
            pecas![chave_combustivel_falta((-saldo).ceil() as i32)]
        }

        Intencao::Bandeira => {
            // Sem ramo para `e.bandeira_preta`: no Loop a preta é quebra de peça, não
            // punição. Ver a nota em `familia_estatica`.
            let chave = if e.desclassificado {
                "band_dq"
            } else if e.reparo_obrigatorio_s > 0.0 {
                "reparo"
            } else {
                // O rótulo já saiu de um decodificador único lá no monitor; aqui é só a
                // tradução para a chave da gravação.
                match e.bandeira.as_str() {
                    "" => "band_nenhuma",
                    "Bandeira amarela" => "amarela",
                    "Bandeira vermelha" => "band_vermelha",
                    "Bandeira azul" => "band_azul",
                    "Bandeirada" => "band_bandeirada",
                    // A BANDEIRA BRANCA. A peça é emprestada do spotter (ver
                    // [`PECAS_DO_SPOTTER`]) e já era emitida pela pergunta do RESTANTE, em
                    // `chave_restante`: a mesma frase, pela outra porta. Sem este braço a
                    // pergunta sobre bandeira caía no `_` e ia ao modelo — ~2,9 s e uma ida
                    // ao servidor para dizer o que o acervo responde em ~0,7 s, na última
                    // volta, que é o pior instante da corrida para o rádio ficar devendo.
                    "Última volta" => "ultima_volta",
                    "Reparo obrigatório" => "reparo",
                    // Bandeira que o acervo não conhece — inclusive uma que venha a ser
                    // acrescentada ao monitor sem passar por aqui. O modelo cobre.
                    _ => return None,
                }
            };
            Some(vec![chave.to_string()])
        }

        Intencao::Pista => Some(vec![if e.chuva_agora > 0.0 {
            "pista_chovendo"
        } else if e.pista_molhada {
            "pista_molhada"
        } else {
            "pista_seca"
        }
        .to_string()]),

        Intencao::Pneu => {
            let meu = Compound::from_indice(e.meu_composto);
            if meu == Compound::Unknown {
                return None;
            }
            let mut v = vec![if meu == Compound::Wet {
                "pneu_chuva"
            } else {
                "pneu_seco"
            }
            .to_string()];
            v.push(chave_idade_pneu(e.meu_pneu_voltas)?);

            // O descasamento do carro da frente é o fato mais acionável do assunto, e vem
            // depois do próprio pneu porque só faz sentido depois de saber o seu.
            if let Some(f) = e.frente.as_ref() {
                let dele = Compound::from_indice(f.composto);
                if e.pista_molhada && dele == Compound::Dry {
                    v.push("pneu_frente_seco_pista_molhada".to_string());
                } else if !e.pista_molhada && dele == Compound::Wet {
                    v.push("pneu_frente_chuva_pista_seca".to_string());
                } else if f.pneu_voltas >= 0 && e.meu_pneu_voltas >= 0 {
                    let dif = f.pneu_voltas - e.meu_pneu_voltas;
                    v.push(if dif == 0 {
                        "pneu_dele_mesma_idade".to_string()
                    } else {
                        chave_diferenca_pneu(dif)?
                    });
                }
            }
            if e.pista_molhada && meu == Compound::Dry {
                v.push("pneu_voce_seco_pista_molhada".to_string());
            } else if !e.pista_molhada && meu == Compound::Wet {
                v.push("pneu_voce_chuva_pista_seca".to_string());
            }
            Some(v)
        }

        // "Qual foi minha volta?" — a biblioteca de tempos existe agora, então esta pergunta
        // saiu do caminho do modelo (~2,9 s) para o gravado (~0,7 s).
        //
        // Duas frases, e a segunda só quando ela informa: o tempo, e o quanto falta para a
        // volta mais rápida da CORRIDA. Se a volta mais rápida já é nossa, não há o que
        // perseguir e o engenheiro cala a segunda — dizer "faltam zero décimos" seria pior que
        // não dizer nada.
        Intencao::Ritmo => {
            use crate::engenheiro::tempo_volta as tv;
            if e.ultima_volta_s <= 0.0 {
                return None; // sem volta válida ainda: o modelo explica melhor que um template
            }
            let mut v = pecas![
                Some(tv::LEAD_VOLTA.0.to_string()),
                tv::chave_tempo(tv::decimos_de(e.ultima_volta_s)),
            ]?;
            if !e.melhor_da_corrida_e_minha && e.melhor_da_corrida_s > 0.0 {
                let falta = tv::decimos_de(e.ultima_volta_s - e.melhor_da_corrida_s);
                if let Some((chave, _)) = tv::aproximacao_frase(falta) {
                    v.push(chave);
                }
            }
            Some(v)
        }

        // `Carro` mistura segundos de reparo com peças avariadas e não tem forma fixa.
        // `Geral` é a pergunta aberta, que é exatamente o que o modelo faz melhor que
        // qualquer template.
        //
        // `Campeonato` sai daqui por outro motivo: a resposta não está na telemetria, está
        // no save, e este módulo não conhece banco. Quem a monta é
        // [`super::responder`](super::responder), que tem as duas fontes em mãos.
        Intencao::Carro | Intencao::Geral | Intencao::Campeonato => None,
    }
}
