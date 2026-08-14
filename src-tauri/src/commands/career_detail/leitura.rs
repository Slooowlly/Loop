//! Leituras qualitativas: escala tecnica, estrelato (fama + carisma) e a leitura de entrega (esperado vs entregue) com o rank de carro do grid.

use super::*;

/// Leitor de um eixo dentro dos atributos — o que faz a tabela de eixos abaixo
/// servir tanto ao piloto quanto a mediana do grid e ao snapshot do ano passado
/// sem repetir catorze vezes qual campo e qual.
type Eixo = fn(&DriverAttributes) -> f64;

/// Os eixos de QUALIDADE, na ordem em que a tela os desenha.
///
/// Eram QUATRO — ritmo, consistencia, racecraft e uma "resistencia" que era
/// fitness e pneus misturados num numero so. A mistura existia porque o bloco
/// morava num rodape de outra aba e nao cabia mais nada; com aba propria os dois
/// se separam e os outros saem do banco, onde estavam desde sempre.
///
/// `mentalidade` entrou depois, e nao por decoracao: era um dos dois atributos
/// que a ficha inteira nunca mostrava — so vazava como chip em Tracos quando era
/// extremo. E decide corrida: resolve clutch/choke sob pressao de campeonato,
/// evento e duelo ([`crate::commands::race`]). Fica na CORRIDA porque e ali que
/// a pressao se manifesta; grupo proprio para um eixo so seria cerimonia.
///
/// `confianca` era o outro, e foi para o espectro de estilo — ver [`STYLE_AXES`].
const QUALITY_AXES: &[(&str, &str, Eixo)] = &[
    ("volta_seca", "ritmo", |a| a.skill),
    ("volta_seca", "classificacao", |a| a.ritmo_classificacao),
    ("volta_seca", "consistencia", |a| a.consistencia),
    ("corrida", "racecraft", |a| a.racecraft),
    ("corrida", "defesa", |a| a.defesa),
    ("corrida", "largada", |a| a.habilidade_largada),
    // Fecha a coluna: os tres de cima dizem como ele corre, e este diz se
    // continua correndo assim quando a temporada esta em jogo.
    ("corrida", "mentalidade", |a| a.mentalidade),
    ("condicoes", "chuva", |a| a.fator_chuva),
    ("condicoes", "pneus", |a| a.gestao_pneus),
    ("condicoes", "adaptabilidade", |a| a.adaptabilidade),
    ("condicoes", "preparo", |a| a.fitness),
];

/// Os eixos que nao tem lado bom, cada um como um espectro entre DOIS polos.
///
/// Um piloto muito agressivo nao e "elite em agressividade" — ele e agressivo, o
/// que ganha posicao na largada e paga em pneu e em incidente. Dizer "Elite" ali
/// seria o julgamento errado com a cara de certo, e por isso eles saem em grupo
/// proprio: misturados aos eixos com nota, o marcador no meio da regua se lia
/// como nota media e o julgamento voltava pela vizinhanca.
///
/// Sao DOIS polos e nao quatro faixas. A escala anterior gastava quatro palavras
/// ("cirurgico/calculista/agressivo/beligerante") para dizer uma coisa que tem
/// dois lados, e a tela acabava mostrando o nome do eixo, a faixa e os dois
/// extremos — quatro rotulos para uma informacao. O eixo E o par: "calculista ou
/// agressivo", e onde o marcador cai entre os dois.
///
/// `confianca` entra aqui e nao na escala de qualidade apesar de o motor lhe dar
/// peso positivo no trecho final da prova. E o mesmo trio que sai no roster de IA
/// do iRacing (`driverAggression`, `driverSmoothness`, `driverOptimism`), e o
/// dossie de pre-temporada ja a lia como ousado vs comedido: cauteloso nao e um
/// defeito, e "Fraco em confianca" seria.
const STYLE_AXES: &[(&str, &str, Eixo, [&str; 2])] = &[
    (
        "estilo",
        "agressividade",
        |a| a.aggression,
        ["calculista", "agressivo"],
    ),
    ("estilo", "suavidade", |a| a.smoothness, ["bruto", "suave"]),
    (
        "estilo",
        "confianca",
        |a| a.confianca,
        ["cauteloso", "confiante"],
    ),
];

