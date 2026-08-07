//! Manutencao e reparo do carro depois da corrida: quebra do dano por area, custo, e a mensagem que a UI mostra ao jogador.

use super::*;

/// Um item da fatura do fim de semana (gasolina, frete, carroceria...). Só itens com
/// custo > 0 entram na lista.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceItem {
    pub key: String,
    pub label: String,
    pub cost: f64,
    /// Bloco ao qual o item pertence (`carro` / `logistica` / `equipe` / `reparo`).
    /// `default` cobre as telas salvas antes dos blocos existirem.
    #[serde(default)]
    pub group: String,
}

/// Grupo dos itens de conserto — o único bloco da fatura que responde a como o jogador
/// pilotou, e por isso o único que a tela destaca.
pub(super) const GROUP_REPAIR: &str = "reparo";

/// Fatura do fim de semana. Não é informativa nem paralela: as linhas de operação SÃO a
/// decomposição do `event_operations_cost` que saiu do caixa em `apply_round_cashflow`,
/// normalizadas ao valor exatamente debitado; o bloco de conserto soma o débito extra da
/// batida (cobrado em `race.rs`/`importacao.rs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaintenanceBreakdown {
    pub total: f64,
    pub items: Vec<MaintenanceItem>,
    /// Só o que a batida cobrou. 0 num fim de semana limpo — a tela usa isso para
    /// decidir se pinta o total de alerta ou de custo de rotina.
    #[serde(default)]
    pub repair_total: f64,
}

/// Como o custo do conserto se divide entre os itens mecânicos, por severidade. NÃO
/// simula o dano real — só fatia o valor total entre componentes plausíveis. Frações
/// somam ~1.0.
/// Divisão do custo de conserto por peça, como (key, proporção). A `key` é o token
/// da peça; o rótulo de display resolve em `maintenance_label`.
pub(super) fn damage_split(severity: &str) -> Vec<(&'static str, f64)> {
    match severity {
        "moderado" => vec![("carroceria", 0.70), ("suspensao", 0.30)],
        "grave" => vec![
            ("carroceria", 0.50),
            ("suspensao", 0.25),
            ("freio", 0.15),
            ("embreagem", 0.10),
        ],
        "destruído" => vec![
            ("carroceria", 0.40),
            ("suspensao", 0.20),
            ("motor", 0.15),
            ("freio", 0.12),
            ("cambio", 0.08),
            ("embreagem", 0.05),
        ],
        "catastrófico" => vec![
            ("carroceria", 0.32),
            ("motor", 0.22),
            ("suspensao", 0.18),
            ("cambio", 0.12),
            ("freio", 0.08),
            ("embreagem", 0.08),
        ],
        _ => vec![],
    }
}

/// Rótulo de display de um item de manutenção (i18n) a partir da `key`.
pub(super) fn maintenance_label(key: &str) -> String {
    let full = format!("maintenance.{key}");
    rust_i18n::t!(&full).to_string()
}

