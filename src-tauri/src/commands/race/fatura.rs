//! A FATURA DA ETAPA que o jogador lê — a ponte entre `economia::fatura` e a tela.
//!
//! ## De onde vem cada número
//!
//! | o quê | quem manda |
//! |---|---|
//! | dinheiro de cada linha | o **ledger** (`team_finance_history`), que registra o que saiu do caixa |
//! | quantidade física e preço | `economia::evento` / `economia::temporada`, reconstruídos da etapa |
//! | conserto | o débito da batida, cobrado à parte em `race.rs`/`importacao.rs` |
//!
//! Os dois lados se encontram em [`FaturaVisivel::reancorar_despesa`]: as quantidades
//! físicas ficam como estão e o preço unitário absorve a diferença. Recalcular o dinheiro
//! aqui daria um segundo lugar onde a conta pode divergir do caixa — o defeito que este
//! redesign existe para remover.
//!
//! Sem linha de ledger para a rodada não há fatura: numa corrida de bloco especial o
//! caixa não se move, e uma fatura com números plausíveis para um dinheiro que nunca saiu
//! é pior que fatura nenhuma.

use super::*;

use crate::commands::career_types::StageInvoiceDto;
use crate::db::queries::teams::TeamFinanceHistoryEntry;
use crate::economia::fatura::{self, fatura_visivel, EntradaDaFatura};
use crate::economia::receita::ReceitaDaEtapa;
use crate::economia::temporada::{self, EquipeNaTemporada};

/// Quantas linhas do histórico financeiro varrer atrás da rodada pedida. Uma temporada
/// longa tem 14 corridas mais o fechamento; três temporadas de folga cobrem qualquer
/// tela que o jogador consiga reabrir.
const HISTORICO_A_VARRER: i64 = 60;

/// A fatura visível de UMA rodada de UMA equipe.
///
/// `reparo` é `(token_da_peça, custo)` — vem de `manutencao::damage_split`, já cobrado.
/// `folha_de_pilotos_anual` é a folha REAL dos contratos, quando o chamador a conhece.
///
/// Devolve `None` quando não há linha de ledger para `(season_number, round)`.
pub(crate) fn fatura_da_rodada(
    conn: &rusqlite::Connection,
    team: &Team,
    result: &RaceResult,
    track_id: u32,
    race_duration_min: i32,
    rounds_in_season: f64,
    season_number: i32,
    round: i32,
    reparo: &[(String, f64)],
    folha_de_pilotos_anual: Option<f64>,
) -> Option<StageInvoiceDto> {
    let ledger = linha_do_ledger(conn, &team.id, season_number, round)?;

    // ── A física: a mesma entrada que produziu o débito ──────────────────────────────
    let entrada = entrada_da_etapa(
        team,
        round_operation_context(result, &team.id, track_id),
        EtapaFisica::da_corrida(result, team, race_duration_min),
    );
    let etapa = crate::economia::evento::fatura_da_etapa(&entrada);
    let recorrentes = temporada::fatura_de_temporada(
        &team.categoria,
        team.classe.as_deref(),
        &EquipeNaTemporada {
            instalacoes: team.facilities,
        },
    );

    let mut visivel = fatura_visivel(&EntradaDaFatura {
        etapa: &etapa,
        temporada: &recorrentes,
        etapas_na_temporada: rounds_in_season.max(1.0),
        receita: receita_do_ledger(&ledger),
        // No modelo físico `technical_investment_cost` é a compra de peça e nada mais —
        // o termo abstrato de "investimento técnico" morreu na troca da despesa, e
        // `no_modelo_fisico_a_linha_tecnica_e_so_a_peca` trava isso. Nada a calcular aqui.
        peca_comprada: ledger.technical_investment_cost.max(0.0),
        folha_de_pilotos_anual,
    });

    // ── O dinheiro: o que de fato saiu do caixa, linha a linha ───────────────────────
    // O rodapé entra junto, ancorado em `custo_estrutura + salary_expense` — ver a nota
    // sobre a dupla contagem em [`custo_fixo_por_rodada_no_ledger`].
    if let Some(rodape) = visivel.custo_fixo_do_ano.as_mut() {
        let alvo_anual = custo_fixo_por_rodada_no_ledger(&ledger) * rounds_in_season.max(1.0);
        let atual = rodape.total_do_detalhe();
        if atual > 0.0 && alvo_anual > 0.0 {
            let fator = alvo_anual / atual;
            for d in rodape.detalhe.iter_mut() {
                d.preco_unitario *= fator;
            }
        }
    }

    let l = &ledger.linhas;
    visivel.reancorar_despesa(|chave| match chave {
        fatura::V_COMBUSTIVEL => Some(l.combustivel),
        fatura::V_PNEUS => Some(l.pneus),
        fatura::V_REVISAO_MECANICA => Some(l.desgaste_de_peca),
        fatura::V_FRETE => Some(l.frete),
        fatura::V_VIAGEM_E_ESTADIA => Some(l.viagem_e_estadia),
        fatura::V_INSCRICAO => Some(l.inscricao),
        fatura::V_DIARIAS => Some(l.diarias),
        // A peça já ENTROU pelo valor do ledger (`peca_comprada` acima) — reancorá-la
        // seria dividir um número por ele mesmo.
        _ => None,
    });

    Some(StageInvoiceDto::from_fatura(
        &visivel,
        rounds_in_season.max(1.0),
        reparo,
    ))
}

