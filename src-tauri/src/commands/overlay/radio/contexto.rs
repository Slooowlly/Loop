//! Os fatos que a fala de quebra precisa saber sobre o piloto que quebrou.
//!
//! [`quebra::montar`](crate::engenheiro::quebra::montar) decide a redação a partir de sete
//! booleanos e um delta de pontos. Este módulo é quem os preenche a partir do banco — e é
//! deliberadamente a única parte do caminho que sabe o que é uma tabela SQL.
//!
//! ## Por que carregar tudo de uma vez
//!
//! O feed do rádio é lido em polling durante a corrida, e uma quebra traz de uma a algumas
//! linhas por vez. Consultar rivalidade, nêmesis, equipe e campeonato POR LINHA daria quatro
//! consultas por quebra, num caminho que roda a cada segundo. [`Mundo::carregar`] paga isso
//! uma vez por chamada do feed e responde o resto em memória.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::engenheiro::quebra::Contexto;

/// Tudo o que o rádio precisa saber do save para redigir qualquer quebra desta corrida.
pub(crate) struct Mundo {
    /// Pontos da temporada corrente por piloto, na categoria do jogador.
    pontos: HashMap<String, f64>,
    /// Quem lidera. `None` quando ninguém pontuou ainda — e aí ninguém "lidera o campeonato",
    /// o que é diferente de estar em primeiro por desempate de id.
    lider: Option<String>,
    pontos_jogador: Option<f64>,
    nemesis: Option<String>,
    rivais: HashSet<String>,
    /// Companheiros de equipe do jogador (os outros ocupantes do mesmo `equipe_atual_id`).
    companheiros: HashSet<String>,
    /// Piloto → (nome da equipe no catálogo, assento 1 ou 2).
    equipe: HashMap<String, (String, u8)>,
}

impl Mundo {
    pub(crate) fn carregar(conn: &Connection) -> Mundo {
        let jogador = crate::db::queries::drivers::get_player_driver(conn).ok();
        let jogador_id = jogador.as_ref().map(|d| d.id.clone());
        let categoria = jogador.as_ref().and_then(|d| d.categoria_atual.clone());

        let pontos = categoria
            .as_deref()
            .map(|c| pontos_da_temporada(conn, c))
            .unwrap_or_default();
        // Lidera quem tem MAIS pontos, e só se houver ponto. Empate no topo não tem líder:
        // dizer "que lidera o campeonato" para um de dois empatados é dizer algo falso sobre
        // o outro.
        let melhor = pontos.values().copied().fold(f64::NEG_INFINITY, f64::max);
        let no_topo: Vec<&String> = pontos
            .iter()
            .filter(|(_, p)| **p == melhor)
            .map(|(id, _)| id)
            .collect();
        let lider = (melhor > 0.0 && no_topo.len() == 1).then(|| no_topo[0].clone());

        let pontos_jogador = jogador_id.as_deref().and_then(|id| pontos.get(id).copied());

        let nemesis = crate::db::queries::player_nemesis::get_current_nemesis(conn)
            .ok()
            .flatten();
        let rivais = jogador_id
            .as_deref()
            .and_then(|id| crate::db::queries::rivalries::get_rivalries_for_pilot(conn, id).ok())
            .map(|rs| {
                rs.iter()
                    .flat_map(|r| [r.piloto1_id.clone(), r.piloto2_id.clone()])
                    .filter(|id| Some(id.as_str()) != jogador_id.as_deref())
                    .collect()
            })
            .unwrap_or_default();

        let equipe = equipes_por_piloto(conn);
        let companheiros = jogador_id
            .as_deref()
            .and_then(|id| equipe.get(id))
            .map(|(nome, _)| {
                equipe
                    .iter()
                    .filter(|(pid, (n, _))| n == nome && Some(pid.as_str()) != jogador_id.as_deref())
                    .map(|(pid, _)| pid.clone())
                    .collect()
            })
            .unwrap_or_default();

        Mundo {
            pontos,
            lider,
            pontos_jogador,
            nemesis,
            rivais,
            companheiros,
            equipe,
        }
    }

    /// Monta o contexto de um piloto que quebrou.
    pub(crate) fn contexto(
        &self,
        piloto_id: Option<&str>,
        nome_completo: &str,
        peca: &str,
        severidade: &str,
        variante: usize,
        abandonos_ate_aqui: u32,
    ) -> Contexto {
        // Sem id resolvido (carro que não bateu com nosso elenco), o piloto não tem vínculo
        // nenhum — nem sequer equipe. A fala cai na forma mais neutra que existir, e é assim
        // que tem que ser: inventar vínculo por nome seria pior que não falar.
        let (equipe, assento) = piloto_id
            .and_then(|id| self.equipe.get(id))
            .map(|(nome, a)| (Some(nome.clone()), *a))
            .unwrap_or((None, 2));
        let delta_pontos = match (piloto_id.and_then(|id| self.pontos.get(id)), self.pontos_jogador)
        {
            (Some(dele), Some(meu)) => Some((dele - meu).round() as i32),
            _ => None,
        };
        Contexto {
            nome_completo: nome_completo.to_string(),
            equipe,
            assento,
            e_nemesis: piloto_id.is_some_and(|id| self.nemesis.as_deref() == Some(id)),
            e_rival: piloto_id.is_some_and(|id| self.rivais.contains(id)),
            e_companheiro: piloto_id.is_some_and(|id| self.companheiros.contains(id)),
            lidera_campeonato: piloto_id.is_some_and(|id| self.lider.as_deref() == Some(id)),
            delta_pontos,
            peca: peca.to_string(),
            severidade: severidade.to_string(),
            variante,
            abandonos_ate_aqui,
        }
    }
}

/// Pontos por piloto na temporada em andamento, na categoria dada. Erro de SQL devolve mapa
/// vazio: sem tabela, o rádio perde o enquadramento de campeonato e mantém todo o resto.
fn pontos_da_temporada(conn: &Connection, categoria: &str) -> HashMap<String, f64> {
    let sql = "SELECT r.piloto_id, COALESCE(SUM(r.pontos), 0.0)
               FROM race_results r
               INNER JOIN calendar c ON c.id = r.race_id
               INNER JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
               WHERE s.status = 'EmAndamento' AND c.categoria = ?1
               GROUP BY r.piloto_id";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return HashMap::new();
    };
    let Ok(linhas) = stmt.query_map([categoria], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    }) else {
        return HashMap::new();
    };
    linhas.filter_map(Result::ok).collect()
}

/// Piloto → (nome da equipe, assento). O assento sai da coluna em que o id aparece.
fn equipes_por_piloto(conn: &Connection) -> HashMap<String, (String, u8)> {
    let sql = "SELECT nome, piloto_1_id, piloto_2_id FROM teams WHERE ativa = 1";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return HashMap::new();
    };
    let Ok(linhas) = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    }) else {
        return HashMap::new();
    };
    let mut mapa = HashMap::new();
    for (nome, p1, p2) in linhas.filter_map(Result::ok) {
        if let Some(id) = p1 {
            mapa.insert(id, (nome.clone(), 1u8));
        }
        if let Some(id) = p2 {
            mapa.insert(id, (nome, 2u8));
        }
    }
    mapa
}