/// Monta a fatura do fim de semana do time do jogador.
///
/// As linhas de operação (carro / logística / equipe) são a MESMA conta que produziu o
/// `event_operations_cost` desta rodada — hoje a fatura física de [`super::despesa`]. Para
/// não depender de os dois cálculos coincidirem, o total real é lido do histórico financeiro
/// já gravado e as linhas são normalizadas até somarem exatamente ele. Se houve batida,
/// o conserto entra num bloco à parte, porque é o único débito que responde à pilotagem.
///
/// Os rótulos continuam sendo os de `maintenance.*`: as chaves do modelo físico
/// (`combustivel`, `revisao`) descrevem as mesmas coisas que `gasolina` e `pecas`
/// descreviam, e `despesa::rotulo_da_linha` faz a tradução. O que mudou é que agora o
/// número atrás do rótulo É a conta que o rótulo nomeia.
///
/// A âncora é a linha de `(season_number, round)` DESTA rodada, nunca "a mais recente":
/// numa corrida de fase especial não existe linha (o caixa não se move) e a linha mais
/// recente seria a da última rodada REGULAR — a fatura mostraria, com números plausíveis,
/// um dinheiro que nunca saiu. Sem linha desta rodada, o bloco de operação simplesmente
/// não é emitido; só o conserto, que é debitado à parte.
pub(crate) fn compute_maintenance_breakdown(
    conn: &rusqlite::Connection,
    team: &Team,
    result: &RaceResult,
    track_id: u32,
    // Duração REAL da prova (`CalendarEntry::duracao_corrida_min`) — a mesma que dimensionou
    // o débito. `0` cai na duração de referência da divisão.
    race_duration_min: i32,
    rounds_in_season: f64,
    economic_health: GlobalEconomicHealth,
    repair_cost: f64,
    repair_severity: &str,
    season_number: i32,
    round: i32,
) -> MaintenanceBreakdown {
    let debitado =
        team_queries::get_team_round_operations_cost(conn, &team.id, season_number, round)
            .ok()
            .flatten()
            .filter(|v| *v > 0.0);

    let mut items: Vec<MaintenanceItem> = match debitado {
        Some(debitado) => {
            let lines = despesa_da_rodada(
                team,
                round_operating_base(&team.categoria, team.classe.as_deref(), rounds_in_season),
                economy_cost_modifier(economic_health),
                rounds_in_season,
                round_operation_context(result, &team.id, track_id),
                EtapaFisica::da_corrida(result, team, race_duration_min),
            )
            .linhas;
            let bruto: f64 = lines.iter().map(|l| l.cost).sum();
            let ajuste = if bruto > 0.0 { debitado / bruto } else { 0.0 };
            lines
                .iter()
                .map(|l| MaintenanceItem {
                    key: l.key.to_string(),
                    label: maintenance_label(l.key),
                    cost: (l.cost * ajuste).round(),
                    group: l.group.to_string(),
                })
                .filter(|i| i.cost > 0.0)
                .collect()
        }
        None => Vec::new(),
    };

    if repair_cost > 0.0 {
        for (key, prop) in damage_split(repair_severity) {
            let cost = (repair_cost * prop).round();
            if cost > 0.0 {
                items.push(MaintenanceItem {
                    key: key.into(),
                    label: maintenance_label(key),
                    cost,
                    group: GROUP_REPAIR.into(),
                });
            }
        }
    }

    let total = items.iter().map(|i| i.cost).sum();
    MaintenanceBreakdown {
        total,
        items,
        repair_total: repair_cost.max(0.0).round(),
    }
}

/// Fração do custo de OPERAÇÃO da categoria que cada severidade de batida cobra em
/// conserto. A batida já chega rebaixada se o carro cruzou a linha (race_monitor).
pub(super) fn repair_cost_fraction(severity: &str) -> f64 {
    match severity {
        "moderado" => 0.03,
        "grave" => 0.10,
        "destruído" => 0.25,
        "catastrófico" => 0.45,
        _ => 0.0, // leve / nenhum: sem cobrança
    }
}

/// Custo de conserto do carro do jogador: fração(severidade) × custo de operação da
/// categoria × fator do carro × margem aleatória (±35%). A margem dá variação dentro
/// do tier — duas batidas "graves" não custam idêntico. Calculado UMA vez e persistido
/// (caixa + summary + fatura), então não muda ao reabrir a tela. SÓ jogador.
pub(super) fn compute_repair_cost(
    severity: &str,
    category: &str,
    classe: Option<&str>,
    car_performance: f64,
    rng: &mut impl rand::Rng,
) -> f64 {
    let frac = repair_cost_fraction(severity);
    if frac <= 0.0 {
        return 0.0;
    }
    let operating = category_finance_scale_for(category, classe).operating_cost_midpoint();
    let car_factor =
        1.0 + crate::simulation::math::normalize_car_performance(car_performance) / 100.0 * 0.5;
    let variance = 0.65 + rng.gen::<f64>() * 0.70; // 0.65..1.35
    (frac * operating * car_factor * variance).round()
}

