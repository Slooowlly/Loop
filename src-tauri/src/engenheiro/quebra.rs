//! A FALA DE QUEBRA: o engenheiro conta que um carro do grid teve problema.
//!
//! É o evento mais raro que o rádio narra — algumas quebras por corrida no grid inteiro — e
//! por isso o único que pode gastar quatro peças e sete segundos. Um aviso de spotter tem que
//! caber em 1,2 s porque sai dezenas de vezes; este não concorre com nada.
//!
//! ## A gramática
//!
//! ```text
//! [abertura] [sobrenome] [aposto] [trecho]. [coda]
//!   ↑ vínculo   ↑ nome     ↑ frame  ↑ peça+severidade   ↑ só em abandono
//! ```
//!
//! Cada colchete é um `.wav` gravado à parte, e a fala é a colagem deles. A alternativa —
//! gravar a frase inteira — custaria 355 sobrenomes × 108 trechos × 8 enquadramentos, que é
//! um número que não existe. Por peça, custa 576.
//!
//! A colagem foi medida antes de qualquer coisa ser gravada
//! (`scripts/tts-poc/audicao-quebra.mjs`): as quatro formas montadas contra a mesma frase
//! numa tomada só. A montada fica de 1,5% a 24% mais longa que a inteira, e o inchaço não
//! vem das pausas inseridas — vem da peça curta falada sozinha, que sai com entoação
//! deliberada que a mesma palavra no meio de uma frase não tem. Aprovado de ouvido nas
//! quatro formas.
//!
//! ## Um enquadramento por fala, nunca dois
//!
//! Se o piloto é seu rival E lidera o campeonato, dizer as duas coisas dá quatro emendas e um
//! trava-língua. [`Vinculo::escolher`] resolve por precedência e o resto cala. A ordem é por
//! quanto o fato muda a corrida DO JOGADOR, não por quanto ele é interessante:
//!
//! 1. nêmesis / rival — a única relação pessoal que o jogo modela
//! 2. companheiro de equipe — mesmo carro, mesma engenharia, dado técnico
//! 3. líder do campeonato — muda a tabela de todo mundo
//! 4. vizinho de pontos — muda a tabela do jogador
//! 5. nenhum — pela equipe, sem nome
//!
//! ## Por que a forma pela equipe sobrevive tendo 355 sobrenomes gravados
//!
//! Dois motivos, e o segundo é o que a torna obrigatória:
//!
//! - **Registro.** "O piloto dois da Kitsune" diz ao jogador, sem dizer, que ele não precisa
//!   se importar com esse. Nomear todo mundo achata a diferença entre o rival e o 19º.
//! - **Fallback.** O nome do JOGADOR não sai de pool nenhum, e piloto de save antigo pode
//!   ter sobrenome que não gravamos. Sem esta forma a fala morreria; com ela, degrada.

use crate::engenheiro::nomes;

/// Prefixo das chaves desta família no catálogo do engenheiro.
pub const PREFIXO: &str = "qb_";

/// Dentro de quantos pontos o outro piloto ainda é "alguns pontos" de distância.
///
/// Primeiro corte, ancorado no que uma corrida vale: uma vitória tira 25 pontos de diferença.
/// Abaixo de 15 a queda do outro muda a conta do jogador na mesma corrida; acima, é notícia
/// de campeonato, não de rádio.
pub const PONTOS_VIZINHO: i32 = 15;

/// Como o piloto que quebrou se relaciona com o jogador — é isto que decide a redação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vinculo {
    Nemesis,
    Rival,
    Companheiro,
    Lider,
    /// Vizinho de tabela À FRENTE do jogador. A queda dele é ganho direto.
    PontosAFrente,
    /// Vizinho de tabela ATRÁS. A queda dele não muda nada para o jogador — e por isso não
    /// ganha coda de comemoração.
    PontosAtras,
    Nenhum,
}

