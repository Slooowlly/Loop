//! Assédio a um piloto do jogador que JÁ tem contrato: a equipe grande bate na porta
//! no meio da pré-temporada e o jogador aceita ou fica.
//!
//! Espelha o assédio da IA ([`super::assedio`]), mas com o leilão de lances exposto na
//! tela (`PoachBid`) e o desfecho decidido pelo jogador (`resolve_player_poach`).

use super::*;

/// Fama mínima (Estrela+) pra o jogador virar alvo de quebra de contrato. RARO.
pub(super) const PLAYER_POACH_MIN_FAMA: f64 = 70.0;

/// Gap MÍNIMO de qualidade pra um time cobiçar o jogador contratado — o assédio é
/// especial: só um time claramente MELHOR que o atual faz esse esforço.
pub(super) const PLAYER_POACH_QUALITY_MARGIN: f64 = 15.0;

/// Um lance do leilão, já pronto pra tela (nome do time + de quem é o lance).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PoachBid {
    pub team_name: String,
    pub is_poacher: bool,
    pub salary: f64,
    /// "abertura" no 1º (status quo do time atual) | "lance N" nos seguintes.
    pub label: String,
}

/// Rótulo de um lance do leilão (display): 1º = "abertura", seguintes = "lance N".
pub(super) fn bid_label(i: usize) -> String {
    if i == 0 {
        rust_i18n::t!("market.bid.opening").to_string()
    } else {
        rust_i18n::t!("market.bid.nth", n = i).to_string()
    }
}

/// Uma proposta de quebra de contrato dirigida ao JOGADOR — carrega os ids p/ a
/// resolução E os campos de exibição p/ a tela do leilão. Persistida no plano da
/// pré-temporada (uma por janela, no máx), consumida ao decidir.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayerPoachOffer {
    // --- resolução ---
    pub current_contract_id: String,
    pub current_team_id: String,
    pub suitor_team_id: String,
    // --- números do negócio ---
    pub buyout: f64,
    pub current_salary: f64,
    /// Salário se SAIR (melhor lance do assediante).
    pub poacher_best: f64,
    /// Salário se FICAR (melhor cobertura do time atual; ≥ atual).
    pub holder_best: f64,
    // --- exibição ---
    pub suitor_name: String,
    pub suitor_color: String,
    pub suitor_car_rating: u8,
    pub current_team_name: String,
    pub current_team_color: String,
    pub category_label: String,
    /// Quem sairia da vaga do assediante pra dar lugar ao jogador (None se vaga aberta).
    pub incumbent_name: Option<String>,
    pub player_fama: u8,
    pub bids: Vec<PoachBid>,
    /// Quem venceu ECONOMICAMENTE (só narrativa — a palavra é do jogador).
    pub poacher_wins: bool,
}

/// Resultado de resolver a quebra de contrato do jogador.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerPoachOutcome {
    /// A decisão surtiu efeito? (false = a oferta estava velha/expirada.)
    pub applied: bool,
    /// O jogador saiu (true) ou ficou (false).
    pub left: bool,
    pub salary: f64,
    pub team_name: String,
    pub note: String,
}

/// Valor de um piloto (Driver) para um time, dado o need factor do time.
pub(super) fn driver_poach_value(driver: &Driver, need: f64) -> f64 {
    crate::market::poaching::poach_target_value(
        driver.atributos.skill,
        driver.atributos.midia,
        need,
    )
}

/// Mínimo de lances mostrados na tela do jogador quando há disputa de verdade
/// (abertura + 4 turnos). Só afeta a EXIBIÇÃO — a economia real (poacher_best/
/// holder_best/vencedor) já está decidida.
pub(super) const PLAYER_MIN_DISPLAY_BIDS: usize = 5;

