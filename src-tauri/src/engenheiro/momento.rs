//! O MOMENTO da corrida: quando o engenheiro tem de calar a boca.
//!
//! Ele fala quando perguntado e quando um evento dispara — e o evento não olha a pista. Uma
//! quebra na grade anunciada no meio de uma disputa roda por cima da única coisa que
//! importa naquele segundo, e o jogador não perde só a fala: perde a curva.
//!
//! Calar nesses instantes é o que mais faz parecer que tem alguém do outro lado. Não é
//! ausência de trabalho, é trabalho — um engenheiro de verdade some na última volta e volta
//! a conversar na relargada.
//!
//! ## O que este módulo NÃO cala
//!
//! - **O spotter.** Ele é segurança: "carro na esquerda" existe exatamente para o momento
//!   quente. Silenciá-lo seria trocar um rádio bonito por uma batida.
//! - **A resposta ao push-to-talk.** O piloto apertou o botão; quase sempre ele apertou
//!   *por causa* do momento quente ("onde ele está?"). Recusar ali seria o oposto de
//!   atender.
//!
//! O que cala é a fala NÃO SOLICITADA — a fila de anúncios de `engenheiroVoz.js`. Ela já
//! espera a vez; agora espera também o momento.
//!
//! ## A amarela é o momento CALMO
//!
//! Sob amarela os carros ficam colados, e o teste de duelo diria "quente" justamente quando
//! ninguém está disputando nada. É o contrário: a amarela é a janela clássica de conversa de
//! rádio, e por isso ela desarma o resto das condições em vez de somar a elas.

use std::sync::Mutex;
use std::time::Instant;

use crate::iracing_sdk::race_monitor::EstadoAgora;

/// Abaixo desta distância, para qualquer um dos lados, é duelo.
///
/// Um segundo. É a mesma ordem de grandeza em que o spotter passa a avisar de carro ao
/// lado, e não por acaso: dentro de um segundo o outro carro já é uma decisão de pilotagem
/// a cada curva, não um número no painel.
const DUELO_S: f64 = 1.0;

/// Voltas do fim em que o rádio se cala. Uma: a última.
const FIM_VOLTAS: i32 = 1;

/// Por que o rádio está calado. Existe para o diagnóstico poder mostrar a razão — "o
/// engenheiro não falou" e "o engenheiro está calado de propósito" são a mesma tela em
/// branco sem isto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motivo {
    Largada,
    Duelo,
    UltimaVolta,
}

/// Os momentos SEM PRESSA — onde cabe a única fala longa do rádio.
///
/// O resto do engenheiro é telegráfico por necessidade: o piloto está a 200 por hora e
/// cada palavra disputa espaço com uma curva. Nestes dois instantes não disputa nada, e é
/// aqui que cabe uma frase sobre o que a corrida significa — a tabela, o que está em jogo,
/// o que acabou de acontecer. É também o único lugar onde os segundos do modelo não custam.
///
/// **"Sentar no carro" não está aqui**, e não por escolha: `EstadoAgora` não carrega esse
/// sinal — ele vive no observador do spotter, com amostragem própria. A volta de formação
/// cobre a mesma batida do produto com um sinal que já existe, e com timing melhor: o
/// piloto está no carro, andando devagar, sem nada para fazer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ocasiao {
    /// Sessão de corrida antes do primeiro verde da tentativa: grid, apresentação, pace
    /// car. Já foi o `pace_mode` do iRacing, que parecia fato e mentia dos dois lados nas
    /// sessões com IA — ver `EstadoAgora::em_formacao`.
    AntesDaLargada,
    /// Bandeirada. O resultado já existe, e é sobre ele que se fala.
    DepoisDaBandeirada,
}

