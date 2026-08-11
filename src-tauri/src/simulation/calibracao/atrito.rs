//! **Atrito de posição** — as métricas e os alvos do pacote D (ar sujo e ultrapassagem),
//! definidos ANTES de D existir.
//!
//! A razão de este arquivo vir primeiro é a lição central do projeto: ninguém mediu antes de
//! tunar, e foi assim que se chegou a cinco corridas com o mesmo resultado. O D é o pacote mais
//! importante que sobrou; ele tem que ser construído CONTRA alvo, não ajustado depois.
//!
//! ## O que já dá para medir e o que falta
//!
//! Derivável hoje de `RaceResult`: recuperação máxima, forma da distribuição de gaps, fração do
//! pelotão dentro da janela de 1 s ao fim da corrida, e a sonda de grade sorteada
//! ([`super::processo::medir_poder_da_largada`]).
//!
//! Pendente do campo `posicoes_por_segmento` (pedido ao pacote C, ver [`DADO_PENDENTE`]): a curva
//! ρ(segmento × chegada), o segmento de estabilização, as trocas por segmento e o trem de carros.
//! Toda a matemática dessas métricas está escrita e testada aqui contra entrada sintética — o que
//! falta é uma linha em [`posicoes_por_segmento`], e não a lógica.
//!
//! Fora do alcance de qualquer dado existente, e portanto PEDIDO AO PACOTE D (ver
//! [`DADO_PEDIDO_AO_D`]): tentativas de ultrapassagem, taxa de sucesso e voltas passadas em ar
//! sujo. São subprodutos naturais da implementação do D; pedir agora significa que ele os emite
//! por construção em vez de ser retrofitado.

use crate::simulation::context::SimDriver;
use crate::simulation::race::{RaceDriverResult, RaceResult};

use super::metricas::spearman;

/// O campo pedido ao pacote C. Quando ele existir, [`posicoes_por_segmento`] vira uma linha.
pub const DADO_PENDENTE: &str = "\
`RaceDriverResult::posicoes_por_segmento: Vec<i32>` (5 entradas, a posição ao fim de cada \
segmento). Existe durante a corrida em `RaceState::current_position` e morre em \
`build_race_results`. Pedido ao pacote C.";

/// Os campos que o pacote D deveria emitir por construção — nenhum deles é derivável de nada que
/// exista hoje, e todos são subproduto natural de um modelo de posição na pista.
pub const DADO_PEDIDO_AO_D: &str = "\
Por piloto, no `RaceDriverResult`:\n\
  - `tentativas_ultrapassagem: i32` — quantas vezes tentou passar alguém.\n\
  - `ultrapassagens_concluidas: i32` — quantas vieram. A razão entre os dois é a taxa de \
sucesso, e é o parâmetro central do D: hoje ela é implicitamente 100%.\n\
  - `segmentos_em_ar_sujo: i32` — em quantos dos 5 segmentos o carro terminou dentro da janela \
de perda de apoio do carro da frente. Sem isto, 'fração de tempo em ar sujo' só dá para ser \
aproximada pelo retrato final da bandeirada.";

// ---------------------------------------------------------------------------
// Adaptador do dado pendente
// ---------------------------------------------------------------------------

/// Posições ao fim de cada segmento, por piloto. `None` enquanto o campo não existir.
///
/// **Ponto de ligação**: quando `RaceDriverResult::posicoes_por_segmento` entrar, o corpo vira
/// `Some(corrida.race_results.iter().map(|r| (r.pilot_id.clone(), r.posicoes_por_segmento.clone())).collect())`
/// e todas as métricas de segmento abaixo passam a rodar sobre corrida de verdade. Nada mais muda.
pub fn posicoes_por_segmento(corrida: &RaceResult) -> Option<Vec<(String, Vec<i32>)>> {
    let _ = corrida;
    None
}

// ---------------------------------------------------------------------------
// Métricas de segmento (matemática pronta, entrada pendente)
// ---------------------------------------------------------------------------

pub const SEGMENTOS: usize = 5;

/// Acima deste ρ contra a ordem final, considera-se que a corrida já está definida.
/// 0,95 é deliberadamente severo: é o ponto em que sobram, num grid de 20, no máximo umas duas
/// trocas de posição até a bandeirada.
pub const LIMIAR_DE_ESTABILIZACAO: f64 = 0.95;