/// Monta os lances a MOSTRAR pro jogador. Se os dois lados subiram de verdade e o
/// leilão real convergiu rápido (poucos lances), dramatiza numa sequência crescente
/// de ao menos [`PLAYER_MIN_DISPLAY_BIDS`] alternando os times e terminando no
/// vencedor pelo salário final. Se o time atual NÃO brigou (holder_best == atual),
/// não inventa disputa: devolve os lances reais.
pub(super) fn build_player_display_bids(
    real: &[PoachBid],
    suitor_name: &str,
    holder_name: &str,
    current_salary: f64,
    poacher_best: f64,
    holder_best: f64,
) -> Vec<PoachBid> {
    let both_fought = poacher_best > current_salary + 1.0 && holder_best > current_salary + 1.0;
    if real.len() >= PLAYER_MIN_DISPLAY_BIDS || !both_fought {
        return real.to_vec();
    }

    // Candidatos crescentes: abertura (status quo) + subidas intermediárias de cada
    // lado + os finais reais dos dois. Ordena por valor → sobe monotônico, o maior
    // (do vencedor) fica por último.
    let mut vals: Vec<(bool, f64)> = vec![
        (false, current_salary),
        (true, current_salary + 0.5 * (poacher_best - current_salary)),
        (false, current_salary + 0.6 * (holder_best - current_salary)),
        (true, poacher_best),
        (false, holder_best),
    ];
    vals.sort_by(|a, b| a.1.total_cmp(&b.1));

    vals.into_iter()
        .enumerate()
        .map(|(i, (is_poacher, salary))| PoachBid {
            team_name: if is_poacher { suitor_name } else { holder_name }.to_string(),
            is_poacher,
            salary: salary.round(),
            label: bid_label(i),
        })
        .collect()
}

/// Detecta se o JOGADOR está sendo cobiçado por um time claramente melhor e monta o
/// leilão de salário SEM executar. Retorna None na esmagadora maioria das viradas —
/// é raro por design (fama Estrela+ E um pretendente muito acima do time atual).
pub(crate) fn compute_player_poach_offer(
    conn: &Connection,
    new_season_number: i32,
) -> Result<Option<PlayerPoachOffer>, String> {
    compute_player_poach_offer_inner(conn, new_season_number, false)
}

/// Variante de DEBUG: relaxa os portões (fama, gap de qualidade, upgrade, caixa) e
/// escolhe o pretendente mais RICO da categoria — pra forçar o leilão a aparecer e
/// dar pra testar a tela mesmo num save sem o cenário raro. Só usada pelo debug, e por
/// isso fora do binário de release junto com ele.
#[cfg(debug_assertions)]
pub(crate) fn debug_build_player_poach_offer(
    conn: &Connection,
    new_season_number: i32,
) -> Result<Option<PlayerPoachOffer>, String> {
    compute_player_poach_offer_inner(conn, new_season_number, true)
}

/// O que o assédio precisa saber antes de sair procurando pretendente: quem é o
/// jogador, o contrato que o segura hoje, o time dono desse contrato e quanto custa
/// arrancá-lo de lá.
struct ContextoDoAssedio {
    player: Driver,
    current: Contract,
    current_team: crate::models::team::Team,
    current_quality: f64,
    buyout: f64,
    /// Todos os pilotos por id — para avaliar os ocupantes do assento do pretendente.
    drivers_by_id: HashMap<String, Driver>,
}

/// O pretendente escolhido e quem sairia do assento dele para dar lugar ao jogador.
struct Pretendente {
    time: crate::models::team::Team,
    /// `None` quando a vaga é aberta (não acontece hoje: o filtro exige as duas cheias).
    deslocado_id: Option<String>,
}

/// O desfecho ECONÔMICO do leilão, já com os lances prontos para a tela.
struct LeilaoDoAssedio {
    poacher_best: f64,
    holder_best: f64,
    bids: Vec<PoachBid>,
    poacher_wins: bool,
}

pub(super) fn compute_player_poach_offer_inner(
    conn: &Connection,
    new_season_number: i32,
    force: bool,
) -> Result<Option<PlayerPoachOffer>, String> {
    let Some(contexto) = contexto_do_assedio(conn, new_season_number, force)? else {
        return Ok(None);
    };
    let Some(pretendente) = melhor_pretendente(conn, &contexto, force)? else {
        return Ok(None);
    };
    let Some(leilao) = leilao_do_assedio(conn, &contexto, &pretendente.time, force)? else {
        return Ok(None);
    };
    Ok(Some(montar_oferta(&contexto, &pretendente, leilao)))
}

