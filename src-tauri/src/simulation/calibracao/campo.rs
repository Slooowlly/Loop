//! Gerador de campo sintético: monta um grid de [`SimDriver`] reprodutível a partir de uma
//! semente, com distribuição de atributos parametrizável.
//!
//! O ponto que separa isto do `build_driver` dos testes de `engine.rs` é deliberado: lá o grid é
//! uma **escada linear perfeita** (skill = 60 + i, carro = 7 + 0.2i), o que faz o pelotão inteiro
//! estar perfeitamente ordenado em TODOS os eixos ao mesmo tempo. Um grid assim mede a simulação
//! no melhor caso possível para ela — qualquer motor devolve a ordem de entrada. Aqui:
//!
//! - o talento vem de uma normal com **cauda superior esticada** (poucos craques muito acima da
//!   média, um monte de medianos, alguns fracos), não de uma régua;
//! - a repartição dos atributos **espelha a do jogo** (ver abaixo);
//! - carro e piloto se correlacionam sem serem a mesma coisa: o bom piloto tende ao bom carro,
//!   com encaixes trocados de propósito.
//!
//! # A repartição não é escolha do harness
//!
//! `models/driver_generation.rs` separa os atributos em dois grupos, e **este gerador tem que
//! seguir a mesma separação** — senão a régua mede um mundo que não existe:
//!
//! | Grupo | Atributos | Como |
//! |---|---|---|
//! | Correlacionados com o skill | `consistencia` ±10, `racecraft` ±8, `defesa` ±8, `ritmo_classificacao` ±12 | `correlated_stat` |
//! | Independentes do talento | `gestao_pneus`, `habilidade_largada`, `adaptabilidade`, `mentalidade` 40–70; `fator_chuva` 30–70; `aggression` 20–85; `confianca` 35–75 | `roll_stat` |
//! | Ancorado na agressividade | `smoothness` = 100 − `aggression` ±10 | `inverse_correlated_stat` |
//!
//! **Isto foi um defeito real desta peça**, corrigido só no A6. Sete atributos saíam do talento
//! quando no jogo são livres — e eram justamente os eixos que os pacotes D e G fortaleceram:
//! chuva reordena o pelotão, `gestao_pneus` é o eixo do undercut, `habilidade_largada` pesa 0,35
//! no primeiro segmento, e `smoothness` no jogo é o INVERSO da agressividade (o trade-off "quem
//! anda no limite castiga o pneu", que derivar do talento destrói).
//!
//! A consequência era subestimar quanto mecanismo existe. Numa campanha de calibração isso é pior
//! que um erro de medida: se a régua diz que um mecanismo tem menos alavanca do que tem, a busca
//! empurra os parâmetros dele acima do necessário para bater o alvo — e no jogo, onde os atributos
//! são livres, o resultado passa do ponto. Teria parecido convergência.
//!
//! O guard contra deriva é [`comparar_com_gerador_real`], que confere a estrutura de correlação
//! contra o gerador de verdade em vez de contra constantes copiadas à mão.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::models::driver::Driver;
use crate::models::team::placeholder_team_from_db;
use crate::simulation::context::SimDriver;

/// Amostra de uma normal por Box–Muller. Evita depender do `rand_distr` só para isto.
pub(crate) fn normal(rng: &mut impl Rng, media: f64, desvio: f64) -> f64 {
    let u1: f64 = rng.gen_range(f64::EPSILON..1.0);
    let u2: f64 = rng.gen::<f64>();
    let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
    media + z * desvio
}

/// Forma da distribuição de atributos de um grid. Um perfil por categoria — o campo de uma
/// rookie e o de uma GT3 não têm nem a mesma média nem o mesmo aperto.
#[derive(Debug, Clone)]
pub struct PerfilCampo {
    /// Categoria a ser usada no contexto de simulação (`mazda_rookie`, `gt3`, ...).
    pub category_id: String,
    /// Tier da categoria na escada (0 = entrada).
    pub category_tier: u8,