/// Os fatos que o rádio precisa saber para redigir. Quem preenche é quem tem o banco aberto;
/// aqui só se decide o que fazer com eles.
#[derive(Debug, Clone)]
pub struct Contexto {
    pub nome_completo: String,
    /// Nome da equipe como está no catálogo (`"Kitsune"`, `"Grid Start Racing School"`).
    pub equipe: Option<String>,
    /// Assento na equipe: 1 ou 2. Fora dessa faixa cai no 2 — é o assento do coadjuvante, e
    /// errar para baixo é mais barato que anunciar um desconhecido como piloto 1.
    pub assento: u8,
    pub e_nemesis: bool,
    pub e_rival: bool,
    pub e_companheiro: bool,
    pub lidera_campeonato: bool,
    /// Pontos do outro MENOS os do jogador. Positivo = ele está à frente. `None` quando a
    /// tabela não está disponível (pré-temporada, save sem campeonato).
    pub delta_pontos: Option<i32>,
    /// Chave da peça (`"engine"`, `"gearbox"`, …).
    pub peca: String,
    /// `"light"`, `"heavy"` ou `"dnf"`.
    pub severidade: String,
    /// Semente da variação. Estável por carro+ordem no chamador, para a mesma quebra dar
    /// sempre a mesma frase.
    pub variante: usize,
    /// Quantos ABANDONOS a corrida já teve, contando este. Alimenta o comentário de atrito.
    /// Zero quando o chamador não tem a conta (demo, teste).
    pub abandonos_ate_aqui: u32,
}

impl Vinculo {
    /// Aplica a precedência. Ver o cabeçalho do módulo para a ordem e o porquê dela.
    pub fn escolher(c: &Contexto) -> Vinculo {
        if c.e_nemesis {
            return Vinculo::Nemesis;
        }
        if c.e_rival {
            return Vinculo::Rival;
        }
        if c.e_companheiro {
            return Vinculo::Companheiro;
        }
        if c.lidera_campeonato {
            return Vinculo::Lider;
        }
        match c.delta_pontos {
            Some(d) if d > 0 && d <= PONTOS_VIZINHO => Vinculo::PontosAFrente,
            Some(d) if d < 0 && -d <= PONTOS_VIZINHO => Vinculo::PontosAtras,
            _ => Vinculo::Nenhum,
        }
    }

    /// O vínculo exige o nome do piloto? Rival e nêmesis exigem — é a regra do produto, e é
    /// ela que faz a fala valer a pena. Os outros aceitam a degradação para a forma da equipe.
    pub fn exige_nome(self) -> bool {
        matches!(self, Vinculo::Nemesis | Vinculo::Rival)
    }
}

// ─── As peças ────────────────────────────────────────────────────────────────

/// Abertura, quando há uma. Vem ANTES do nome.
fn abertura(v: Vinculo) -> Option<(&'static str, &'static str)> {
    match v {
        Vinculo::Nemesis => Some(("ab_nemesis", "Seu maior rival,")),
        Vinculo::Rival => Some(("ab_rival", "Seu rival,")),
        Vinculo::Companheiro => Some(("ab_companheiro", "Seu companheiro,")),
        _ => None,
    }
}

/// Aposto, quando há um. Vem DEPOIS do nome, entre vírgulas.
fn aposto(v: Vinculo) -> Option<(&'static str, &'static str)> {
    match v {
        Vinculo::Lider => Some(("ap_lider", "que lidera o campeonato,")),
        Vinculo::PontosAFrente => Some(("ap_frente", "que está alguns pontos à sua frente,")),
        Vinculo::PontosAtras => Some(("ap_atras", "que está alguns pontos atrás de você,")),
        _ => None,
    }
}

/// As duas aberturas de assento, usadas só na forma sem vínculo (`[assento] [equipe]`).
fn assento(n: u8) -> (&'static str, &'static str) {
    if n == 1 {
        ("ab_piloto1", "O piloto um da")
    } else {
        ("ab_piloto2", "O piloto dois da")
    }
}

