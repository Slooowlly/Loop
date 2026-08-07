//! Dinâmica da FAMA (`midia`) modulada pelo CARISMA. **Lógica pura.**
//!
//! Dois eixos, travados com o user:
//! - **Carisma** (atributo estável da pessoa): magnetismo/qualidade de estrela.
//! - **Fama / midia** (estoque 0–100): atenção pública acumulada, sobe e desce.
//!
//! O carisma governa TRÊS taxas sobre a fama:
//! 1. **Ganho** (bom resultado): carismático converte conquista em fama melhor.
//! 2. **Amortecedor** (mau resultado): carismático quase não perde fama (teflon) —
//!    a estrela pode terminar em último e mal sentir.
//! 3. **Decaimento passivo** (tempo/sumiço): fama de carismático é grudenta; a de
//!    um piloto sem graça esvai rápido.
//!
//! Emerge o que o user pediu: estrela ganha rápido e não cai; apagado ganha só com
//! resultado forte e sangra depressa. Números na mesa de balanceamento (tunáveis aqui).

/// Multiplicador do GANHO de fama (delta ≥ 0) pelo carisma: 0.6× (apagado) a
/// 1.4× (estrela). Carismático capitaliza mais cada conquista.
pub fn fame_gain_mult(carisma: f64) -> f64 {
    0.6 + 0.8 * (carisma.clamp(0.0, 100.0) / 100.0)
}

/// Multiplicador da PERDA de fama (delta < 0) pelo carisma: 1.4× (apagado, sangra)
/// a 0.4× (estrela, quase não sente). É o amortecedor do "terminou em último".
pub fn fame_loss_mult(carisma: f64) -> f64 {
    1.4 - (carisma.clamp(0.0, 100.0) / 100.0)
}

/// Multiplicador do DECAIMENTO passivo da fama (tempo/sumiço) pelo carisma: 1.5×
/// (apagado, esvai rápido) a 0.5× (estrela, grudenta).
pub fn fame_decay_mult(carisma: f64) -> f64 {
    1.5 - (carisma.clamp(0.0, 100.0) / 100.0)
}

/// Aplica o carisma a um delta de fama BRUTO. Ganho e perda usam multiplicadores
/// distintos — o amortecedor só age no lado negativo, o ganho só no positivo.
pub fn apply_carisma_to_fame_delta(raw_delta: f64, carisma: f64) -> f64 {
    if raw_delta >= 0.0 {
        raw_delta * fame_gain_mult(carisma)
    } else {
        raw_delta * fame_loss_mult(carisma)
    }
}

// ── Constantes de balanceamento (tunáveis; mesa de balanceamento) ─────────────
/// Piso BASE da fama no decaimento passivo — o piso de quem não fez nada ainda.
/// Alinhado ao limiar "Baixa/Discreto" do mercado. O piso EFETIVO de um piloto é
/// este mais o pedigree dele: ver [`personal_fame_floor`].
pub const FAME_DECAY_FLOOR: f64 = 25.0;
/// Fração base da distância até o piso perdida por corrida (antes do carisma).
/// A taxa é o divisor do espalhamento: a fama de equilíbrio de um piloto é
/// `piso + ganho_por_corrida / taxa`, então baixá-la ESPALHA a população na mesma
/// proporção. Calibrada em `public_presence::medicao` contra o alvo de σ ≥ 15.
pub const FAME_DECAY_BASE_RATE: f64 = 0.021;
/// Deriva de carisma por marco de carreira — PEQUENA de propósito (carisma é
/// estável; só uma vida inteira de drama move de verdade).
pub const CARISMA_DRIFT_INCIDENT: f64 = 0.4; // vilão/incidente notável — "drama vende"
pub const CARISMA_DRIFT_COMEBACK: f64 = 0.5; // remontada de muitas posições
pub const CARISMA_DRIFT_BIG_WIN: f64 = 0.6; // vitória num evento grande
/// Posições ganhadas (grid − chegada) a partir das quais conta como remontada.
pub const COMEBACK_MIN_POSITIONS: i32 = 6;

// ── PISO PESSOAL: o que a carreira sedimentou e o tempo não leva ──────────────
//
// O piso global de 25 era a trava da compressão: um campeão com um ano ruim decaía
// para o mesmo lugar de um anônimo, então toda a população convergia para a mesma
// faixa e a presença pública das equipes ficava idêntica dentro do grid. O piso
// passa a ser PESSOAL — quem construiu nome retém nome.
//
// Os três termos são CÔNCAVOS de propósito (raiz, não linear): o primeiro título
// muda muito mais do que o sexto, e ninguém escala o piso acumulando estatística.