/// Como o piloto corre, eixo a eixo, em quatro grupos — com as duas ancoras que
/// transformam a leitura em julgamento: contra QUEM e desde QUANDO.
pub(super) fn build_driver_technical_read_block(
    conn: &Connection,
    driver: &Driver,
    category: Option<&str>,
    team: Option<&Team>,
) -> DriverTechnicalReadBlock {
    let contexto = TechnicalContext::load(conn, driver, category, team);
    let atributos = &driver.atributos;

    let mut itens: Vec<DriverTechnicalReadItem> = QUALITY_AXES
        .iter()
        .map(|(grupo, chave, eixo)| {
            let value = eixo(atributos);
            let (nivel, tom) = technical_level_for_value(value);
            technical_item(grupo, chave, nivel, tom, value, false, &contexto, *eixo)
        })
        .collect();

    itens.extend(STYLE_AXES.iter().map(|(grupo, chave, eixo, polos)| {
        let value = eixo(atributos).clamp(0.0, 100.0);
        let polo = |extremo: &str| {
            let chave = format!("driver_read.style.{extremo}");
            rust_i18n::t!(&chave).to_string()
        };
        // O `nivel` de um eixo de estilo e o polo para o qual ele PENDE, e nao uma
        // faixa a mais: e o que a leitura de tela anuncia, enquanto quem enxerga
        // le a mesma coisa na posicao do marcador.
        let pendendo = polo(polos[usize::from(value >= 50.0)]);
        let mut item = technical_item(
            grupo, chave, pendendo, "neutral", value, true, &contexto, *eixo,
        );
        item.polo_min = Some(polo(polos[0]));
        item.polo_max = Some(polo(polos[1]));
        item
    }));

    DriverTechnicalReadBlock { itens }
}

/// O que a leitura tecnica precisa saber ALEM do piloto.
///
/// A regua de 0–100 abriu duas perguntas que a ficha nao respondia. "Instavel"
/// contra quem — 45 de ritmo na F4 e 45 na GT3 descrevem pilotos diferentes — e
/// desde quando, porque um eixo caindo e um eixo subindo se leem igual na foto.
#[derive(Default)]
struct TechnicalContext {
    /// O grid inteiro — a mediana sai dele eixo a eixo, na hora. Sai na hora e nao
    /// pre-computada porque um piloto FANTASMA com a mediana de cada campo nao
    /// existe no grid, e guardar um so convidaria a lê-lo como se existisse. Cada
    /// regua e lida sozinha; a mediana de cada uma e que e honesta.
    grid: Vec<DriverAttributes>,
    /// Como o piloto estava no fim da ultima temporada arquivada.
    anterior: Option<DriverAttributes>,
}

impl TechnicalContext {
    fn load(
        conn: &Connection,
        driver: &Driver,
        category: Option<&str>,
        team: Option<&Team>,
    ) -> Self {
        Self {
            grid: category
                .map(|value| grid_da_categoria(conn, value, team))
                .unwrap_or_default(),
            anterior: last_archived_attributes(conn, &driver.id),
        }
    }

    /// Mediana do grid neste eixo. `None` com menos de tres pilotos: com dois, "a
    /// mediana" e so o outro piloto com nome de estatistica.
    fn referencia(&self, eixo: Eixo) -> Option<u8> {
        if self.grid.len() < 3 {
            return None;
        }
        let mut valores: Vec<f64> = self
            .grid
            .iter()
            .map(|atributos| eixo(atributos).clamp(0.0, 100.0))
            .collect();
        valores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        Some(valores[valores.len() / 2].round() as u8)
    }
}