/// Comentário do engenheiro, em FRASE PRÓPRIA depois do trecho.
///
/// Duas famílias disputam esta posição, e o ATRITO ganha quando as duas cabem. Uma fala com
/// as duas seria três frases e uns nove segundos — e o atrito sai UMA vez por corrida contra
/// as três ou quatro do ganho, então é ele a novidade.
///
/// O comentário de ganho só sai em abandono e só quando o piloto estava à frente do jogador.
/// Comemorar uma quebra leve é comemorar cedo (o cara pode terminar na frente mesmo assim) e
/// comemorar a queda de quem estava atrás é comemorar nada.
fn coda(
    v: Vinculo,
    severidade: &str,
    variante: usize,
    abandonos: u32,
) -> Option<(&'static str, &'static str)> {
    if severidade != "dnf" {
        return None;
    }
    // O cruzamento EXATO do limiar, não "acima dele": é o que faz a fala sair uma vez só.
    if abandonos == ABANDONOS_PARA_COMENTAR + 1 {
        return Some(
            [
                ("co_atrito", "A prova está devorando carros hoje."),
                ("co_atrito_2", "Já são muitos abandonos nesta corrida."),
                ("co_atrito_3", "Está caindo gente demais aqui."),
            ][variante % 3],
        );
    }
    let ganho = matches!(
        v,
        Vinculo::Nemesis | Vinculo::Rival | Vinculo::Lider | Vinculo::PontosAFrente
    );
    if !ganho {
        return None;
    }
    Some(
        [
            ("co_otima", "Ótima notícia pra nós."),
            ("co_ummenos", "Um a menos na nossa frente."),
            ("co_ajuda", "Isso ajuda a gente."),
        ][variante % 3],
    )
}

/// Peça com preposição/artigo ("no motor", "na suspensão").
pub fn part_com_artigo(part_key: &str) -> &'static str {
    match part_key {
        "engine" => "no motor",
        "gearbox" => "no câmbio",
        "brakes" => "nos freios",
        "suspension" => "na suspensão",
        "cooling" => "no arrefecimento",
        "front_wing" => "na asa dianteira",
        "rear_wing" => "na asa traseira",
        "sidepods" => "nas laterais",
        "underbody" => "no assoalho",
        "chassis" => "no chassi",
        "electronics" => "na parte elétrica",
        _ => "no carro",
    }
}

/// As 11 peças mais o genérico, na ordem em que entram no catálogo.
pub const PECAS: [&str; 12] = [
    "engine",
    "gearbox",
    "brakes",
    "suspension",
    "cooling",
    "front_wing",
    "rear_wing",
    "sidepods",
    "underbody",
    "chassis",
    "electronics",
    "outra",
];

