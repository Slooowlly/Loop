//! Curva de campeonato do piloto: onde ele terminou, temporada a temporada.
//!
//! A irma da curva de mercado ([`super::mercado`]) — mesma unidade de tempo,
//! mesma moldura no frontend, outra pergunta. La o eixo e dinheiro; aqui e
//! POSICAO, e a leitura muda de natureza junto: dinheiro cresce sem teto e por
//! isso pede escala logaritmica, posicao tem topo (P1) e tem CHAO (o ultimo do
//! grid), e e a distancia ate esses dois extremos que conta a historia.
//!
//! O chao e a razao de `grid` existir. Um P8 num campeonato de doze e um P8 num
//! de trinta sao carreiras opostas, e o numero solto nao distingue as duas:
//! sem o denominador, o grafico desenharia a subida de um piloto que so mudou
//! de categoria. O denominador vem arquivado (`total_pilotos` no snapshot da
//! temporada), entao e o tamanho que o campeonato tinha NAQUELE ano — e nao o
//! de hoje projetado para tras.

use super::*;

/// A classificacao de hoje, para o ponto da temporada em curso — que ainda nao
/// foi arquivada e por isso nao esta no historico.
pub(super) struct PosicaoDeHoje {
    pub posicao: i32,
    pub total: i32,
}

/// A curva de campeonato da carreira inteira.
///
/// Uma temporada por ponto, do arquivo, mais a temporada em curso montada com a
/// classificacao viva. Temporada em que ele nao correu por ninguem entra como
/// ponto SEM posicao e SEM equipe: e o ano fora do grid, que a moldura desenha
/// como faixa hachurada em vez de vao mudo — o mesmo vocabulario da curva de
/// mercado.
pub(super) fn build_driver_championship_curve(
    conn: &Connection,
    driver_id: &str,
    season_archive: &[CareerSeasonArchiveRow],
    season_numero: i32,
    season_ano: i32,
    categoria_atual: Option<&str>,
    equipe_atual: Option<&Team>,
    hoje: Option<PosicaoDeHoje>,
) -> Vec<DriverChampionshipCurvePoint> {
    let grids = grids_por_temporada(conn, driver_id, season_archive);
    let esperados = esperados_por_temporada(conn, season_archive);
    // Uma leitura so do plantel para a fita inteira, por id: nome de equipe nao e
    // chave, e uma consulta por ponto do grafico seria uma consulta por ano de
    // carreira. A falha e engolida — cor de chip e acabamento, e uma curva que
    // morre porque o plantel nao pode ser lido troca desenho por nada.
    let equipes: HashMap<String, (String, String)> =
        crate::db::queries::teams::get_all_teams(conn)
            .unwrap_or_default()
            .into_iter()
            .map(|equipe| (equipe.id, (equipe.nome, equipe.cor_primaria)))
            .collect();

    let mut pontos: Vec<DriverChampionshipCurvePoint> = season_archive
        .iter()
        .map(|temporada| {
            let equipe = temporada
                .equipe_id
                .as_deref()
                .and_then(|id| equipes.get(id));
            // Posicao sem NENHUMA corrida nao e classificacao: e a linha que o
            // arquivo escreve para quem passou a temporada sem assento. Desenhar
            // esse P1 fantasma poria um titulo no grafico de quem nao correu.
            let correu = temporada.corridas > 0;
            DriverChampionshipCurvePoint {
                season_number: temporada.season_number,
                ano: temporada.ano,
                categoria: timeline_division_key(
                    &temporada.categoria,
                    temporada.classe.as_deref(),
                )
                .filter(|_| correu)
                .unwrap_or_default(),
                equipe_nome: equipe.map(|(nome, _)| nome.clone()).filter(|_| correu),
                equipe_cor: equipe.map(|(_, cor)| cor.clone()).filter(|_| correu),
                posicao: temporada.posicao_campeonato.filter(|_| correu),
                grid: grids.get(&temporada.season_number).copied(),
                esperado: esperados
                    .get(&temporada.season_number)
                    .copied()
                    .filter(|_| correu),
                monomarca: categoria_monomarca(&temporada.categoria) && correu,
                pontos: Some(temporada.pontos).filter(|_| correu),
                vitorias: temporada.vitorias,
                podios: temporada.podios,
                corridas: temporada.corridas,
                titulo: archived_season_is_title(temporada),
                atual: false,
            }
        })
        .collect();

    // A temporada corrente ainda nao foi arquivada. Sem ela a curva pararia no
    // ano passado, e o ponto que o jogador mais quer ver — onde ele esta AGORA —
    // seria o unico que faltaria.
    if let Some(corrente) = pontos
        .iter_mut()
        .find(|ponto| ponto.season_number == season_numero)
    {
        corrente.atual = true;
    } else {
        pontos.push(DriverChampionshipCurvePoint {
            season_number: season_numero,
            ano: season_ano,
            categoria: categoria_atual.unwrap_or_default().to_string(),
            equipe_nome: equipe_atual.map(|equipe| equipe.nome.clone()),
            equipe_cor: equipe_atual.map(|equipe| equipe.cor_primaria.clone()),
            posicao: hoje.as_ref().map(|valor| valor.posicao),
            grid: hoje.as_ref().map(|valor| valor.total),
            // A temporada em curso tem o carro NA MAO — entao a expectativa dela
            // sai do modelo de verdade, o mesmo numero que a aba Temporada atual
            // imprime. So o passado precisa da reconstrucao pelo construtor.
            esperado: equipe_atual.and_then(|equipe| expected_position_for_team(conn, equipe)),
            pontos: None,
            vitorias: 0,
            podios: 0,
            corridas: 0,
            // A temporada em curso nao tem campeao ainda. Marcar o lider como
            // titulo aqui poria o trofeu numa disputa que nao acabou.
            titulo: false,
            atual: true,
            monomarca: categoria_atual.is_some_and(categoria_monomarca),
        });
    }

    pontos.sort_by_key(|ponto| ponto.season_number);
    pontos
}

