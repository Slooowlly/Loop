//! **A troca da despesa**: a costura entre a corrida e [`crate::economia`].
//!
//! Até aqui o módulo `economia` foi construído AO LADO do modelo antigo e ninguém o
//! chamava. Este arquivo é o ponto em que a rodada passa a debitar `quantidade × preço`
//! em vez de `peso × orçamento`.
//!
//! ## Por que a troca importa mesmo com a âncora já trocada
//!
//! O NÍVEL já vinha do modelo físico: desde a troca da âncora de fluxo,
//! `round_operating_base` é o custo operacional anual de referência dividido pelas
//! rodadas. O que continuava velho era a FORMA — e a forma tinha um erro de escala
//! embutido. Somados, os pesos da fatura antiga cobram por rodada:
//!
//! | linha | peso |
//! |---|---:|
//! | operação (a fatura de pesos, hoje em `tests::despesa_legada`) | ~0,62 × base |
//! | manutenção estrutural | ~0,38 × base (0,18 + 0,0025·eng + 0,0015·crew) |
//! | investimento técnico | 0,16 × base |
//!
//! São ~1,16 × base de despesa técnica por rodada, mais a folha REAL dos contratos e mais
//! a peça — contra uma âncora que diz que o ano inteiro, folha de piloto incluída, custa
//! 1,00 × base × rodadas. Daí a despesa medida ficar em 1,65–2,27× o custo operacional
//! declarado, e daí todo coeficiente de `economia::receita::ParametrosDeReceita` carregar
//! um fator ~1,96 embutido. **Enquanto a despesa não trocasse, não havia alvo honesto
//! contra o que calibrar receita.**
//!
//! ## O que o modelo físico debita
//!
//! | linha do ledger | de onde vem agora |
//! |---|---|
//! | `event_operations_cost` | [`crate::economia::evento::fatura_da_etapa`] — a etapa concreta |
//! | `structural_maintenance_cost` | [`crate::economia::temporada::fatura_de_temporada`] ÷ rodadas, **sem** a folha de pilotos |
//! | `technical_investment_cost` | só a peça REAL (`car_maintenance_cost`) |
//!
//! Três decisões que valem ser ditas em voz alta:
//!
//! - **A folha técnica passa a existir.** Mecânicos e engenheiros nunca foram debitados; o
//!   jogo só pagava piloto. Agora a linha `folha_tecnica` dos recorrentes sai do caixa toda
//!   rodada, diluída pelo calendário.
//! - **A folha de PILOTOS dos recorrentes é descontada.** Ela é o valor de referência de
//!   uma dupla mediana e existe só para a âncora cobrir o mesmo escopo que a tabela que ela
//!   substituiu. O gasto real com piloto é o contrato, e ele já é debitado como
//!   `salary_expense`. Somar os dois seria pagar piloto duas vezes.
//! - **O termo `0,16 × base` de investimento técnico morre.** Ele não tinha referente
//!   físico nenhum: a peça de reposição REAL já é decidida pelo cérebro de manutenção e
//!   debitada crua, e a melhoria de estrutura é [`crate::economia::desenvolvimento`], que
//!   é outro botão. O que sobra em `technical_investment_cost` é a peça, e só.
//!
//! ## A peça comprada precisa de linha própria — medido
//!
//! A fatura visível da etapa é `event_operations_cost`, as sete linhas físicas. A COMPRA de
//! peça sai do caixa na mesma rodada por `technical_investment_cost` e não aparece em lugar
//! nenhum. A invariante "fatura mostrada = dinheiro debitado" vale para o bloco de operação
//! e é falsa para a saída de caixa da etapa.
//!
//! `relatorio_peso_da_peca_contra_a_fatura_visivel` mede as duas coisas que decidem se isso
//! é rodapé ou omissão: com que frequência a equipe compra peça, e quanto a compra pesa
//! contra a fatura daquela rodada. Cinco temporadas, carro herdado entre elas, equipe no
//! **caixa de referência de 6 meses** (a primeira versão desta medição rodou a 16 e 24 meses
//! porque `spending_power` estava quebrado e nada era comprado abaixo de 15,2):
//!
//! | | rodadas com compra | peça ÷ fatura, média | pior rodada |
//! |---|---:|---:|---:|
//! | Rookie | 96% | 30% | 44% |
//! | Amador | 98% | 40% | 58% |
//! | BMW M2 | 100% | 68% | 76% |
//! | Production | 98% | 73–81% | 88% |
//! | GT4 | 98% | 89% | 110% |
//! | GT3 | 96% | **109%** | **135%** |
//! | LMP2 | 98% | 119% | 136% |
//! | Endurance | 97% | 43–65% | 74% |
//!
//! **Frequente e grande, não raro e pequeno.** Do GT3 para cima a peça é, em média, MAIOR
//! que a fatura visível inteira, e mesmo no degrau de baixo ela é um terço dela. Um rodapé
//! para o maior item da página não é uma ressalva, é letra miúda. A peça ganha linha
//! visível, e a fatura passa a ter nove.
//!
//! Duas ressalvas que quem ler a tabela como teto vai errar:
//!
//! - **É piso.** Só o plano do cérebro entra; a troca FORÇADA por quebra grave, por DNF e
//!   por contato destrutivo é debitada mesmo sem caixa e não está aqui.
//! - **Sobe com o caixa.** A 16 meses — onde o placar da economia mediu boa parte do mundo
//!   em regime — a GT3 vai a 163% de média e 352% na pior rodada.
//!
//! Duas consequências para quem monta a tela:
//!
//! - **Os dois nomes não podem ser parecidos.** `desgaste_de_peca` é a revisão mecânica
//!   amortizada por quilômetro (o token `revisao` do modelo) e existe em toda etapa;
//!   a linha nova é a TROCA — a peça que o cérebro comprou nesta rodada. São despesas
//!   diferentes e o jogador vai somar as duas se os rótulos convergirem.
//! - **O dado já está pronto.** No modelo físico `technical_investment_cost` é a compra e
//!   nada mais (`no_modelo_fisico_a_linha_tecnica_e_so_a_peca` trava isso), já está gravado
//!   no ledger e já sai no `TeamFinanceRound`. A nona linha não precisa de cálculo novo.
//!
//! ## O modelo antigo saiu daqui
//!
//! Não existe mais interruptor: esta é a única conta que a rodada debita. O modelo velho
//! (`finance::operacao` + os dois termos achatados) virou dado histórico congelado em
//! [`super::tests::despesa_legada`], que é o que mantém o comparador com dois lados para
//! comparar sem que nada disso continue no caminho do jogador.

