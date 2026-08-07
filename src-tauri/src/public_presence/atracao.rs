//! ATRAÇÃO DE PÚBLICO de uma equipe — quanta gente aquele carro puxa para a pista.
//!
//! É a chave de rateio da BILHETERIA (`commands::race::financas`, bloco do portão).
//! Existe porque a fama sozinha não serve de chave — ela leva temporadas para se
//! espalhar, enquanto a posição no campeonato tem espalhamento **total por construção**
//! (alguém é primeiro, alguém é último, sempre). Então o composto separa as equipes de
//! um grid já na primeira temporada, mesmo com a fama ainda comprimida.
//!
//! Não confundir com a PRESENÇA (`super::team`): aquela é a fama do lineup e continua
//! sendo o que o termo de patrocínio lê. São dois canais com causas diferentes — é a
//! fama do rosto que capta patrocinador, e é a corrida que enche arquibancada.
//!
//! Composição, na ordem do desenho (`docs/economia-redesign.md`, §3.6):
//!
//! ```text
//! atração = presença_do_lineup × competitividade  +  vínculo_local  +  história
//! ```
//!
//! - **presença do lineup** é [`super::team::derive_team_public_presence`] — a mesma
//!   grandeza que a aba My Team já mostra, sem tier e sem label;
//! - **competitividade** é MULTIPLICATIVA de propósito: um lineup famoso num carro que
//!   anda em último não enche arquibancada, e um lineup anônimo liderando enche;
//! - **vínculo local** e **história** são ADITIVOS: são motivos de ir à pista que não
//!   dependem de quem está no carro nesta temporada.
//!
//! O resultado é 0–100, contínuo, sem níveis. Não é comensurável com o atributo `midia`
//! de um piloto nem com os 6 níveis da ficha — é outra unidade, como a presença de equipe.

/// Entrada da atração de público. Tudo explícito e sem I/O: a função é pura e os
/// campos vêm de quem já tem o dado em mãos (o call site futuro da receita).
#[derive(Debug, Clone)]
pub struct TeamAudienceInput<'a> {
    /// Mídia (0–100) dos pilotos do lineup ativo. Os dois maiores é que contam.
    pub lineup_medias: &'a [f64],
    /// Posição da equipe no campeonato de construtores (1 = líder). 0 é tratado como
    /// "sem campeonato ainda" e cai no meio da tabela.
    pub posicao_campeonato: u32,
    /// Quantas equipes disputam a categoria. < 2 desliga o termo de posição.
    pub equipes_na_categoria: u32,
    /// Forma recente normalizada 0–1: 0 = nada nas últimas etapas, 1 = pontuando forte.
    /// Fica separada da posição porque campeonato é acumulado e forma é agora.
    pub forma_recente: f64,
    /// A equipe é sediada no país da pista desta etapa.
    pub equipe_local: bool,
    /// Títulos históricos da equipe (pilotos + construtores).
    pub titulos_da_equipe: i32,
}

// ── Constantes de balanceamento (tunáveis; mesa de balanceamento) ─────────────

/// Faixa do multiplicador de competitividade. O lanterna sem forma corta a atração do
/// lineup para menos da metade; o líder em forma a multiplica por mais de 1,5. É daqui
/// que vem a maior parte do espalhamento dentro de um grid.
pub const COMPETITIVIDADE_MIN: f64 = 0.45;
pub const COMPETITIVIDADE_MAX: f64 = 1.55;
/// Peso da posição no campeonato dentro da competitividade; o resto é forma recente.
/// A posição pesa mais porque é o que o público acompanha ao longo do ano.
pub const PESO_POSICAO: f64 = 0.65;
/// Bônus de VÍNCULO LOCAL: equipe da casa. Aditivo — vale mesmo para a equipe que vai
/// mal, e é o que faz uma etapa ter público diferente de outra.
pub const BONUS_EQUIPE_LOCAL: f64 = 12.0;
/// Peso e teto do termo de HISTÓRIA (côncavo na contagem de títulos): a primeira taça
/// muda o nome da equipe, a décima quinta já não.
pub const PESO_TITULOS: f64 = 3.0;
pub const TETO_TITULOS: f64 = 15.0;

