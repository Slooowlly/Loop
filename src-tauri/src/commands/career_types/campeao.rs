//! DTOs da tela "Campeão da Temporada" — o pop-up que celebra o fim do campeonato
//! da categoria do jogador.
//!
//! Tudo aqui é DADO, nunca prosa: rótulos e frases moram no i18n do frontend, e o
//! backend manda só o `id` do recorde/prêmio mais os valores para interpolar. Assim
//! a tela continua bilíngue sem o Rust conhecer o idioma da UI.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SeasonChampionPayload {
    pub year: i32,
    pub season_number: i32,
    pub category_id: String,
    /// Etapas já disputadas da categoria — o eixo X do gráfico acumulado.
    pub rounds: i32,
    pub player_is_champion: bool,
    /// Vantagem do campeão para o vice, em pontos.
    pub margin: f64,
    /// Os primeiros do campeonato, em ordem de classificação. O gráfico desenha
    /// todos; o pódio usa os três primeiros.
    pub drivers: Vec<SeasonChampionDriver>,
    /// A classificação INTEIRA, enxuta (sem a curva de pontos). Alimenta a lista
    /// miúda embaixo do pódio e o realce por menção de nome: sem ela, passar o mouse
    /// num piloto citado num prêmio não teria onde acender.
    pub standings: Vec<SeasonChampionStanding>,
    /// O campeonato de construtores da categoria, em ordem de classificação.
    pub constructors: Vec<SeasonChampionConstructor>,
    pub awards: Vec<SeasonChampionAward>,
    pub records: Vec<SeasonChampionRecord>,
}

/// Uma equipe no campeonato de construtores da temporada.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SeasonChampionConstructor {
    pub nome: String,
    pub cor: Option<String>,
    pub posicao: i32,
    pub pontos: f64,
    pub vitorias: i32,
    pub podios: i32,
    pub poles: i32,
    pub voltas_rapidas: i32,
    /// Pontos ACUMULADOS por etapa, uma entrada por rodada disputada — a mesma curva
    /// do quadro de pilotos, agora com a camisa somando o que os seus fizeram.
    pub cumulative: Vec<f64>,
    /// Quem pontuou pela equipe no ano, do maior para o menor. Quem se transferiu no
    /// meio da temporada aparece nas DUAS equipes, com o que fez em cada camisa.
    pub pilotos: Vec<SeasonChampionConstructorDriver>,
    /// A equipe pela qual o jogador correu no ano.
    pub is_player_team: bool,
}

/// A contribuição de um piloto para a equipe — o que ele somou COM ESSA camisa.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SeasonChampionConstructorDriver {
    pub id: String,
    pub nome: String,
    pub pontos: f64,
    pub vitorias: i32,
    pub is_player: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SeasonChampionStanding {
    pub id: String,
    pub nome: String,
    pub equipe: Option<String>,
    /// Cor primária da equipe — vira o traço ao lado do nome na lista miúda.
    pub equipe_cor: Option<String>,
    pub posicao: i32,
    pub pontos: f64,
    pub is_player: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SeasonChampionDriver {
    pub id: String,
    pub nome: String,
    pub equipe: Option<String>,
    pub equipe_cor: Option<String>,
    pub nacionalidade: String,
    pub posicao: i32,
    pub pontos: f64,
    /// A campanha em números. O cabeçalho usa a do campeão para contar COMO o título
    /// foi ganho, ao lado do nome — sem isso o dado só existe espalhado nos recordes,
    /// e só para quem liderou cada um deles.
    pub vitorias: i32,
    pub podios: i32,
    pub poles: i32,
    pub voltas_rapidas: i32,
    /// Pontos ACUMULADOS por etapa, uma entrada por rodada disputada.
    pub cumulative: Vec<f64>,
    pub is_champion: bool,
    pub is_player: bool,
}

/// Menção especial da temporada (Grand Chelem, duelo, revelação, virada, etapa do ano).
///
/// `args` alimenta a interpolação da frase no i18n — cada prêmio tem o seu
/// conjunto de chaves, documentado em `career/champion.rs`. O ÍCONE não vem daqui:
/// é apresentação, e o mapa `id → ícone` mora no frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SeasonChampionAward {
    pub id: String,
    pub who: String,
    /// O piloto premiado, quando o prêmio é de UM piloto. `None` nos que falam de
    /// uma dupla, de uma pista ou de uma equipe. A tela usa isso para achar a linha
    /// dele na classificação e mostrar equipe e posição junto do nome.
    pub who_id: Option<String>,
    pub is_player: bool,
    pub args: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SeasonChampionRecord {
    pub id: String,
    pub who: String,
    pub is_player: bool,
    pub valor: String,
    /// Onde o recorde aconteceu (nome da pista), quando ele é de uma etapa só.
    pub sufixo: Option<String>,
}