/// O GRID, e nao a categoria.
///
/// A diferenca nao e de palavra. `categoria_atual` inclui quem esta SEM ASSENTO —
/// agente livre, reserva, quem caiu de categoria e ainda nao assinou. Nenhum
/// deles corre contra o piloto, e a mediana existe justamente para responder
/// "contra quem eu corro". E a mesma nocao de grid que [`expected_position_for_team`]
/// ja usa para ranquear carro: assento ocupado, nao ficha cadastrada.
///
/// Em multiclasse a comparacao e DENTRO DA CLASSE. Um LMP2 medido contra o campo
/// inteiro de endurance leva o GT4 na conta, e a mediana deixa de descrever
/// qualquer coisa — sao carros que nem disputam a mesma posicao.
///
/// Os pilotos saem por id e nao por `get_drivers_by_category` de proposito: nas
/// categorias especiais o assento vive em `categoria_especial_ativa` enquanto o
/// `categoria_atual` continua sendo o da escada regular, e cruzar as duas listas
/// devolveria grid vazio exatamente ali.
pub(super) fn grid_da_categoria(
    conn: &Connection,
    category: &str,
    team: Option<&Team>,
) -> Vec<DriverAttributes> {
    let Ok(equipes) = team_queries::get_teams_by_category(conn, base_category_of(category)) else {
        return Vec::new();
    };
    let classe = team.and_then(|value| value.classe.as_deref());

    equipes
        .iter()
        .filter(|rival| classe.is_none() || rival.classe.as_deref() == classe)
        .flat_map(|rival| [rival.piloto_1_id.as_deref(), rival.piloto_2_id.as_deref()])
        .flatten()
        .filter_map(|id| driver_queries::get_driver(conn, id).ok())
        .map(|piloto| piloto.atributos)
        .collect()
}

/// Atributos do piloto no fim da ultima temporada arquivada.
///
/// O snapshot guarda os atributos com os NOMES dos campos, entao ele desserializa
/// direto em [`DriverAttributes`] — `potencial` e `carisma` tem default de serde,
/// que e o que cobre os arquivos gravados antes de esses campos existirem.
fn last_archived_attributes(conn: &Connection, driver_id: &str) -> Option<DriverAttributes> {
    let json: String = conn
        .query_row(
            "SELECT snapshot_json
             FROM driver_season_archive
             WHERE piloto_id = ?1
             ORDER BY CAST(season_number AS INTEGER) DESC
             LIMIT 1",
            rusqlite::params![driver_id],
            |row| row.get(0),
        )
        .ok()?;
    let snapshot: serde_json::Value = serde_json::from_str(&json).ok()?;
    serde_json::from_value(snapshot.get("atributos")?.clone()).ok()
}

fn technical_item(
    grupo: &str,
    chave: &str,
    nivel: String,
    tom: &str,
    valor: f64,
    estilo: bool,
    contexto: &TechnicalContext,
    eixo: Eixo,
) -> DriverTechnicalReadItem {
    // O rotulo sai do locale do backend, e nao mais de uma string PT crua no
    // codigo: era o unico campo desta ficha que ignorava o idioma escolhido.
    let full = format!("driver_read.axis.{chave}");
    let valor = valor.clamp(0.0, 100.0);
    // Delta zero vira `None`: e o caso da maioria dos eixos numa temporada, e um
    // "+0" repetido dez vezes por coluna esconde os dois que de fato andaram.
    let delta = contexto
        .anterior
        .as_ref()
        .map(|antes| (valor - eixo(antes).clamp(0.0, 100.0)).round() as i8)
        .filter(|value| *value != 0);

    DriverTechnicalReadItem {
        chave: chave.to_string(),
        grupo: grupo.to_string(),
        label: rust_i18n::t!(&full).to_string(),
        nivel,
        tom: tom.to_string(),
        escala: valor.round() as u8,
        estilo,
        polo_min: None,
        polo_max: None,
        referencia: contexto.referencia(eixo),
        delta,
    }
}

