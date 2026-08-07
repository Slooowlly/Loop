//! A TABELA no rádio: o engenheiro sabendo quanto vale a posição.
//!
//! Todo o resto do que o engenheiro fala sai da telemetria — posição, gap, pneu, bandeira.
//! Isto sai do **save**, e é a única coisa que ele diz que um app de spotter de iRacing não
//! teria como dizer: ele leu a carreira.
//!
//! A diferença muda a fala mais do que parece. "Você está em quinto" é verdadeiro e não
//! pesa nada; "Você está em quinto, e no campeonato isso te deixa a doze pontos do
//! próximo" é a mesma informação virando um motivo para atacar ou para se conformar.
//!
//! ## Ele faz a conta
//!
//! "Terminando assim, você sobe para terceiro" exige casar cada carro do grid do iRacing
//! com um piloto do save e repontuar a temporada inteira. No Loop isso é possível porque o
//! grid SAI da carreira: o `iracing_numbers/<carreira>.json`, escrito no export do roster,
//! é o mapa número→piloto, e [`get_points_for_position`] é a mesma função que pontua a
//! corrida de verdade.
//!
//! O risco é o modo de falha: um carro do grid que não casa faz a projeção sair errada com
//! a voz confiante do engenheiro. Por isso a projeção é TUDO OU NADA e o corte está nas
//! posições que pontuam — se qualquer uma das dez primeiras não resolver para um piloto,
//! não há projeção nenhuma, e ele volta a dizer só a tabela como está. Ver [`projetar`].
//!
//! ## Quando ele fala disso
//!
//! Numa pergunta direta ("como estou no campeonato?"), sempre. Numa pergunta de posição ou
//! na pergunta aberta, só a posição na tabela — e a margem apenas quando ela está ao
//! alcance de uma vitória. Um engenheiro não recita a classificação a cada volta; ele
//! menciona a diferença quando ela ainda é uma diferença.

use super::fala::{cardinal, ordinal};

/// Maior posição de campeonato com fala gravada. O mesmo teto do acervo de posição de
/// pista, pelo mesmo motivo: acima disso a pergunta cai no modelo em vez de inventar uma
/// chave para um arquivo que não existe.
const MAX_POSICAO: i32 = 40;

/// Maior diferença de pontos que se diz por extenso.
///
/// Sessenta é pouco mais que duas vitórias. Acima disso a diferença deixou de ser uma
/// diferença e virou um fato da temporada — o número exato não muda nada do que o piloto
/// faz nesta corrida, e o engenheiro simplesmente não o menciona.
const MAX_PONTOS: i32 = 60;

/// Diferença que ainda cabe numa corrida. Vitória vale 25 (ver `constants::scoring`), e é
/// esse o limiar de "dá para tirar hoje" — abaixo dele a margem entra na resposta de
/// posição sem ser pedida; acima, só se a pergunta for do campeonato.
const AO_ALCANCE: f64 = 25.0;

/// Onde o jogador está na tabela, e por quanto.
///
/// Vem do save (ver `commands::engenheiro`), não da telemetria. `None` em qualquer campo é
/// "não sei", e "não sei" faz a fala sumir em vez de virar zero.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contexto {
    /// 1 = líder. `0` quando o jogador não pontuou ou a temporada não começou.
    pub posicao: i32,
    /// Diferença para quem está imediatamente acima. `None` quando ele lidera.
    pub para_o_proximo: Option<f64>,
    /// Folga para quem está imediatamente abaixo. `None` quando ele é o último da tabela.
    pub folga: Option<f64>,
    /// Onde ele TERMINARIA a temporada se a corrida acabasse agora. `None` quando não dá
    /// para garantir a conta — ver [`projetar`].
    pub projecao: Option<i32>,
}

impl Contexto {
    /// Há tabela o bastante para falar? Posição zero é temporada sem pontos — e um
    /// engenheiro que anuncia "você está em zero no campeonato" na primeira corrida do ano
    /// está dizendo uma coisa sem sentido com a maior convicção.
    pub fn conhecido(&self) -> bool {
        self.posicao >= 1
    }
}

/// Quantas posições pontuam. Fora delas o resultado da corrida não move a tabela, e é isso
/// que torna a projeção verificável: basta resolver estas.
const POSICOES_QUE_PONTUAM: i32 = 10;