/// Competitividade atual da equipe, 0–1: posição no campeonato ponderada com a forma
/// recente. Exposta separada porque é o termo que o desenho da receita vai querer
/// inspecionar sozinho — é ele que garante espalhamento total no grid.
pub fn competitividade(posicao_campeonato: u32, equipes_na_categoria: u32, forma: f64) -> f64 {
    let forma = forma.clamp(0.0, 1.0);
    let posicao_norm = if equipes_na_categoria < 2 || posicao_campeonato == 0 {
        0.5
    } else {
        let n = equipes_na_categoria as f64;
        let pos = (posicao_campeonato as f64).clamp(1.0, n);
        (n - pos) / (n - 1.0)
    };
    (PESO_POSICAO * posicao_norm + (1.0 - PESO_POSICAO) * forma).clamp(0.0, 1.0)
}

/// Multiplicador de competitividade aplicado à presença do lineup.
fn multiplicador_competitividade(competitividade: f64) -> f64 {
    COMPETITIVIDADE_MIN + competitividade.clamp(0.0, 1.0) * (COMPETITIVIDADE_MAX - COMPETITIVIDADE_MIN)
}

/// Termo de HISTÓRIA: títulos da equipe, côncavo e com teto.
fn termo_de_historia(titulos: i32) -> f64 {
    (PESO_TITULOS * (titulos.max(0) as f64).sqrt()).min(TETO_TITULOS)
}

/// Atração de público de uma equipe numa etapa, 0–100.
///
/// É o que a cota de bilheteria rateia. O bolo e o peso do piso continuam sendo da
/// receita (`finance::cashflow`); daqui sai só a grandeza que se divide.
pub fn compute_team_audience_appeal(input: &TeamAudienceInput<'_>) -> f64 {
    appeal_from_presence(
        super::team::derive_team_public_presence(input.lineup_medias),
        input.posicao_campeonato,
        input.equipes_na_categoria,
        input.forma_recente,
        input.equipe_local,
        input.titulos_da_equipe,
    )
}

/// O núcleo da atração, a partir da PRESENÇA já derivada. Existe porque quem já
/// tem a presença pré-computada da rodada (produção e harness) não deve pagar uma
/// segunda leitura do lineup só para recalcular a mesma média ponderada.
pub fn appeal_from_presence(
    presenca_do_lineup: f64,
    posicao_campeonato: u32,
    equipes_na_categoria: u32,
    forma_recente: f64,
    equipe_local: bool,
    titulos_da_equipe: i32,
) -> f64 {
    let comp = competitividade(posicao_campeonato, equipes_na_categoria, forma_recente);
    let esportivo = presenca_do_lineup.clamp(0.0, 100.0) * multiplicador_competitividade(comp);
    let local = if equipe_local { BONUS_EQUIPE_LOCAL } else { 0.0 };
    (esportivo + local + termo_de_historia(titulos_da_equipe)).clamp(0.0, 100.0)
}

// ── Adaptador de PRODUÇÃO ─────────────────────────────────────────────────────

/// Faixa da moral de equipe usada como proxy de FORMA RECENTE. A moral é movida
/// pelo resultado da temporada e pela treta interna (`finance::morale`), então é o
/// sinal de "como esta equipe está agora" que a rodada tem em mãos sem uma segunda
/// consulta ao banco.
const MORAL_MIN: f64 = 0.80;
const MORAL_MAX: f64 = 1.30;

/// Atração de público de uma equipe com os dados que a RODADA já tem em mãos.
///
/// É o adaptador entre [`compute_team_audience_appeal`] (pura, sem I/O) e a
/// produção: monta a entrada a partir do `Team` persistido, da presença do lineup
/// já pré-computada na rodada e do país da pista da etapa. Não consulta o banco.
///
/// - **posição no campeonato** vem de fora (ver [`posicoes_por_pontos`]);
/// - **forma recente** vem da moral, normalizada da faixa em que ela vive;
/// - **vínculo local** compara `pais_sede` com o país da pista;
/// - **história** soma os títulos de pilotos e de construtores.
pub fn team_audience_appeal_in_round(
    team: &crate::models::team::Team,
    lineup_presence_medias: &[f64],
    posicao_campeonato: u32,
    equipes_na_categoria: u32,
    pais_da_pista: &str,
) -> f64 {
    team_audience_appeal_from_presence(
        team,
        super::team::derive_team_public_presence(lineup_presence_medias),
        posicao_campeonato,
        equipes_na_categoria,
        pais_da_pista,
    )
}

