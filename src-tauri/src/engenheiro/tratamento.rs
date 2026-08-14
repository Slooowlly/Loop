//! **Como ele te chama.**
//!
//! Tudo o mais que o engenheiro diz é informação. Isto não é: "Novato, você está em quinto"
//! carrega exatamente os mesmos fatos que "Você está em quinto", e é outra frase. A diferença
//! é que na segunda alguém está falando com um carro, e na primeira com uma pessoa.
//!
//! ## Duas fases, e o que faz a virada
//!
//! No começo ele te chama de **novato** — porque você é, e porque um engenheiro de verdade
//! usaria essa palavra. Depois ele te chama pelo **nome**.
//!
//! O que encerra a primeira fase é a **primeira vitória**, com a contagem de corridas como
//! rede de segurança para quem demora a vencer. Essa ordem é o produto inteiro em uma regra:
//! o apelido não expira por calendário, ele é *trocado* por um resultado. Quem vence na
//! quinta corrida deixa de ser novato na quinta; quem nunca vence deixa de ser novato quando
//! já não dá para fingir que acabou de chegar.
//!
//! ## O nome tem uma peça, e ela é gerada por carreira
//!
//! O acervo tem 355 sobrenomes gravados, e eles vêm dos pools de IA ([`super::nomes`]). O
//! jogador **digita** o dele. A interseção entre o que ele digita e o que existe em áudio é
//! uma coincidência, e uma feature de proximidade construída em cima de uma coincidência daria
//! o pior resultado possível: funcionaria para uns e para outros não, sem que ninguém
//! entendesse por quê.
//!
//! Por isso o nome dele é **sintetizado uma vez e guardado na máquina dele**, na
//! [`CORRIDAS_PARA_GRAVAR`]-ésima corrida da carreira — ver
//! [`commands::engenheiro::voz_propria`](crate::commands::engenheiro). Uma ida ao servidor por
//! carreira, um arquivo no save, e a partir dali o caminho GRAVADO — que é a maioria
//! esmagadora do que o engenheiro fala — chama o piloto pelo nome como chama qualquer piloto
//! do grid.
//!
//! Enquanto o arquivo não existe, [`Tratamento::Nome`] vem com `chave: None` e o caminho
//! gravado simplesmente não usa vocativo. O modelo continua usando, porque lá o texto é
//! sintetizado na hora e qualquer nome é dizível.
//!
//! ## O PRIMEIRO NOME, e não o sobrenome
//!
//! Rádio de automobilismo usa sobrenome, e é assim que o engenheiro nomeia todo mundo no grid
//! ("Seu rival, Cooper,"). Com o JOGADOR é outra coisa, e a diferença é o ponto: os outros
//! são adversários e ele é a pessoa com quem você fala a corrida inteira. "Magno" e "Silva"
//! não têm a mesma temperatura em português — a segunda é a forma da súmula.
//!
//! É o PRIMEIRO token do que ele digitou, e não há adivinhação nenhuma em cima disso: quem
//! decide como ser chamado é ele, escrevendo o nome que quer ouvir primeiro.
//!
//! ## Ele não te chama toda vez
//!
//! Um vocativo em toda resposta deixa de ser proximidade e vira cacoete. A cadência é de uma
//! a cada [`A_CADA`] respostas, e quem conta é `commands::engenheiro` — este módulo é puro e
//! não guarda nada. As **ocasiões** (volta de formação, bandeirada) ficam fora da conta e
//! levam o vocativo sempre: são os dois momentos em que ele fala sem pressa, e é neles que
//! chamar pelo nome tem para onde ir.

/// Corridas de carreira até o "novato" cair sozinho, para quem ainda não venceu.
///
/// Cinco é meia temporada — tempo de a palavra significar uma fase e não um rótulo
/// permanente. E é folgadamente acima do mínimo da primeira vitória (três corridas), o que
/// impede o caso bobo de a fase de novato acabar antes de a vitória de estreia poder existir.
pub const CORRIDAS_DE_NOVATO: u32 = 5;

/// De quantas em quantas respostas ele te chama. Ver o cabeçalho.
pub const A_CADA: u32 = 4;

/// Corridas de carreira a partir das quais o nome do jogador é sintetizado e guardado.
///
/// Três, e não zero: a peça custa uma ida ao servidor e um arquivo no disco de quem joga, e
/// não faz sentido pagá-los por uma carreira que foi criada para ser olhada e abandonada.
/// Três corridas é o sinal mais barato de que a carreira é de verdade — e é o mesmo número
/// que a primeira vitória usa para virar uma história, em [`super::marco`].
pub const CORRIDAS_PARA_GRAVAR: u32 = 3;

/// Prefixo das peças de vocativo no catálogo.
pub const PREFIXO: &str = "voc_";

/// Como o engenheiro se dirige ao jogador agora.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tratamento {
    /// A primeira fase. Tem peça no acervo que vai no instalador.
    Novato,
    /// O primeiro nome do jogador.
    Nome {
        /// Como se diz — o que sobe ao modelo.
        falado: String,
        /// A chave da peça própria, quando ela **já existe** no save. `None` enquanto a
        /// gravação não chegou: o caminho gravado fica sem vocativo, e o modelo não.
        chave: Option<String>,
    },
}