/// Diferença de skill a partir da qual o carro da frente conta como "estruturalmente mais lento".
/// Abaixo disso, estar atrás dele é disputa normal, não trem.
pub const MARGEM_DE_RITMO: f64 = 5.0;

#[derive(Debug, Clone)]
pub struct MetricasSegmento {
    /// ρ(ordem no segmento s, ordem final), s = Start..Finish. A curva inteira, porque ela diz
    /// mais que qualquer número derivado dela: se já começa em 0,95, a corrida acaba na largada.
    pub rho_por_segmento: [f64; SEGMENTOS],
    /// Primeiro segmento (1..5) em que ρ passa de [`LIMIAR_DE_ESTABILIZACAO`] e não volta mais.
    pub segmento_de_estabilizacao: f64,
    /// Trocas de posição (distância de Kendall) DENTRO de cada segmento. O índice 0 mede
    /// grid → fim do Start, e por isso é o único que deveria concentrar.
    pub trocas_por_segmento: [f64; SEGMENTOS],
    /// Fração das células (piloto × segmento) em que o piloto terminou o segmento atrás de
    /// alguém estruturalmente mais lento. É a medida direta de trânsito.
    pub fracao_travado: f64,
    /// Maior número de segmentos CONSECUTIVOS que um piloto passou atrás do MESMO carro mais
    /// lento. É a medida direta de trem de carros: 1 é disputa, 4 é comboio.
    pub maior_sequencia_travado: f64,
}

fn kendall(a: &[f64], b: &[f64]) -> f64 {
    let mut total = 0.0;
    for i in 0..a.len() {
        for j in (i + 1)..a.len() {
            if (a[i] - a[j]) * (b[i] - b[j]) < 0.0 {
                total += 1.0;
            }
        }
    }
    total
}

/// `posicoes[i] = (pilot_id, [pos ao fim de cada um dos 5 segmentos])`.
/// `grid_inicial[i]` = posição de largada do mesmo piloto, na mesma ordem.
pub fn medir_segmentos(
    pilotos: &[SimDriver],
    posicoes: &[(String, Vec<i32>)],
    grid_inicial: &[i32],
) -> MetricasSegmento {
    let n = posicoes.len();
    let mut rho_por_segmento = [f64::NAN; SEGMENTOS];
    let mut trocas_por_segmento = [f64::NAN; SEGMENTOS];

    if n < 3 || posicoes.iter().any(|(_, v)| v.len() < SEGMENTOS) {
        return MetricasSegmento {
            rho_por_segmento,
            segmento_de_estabilizacao: f64::NAN,
            trocas_por_segmento,
            fracao_travado: f64::NAN,
            maior_sequencia_travado: f64::NAN,
        };
    }

    let coluna = |s: usize| -> Vec<f64> { posicoes.iter().map(|(_, v)| v[s] as f64).collect() };
    let final_ = coluna(SEGMENTOS - 1);

    for s in 0..SEGMENTOS {
        let atual = coluna(s);
        rho_por_segmento[s] = spearman(&atual, &final_).unwrap_or(f64::NAN);
        let anterior = if s == 0 {
            grid_inicial.iter().map(|p| *p as f64).collect()
        } else {
            coluna(s - 1)
        };
        trocas_por_segmento[s] = kendall(&anterior, &atual);
    }

    // Estabilização: o primeiro segmento a partir do qual ρ nunca mais cai abaixo do limiar.
    let mut segmento_de_estabilizacao = SEGMENTOS as f64;
    for s in (0..SEGMENTOS).rev() {
        if rho_por_segmento[s].is_finite() && rho_por_segmento[s] >= LIMIAR_DE_ESTABILIZACAO {
            segmento_de_estabilizacao = (s + 1) as f64;
        } else {
            break;
        }
    }

    // --- Trânsito e trem de carros ---
    // O "ritmo estrutural" vem do skill do grid sintético: no harness nós SABEMOS quem é mais
    // rápido, o que é justamente o que permite perguntar "este ficou preso atrás de quem?".
    let skill_de =
        |id: &str| -> Option<f64> { pilotos.iter().find(|d| d.id == id).map(|d| d.skill as f64) };

    let mut celulas = 0.0;
    let mut travadas = 0.0;
    // sequencias[i] = há quantos segmentos o piloto i está atrás do MESMO carro lento.
    let mut sequencia_atual: Vec<(Option<String>, f64)> = vec![(None, 0.0); n];
    let mut maior_sequencia = 0.0_f64;

    for s in 0..SEGMENTOS {
        // Quem está em cada posição neste segmento.
        let mut por_posicao: Vec<(&str, i32)> =
            posicoes.iter().map(|(id, v)| (id.as_str(), v[s])).collect();
        por_posicao.sort_by_key(|(_, p)| *p);

        for (indice_na_pista, (id, _)) in por_posicao.iter().enumerate() {
            let Some(i) = posicoes.iter().position(|(pid, _)| pid == id) else {
                continue;
            };
            celulas += 1.0;

            let da_frente = if indice_na_pista == 0 {
                None
            } else {
                Some(por_posicao[indice_na_pista - 1].0)
            };

            let esta_travado = match (da_frente, skill_de(id)) {
                (Some(frente), Some(meu)) => skill_de(frente)
                    .map(|dele| meu - dele >= MARGEM_DE_RITMO)
                    .unwrap_or(false),
                _ => false,
            };

            if esta_travado {
                travadas += 1.0;
                let frente_id = da_frente.map(|s| s.to_string());
                if sequencia_atual[i].0 == frente_id {
                    sequencia_atual[i].1 += 1.0;
                } else {
                    sequencia_atual[i] = (frente_id, 1.0);
                }
                maior_sequencia = maior_sequencia.max(sequencia_atual[i].1);
            } else {
                sequencia_atual[i] = (None, 0.0);
            }
        }
    }

    MetricasSegmento {
        rho_por_segmento,
        segmento_de_estabilizacao,
        trocas_por_segmento,
        fracao_travado: if celulas > 0.0 {
            travadas / celulas
        } else {
            f64::NAN
        },
        maior_sequencia_travado: maior_sequencia,
    }
}