    /// Média do talento-base do pelotão.
    pub talento_medio: f64,
    /// Desvio-padrão do talento-base.
    pub talento_desvio: f64,
    /// Multiplicador aplicado só ao lado POSITIVO do desvio. > 1 alonga a cauda de cima:
    /// é o que cria "poucos craques bem acima" em vez de um sino simétrico.
    pub cauda_superior: f64,
    /// Piso e teto do talento depois da cauda.
    pub talento_min: f64,
    pub talento_max: f64,

    /// **Não usado na repartição dos atributos** — ela é fixa e espelha
    /// `models/driver_generation.rs`. Mantido só como referência de intenção do perfil.
    ///
    /// Um harness não pode ter knob próprio para a estrutura de correlação dos atributos: se ela
    /// divergir da do jogo, a régua mede um mundo que não existe. Foi o defeito que custou uma
    /// remedição inteira deste baseline.
    pub ruido_atributo: f64,

    /// `car_performance` das equipes (a escala do jogo: ~ -5 a 16+). Irrelevante nas categorias
    /// spec (rookie), onde `category_car_performance` sobrescreve tudo pelo valor spec.
    pub carro_medio: f64,
    pub carro_desvio: f64,

    /// Pilotos por equipe (o padrão do jogo é 2).
    pub pilotos_por_equipe: usize,
    /// Trocas de encaixe piloto↔equipe, em fração do grid. 0 = os melhores pilotos sempre nos
    /// melhores carros; 0.5 = mercado bem bagunçado.
    pub ruido_encaixe: f64,

    /// Experiência na categoria. < 10 aciona a penalidade de novato do motor.
    pub corridas_na_categoria: i32,
}

impl PerfilCampo {
    /// Categoria de entrada: pelotão fraco, muito espalhado, cheio de irregularidade — e carro
    /// spec, então o resultado deveria ser talento + sorte e nada mais.
    pub fn rookie() -> Self {
        Self {
            category_id: "mazda_rookie".to_string(),
            category_tier: 0,
            // Média e desvio ANCORADOS no gerador do jogo, medidos com
            // `espalhamento_do_skill`: `generate_for_category("mazda_rookie", 0, "medio")` dá
            // 45,1 / 12,1. Não é escolha estética — o espalhamento do talento multiplica toda
            // vantagem determinística, e um campo mais largo que o do jogo mede determinismo que
            // não existe. Guardado por `espalhamento_do_skill_acompanha_o_jogo`.
            talento_medio: 45.0,
            talento_desvio: 9.4,
            cauda_superior: 1.6,
            talento_min: 28.0,
            talento_max: 88.0,
            ruido_atributo: 7.0,
            // Spec: o valor não é usado (ver `math::category_car_performance`), mas mantemos
            // algo plausível para que o perfil siga legível fora da simulação.
            carro_medio: 2.0,
            carro_desvio: 0.0,
            pilotos_por_equipe: 2,
            ruido_encaixe: 0.45,
            corridas_na_categoria: 6,
        }
    }

    /// Categoria de topo: pelotão forte e apertado, gente muito consistente — o caos aqui teria
    /// que vir do carro, da estratégia e do trânsito, não do erro grosseiro.
    pub fn gt3() -> Self {
        Self {
            category_id: "gt3".to_string(),
            category_tier: 4,
            // Idem: o jogo dá 81,1 / 4,4 para gt3. O desvio anterior (5,0 com cauda 1,5) produzia
            // 6,1 — **38% mais largo que o jogo**, o que inflava a vantagem determinística
            // justamente na categoria onde o carro já responde por metade da variância permanente.
            talento_medio: 81.0,
            talento_desvio: 3.6,
            cauda_superior: 1.5,
            talento_min: 62.0,
            talento_max: 97.0,
            ruido_atributo: 4.5,
            carro_medio: 9.0,
            carro_desvio: 3.0,
            pilotos_por_equipe: 2,
            ruido_encaixe: 0.20,
            corridas_na_categoria: 40,
        }
    }

    /// Nome curto para relatório e mensagem de teste.
    pub fn rotulo(&self) -> &str {
        &self.category_id
    }
}