/// Tier mínimo da escada que conta como temporada "na elite" para o piso pessoal
/// (gt3 = 4, lmp2 = 5, endurance = 6). Abaixo disso a temporada não sedimenta nome:
/// ser conhecido na Rookie é ser conhecido por ninguém.
pub const FAME_ELITE_TIER_MIN: u8 = 4;
/// Teto do piso pessoal. Fica DENTRO da faixa "Estrela" (70–87) e nunca alcança
/// "Ídolo": nem a maior lenda passa a ter fama de Ídolo garantida por decreto.
pub const FAME_FLOOR_MAX: f64 = 78.0;
/// Peso do termo de TÍTULOS no piso pessoal (multiplica a raiz da contagem).
pub const FAME_FLOOR_TITLE_WEIGHT: f64 = 12.0;
/// Peso do termo de VITÓRIAS de carreira no piso pessoal (multiplica a raiz).
pub const FAME_FLOOR_WIN_WEIGHT: f64 = 1.6;
/// Peso e teto do termo de TEMPORADAS NA ELITE — longevidade no topo é nome, mas
/// é o termo mais fraco dos três (estar lá não é vencer lá).
pub const FAME_FLOOR_ELITE_SEASON_WEIGHT: f64 = 1.0;
pub const FAME_FLOOR_ELITE_SEASON_CAP: f64 = 10.0;

/// Piso PESSOAL do decaimento da fama: [`FAME_DECAY_FLOOR`] mais o que a carreira
/// sedimentou (títulos, vitórias, temporadas na elite), preso a [`FAME_FLOOR_MAX`].
///
/// Ordem de grandeza: estreante 25; piloto sólido sem título (12 vitórias, 6
/// temporadas na elite) ~36; campeão (3 títulos, 40 vitórias, 10 temporadas) ~66;
/// lenda satura em 78.
pub fn personal_fame_floor(titulos: u32, vitorias_carreira: u32, temporadas_elite: u32) -> f64 {
    let por_titulos = FAME_FLOOR_TITLE_WEIGHT * (titulos as f64).sqrt();
    let por_vitorias = FAME_FLOOR_WIN_WEIGHT * (vitorias_carreira as f64).sqrt();
    let por_elite =
        (FAME_FLOOR_ELITE_SEASON_WEIGHT * temporadas_elite as f64).min(FAME_FLOOR_ELITE_SEASON_CAP);
    (FAME_DECAY_FLOOR + por_titulos + por_vitorias + por_elite).clamp(FAME_DECAY_FLOOR, FAME_FLOOR_MAX)
}

/// Última posição de chegada que ainda rende fama, dado o tamanho do campo. A faixa de
/// "aparece" é RELATIVA ao grid, não absoluta: P8 num grid de 28 é meio de tabela, mas
/// num grid de 12 é fundo. Uma faixa fixa até P10 premiava 10 dos 12 carros da Rookie —
/// todo mundo ganhando é o mesmo que ninguém ganhando, a população volta a comprimir.
/// Presa entre P5 e P10 para não degenerar nos extremos.
pub fn fame_visibility_last_position(grid_size: usize) -> i32 {
    ((grid_size as f64 * FAME_VISIBILITY_SHARE).round() as i32).clamp(5, 10)
}

/// Fração do grid que ainda "aparece" — pouco mais de um terço do campo.
pub const FAME_VISIBILITY_SHARE: f64 = 0.40;

// ── A escada de ganho por POSIÇÃO DE CHEGADA ─────────────────────────────────────────
//
// **Uma escada só, para o jogador e para a IA.** Havia duas, escritas em lugares
// diferentes, e elas não batiam: o bloco player-facing de `commands::race::financas`
// pagava +2,0 por pódio e +1,0 por top-5 onde `event_interest::public_impact` pagava
// +1,0 e +0,5 pelo MESMO resultado na MESMA corrida.
//
// A divergência era REAL mas TROCAVA DE SINAL, e foi medida antes de ser consertada
// (`public_presence::medicao::medir_simetria_da_escada_de_fama`): fora do pódio o jogador
// recebia de 1,2× a 3,0× o que a IA recebia; na VITÓRIA de um evento grande ele recebia
// 0,62×, porque a base da vitória já era igual e o multiplicador de interesse do mundo
// (2,5) passava o dele (1,60). No saldo de uma temporada de GT3 o jogador recebia **0,72×**
// o que o mundo recebe pelos mesmos resultados — menos, não mais.
//
// Ou seja: esta divergência não era a causa da saturação da fama do jogador (essa é
// aritmética de topo de escala, e está em `apply_fame_gain`). Ela é consertada aqui porque
// duas tabelas discordando sobre o mesmo evento são um defeito de qualquer modo.
//
// Os valores que sobreviveram são os do mundo, não os do jogador: são eles que foram
// calibrados contra o espalhamento da população (σ ≥ 15 por categoria), e um número
// calibrado contra uma medição ganha de um número escrito à mão.