/// Etapa 1 — os portões de entrada e a conta da multa. `None` na esmagadora maioria das
/// viradas: o jogador precisa estar ativo, ser Estrela+ e TER contrato (agente livre não
/// é arrancado — ele já escolhe pela escada).
fn contexto_do_assedio(
    conn: &Connection,
    new_season_number: i32,
    force: bool,
) -> Result<Option<ContextoDoAssedio>, String> {
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return Ok(None);
    };
    if player.status != DriverStatus::Ativo
        || (!force && player.atributos.midia <= PLAYER_POACH_MIN_FAMA)
    {
        return Ok(None);
    }
    let Some(current) = contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?
    else {
        return Ok(None);
    };
    let Some(current_team) = team_queries::get_team_by_id(conn, &current.equipe_id)
        .map_err(|e| format!("Falha ao carregar time atual do jogador: {e}"))?
    else {
        return Ok(None);
    };
    let current_quality = team_quality(&current_team);

    let years = (current.temporada_fim - new_season_number + 1).max(1);
    let buyout = crate::market::poaching::buyout_fee(
        current.salario_anual,
        years,
        player.atributos.skill,
        player.atributos.midia,
    );

    let drivers_by_id: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos p/ poach do jogador: {e}"))?
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();

    Ok(Some(ContextoDoAssedio {
        player,
        current,
        current_team,
        current_quality,
        buyout,
        drivers_by_id,
    }))
}

/// Etapa 2 — o pretendente: time regular, não-jogador, mesma categoria+classe, as DUAS
/// vagas cheias, qualidade claramente acima da atual, que vê o jogador como upgrade sobre
/// o mais fraco do próprio elenco e consegue pagar a multa. Fica com o de MAIOR qualidade.
///
/// No modo debug (`force`) todos os portões caem e o critério vira o CAIXA: o leilão
/// precisa de um time rico para ter fôlego de dar lances de verdade na tela.
fn melhor_pretendente(
    conn: &Connection,
    contexto: &ContextoDoAssedio,
    force: bool,
) -> Result<Option<Pretendente>, String> {
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao carregar times p/ poach do jogador: {e}"))?;

    let mut best: Option<(crate::models::team::Team, f64, Option<String>)> = None;
    for t in &teams {
        if t.is_player_team
            || t.id == contexto.current_team.id
            || !uses_regular_contracts(&t.categoria)
        {
            continue;
        }
        if t.categoria != contexto.current.categoria || t.classe != contexto.current.classe {
            continue;
        }
        let (Some(p1), Some(p2)) = (t.piloto_1_id.clone(), t.piloto_2_id.clone()) else {
            continue;
        };
        let q = team_quality(t);
        if !force && q <= contexto.current_quality + PLAYER_POACH_QUALITY_MARGIN {
            continue;
        }
        if !force && !crate::market::poaching::can_afford_buyout(t.cash_balance, contexto.buyout) {
            continue;
        }
        let need = crate::fame::team_need_factor(
            crate::finance::planning::derive_budget_index_from_money(t),
            t.reputacao,
        );
        let player_value = crate::market::poaching::poach_target_value(
            contexto.player.atributos.skill,
            contexto.player.atributos.midia,
            need,
        );
        let mut occ: Vec<(String, f64)> = [p1, p2]
            .into_iter()
            .filter_map(|id| {
                contexto
                    .drivers_by_id
                    .get(&id)
                    .map(|d| (id, driver_poach_value(d, need)))
            })
            .collect();
        occ.sort_by(|a, b| a.1.total_cmp(&b.1));
        let Some((weak_id, weak_value)) = occ.first().cloned() else {
            continue;
        };
        if !force && !crate::market::poaching::is_clear_upgrade(player_value, weak_value) {
            continue;
        }
        let rank = if force { t.cash_balance } else { q };
        if best.as_ref().is_none_or(|(_, br, _)| rank > *br) {
            best = Some((t.clone(), rank, Some(weak_id)));
        }
    }

    Ok(best.map(|(time, _, deslocado_id)| Pretendente { time, deslocado_id }))
}