use super::*;

use crate::economia::ancora;
use crate::economia::evento::{self, fatura_da_etapa};
use crate::economia::temporada::{self, EquipeNaTemporada};
use crate::economia::tipos::{
    DesempenhoNaEtapa, EntradaDaEtapa, EquipeNaEtapa, PistaDaEtapa, DISTANCIA_CASA_KM,
    DISTANCIA_CONTINENTAL_KM, DISTANCIA_INTERCONTINENTAL_KM,
};
use crate::finance::cashflow::LinhasDaDespesa;

/// Bloco da fatura ao qual uma linha pertence. Vira agrupamento visual na tela do
/// pós-corrida — token, não texto de UI.
pub(crate) const GROUP_CAR: &str = "carro";
pub(crate) const GROUP_LOGISTICS: &str = "logistica";
pub(crate) const GROUP_CREW: &str = "equipe";

/// Uma linha da fatura de operação da rodada.
#[derive(Debug, Clone)]
pub(crate) struct OperationLine {
    /// Token da linha (`gasolina`, `frete`, ...). Resolve o rótulo em `maintenance.<key>`.
    pub key: &'static str,
    /// Bloco ao qual a linha pertence (`carro` / `logistica` / `equipe`).
    pub group: &'static str,
    pub cost: f64,
}

/// Como uma rodada calcula a despesa de uma equipe.
///
/// Existe como tipo — e não como enum de modelos — porque a produção tem uma conta só:
/// [`despesa_da_rodada`]. O ponteiro é o encaixe por onde o harness pluga a conta ANTIGA
/// (`super::tests::despesa_legada::despesa_legada_da_rodada`) para rodar os dois lados sob a
/// mesma semente. Nenhum caminho de produção escolhe.
pub(crate) type CalculoDaDespesa =
    fn(&Team, f64, f64, f64, RoundOperationContext, EtapaFisica) -> DespesaDaRodada;

/// Os fatos FÍSICOS da etapa que a fatura antiga não precisava saber e a nova precisa.
///
/// Entra separado de [`RoundOperationContext`] de propósito: aquele struct é construído em
/// literal exaustivo pelo harness de calibração, e acrescentar campo lá quebraria um
/// arquivo que está sendo escrito por outra frente.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EtapaFisica {
    /// Duração REAL desta prova, em minutos.
    ///
    /// **Não** é `get_category_config(...).duracao_corrida_min`: aquela constante vale 0 no
    /// Endurance, porque lá o calendário sorteia entre 120, 180, 240 e 360
    /// (`calendar::montagem::resolve_race_duration`). A duração real mora no
    /// `CalendarEntry`, e é ela que decide quanto combustível e quanta borracha a etapa
    /// pede.
    pub duracao_corrida_min: f64,
    /// Carros que a equipe efetivamente inscreveu nesta etapa.
    pub carros_inscritos: u32,
}

impl EtapaFisica {
    /// A etapa de REFERÊNCIA da divisão: duração nominal e a lotação de carros que a
    /// categoria declara. É o que o harness de calibração mede quando não tem uma corrida
    /// concreta em mãos.
    pub(crate) fn de_referencia(categoria: &str, classe: Option<&str>) -> Self {
        Self {
            duracao_corrida_min: ancora::parametros(categoria, classe).duracao_corrida_min,
            carros_inscritos: carros_por_equipe(categoria),
        }
    }

    /// A etapa CONCRETA: a duração que o calendário sorteou e os carros que a equipe
    /// colocou na pista.
    ///
    /// Duas equipes da mesma categoria podem inscrever números diferentes de carros (piloto
    /// lesionado, carro destruído), então a contagem sai do resultado. Equipe ausente do
    /// resultado cai na lotação da categoria — **dois carros por equipe também no
    /// Endurance**, que é como este jogo modela o campeonato.
    pub(crate) fn da_corrida(result: &RaceResult, team: &Team, duracao_corrida_min: i32) -> Self {
        let inscritos = result
            .race_results
            .iter()
            .filter(|r| r.team_id == team.id)
            .count() as u32;
        Self {
            duracao_corrida_min: duracao_de_prova(
                &team.categoria,
                team.classe.as_deref(),
                duracao_corrida_min,
            ),
            carros_inscritos: if inscritos > 0 {
                inscritos
            } else {
                carros_por_equipe(&team.categoria)
            },
        }
    }
}

/// Lotação de carros da categoria. Lê `pilotos_por_equipe` em vez de fixar o 2 para que uma
/// categoria futura de piloto solo não passe despercebida pela fatura.
fn carros_por_equipe(categoria: &str) -> u32 {
    get_category_config(categoria)
        .map(|c| u32::from(c.pilotos_por_equipe))
        .unwrap_or(2)
        .max(1)
}

/// A duração de prova a usar: a informada pelo calendário, com queda para a duração de
/// referência da divisão quando ela não veio (`0` é o que a config da categoria devolve no
/// Endurance, e é justamente o caso em que ela não pode ser acreditada).
fn duracao_de_prova(categoria: &str, classe: Option<&str>, informada: i32) -> f64 {
    if informada > 0 {
        f64::from(informada)
    } else {
        ancora::parametros(categoria, classe).duracao_corrida_min
    }
}

/// O que a rodada cobra da equipe.
///
/// `operacao` e `estrutura` são DERIVADOS de [`Self::linhas_do_ledger`] — não são somas
/// paralelas. É a invariante "fatura mostrada = dinheiro debitado" escrita em código: não
/// existe caminho em que o agregado e a decomposição discordem, porque o agregado é a
/// decomposição somada.
#[derive(Debug, Clone)]
pub(crate) struct DespesaDaRodada {
    /// `event_operations_cost` — a fatura da etapa. Igual a
    /// `linhas_do_ledger.total_da_etapa()`.
    pub operacao: f64,
    /// `structural_maintenance_cost` — a fatia desta rodada nos recorrentes do ano. Igual a
    /// `linhas_do_ledger.estrutura`.
    pub estrutura: f64,
    /// `technical_investment_cost` **sem** a peça: a peça real é somada fora, porque é
    /// decidida pelo cérebro de manutenção e entra crua.
    pub tecnica: f64,
    /// As linhas itemizadas da operação, já em dinheiro, com os tokens de exibição de
    /// `maintenance.*`. Somam exatamente `operacao`.
    pub linhas: Vec<OperationLine>,
    /// As oito linhas que ficam no ledger. Mesma origem das outras duas.
    pub linhas_do_ledger: LinhasDaDespesa,
}