/// Vitória. O único degrau em que as duas escadas já concordavam.
pub const FAME_FINISH_WIN: f64 = 3.0;
/// P2–P3.
pub const FAME_FINISH_PODIUM: f64 = 1.0;
/// P4–P5: aparece no resumo, não na manchete.
pub const FAME_FINISH_TOP5: f64 = 0.5;
/// Do sexto até a última posição visível ([`fame_visibility_last_position`]).
pub const FAME_FINISH_TOP10: f64 = 0.2;

/// Terminar fora da faixa visível. **Só o jogador tem lado negativo**, e é uma assimetria
/// declarada, não um resto: para a IA, "não apareceu" é a ausência de impacto mais o
/// decaimento passivo, que o jogador também sofre. O assento do jogador é o único que o
/// jogo narra corrida a corrida, e um fim de semana ruim que não custa nada não é um fim
/// de semana ruim.
///
/// Era −1,0 quando o lado positivo do jogador era o dobro do do mundo. Como o lado
/// positivo caiu pela metade ao virar escada única, o negativo cai junto: manter −1,0
/// teria dobrado o PESO relativo da punição sem que ninguém tivesse decidido isso.
pub const FAME_FINISH_FORA: f64 = -0.5;
/// Abandono do jogador. O dobro de terminar fora, pelo mesmo motivo: quebrar é mais
/// visível que ser esquecido. Era −2,0, pela mesma proporção.
pub const FAME_FINISH_DNF: f64 = -1.0;

/// Ganho-base de fama por posição de chegada, antes de qualquer modulação (interesse do
/// evento, tier da categoria, carisma). `ultima_visivel` vem de
/// [`fame_visibility_last_position`] — a faixa que "aparece" é relativa ao tamanho do campo.
pub fn fame_finish_base_delta(posicao: i32, dnf: bool, ultima_visivel: i32) -> f64 {
    if dnf {
        return FAME_FINISH_DNF;
    }
    match posicao {
        1 => FAME_FINISH_WIN,
        2..=3 => FAME_FINISH_PODIUM,
        4..=5 => FAME_FINISH_TOP5,
        p if p <= ultima_visivel => FAME_FINISH_TOP10,
        _ => FAME_FINISH_FORA,
    }
}

/// Escala do GANHO de fama pelo TIER da categoria (0 Rookie → 6 Endurance). Um
/// pódio no Endurance vira nome; o mesmo pódio na Rookie quase não sai do paddock.
/// É o que faz a pirâmide ter inclinação em vez de todo degrau ter a mesma fama.
/// **Não** mexe na faixa 0–100 nem nos 6 níveis da ficha — só na velocidade de subir.
pub fn fame_category_tier_mult(tier: u8) -> f64 {
    0.85 + 0.10 * tier.min(6) as f64
}

// ── Retorno decrescente no topo ──────────────────────────────────────────────────────
//
// **Uma progressão que morre na metade não é progressão.** Medido: um jogador de perfil
// "Vencedor" e carisma médio cruzava os 87 pontos de Ídolo na 8ª de 20 temporadas — ainda
// na GT4 — e passava as doze seguintes cravado entre 97 e 99. Da metade da carreira em
// diante a fama parava de dizer qualquer coisa: ganhar o campeonato do Endurance movia o
// número tanto quanto um top-10 na Rookie, ou seja, nada.
//
// A causa NÃO era a escada player-facing render mais que a do mundo — essa hipótese foi
// medida e refutada (`public_presence::medicao::medir_simetria_da_escada_de_fama`): no
// saldo de uma temporada o jogador recebia 0,72× o que o mundo recebe, não mais.
//
// A causa é aritmética, e vale para a IA também: acima de ~90 o ganho de uma vitória ainda
// é grande em pontos absolutos, e nada no modelo tornava o último pedaço da escala mais
// caro que o primeiro. O que distingue o jogador é que ele CONCENTRA num assento só os
// resultados que o mundo espalha por 28 carros.
//
// Aqui ele fica. Subir de 90 para 95 passa a custar muito mais que subir de 50 para 55, e
// os últimos pontos ficam praticamente inalcançáveis — que é o que "topo" quer dizer.
// **Só a subida é amortecida**: perder fama continua custando o preço cheio, senão o topo
// viraria um platô ainda mais grudento.