/// Sorteia o talento-base de um piloto: normal com o lado de cima esticado por `cauda_superior`.
fn talento_base(perfil: &PerfilCampo, rng: &mut impl Rng) -> f64 {
    let z = normal(rng, 0.0, 1.0);
    let z_ajustado = if z > 0.0 {
        z * perfil.cauda_superior
    } else {
        z
    };
    (perfil.talento_medio + z_ajustado * perfil.talento_desvio)
        .clamp(perfil.talento_min, perfil.talento_max)
}

/// Atributo CORRELACIONADO ao talento — espelha `driver_generation::correlated_stat`: o mesmo
/// centro do skill mais um desvio uniforme de ±`desvio`.
fn correlacionado(talento: f64, desvio: f64, rng: &mut impl Rng) -> f64 {
    (talento + rng.gen_range(-desvio..=desvio)).clamp(0.0, 100.0)
}

/// Atributo INDEPENDENTE do talento — espelha `driver_generation::roll_stat`: sorteio absoluto
/// numa faixa fixa, sem relação nenhuma com o nível do piloto.
fn independente(min: f64, max: f64, rng: &mut impl Rng) -> f64 {
    rng.gen_range(min..=max)
}

/// `smoothness` — espelha `driver_generation::inverse_correlated_stat`: ancorado no INVERSO da
/// agressividade, não no talento. É o trade-off "quem anda no limite castiga o pneu".
fn inverso_da_agressividade(aggression: f64, rng: &mut impl Rng) -> f64 {
    (100.0 - aggression + rng.gen_range(-10.0..=10.0)).clamp(0.0, 100.0)
}