/// A despesa de UMA rodada de UMA equipe: `quantidade × preço` da etapa concreta mais a
/// fatia desta rodada nos recorrentes do ano.
pub(crate) fn despesa_da_rodada(
    team: &Team,
    round_operating_base: f64,
    cost_modifier: f64,
    rounds_in_season: f64,
    ctx: RoundOperationContext,
    etapa: EtapaFisica,
) -> DespesaDaRodada {
    // `round_operating_base` não dimensiona mais nada aqui — a fatura sai da física da etapa,
    // não de uma fração do orçamento. Fica na assinatura porque é o que dá a
    // [`CalculoDaDespesa`] a forma que o modelo antigo também tem, e é assim que o comparador
    // consegue rodar os dois lados sem saber qual está segurando.
    let _ = round_operating_base;

    let entrada = entrada_da_etapa(team, ctx, etapa);
    let fatura = fatura_da_etapa(&entrada);
    let linhas: Vec<OperationLine> = fatura
        .linhas
        .iter()
        .filter_map(|l| {
            rotulo_da_linha(l.chave).map(|(key, group)| OperationLine {
                key,
                group,
                cost: l.total() * cost_modifier,
            })
        })
        .filter(|l| l.cost > 0.0)
        .collect();

    // Os recorrentes do ano, diluídos pelo calendário. A folha de pilotos sai fora:
    // o contrato de verdade já é debitado como `salary_expense`.
    let recorrentes = temporada::fatura_de_temporada(
        &team.categoria,
        team.classe.as_deref(),
        &EquipeNaTemporada {
            instalacoes: team.facilities,
        },
    );
    let estrutura_anual =
        (recorrentes.total() - recorrentes.valor(temporada::FOLHA_DE_PILOTOS)).max(0.0);

    let valor = |chave: &str| fatura.valor(chave) * cost_modifier;
    let linhas_do_ledger = LinhasDaDespesa {
        combustivel: valor(evento::COMBUSTIVEL),
        pneus: valor(evento::PNEUS),
        desgaste_de_peca: valor(evento::REVISAO),
        frete: valor(evento::FRETE),
        viagem_e_estadia: valor(evento::VIAGEM) + valor(evento::ESTADIA),
        inscricao: valor(evento::INSCRICAO),
        diarias: valor(evento::DIARIAS),
        estrutura: estrutura_anual / rounds_in_season.max(1.0) * cost_modifier,
    };

    DespesaDaRodada {
        // Derivados das linhas, não somas paralelas: o agregado É a decomposição.
        operacao: linhas_do_ledger.total_da_etapa(),
        estrutura: linhas_do_ledger.estrutura,
        // Sem termo abstrato: o que existe de técnico nesta rodada é a peça, e ela
        // entra crua no caller.
        tecnica: 0.0,
        linhas,
        linhas_do_ledger,
    }
}

/// Monta a entrada da fatura física a partir do que a rodada já sabe.
pub(crate) fn entrada_da_etapa(
    team: &Team,
    ctx: RoundOperationContext,
    etapa: EtapaFisica,
) -> EntradaDaEtapa {
    let pista = PistaDaEtapa {
        comprimento_km: crate::constants::tracks::get_track(ctx.track_id)
            .map(|t| t.comprimento_km)
            .unwrap_or(PistaDaEtapa::default().comprimento_km),
        distancia_da_sede_km: distancia_da_sede_km(&team.pais_sede, ctx.track_id),
    };

    // ── A armadilha das voltas ───────────────────────────────────────────────────────
    //
    // O número ABSOLUTO de voltas não pode vir da corrida. `calendar::montagem::estimate_laps`
    // grampeia o calendário em 50 voltas — um limite de exibição — e a simulação roda
    // exatamente esse `entry.voltas` (`SimulationContext::from_calendar_entry`). Numa prova
    // de 6 horas isso são ~250 km simulados contra ~1.140 km de prova de verdade: alimentar a
    // fatura com `laps_completed` faria o Endurance queimar um quinto do combustível que ele
    // queima.
    //
    // O que a corrida entrega de confiável é a FRAÇÃO completada — quem abandonou, abandonou
    // de verdade, e a razão sobrevive ao grampo porque numerador e denominador são grampeados
    // juntos. Então a distância da prova sai da física (velocidade média × duração real) e a
    // fração vem do resultado.
    let p = ancora::parametros(&team.categoria, team.classe.as_deref());
    let voltas_da_prova = p.voltas_em(etapa.duracao_corrida_min, pista.comprimento_km);

    EntradaDaEtapa {
        categoria: team.categoria.clone(),
        classe: team.classe.clone(),
        duracao_corrida_min: etapa.duracao_corrida_min,
        equipe: EquipeNaEtapa {
            carros_inscritos: etapa.carros_inscritos,
            instalacoes: team.facilities,
            qualidade_pit_crew: team.pit_crew_quality,
        },
        pista,
        corrida: DesempenhoNaEtapa {
            voltas_completadas: voltas_da_prova * ctx.laps_ratio.clamp(0.0, 1.0),
            voltas_da_prova,
            desgaste_final_pneus: ctx.tire_wear,
        },
    }
}