/// Ponto da escala em que a fama entra em retorno decrescente. Abaixo dele nada muda —
/// a subida da base ao "nome forte" continua com o custo de sempre.
pub const FAME_SATURATION_KNEE: f64 = 70.0;

/// Expoente da curva acima do joelho. Quanto maior, mais íngreme fica o último trecho.
pub const FAME_SATURATION_EXP: f64 = 1.6;

/// Quanto de um GANHO de fama sobrevive, dada a fama atual. 1,0 abaixo do joelho, 0,0 em 100.
pub fn fame_headroom_mult(fama_atual: f64) -> f64 {
    fame_headroom_mult_com(fama_atual, FAME_SATURATION_KNEE, FAME_SATURATION_EXP)
}

/// Variante com os parâmetros explícitos. Existe para o harness varrer a curva sem
/// recompilar: onde o Ídolo cai na carreira é decisão de design, e decisão de design se
/// mede antes de escrever.
pub fn fame_headroom_mult_com(fama_atual: f64, joelho: f64, expoente: f64) -> f64 {
    if fama_atual <= joelho {
        return 1.0;
    }
    let restante = ((100.0 - fama_atual) / (100.0 - joelho)).clamp(0.0, 1.0);
    restante.powf(expoente)
}

/// Aplica um delta de fama já modulado por carisma, com retorno decrescente no topo.
/// É o único caminho por onde a fama de alguém sobe — jogador ou IA.
pub fn apply_fame_gain(atual: f64, delta: f64) -> f64 {
    apply_fame_gain_com(atual, delta, FAME_SATURATION_KNEE, FAME_SATURATION_EXP)
}

/// Variante com os parâmetros explícitos, para a varredura.
pub fn apply_fame_gain_com(atual: f64, delta: f64, joelho: f64, expoente: f64) -> f64 {
    let efetivo = if delta > 0.0 {
        delta * fame_headroom_mult_com(atual, joelho, expoente)
    } else {
        delta
    };
    (atual + efetivo).clamp(0.0, 100.0)
}

/// Um passo de DECAIMENTO passivo da fama rumo a um piso, escalado pelo carisma
/// (carismático decai mais devagar). `base_rate` = fração base da distância até o
/// piso percorrida por passo (ex.: por corrida). Nunca desce abaixo do piso nem
/// sobe (só decai).
pub fn decay_fame_toward(current: f64, floor: f64, base_rate: f64, carisma: f64) -> f64 {
    if current <= floor {
        return current;
    }
    let rate = (base_rate * fame_decay_mult(carisma)).clamp(0.0, 1.0);
    current - (current - floor) * rate
}

// ── FASE 2a: fama como VALOR COMERCIAL no mercado ─────────────────────────────
//
// A fama vira "segunda moeda" que o time AGE em cima ao contratar: um piloto
// famoso projeta patrocínio (ver Chunk B), então vale dinheiro além do mérito
// esportivo. O peso desse valor escala com a NECESSIDADE do time (pobre pesa
// alto — precisa da grana; dinastia pesa baixo — quer resultado).

/// Valor comercial da fama em "unidades de score" — os MESMOS 6 níveis da ficha
/// (Anônimo→Ídolo), com escalada CONVEXA: o topo pesa muito mais que proporcional
/// (um Ídolo é desproporcionalmente valioso). Some ao skill no desempate da
/// contratação, depois de ponderado pela necessidade do time.
pub fn fame_commercial_units(fama: f64) -> f64 {
    let fama = fama.clamp(0.0, 100.0);
    if fama <= 15.0 {
        0.0
    } else if fama <= 30.0 {
        3.0
    } else if fama <= 50.0 {
        8.0
    } else if fama <= 70.0 {
        16.0
    } else if fama <= 87.0 {
        30.0
    } else {
        55.0
    }
}

/// Fator de NECESSIDADE do time pelo dinheiro da fama, de `strength` financeira.
/// Vai de [`TEAM_NEED_MIN`] (dinastia rica, quase ignora fama) a [`TEAM_NEED_MAX`]
/// (time carente, pesa a fama pesado). `budget_index` e `reputacao` são 0–100.
pub fn team_need_factor(budget_index: f64, reputacao: f64) -> f64 {
    let strength =
        (0.6 * budget_index.clamp(0.0, 100.0) + 0.4 * reputacao.clamp(0.0, 100.0)) / 100.0;
    TEAM_NEED_MAX - strength * (TEAM_NEED_MAX - TEAM_NEED_MIN)
}