/// Igual a [`team_audience_appeal_in_round`], para quem já tem a presença da rodada.
///
/// A POSIÇÃO vem de fora e não de `team.temp_posicao`, e isso não é preferência de
/// estilo: `temp_posicao` é zerado no começo da temporada
/// (`reset_team_season_stats`) e só volta a ser escrito no arquivamento do fim do
/// ano. Lê-lo aqui daria 0 — "sem campeonato" — para o grid inteiro durante a
/// temporada toda, e a competitividade cairia no neutro justamente no período em
/// que a bilheteria é cobrada. Ver [`posicoes_por_pontos`].
pub fn team_audience_appeal_from_presence(
    team: &crate::models::team::Team,
    presenca_do_lineup: f64,
    posicao_campeonato: u32,
    equipes_na_categoria: u32,
    pais_da_pista: &str,
) -> f64 {
    let forma = ((team.morale - MORAL_MIN) / (MORAL_MAX - MORAL_MIN)).clamp(0.0, 1.0);
    let local = !pais_da_pista.trim().is_empty()
        && team.pais_sede.trim().eq_ignore_ascii_case(pais_da_pista.trim());
    appeal_from_presence(
        presenca_do_lineup,
        posicao_campeonato,
        equipes_na_categoria,
        forma,
        local,
        team.historico_titulos_pilotos + team.historico_titulos_construtores,
    )
}