/// Distância de ida entre a sede da equipe e a pista.
///
/// **Distância de verdade, não faixa.** A versão anterior classificava a viagem em três
/// degraus (casa / mesmo continente / outro continente) e traduzia cada um numa constante.
/// Do lado do modelo era uma aproximação defensável; do lado da TELA era um defeito de
/// credibilidade, e foi assim que ele apareceu: uma etapa no Reino Unido e outra na Noruega
/// cobraram os mesmos 9.679,4 km de frete, ao centavo. Duas corridas em países diferentes
/// com a mesma conta ensinam o jogador que o número é decorativo.
///
/// Agora sai de [`crate::constants::geografia`], que é a distância de grande círculo entre
/// o país da equipe e o país do circuito. Continua sendo uma aproximação — o jogo não
/// guarda endereço de sede —, mas é uma que distingue destinos.
///
/// Sem coordenada para um dos dois países, cai nas faixas antigas: falta de dado vira a
/// referência declarada, nunca um número inventado. E o piso é sempre
/// [`DISTANCIA_CASA_KM`] — nem uma corrida na cidade da sede tem logística de graça.
fn distancia_da_sede_km(pais_sede: &str, track_id: u32) -> f64 {
    use crate::constants::geografia;

    let Some(track) = crate::constants::tracks::get_track(track_id) else {
        return DISTANCIA_CONTINENTAL_KM;
    };
    if geografia::mesmo_pais(pais_sede, track.pais) {
        return DISTANCIA_CASA_KM;
    }
    if let Some(km) = geografia::distancia_entre_paises_km(pais_sede, track.pais) {
        return km.max(DISTANCIA_CASA_KM);
    }

    // Queda: a classificação em faixas, como era. Sem coordenada para um dos dois países a
    // única coisa que ainda dá para dizer é se a viagem atravessa oceano.
    match (
        geografia::continente(pais_sede),
        geografia::continente(track.pais),
    ) {
        (Some(a), Some(b)) if a != b => DISTANCIA_INTERCONTINENTAL_KM,
        _ => DISTANCIA_CONTINENTAL_KM,
    }
}