/// O custo fixo do ANO que o ledger de fato cobra, para a rodada dada.
///
/// **A armadilha da dupla contagem.** O rodapé sai de `economia::temporada`, cujo bloco
/// `Estrutura` INCLUI a folha de pilotos. A coluna `custo_estrutura` do ledger a EXCLUI,
/// porque o contrato de piloto sai do caixa como `salary_expense`. Ou seja:
///
/// ```text
/// custo_fixo_do_ano  ==  custo_estrutura + salary_expense     (por rodada)
/// ```
///
/// Somar as duas colunas do ledger como se fossem disjuntas paga piloto duas vezes na
/// tela; usar só `custo_estrutura` some com a folha. É a mesma armadilha por dois lados, e
/// [`o_rodape_nao_paga_piloto_duas_vezes`] existe para que ela falhe num teste em vez de
/// falhar na fatura do jogador.
fn custo_fixo_por_rodada_no_ledger(e: &TeamFinanceHistoryEntry) -> f64 {
    (e.linhas.estrutura + e.salary_expense).max(0.0)
}

/// A linha do ledger DESTA rodada. Nunca "a mais recente": numa corrida de fase especial
/// não existe linha, e a mais recente seria a da última rodada REGULAR — a fatura
/// mostraria, com números plausíveis, um dinheiro que nunca saiu.
fn linha_do_ledger(
    conn: &rusqlite::Connection,
    team_id: &str,
    season_number: i32,
    round: i32,
) -> Option<TeamFinanceHistoryEntry> {
    team_queries::get_team_finance_history_recent(conn, team_id, HISTORICO_A_VARRER)
        .ok()?
        .into_iter()
        .find(|e| e.season_number == season_number && e.round == round)
        .filter(|e| e.linhas.total_da_etapa() > 0.0)
}

/// Os quatro canais de receita da etapa, como o ledger os gravou.
///
/// `volta_mais_rapida` do modelo carrega o `result_bonus` — pontos, vitória, pódio e
/// top-5 somados. É por isso que o token de tela dele é `bonus_por_resultado` e não o
/// nome do campo: ver [`crate::economia::fatura::V_BONUS_POR_RESULTADO`].
///
/// `constructor_prize_income` fica de fora: é receita de FECHAMENTO de temporada, gravada
/// numa linha sintética própria, e não pertence à fatura de uma corrida.
fn receita_do_ledger(e: &TeamFinanceHistoryEntry) -> ReceitaDaEtapa {
    ReceitaDaEtapa {
        premio_de_corrida: e.partial_prize_income.max(0.0),
        volta_mais_rapida: e.result_bonus.max(0.0),
        patrocinio: e.sponsorship_income.max(0.0),
        bilheteria: e.gate_income.max(0.0),
    }
}