/// Etapa 3 — o leilão de salário: assediante contra o time atual, com o status quo como
/// lance de abertura. `None` quando o assediante não dá nem um lance (não cobriu nem o
/// salário de hoje) — sem lance dele não existe negócio a mostrar.
fn leilao_do_assedio(
    conn: &Connection,
    contexto: &ContextoDoAssedio,
    suitor: &crate::models::team::Team,
    force: bool,
) -> Result<Option<LeilaoDoAssedio>, String> {
    let ContextoDoAssedio {
        player,
        current,
        current_team,
        current_quality,
        buyout,
        ..
    } = contexto;

    let suitor_need = crate::fame::team_need_factor(
        crate::finance::planning::derive_budget_index_from_money(suitor),
        suitor.reputacao,
    );
    let current_need = crate::fame::team_need_factor(
        crate::finance::planning::derive_budget_index_from_money(current_team),
        current_team.reputacao,
    );
    let poacher_side = crate::market::poaching::AuctionSide {
        team_id: suitor.id.clone(),
        team_quality: team_quality(suitor),
        bond: crate::market::bond::get_bond(conn, &player.id, &suitor.id)?,
        ceiling: crate::market::poaching::salary_ceiling(
            current.salario_anual,
            crate::market::poaching::poach_target_value(
                player.atributos.skill,
                player.atributos.midia,
                suitor_need,
            ),
            suitor.cash_balance - buyout,
        ),
    };
    let holder_side = crate::market::poaching::AuctionSide {
        team_id: current_team.id.clone(),
        team_quality: *current_quality,
        bond: crate::market::bond::get_bond(conn, &player.id, &current_team.id)?,
        ceiling: crate::market::poaching::salary_ceiling(
            current.salario_anual,
            crate::market::poaching::poach_target_value(
                player.atributos.skill,
                player.atributos.midia,
                current_need,
            ),
            current_team.cash_balance,
        ),
    };
    let auction = crate::market::poaching::resolve_salary_auction(
        current.salario_anual,
        &poacher_side,
        &holder_side,
    );

    if !auction.bids.iter().any(|b| b.team_id == suitor.id) {
        return Ok(None);
    }
    let poacher_best = auction
        .bids
        .iter()
        .filter(|b| b.team_id == suitor.id)
        .map(|b| b.salary)
        .fold(0.0_f64, f64::max);
    let holder_best = auction
        .bids
        .iter()
        .filter(|b| b.team_id == current_team.id)
        .map(|b| b.salary)
        .fold(current.salario_anual, f64::max);
    // DEBUG: se o time atual não teria como brigar (pobre), finge uma cobertura
    // competitiva SÓ pra dar pra ver a disputa de 4 turnos na tela. Nunca no jogo real.
    let holder_best = if force && holder_best <= current.salario_anual + 1.0 {
        (current.salario_anual + 0.7 * (poacher_best - current.salario_anual)).round()
    } else {
        holder_best
    };

    let real_bids: Vec<PoachBid> = auction
        .bids
        .iter()
        .enumerate()
        .map(|(i, b)| PoachBid {
            team_name: if b.team_id == suitor.id {
                suitor.nome.clone()
            } else {
                current_team.nome.clone()
            },
            is_poacher: b.team_id == suitor.id,
            salary: b.salary,
            label: bid_label(i),
        })
        .collect();
    // Garante uma disputa com fôlego pra tela do jogador: quando os DOIS lados sobem de
    // verdade, mostra ao menos 4 turnos (a economia real — poacher_best/holder_best/
    // vencedor — não muda; isto é só a dramatização dos lances).
    let bids = build_player_display_bids(
        &real_bids,
        &suitor.nome,
        &current_team.nome,
        current.salario_anual,
        poacher_best,
        holder_best,
    );

    Ok(Some(LeilaoDoAssedio {
        poacher_best,
        holder_best,
        bids,
        poacher_wins: auction.poacher_wins,
    }))
}

/// Etapa 4 — o DTO que atravessa a ponte. Os nomes dos campos estão congelados pelo
/// guard em `pipeline/tests/contrato.rs`: a tela lê cada um deles pelo nome, e um campo
/// renomeado compila, serializa e chega ao React como `undefined`.
fn montar_oferta(
    contexto: &ContextoDoAssedio,
    pretendente: &Pretendente,
    leilao: LeilaoDoAssedio,
) -> PlayerPoachOffer {
    let suitor = &pretendente.time;
    PlayerPoachOffer {
        current_contract_id: contexto.current.id.clone(),
        current_team_id: contexto.current_team.id.clone(),
        suitor_team_id: suitor.id.clone(),
        buyout: contexto.buyout,
        current_salary: contexto.current.salario_anual,
        poacher_best: leilao.poacher_best,
        holder_best: leilao.holder_best,
        suitor_name: suitor.nome.clone(),
        suitor_color: suitor.cor_primaria.clone(),
        suitor_car_rating: suitor.car_strength().round().clamp(0.0, 100.0) as u8,
        current_team_name: contexto.current_team.nome.clone(),
        current_team_color: contexto.current_team.cor_primaria.clone(),
        category_label: crate::constants::categories::get_category_config(&suitor.categoria)
            .map(|c| c.nome_curto.to_string())
            .unwrap_or_else(|| suitor.categoria.clone()),
        incumbent_name: pretendente
            .deslocado_id
            .as_deref()
            .and_then(|id| contexto.drivers_by_id.get(id))
            .map(|d| d.nome.clone()),
        player_fama: contexto.player.atributos.midia.round().clamp(0.0, 100.0) as u8,
        bids: leilao.bids,
        poacher_wins: leilao.poacher_wins,
    }
}