/// Monta o bloco de ESTRELATO (fama + carisma) para a ficha do piloto.
/// Fama (`midia`) usa a MESMA classificação de tier do mercado (visibility.rs) pra
/// ficar coerente com o resto do jogo; carisma tem escala descritiva própria; e o
/// `resumo` traduz a DINÂMICA (carisma modula a fama) em uma linha.
pub(super) fn build_driver_stardom_block(driver: &Driver) -> DriverStardomBlock {
    let fama = driver.atributos.midia.clamp(0.0, 100.0);
    let carisma = driver.atributos.carisma.clamp(0.0, 100.0);

    let (nivel_fama, tom_fama) = fama_level_for_value(fama);
    let (nivel_carisma, tom_carisma) = carisma_level_for_value(carisma);
    let resumo = stardom_reading(fama, carisma);

    DriverStardomBlock {
        fama: fama.round() as u8,
        carisma: carisma.round() as u8,
        nivel_fama: nivel_fama.to_string(),
        tom_fama: tom_fama.to_string(),
        nivel_carisma: nivel_carisma.to_string(),
        tom_carisma: tom_carisma.to_string(),
        resumo,
    }
}

/// ARCO: onde o piloto está na própria curva, e não em relação aos rivais.
///
/// As faixas de idade NÃO são inventadas — são as mesmas em que a simulação muda
/// de comportamento: crescimento cheio até 20, forte até 24, normal até 28, meio
/// passo até 32, e declínio a partir daí ([`crate::evolution`]). Uma tela que
/// dissesse "no auge" enquanto o motor já derruba fitness todo ano seria uma
/// segunda opinião sobre o mesmo piloto.
pub(super) fn build_driver_career_arc_block(driver: &Driver) -> DriverCareerArcBlock {
    let idade = driver.idade as i32;
    let potencial = driver.atributos.potencial;
    // Potencial 0.0 nao e "teto no chao": e teto NAO DERIVADO (jogador e saves
    // antigos). Sem isso a ficha anunciaria "chegou ao teto" para quem nunca teve
    // um teto medido — o erro mais caro que este bloco poderia cometer.
    let margem = (potencial > 0.0).then(|| (potencial - driver.atributos.skill).max(0.0));
    let nivel_margem = margem.map(|value| {
        let key = if value < 2.0 {
            "no_teto"
        } else if value < 8.0 {
            "curta"
        } else if value < 22.0 {
            "boa"
        } else {
            "larga"
        };
        let full = format!("driver_read.margin.{key}");
        rust_i18n::t!(&full).to_string()
    });

    // Sem teto medido, a fase sai so da idade: e menos do que se sabe de um piloto
    // de IA, e mais honesto do que fingir folga que ninguem calculou.
    let tem_folga = margem.is_some_and(|value| value >= 8.0);
    let (fase_key, tom) = if idade <= 20 {
        ("formacao", "info")
    } else if idade <= 28 && tem_folga {
        ("ascensao", "info")
    } else if idade <= 32 {
        ("auge", "success")
    } else if idade <= 36 {
        ("plato", "neutral")
    } else {
        ("crepusculo", "warning")
    };

    let fase_full = format!("driver_read.arc.{fase_key}");
    let resumo_full = format!("driver_read.arc_summary.{fase_key}");

    DriverCareerArcBlock {
        idade,
        fase: rust_i18n::t!(&fase_full).to_string(),
        fase_chave: fase_key.to_string(),
        tom_fase: tom.to_string(),
        nivel_experiencia: scale_label("experience", driver.atributos.experiencia),
        nivel_desenvolvimento: scale_label("development", driver.atributos.desenvolvimento),
        nivel_margem,
        resumo: rust_i18n::t!(&resumo_full).to_string(),
    }
}

/// Rótulo de uma escala descritiva de quatro faixas (`driver_read.<escala>.*`).
fn scale_label(escala: &str, value: f64) -> String {
    let value = value.clamp(0.0, 100.0);
    let faixa = if value < 30.0 {
        "baixo"
    } else if value < 55.0 {
        "medio"
    } else if value < 78.0 {
        "alto"
    } else {
        "maximo"
    };
    let full = format!("driver_read.{escala}.{faixa}");
    rust_i18n::t!(&full).to_string()
}