/// FONTE ÚNICA das redações do rádio: 3 opções por (peça, severidade), voz do engenheiro,
/// 3ª pessoa. Devolve o TRECHO depois do nome (a peça já vem escrita no texto). Peça ou
/// severidade desconhecida cai num genérico. O abandono é tratado à parte
/// ([`dnf_trecho`]), porque a redação é diferente.
pub fn breakdown_frases(part_key: &str, severity: &str) -> [&'static str; 3] {
    match (severity, part_key) {
        // ── MOTOR ──
        ("light", "engine") => [
            "sente o motor perdendo fôlego",
            "relata o motor engasgando",
            "avisa que o motor não está redondo",
        ],
        ("heavy", "engine") => [
            "está com o motor em pane",
            "perdeu potência e o motor pode não aguentar",
            "relata o motor no limite da quebra",
        ],
        // ── CÂMBIO ──
        ("light", "gearbox") => [
            "está com o câmbio arisco nas trocas",
            "relata engates falhando no câmbio",
            "sente o câmbio embolando as marchas",
        ],
        ("heavy", "gearbox") => [
            "está com o câmbio travando",
            "perdeu marchas e o câmbio está indo embora",
            "relata o câmbio prestes a parar",
        ],
        // ── FREIOS ──
        ("light", "brakes") => [
            "sente o pedal de freio amolecendo",
            "relata os freios pedindo água",
            "avisa que os freios estão longos",
        ],
        ("heavy", "brakes") => [
            "está praticamente sem freio",
            "relata os freios cozinhando",
            "perdeu o pedal e está com os freios em pane",
        ],
        // ── SUSPENSÃO ──
        ("light", "suspension") => [
            "sente a suspensão reclamando nas zebras",
            "relata o carro batendo demais atrás",
            "avisa de uma folga na suspensão",
        ],
        ("heavy", "suspension") => [
            "está com a suspensão comprometida",
            "relata algo quebrado na suspensão",
            "perdeu firmeza com a suspensão em pane",
        ],
        // ── ARREFECIMENTO ──
        ("light", "cooling") => [
            "vê a temperatura subindo aos poucos",
            "relata o arrefecimento no limite",
            "avisa que a água está esquentando",
        ],
        ("heavy", "cooling") => [
            "está com o arrefecimento estourando",
            "relata superaquecimento crítico",
            "está com a temperatura no vermelho",
        ],
        // ── ASA DIANTEIRA ──
        ("light", "front_wing") => [
            "relata a asa dianteira leve",
            "sente o bico perdendo apoio",
            "avisa de dano na asa dianteira",
        ],
        ("heavy", "front_wing") => [
            "está com a asa dianteira danificada",
            "perdeu apoio na frente com a asa comprometida",
            "relata a asa dianteira se soltando",
        ],
        // ── ASA TRASEIRA ──
        ("light", "rear_wing") => [
            "sente a traseira solta na reta",
            "relata a asa traseira leve",
            "avisa de dano na asa traseira",
        ],
        ("heavy", "rear_wing") => [
            "está com a asa traseira danificada",
            "perdeu apoio atrás e a traseira está nervosa",
            "relata a asa traseira cedendo",
        ],
        // ── LATERAIS ──
        ("light", "sidepods") => [
            "relata dano nas laterais",
            "sente o carro puxando de lado",
            "avisa de um amassado nas laterais",
        ],
        ("heavy", "sidepods") => [
            "está com as laterais abertas",
            "perdeu parte da lateral do carro",
            "relata dano sério nas laterais",
        ],
        // ── ASSOALHO ──
        ("light", "underbody") => [
            "sente o assoalho raspando",
            "relata perda de apoio no assoalho",
            "avisa de dano no assoalho",
        ],
        ("heavy", "underbody") => [
            "está com o assoalho comprometido",
            "perdeu downforce com o assoalho ferido",
            "relata o assoalho arrastando forte",
        ],
        // ── CHASSI ──
        ("light", "chassis") => [
            "sente o chassi estranho",
            "relata o carro desalinhado",
            "avisa de algo torto no chassi",
        ],
        ("heavy", "chassis") => [
            "está com o chassi comprometido",
            "relata dano estrutural no chassi",
            "perdeu rigidez com o chassi ferido",
        ],
        // ── PARTE ELÉTRICA ──
        ("light", "electronics") => [
            "relata oscilações na parte elétrica",
            "sente o painel piscando",
            "avisa de falha elétrica intermitente",
        ],
        ("heavy", "electronics") => [
            "está com pane elétrica",
            "perdeu comandos com a parte elétrica em pane",
            "relata o carro cortando por falha elétrica",
        ],
        // ── genérico ──
        ("heavy", _) => [
            "está com um problema grave no carro",
            "relata um problema sério no carro",
            "perdeu desempenho por algo grave no carro",
        ],
        _ => [
            "apresenta um problema no carro",
            "relata algo estranho no carro",
            "avisa de um problema no carro",
        ],
    }
}