/// A categoria daquele ano punha todo mundo no mesmo carro.
///
/// Sai da constante da categoria, e nao de uma lista repetida aqui: quem decide
/// o que e monomarca e a escada, e uma segunda lista divergiria dela na primeira
/// categoria nova. A chave e a categoria CRUA — a classe (`endurance:lmp2`) nao
/// entra, e nenhuma divisao de bloco especial e monomarca de qualquer forma.
///
/// Categoria desconhecida cai em `false`: sem saber, o grafico desenha a
/// expectativa como desenha em GT3, que e a leitura sem ressalva.
fn categoria_monomarca(categoria: &str) -> bool {
    crate::constants::categories::get_category(categoria)
        .is_some_and(|config| config.monomarca)
}

/// Onde o CARRO deveria ter terminado, em cada temporada arquivada do piloto.
///
/// A regra e a mesma da temporada corrente — [`expected_position_from_grid`]:
/// conta os assentos das equipes a frente e cai no meio da faixa de assentos da
/// propria equipe. O que muda e o ranking que alimenta a regra. Para o ano em
/// curso o ranking sai do carro efetivo; para um ano encerrado ele nao existe
/// mais (pacote tecnico de temporada passada nao e arquivado), e o que sobra e o
/// resultado do CONSTRUTOR daquele ano.
///
/// A troca de fonte tem um preco que vale dizer em voz alta: os pontos do
/// proprio piloto entram na classificacao de construtores da equipe dele. Um ano
/// muito acima da media empurra a equipe para cima e, com ela, a propria
/// expectativa — entao a distancia desenhada e CONSERVADORA, nunca inflada. Errar
/// para menos e o lado certo de errar num grafico que existe para dizer "ele
/// entregou mais do que o carro dava".
///
/// A posicao vira "desempenho" invertida (`-posicao`) so para reusar a funcao
/// sem duplicar a regra: quem terminou em P1 tem o maior valor, e o empate
/// tecnico da funcao casa exato porque posicao e inteiro.
fn esperados_por_temporada(
    conn: &Connection,
    season_archive: &[CareerSeasonArchiveRow],
) -> HashMap<i32, i32> {
    let mut esperados = HashMap::new();
    if season_archive.is_empty() {
        return esperados;
    }

    let temporadas: Vec<String> = season_archive
        .iter()
        .map(|linha| linha.season_number.to_string())
        .collect();
    let sql = format!(
        "SELECT season_number, categoria, classe, team_id, posicao_campeonato,
                (CASE WHEN COALESCE(piloto_1_id, '') <> '' THEN 1 ELSE 0 END)
              + (CASE WHEN COALESCE(piloto_2_id, '') <> '' THEN 1 ELSE 0 END)
         FROM team_season_archive
         WHERE season_number IN ({}) AND posicao_campeonato IS NOT NULL",
        temporadas.join(",")
    );

    // Cada campeonato (temporada + categoria + classe) junta as suas equipes.
    // Sem a classe no agrupamento, o LMP2 e o GT3 de uma mesma etapa de
    // endurance virariam um campeonato so e todo piloto de LMP2 "esperaria"
    // posicao de fundo.
    let mut grids: HashMap<(i32, String, Option<String>), Vec<(String, i32, i32)>> = HashMap::new();
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return esperados;
    };
    let linhas = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i32>(4)?,
            row.get::<_, i32>(5)?,
        ))
    });
    let Ok(linhas) = linhas else {
        return esperados;
    };
    for (temporada, categoria, classe, team_id, posicao, assentos) in linhas.flatten() {
        grids
            .entry((temporada, categoria, classe))
            .or_default()
            .push((team_id, posicao, assentos));
    }

    for linha in season_archive {
        let Some(minha_equipe) = linha.equipe_id.as_deref() else {
            continue;
        };
        let chave = (
            linha.season_number,
            linha.categoria.clone(),
            linha.classe.clone(),
        );
        let Some(grid) = grids.get(&chave) else {
            continue;
        };
        let Some((_, minha_posicao, _)) = grid.iter().find(|(id, _, _)| id == minha_equipe) else {
            continue;
        };

        let ranking: Vec<(f64, i32)> = grid
            .iter()
            .map(|(_, posicao, assentos)| (-(*posicao as f64), *assentos))
            .collect();
        if let Some(esperado) =
            expected_position_from_grid(-(*minha_posicao as f64), &ranking)
        {
            esperados.insert(linha.season_number, esperado);
        }
    }

    esperados
}