/// Formata um valor em dólar no estilo en-US: "$18,500" (negativo: "-$18,500").
pub(super) fn format_brl(value: f64) -> String {
    let n = value.round() as i64;
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{}${}", if n < 0 { "-" } else { "" }, out)
}

/// Lê/incrementa a contagem de batidas COBRÁVEIS do jogador na temporada atual,
/// persistida em `iracing_repair_count.json` no diretório do save. Reinicia a cada
/// temporada. Devolve a contagem JÁ incluindo esta batida.
pub(super) fn bump_repair_count(career_dir: &Path, season_number: i32) -> i32 {
    let path = career_dir.join("iracing_repair_count.json");
    let stored: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);
    let same_season = stored.get("season").and_then(|v| v.as_i64()) == Some(season_number as i64);
    let prev = if same_season {
        stored.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
    } else {
        0
    };
    let count = (prev + 1) as i32;
    let _ = std::fs::write(
        &path,
        serde_json::json!({ "season": season_number, "count": count }).to_string(),
    );
    count
}

/// Frase pronta do pop-up de conserto, variando por VALOR (severidade) e por
/// QUANTAS vezes já aconteceu nesta temporada. Sorteio dentro da célula.
pub(super) fn pick_repair_message(
    rng: &mut impl rand::Rng,
    severity: &str,
    valor: &str,
    count: i32,
) -> String {
    const FIRST: &[&str] = &[
        "Acontece. O pessoal do box já está no conserto — {valor}. Cabeça erguida pra próxima.",
        "É assim que se aprende. {valor} no reparo; a gente absorve dessa vez.",
        "Bateu, mas trouxe lições. Custou {valor} — na próxima a gente traz inteiro.",
    ];
    const REPEAT: &[&str] = &[
        "De novo. O conserto foi {valor}. Precisamos trazer esse carro pra casa inteiro.",
        "Já são {n}. Mais {valor} que saíram do caixa — isso começa a pesar.",
        "Segunda chamada. {valor}. O orçamento não é infinito, combinado?",
    ];
    const CHRONIC: &[&str] = &[
        "Outra?! {valor}. Já são {n} carros destruídos — não pode virar hábito.",
        "{n} batidas nesta temporada. {valor} dessa. O conselho está perguntando o que acontece no cockpit.",
        "{valor}. Se continuar assim, vamos ter uma conversa séria sobre o seu lugar aqui.",
    ];
    const LIGHT: &[&str] = &[
        "Nada grave — {valor} e o carro tá novo de novo.",
        "Só um susto. {valor} no conserto, seguimos.",
    ];
    const CATASTROPHIC: &[&str] = &[
        "O carro voltou em pedaços. {valor}. Isso vai doer no orçamento o resto da temporada.",
        "Perda quase total. {valor}. Precisamos de um fim de semana limpo, urgente.",
        "{valor} num capote só. O caixa sentiu — e o chefe também.",
    ];

    let pool: &[&str] = if severity == "catastrófico" {
        CATASTROPHIC
    } else if severity == "moderado" && count <= 1 {
        LIGHT
    } else if count <= 1 {
        FIRST
    } else if count <= 3 {
        REPEAT
    } else {
        CHRONIC
    };
    let idx = rng.gen_range(0..pool.len());
    pool[idx]
        .replace("{valor}", valor)
        .replace("{n}", &count.to_string())
}

/// Média das chegadas recentes (do JSON `ultimos_resultados`) — menor = melhor
/// forma. `None` se não houver histórico.
pub(super) fn recent_avg_finish(v: &serde_json::Value) -> Option<f64> {
    let positions: Vec<f64> = v
        .as_array()?
        .iter()
        .filter_map(|e| e.get("position").and_then(|p| p.as_i64()))
        .filter(|p| *p > 0)
        .map(|p| p as f64)
        .collect();
    if positions.is_empty() {
        None
    } else {
        Some(positions.iter().sum::<f64>() / positions.len() as f64)
    }
}