/// Escala de FAMA para exibição — 6 níveis, mais rica que os 4 tiers de mercado
/// internos (o display pode ser mais granular que a lógica comercial de
/// salário/patrocínio). Vai de Anônimo a Ídolo; o topo é aspiracional e raro.
pub(super) fn fama_level_for_value(value: f64) -> (String, &'static str) {
    let value = value.clamp(0.0, 100.0);
    let (key, tom) = if value <= 15.0 {
        ("anonimo", "neutral")
    } else if value <= 30.0 {
        ("discreto", "neutral")
    } else if value <= 50.0 {
        ("conhecido", "info")
    } else if value <= 70.0 {
        ("nome_forte", "info")
    } else if value <= 87.0 {
        ("estrela", "success")
    } else {
        ("idolo", "elite")
    };
    let full = format!("driver_read.fama.{key}");
    (rust_i18n::t!(&full).to_string(), tom)
}

pub(super) fn carisma_level_for_value(value: f64) -> (String, &'static str) {
    let value = value.clamp(0.0, 100.0);
    let (key, tom) = if value < 30.0 {
        ("apagado", "danger")
    } else if value < 45.0 {
        ("reservado", "warning")
    } else if value < 60.0 {
        ("cativante", "neutral")
    } else if value < 75.0 {
        ("magnetico", "info")
    } else if value < 88.0 {
        ("carismatico", "success")
    } else {
        ("idolo_natural", "elite")
    };
    let full = format!("driver_read.carisma.{key}");
    (rust_i18n::t!(&full).to_string(), tom)
}

/// Leitura de uma linha: como o carisma (retenção/conversão) conversa com a fama
/// (estoque). Alto carisma + baixa fama = pólvora seca; baixo carisma + alta fama =
/// holofote volátil construído só pelo resultado.
pub(super) fn stardom_reading(fama: f64, carisma: f64) -> String {
    let fama_alta = fama >= 60.0;
    let carisma_alto = carisma >= 60.0;

    let key = match (fama_alta, carisma_alto) {
        (true, true) => "idol_consolidated",
        (false, true) => "powder_keg",
        (true, false) => "volatile_spotlight",
        (false, false) => "off_radar",
    };
    let full = format!("driver_read.stardom.{key}");
    rust_i18n::t!(&full).to_string()
}

pub(super) fn technical_level_for_value(value: f64) -> (String, &'static str) {
    let value = value.clamp(0.0, 100.0);
    let (key, tom) = if value < 12.5 {
        ("muito_fraco", "danger")
    } else if value < 25.0 {
        ("fraco", "danger")
    } else if value < 37.5 {
        ("abaixo", "warning")
    } else if value < 50.0 {
        ("instavel", "warning")
    } else if value < 62.5 {
        ("competente", "neutral")
    } else if value < 75.0 {
        ("forte", "info")
    } else if value < 87.5 {
        ("muito_forte", "success")
    } else {
        ("elite", "elite")
    };
    let full = format!("driver_read.technical.{key}");
    (rust_i18n::t!(&full).to_string(), tom)
}

pub(super) fn build_performance_read_block(
    conn: &Connection,
    driver: &Driver,
    team: Option<&Team>,
    teammate: Option<&Driver>,
    championship_position: Option<i32>,
    category: Option<&str>,
) -> DriverPerformanceReadBlock {
    let expected = team.and_then(|value| expected_position_for_team(conn, value));
    let delta = match (expected, championship_position) {
        (Some(expected_position), Some(position)) => Some(expected_position - position),
        _ => None,
    };
    let teammate_points = teammate.map(|value| value.stats_temporada.pontos.round() as i32);
    // Companheiro de equipe corre a mesma categoria, entao a posicao dele sai da
    // MESMA ordenacao que deu a do piloto — e nao de uma contagem paralela que
    // poderia discordar dela. Falha de leitura aqui vira `None` e some da tela:
    // e um detalhe de exibicao, nao motivo para derrubar a ficha inteira.
    let teammate_position = match (category, teammate) {
        (Some(category), Some(value)) => find_championship_context(conn, category, &value.id)
            .ok()
            .flatten()
            .map(|context| context.posicao),
        _ => None,
    };
    let reading = match delta {
        Some(value) if value >= 3 => rust_i18n::t!("driver_read.delivery.above"),
        Some(value) if value <= -3 => rust_i18n::t!("driver_read.delivery.below"),
        Some(_) => rust_i18n::t!("driver_read.delivery.within"),
        None => rust_i18n::t!("driver_read.delivery.no_context"),
    };

    DriverPerformanceReadBlock {
        esperado_posicao: expected,
        entregue_posicao: championship_position,
        delta_posicao: delta,
        car_performance: team.map(|value| value.effective_car_performance()),
        companheiro_nome: teammate.map(|value| value.nome.clone()),
        companheiro_pontos: teammate_points,
        companheiro_posicao: teammate_position,
        companheiro_nacionalidade: teammate
            .map(|value| nationality_display_label(&value.nacionalidade, &value.genero)),
        piloto_pontos: driver.stats_temporada.pontos.round() as i32,
        leitura: reading.to_string(),
    }
}