/// Constrói um grid de `pilotos` [`SimDriver`] segundo o `perfil`, reprodutível pela `semente`.
///
/// O mesmo par (perfil, semente) sempre devolve o mesmo grid — é isso que permite comparar
/// medições feitas antes e depois de um conserto na simulação.
pub fn gerar_campo(perfil: &PerfilCampo, pilotos: usize, semente: u64) -> Vec<SimDriver> {
    let mut rng = StdRng::seed_from_u64(semente);

    // 1) Talento de cada piloto e os atributos pendurados nele.
    let mut brutos: Vec<(f64, Driver)> = (0..pilotos)
        .map(|i| {
            let talento = talento_base(perfil, &mut rng);
            let ruido = perfil.ruido_atributo;

            let mut piloto = Driver::create_player(
                format!("CAL{:03}", i + 1),
                format!("Piloto {:03}", i + 1),
                "Brasileiro".to_string(),
                24,
            );
            piloto.is_jogador = false;
            piloto.corridas_na_categoria = perfil.corridas_na_categoria.max(0) as u32;

            // A REPARTIÇÃO É A DO JOGO, não uma invenção do harness. Ver
            // `models/driver_generation.rs` (~linha 93): a geração real separa os atributos em
            // correlacionados com o skill e sorteados em absoluto, e um harness que derive tudo do
            // talento MEDE MENOS MECANISMO DO QUE EXISTE.
            //
            // Foi o defeito desta peça até aqui: `fator_chuva`, `gestao_pneus`,
            // `habilidade_largada`, `adaptabilidade`, `mentalidade`, `confianca` e `smoothness`
            // saíam do talento, quando no jogo são livres. Justamente os eixos que os pacotes D e
            // G fortaleceram — chuva reordena, gestão de pneus é o eixo do undercut, largada pesa
            // 0,35 no primeiro segmento, e smoothness é o inverso da agressividade (o trade-off
            // "quem anda no limite castiga o pneu", que derivar do talento destrói).
            //
            // O guard contra deriva é `repartição_espelha_a_geração_do_jogo`, que compara a
            // estrutura de correlação deste gerador contra a do gerador REAL.
            let a = &mut piloto.atributos;
            a.skill = talento;

            // --- Correlacionados com o skill (mesmas variâncias de `correlated_stat`) ---
            a.consistencia = correlacionado(talento, 10.0, &mut rng);
            a.racecraft = correlacionado(talento, 8.0, &mut rng);
            a.defesa = correlacionado(talento, 8.0, &mut rng);
            a.ritmo_classificacao = correlacionado(talento, 12.0, &mut rng);

            // --- Independentes do talento (sorteio absoluto, como `roll_stat`) ---
            a.gestao_pneus = independente(40.0, 70.0, &mut rng);
            a.habilidade_largada = independente(40.0, 70.0, &mut rng);
            a.adaptabilidade = independente(40.0, 70.0, &mut rng);
            a.mentalidade = independente(40.0, 70.0, &mut rng);
            a.fator_chuva = independente(30.0, 70.0, &mut rng);
            // Faixas LARGAS nos eixos de estilo, acompanhando a geração real: a
            // régua tem que medir o mundo que existe, e o mundo passou a ter
            // piloto cauteloso e piloto beligerante no mesmo grid.
            a.aggression = independente(20.0, 85.0, &mut rng);
            a.confianca = independente(35.0, 75.0, &mut rng);

            // --- Ancorado na agressividade, não no talento ---
            a.smoothness = inverso_da_agressividade(a.aggression, &mut rng);

            // --- Derivados de idade / carreira ---
            // `create_player(.., 24)` cai na faixa 23–32 de `fitness_for_age`.
            a.fitness = independente(60.0, 75.0, &mut rng);
            a.experiencia = (perfil.corridas_na_categoria as f64 * 1.5).clamp(5.0, 99.0);
            let _ = ruido;

            (talento, piloto)
        })
        .collect();

    // 2) Equipes: uma escala de carro própria, também com cauda.
    let equipes = pilotos.div_ceil(perfil.pilotos_por_equipe.max(1));
    let mut carros: Vec<f64> = (0..equipes)
        .map(|_| {
            let z = normal(&mut rng, 0.0, 1.0);
            let z_ajustado = if z > 0.0 { z * 1.4 } else { z };
            perfil.carro_medio + z_ajustado * perfil.carro_desvio
        })
        .collect();

    // 3) Encaixe piloto↔equipe: correlacionado (bom piloto tende a bom carro), com trocas
    //    de vizinhos suficientes para que a correlação não seja perfeita.
    brutos.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    carros.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    if perfil.ruido_encaixe >= 1.0 {
        // Encaixe INDEPENDENTE: Fisher–Yates completo. Não é realista (no jogo o bom piloto
        // vai para o bom carro), mas é o único jeito de a decomposição de variância separar a
        // fatia do piloto da fatia do carro — com os dois correlacionados, o congelamento
        // isolado só entrega limites, e o limite superior fica vazio.
        for i in (1..pilotos).rev() {
            let j = rng.gen_range(0..=i);
            brutos.swap(i, j);
        }
    } else {
        let trocas = (pilotos as f64 * perfil.ruido_encaixe).round() as usize;
        for _ in 0..trocas {
            if pilotos < 2 {
                break;
            }
            let i = rng.gen_range(0..pilotos - 1);
            brutos.swap(i, i + 1);
        }
    }

    brutos
        .into_iter()
        .enumerate()
        .map(|(indice, (_, piloto))| {
            let equipe_idx = indice / perfil.pilotos_por_equipe.max(1);
            let mut equipe = placeholder_team_from_db(
                format!("CALT{:03}", equipe_idx + 1),
                format!("Equipe {:03}", equipe_idx + 1),
                perfil.category_id.clone(),
                "2026-01-01T00:00:00".to_string(),
            );
            equipe.car_performance = carros[equipe_idx.min(carros.len() - 1)];
            // Moral neutra: a calibração mede a simulação, não o efeito de moral.
            equipe.morale = 1.0;
            equipe.confiabilidade = 80.0;

            SimDriver::from_driver_and_team(&piloto, &equipe)
        })
        .collect()
}

/// Achata a diferença de carro: todo mundo passa a correr com a média do grid.
///
/// É o congelamento seletivo da fonte "equipe/carro" na decomposição de variância. A diferença
/// entre a variância entre-pilotos de um grid normal e a de um grid nivelado É a contribuição do
/// carro. Nas categorias spec (rookie) isto não muda nada por construção — `category_car_performance`
/// já sobrescreve tudo pelo valor spec — e é justamente por isso que o número medido lá tem que
/// dar ~0: serve de aferição do próprio método.
pub fn nivelar_carros(grid: &mut [SimDriver]) {
    if grid.is_empty() {
        return;
    }
    let media = grid.iter().map(|d| d.car_performance).sum::<f64>() / grid.len() as f64;
    for piloto in grid.iter_mut() {
        piloto.car_performance = media;
    }
}

