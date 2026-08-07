//! Fama e atração de público medidas NO MUNDO RODANDO — não no harness isolado.
//!
//! O harness de `public_presence::medicao` compara os dois regimes de fama num mundo
//! sorteado à parte, com grid fechado. Este aqui lê o save de verdade no fim de cada
//! run do Monte Carlo, depois de o mundo ter passado por mercado, promoção,
//! rebaixamento, aposentadoria e falência. É a medição que vale para decidir se a
//! bilheteria tem sobre o que variar.
//!
//! O que sai: mídia por categoria (média, σ, mín, máx), a população nas seis faixas da
//! ficha e o espalhamento da ATRAÇÃO DE PÚBLICO entre a melhor e a pior equipe do grid
//! — que é a grandeza que a cota de bilheteria consome.

use std::collections::HashMap;
use std::path::Path;

use crate::db::connection::Database;

use super::super::Totals;

/// Mídia de cada piloto com assento, por categoria. Só quem corre: agente livre não
/// entra em grid nenhum e distorceria a distribuição da categoria.
pub(super) fn coletar_fama(db_path: &Path, t: &mut Totals) {
    let Ok(db) = Database::open_existing(db_path) else {
        return;
    };
    let mut stmt = match db.conn.prepare(
        "SELECT d.categoria_atual, d.midia
           FROM drivers d
          WHERE d.status != 'Aposentado'
            AND d.categoria_atual IS NOT NULL
            AND TRIM(d.categoria_atual) != ''",
    ) {
        Ok(s) => s,
        Err(_) => return,
    };
    let linhas = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    });
    if let Ok(linhas) = linhas {
        for (categoria, midia) in linhas.flatten() {
            t.fama_por_categoria.entry(categoria).or_default().push(midia);
        }
    }
}

/// Atração de público de cada equipe ativa, por categoria. Reusa exatamente a função
/// que a bilheteria consome em produção — se ela mudar, este número muda junto.
///
/// A pista do vínculo local é a da PRÓXIMA etapa de cada categoria no calendário; sem
/// calendário legível o termo local fica desligado (é aditivo, não distorce a ordem).
pub(super) fn coletar_atracao(db_path: &Path, t: &mut Totals) {
    let Ok(db) = Database::open_existing(db_path) else {
        return;
    };
    let Ok(equipes) = crate::db::queries::teams::get_all_teams(&db.conn) else {
        return;
    };

    // Quantas equipes ativas por categoria — é o divisor da normalização de posição.
    let mut por_categoria: HashMap<String, u32> = HashMap::new();
    for equipe in equipes.iter().filter(|e| e.ativa) {
        *por_categoria.entry(equipe.categoria.clone()).or_insert(0) += 1;
    }
    // A mesma classificação viva que a produção usa (pontos da temporada corrente).
    let ativas: Vec<crate::models::team::Team> =
        equipes.iter().filter(|e| e.ativa).cloned().collect();
    let posicoes = crate::public_presence::atracao::posicoes_por_pontos(&ativas);

    for equipe in equipes.iter().filter(|e| e.ativa) {
        let medias =
            crate::db::queries::teams::get_team_lineup_medias(&db.conn, &equipe.id).unwrap_or_default();
        let n = por_categoria
            .get(&equipe.categoria)
            .copied()
            .unwrap_or(1)
            .max(1);
        let atracao = crate::public_presence::atracao::team_audience_appeal_in_round(
            equipe,
            &medias,
            posicoes.get(&equipe.id).copied().unwrap_or(0),
            n,
            "",
        );
        t.atracao_por_categoria
            .entry(equipe.categoria.clone())
            .or_default()
            .push(atracao);
        // Presença é o que o termo de fama do patrocínio lê — guardada para o
        // relatório dizer quanto aquele canal passou a render.
        t.presenca_por_categoria
            .entry(equipe.categoria.clone())
            .or_default()
            .push(crate::public_presence::team::derive_team_public_presence(&medias));
    }
}

// ── Relatório ─────────────────────────────────────────────────────────────────

struct Resumo {
    media: f64,
    desvio: f64,
    minimo: f64,
    maximo: f64,
}

fn resumir(v: &[f64]) -> Resumo {
    let n = v.len().max(1) as f64;
    let media = v.iter().sum::<f64>() / n;
    Resumo {
        media,
        desvio: (v.iter().map(|x| (x - media).powi(2)).sum::<f64>() / n).sqrt(),
        minimo: v.iter().cloned().fold(f64::INFINITY, f64::min),
        maximo: v.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    }
}