// ─── Duas quebras de uma vez ─────────────────────────────────────────────────

/// A conjunção entre os dois nomes. Peça própria, e ela quase não existiu.
///
/// `"e,"` é MONOSSÍLABO, o mesmo caso que reprovou o `"um,"` do tempo de volta — e a medição
/// concordou: a montagem com ela saiu 46% mais longa que a tomada única. Foi aprovada de
/// OUVIDO mesmo assim, e a alternativa que o número favorecia (`"e Silva,"` fundido, 34%)
/// custaria 355 peças, um banco de sobrenomes inteiro, para chegar à mesma duração final.
///
/// A terceira via — lista sem conjunção, `"Cooper, Silva, tiveram…"` — foi reprovada de ouvido
/// inclusive em tomada única. O problema ali não era a colagem, era o texto.
pub const CONJUNCAO: (&str, &str) = ("conj_e", "e,");

/// Trecho PLURAL: dois pilotos, um problema genérico. 3 opções por severidade.
///
/// A peça some de propósito. `"Cooper e Silva tiveram problemas no motor e no câmbio"` é um
/// trava-língua, e dizer a peça de um só mentiria sobre o outro. Duas quebras ao mesmo tempo
/// são um fato sobre a CORRIDA, não sobre um componente — e é assim que o rádio as trata.
pub fn dupla_frases(severidade: &str) -> [&'static str; 3] {
    match severidade {
        "dnf" => [
            "abandonaram a corrida com problemas no carro",
            "estão fora da corrida com problemas no carro",
            "foram retirados da corrida com problemas no carro",
        ],
        "heavy" => [
            "estão com problemas graves no carro",
            "relatam panes no carro",
            "tiveram problemas sérios no carro",
        ],
        _ => [
            "tiveram problemas no carro",
            "relatam algo estranho no carro",
            "avisaram de problemas no carro",
        ],
    }
}

/// A chave do trecho plural: `qb_dupla_dnf_0`.
pub fn chave_dupla(severidade: &str, variante: usize) -> String {
    let sev = match severidade {
        "dnf" => "dnf",
        "heavy" => "heavy",
        _ => "light",
    };
    format!("{PREFIXO}dupla_{sev}_{}", variante % 3)
}

/// Junta DUAS quebras simultâneas numa fala só, ou `None` quando não deve juntar.
///
/// Recusa em três casos, e cada um por um motivo diferente:
///
/// - **Severidades diferentes.** Um abandono e uma quebra leve viram `"tiveram problemas"`, e a
///   fala apaga justamente a que importa.
/// - **Algum dos dois tem VÍNCULO.** O rival do jogador não entra em fala coletiva: o
///   enquadramento é o que fazia aquela fala valer a pena, e juntar o dissolve.
/// - **Falta gravação de algum sobrenome.** A forma pela equipe não cabe aqui —
///   `"O piloto dois da Kitsune e o piloto um da Ferrari…"` é longo demais para um evento que
///   a fusão existia para ENCURTAR.
///
/// Recusado, o chamador emite as duas separadas, que é o comportamento de sempre.
pub fn montar_duplo(a: &Contexto, b: &Contexto) -> Option<Fala> {
    if a.severidade != b.severidade {
        return None;
    }
    if Vinculo::escolher(a) != Vinculo::Nenhum || Vinculo::escolher(b) != Vinculo::Nenhum {
        return None;
    }
    let sob_a = nomes::sobrenome_de(&a.nome_completo);
    let sob_b = nomes::sobrenome_de(&b.nome_completo);
    // Dois nomes iguais na mesma fala ("Silva e Silva") seria pior que duas falas separadas.
    if sob_a.eq_ignore_ascii_case(sob_b) {
        return None;
    }
    let chave_a = nomes::chave_sobrenome(sob_a)?;
    let chave_b = nomes::chave_sobrenome(sob_b)?;

    let trecho = dupla_frases(&a.severidade)[a.variante % 3];
    let mut pecas = vec![
        chave_a,
        CONJUNCAO.0.to_string(),
        chave_b,
        chave_dupla(&a.severidade, a.variante),
    ];
    let mut texto = format!("{sob_a} e {sob_b} {trecho}.");

    // O comentário de atrito sai no cruzamento EXATO do limiar, e a fusão engolia esse
    // cruzamento: duas quebras na mesma volta viram uma fala só, então a contagem pulava de
    // 3 para 5 e a fala nunca saía naquela corrida. Aqui a dupla também é candidata — basta
    // que UM dos dois abandonos seja o que cruzou.
    let cruzou = [a, b]
        .into_iter()
        .find_map(|c| coda(Vinculo::Nenhum, &c.severidade, c.variante, c.abandonos_ate_aqui));
    if let Some((k, t)) = cruzou {
        pecas.push(k.to_string());
        texto.push(' ');
        texto.push_str(t);
    }

    Some(Fala { pecas, texto })
}