/// **A conta.** Onde o jogador terminaria a temporada se a corrida acabasse agora.
///
/// `pontos` é a tabela como está (piloto → pontos). `ordem` é a corrida agora, em pares
/// `(posição, número do carro)`. `por_numero` é o mapa do `iracing_numbers/<carreira>.json`,
/// escrito no export do roster. `minha_posicao` vem da telemetria.
///
/// ## Por que é tudo ou nada
///
/// Devolve `None` — e o rádio volta a dizer só a tabela como está — se **qualquer** posição
/// pontuadora não resolver para um piloto da carreira. Não é preciosismo: um carro do grid
/// que não casa é um rival ganhando pontos que a conta não viu, e o resultado seria uma
/// projeção otimista dita com a mesma convicção de um fato. Entre não falar e falar errado,
/// um engenheiro não fala.
///
/// A posição do JOGADOR resolve sozinha, sem passar pelo mapa: ele não está no roster de IA
/// exportado, e a telemetria já diz em que lugar ele está.
pub fn projetar(
    pontos: &[(String, f64)],
    ordem: &[(i32, i32)],
    por_numero: &std::collections::HashMap<i64, String>,
    jogador_id: &str,
    minha_posicao: i32,
    endurance: bool,
) -> Option<i32> {
    use crate::constants::scoring::get_points_for_position;
    use std::collections::HashMap;

    if minha_posicao < 1 || ordem.is_empty() {
        return None;
    }

    let mut projetado: HashMap<&str, f64> =
        pontos.iter().map(|(id, p)| (id.as_str(), *p)).collect();
    // Quem ainda não pontuou na temporada não está na tabela e precisa entrar com zero,
    // senão um estreante subindo ao pódio some da projeção — e a posição do jogador sai
    // melhor do que vai ser.
    projetado.entry(jogador_id).or_insert(0.0);

    for (posicao, numero) in ordem {
        if *posicao > POSICOES_QUE_PONTUAM {
            continue;
        }
        let id = if *posicao == minha_posicao {
            jogador_id
        } else {
            por_numero.get(&i64::from(*numero)).map(String::as_str)?
        };
        let ganho = f64::from(get_points_for_position(*posicao as u8, endurance));
        *projetado.entry(id).or_insert(0.0) += ganho;
    }

    let mut tabela: Vec<(&str, f64)> = projetado.into_iter().collect();
    // Mesmo desempate do carregador: pontos desc, id asc. Empatado com o jogador, a ordem
    // entre os dois é arbitrária — e continua sendo, aqui e lá, do mesmo jeito.
    tabela.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    tabela
        .iter()
        .position(|(id, _)| *id == jogador_id)
        .map(|i| i as i32 + 1)
}

/// A chave da projeção, comparada com onde ele está hoje.
///
/// `None` quando não há projeção ou quando a posição está fora do teto do acervo.
/// Só existe frase para a projeção que MOVE a tabela.
///
/// Terminar onde já se está não é notícia, e a alternativa àquela segunda frase — a
/// diferença de pontos — diz mais. Chegou a haver uma família `camp_proj_mantem_*` no
/// catálogo; ela foi gerada, empacotada, e o teste de alcance mostrou que nada a tocava.
/// Peça que ninguém toca não é peça, é peso.
fn chave_projecao(atual: i32, projetada: i32) -> Option<String> {
    use std::cmp::Ordering;
    match projetada.cmp(&atual) {
        Ordering::Equal => None,
        // Assumir a liderança é a única troca que o automobilismo nomeia, e "sobe para
        // primeiro" seria a forma burocrática de dizê-la.
        Ordering::Less if projetada == 1 => Some("camp_proj_lidera".to_string()),
        Ordering::Less if projetada <= MAX_POSICAO => Some(format!("camp_proj_sobe_{projetada}")),
        Ordering::Greater if projetada <= MAX_POSICAO => {
            Some(format!("camp_proj_cai_{projetada}"))
        }
        _ => None,
    }
}

/// Arredonda para o inteiro mais próximo, para virar chave.
///
/// A pontuação é `f64` porque o banco guarda assim (há categorias com meio ponto), mas o
/// rádio fala número inteiro: "doze pontos e meio para o próximo" é precisão que ninguém
/// pediu no meio de uma curva.
fn inteiro(p: f64) -> Option<i32> {
    if !p.is_finite() || p < 0.0 {
        return None;
    }
    let n = p.round() as i32;
    (1..=MAX_PONTOS).contains(&n).then_some(n)
}

/// A chave da posição na tabela.
fn chave_posicao(n: i32) -> Option<String> {
    if n == 1 {
        return Some("camp_lidera".to_string());
    }
    (2..=MAX_POSICAO)
        .contains(&n)
        .then(|| format!("camp_pos_{n}"))
}

/// **A resposta à pergunta direta sobre o campeonato.**
///
/// Posição mais a margem, quando a margem couber no acervo. Devolve `None` quando não há
/// tabela ou quando a posição está fora do teto — e aí a pergunta vai ao modelo, que tem as
/// mesmas linhas em [`linhas`].
pub fn pecas(c: &Contexto) -> Option<Vec<String>> {
    if !c.conhecido() {
        return None;
    }
    let mut v = vec![chave_posicao(c.posicao)?];

    // Duas frases, e a segunda é a mais informativa das duas candidatas. Quando o resultado
    // de hoje MOVE a tabela, é isso que importa — a diferença de pontos vira detalhe de uma
    // situação que está mudando enquanto se fala dela. Quando não move, a diferença volta a
    // ser o número que responde "estou perto?".
    if let Some(chave) = c.projecao.and_then(|p| chave_projecao(c.posicao, p)) {
        v.push(chave);
        return Some(v);
    }
    // Quem lidera fala da FOLGA; quem não lidera, da diferença para cima. É a assimetria
    // do próprio esporte: o líder defende, o resto persegue.
    if c.posicao == 1 {
        if let Some(n) = c.folga.and_then(inteiro) {
            v.push(format!("camp_folga_{n}"));
        }
    } else if let Some(n) = c.para_o_proximo.and_then(inteiro) {
        v.push(format!("camp_para_{n}"));
    }
    Some(v)
}