/// Piso/teto do fator de necessidade. Dinastia rica: 0.25 (fama quase irrelevante);
/// time carente: 1.15 (fama pode superar a velocidade num candidato elegível).
pub const TEAM_NEED_MIN: f64 = 0.25;
pub const TEAM_NEED_MAX: f64 = 1.15;
/// Prêmio salarial da oferta de um time com interesse ativo (apelo comercial paga
/// mais pra fisgar o nome). +30% sobre a oferta normal.
pub const ACTIVE_INTEREST_SALARY_PREMIUM: f64 = 1.30;

// ── INTERESSE ATIVO visível ao JOGADOR — DECOPLADO da economia da IA ──────────
//
// A economia acima (need_factor) é da IA: no mercado, é o time CARENTE que valoriza
// a fama (patrocínio). Mas mostrar isso ao jogador confunde — "essa equipe me quer"
// lê como "essa equipe é boa", quando na verdade é a mais pobre. Então o destaque
// que o JOGADOR vê (badge + N1 + prêmio + e-mail) é dos POUCOS MELHORES times, e
// quantos deles escala com a fama. Poucos, e os melhores.

/// Quantos dos MELHORES times cortejam o jogador, pela fama. Escala pelos níveis da
/// ficha: até Conhecido→0, Nome forte→1, Estrela→2, Ídolo→3. Poucos de propósito.
pub fn active_interest_team_count(fama: f64) -> usize {
    let fama = fama.clamp(0.0, 100.0);
    if fama <= 50.0 {
        0 // Anônimo / Discreto / Conhecido — ninguém cobiça o nome ainda
    } else if fama <= 70.0 {
        1 // Nome forte — o melhor time da categoria
    } else if fama <= 87.0 {
        2 // Estrela — os 2 melhores
    } else {
        3 // Ídolo — os 3 melhores
    }
}