// ---------------------------------------------------------------------------
// Métricas de atrito deriváveis hoje
// ---------------------------------------------------------------------------

/// Janela dentro da qual um carro perde apoio atrás do da frente. 1 s é a convenção do
/// automobilismo e é o que o pacote D vai ter que parametrizar por categoria.
///
/// **Passou a DERIVAR da simulação, e isso conserta um espelho perigoso.** Ela nasceu como
/// literal `1_000.0` aqui, igual por coincidência ao `JANELA_AR_SUJO_MS` de `race::trafego`. O
/// problema não era a duplicação em si — era o que ela faria com a calibração: a partir do
/// momento em que a janela da simulação virou knob varrível (A1.1), a régua continuaria medindo
/// "fração do pelotão a menos de 1 s" enquanto a corrida rodava com outra janela. A varredura
/// mediria a constante contra uma régua que não a acompanha, e o resultado teria cara de
/// medida.
pub const JANELA_AR_SUJO_MS: f64 =
    crate::simulation::race::trafego::ParametrosDeTrafego::PADRAO.janela_ar_sujo_ms;

#[derive(Debug, Clone, Default)]
pub struct MetricasAtrito {
    /// Maior ganho de posição da corrida.
    pub recuperacao_maxima: f64,
    /// Fração do pelotão que cruza a bandeirada a menos de 1 s do carro da frente. É o retrato
    /// final do trânsito — o proxy possível enquanto `segmentos_em_ar_sujo` não existir.
    pub fracao_em_janela: f64,
    /// Coeficiente de variação dos gaps entre carros consecutivos. Escada regular tende a 0;
    /// trens de carros separados por buracos empurram para cima.
    pub cv_gaps: f64,
    /// Grupos separados por um buraco maior que 2× a mediana.
    pub trens: f64,
}