/// Classificação de construtores VIVA da temporada corrente, por categoria: 1 = líder.
///
/// Ordena por `stats_pontos` (o contador que a rodada atualiza de verdade), com o id
/// como desempate para o resultado não depender da ordem de leitura do banco.
///
/// **Categoria inteira zerada devolve 0 para todos**, que a competitividade lê como
/// "sem campeonato ainda" e trata como meio de tabela. Sem essa guarda, na primeira
/// etapa do ano — quando ninguém pontuou — o desempate por id carimbaria alguém como
/// líder e daria a ele uma fatia maior da bilheteria por ter o nome mais no começo do
/// alfabeto. O empate real é empate, não uma ordem.
pub fn posicoes_por_pontos(
    equipes: &[crate::models::team::Team],
) -> std::collections::HashMap<String, u32> {
    let mut por_categoria: std::collections::HashMap<&str, Vec<&crate::models::team::Team>> =
        std::collections::HashMap::new();
    for equipe in equipes {
        por_categoria
            .entry(equipe.categoria.as_str())
            .or_default()
            .push(equipe);
    }
    let mut out = std::collections::HashMap::new();
    for (_, mut grupo) in por_categoria {
        if grupo.iter().all(|e| e.stats_pontos <= 0) {
            for equipe in grupo {
                out.insert(equipe.id.clone(), 0);
            }
            continue;
        }
        grupo.sort_by(|a, b| {
            b.stats_pontos
                .cmp(&a.stats_pontos)
                .then_with(|| a.id.cmp(&b.id))
        });
        for (i, equipe) in grupo.iter().enumerate() {
            out.insert(equipe.id.clone(), (i + 1) as u32);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entrada(medias: &[f64], pos: u32, forma: f64) -> TeamAudienceInput<'_> {
        TeamAudienceInput {
            lineup_medias: medias,
            posicao_campeonato: pos,
            equipes_na_categoria: 12,
            forma_recente: forma,
            equipe_local: false,
            titulos_da_equipe: 0,
        }
    }

    #[test]
    fn competitividade_tem_espalhamento_total() {
        // O motivo de o termo existir: primeiro e último ocupam as pontas da faixa,
        // sempre, sem depender de a fama já ter se espalhado.
        let lider = competitividade(1, 12, 1.0);
        let lanterna = competitividade(12, 12, 0.0);
        assert!((lider - 1.0).abs() < 1e-9, "líder em forma satura: {lider}");
        assert!(lanterna.abs() < 1e-9, "lanterna sem forma zera: {lanterna}");
    }

    #[test]
    fn competitividade_sem_campeonato_cai_no_meio() {
        // Antes da primeira etapa não há posição; não pode virar nem líder nem lanterna.
        let neutro = competitividade(0, 12, 0.5);
        assert!((neutro - 0.5).abs() < 1e-9, "neutro={neutro}");
        assert!((competitividade(1, 1, 0.5) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn lineup_famoso_em_carro_ruim_perde_para_lineup_anonimo_na_ponta() {
        // A competitividade é multiplicativa justamente para isto: fama parada não
        // enche arquibancada.
        let famoso_lanterna = compute_team_audience_appeal(&entrada(&[85.0, 70.0], 12, 0.0));
        let anonimo_lider = compute_team_audience_appeal(&entrada(&[35.0, 30.0], 1, 1.0));
        assert!(
            anonimo_lider > famoso_lanterna,
            "anônimo líder={anonimo_lider}, famoso lanterna={famoso_lanterna}"
        );
    }

    #[test]
    fn posicao_no_campeonato_espalha_o_grid_com_fama_comprimida() {
        // O cenário medido no save: TODO o grid com mídia ~40 ± 3. Mesmo assim a atração
        // tem que separar a primeira equipe da última.
        let melhor = compute_team_audience_appeal(&entrada(&[43.0, 40.0], 1, 0.9));
        let pior = compute_team_audience_appeal(&entrada(&[40.0, 37.0], 12, 0.1));
        assert!(
            melhor - pior > 25.0,
            "espalhamento insuficiente: melhor={melhor}, pior={pior}"
        );
    }

    #[test]
    fn vinculo_local_e_historia_somam_sem_depender_da_temporada() {
        let base = entrada(&[40.0, 30.0], 8, 0.3);
        let neutra = compute_team_audience_appeal(&base);

        let mut local = base.clone();
        local.equipe_local = true;
        assert!(
            (compute_team_audience_appeal(&local) - neutra - BONUS_EQUIPE_LOCAL).abs() < 1e-9
        );

        let mut tradicional = base.clone();
        tradicional.titulos_da_equipe = 9;
        assert!(compute_team_audience_appeal(&tradicional) > neutra);
    }

    #[test]
    fn historia_e_concava_e_tem_teto() {
        let primeiro = termo_de_historia(1) - termo_de_historia(0);
        let decimo = termo_de_historia(10) - termo_de_historia(9);
        assert!(primeiro > decimo, "primeiro={primeiro}, décimo={decimo}");
        assert_eq!(termo_de_historia(1000), TETO_TITULOS);
        assert_eq!(termo_de_historia(-5), 0.0);
    }

    #[test]
    fn resultado_fica_sempre_na_faixa_0_100() {
        let extremo = TeamAudienceInput {
            lineup_medias: &[100.0, 100.0],
            posicao_campeonato: 1,
            equipes_na_categoria: 12,
            forma_recente: 1.0,
            equipe_local: true,
            titulos_da_equipe: 40,
        };
        let vazio = TeamAudienceInput {
            lineup_medias: &[],
            posicao_campeonato: 12,
            equipes_na_categoria: 12,
            forma_recente: 0.0,
            equipe_local: false,
            titulos_da_equipe: 0,
        };
        assert_eq!(compute_team_audience_appeal(&extremo), 100.0);
        assert_eq!(compute_team_audience_appeal(&vazio), 0.0);
    }

    fn equipe_de_producao() -> crate::models::team::Team {
        let mut team = crate::models::team::placeholder_team_from_db(
            "T001".to_string(),
            "Equipe".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.pais_sede = "Brasil".to_string();
        team.temp_posicao = 6;
        team.morale = 1.05;
        team.historico_titulos_pilotos = 2;
        team.historico_titulos_construtores = 1;
        team
    }

    #[test]
    fn adaptador_de_producao_le_posicao_moral_e_sede() {
        // O adaptador é o que a rodada chama; ele tem que ler os campos certos do
        // `Team` em vez de exigir uma segunda consulta ao banco.
        let mut team = equipe_de_producao();
        let base = team_audience_appeal_from_presence(&team, 45.0, 6, 12, "Itália");

        // Sede batendo com o país da pista → bônus local, e só ele.
        let em_casa = team_audience_appeal_from_presence(&team, 45.0, 6, 12, "Brasil");
        assert!((em_casa - base - BONUS_EQUIPE_LOCAL).abs() < 1e-9);
        // Comparação de país é indiferente a caixa e a espaço em volta.
        assert!(
            (team_audience_appeal_from_presence(&team, 45.0, 6, 12, "  brasil ") - em_casa).abs()
                < 1e-9
        );
        // Pista sem país conhecido não vale vínculo local.
        assert!((team_audience_appeal_from_presence(&team, 45.0, 6, 12, "") - base).abs() < 1e-9);

        // Subir no campeonato e melhorar a moral aumentam a atração.
        let lider = team_audience_appeal_from_presence(&team, 45.0, 1, 12, "Itália");
        assert!(lider > base, "líder={lider}, meio={base}");
        team.morale = 1.30;
        assert!(team_audience_appeal_from_presence(&team, 45.0, 1, 12, "Itália") > lider);

        // E `temp_posicao` NÃO é lido: ele fica zerado a temporada inteira, então
        // deixá-lo entrar aqui devolveria o grid ao neutro.
        team.temp_posicao = 1;
        assert!(
            (team_audience_appeal_from_presence(&team, 45.0, 12, 12, "Itália")
                - team_audience_appeal_from_presence(&team, 45.0, 12, 12, "Itália"))
            .abs()
                < 1e-9
        );
        assert!(
            team_audience_appeal_from_presence(&team, 45.0, 12, 12, "Itália")
                < team_audience_appeal_from_presence(&team, 45.0, 1, 12, "Itália"),
            "quem manda é o parâmetro, não o campo do banco"
        );
    }

    #[test]
    fn posicoes_saem_dos_pontos_e_nao_do_campo_zerado() {
        // `temp_posicao` é zerado no início da temporada e só reescrito no
        // arquivamento; a classificação viva tem que sair de `stats_pontos`.
        let mut lider = equipe_de_producao();
        lider.id = "A".to_string();
        lider.stats_pontos = 120;
        lider.temp_posicao = 0;
        let mut meio = equipe_de_producao();
        meio.id = "B".to_string();
        meio.stats_pontos = 60;
        let mut lanterna = equipe_de_producao();
        lanterna.id = "C".to_string();
        lanterna.stats_pontos = 5;
        // Outra categoria: não pode disputar posição com as de cima.
        let mut outra = equipe_de_producao();
        outra.id = "D".to_string();
        outra.categoria = "gt4".to_string();
        outra.stats_pontos = 1;

        let pos = posicoes_por_pontos(&[lanterna, lider, outra, meio]);
        assert_eq!(pos.get("A"), Some(&1));
        assert_eq!(pos.get("B"), Some(&2));
        assert_eq!(pos.get("C"), Some(&3));
        assert_eq!(pos.get("D"), Some(&1), "cada categoria tem o seu líder");
    }

    #[test]
    fn antes_da_primeira_etapa_ninguem_e_lider() {
        // Todo mundo com zero ponto é EMPATE, não uma ordem: carimbar o primeiro do
        // alfabeto como líder lhe daria uma fatia maior da bilheteria de graça.
        let ids = ["T3", "T1", "T2"];
        let equipes: Vec<crate::models::team::Team> = ids
            .iter()
            .map(|id| {
                let mut t = equipe_de_producao();
                t.id = id.to_string();
                t.stats_pontos = 0;
                t
            })
            .collect();
        let pos = posicoes_por_pontos(&equipes);
        assert!(pos.values().all(|p| *p == 0), "empate geral → sem posição");
        // E "sem posição" é o meio da tabela, igual para todos.
        let atracoes: Vec<f64> = equipes
            .iter()
            .map(|e| team_audience_appeal_from_presence(e, 45.0, pos[&e.id], 3, ""))
            .collect();
        assert!(atracoes.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9));

        // Bastou uma equipe pontuar para a classificação existir de novo.
        let mut com_pontos = equipes.clone();
        com_pontos[1].stats_pontos = 10;
        let pos = posicoes_por_pontos(&com_pontos);
        assert_eq!(pos.get("T1"), Some(&1), "quem pontuou lidera");
    }

    #[test]
    fn adaptador_bate_com_a_funcao_pura() {
        // As duas portas de entrada não podem divergir: a que recebe médias deriva a
        // presença e cai exatamente na que recebe presença.
        let team = equipe_de_producao();
        let medias = [70.0, 40.0];
        let presenca = super::super::team::derive_team_public_presence(&medias);
        assert!(
            (team_audience_appeal_in_round(&team, &medias, 6, 12, "Brasil")
                - team_audience_appeal_from_presence(&team, presenca, 6, 12, "Brasil"))
            .abs()
                < 1e-9
        );
    }

    #[test]
    fn moral_fora_da_faixa_nao_estoura_a_forma() {
        // Moral é um campo do banco e saves antigos podem trazer qualquer coisa; a
        // forma continua presa em 0..1.
        let mut team = equipe_de_producao();
        team.morale = -5.0;
        let piso = team_audience_appeal_from_presence(&team, 45.0, 6, 12, "Itália");
        team.morale = 99.0;
        let teto = team_audience_appeal_from_presence(&team, 45.0, 6, 12, "Itália");
        assert!(piso < teto);
        assert!((0.0..=100.0).contains(&piso) && (0.0..=100.0).contains(&teto));
    }

    #[test]
    fn e_monotonica_em_cada_eixo() {
        let base = entrada(&[50.0, 40.0], 6, 0.5);
        let mais_fama = compute_team_audience_appeal(&entrada(&[70.0, 40.0], 6, 0.5));
        let melhor_posicao = compute_team_audience_appeal(&entrada(&[50.0, 40.0], 2, 0.5));
        let melhor_forma = compute_team_audience_appeal(&entrada(&[50.0, 40.0], 6, 0.9));
        let referencia = compute_team_audience_appeal(&base);
        assert!(mais_fama > referencia);
        assert!(melhor_posicao > referencia);
        assert!(melhor_forma > referencia);
    }
}