/// Trecho de ABANDONO — 3 opções, com a peça já embutida.
///
/// A redação antiga (`"{nome} está fora — problemas no motor"`) tinha travessão no meio, e o
/// travessão faz o modelo respirar: 0,44 s de silêncio DENTRO da peça, medido na audição. Numa
/// fala já montada por colagem, a pausa interna soma com a da emenda e vira buraco. As três
/// aqui são de oração única, sem pontuação no meio.
pub fn dnf_trecho(part_key: &str, variante: usize) -> String {
    let peca = part_com_artigo(part_key);
    match variante % 3 {
        0 => format!("abandona a corrida com problemas {peca}"),
        1 => format!("está fora da corrida com problemas {peca}"),
        _ => format!("foi retirado da corrida com problemas {peca}"),
    }
}

/// A chave do trecho no catálogo: `qb_light_engine_0`, `qb_dnf_gearbox_2`.
pub fn chave_trecho(severidade: &str, peca: &str, variante: usize) -> String {
    format!("{PREFIXO}{severidade}_{peca}_{}", variante % 3)
}

// ─── A montagem ──────────────────────────────────────────────────────────────

/// Uma fala pronta: as peças na ordem de tocar, e o texto que elas somam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fala {
    /// Chaves do catálogo, na ordem. Vazio quando a quebra não merece rádio.
    pub pecas: Vec<String>,
    /// O mesmo conteúdo em texto, para o card do overlay. Sai SEMPRE, mesmo quando `pecas`
    /// está vazio — a tela mostra o que o rádio cala.
    pub texto: String,
}

/// A partir de quantos abandonos a corrida vira assunto POR SI, e não mais só o piloto.
///
/// Quatro (`> 3`). Medido em `car::breakdown::medicao`: um grid de 24 carros em 18 voltas
/// produz 3,7 abandonos por corrida em média, e 75% das corridas passam de três. Ou seja, o
/// comentário sai na maioria das corridas — mas UMA vez em cada, no cruzamento do limiar.
/// Repeti-lo a cada abandono seguinte seria dizer a mesma novidade quatro vezes.
pub const ABANDONOS_PARA_COMENTAR: u32 = 3;