/// Quantos pilotos tinha o campeonato de cada temporada arquivada do piloto.
///
/// A fonte primaria e `total_pilotos` dentro do snapshot: e o tamanho do grid
/// que a propria classificacao daquele ano viu, ja resolvido por CLASSE nas
/// categorias multiclasse (o LMP2 nao disputa pontos com o GT3, entao o
/// denominador do piloto de LMP2 e o LMP2).
///
/// Save antigo nao guardava o campo. Ai vale a contagem de linhas arquivadas na
/// mesma temporada e categoria — que so e honesta onde nao ha classe, porque
/// somaria as duas classes num campeonato so. Sem nenhuma das duas, o ano fica
/// sem chao e a linha de fundo se parte ali: melhor um buraco declarado que um
/// denominador inventado.
///
/// Nao devolve `Result` de proposito: o chao do grid e CONTEXTO da curva, nao a
/// curva. Uma leitura que falha (schema enxuto, snapshot ilegivel) tem que
/// custar o contorno, e nunca o grafico inteiro.
fn grids_por_temporada(
    conn: &Connection,
    piloto_id: &str,
    season_archive: &[CareerSeasonArchiveRow],
) -> HashMap<i32, i32> {
    let mut grids: HashMap<i32, i32> = HashMap::new();
    if season_archive.is_empty() {
        return grids;
    }

    // Releitura do snapshot em vez de alargar `CareerSeasonArchiveRow`: aquela
    // linha e compartilhada com o ranking do mundo e com o dossie inteiro, e nao
    // deve crescer por causa de um grafico.
    if let Ok(mut stmt) = conn.prepare(
        "SELECT season_number, snapshot_json
         FROM driver_season_archive
         WHERE piloto_id = ?1",
    ) {
        let linhas = stmt.query_map(rusqlite::params![piloto_id], |row| {
            let season_number: i32 = row.get(0)?;
            let snapshot_json: String = row.get(1)?;
            let snapshot: serde_json::Value =
                serde_json::from_str(&snapshot_json).unwrap_or_default();
            let total = snapshot
                .get("total_pilotos")
                .and_then(serde_json::Value::as_i64)
                .filter(|valor| *valor > 0)
                .map(|valor| valor as i32);
            Ok((season_number, total))
        });
        if let Ok(linhas) = linhas {
            for (season_number, total) in linhas.flatten() {
                if let Some(total) = total {
                    grids.insert(season_number, total);
                }
            }
        }
    }

    for temporada in season_archive {
        if grids.contains_key(&temporada.season_number)
            || temporada.classe.is_some()
            || temporada.categoria.trim().is_empty()
        {
            continue;
        }
        let total: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM driver_season_archive
                 WHERE season_number = ?1 AND categoria = ?2",
                rusqlite::params![temporada.season_number, &temporada.categoria],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if total > 0 {
            grids.insert(temporada.season_number, total);
        }
    }

    grids
}