/// Os 6 níveis da ficha. Nada aqui os altera — a quebra existe para medir para onde a
/// população se moveu, que é o que o mercado e as notícias enxergam.
const NIVEIS: [(&str, f64); 6] = [
    ("Anônimo", 15.0),
    ("Discreto", 30.0),
    ("Conhecido", 50.0),
    ("Nome forte", 70.0),
    ("Estrela", 87.0),
    ("Ídolo", 100.0),
];

/// Ordem da escada, para o relatório sair da base para o topo.
fn tier_de(categoria: &str) -> u8 {
    crate::constants::categories::get_category_config(categoria)
        .map(|c| c.tier)
        .unwrap_or(99)
}

fn categorias_ordenadas(mapa: &HashMap<String, Vec<f64>>) -> Vec<String> {
    let mut cats: Vec<String> = mapa.keys().cloned().collect();
    cats.sort_by_key(|c| (tier_de(c), c.clone()));
    cats
}

pub(super) fn imprimir(t: &Totals) {
    println!("\n┌─────────────────────────────────────────────────────────────┐");
    println!("│ FAMA NO MUNDO RODANDO (fim de cada run)");
    println!("└─────────────────────────────────────────────────────────────┘");

    if t.fama_por_categoria.is_empty() {
        println!("  (sem dados de fama coletados)");
        return;
    }

    println!(
        "\n{:<22} {:>7} {:>8} {:>8} {:>8} {:>8}",
        "Categoria", "n", "média", "σ", "mín", "máx"
    );
    println!("{}", "─".repeat(66));
    for cat in categorias_ordenadas(&t.fama_por_categoria) {
        let v = &t.fama_por_categoria[&cat];
        let r = resumir(v);
        println!(
            "{:<22} {:>7} {:>8.1} {:>8.1} {:>8.1} {:>8.1}",
            cat,
            v.len(),
            r.media,
            r.desvio,
            r.minimo,
            r.maximo
        );
    }
    println!("\nAlvo do redesenho: σ ≥ 15 dentro da categoria (o save medido dava ~8).");

    // ── População nas faixas da ficha ──
    let todas: Vec<f64> = t.fama_por_categoria.values().flatten().copied().collect();
    let mut contagem = [0usize; 6];
    for m in &todas {
        let i = NIVEIS
            .iter()
            .position(|(_, teto)| *m <= *teto)
            .unwrap_or(NIVEIS.len() - 1);
        contagem[i] += 1;
    }
    let total = todas.len().max(1);
    println!("\n{:<14} {:>10} {:>10}", "Nível da ficha", "pilotos", "%");
    println!("{}", "─".repeat(36));
    for (i, (nome, _)) in NIVEIS.iter().enumerate() {
        println!(
            "{:<14} {:>10} {:>9.1}%",
            nome,
            contagem[i],
            100.0 * contagem[i] as f64 / total as f64
        );
    }
    println!("Referência de hoje: Estrela 0,9% · Ídolo 0,9%.");

    // ── Atração de público: é ela que a bilheteria rateia ──
    if !t.atracao_por_categoria.is_empty() {
        println!(
            "\n{:<22} {:>8} {:>8} {:>8} {:>10} {:>12}",
            "Categoria (atração)", "média", "σ", "melhor", "pior", "espalhamento"
        );
        println!("{}", "─".repeat(74));
        for cat in categorias_ordenadas(&t.atracao_por_categoria) {
            let r = resumir(&t.atracao_por_categoria[&cat]);
            println!(
                "{:<22} {:>8.1} {:>8.1} {:>8.1} {:>10.1} {:>11.2}×",
                cat,
                r.media,
                r.desvio,
                r.maximo,
                r.minimo,
                if r.minimo > 0.01 {
                    r.maximo / r.minimo
                } else {
                    f64::INFINITY
                }
            );
        }
        println!("Critério 11 da receita: espalhamento da bilheteria ≥ 2,5×.");
    }

    // ── O que o termo de fama do patrocínio passou a ler ──
    if !t.presenca_por_categoria.is_empty() {
        println!(
            "\n{:<22} {:>10} {:>10}",
            "Categoria (presença)", "média", "σ"
        );
        println!("{}", "─".repeat(44));
        for cat in categorias_ordenadas(&t.presenca_por_categoria) {
            let r = resumir(&t.presenca_por_categoria[&cat]);
            println!("{:<22} {:>10.1} {:>10.1}", cat, r.media, r.desvio);
        }
        println!(
            "É esta a entrada do termo `presença × base × FAME_SPONSORSHIP_COEFF`\n\
             (não recalibrado nesta rodada — o coeficiente muda de dono na receita)."
        );
    }
}
