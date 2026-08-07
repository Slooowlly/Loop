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
    /// Volta de formação. O `pace_mode` do iRacing, que é fato e não estimativa.
    AntesDaLargada,
    /// Bandeirada. O resultado já existe, e é sobre ele que se fala.
    DepoisDaBandeirada,
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