/// **A fase.** `None` quando não há como chamá-lo de nada.
///
/// `corridas` e `vitorias` são os totais de CARREIRA do piloto do jogador, lidos do save.
/// Um save que não abre devolve zero em tudo, e zero-zero é justamente "novato" — o padrão
/// certo, e não um acidente: quem acabou de começar e quem não pôde ser lido merecem o mesmo
/// tratamento, porque é o mesmo que o engenheiro daria a alguém que ele ainda não conhece.
///
/// `gravado` responde "o arquivo da voz própria já está no disco?". Quem sabe disso é a camada
/// que enxerga o sistema de arquivos, não este módulo.
pub fn decidir(
    corridas: u32,
    vitorias: u32,
    nome_completo: &str,
    gravado: bool,
) -> Option<Tratamento> {
    if vitorias == 0 && corridas < CORRIDAS_DE_NOVATO {
        return Some(Tratamento::Novato);
    }
    let primeiro = primeiro_nome(nome_completo);
    if primeiro.is_empty() {
        // Sem nome não há segunda fase. Voltar para "novato" seria pior que calar: rebaixaria
        // de volta quem já venceu, por causa de um campo em branco.
        return None;
    }
    Some(Tratamento::Nome {
        falado: primeiro.to_string(),
        chave: gravado.then(|| chave_do_nome(nome_completo)).flatten(),
    })
}

/// O primeiro token do nome. `"Magno Silva"` → `"Magno"`.
///
/// `split_whitespace` e não `split(' ')`: ele já descarta o espaço da frente, então um nome
/// digitado com espaço sobrando não vira um primeiro nome vazio.
fn primeiro_nome(nome_completo: &str) -> &str {
    nome_completo.split_whitespace().next().unwrap_or("")
}

/// A chave da peça própria deste jogador: `voc_magno`.
///
/// `None` quando o nome não sobrevive à normalização — campo em branco, ou escrito só com
/// caracteres que o `slug` descarta. Nesse caso não há arquivo a pedir nem a procurar.
///
/// Mesmo `slug` do resto do acervo, e de propósito: é ele que já sabe que "Gonçalo" vira
/// `goncalo` e não `gon_alo`.
pub fn chave_do_nome(nome_completo: &str) -> Option<String> {
    let s = super::nomes::slug(primeiro_nome(nome_completo));
    (!s.is_empty()).then(|| format!("{PREFIXO}{s}"))
}

/// **Já vale gastar a síntese com este jogador?**
///
/// A vitória entra junto com a contagem porque ela também vira a fase: quem vence na segunda
/// corrida passa a ser chamado pelo nome ali, e esperar a terceira o deixaria uma corrida sem
/// vocativo por causa de um limiar pensado para outra coisa.
pub fn pode_gravar(corridas: u32, vitorias: u32) -> bool {
    corridas >= CORRIDAS_PARA_GRAVAR || vitorias > 0
}

/// O texto exato a sintetizar. É o mesmo desenho das peças do acervo: nome mais vírgula, com
/// a maiúscula que abre uma fala.
pub fn texto_da_peca(nome_completo: &str) -> Option<String> {
    let mut chars = primeiro_nome(nome_completo).chars();
    let primeira = chars.next()?;
    Some(primeira.to_uppercase().chain(chars).collect::<String>())
}

/// As redações de "novato", na ordem do rodízio.
///
/// Três, pelo mesmo motivo que a espera e o empurrão do freio têm três: numa corrida o
/// vocativo sai várias vezes, e a mesma palavra idêntica na quarta vez para de soar como
/// alguém falando com você e passa a soar como um arquivo tocando.
const NOVATO: [(&str, &str); 3] = [
    ("voc_novato", "Novato,"),
    ("voc_novato_2", "Ei, novato,"),
    ("voc_novato_3", "Olha, novato,"),
];

/// A peça a tocar neste tratamento, se houver uma.
///
/// O `None` de um [`Tratamento::Nome`] sem gravação é a espera, e não um buraco: o caminho
/// gravado fica sem vocativo até o arquivo chegar, e o modelo continua chamando pelo nome.
///
/// O RODÍZIO só vale para "novato". A peça própria é uma só — três tomadas do mesmo nome
/// custariam três sínteses e três arquivos para variar uma palavra que já é a variação.
pub fn peca(t: &Tratamento, rodizio: usize) -> Option<String> {
    match t {
        Tratamento::Novato => Some(NOVATO[rodizio % NOVATO.len()].0.to_string()),
        Tratamento::Nome { chave, .. } => chave.clone(),
    }
}

/// Como o MODELO deve chamá-lo. É o texto que vai ao servidor.
pub fn como_chamar(t: &Tratamento) -> String {
    match t {
        Tratamento::Novato => "novato".to_string(),
        Tratamento::Nome { falado, .. } => falado.clone(),
    }
}

/// A família no catálogo do pacote de voz.
pub fn familia_tratamento() -> Vec<(String, String)> {
    NOVATO
        .iter()
        .map(|(k, t)| ((*k).to_string(), (*t).to_string()))
        .collect()
}
