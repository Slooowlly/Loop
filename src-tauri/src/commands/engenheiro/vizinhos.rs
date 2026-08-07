//! Quem são os vizinhos, para o save.
//!
//! O NOME do vizinho não passa por aqui: ele vem no `EstadoAgora`, do YAML da sessão, e o
//! grid do iRacing saiu da própria carreira — a fala nomeada precisa só do sobrenome, e o
//! sobrenome já está na telemetria. O que este módulo resolve é o VÍNCULO: nêmesis, rival
//! ou companheiro de equipe. Isso o iRacing não tem como saber.
//!
//! O caminho é o mesmo da projeção do campeonato — número do carro → `iracing_numbers` →
//! piloto — e o custo de errar é diferente. Uma projeção errada é uma conta errada; um
//! vínculo errado é o engenheiro chamando de "seu maior rival" alguém que o jogador nunca
//! viu. Por isso o número que não casa vira `Nenhum`, e não um palpite.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::engenheiro::quebra::Vinculo;
use crate::engenheiro::vizinhanca::Contexto;
use crate::iracing_sdk::race_monitor::EstadoAgora;

/// O vínculo de cada vizinho com o jogador.
///
/// `por_numero` é o mesmo mapa que a projeção usa — passado de fora para a leitura do
/// arquivo acontecer uma vez por pergunta, e não duas.
pub fn carregar(
    conn: &Connection,
    estado: &EstadoAgora,
    por_numero: &HashMap<i64, String>,
) -> Contexto {
    if por_numero.is_empty() {
        return Contexto::default();
    }
    let Ok(jogador) = crate::db::queries::drivers::get_player_driver(conn) else {
        return Contexto::default();
    };

    let nemesis = crate::db::queries::player_nemesis::get_current_nemesis(conn)
        .ok()
        .flatten();
    let rivais: Vec<String> = crate::db::queries::rivalries::get_rivalries_for_pilot(conn, &jogador.id)
        .map(|rs| {
            rs.iter()
                .flat_map(|r| [r.piloto1_id.clone(), r.piloto2_id.clone()])
                .filter(|id| *id != jogador.id)
                .collect()
        })
        .unwrap_or_default();
    let companheiros = companheiros_de(conn, &jogador.id);

    let de = |numero: i32| -> Option<Vinculo> {
        let id = por_numero.get(&i64::from(numero))?;
        // A ordem é a de FORÇA do vínculo, e ela importa: um nêmesis costuma ser rival
        // também, e anunciá-lo como "seu rival" seria descrever a relação por baixo.
        Some(if nemesis.as_deref() == Some(id.as_str()) {
            Vinculo::Nemesis
        } else if companheiros.iter().any(|c| c == id) {
            Vinculo::Companheiro
        } else if rivais.iter().any(|r| r == id) {
            Vinculo::Rival
        } else {
            Vinculo::Nenhum
        })
    };

    Contexto {
        frente: estado.frente.as_ref().and_then(|v| de(v.numero)),
        atras: estado.atras.as_ref().and_then(|v| de(v.numero)),
    }
}

/// Os outros ocupantes da equipe do jogador.
fn companheiros_de(conn: &Connection, jogador_id: &str) -> Vec<String> {
    let sql = "SELECT piloto_1_id, piloto_2_id FROM teams
               WHERE ativa = 1 AND (piloto_1_id = ?1 OR piloto_2_id = ?1)";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return Vec::new();
    };
    let Ok(linhas) = stmt.query_map([jogador_id], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
        ))
    }) else {
        return Vec::new();
    };
    linhas
        .filter_map(Result::ok)
        .flat_map(|(a, b)| [a, b])
        .flatten()
        .filter(|id| id != jogador_id)
        .collect()
}