/// Qualidade de um time para eleger quem cobiça o astro (os MELHORES primeiro):
/// prestígio (reputação) + carro + pedigree histórico de títulos. Escala ~0–100+.
pub fn team_prestige_quality(reputacao: f64, car_performance: f64, historic_titles: i32) -> f64 {
    reputacao.clamp(0.0, 100.0)
        + car_performance.clamp(0.0, 100.0) * 0.6
        + historic_titles.max(0) as f64 * 4.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ganho_estrela_maior_que_apagado() {
        assert!(fame_gain_mult(85.0) > fame_gain_mult(25.0));
        // Estrela converte acima de 1×, apagado abaixo.
        assert!(fame_gain_mult(85.0) > 1.0 && fame_gain_mult(25.0) < 1.0);
    }

    #[test]
    fn estrela_quase_nao_perde_terminando_em_ultimo() {
        // Mesma perda-base −3: estrela amortece pra bem menos que o apagado.
        let perda_estrela = apply_carisma_to_fame_delta(-3.0, 85.0);
        let perda_apagado = apply_carisma_to_fame_delta(-3.0, 25.0);
        assert!(
            perda_estrela > perda_apagado,
            "estrela perde menos (menos negativo)"
        );
        assert!(
            perda_estrela.abs() < 2.0,
            "estrela mal sente: {perda_estrela}"
        );
    }

    #[test]
    fn apagado_ganha_no_top5_mas_menos() {
        // Ganho-base +2 (top 5): apagado ganha, mas menos que 2.
        let ganho = apply_carisma_to_fame_delta(2.0, 25.0);
        assert!(ganho > 0.0 && ganho < 2.0, "ganha menos: {ganho}");
    }

    #[test]
    fn apagado_decai_mais_rapido_que_estrela() {
        let dec_apagado = decay_fame_toward(80.0, 30.0, 0.05, 25.0);
        let dec_estrela = decay_fame_toward(80.0, 30.0, 0.05, 85.0);
        // Ambos caem, mas o apagado cai mais (fica com menos fama).
        assert!(
            dec_apagado < dec_estrela,
            "apagado={dec_apagado}, estrela={dec_estrela}"
        );
        assert!(dec_estrela < 80.0, "estrela ainda decai um pouco");
    }

    #[test]
    fn decaimento_nunca_passa_do_piso() {
        // Já no piso (ou abaixo) não mexe.
        assert_eq!(decay_fame_toward(30.0, 30.0, 0.5, 25.0), 30.0);
        assert_eq!(decay_fame_toward(20.0, 30.0, 0.5, 25.0), 20.0);
        // Decaimento de um passo não ultrapassa o piso.
        let d = decay_fame_toward(31.0, 30.0, 1.0, 0.0); // rate satura em 1.0
        assert!(d >= 30.0, "não passou do piso: {d}");
    }

    #[test]
    fn ganho_e_amortecedor_sao_monotonicos_no_carisma() {
        // Faixas com folga de ponto flutuante (0.6+0.8 = 1.4000…1).
        const EPS: f64 = 1e-9;
        for c in [0.0, 30.0, 60.0, 100.0] {
            assert!((0.6 - EPS..=1.4 + EPS).contains(&fame_gain_mult(c)));
            assert!((0.4 - EPS..=1.4 + EPS).contains(&fame_loss_mult(c)));
            assert!((0.5 - EPS..=1.5 + EPS).contains(&fame_decay_mult(c)));
        }
        // Mais carisma → mais ganho, menos perda, menos decaimento.
        assert!(fame_gain_mult(100.0) > fame_gain_mult(0.0));
        assert!(fame_loss_mult(100.0) < fame_loss_mult(0.0));
        assert!(fame_decay_mult(100.0) < fame_decay_mult(0.0));
    }

    #[test]
    fn piso_pessoal_separa_campeao_de_anonimo() {
        let estreante = personal_fame_floor(0, 0, 0);
        let solido = personal_fame_floor(0, 12, 6);
        let campeao = personal_fame_floor(3, 40, 10);
        let lenda = personal_fame_floor(8, 120, 18);
        assert_eq!(estreante, FAME_DECAY_FLOOR, "sem carreira, piso base");
        assert!(estreante < solido && solido < campeao && campeao < lenda);
        // A distância que importa: o campeão não decai para dentro da faixa do sólido.
        assert!(
            campeao - solido > 20.0,
            "sólido={solido}, campeão={campeao}"
        );
        // E o piso da lenda satura sem invadir a faixa de Ídolo (>87).
        assert_eq!(lenda, FAME_FLOOR_MAX);
    }

    #[test]
    fn piso_pessoal_e_concavo_nos_titulos() {
        // O primeiro título vale muito mais que o sexto — senão o piso vira placar.
        let d1 = personal_fame_floor(1, 0, 0) - personal_fame_floor(0, 0, 0);
        let d6 = personal_fame_floor(6, 0, 0) - personal_fame_floor(5, 0, 0);
        assert!(d1 > d6, "primeiro título={d1}, sexto={d6}");
    }

    #[test]
    fn piso_pessoal_nunca_sai_da_faixa_travada() {
        for (t, v, e) in [(0, 0, 0), (1, 5, 2), (5, 60, 12), (40, 500, 60)] {
            let piso = personal_fame_floor(t, v, e);
            assert!(
                (FAME_DECAY_FLOOR..=FAME_FLOOR_MAX).contains(&piso),
                "piso fora da faixa: {piso}"
            );
        }
    }

    #[test]
    fn campeao_com_ano_ruim_nao_vira_anonimo() {
        // O caso que motivou o piso pessoal: um campeão passa uma temporada inteira
        // (20 etapas) sem marcar nada. Com o piso global de 25 ele terminava perto do
        // anônimo; com o piso pessoal ele continua sendo alguém.
        let piso = personal_fame_floor(3, 40, 10);
        let mut fama_pessoal = 80.0;
        let mut fama_global = 80.0;
        for _ in 0..20 {
            fama_pessoal =
                decay_fame_toward(fama_pessoal, piso, FAME_DECAY_BASE_RATE, 50.0);
            fama_global =
                decay_fame_toward(fama_global, FAME_DECAY_FLOOR, FAME_DECAY_BASE_RATE, 50.0);
        }
        assert!(fama_pessoal > 70.0, "campeão segue Estrela: {fama_pessoal}");
        assert!(
            fama_pessoal - fama_global > 10.0,
            "pessoal={fama_pessoal}, global={fama_global}"
        );
    }

    #[test]
    fn escala_de_categoria_inclina_a_piramide() {
        let rookie = fame_category_tier_mult(0);
        let gt3 = fame_category_tier_mult(4);
        let endurance = fame_category_tier_mult(6);
        assert!(rookie < gt3 && gt3 < endurance);
        assert!(rookie < 1.0 && endurance > 1.0, "rookie={rookie}, endurance={endurance}");
        // Tier fora da escada satura no topo em vez de explodir.
        assert_eq!(fame_category_tier_mult(200), endurance);
    }

    #[test]
    fn valor_comercial_e_convexo_no_topo() {
        // Anônimo não vale nada; sobe monotônico; e o salto pro topo é o MAIOR
        // (convexo — Ídolo desproporcionalmente valioso).
        assert_eq!(fame_commercial_units(10.0), 0.0); // Anônimo
        let discreto = fame_commercial_units(25.0);
        let conhecido = fame_commercial_units(45.0);
        let nome_forte = fame_commercial_units(65.0);
        let estrela = fame_commercial_units(80.0);
        let idolo = fame_commercial_units(95.0);
        assert!(discreto < conhecido && conhecido < nome_forte);
        assert!(nome_forte < estrela && estrela < idolo);
        // Salto Estrela→Ídolo é o maior de todos.
        assert!(idolo - estrela > estrela - nome_forte);
        assert!(idolo - estrela > nome_forte - conhecido);
    }

    #[test]
    fn need_factor_maior_para_time_carente() {
        let carente = team_need_factor(10.0, 15.0);
        let dinastia = team_need_factor(95.0, 90.0);
        assert!(carente > dinastia, "carente={carente}, dinastia={dinastia}");
        // Dentro dos limites travados.
        for (b, r) in [(0.0, 0.0), (50.0, 50.0), (100.0, 100.0)] {
            let n = team_need_factor(b, r);
            assert!((TEAM_NEED_MIN..=TEAM_NEED_MAX).contains(&n), "n={n}");
        }
    }

    #[test]
    fn time_carente_prefere_idolo_mediocre_a_rapido_anonimo() {
        // O caso-alvo do design: num time carente, o apelo de um Ídolo compensa um
        // gap de skill contra um rápido sem fama; numa dinastia, não. (É o mesmo termo
        // que a escada soma ao skill: fame_commercial_units × need_factor.)
        let idolo_appeal_carente = fame_commercial_units(95.0) * team_need_factor(10.0, 15.0);
        let idolo_appeal_dinastia = fame_commercial_units(95.0) * team_need_factor(95.0, 90.0);
        // Carente: 55 × ~1.05 ≈ 58 → cobre um gap de skill (60+58 > 90).
        assert!(60.0 + idolo_appeal_carente > 90.0);
        // Dinastia: 55 × ~0.28 ≈ 15 → não cobre (60+15 < 90).
        assert!(60.0 + idolo_appeal_dinastia < 90.0);
    }

    #[test]
    fn interesse_visivel_e_poucos_e_escala_com_a_fama() {
        // Poucos, e crescendo com a fama: ninguém até Conhecido, depois 1→2→3.
        assert_eq!(active_interest_team_count(40.0), 0); // Conhecido
        assert_eq!(active_interest_team_count(65.0), 1); // Nome forte
        assert_eq!(active_interest_team_count(80.0), 2); // Estrela
        assert_eq!(active_interest_team_count(95.0), 3); // Ídolo
                                                         // Nunca é a grade toda (o problema do screenshot).
        assert!(active_interest_team_count(100.0) <= 3);
    }

    #[test]
    fn qualidade_do_time_ordena_os_melhores_primeiro() {
        // Um time forte (reputação + carro + títulos) pontua mais que um fraco —
        // são esses os POUCOS que cobiçam o astro visivelmente.
        let forte = team_prestige_quality(85.0, 90.0, 7);
        let fraco = team_prestige_quality(30.0, 40.0, 0);
        assert!(forte > fraco);
    }
}