pub fn medir_atrito(corrida: &RaceResult) -> MetricasAtrito {
    let mut terminaram: Vec<&RaceDriverResult> =
        corrida.race_results.iter().filter(|r| !r.is_dnf).collect();
    if terminaram.len() < 3 {
        return MetricasAtrito::default();
    }
    terminaram.sort_by_key(|r| r.finish_position);

    let gaps: Vec<f64> = terminaram
        .windows(2)
        .map(|w| (w[1].gap_to_winner_ms - w[0].gap_to_winner_ms).max(0.0))
        .collect();

    let media = gaps.iter().sum::<f64>() / gaps.len() as f64;
    let dp = if gaps.len() > 1 {
        (gaps.iter().map(|g| (g - media).powi(2)).sum::<f64>() / (gaps.len() - 1) as f64).sqrt()
    } else {
        0.0
    };
    let mut ordenados = gaps.clone();
    ordenados.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mediana = ordenados[ordenados.len() / 2];

    MetricasAtrito {
        recuperacao_maxima: terminaram
            .iter()
            .map(|r| r.positions_gained)
            .max()
            .unwrap_or(0) as f64,
        fracao_em_janela: gaps.iter().filter(|g| **g < JANELA_AR_SUJO_MS).count() as f64
            / gaps.len() as f64,
        cv_gaps: if media > f64::EPSILON {
            dp / media
        } else {
            f64::NAN
        },
        trens: gaps.iter().filter(|g| **g > mediana * 2.0).count() as f64 + 1.0,
    }
}

// ---------------------------------------------------------------------------
// Os alvos
// ---------------------------------------------------------------------------

use super::alvos::Faixa;

/// Critério de aceitação do pacote D, por categoria.
///
/// Cada faixa vem com o argumento ao lado. Discordar é editar o número e o comentário juntos.
#[derive(Debug, Clone)]
pub struct AlvosAtrito {
    /// Sonda de grade sorteada: ρ(grid × chegada). Quanto da posição de largada SOBREVIVE quando
    /// ela não tem relação nenhuma com o ritmo.
    pub rho_grid_sorteado: Faixa,
    /// Sonda de grade sorteada: ρ(skill × chegada). Quanto o ritmo consegue se impor apesar da
    /// largada ruim.
    pub rho_skill_com_grid_sorteado: Faixa,
    /// ρ(fim do Start × chegada). Se isto for alto, a corrida acaba na largada.
    pub rho_start_vs_final: Faixa,
    /// Primeiro segmento em que a ordem final já está formada (1..5).
    pub segmento_de_estabilizacao: Faixa,
    /// Fração de células piloto×segmento atrás de alguém estruturalmente mais lento.
    pub fracao_travado: Faixa,
    /// Maior comboio: segmentos consecutivos atrás do mesmo carro lento.
    pub maior_sequencia_travado: Faixa,
    /// Maior ganho de posição da corrida.
    pub recuperacao_maxima: Faixa,
    /// Fração do pelotão dentro de 1 s do carro da frente na bandeirada.
    pub fracao_em_janela: Faixa,
    /// CV dos gaps entre carros consecutivos.
    pub cv_gaps: Faixa,
}

impl AlvosAtrito {
    /// **Monomarca de entrada** (mazda_rookie): carros idênticos, pouca aerodinâmica, muito
    /// rebaixo e vácuo, corrida curta (20 min, ~13 voltas).
    ///
    /// O eixo do argumento é a sonda de grade sorteada, que hoje mede ρ(grid) = 0,153 e
    /// ρ(skill) = 0,885 — o rápido atravessa o pelotão como se não houvesse ninguém no caminho.
    ///
    /// Numa MX-5 de verdade passar É barato: sem asa, o carro de trás ganha no vácuo em vez de
    /// perder apoio, e uma diferença de 0,5 s/volta se converte em posição quase toda vez. Mas
    /// não é de graça, e o tempo é curto: em ~13 voltas, quem larga em 20º e é o mais rápido do
    /// grid chega tipicamente entre 4º e 8º, não em 1º. Isso corresponde a **ρ(skill) por volta
    /// de 0,65–0,82 e ρ(grid) de 0,30–0,50**.
    ///
    /// A escolha de deixar a soma dos quadrados abaixo de 1 é proposital: sobra espaço para o
    /// ruído que C e D introduzem, e uma corrida em que grid e ritmo explicam 100% do resultado
    /// é outra forma de determinismo.
    pub fn entrada() -> Self {
        Self {
            rho_grid_sorteado: Faixa::nova(0.30, 0.50),
            rho_skill_com_grid_sorteado: Faixa::nova(0.65, 0.82),
            // Se o Start já explica 90% da ordem final, os outros quatro segmentos são
            // decorativos. Numa categoria fácil de passar, o Start deve valer pouco.
            rho_start_vs_final: Faixa::nova(0.40, 0.75),
            // A ordem não pode estar fechada antes do Mid.
            segmento_de_estabilizacao: Faixa::nova(3.0, 5.0),
            // Passar é barato, então trânsito existe mas dura pouco.
            fracao_travado: Faixa::nova(0.08, 0.22),
            maior_sequencia_travado: Faixa::nova(1.5, 3.0),
            // Hoje 4,1. Tem que SUBIR: ver a nota sobre recuperação abaixo.
            recuperacao_maxima: Faixa::nova(5.0, 10.0),
            // Monomarca chega colada. Metade do pelotão dentro de 1 s é conservador.
            fracao_em_janela: Faixa::nova(0.35, 0.70),
            cv_gaps: Faixa::nova(1.20, 2.50),
        }
    }