/// Como o piloto é NOMEADO nesta fala. É a única decisão que pode falhar, e a falha tem
/// consequência diferente em cada braço — por isso ela é explícita.
enum Sujeito {
    /// Sobrenome gravado. Chave do `.wav` e o texto para o card.
    Nome(String, String),
    /// Sem gravação do sobrenome, mas a equipe está no catálogo: assento + equipe.
    Equipe(&'static str, &'static str, String, &'static str),
    /// Nem nome nem equipe graváveis — ou o vínculo exigia o nome e não há. Sem áudio.
    Mudo,
}

/// Monta a fala. Ver o cabeçalho do módulo para a gramática.
pub fn montar(c: &Contexto) -> Fala {
    let v = Vinculo::escolher(c);

    let sobrenome = nomes::sobrenome_de(&c.nome_completo);
    // O sobrenome só existe em áudio se ele saiu de um pool. O nome do JOGADOR não sai, e
    // piloto de save antigo pode não sair — daí o `Option`.
    let por_nome = nomes::chave_sobrenome(sobrenome)
        .map(|chave| Sujeito::Nome(chave, sobrenome.to_string()));
    let por_equipe = match (
        c.equipe.as_deref().and_then(nomes::chave_equipe),
        c.equipe.as_deref().and_then(nomes::equipe_falada),
    ) {
        (Some(chave_eq), Some(falado)) => {
            let (k, t) = assento(c.assento);
            Some(Sujeito::Equipe(k, t, chave_eq, falado))
        }
        _ => None,
    };

    // Qual das duas formas vem PRIMEIRO depende do vínculo, e essa é a decisão de produto do
    // recurso inteiro:
    //
    // - Com vínculo, o nome é o assunto. Se o vínculo é rival ou nêmesis ele é OBRIGATÓRIO —
    //   chamar o rival do jogador de "o piloto dois da Kitsune" apaga exatamente o que fazia
    //   a fala valer a pena, então sem gravação do nome o áudio cala e só o card conta.
    // - Sem vínculo, a equipe vem primeiro mesmo havendo gravação do nome. Não é economia: é
    //   registro. Nomear todo mundo achata a diferença entre o rival e o 19º, e é justamente
    //   NÃO nomear que diz ao jogador, sem dizer, que aquele ali não importa.
    let sujeito = if v == Vinculo::Nenhum {
        por_equipe.or(por_nome)
    } else if v.exige_nome() {
        por_nome
    } else {
        por_nome.or(por_equipe)
    }
    .unwrap_or(Sujeito::Mudo);

    let trecho_texto = if c.severidade == "dnf" {
        dnf_trecho(&c.peca, c.variante)
    } else {
        breakdown_frases(&c.peca, &c.severidade)[c.variante % 3].to_string()
    };
    // Peça ou severidade fora do conhecido usa a redação genérica — e tem que usar a CHAVE
    // genérica junto, senão o áudio pedido não existe.
    let peca_chave = if PECAS.contains(&c.peca.as_str()) { c.peca.as_str() } else { "outra" };
    let sev_chave = match c.severidade.as_str() {
        "dnf" => "dnf",
        "heavy" => "heavy",
        _ => "light",
    };

    let mut pecas: Vec<String> = Vec::new();
    let mut texto = String::new();

    match &sujeito {
        Sujeito::Nome(chave, nome) => {
            // Abertura de vínculo OU aposto, nunca os dois.
            if let Some((k, t)) = abertura(v) {
                pecas.push(k.to_string());
                texto.push_str(t.trim_end_matches(','));
                texto.push(' ');
            }
            pecas.push(chave.clone());
            texto.push_str(nome);
            if let Some((k, t)) = aposto(v) {
                pecas.push(k.to_string());
                texto.push_str(", ");
                texto.push_str(t);
            }
            texto.push(' ');
        }
        Sujeito::Equipe(k_assento, t_assento, chave_eq, falado) => {
            pecas.push(k_assento.to_string());
            pecas.push(chave_eq.clone());
            texto.push_str(t_assento);
            texto.push(' ');
            texto.push_str(falado);
            // Sem nome, o aposto ainda cabe e ainda informa — só perde o nome próprio.
            if let Some((k, t)) = aposto(v) {
                pecas.push(k.to_string());
                texto.push_str(", ");
                texto.push_str(t);
            }
            texto.push(' ');
        }
        // Sem áudio: o card mantém o nome COMPLETO, que é o que a tela sempre teve espaço
        // para mostrar. É o áudio que não tem.
        Sujeito::Mudo => {
            texto.push_str(&c.nome_completo);
            texto.push(' ');
        }
    }

    pecas.push(chave_trecho(sev_chave, peca_chave, c.variante));
    texto.push_str(&trecho_texto);
    texto.push('.');

    if let Some((k, t)) = coda(v, &c.severidade, c.variante, c.abandonos_ate_aqui) {
        pecas.push(k.to_string());
        texto.push(' ');
        texto.push_str(t);
    }

    // A ÚNICA razão de silêncio é não haver como nomear o sujeito. Toda quebra fala — a
    // decisão é do produto: se um carro do grid teve problema, o rádio conta. Custa, e o
    // custo está medido: ~33 falas por corrida num grid de 24 em 18 voltas
    // (`car::breakdown::medicao`), das quais ~4 são abandonos.
    let mudo = matches!(sujeito, Sujeito::Mudo);
    Fala {
        pecas: if mudo { Vec::new() } else { pecas },
        texto,
    }
}

// ─── O catálogo ──────────────────────────────────────────────────────────────

/// TODA peça que [`montar`] pode emitir, com o texto exato a gravar.
///
/// Entra em [`crate::engenheiro::catalogo`], que é o que o `engenheiro-pack.mjs` lê. A ordem é
/// determinística de ponta a ponta — inclusive nos sobrenomes, que vêm de um `BTreeSet` — para
/// que o gerador não veja a lista inteira como nova a cada execução.
pub fn familia_quebra() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();