#[cfg(test)]
mod tests_saturacao {
    use super::*;

    /// A escada é UMA só. Este teste existe porque havia duas, escritas em arquivos
    /// diferentes, discordando sobre quanto vale um pódio.
    #[test]
    fn a_escada_de_chegada_e_a_mesma_para_os_dois_lados() {
        assert_eq!(fame_finish_base_delta(1, false, 10), FAME_FINISH_WIN);
        assert_eq!(fame_finish_base_delta(3, false, 10), FAME_FINISH_PODIUM);
        assert_eq!(fame_finish_base_delta(5, false, 10), FAME_FINISH_TOP5);
        assert_eq!(fame_finish_base_delta(10, false, 10), FAME_FINISH_TOP10);
        assert_eq!(fame_finish_base_delta(11, false, 10), FAME_FINISH_FORA);
        // Abandonar é pior que terminar fora, e vencer abandonando não existe.
        assert_eq!(fame_finish_base_delta(1, true, 10), FAME_FINISH_DNF);
        assert!(FAME_FINISH_DNF < FAME_FINISH_FORA);
    }

    /// A faixa visível é relativa ao grid, então a MESMA posição pode render ou não
    /// dependendo do tamanho do campo. P8 num grid de 28 aparece; num de 12, não.
    #[test]
    fn a_faixa_visivel_acompanha_o_tamanho_do_campo() {
        let grande = fame_visibility_last_position(28);
        let pequeno = fame_visibility_last_position(12);
        assert!(fame_finish_base_delta(8, false, grande) > 0.0);
        assert!(fame_finish_base_delta(8, false, pequeno) < 0.0);
    }