impl Ocasiao {
    /// O nome ESTÁVEL da ocasião. Ele viaja em três direções — o `desfecho` do registro do
    /// rádio, o campo que sobe ao servidor e escolhe o tom, e o cutucão que avisa o front de
    /// que a janela abriu —, e nas três ele é comparado como texto. Renomear um é quebrar a
    /// leitura de todos os registros antigos, então a tradução mora num lugar só.
    pub fn chave(self) -> &'static str {
        match self {
            Ocasiao::AntesDaLargada => "antes_da_largada",
            Ocasiao::DepoisDaBandeirada => "depois_da_bandeirada",
        }
    }
}

/// A ocasião AGORA, se houver uma.
///
/// Pura e sem memória: dizer que a volta de formação está acontecendo é diferente de
/// decidir se já se falou nela. O trinco de uma vez por corrida mora em
/// `commands::engenheiro`, junto com o resto do estado vivo.
pub fn ocasiao(e: &EstadoAgora) -> Option<Ocasiao> {
    if !e.conectado {
        return None;
    }
    if e.bandeira == "Bandeirada" {
        return Some(Ocasiao::DepoisDaBandeirada);
    }
    if e.em_formacao {
        return Some(Ocasiao::AntesDaLargada);
    }
    None
}

/// O momento está quente? `Some(motivo)` cala a fala não solicitada.
pub fn quente(e: &EstadoAgora) -> Option<Motivo> {
    // Fora de corrida não há o que suprimir. No treino e na classificação a fala não
    // solicitada é justamente o que dá vida ao box, e a volta de formação é conversa.
    if !e.em_corrida || e.em_formacao {
        return None;
    }
    // A amarela desarma tudo. Ver o cabeçalho.
    if e.bandeira == "Bandeira amarela" {
        return None;
    }
    // E a BANDEIRADA desarma pelo mesmo motivo, um passo adiante: depois dela ninguém está
    // disputando nada com ninguém. Os carros cruzam a linha colados e desaceleram juntos, o
    // que é o retrato exato de um duelo para quem só olha o gap — e sem esta linha o teste de
    // duelo calaria o rádio na única fala que a bandeirada existe para produzir. É a fala
    // mais cara do rádio: a primeira vitória e o título saem daqui, e cada uma sai uma vez na
    // carreira inteira. Ver `super::marco`.
    if e.bandeira == "Bandeirada" {
        return None;
    }
    // A última volta. `voltas_restantes` é estimativa em prova por tempo, então o rótulo da
    // bandeira branca manda quando existe — ele é fato, e a estimativa é conta.
    if e.bandeira == "Última volta" {
        return Some(Motivo::UltimaVolta);
    }
    if !e.voltas_restantes_estimadas && e.voltas_restantes >= 0 && e.voltas_restantes <= FIM_VOLTAS
    {
        return Some(Motivo::UltimaVolta);
    }
    // A largada. A volta 1 é o instante mais cheio da corrida inteira.
    if e.volta <= 1 {
        return Some(Motivo::Largada);
    }
    // O duelo, dos dois lados. Carro no box ou uma volta à parte não é duelo — é tráfego, e
    // tráfego não justifica calar o rádio.
    for viz in [&e.frente, &e.atras].into_iter().flatten() {
        if viz.no_box || viz.volta_a_parte {
            continue;
        }
        if viz.gap_s.is_finite() && (0.0..DUELO_S).contains(&viz.gap_s) {
            return Some(Motivo::Duelo);
        }
    }
    None
}

// ─── O portão no REGISTRO DO RÁDIO ───────────────────────────────────────────────────────
//
// O medidor do rádio registra fala. Silêncio não deixava rastro nenhum, e é justamente o
// silêncio que este módulo produz: um portão fechado descarta a fala ANTES de ela existir,
// então não há chave, não há áudio e não há linha. Lido depois, o arquivo de uma corrida
// inteira calada por duelo é idêntico ao de uma corrida em que ninguém teve nada a dizer.
//
// O que vai para o arquivo são as TRANSIÇÕES, nunca as amostras: a 60 Hz o estado do portão
// renderia 3.600 linhas por minuto para descrever as ~20 viradas de uma sessão. A virada
// leva o tempo que o estado anterior durou, e é essa a medida que faltava — "o rádio ficou
// fechado por quatro minutos" é uma frase que nenhum outro campo do registro sabe dizer.