/// **O apêndice** — o que o campeonato acrescenta a uma resposta que era sobre outra coisa.
///
/// Mais contido que [`pecas`]: a posição na tabela sempre, a margem só quando ela ainda
/// cabe numa corrida. Quem perguntou a posição na pista não pediu a classificação inteira,
/// e um engenheiro que a recita toda vez vira um locutor.
pub fn apendice(c: &Contexto) -> Vec<String> {
    if !c.conhecido() {
        return Vec::new();
    }
    let Some(posicao) = chave_posicao(c.posicao) else {
        return Vec::new();
    };
    let mut v = vec![posicao];

    // A projeção QUE MUDA a tabela entra sem ser pedida — é a única notícia do campeonato
    // que vale interromper uma resposta sobre outra coisa. A que não muda nada não entra:
    // "terminando assim você segue em terceiro" é informação de quem perguntou, e aqui
    // ninguém perguntou.
    if let Some(chave) = c.projecao.and_then(|p| chave_projecao(c.posicao, p)) {
        v.push(chave);
        return v;
    }

    let margem = if c.posicao == 1 { c.folga } else { c.para_o_proximo };
    if let Some(p) = margem {
        if p <= AO_ALCANCE {
            if let Some(n) = inteiro(p) {
                v.push(if c.posicao == 1 {
                    format!("camp_folga_{n}")
                } else {
                    format!("camp_para_{n}")
                });
            }
        }
    }
    v
}

/// As mesmas informações em prosa, para o dossiê do modelo.
pub fn linhas(c: &Contexto) -> Vec<String> {
    if !c.conhecido() {
        return Vec::new();
    }
    let mut v = Vec::new();
    if c.posicao == 1 {
        v.push("Campeonato: você LIDERA a temporada".to_string());
    } else {
        v.push(format!("Campeonato: você está em {}º na temporada", c.posicao));
    }
    if let Some(p) = c.para_o_proximo {
        v.push(format!(
            "Campeonato: {} pontos para quem está logo acima de você",
            p.round()
        ));
    }
    if let Some(p) = c.folga {
        v.push(format!(
            "Campeonato: {} pontos de folga para quem vem logo atrás",
            p.round()
        ));
    }
    if let Some(p) = c.projecao {
        // A conta vai ao modelo como CONTA fechada, não como ingredientes. Mandar a tabela
        // e a ordem do grid para ele somar seria pedir aritmética a quem redige — e ele
        // erraria em prosa perfeita.
        v.push(format!(
            "Campeonato: terminando esta corrida como está agora, você fica em {p}º na temporada"
        ));
    }
    v
}

/// Toda peça desta família, com o texto a gravar.
pub fn catalogo() -> Vec<(String, String)> {
    let mut v = vec![(
        "camp_lidera".to_string(),
        "Você lidera o campeonato.".to_string(),
    )];
    for n in 2..=MAX_POSICAO {
        if let Some(o) = ordinal(n) {
            v.push((
                format!("camp_pos_{n}"),
                format!("No campeonato, você está em {o}."),
            ));
        }
    }
    for n in 1..=MAX_PONTOS {
        let Some(c) = cardinal(n) else { continue };
        // "ponto" no singular quando é um só. Num texto isso seria deselegante; numa
        // GRAVAÇÃO é permanente — "você está a um pontos do próximo" ficaria no pacote até
        // alguém regerar.
        let unidade = if n == 1 { "ponto" } else { "pontos" };
        v.push((
            format!("camp_para_{n}"),
            format!("Você está a {c} {unidade} do próximo."),
        ));
        v.push((
            format!("camp_folga_{n}"),
            format!("Você tem {c} {unidade} de folga."),
        ));
    }
    // A PROJEÇÃO. "Terminando assim" é a fórmula do rádio para uma conta que só vale
    // enquanto a corrida não muda — e ela muda a cada volta, que é justamente o motivo de a
    // frase existir.
    v.push((
        "camp_proj_lidera".to_string(),
        "Terminando assim, você assume a liderança.".to_string(),
    ));
    for n in 2..=MAX_POSICAO {
        let Some(o) = ordinal(n) else { continue };
        // SUBIR para o último lugar do acervo é impossível: exigiria estar em quadragésimo
        // primeiro, e essa posição já não tem peça — a resposta inteira teria caído no
        // modelo antes de chegar à projeção. A assimetria é real, e gravar a peça mesmo
        // assim deixaria um arquivo que nada toca.
        if n < MAX_POSICAO {
            v.push((
                format!("camp_proj_sobe_{n}"),
                format!("Terminando assim, você sobe para {o}."),
            ));
        }
        v.push((
            format!("camp_proj_cai_{n}"),
            format!("Terminando assim, você cai para {o}."),
        ));
    }
    v
}