    /// **Categoria de topo** (gt3): muita aerodinâmica, ar sujo severo, corrida longa (45 min,
    /// ~25–30 voltas) e diferença de carro que responde por metade da variância permanente.
    ///
    /// Aqui há uma **tensão real que o briefing não menciona**, e ela é a razão de a separação
    /// entre as duas categorias ser modesta em vez de dramática:
    ///
    /// - Ar sujo empurra para MAIS persistência de grid: uma GT3 a 0,3 s/volta mais rápida
    ///   frequentemente não converte nada, porque perde justamente nas curvas onde precisaria
    ///   se aproximar.
    /// - Mas a corrida é **mais que o dobro** da rookie, e a diferença de ritmo é maior (o carro
    ///   vale ~50% da variância, contra ~0 na spec). Mais tempo e mais delta empurram de volta
    ///   para o ritmo.
    ///
    /// Os dois efeitos se cancelam em boa parte. Por isso proponho apenas um deslocamento de
    /// ~0,10 em relação à entrada, e não a inversão que "GT3 é muito mais difícil de passar"
    /// sugeriria se olhasse só a aerodinâmica: **ρ(grid) 0,40–0,60, ρ(skill) 0,55–0,75**.
    ///
    /// Onde a gt3 DEVE se separar claramente é no trânsito, não na correlação final: o comboio é
    /// o fenômeno característico dela. Daí `fracao_travado` e `maior_sequencia_travado` bem mais
    /// altos, e `fracao_em_janela` maior mesmo com correlações parecidas.
    pub fn topo() -> Self {
        Self {
            rho_grid_sorteado: Faixa::nova(0.40, 0.60),
            rho_skill_com_grid_sorteado: Faixa::nova(0.55, 0.75),
            rho_start_vs_final: Faixa::nova(0.50, 0.82),
            segmento_de_estabilizacao: Faixa::nova(3.0, 5.0),
            // O comboio é o fenômeno característico da categoria.
            fracao_travado: Faixa::nova(0.20, 0.40),
            maior_sequencia_travado: Faixa::nova(2.5, 5.0),
            // Hoje 2,0. Tem que subir MAIS que na rookie, apesar de passar ser mais caro —
            // porque a corrida é longa e o delta de carro é grande.
            recuperacao_maxima: Faixa::nova(4.0, 8.0),
            fracao_em_janela: Faixa::nova(0.30, 0.65),
            cv_gaps: Faixa::nova(1.40, 3.00),
        }
    }
}

/// A armadilha número um do pacote D, escrita para não ser esquecida.
///
/// Hoje a recuperação máxima é 4,1 (rookie) e 2,0 (gt3), e os alvos acima pedem que ela SUBA
/// enquanto a ultrapassagem fica mais CARA. Parece contraditório e não é: hoje ninguém recupera
/// porque ninguém está fora de posição — o grid sai na ordem do ritmo e a corrida a confirma.
/// Não há de onde recuperar.
///
/// Depois do D, carros vão terminar fora de posição com frequência (preso em trem, largada ruim,
/// ar sujo no momento errado), e é isso que cria a recuperação. Atrito e recuperação sobem
/// JUNTOS, porque um cria a matéria-prima do outro.
///
/// **Sinal de alarme**: se o D entrar e a recuperação máxima CAIR abaixo dos 4 de hoje, ele
/// adicionou atrito sem adicionar desalinhamento. Nesse caso a corrida ficou mais travada, não
/// mais disputada — que é o modo de falha mais provável deste pacote.
pub const ARMADILHA_DA_RECUPERACAO: &str = "\
Atrito e recuperação sobem juntos. Se a recuperação máxima cair abaixo de 4,0 (rookie) ou 2,0 \
(gt3) depois do D, ele adicionou atrito sem criar desalinhamento — corrida mais travada, não \
mais disputada.";