/// Quantas leituras seguidas um estado novo precisa para valer como virada.
///
/// Existe por causa do DUELO: o gap de dois carros brigando orbita o limiar de um segundo, e
/// sem confirmação cada travessia viraria duas linhas — dezenas por corrida, todas com um
/// décimo de duração, afogando as viradas de verdade e estragando a soma de tempo calado. Três
/// leituras a ~8 Hz são 0,4 s, menos que o tempo de uma fala começar a sair.
const CONFIRMACOES: u8 = 3;

/// O portão como o registro o vê: o estado vigente, desde quando, e o candidato a substituí-lo.
pub(super) struct Portao {
    atual: (&'static str, Option<&'static str>),
    /// `Instant`, e não o tempo de sessão do sim: a conta que interessa é de quantos segundos o
    /// rádio passou de boca fechada, e ela tem de valer também quando a telemetria pisca.
    desde: Instant,
    /// O estado novo à espera de confirmação, o instante em que foi visto pela PRIMEIRA vez, e
    /// quantas vezes seguidas apareceu. O instante é o da primeira vez de propósito: usar o da
    /// confirmação daria a cada estado 0,4 s de duração a menos do que ele durou.
    candidato: Option<((&'static str, Option<&'static str>), Instant, u8)>,
}

static ULTIMO: Mutex<Option<Portao>> = Mutex::new(None);

/// O portão AGORA em duas palavras: o que ele faz com a fala não solicitada, e a ocasião
/// aberta, se houver.
///
/// Separada do registro para poder ser provada sem arquivo nenhum. Os rótulos daqui são o que
/// sai no `desfecho` da linha, e é por eles que se lê o arquivo meses depois — renomear um é
/// quebrar a leitura de todos os registros antigos.
pub fn estado_do_portao(e: &EstadoAgora) -> (&'static str, Option<&'static str>) {
    let portao = match quente(e) {
        None => "aberto",
        Some(Motivo::Largada) => "largada",
        Some(Motivo::Duelo) => "duelo",
        Some(Motivo::UltimaVolta) => "ultima_volta",
    };
    let ocasiao = ocasiao(e).map(Ocasiao::chave);
    (portao, ocasiao)
}

/// A máquina de estado do portão, **sem I/O e sem estado global**: recebe o retrato e devolve a
/// virada, quando houve uma, com o tempo que o estado anterior durou.
///
/// Separada de [`registrar_transicao`] porque é o único pedaço com lógica de verdade aqui, e o
/// defeito que ela evita — o tremor do gap em torno de um segundo virando cem linhas de um
/// décimo — não apareceria em teste nenhum: só no arquivo, meses depois, já misturado às viradas
/// que interessam.
///
/// A desconexão esquece o estado em vez de virar um rótulo próprio: com o sim fechado
/// `em_corrida` é falso e o portão pareceria aberto, o que produziria uma virada falsa a cada vez
/// que o jogador sai da sessão.
///
/// A primeira leitura depois de conectar **escreve a ABERTURA**, com `de` nulo. Medido numa
/// classificatória de duas voltas: lá o portão fica aberto de ponta a ponta, nunca vira, e o
/// arquivo saiu sem uma única linha dizendo em que estado o rádio esteve. Uma sessão que não vira
/// é o caso mais comum de todos, e era exatamente o que ficava invisível.
pub(super) struct Virada {
    /// O estado que vigorava antes. `None` na abertura da sessão.
    pub de: Option<(&'static str, Option<&'static str>)>,
    /// Quanto o anterior durou. `None` na abertura, porque não havia anterior.
    pub durou_s: Option<f64>,
}