/// Dois carros dentro desta distância de `car_performance` estão em EMPATE TÉCNICO — o
/// pacote não separa os dois. Carros de mesmo nível dão magnitude idêntica, então a margem
/// só existe pro caminho legado (equipe sem peças persistidas, escalar contínuo).
pub(super) const CAR_TIE_EPSILON: f64 = 1e-6;

/// Assentos OCUPADOS da equipe (0–2) — o grid REAL, não a capacidade nominal.
pub(super) fn filled_seats(team: &Team) -> i32 {
    i32::from(team.piloto_1_id.is_some()) + i32::from(team.piloto_2_id.is_some())
}

/// Posição ESPERADA pelo pacote (carro), RELATIVA ao grid da categoria.
///
/// Era uma tabela de limiares ABSOLUTOS sobre o escalar de carro, e ela mentia por dois
/// lados: o escalar não tem escala comum entre categorias (as de cima estouram o topo da
/// tabela e o grid inteiro "espera" P2), e num grid SPEC — rookie, todo carro no nível 1 —
/// o escalar é IDÊNTICO pra todo mundo, então a tabela dava posição de fundo pra todas as
/// equipes de uma vez. Aqui a equipe é ranqueada pelo carro EFETIVO ([`Team::effective_car_performance`])
/// contra as rivais do mesmo grid (categoria + classe) e a expectativa é o meio da faixa de
/// assentos do seu rank. Grid spec (todo mundo empatado) → todo mundo espera o meio do grid,
/// que é a leitura honesta quando o carro não separa ninguém e o resultado é só piloto.
pub(super) fn expected_position_for_team(conn: &Connection, team: &Team) -> Option<i32> {
    let rivals = team_queries::get_teams_by_category(conn, &team.categoria).ok()?;
    let grid: Vec<(f64, i32)> = rivals
        .iter()
        .filter(|rival| rival.classe == team.classe)
        .map(|rival| (rival.effective_car_performance(), filled_seats(rival)))
        .collect();

    expected_position_from_grid(team.effective_car_performance(), &grid)
}

/// Núcleo PURO do rank: dado o carro da equipe e o grid `(carro, assentos ocupados)`, cai no
/// MEIO da faixa de assentos do bloco em que ela está. Assentos com carro estritamente melhor
/// ficam à frente; o bloco do empate técnico inclui a própria equipe. `None` quando o bloco
/// está vazio (equipe sem assento ocupado — não há expectativa a dar).
pub(super) fn expected_position_from_grid(mine: f64, grid: &[(f64, i32)]) -> Option<i32> {
    let mut seats_ahead = 0;
    let mut seats_tied = 0;
    for &(perf, seats) in grid {
        let delta = perf - mine;
        if delta > CAR_TIE_EPSILON {
            seats_ahead += seats;
        } else if delta >= -CAR_TIE_EPSILON {
            seats_tied += seats;
        }
    }

    if seats_tied == 0 {
        return None;
    }
    Some(seats_ahead + (seats_tied + 1) / 2)
}