/// Achata a diferença de PILOTO: todo mundo passa a ter os atributos médios do grid, preservando
/// carro e equipe. É o congelamento simétrico ao [`nivelar_carros`].
pub fn nivelar_pilotos(grid: &mut [SimDriver]) {
    if grid.is_empty() {
        return;
    }
    let n = grid.len() as f64;
    macro_rules! nivelar {
        ($($campo:ident),+ $(,)?) => {
            $(
                let media = (grid.iter().map(|d| d.$campo as f64).sum::<f64>() / n)
                    .round()
                    .clamp(0.0, 100.0) as u8;
                for piloto in grid.iter_mut() {
                    piloto.$campo = media;
                }
            )+
        };
    }
    nivelar!(
        skill,
        consistencia,
        racecraft,
        defesa,
        ritmo_classificacao,
        gestao_pneus,
        habilidade_largada,
        adaptabilidade,
        fator_chuva,
        fitness,
        experiencia,
        aggression,
        smoothness,
        mentalidade,
        confianca,
    );
}

/// Correlação de Pearson entre skill e cada atributo, num grid grande. É a assinatura estrutural
/// de um gerador de pilotos — e comparar a deste harness com a do gerador REAL do jogo é o único
/// jeito de garantir que a régua não drifte.
pub fn assinatura_de_correlacao(
    atributos: &[(&'static str, Vec<f64>)],
    skill: &[f64],
) -> Vec<(&'static str, f64)> {
    let n = skill.len() as f64;
    let media = |v: &[f64]| v.iter().sum::<f64>() / n;
    let ms = media(skill);
    atributos
        .iter()
        .map(|(nome, valores)| {
            let mv = media(valores);
            let mut cov = 0.0;
            let mut vs = 0.0;
            let mut vv = 0.0;
            for (s, x) in skill.iter().zip(valores.iter()) {
                cov += (s - ms) * (x - mv);
                vs += (s - ms).powi(2);
                vv += (x - mv).powi(2);
            }
            let rho = if vs > f64::EPSILON && vv > f64::EPSILON {
                cov / (vs.sqrt() * vv.sqrt())
            } else {
                0.0
            };
            (*nome, rho)
        })
        .collect()
}

/// Extrai a assinatura de correlação de um grid deste gerador.
pub fn assinatura_do_campo(grid: &[SimDriver]) -> Vec<(&'static str, f64)> {
    let coluna = |f: fn(&SimDriver) -> f64| -> Vec<f64> { grid.iter().map(f).collect() };
    let skill = coluna(|d| d.skill as f64);
    let atributos: Vec<(&'static str, Vec<f64>)> = vec![
        ("consistencia", coluna(|d| d.consistencia as f64)),
        ("racecraft", coluna(|d| d.racecraft as f64)),
        ("defesa", coluna(|d| d.defesa as f64)),
        (
            "ritmo_classificacao",
            coluna(|d| d.ritmo_classificacao as f64),
        ),
        ("gestao_pneus", coluna(|d| d.gestao_pneus as f64)),
        (
            "habilidade_largada",
            coluna(|d| d.habilidade_largada as f64),
        ),
        ("adaptabilidade", coluna(|d| d.adaptabilidade as f64)),
        ("mentalidade", coluna(|d| d.mentalidade as f64)),
        ("fator_chuva", coluna(|d| d.fator_chuva as f64)),
        ("aggression", coluna(|d| d.aggression as f64)),
        ("confianca", coluna(|d| d.confianca as f64)),
        ("smoothness", coluna(|d| d.smoothness as f64)),
    ];
    assinatura_de_correlacao(&atributos, &skill)
}

/// Assinatura de correlação do gerador REAL do jogo, para a mesma categoria.
///
/// Usa `Driver::generate_for_category` direto — é a fonte, não uma cópia. Se alguém mudar a
/// repartição em `driver_generation.rs`, esta função acompanha e o guard acusa a divergência.
pub fn comparar_com_gerador_real(
    perfil: &PerfilCampo,
    pilotos: usize,
    semente: u64,
) -> Vec<(&'static str, f64, f64)> {
    use rand::SeedableRng;
    let mut rng = StdRng::seed_from_u64(semente);
    let mut nomes = std::collections::HashSet::new();
    let reais = Driver::generate_for_category(
        &perfil.category_id,
        perfil.category_tier,
        "medio",
        pilotos,
        &mut nomes,
        &mut rng,
    );

    let coluna = |f: fn(&crate::models::driver::DriverAttributes) -> f64| -> Vec<f64> {
        reais.iter().map(|d| f(&d.atributos)).collect()
    };
    let skill = coluna(|a| a.skill);
    let atributos: Vec<(&'static str, Vec<f64>)> = vec![
        ("consistencia", coluna(|a| a.consistencia)),
        ("racecraft", coluna(|a| a.racecraft)),
        ("defesa", coluna(|a| a.defesa)),
        ("ritmo_classificacao", coluna(|a| a.ritmo_classificacao)),
        ("gestao_pneus", coluna(|a| a.gestao_pneus)),
        ("habilidade_largada", coluna(|a| a.habilidade_largada)),
        ("adaptabilidade", coluna(|a| a.adaptabilidade)),
        ("mentalidade", coluna(|a| a.mentalidade)),
        ("fator_chuva", coluna(|a| a.fator_chuva)),
        ("aggression", coluna(|a| a.aggression)),
        ("confianca", coluna(|a| a.confianca)),
        ("smoothness", coluna(|a| a.smoothness)),
    ];
    let do_jogo = assinatura_de_correlacao(&atributos, &skill);
    let do_harness = assinatura_do_campo(&gerar_campo(perfil, pilotos, semente));

    do_jogo
        .into_iter()
        .zip(do_harness)
        .map(|((nome, jogo), (_, harness))| (nome, jogo, harness))
        .collect()
}

/// Média e desvio-padrão do skill, no gerador do jogo e neste harness.
///
/// Existe porque a comparação de correlação sozinha engana: ρ(skill, atributo correlacionado)
/// depende do ESPALHAMENTO do skill no campo. Com ±8 de ruído fixo, um campo de skill apertado dá
/// ρ baixo e um campo largo dá ρ alto — pela mesma regra de geração. Se o harness espalhar o
/// talento mais que o jogo, ele infla toda vantagem determinística e mede mais determinismo do que
/// existe.
pub fn espalhamento_do_skill(
    perfil: &PerfilCampo,
    pilotos: usize,
    semente: u64,
) -> ((f64, f64), (f64, f64)) {
    let resumo = |v: &[f64]| -> (f64, f64) {
        let n = v.len() as f64;
        let m = v.iter().sum::<f64>() / n;
        let dp = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
        (m, dp)
    };

    let mut rng = StdRng::seed_from_u64(semente);
    let mut nomes = std::collections::HashSet::new();
    let reais = Driver::generate_for_category(
        &perfil.category_id,
        perfil.category_tier,
        "medio",
        pilotos,
        &mut nomes,
        &mut rng,
    );
    let do_jogo: Vec<f64> = reais.iter().map(|d| d.atributos.skill).collect();
    let do_harness: Vec<f64> = gerar_campo(perfil, pilotos, semente)
        .iter()
        .map(|d| d.skill as f64)
        .collect();

    (resumo(&do_jogo), resumo(&do_harness))
}

/// ID do piloto mais forte do grid pelo skill bruto. Usado pela métrica "melhor piloto fora do
/// top 5" — é a única maneira honesta de nomear o favorito sem inspecionar a simulação.
pub fn melhor_do_grid(grid: &[SimDriver]) -> String {
    grid.iter()
        .max_by_key(|d| d.skill)
        .map(|d| d.id.clone())
        .unwrap_or_default()
}