/// Token e bloco de exibição de uma linha física.
///
/// As chaves do modelo novo não são as do antigo (`combustivel` vs `gasolina`, `revisao` vs
/// `pecas`), mas descrevem a mesma coisa — então a fatura da tela reusa os rótulos que já
/// existem em `maintenance.*` em vez de duplicar i18n. A linha `estrutura` do modelo antigo
/// não tem par: ela virou recorrente de temporada e saiu da fatura da etapa.
fn rotulo_da_linha(chave: &str) -> Option<(&'static str, &'static str)> {
    match chave {
        evento::COMBUSTIVEL => Some(("gasolina", GROUP_CAR)),
        evento::PNEUS => Some(("pneus", GROUP_CAR)),
        evento::REVISAO => Some(("pecas", GROUP_CAR)),
        evento::FRETE => Some(("frete", GROUP_LOGISTICS)),
        evento::VIAGEM => Some(("viagem", GROUP_LOGISTICS)),
        evento::ESTADIA => Some(("estadia", GROUP_LOGISTICS)),
        evento::INSCRICAO => Some(("inscricao", GROUP_LOGISTICS)),
        evento::DIARIAS => Some(("diarias", GROUP_CREW)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::placeholder_team_from_db;

    fn equipe(categoria: &str) -> Team {
        let mut team = placeholder_team_from_db(
            "TDSP".to_string(),
            "Despesa".to_string(),
            categoria.to_string(),
            "2026-01-01".to_string(),
        );
        team.facilities = 50.0;
        team.pit_crew_quality = 50.0;
        team.engineering = 50.0;
        team
    }

    fn contexto() -> RoundOperationContext {
        RoundOperationContext {
            track_id: 47,
            laps_ratio: 1.0,
            tire_wear: 0.65,
        }
    }

    fn despesa_com(calculo: CalculoDaDespesa, team: &Team, rodadas: f64) -> DespesaDaRodada {
        let base = round_operating_base(&team.categoria, team.classe.as_deref(), rodadas);
        calculo(
            team,
            base,
            1.0,
            rodadas,
            contexto(),
            EtapaFisica::de_referencia(&team.categoria, team.classe.as_deref()),
        )
    }

    /// A conta que a produção debita.
    fn despesa(team: &Team, rodadas: f64) -> DespesaDaRodada {
        despesa_com(despesa_da_rodada, team, rodadas)
    }

    /// A conta ANTIGA, congelada em `super::super::tests::despesa_legada`. Vários testes
    /// daqui comparam as duas — é o que garante que o modelo velho continua alcançável e
    /// respondendo à mesma entrada, que é a condição do comparador.
    fn despesa_legada(team: &Team, rodadas: f64) -> DespesaDaRodada {
        despesa_com(
            crate::commands::race::tests::despesa_legada::despesa_legada_da_rodada,
            team,
            rodadas,
        )
    }

    /// Os dois lados, rotulados, para os testes que precisam valer nos dois.
    const MODELOS: [(&str, CalculoDaDespesa); 2] = [
        ("fisico", despesa_da_rodada),
        (
            "legado",
            crate::commands::race::tests::despesa_legada::despesa_legada_da_rodada,
        ),
    ];

    fn rodadas(categoria: &str) -> f64 {
        get_category_config(categoria)
            .map(|c| f64::from(c.corridas_por_temporada.max(1)))
            .unwrap_or(12.0)
    }

    /// Uma temporada de decisões do cérebro de manutenção, sem banco.
    ///
    /// Devolve, por rodada, quanto a equipe GASTOU EM PEÇA. Roda o mesmo
    /// `decide_car_maintenance` + `apply_plan_scaled` que a produção roda; o que fica de fora
    /// são as trocas FORÇADAS (quebra grave/DNF e contato destrutivo), que dependem de uma
    /// corrida de verdade. Isso torna o resultado um **piso**: na produção a peça é comprada
    /// pelo menos com esta frequência.
    fn temporadas_de_pecas(
        categoria: &str,
        classe: Option<&str>,
        meses_de_caixa: f64,
        temporadas: usize,
    ) -> Vec<f64> {
        let chave = crate::constants::categories::competitive_division_key(categoria, classe);
        let mut team = equipe(categoria);
        team.classe = classe.map(str::to_string);
        team.cash_balance =
            crate::economia::temporada::caixa_para_meses(categoria, classe, meses_de_caixa);
        team.reputacao = 50.0;

        let etapas = rodadas(categoria) as usize;
        let pistas: Vec<u32> = crate::constants::tracks::get_tracks_for_category(categoria)
            .iter()
            .map(|t| t.track_id)
            .cycle()
            .take(etapas)
            .collect();

        let mut car = crate::car::seed::seed_car(&chave, 0.5);
        let neutro = crate::car::breakdown::conditions_wear_mults(
            crate::market::car_maintenance::maintenance_demand(&pistas),
            crate::car::breakdown::Weather::NEUTRAL,
        );
        let com_tenda = crate::car::cost::category_ceiling(&chave) > 2;
        let rel = crate::car::wear::reliability_life_mult(team.pit_crew_quality);

        (0..etapas * temporadas)
            .map(|n| {
                let i = n % etapas;
                let restantes = &pistas[(i + 1).min(pistas.len())..];
                let max_up = crate::market::car_maintenance::upgrades_permitidos_nesta_corrida(
                    &team.id,
                    restantes.len(),
                );
                let plano = crate::market::car_maintenance::decide_car_maintenance(
                    &team,
                    &car,
                    &chave,
                    restantes,
                    Some(max_up),
                );
                let custo = plano.estimated_cost;
                crate::market::car_maintenance::apply_plan_scaled(
                    &mut car, &plano, &neutro, com_tenda, rel,
                );
                custo
            })
            .collect()
    }

    /// **O bloqueio que apareceu antes da medição da peça.**
    ///
    /// `decide_car_maintenance` orça a corrida em `spending_power / ETAPAS_DE_REFERENCIA`.
    /// A fórmula soma um ESTOQUE (caixa) com FLUXOS anuais (receita projetada, crédito) e
    /// subtrai compromissos e reserva, ambos múltiplos do custo operacional ANUAL:
    ///
    /// ```text
    /// spending = caixa + receita·confiança + crédito·agressividade
    ///          − comprometido − pressão de dívida − reserva
    /// ```
    ///
    /// Enquanto o caixa esperado valia ~24 meses de operação (2,05 × o anual), o termo de
    /// estoque pagava sozinho a reserva e sobrava. Com o caixa virando 1–11 meses, ele
    /// encolheu ~4× e os coeficientes de fluxo ficaram onde estavam.
    ///
    /// O relatório separa as duas mudanças para que dê para ver qual fez o quê:
    ///
    /// | coluna | fórmula | caixa |
    /// |---|---|---|
    /// | VELHO | a de antes | 24 meses (tabela antiga) |
    /// | QUEBRADO | a de antes | 6 meses (âncora física) |
    /// | NOVO | a re-derivada | 6 meses (âncora física) |
    ///
    /// A do meio é o mundo que existiu entre a troca da âncora de estoque e esta correção, e
    /// é a que precisa aparecer: ela é o defeito, não uma etapa de raciocínio.
    ///
    /// `cargo test --lib relatorio_poder_de_gasto -- --nocapture`
    #[test]
    fn relatorio_poder_de_gasto_contra_a_ancora_velha() {
        use crate::finance::planning::*;

        /// Quantos custos operacionais anuais a tabela ANTIGA mandava a equipe mediana
        /// guardar em caixa. Medido em `economia::tests::legado` na troca da âncora de
        /// estoque: `expected_cash_midpoint ÷ operating_cost_midpoint` dava 2,05 em toda a
        /// escada — ~24 meses de operação. Entra aqui como constante porque é história
        /// congelada, não um número a recalcular.
        const CAIXA_LEGADO_EM_ANOS: f64 = 2.05;

        /// A fórmula ANTIGA de `spending_power`, congelada. Existe para a comparação não ser
        /// contra si mesma: com a produção re-derivada, medir "o velho" chamando a produção
        /// devolveria o novo com outro nome.
        fn spending_power_legado(team: &Team, caixa_em_anos: f64, op: f64) -> f64 {
            op * caixa_em_anos
                + calculate_projected_income(team)
                    * income_confidence_for_state(&team.financial_state)
                + calculate_available_credit(team)
                    * credit_aggressiveness_for_state(&team.financial_state)
                - calculate_committed_costs(team)
                - calculate_debt_pressure(team)
                - op * safety_reserve_multiplier_for_state(&team.financial_state)
        }

        println!("\n=== Poder de gasto da equipe MEDIANA, em múltiplos do operacional anual ===");
        println!(
            "\n{:<16} {:>8} {:>9} {:>9} {:>9} {:>9} {:>10} {:>10}",
            "divisão", "caixa", "receita", "crédito", "compr.", "reserva", "QUEBRADO", "NOVO"
        );
        let mut velho_medio = 0.0;
        let mut linhas = 0.0;
        for (categoria, classe) in [
            ("mazda_rookie", None),
            ("mazda_amador", None),
            ("bmw_m2", None),
            ("gt4", None),
            ("gt3", None),
            ("lmp2", None),
            ("endurance", Some("gt3")),
        ] {
            let mut team = equipe(categoria);
            team.classe = classe.map(str::to_string);
            let op = category_finance_scale_for(categoria, classe).operating_cost_midpoint();
            team.cash_balance = crate::economia::temporada::caixa_de_referencia(categoria, classe);

            let conf = income_confidence_for_state(&team.financial_state);
            let agr = credit_aggressiveness_for_state(&team.financial_state);
            let receita = calculate_projected_income(&team) * conf;
            let credito = calculate_available_credit(&team) * agr;
            let compr = calculate_committed_costs(&team);
            let reserva = calculate_safety_reserve(&team);

            velho_medio += spending_power_legado(&team, CAIXA_LEGADO_EM_ANOS, op) / op;
            linhas += 1.0;
            let quebrado = spending_power_legado(&team, team.cash_balance / op, op) / op;
            let novo = calculate_spending_power(&team) / op;

            println!(
                "{:<16} {:>8.2} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>10.2} {:>10.2}",
                crate::constants::categories::competitive_division_key(categoria, classe),
                team.cash_balance / op,
                receita / op,
                credito / op,
                -compr / op,
                -reserva / op,
                quebrado,
                novo
            );
        }
        println!(
            "\n  VELHO (fórmula antiga + caixa de 24 meses): {:.2} em toda a escada",
            velho_medio / linhas
        );
        println!(
            "  (as colunas do meio são os termos do modelo NOVO; a reserva caiu de 0,90 do\n   \
             anual — dez meses e meio — para {:.0} meses)",
            reserva_de_sobrevivencia_em_meses()
        );

        // O número que decide se isto é um detalhe ou uma parede: quantos MESES de operação
        // a equipe precisa ter em caixa para o poder de gasto virar positivo — contra a
        // faixa de caixa que o próprio modelo declara (1 a 11 meses).
        println!("\n  Meses de caixa necessários para spending_power > 0:");
        for categoria in ["mazda_rookie", "bmw_m2", "gt4", "gt3", "lmp2"] {
            let mut team = equipe(categoria);
            let mut necessario = None;
            for passo in 0..=480 {
                let meses = passo as f64 / 10.0;
                team.cash_balance =
                    crate::economia::temporada::caixa_para_meses(categoria, None, meses);
                if calculate_spending_power(&team) > 0.0 {
                    necessario = Some(meses);
                    break;
                }
            }
            match necessario {
                Some(m) => println!(
                    "    {categoria:<16} {m:>5.1} meses   (faixa declarada: {}–{})",
                    crate::economia::temporada::CAIXA_MESES_MIN,
                    crate::economia::temporada::CAIXA_MESES_MAX
                ),
                None => println!("    {categoria:<16} nunca, até 48 meses"),
            }
        }

        // Quem lê `spending_power` e o que estava acontecendo com eles. Os três consumidores
        // grampeiam a razão `spending_power ÷ operacional` em pisos diferentes; com −0,76
        // todos estavam no piso ao mesmo tempo, para o grid inteiro, o que apaga a diferença
        // entre uma equipe rica e uma quebrada exatamente onde ela deveria decidir.
        println!("\n  Consumidores de spending_power, por meses de caixa (gt3):");
        println!(
            "  {:>7} {:>10} {:>12} {:>14} {:>14}",
            "meses", "sp÷anual", "teto salarial", "spending_score", "budget_index"
        );
        let op = category_finance_scale("gt3").operating_cost_midpoint();
        for meses in [1.0, 3.0, 6.0, 9.0, 12.0, 18.0, 24.0] {
            let mut team = equipe("gt3");
            team.cash_balance = crate::economia::temporada::caixa_para_meses("gt3", None, meses);
            let razao = calculate_spending_power(&team) / op;
            println!(
                "  {meses:>7.0} {razao:>10.2} {:>12.2}× {:>14.1} {:>14.1}",
                // `finance::salary` grampeia em −0,5..2,5 — abaixo disso todo mundo oferece
                // o mínimo, e o mercado para de separar quem pode pagar de quem não pode.
                razao.clamp(-0.5, 2.5),
                (razao * 55.0).clamp(-25.0, 100.0),
                derive_budget_index_from_money(&team),
            );
        }
    }

    /// **A medição que decide se a peça precisa de linha própria na fatura.**
    ///
    /// A fatura visível da etapa é `event_operations_cost` — as sete linhas físicas. A compra
    /// de peça (`technical_investment_cost`) sai do caixa na MESMA rodada e não aparece.
    /// A pergunta é se isso é uma omissão de rodapé ou uma mentira: quantas rodadas por
    /// temporada compram peça, e quanto a compra pesa contra a fatura daquela rodada.
    ///
    /// Rode com
    /// `cargo test --lib relatorio_peso_da_peca -- --nocapture`.
    #[test]
    fn relatorio_peso_da_peca_contra_a_fatura_visivel() {
        const TEMPORADAS: usize = 5;
        println!("\n=== Compra de peça × fatura visível da etapa ===");
        println!("    {TEMPORADAS} temporadas seguidas, carro herdado entre elas");
        println!("    (piso: só o plano do cérebro; troca forçada por quebra/contato não entra)");
        println!("    6 meses = o caixa de referência de hoje · 24 meses = o que a tabela velha");
        println!("    declarava, e o mundo em que este cérebro foi calibrado");
        println!(
            "\n{:<24} {:>9} {:>10} {:>10} {:>11} {:>9} {:>8}",
            "divisão", "caixa", "com peça", "fatura", "peça média", "% fatura", "pior"
        );
        for (categoria, classe) in crate::economia::ancora::DIVISOES {
            for (rotulo, meses) in [("6 meses", 6.0), ("16 meses", 16.0), ("24 meses", 24.0)] {
                let r = rodadas(categoria);
                let mut team = equipe(categoria);
                team.classe = classe.map(str::to_string);
                let fatura = despesa(&team, r).operacao;

                let pecas = temporadas_de_pecas(categoria, classe, meses, TEMPORADAS);
                let com_peca = pecas.iter().filter(|c| **c > 0.0).count();
                let media: f64 = if com_peca > 0 {
                    pecas.iter().filter(|c| **c > 0.0).sum::<f64>() / com_peca as f64
                } else {
                    0.0
                };
                let pior = pecas.iter().copied().fold(0.0_f64, f64::max);

                println!(
                    "{:<24} {rotulo:>9} {:>7}/{:<3} {:>9.0} {:>10.0} {:>8.0}% {:>7.0}%",
                    crate::constants::categories::competitive_division_key(categoria, classe),
                    com_peca,
                    pecas.len(),
                    fatura,
                    media,
                    media / fatura.max(1.0) * 100.0,
                    pior / fatura.max(1.0) * 100.0,
                );
            }
        }
    }

    /// **A propriedade em que a nona linha se apoia.**
    ///
    /// No modelo físico `technical_investment_cost` é a compra de peça e **nada mais** —
    /// `despesa.tecnica` é zero, então o que sobra naquela linha do ledger é exatamente o que
    /// o cérebro de manutenção decidiu gastar. É isso que permite a fatura mostrar a troca de
    /// peça sem recalcular nada: o número já está gravado e já é só ele.
    ///
    /// No modelo legado a mesma linha carregava `0,16 × base` de investimento abstrato POR
    /// CIMA da peça, e não dava para separar os dois depois.
    #[test]
    fn no_modelo_fisico_a_linha_tecnica_e_so_a_peca() {
        for (categoria, classe) in crate::economia::ancora::DIVISOES {
            let mut team = equipe(categoria);
            team.classe = classe.map(str::to_string);
            let d = despesa(&team, rodadas(categoria));
            assert_eq!(
                d.tecnica, 0.0,
                "{categoria}: a parcela abstrata de investimento técnico precisa ser zero \
                 para `technical_investment_cost` ser a peça comprada e mais nada"
            );
        }
        // O legado, por contraste, mistura — e é por isso que a fatura antiga não tinha como
        // mostrar a peça mesmo se quisesse.
        let team = equipe("gt3");
        assert!(despesa_legada(&team, rodadas("gt3")).tecnica > 0.0);
    }

    /// Os dois modelos continuam alcançáveis com a MESMA entrada. É a condição da
    /// comparação lado a lado — se um dos caminhos deixasse de compilar ou de responder,
    /// não haveria contra o que comparar.
    #[test]
    fn os_dois_modelos_respondem_a_mesma_entrada() {
        for categoria in ["mazda_rookie", "bmw_m2", "gt4", "gt3", "lmp2"] {
            let team = equipe(categoria);
            let r = rodadas(categoria);
            let velho = despesa_legada(&team, r);
            let novo = despesa(&team, r);
            assert!(velho.operacao > 0.0 && novo.operacao > 0.0, "{categoria}");
            assert!(
                !velho.linhas.is_empty() && !novo.linhas.is_empty(),
                "{categoria}"
            );
        }
    }

    /// **As linhas do ledger não são um resumo — são a conta.** O que vai para o banco tem
    /// que somar exatamente os dois agregados que saíram do caixa, nos dois modelos e em
    /// toda a escada. É a mesma invariante da fatura da tela, um nível abaixo: o dossiê lê o
    /// ledger, e um ledger que não fecha vira uma decomposição que mente com números
    /// plausíveis.
    #[test]
    fn as_linhas_do_ledger_somam_os_agregados() {
        for (modelo, calculo) in MODELOS {
            for (categoria, classe) in crate::economia::ancora::DIVISOES {
                let mut team = equipe(categoria);
                team.classe = classe.map(str::to_string);
                let d = despesa_com(calculo, &team, rodadas(categoria));
                let folga = d.operacao.abs().max(1.0) * 1e-9;
                assert!(
                    (d.linhas_do_ledger.total_da_etapa() - d.operacao).abs() < folga,
                    "{modelo}/{categoria}: linhas do ledger somam {:.2}, operação é {:.2}",
                    d.linhas_do_ledger.total_da_etapa(),
                    d.operacao
                );
                assert!(
                    (d.linhas_do_ledger.estrutura - d.estrutura).abs() < folga,
                    "{modelo}/{categoria}: estrutura do ledger {:.2} vs debitada {:.2}",
                    d.linhas_do_ledger.estrutura,
                    d.estrutura
                );
            }
        }
    }

    /// Nenhuma linha da fatura física pode se perder no caminho para o ledger. O mapeamento
    /// funde `viagem` e `estadia` numa linha só — e é a ÚNICA fusão; qualquer outra chave
    /// que caísse fora do `match` sumiria em silêncio, deixando a soma menor que o débito.
    #[test]
    fn nenhuma_linha_fisica_se_perde_no_caminho_do_ledger() {
        let team = equipe("gt3");
        let entrada = entrada_da_etapa(&team, contexto(), EtapaFisica::de_referencia("gt3", None));
        let fatura = fatura_da_etapa(&entrada);
        let d = despesa(&team, rodadas("gt3"));
        assert!(
            (fatura.total() - d.linhas_do_ledger.total_da_etapa()).abs() < fatura.total() * 1e-9,
            "a fatura física soma {:.2} e o ledger guardou {:.2}",
            fatura.total(),
            d.linhas_do_ledger.total_da_etapa()
        );
    }

    /// A fatura mostrada e o dinheiro debitado são a mesma conta — em qualquer dos dois
    /// modelos. Era isto que a fatura antiga prometia e que a troca não pode perder.
    #[test]
    fn as_linhas_somam_o_que_sai_do_caixa() {
        for (modelo, calculo) in MODELOS {
            for categoria in ["mazda_rookie", "gt3", "endurance"] {
                let team = equipe(categoria);
                let d = despesa_com(calculo, &team, rodadas(categoria));
                let soma: f64 = d.linhas.iter().map(|l| l.cost).sum();
                assert!(
                    (soma - d.operacao).abs() < d.operacao * 1e-9,
                    "{modelo}/{categoria}: linhas somam {soma:.2}, operação é {:.2}",
                    d.operacao
                );
            }
        }
    }

    /// **O motivo da troca, medido.** A despesa técnica de uma temporada, contra o custo
    /// operacional anual que a categoria DECLARA. O modelo antigo cobra quase o dobro do que
    /// a própria âncora dele diz; o novo cobra o que ela diz, menos a folha de pilotos, que é
    /// debitada como contrato.
    ///
    /// Sem este teste o número 1,96 embutido nos coeficientes de receita seria folclore.
    #[test]
    fn relatorio_despesa_da_temporada_contra_a_ancora() {
        println!("\n=== Despesa técnica da temporada ÷ custo operacional anual declarado ===");
        println!(
            "{:<22} {:>14} {:>10} {:>10} {:>10}",
            "divisão", "anual declarado", "legado", "físico", "alvo"
        );
        for (categoria, classe) in crate::economia::ancora::DIVISOES {
            let mut team = equipe(categoria);
            team.classe = classe.map(str::to_string);
            let r = rodadas(categoria);
            let anual = crate::economia::temporada::custo_operacional_anual_de_referencia(
                categoria, classe,
            );
            // O alvo não é 1,00: a folha de pilotos está dentro do anual declarado e sai do
            // caixa como contrato, não como despesa técnica.
            let alvo = 1.0
                - crate::economia::temporada::decomposicao_anual(categoria, classe)
                    .folha_de_pilotos
                    / anual;

            let soma = |c: CalculoDaDespesa| {
                let d = despesa_com(c, &team, r);
                (d.operacao + d.estrutura + d.tecnica) * r / anual
            };
            println!(
                "{:<22} {:>14.0} {:>10.2} {:>10.2} {:>10.2}",
                crate::constants::categories::competitive_division_key(categoria, classe),
                anual,
                soma(crate::commands::race::tests::despesa_legada::despesa_legada_da_rodada),
                soma(despesa_da_rodada),
                alvo
            );
        }
    }

    /// A prova de 6 horas do Endurance tem que custar mais que a de 2 — e o único caminho
    /// por onde essa diferença entra é a duração REAL do `CalendarEntry`. Se a costura
    /// tivesse lido `get_category_config("endurance").duracao_corrida_min` (que vale 0) as
    /// quatro provas do calendário custariam igual.
    #[test]
    fn a_duracao_real_da_prova_move_a_fatura_do_endurance() {
        let mut team = equipe("endurance");
        team.classe = Some("gt3".to_string());
        let r = rodadas("endurance");
        let base = round_operating_base("endurance", Some("gt3"), r);

        let com = |minutos: f64| {
            despesa_da_rodada(
                &team,
                base,
                1.0,
                r,
                contexto(),
                EtapaFisica {
                    duracao_corrida_min: minutos,
                    carros_inscritos: 2,
                },
            )
            .operacao
        };

        let curta = com(120.0);
        let longa = com(360.0);
        assert!(
            longa > curta * 1.3,
            "prova de 6h ({longa:.0}) precisa custar bem mais que a de 2h ({curta:.0})"
        );
    }

    /// As voltas da fatura saem da física, não do campo grampeado do calendário. Uma prova
    /// de 6 horas pede muito mais que as 50 voltas que `estimate_laps` sabe contar.
    #[test]
    fn as_voltas_da_fatura_ignoram_o_grampo_de_cinquenta() {
        let mut team = equipe("endurance");
        team.classe = Some("lmp2".to_string());
        let entrada = entrada_da_etapa(
            &team,
            contexto(),
            EtapaFisica {
                duracao_corrida_min: 360.0,
                carros_inscritos: 2,
            },
        );
        assert!(
            entrada.corrida.voltas_da_prova > 50.0,
            "a prova física deu {:.0} voltas — o grampo de 50 voltou a entrar na conta",
            entrada.corrida.voltas_da_prova
        );
    }

    /// Abandonar cedo continua barateando a etapa: é a fração completada, e ela é o que
    /// sobrevive ao grampo do calendário.
    #[test]
    fn abandono_barateia_a_etapa_no_modelo_fisico() {
        let team = equipe("gt3");
        let r = rodadas("gt3");
        let base = round_operating_base("gt3", None, r);
        let com = |laps_ratio: f64, tire_wear: f64| {
            despesa_da_rodada(
                &team,
                base,
                1.0,
                r,
                RoundOperationContext {
                    track_id: 47,
                    laps_ratio,
                    tire_wear,
                },
                EtapaFisica::de_referencia("gt3", None),
            )
            .operacao
        };
        assert!(com(0.1, 0.1) < com(1.0, 0.9));
    }

    /// A folha de pilotos NÃO pode entrar na despesa estrutural: ela já sai do caixa como
    /// contrato. Contar duas vezes o item mais caro dos recorrentes do topo seria o tipo de
    /// erro que só aparece no fim da temporada, como falência inexplicada.
    #[test]
    fn a_folha_de_pilotos_nao_e_cobrada_duas_vezes() {
        for (categoria, classe) in [("gt3", None), ("endurance", Some("lmp2"))] {
            let mut team = equipe(categoria);
            team.classe = classe.map(str::to_string);
            let r = rodadas(categoria);
            let d = despesa(&team, r);

            let recorrentes = temporada::fatura_de_temporada(
                categoria,
                classe,
                &EquipeNaTemporada { instalacoes: 50.0 },
            );
            let esperado =
                (recorrentes.total() - recorrentes.valor(temporada::FOLHA_DE_PILOTOS)) / r;
            assert!(
                (d.estrutura - esperado).abs() < esperado * 1e-9,
                "{categoria}: estrutura {:.2} devia ser {esperado:.2}",
                d.estrutura
            );
            assert!(recorrentes.valor(temporada::FOLHA_DE_PILOTOS) > 0.0);
        }
    }

    /// **A prova de que tirar `finance::operacao` da produção não moveu um quilômetro.**
    ///
    /// A queda de `distancia_da_sede_km` — o caminho de quando um dos dois países não tem
    /// coordenada — era escrita em cima de `travel_factor`: rodava o classificador inteiro e
    /// depois traduzia o fator de volta em faixa. Hoje ela pergunta a geografia direto. As
    /// duas formas são a mesma coisa, e "são a mesma coisa" só vale medido: o cruzamento de
    /// todo país-sede do mundo com toda pista do calendário, 100% dos pares.
    ///
    /// O lado velho aqui não é uma reconstrução — é o `travel_factor` congelado em
    /// `tests::despesa_legada`, o mesmo que o comparador usa.
    #[test]
    fn a_queda_geografica_do_frete_e_a_mesma_de_antes() {
        use crate::commands::race::tests::despesa_legada::{
            travel_factor, TRAVEL_HOME, TRAVEL_INTERCONTINENTAL,
        };
        use crate::constants::geografia;

        // A forma ANTIGA, verbatim: tenta a distância de verdade e cai na classificação em
        // faixas via `travel_factor`.
        let antiga = |pais_sede: &str, track_id: u32| -> f64 {
            if let Some(track) = crate::constants::tracks::get_track(track_id) {
                if geografia::mesmo_pais(pais_sede, track.pais) {
                    return DISTANCIA_CASA_KM;
                }
                if let Some(km) = geografia::distancia_entre_paises_km(pais_sede, track.pais) {
                    return km.max(DISTANCIA_CASA_KM);
                }
            }
            let fator = travel_factor(pais_sede, track_id);
            if fator <= TRAVEL_HOME {
                DISTANCIA_CASA_KM
            } else if fator >= TRAVEL_INTERCONTINENTAL {
                DISTANCIA_INTERCONTINENTAL_KM
            } else {
                DISTANCIA_CONTINENTAL_KM
            }
        };

        let mut pares = 0;
        for time in crate::constants::teams::get_all_team_templates() {
            for track in crate::constants::tracks::get_all_tracks() {
                let esperado = antiga(time.pais_sede, track.track_id);
                let obtido = distancia_da_sede_km(time.pais_sede, track.track_id);
                assert_eq!(
                    esperado.to_bits(),
                    obtido.to_bits(),
                    "{} → pista {}: antes {esperado} km, agora {obtido} km",
                    time.pais_sede,
                    track.track_id
                );
                pares += 1;
            }
        }
        // Pista inexistente é o caso que a reestruturação mais facilmente perderia.
        assert_eq!(
            antiga("🇧🇷 Brasil", 999_999).to_bits(),
            distancia_da_sede_km("🇧🇷 Brasil", 999_999).to_bits()
        );
        assert!(
            pares > 1_000,
            "só {pares} pares — a varredura não cobriu nada"
        );
    }

    /// Etapa em casa custa menos que etapa do outro lado do oceano — a geografia que a
    /// fatura antiga tratava como multiplicador vira quilômetro de frete e de passagem.
    #[test]
    fn a_geografia_vira_quilometro() {
        let mut team = equipe("gt3");
        team.pais_sede = "🇧🇷 Brasil".to_string();
        let entrada_casa = entrada_da_etapa(
            &team,
            RoundOperationContext {
                track_id: 212, // Interlagos
                ..contexto()
            },
            EtapaFisica::de_referencia("gt3", None),
        );
        let entrada_longe =
            entrada_da_etapa(&team, contexto(), EtapaFisica::de_referencia("gt3", None));
        assert!(
            entrada_casa.pista.distancia_da_sede_km < entrada_longe.pista.distancia_da_sede_km,
            "casa {} vs fora {}",
            entrada_casa.pista.distancia_da_sede_km,
            entrada_longe.pista.distancia_da_sede_km
        );
    }
}