/// Reabre a fatura de uma corrida já disputada, a partir do save.
///
/// A chave é o `race_id` — o id da entrada de calendário, que é o que a tela do
/// pós-corrida já tem em mãos (`lastRaceId`). Resolver por `(categoria, rodada)` obrigaria
/// a supor que a temporada ativa ainda é a da corrida, e ela deixa de ser assim que o
/// jogador vira o ano.
///
/// Lê a tela salva do pós-corrida para recuperar o `RaceResult` (é dele que saem as voltas
/// completadas e o desgaste dos pneus) e remonta a fatura. `None` quando qualquer elo
/// falta — corrida nunca disputada, jogador ausente do resultado, rodada sem linha de
/// ledger (uma etapa de bloco especial não move o caixa).
pub fn fatura_da_rodada_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    race_id: &str,
) -> Result<Option<StageInvoiceDto>, String> {
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(career_id);
    let db_path = career_dir.join("career.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;

    let Some(entry) = calendar_queries::get_calendar_entry_by_id(&db.conn, race_id)
        .map_err(|e| format!("Falha ao buscar a etapa: {e}"))?
    else {
        return Ok(None);
    };
    let Some(season) = season_queries::get_season_by_id(&db.conn, &entry.season_id)
        .map_err(|e| format!("Falha ao buscar temporada: {e}"))?
    else {
        return Ok(None);
    };

    // O `RaceResult` mora na tela salva do pós-corrida — é o mesmo payload que a Home
    // reabre. Sem ele não dá para saber quantas voltas os carros da equipe fizeram.
    let tela = career_dir
        .join("race_screens")
        .join(format!("{}.json", entry.id));
    let Ok(bruto) = std::fs::read_to_string(&tela) else {
        return Ok(None);
    };
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(&bruto) else {
        return Ok(None);
    };
    let Some(result) = payload
        .get("race_result")
        .and_then(|v| serde_json::from_value::<RaceResult>(v.clone()).ok())
    else {
        return Ok(None);
    };

    // A equipe sai do RESULTADO, não do cadastro do piloto: a fatura é da equipe pela qual
    // ele correu NAQUELA etapa. Depois de uma transferência, ler a equipe atual mostraria a
    // fatura de quem não estava lá.
    let Some(team_id) = result
        .race_results
        .iter()
        .find(|r| r.is_jogador)
        .map(|r| r.team_id.clone())
        .filter(|id| !id.is_empty())
    else {
        return Ok(None);
    };
    let Ok(Some(team)) = team_queries::get_team_by_id(&db.conn, &team_id) else {
        return Ok(None);
    };

    // O conserto ficou gravado na fatura antiga da tela; reaproveitá-lo evita ressortear
    // um valor que já saiu do caixa.
    let reparo: Vec<(String, f64)> = payload
        .get("maintenance")
        .and_then(|m| m.get("items"))
        .and_then(|i| i.as_array())
        .map(|itens| {
            itens
                .iter()
                .filter(|it| it.get("group").and_then(|g| g.as_str()) == Some("reparo"))
                .filter_map(|it| {
                    Some((
                        it.get("key")?.as_str()?.to_string(),
                        it.get("cost")?.as_f64()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();

    let rodadas = get_category_config(&entry.categoria)
        .map(|c| f64::from(c.corridas_por_temporada))
        .unwrap_or(12.0);

    Ok(fatura_da_rodada(
        &db.conn,
        &team,
        &result,
        entry.track_id,
        entry.duracao_corrida_min,
        rodadas,
        season.numero as i32,
        entry.rodada,
        &reparo,
        folha_anual_da_equipe(&db.conn, &team.id),
    ))
}

/// A folha ANUAL real dos contratos de piloto da equipe. `None` quando não dá para
/// apurá-la — aí a fatura cai na referência de dupla mediana, que é o que o modelo puro
/// já faz e declara.
fn folha_anual_da_equipe(conn: &rusqlite::Connection, team_id: &str) -> Option<f64> {
    let contratos =
        crate::db::queries::contracts::get_active_regular_contracts_by_team(conn, team_id)
            .ok()
            .filter(|c| !c.is_empty())?;
    Some(contratos.iter().map(|c| c.salario_anual.max(0.0)).sum())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::placeholder_team_from_db;

    /// Banco mínimo com a tabela do ledger no formato ATUAL (as oito colunas de linha).
    fn conn_com_ledger() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("abrir banco");
        conn.execute_batch(
            "CREATE TABLE team_finance_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                team_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                round INTEGER NOT NULL,
                category TEXT NOT NULL DEFAULT '',
                sponsorship_income REAL NOT NULL DEFAULT 0.0,
                result_bonus REAL NOT NULL DEFAULT 0.0,
                partial_prize_income REAL NOT NULL DEFAULT 0.0,
                constructor_prize_income REAL NOT NULL DEFAULT 0.0,
                gate_income REAL NOT NULL DEFAULT 0.0,
                aid_income REAL NOT NULL DEFAULT 0.0,
                salary_expense REAL NOT NULL DEFAULT 0.0,
                custo_combustivel REAL NOT NULL DEFAULT 0.0,
                custo_pneus REAL NOT NULL DEFAULT 0.0,
                custo_desgaste_de_peca REAL NOT NULL DEFAULT 0.0,
                custo_frete REAL NOT NULL DEFAULT 0.0,
                custo_viagem_e_estadia REAL NOT NULL DEFAULT 0.0,
                custo_inscricao REAL NOT NULL DEFAULT 0.0,
                custo_diarias REAL NOT NULL DEFAULT 0.0,
                custo_estrutura REAL NOT NULL DEFAULT 0.0,
                event_operations_cost REAL NOT NULL DEFAULT 0.0,
                structural_maintenance_cost REAL NOT NULL DEFAULT 0.0,
                technical_investment_cost REAL NOT NULL DEFAULT 0.0,
                debt_service_cost REAL NOT NULL DEFAULT 0.0,
                income_total REAL NOT NULL DEFAULT 0.0,
                expenses_total REAL NOT NULL DEFAULT 0.0,
                net REAL NOT NULL DEFAULT 0.0,
                cash_balance REAL NOT NULL DEFAULT 0.0,
                debt_balance REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(team_id, season_number, round)
            );",
        )
        .expect("criar tabela");
        conn
    }

    /// As sete linhas de despesa e os quatro canais de receita desta rodada.
    const DESPESA: [f64; 7] = [
        1_600.0, 14_000.0, 1_200.0, 12_000.0, 9_000.0, 4_000.0, 22_000.0,
    ];
    const RECEITA: [f64; 4] = [120_000.0, 9_000.0, 210_000.0, 44_000.0];
    /// A peça comprada nesta rodada (`technical_investment_cost`).
    const PECA_COMPRADA: f64 = 83_000.0;

    fn grava(conn: &rusqlite::Connection, round: i32) {
        conn.execute(
            "INSERT INTO team_finance_history
                (team_id, season_number, round, category,
                 sponsorship_income, result_bonus, partial_prize_income, gate_income,
                 custo_combustivel, custo_pneus, custo_desgaste_de_peca, custo_frete,
                 custo_viagem_e_estadia, custo_inscricao, custo_diarias, custo_estrutura,
                 event_operations_cost, technical_investment_cost)
             VALUES ('T001', 1, ?1, 'gt3', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                round,
                RECEITA[2],
                RECEITA[1],
                RECEITA[0],
                RECEITA[3],
                DESPESA[0],
                DESPESA[1],
                DESPESA[2],
                DESPESA[3],
                DESPESA[4],
                DESPESA[5],
                DESPESA[6],
                183_000.0,
                DESPESA.iter().sum::<f64>(),
                PECA_COMPRADA,
            ],
        )
        .expect("gravar linha");
    }

    fn resultado_vazio() -> RaceResult {
        RaceResult {
            qualifying_results: Vec::new(),
            race_results: Vec::new(),
            pole_sitter_id: String::new(),
            winner_id: String::new(),
            fastest_lap_id: String::new(),
            total_laps: 20,
            weather: "dry".to_string(),
            track_name: "Teste".to_string(),
            total_incidents: 0,
            total_dnfs: 0,
            main_incident_count: 0,
            notable_incident_pilot_ids: Vec::new(),
            most_positions_gained_id: None,
            caution_segments: Vec::new(),
            applied_mechanicals: Vec::new(),
            safety_cars: Vec::new(),
            ordem_pre_safety_car: Vec::new(),
        }
    }

    fn monta(
        conn: &rusqlite::Connection,
        round: i32,
        reparo: &[(String, f64)],
    ) -> Option<StageInvoiceDto> {
        let team = placeholder_team_from_db(
            "T001".to_string(),
            "Equipe Teste".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        fatura_da_rodada(
            conn,
            &team,
            &resultado_vazio(),
            1,
            50,
            14.0,
            1,
            round,
            reparo,
            None,
        )
    }

    /// **A invariante da ponte.** Cada linha visível vale exatamente o que o ledger
    /// registrou — a fatura da tela não é uma segunda conta, é a mesma vista de perto.
    #[test]
    fn cada_linha_vale_o_que_saiu_do_caixa() {
        use crate::economia::fatura as v;
        let conn = conn_com_ledger();
        grava(&conn, 4);
        let f = monta(&conn, 4, &[]).expect("fatura existe");

        let esperado: [(&str, f64); 7] = [
            (v::V_COMBUSTIVEL, DESPESA[0]),
            (v::V_PNEUS, DESPESA[1]),
            (v::V_REVISAO_MECANICA, DESPESA[2]),
            (v::V_FRETE, DESPESA[3]),
            (v::V_VIAGEM_E_ESTADIA, DESPESA[4]),
            (v::V_INSCRICAO, DESPESA[5]),
            (v::V_DIARIAS, DESPESA[6]),
        ];
        for (chave, valor) in esperado {
            let linha = f
                .lines
                .iter()
                .find(|l| l.key == chave)
                .unwrap_or_else(|| panic!("linha {chave} ausente"));
            assert!(
                (linha.total - valor).abs() < 0.01,
                "{chave}: fatura {} ≠ ledger {valor}",
                linha.total
            );
        }
        assert!((f.expense_total - (DESPESA.iter().sum::<f64>() + PECA_COMPRADA)).abs() < 0.01);
        assert!((f.income_total - RECEITA.iter().sum::<f64>()).abs() < 0.01);
        assert!((f.result - (f.income_total - f.expense_total)).abs() < 1e-6);
    }

    /// O expandir continua fechando depois da reancoragem: quantidade × preço dá o total
    /// da linha. É a regra 2 do redesign atravessando a ponte.
    #[test]
    fn o_detalhe_continua_multiplicando_depois_de_reancorar() {
        let conn = conn_com_ledger();
        grava(&conn, 4);
        let f = monta(&conn, 4, &[]).expect("fatura existe");

        for linha in &f.lines {
            if linha.detail.is_empty() {
                continue;
            }
            let soma: f64 = linha.detail.iter().map(|d| d.total).sum();
            assert!(
                (linha.total - soma / linha.divisor).abs() < 0.01,
                "linha {} não fecha com o detalhe",
                linha.key
            );
            for d in &linha.detail {
                assert!(
                    (d.total - d.quantity * d.unit_price).abs() < 0.01,
                    "detalhe {} não multiplica",
                    d.key
                );
                assert!(d.quantity > 0.0, "detalhe {} sem quantidade física", d.key);
            }
        }
    }

    /// A decisão 10 atravessando a ponte: o custo fixo é RODAPÉ, fora do total da etapa.
    #[test]
    fn o_custo_fixo_e_rodape_e_nao_entra_no_total_da_etapa() {
        let conn = conn_com_ledger();
        grava(&conn, 4);
        let f = monta(&conn, 4, &[]).expect("fatura existe");

        assert!(f.fixed_cost.is_some(), "a GT3 tem custo fixo a declarar");
        assert!(f.fixed_cost_annual > 0.0);
        assert!(
            !f.lines
                .iter()
                .any(|l| l.key == crate::economia::fatura::V_CUSTO_FIXO_DO_ANO),
            "o custo fixo voltou a ser linha da etapa"
        );
        assert!((f.expense_total - (DESPESA.iter().sum::<f64>() + PECA_COMPRADA)).abs() < 0.01);
        // E o rodapé fala do ANO: a fatia por etapa é bem menor que o ano inteiro.
        let rodape = f.fixed_cost.as_ref().expect("rodapé");
        assert!(rodape.total < f.fixed_cost_annual / 10.0);
        assert!((rodape.divisor - 14.0).abs() < 1e-9);
    }

    /// A peça comprada é linha visível e soma na despesa da etapa — é dinheiro que saiu do
    /// caixa na mesma rodada, por `technical_investment_cost`.
    #[test]
    fn a_peca_comprada_entra_como_linha_e_soma_na_despesa() {
        let conn = conn_com_ledger();
        grava(&conn, 4);
        let f = monta(&conn, 4, &[]).expect("fatura existe");

        let peca = f
            .lines
            .iter()
            .find(|l| l.key == crate::economia::fatura::V_PECA_DE_REPOSICAO)
            .expect("linha da peça");
        assert!((peca.total - PECA_COMPRADA).abs() < 0.01);
        assert_eq!(peca.block, "corrida");
        assert!(
            !peca.expandable,
            "a compra não tem grandeza física para expandir"
        );
        assert!(
            (f.expense_total - (DESPESA.iter().sum::<f64>() + PECA_COMPRADA)).abs() < 0.01,
            "despesa {} ≠ etapa + peça",
            f.expense_total
        );
    }

    /// **A dupla contagem da folha de pilotos.** O rodapé do custo fixo inclui a folha de
    /// pilotos (é custo fixo do ano); a coluna `custo_estrutura` do ledger a exclui, porque
    /// o contrato sai como `salary_expense`. A identidade é
    /// `custo_fixo_por_etapa == custo_estrutura + salary_expense`.
    ///
    /// Somar as duas colunas achando que são disjuntas paga piloto duas vezes na tela;
    /// usar só uma some com a folha. Este teste trava as duas pontas.
    #[test]
    fn o_rodape_nao_paga_piloto_duas_vezes() {
        let conn = conn_com_ledger();
        grava(&conn, 4);
        let f = monta(&conn, 4, &[]).expect("fatura existe");
        let rodape = f.fixed_cost.as_ref().expect("rodapé");

        // Os valores gravados em `grava`: custo_estrutura 183.000, salary_expense 0.
        const ESTRUTURA: f64 = 183_000.0;
        const SALARIO: f64 = 0.0;
        assert!(
            (rodape.total - (ESTRUTURA + SALARIO)).abs() < 1.0,
            "fatia da etapa {} ≠ estrutura + salário {}",
            rodape.total,
            ESTRUTURA + SALARIO
        );
        // E o ano é a fatia vezes as etapas — nem mais, nem menos.
        assert!((f.fixed_cost_annual - (ESTRUTURA + SALARIO) * 14.0).abs() < 1.0);
        // O rodapé continua FORA da despesa da etapa.
        assert!((f.expense_total - (DESPESA.iter().sum::<f64>() + PECA_COMPRADA)).abs() < 0.01);
    }

    /// Rodada sem linha no ledger não vira fatura de números plausíveis — vira nada.
    /// É o caso da corrida de bloco especial, em que o caixa não se move.
    #[test]
    fn rodada_sem_ledger_nao_produz_fatura() {
        let conn = conn_com_ledger();
        grava(&conn, 4);
        assert!(monta(&conn, 4, &[]).is_some());
        assert!(
            monta(&conn, 7, &[]).is_none(),
            "inventou fatura para uma rodada que não moveu caixa"
        );
    }

    /// Diagnóstico do `None`: o ledger da carreira tem agregado sem as linhas? É a
    /// diferença entre "esta rodada não moveu caixa" e "este save é anterior às colunas
    /// de linha", e as duas produzem `None` pelo mesmo caminho.
    fn por_que_nao(base: &std::path::Path, career_id: &str) -> String {
        let db_path = base.join("saves").join(career_id).join("career.db");
        let Ok(db) = Database::open_existing(&db_path) else {
            return "banco não abriu".to_string();
        };
        // Nada de `unwrap_or` aqui: um erro de SQL engolido vira "0 linhas" e o
        // diagnóstico passa a mentir com a mesma cara de quem está certo.
        let num = |sql: &str| -> String {
            match db.conn.query_row(sql, [], |r| r.get::<_, f64>(0)) {
                Ok(v) => format!("{v:.0}"),
                Err(e) => format!("<{e}>"),
            }
        };
        format!(
            "linhas do ledger={} · agregado=${} · soma das colunas=${} · etapas no calendário={}",
            num("SELECT COUNT(*) FROM team_finance_history"),
            num("SELECT COALESCE(SUM(event_operations_cost), 0) FROM team_finance_history"),
            num(
                "SELECT COALESCE(SUM(custo_combustivel + custo_pneus + custo_desgaste_de_peca
                     + custo_frete + custo_viagem_e_estadia + custo_inscricao + custo_diarias), 0)
                 FROM team_finance_history"
            ),
            num("SELECT COUNT(*) FROM calendar"),
        )
    }

    /// **A ponte contra um SAVE de verdade.** É o que um clique manual na tela testaria:
    /// o save tem as colunas de linha do ledger? a tela salva desserializa? a fatura sai
    /// com número em vez de vazia?
    ///
    /// ```text
    /// LOOP_BASE_DIR="…/scratchpad/saverun" cargo test --lib \
    ///     primeiro_run_contra_um_save_real -- --ignored --nocapture
    /// ```
    ///
    /// Aponte para uma **cópia** do diretório de dados — abrir o banco roda migração, e
    /// migrar o save de campo de alguém para produzir um relatório não é uma troca justa.
    #[test]
    #[ignore = "roda contra um save real — precisa de LOOP_BASE_DIR"]
    fn primeiro_run_contra_um_save_real() {
        let Ok(base) = std::env::var("LOOP_BASE_DIR") else {
            println!("defina LOOP_BASE_DIR para uma CÓPIA do diretório de dados do app");
            return;
        };
        let base = std::path::PathBuf::from(base);
        let saves = base.join("saves");
        let Ok(carreiras) = std::fs::read_dir(&saves) else {
            println!("sem saves em {}", saves.display());
            return;
        };

        println!("\n=== A FATURA CONTRA SAVES REAIS ===\n");
        let mut vistas = 0u32;
        let mut com_fatura = 0u32;

        for carreira in carreiras.flatten() {
            let career_id = carreira.file_name().to_string_lossy().to_string();
            let telas = carreira.path().join("race_screens");
            let Ok(arquivos) = std::fs::read_dir(&telas) else {
                continue;
            };
            for arquivo in arquivos.flatten() {
                let nome = arquivo.file_name().to_string_lossy().to_string();
                let Some(race_id) = nome.strip_suffix(".json") else {
                    continue;
                };
                vistas += 1;
                match fatura_da_rodada_in_base_dir(&base, &career_id, race_id) {
                    Err(e) => println!("  {career_id}/{race_id}: ERRO — {e}"),
                    Ok(None) => {
                        println!(
                            "  {career_id}/{race_id}: SEM FATURA — {}",
                            por_que_nao(&base, &career_id)
                        );
                    }
                    Ok(Some(f)) => {
                        com_fatura += 1;
                        println!("\n── {career_id} · {race_id} ─────────────────────────────");
                        for l in &f.lines {
                            println!("  [{:<9}] {:<20} ${:>12.0}", l.block, l.key, l.total);
                            for d in &l.detail {
                                println!(
                                    "                {:<18} {:>10.1} {:<12} × ${:.2}",
                                    d.key, d.quantity, d.unit, d.unit_price
                                );
                            }
                        }
                        println!(
                            "  despesa ${:.0} · receita ${:.0} · saldo ${:.0}",
                            f.expense_total, f.income_total, f.result
                        );
                        if let Some(r) = &f.fixed_cost {
                            println!(
                                "  rodapé: custo fixo do ano ${:.0} (fatia da etapa ${:.0}, 1/{:.0})",
                                f.fixed_cost_annual, r.total, r.divisor
                            );
                        }
                        // As duas regras do redesign, num save de verdade.
                        for l in &f.lines {
                            assert!(l.total > 0.0, "linha {} zerada na tela", l.key);
                            for d in &l.detail {
                                assert!(
                                    (d.total - d.quantity * d.unit_price).abs() < 0.01,
                                    "detalhe {} não multiplica",
                                    d.key
                                );
                            }
                        }
                    }
                }
            }
        }
        println!("\n{com_fatura} faturas em {vistas} telas salvas.\n");
    }

    /// O conserto entra num bloco próprio e soma no total — é dinheiro que saiu — mas não
    /// ganha detalhe físico: a divisão por peça é curadoria de severidade, não medição.
    #[test]
    fn o_conserto_entra_em_bloco_proprio_e_sem_detalhe_inventado() {
        let conn = conn_com_ledger();
        grava(&conn, 4);
        let reparo = vec![
            ("carroceria".to_string(), 8_000.0),
            ("suspensao".to_string(), 3_000.0),
        ];
        let f = monta(&conn, 4, &reparo).expect("fatura existe");

        assert!((f.repair_total - 11_000.0).abs() < 0.01);
        assert!(
            (f.expense_total - (DESPESA.iter().sum::<f64>() + PECA_COMPRADA + 11_000.0)).abs()
                < 0.01
        );
        let linhas_de_reparo: Vec<_> = f.lines.iter().filter(|l| l.block == "reparo").collect();
        assert_eq!(linhas_de_reparo.len(), 2);
        for l in linhas_de_reparo {
            assert!(l.detail.is_empty(), "conserto não inventa grandeza física");
            assert!(!l.expandable);
        }
    }
}