/// Aplica a decisão do jogador sobre a quebra de contrato. `accept = true` → SAIR
/// (assina no pretendente pelo melhor lance dele, a multa vai do pretendente ao time
/// atual, o mais fraco do pretendente é dispensado limpo). `accept = false` → FICAR
/// (se o time cobriu acima do salário atual, o aumento fica no contrato). Revalida o
/// contrato que o jogador VIU — se mudou (a escada mexeu), a oferta expirou.
pub(crate) fn resolve_player_poach(
    conn: &Connection,
    offer: &PlayerPoachOffer,
    accept: bool,
    new_season_number: i32,
) -> Result<PlayerPoachOutcome, String> {
    let player = driver_queries::get_player_driver(conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let current = contract_queries::get_active_regular_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao checar contrato do jogador: {e}"))?;
    if current
        .as_ref()
        .is_none_or(|c| c.id != offer.current_contract_id)
    {
        return Ok(PlayerPoachOutcome {
            applied: false,
            left: false,
            salary: offer.current_salary,
            team_name: offer.current_team_name.clone(),
            note: rust_i18n::t!("market.poach_outcome.expired").to_string(),
        });
    }
    let current = current.unwrap();

    if !accept {
        // FICAR: se o time cobriu acima do atual, o aumento fica no contrato.
        if offer.holder_best > current.salario_anual {
            contract_queries::update_contract_salary(conn, &current.id, offer.holder_best)
                .map_err(|e| format!("Falha ao aplicar aumento de retencao do jogador: {e}"))?;
        }
        return Ok(PlayerPoachOutcome {
            applied: true,
            left: false,
            salary: offer.holder_best.max(current.salario_anual),
            team_name: offer.current_team_name.clone(),
            note: rust_i18n::t!(
                "market.poach_outcome.stayed",
                team = offer.current_team_name.as_str()
            )
            .to_string(),
        });
    }

    // SAIR: quebra de contrato de verdade.
    let Some(suitor) = team_queries::get_team_by_id(conn, &offer.suitor_team_id)
        .map_err(|e| format!("Falha ao carregar pretendente: {e}"))?
    else {
        return Ok(PlayerPoachOutcome {
            applied: false,
            left: false,
            salary: current.salario_anual,
            team_name: offer.current_team_name.clone(),
            note: rust_i18n::t!("market.poach_outcome.unavailable").to_string(),
        });
    };

    contract_queries::update_contract_status(conn, &current.id, &ContractStatus::Rescindido)
        .map_err(|e| format!("Falha ao rescindir contrato do jogador: {e}"))?;

    // Assento no pretendente: usa vaga aberta se houver; senão desloca o mais fraco
    // (recomputado AGORA, não o do momento da oferta — a escada pode ter mexido).
    let (papel, displaced_id): (TeamRole, Option<String>) = if suitor.piloto_1_id.is_none() {
        (TeamRole::Numero1, None)
    } else if suitor.piloto_2_id.is_none() {
        (TeamRole::Numero2, None)
    } else {
        let a = suitor.piloto_1_id.clone().unwrap();
        let b = suitor.piloto_2_id.clone().unwrap();
        let need = crate::fame::team_need_factor(
            crate::finance::planning::derive_budget_index_from_money(&suitor),
            suitor.reputacao,
        );
        let value_of = |id: &str| {
            driver_queries::get_driver(conn, id)
                .ok()
                .map(|d| driver_poach_value(&d, need))
                .unwrap_or(f64::MAX)
        };
        if value_of(&a) <= value_of(&b) {
            (TeamRole::Numero1, Some(a))
        } else {
            (TeamRole::Numero2, Some(b))
        }
    };

    if let Some(did) = displaced_id.as_ref() {
        if let Some(dc) = contract_queries::get_active_regular_contract_for_pilot(conn, did)
            .map_err(|e| format!("Falha ao carregar contrato do deslocado: {e}"))?
        {
            contract_queries::update_contract_status(conn, &dc.id, &ContractStatus::Rescindido)
                .map_err(|e| format!("Falha ao rescindir deslocado: {e}"))?;
        }
        // Mesma perda de vaga do poaching IA-vs-IA, só que quem ficou com o assento foi o
        // jogador. O efeito é do piloto DESLOCADO — a política do jogador não muda por
        // isso, ele não é quem perde nada aqui.
        if let Ok(mut d) = driver_queries::get_driver(conn, did) {
            d.categoria_atual = None; // agente livre LIMPO (a escada o repesca)
            if !d.is_jogador {
                crate::evolution::motivation::adjust_lost_seat_motivation(&mut d);
            }
            driver_queries::update_driver(conn, &d)
                .map_err(|e| format!("Falha ao liberar deslocado: {e}"))?;
        }
    }

    let mut contract = Contract::new(
        next_id(conn, IdType::Contract)
            .map_err(|e| format!("Falha ao gerar ID de contrato do jogador: {e}"))?,
        player.id.clone(),
        player.nome.clone(),
        suitor.id.clone(),
        suitor.nome.clone(),
        new_season_number,
        2,
        offer.poacher_best,
        papel,
        suitor.categoria.clone(),
    );
    contract.classe = suitor.classe.clone();
    contract_queries::insert_contract(conn, &contract)
        .map_err(|e| format!("Falha ao inserir novo contrato do jogador: {e}"))?;

    let mut moved = player.clone();
    moved.mover_para_categoria(Some(suitor.categoria.clone()));
    driver_queries::update_driver(conn, &moved)
        .map_err(|e| format!("Falha ao mover jogador de equipe: {e}"))?;

    // A multa: pretendente → time atual (o time que perde o jogador é indenizado).
    transfer_between_teams(conn, &suitor.id, &offer.current_team_id, offer.buyout)?;

    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao recarregar times: {e}"))?;
    let refreshed: HashMap<String, Driver> = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao recarregar pilotos: {e}"))?
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();
    sync_team_slots_from_active_regular_contracts(conn, &teams, &refreshed)?;

    Ok(PlayerPoachOutcome {
        applied: true,
        left: true,
        salary: offer.poacher_best,
        team_name: suitor.nome.clone(),
        note: rust_i18n::t!("market.poach_outcome.signed", team = suitor.nome.as_str()).to_string(),
    })
}

/// Salário da oferta ao jogador, com o prêmio de interesse ativo aplicado quando o
/// time cobiça o nome dele — usado no MESMO ponto pela listagem e pela assinatura,
/// pra o que é mostrado bater com o que é assinado.
///
/// B22 — O TETO FINANCEIRO. A fórmula-base (faixa por tier × papel × variação da
/// equipe) e o prêmio de interesse descrevem quanto o jogador VALE; nenhum dos dois
/// olha o caixa de quem paga. A proposta formal da MESMA equipe passa por
/// `calculate_offer_salary_from_money`, que fecha em `calculate_salary_ceiling` — e o
/// resultado era uma equipe em crise ofertando pela porta passiva um salário que ela
/// recusava na proposta formal. O valor devido sai daqui limitado pelo MESMO teto,
/// lido da MESMA fonte financeira da vaga (`vacancy_as_finance_team`): caixa, dívida,
/// estado financeiro e reputação. Equipe saudável não sente o teto (ele fica acima do
/// topo da fórmula); equipe pressionada tem o prêmio aparado; equipe em crise oferta
/// perto do piso.
pub(super) fn player_offer_salary_with_interest(
    vaga: &Vacancy,
    is_n1: bool,
    skill: f64,
    active_interest: bool,
) -> f64 {
    let base = player_offer_salary(vaga.category_tier, is_n1, skill, &vaga.team_id);
    let com_premio = if active_interest {
        base * crate::fame::ACTIVE_INTEREST_SALARY_PREMIUM
    } else {
        base
    };
    // `.max` no piso: o teto já nasce com ele, e sem isso um `clamp` invertido viraria
    // pânico no meio do mercado se a fonte financeira mudar de piso um dia.
    let teto = crate::finance::salary::calculate_salary_ceiling(
        &crate::market::team_ai::vacancy_as_finance_team(vaga),
    )
    .max(5_000.0);
    com_premio.clamp(5_000.0, teto)
}