    for (k, t) in [
        ("ab_nemesis", "Seu maior rival,"),
        ("ab_rival", "Seu rival,"),
        ("ab_companheiro", "Seu companheiro,"),
        ("ab_piloto1", "O piloto um da"),
        ("ab_piloto2", "O piloto dois da"),
        ("ap_lider", "que lidera o campeonato,"),
        ("ap_frente", "que está alguns pontos à sua frente,"),
        ("ap_atras", "que está alguns pontos atrás de você,"),
        ("co_otima", "Ótima notícia pra nós."),
        ("co_ummenos", "Um a menos na nossa frente."),
        ("co_ajuda", "Isso ajuda a gente."),
        ("co_atrito", "A prova está devorando carros hoje."),
        ("co_atrito_2", "Já são muitos abandonos nesta corrida."),
        ("co_atrito_3", "Está caindo gente demais aqui."),
        CONJUNCAO,
    ] {
        v.push((k.to_string(), t.to_string()));
    }

    for sev in ["light", "heavy", "dnf"] {
        for (i, frase) in dupla_frases(sev).iter().enumerate() {
            v.push((chave_dupla(sev, i), format!("{frase}.")));
        }
    }

    for peca in PECAS {
        for sev in ["light", "heavy"] {
            for (i, frase) in breakdown_frases(peca, sev).iter().enumerate() {
                v.push((chave_trecho(sev, peca, i), format!("{frase}.")));
            }
        }
        for i in 0..3 {
            v.push((chave_trecho("dnf", peca, i), format!("{}.", dnf_trecho(peca, i))));
        }
    }

    // O sobrenome sai com VÍRGULA de propósito. Sozinho e com ponto, o modelo lhe dá entoação
    // de fim de frase, e colar isso num verbo produz duas orações em vez de uma. A vírgula
    // pede continuação — o mesmo truque que salvou "Volta em," na POC.
    for s in nomes::sobrenomes() {
        v.push((format!("{}{}", nomes::PREFIXO_SOBRENOME, nomes::slug(s)), format!("{s},")));
    }
    for (_, falado) in nomes::EQUIPES_FALADAS {
        v.push((
            format!("{}{}", nomes::PREFIXO_EQUIPE, nomes::slug(falado)),
            format!("{falado},"),
        ));
    }

    v
}

#[cfg(test)]
mod tests;