    #[test]
    fn abaixo_do_joelho_nada_e_amortecido() {
        assert_eq!(fame_headroom_mult(0.0), 1.0);
        assert_eq!(fame_headroom_mult(FAME_SATURATION_KNEE), 1.0);
        assert!(fame_headroom_mult(FAME_SATURATION_KNEE + 0.1) < 1.0);
    }

    /// O topo fica progressivamente mais caro — é a coisa toda. O mesmo ganho bruto tem
    /// que mover menos a fama de quem já está em 95 do que a de quem está em 75.
    #[test]
    fn o_ultimo_pedaco_da_escala_custa_mais_que_o_primeiro() {
        let ganho = 3.0;
        let de_50 = apply_fame_gain(50.0, ganho) - 50.0;
        let de_75 = apply_fame_gain(75.0, ganho) - 75.0;
        let de_95 = apply_fame_gain(95.0, ganho) - 95.0;
        assert!(de_50 > de_75, "{de_50} vs {de_75}");
        assert!(de_75 > de_95, "{de_75} vs {de_95}");
        // E o último trecho tem que ser MUITO mais caro, não marginalmente.
        assert!(de_95 < de_50 * 0.15, "de_95 {de_95} · de_50 {de_50}");
    }

    /// **Só a subida é amortecida.** Se a perda também fosse, o topo viraria um platô
    /// ainda mais grudento do que o que este mecanismo veio remover.
    #[test]
    fn a_queda_nao_e_amortecida() {
        assert!((apply_fame_gain(95.0, -2.0) - 93.0).abs() < 1e-9);
        assert!((apply_fame_gain(50.0, -2.0) - 48.0).abs() < 1e-9);
    }

    #[test]
    fn a_fama_nunca_sai_da_faixa_de_0_a_100() {
        assert_eq!(apply_fame_gain(99.9, 500.0).min(100.0), apply_fame_gain(99.9, 500.0));
        assert!(apply_fame_gain(99.9, 500.0) <= 100.0);
        assert!(apply_fame_gain(0.5, -500.0) >= 0.0);
    }

    /// GUARD DE ALCANCE. A faixa Ídolo (>87) tem que continuar ALCANÇÁVEL: a curva
    /// existe para atrasar o topo, não para fechá-lo. Uma carreira de vitórias
    /// consistentes na elite tem que chegar lá.
    #[test]
    fn o_topo_continua_alcancavel() {
        let piso = personal_fame_floor(6, 40, 15);
        // Dez temporadas de 14 etapas na elite: 3 vitórias, 5 pódios, o resto no top-10.
        // Escalado pelo tier da GT3, como na produção — a base crua não é o que chega
        // à fama de ninguém, e medir com ela subestimaria o alcance em 25%.
        let escala = fame_category_tier_mult(4);
        let temporada: Vec<f64> = (0..14)
            .map(|i| {
                escala
                    * match i {
                        0..=2 => FAME_FINISH_WIN,
                        3..=7 => FAME_FINISH_PODIUM,
                        _ => FAME_FINISH_TOP10,
                    }
            })
            .collect();
        let mut fama = 60.0;
        for _ in 0..10 {
            for ganho in &temporada {
                fama = apply_fame_gain(fama, *ganho);
                fama = decay_fame_toward(fama, piso, FAME_DECAY_BASE_RATE, 50.0);
            }
        }
        assert!(fama > 87.0, "carreira longa e vencedora parou em {fama:.1}");
        // …e não encosta no teto, senão a saturação não estaria fazendo nada.
        assert!(fama < 99.0, "saturou mesmo assim: {fama:.1}");
    }

    /// O contrário do teste acima, e o motivo de a curva existir: uma carreira só de
    /// pódios, sem vitória, PARA antes do Ídolo em vez de chegar lá por acumulação.
    #[test]
    fn acumular_resultado_medio_nao_leva_ao_idolo() {
        let piso = personal_fame_floor(0, 0, 10);
        let mut fama = 60.0;
        for _ in 0..(30 * 14) {
            fama = apply_fame_gain(fama, FAME_FINISH_PODIUM);
            fama = decay_fame_toward(fama, piso, FAME_DECAY_BASE_RATE, 50.0);
        }
        assert!(fama < 87.0, "só de pódio chegou a {fama:.1}");
    }
}