pub(super) fn virar(
    estado: &mut Option<Portao>,
    agora: (&'static str, Option<&'static str>),
    conectado: bool,
    quando: Instant,
) -> Option<Virada> {
    if !conectado {
        *estado = None;
        return None;
    }
    let Some(p) = estado.as_mut() else {
        *estado = Some(Portao {
            atual: agora,
            desde: quando,
            candidato: None,
        });
        return Some(Virada {
            de: None,
            durou_s: None,
        });
    };
    if agora == p.atual {
        // Voltou a ser o que já era: o candidato perde o embalo e nada é registrado. É este
        // ramo que come o tremor.
        p.candidato = None;
        return None;
    }
    match &mut p.candidato {
        Some((candidato, _, vezes)) if *candidato == agora => *vezes += 1,
        _ => p.candidato = Some((agora, quando, 1)),
    }
    let Some((_, visto_em, vezes)) = p.candidato else {
        return None;
    };
    if vezes < CONFIRMACOES {
        return None;
    }
    let anterior = p.atual;
    let durou = visto_em.duration_since(p.desde).as_secs_f64();
    p.atual = agora;
    p.desde = visto_em;
    p.candidato = None;
    Some(Virada {
        de: Some(anterior),
        durou_s: Some((durou * 10.0).round() / 10.0),
    })
}

/// Escreve uma linha no registro do rádio **quando o portão vira** — e nada, no resto do
/// tempo. Chamada pelo amostrador algumas vezes por segundo.
pub fn registrar_transicao(e: &EstadoAgora) {
    let agora = estado_do_portao(e);
    // O lock morre com o bloco: a escrita da linha é I/O, e segurar o portão durante ela
    // atrasaria o próximo tique do amostrador por nada.
    let virada = {
        let Ok(mut guarda) = ULTIMO.lock() else {
            return;
        };
        virar(&mut guarda, agora, e.conectado, Instant::now())
    };
    let Some(virada) = virada else {
        return;
    };
    let (portao, ocasiao_agora) = agora;
    // A abertura tem prosa própria. Lida no arquivo, "Portão aberto" na primeira linha de uma
    // sessão diz outra coisa que "Portão aberto" no meio dela: a primeira é o retrato de entrada,
    // e é o único lugar onde o estado não veio de virada nenhuma.
    let texto = match (virada.de.is_none(), portao, ocasiao_agora) {
        (true, aberto_ou_nao, Some(o)) => {
            format!("Rádio na abertura: {aberto_ou_nao}, e é ocasião: {o}.")
        }
        (true, aberto_ou_nao, None) => format!("Rádio na abertura: {aberto_ou_nao}."),
        (false, "aberto", Some(o)) => format!("Portão aberto, e é ocasião: {o}."),
        (false, "aberto", None) => "Portão aberto.".to_string(),
        (false, fechado, _) => format!("Portão fechado: {fechado}."),
    };
    crate::radio_registro::registrar(&crate::radio_registro::Registro {
        canal: "portao".to_string(),
        fase: "transicao".to_string(),
        desfecho: portao.to_string(),
        texto,
        detalhe: Some(serde_json::json!({
            "de": virada.de.map(|d| d.0),
            "durou_s": virada.durou_s,
            "ocasiao": ocasiao_agora,
            "de_ocasiao": virada.de.and_then(|d| d.1),
            // O RETRATO da virada. Sem ele, "fechou por duelo" na volta 1 é indistinguível de
            // "fechou por duelo" numa relargada, e as duas pedem decisões opostas no rádio.
            "bandeira": e.bandeira,
            "volta": e.volta,
            "em_corrida": e.em_corrida,
            "em_formacao": e.em_formacao,
            "voltas_restantes": e.voltas_restantes,
            "estimadas": e.voltas_restantes_estimadas,
        })),
        ..Default::default()
    });
}
