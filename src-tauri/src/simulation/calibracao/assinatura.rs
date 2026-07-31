//! **Assinatura temporal** — a propriedade que o portão do orçamento não mede.
//!
//! ## A classe de problema, e ela é maior que um parâmetro
//!
//! O portão da decomposição existe para impedir que "as oito métricas ficaram verdes" seja aceito
//! quando a distribuição bateu pela razão errada. Ele funciona: pegou `race_variance = 10` com 68,7%
//! de ruído puro. Mas ele mede **de onde a variância vem**, e há uma família de defeitos que ele não
//! enxerga — **onde ela vem do lugar certo e perdeu a assinatura no tempo**.
//!
//! O caso que revelou a classe: `FORMA_RHO = 0` com `FORMA_ESCALA_PONTOS = 9,0` leva ρ(N × N+1) a
//! 0,550, o teto exato da faixa-alvo. E o orçamento sai perfeito — forma com ρ = 0 continua sendo
//! variância de EVENTO, atribuível a qualidade, dentro do teto de azar, na fatia certa. O portão
//! aprova.
//!
//! Só que **forma sem memória é acerto de fim de semana com outro nome**. A camada 2 existe para
//! produzir SEQUÊNCIAS — três pódios e depois uma queda, o arco que faz uma temporada ter forma em
//! vez de ser N amostras independentes. Com ρ = 0 o alvo é atingido apagando a razão de ser da
//! camada, e nenhuma das oito métricas de resultado nem o portão do orçamento reclama.
//!
//! E a busca **vai** fazer isso se puder: zerar `FORMA_RHO` é o caminho mais barato para derrubar
//! ρ(N × N+1), porque é o único parâmetro que derruba o numerador da correlação sem precisar
//! aumentar o denominador.
//!
//! ## O instrumento: teste de permutação dentro da temporada
//!
//! Medir "dá para perceber a sequência?" com correlação seria o instrumento errado: o jogador não
//! calcula autocorrelação, ele **nota emendas** — "esse cara engatou três bons fins de semana". E a
//! amostra que ele tem é uma temporada de um piloto, 12 a 24 corridas, onde o erro-padrão de uma
//! correlação é ~0,3 e nada abaixo disso seria distinguível de qualquer forma.
//!
//! Então a medida é a **maior emenda**: quantos resultados consecutivos acima da mediana pessoal do
//! piloto ele encadeia numa temporada. E o nulo certo é **embaralhar a ordem daquela mesma
//! temporada**, o que destrói a assinatura temporal e preserva exatamente a distribuição — um teste
//! de permutação, não um modelo.
//!
//! `excesso = maior emenda real − maior emenda embaralhada`. É zero por construção quando a fonte
//! não tem memória, e é em corridas — uma unidade que dá para defender numa conversa sobre design.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use crate::simulation::context::SimDriver;
use crate::simulation::race::RaceResult;

/// Quantas permutações por temporada-piloto. 24 basta: o nulo é uma média, não um quantil.
const PERMUTACOES: usize = 24;

/// Tamanho de emenda que a interface promete quando mostra seta de tendência. "Três pódios
/// seguidos" é a frase que o jogador usa; três é o número.
pub const EMENDA_QUE_SE_PERCEBE: usize = 3;

#[derive(Debug, Clone, Copy, Default)]
pub struct AssinaturaTemporal {
    /// **A métrica que conta.** Fração de temporadas-piloto que contêm uma emenda de pelo menos
    /// [`EMENDA_QUE_SE_PERCEBE`] resultados consecutivos acima da MÉDIA do próprio piloto.
    ///
    /// Substituiu o excesso de emenda média como grandeza principal, e a razão é a certa: o
    /// jogador não percebe correlação nem comprimento médio, ele percebe **contagem** — "esse cara
    /// engatou três bons fins de semana". Medir o proxy estatístico quando dá para medir o
    /// fenômeno que a interface promete era escolha ruim, e o critério derivado dele ("meia corrida
    /// de excesso") não era atingido por ρ nenhum, o que já era sinal de que o critério estava
    /// errado e não só a amplitude.
    pub p_emenda_percebida: f64,
    /// A mesma fração com a ordem embaralhada — o nulo do teste de permutação.
    pub p_emenda_percebida_nula: f64,
    /// `p_emenda_percebida − p_emenda_percebida_nula`, em pontos de probabilidade. **É a métrica
    /// com faixa-alvo declarada**, ao lado das oito de resultado. Ver [`FAIXA_DE_EMENDA`].
    pub excesso_de_emenda_percebida: f64,

    /// Maior emenda média de resultados acima da mediana pessoal, em corridas.
    pub maior_emenda: f64,
    /// A mesma coisa com a ordem embaralhada dentro da temporada — o nulo.
    pub maior_emenda_nula: f64,
    /// `maior_emenda − maior_emenda_nula`. Mantido como diagnóstico: é a mesma informação numa
    /// unidade que não é a que se percebe.
    pub excesso_de_emenda: f64,
    /// Autocorrelação de defasagem 1 do resíduo (posição menos a média do piloto na temporada).
    /// Fica como diagnóstico: é a grandeza que a teoria prevê, mas não é a que se percebe.
    pub autocorrelacao_do_residuo: f64,
    pub temporadas: usize,
}

fn maior_emenda(acima: &[bool]) -> f64 {
    let mut melhor = 0usize;
    let mut atual = 0usize;
    for &a in acima {
        if a {
            atual += 1;
            melhor = melhor.max(atual);
        } else {
            atual = 0;
        }
    }
    melhor as f64
}

fn mediana(v: &[f64]) -> f64 {
    let mut o = v.to_vec();
    o.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = o.len();
    if n == 0 {
        return 0.0;
    }
    if n % 2 == 1 {
        o[n / 2]
    } else {
        (o[n / 2 - 1] + o[n / 2]) / 2.0
    }
}

/// Mede a assinatura temporal de uma campanha crua (a saída de `arena::rodar_campanha_crua`).
pub fn medir(campanha: &[(Vec<SimDriver>, Vec<RaceResult>)], semente: u64) -> AssinaturaTemporal {
    let mut rng = StdRng::seed_from_u64(semente);

    let mut soma_real = 0.0;
    let mut soma_nula = 0.0;
    let mut soma_p_real = 0.0;
    let mut soma_p_nula = 0.0;
    let mut casos = 0.0f64;

    let mut num_auto = 0.0;
    let mut den_auto = 0.0;

    for (grid, corridas) in campanha {
        for piloto in grid {
            // A sequência de posições deste piloto ao longo da temporada.
            let posicoes: Vec<f64> = corridas
                .iter()
                .filter_map(|c| {
                    c.race_results
                        .iter()
                        .find(|r| r.pilot_id == piloto.id)
                        .map(|r| r.finish_position as f64)
                })
                .collect();
            if posicoes.len() < 4 {
                continue;
            }

            // Posição MENOR é resultado melhor, então "acima da mediana" é `< mediana`.
            let med = mediana(&posicoes);
            let acima: Vec<bool> = posicoes.iter().map(|p| *p < med).collect();

            // A métrica que conta usa a MÉDIA do piloto, não a mediana: é o que a leitura da
            // interface compara ("acima do seu normal"), e a mediana força metade acima por
            // construção, o que apaga a assimetria de quem tem poucos resultados muito bons.
            let media_pos = posicoes.iter().sum::<f64>() / posicoes.len() as f64;
            let acima_da_media: Vec<bool> = posicoes.iter().map(|p| *p < media_pos).collect();

            soma_real += maior_emenda(&acima);
            if maior_emenda(&acima_da_media) >= EMENDA_QUE_SE_PERCEBE as f64 {
                soma_p_real += 1.0;
            }

            let mut embaralhado = acima.clone();
            let mut soma_perm = 0.0;
            let mut emb_media = acima_da_media.clone();
            let mut soma_p_perm = 0.0;
            for _ in 0..PERMUTACOES {
                embaralhado.shuffle(&mut rng);
                soma_perm += maior_emenda(&embaralhado);

                emb_media.shuffle(&mut rng);
                if maior_emenda(&emb_media) >= EMENDA_QUE_SE_PERCEBE as f64 {
                    soma_p_perm += 1.0;
                }
            }
            soma_nula += soma_perm / PERMUTACOES as f64;
            soma_p_nula += soma_p_perm / PERMUTACOES as f64;
            casos += 1.0;

            // Autocorrelação de defasagem 1 do resíduo, acumulada sobre todos os pilotos.
            let media = posicoes.iter().sum::<f64>() / posicoes.len() as f64;
            let res: Vec<f64> = posicoes.iter().map(|p| p - media).collect();
            for i in 0..res.len() - 1 {
                num_auto += res[i] * res[i + 1];
            }
            den_auto += res.iter().map(|r| r * r).sum::<f64>();
        }
    }

    let n = casos.max(1.0);
    let real = soma_real / n;
    let nula = soma_nula / n;

    let p_real = soma_p_real / n;
    let p_nula = soma_p_nula / n;

    AssinaturaTemporal {
        p_emenda_percebida: p_real,
        p_emenda_percebida_nula: p_nula,
        excesso_de_emenda_percebida: p_real - p_nula,
        maior_emenda: real,
        maior_emenda_nula: nula,
        excesso_de_emenda: real - nula,
        autocorrelacao_do_residuo: if den_auto > 0.0 {
            num_auto / den_auto
        } else {
            0.0
        },
        temporadas: campanha.len(),
    }
}

/// **Faixa-alvo do excesso de emenda percebida** — a nona métrica, ao lado das oito de resultado.
///
/// Ela é alvo e não consequência esperada, e a razão é dura: a interface **já** mostra seta de
/// tendência na forma, e a medição diz que com a amplitude de hoje o excesso é ~0 — a tela afirma um
/// mecanismo que mal existe. Deixar isso como efeito colateral esperado do aumento de amplitude
/// seria confiar que uma promessa já feita se cumpra sozinha.
///
/// Os números que a sustentam, medidos (rookie, 12 etapas, ρ = 0,65, 40 temporadas):
///
/// | `FORMA_ESCALA` | P(emenda 3+) | nulo | excesso |
/// |---|---|---|---|
/// | **2,0 (hoje)** | 0,660 | 0,614 | **0,046** |
/// | 4,0 | 0,654 | 0,600 | 0,054 |
/// | 6,0 | 0,674 | 0,588 | 0,086 |
/// | 9,0 | 0,718 | 0,588 | 0,129 |
/// | 14,0 | 0,757 | 0,583 | 0,174 |
///
/// Piso **0,08**: quatro vezes o ruído do instrumento (com ρ = 0 o excesso mede −0,016 a −0,002, o
/// que também valida o nulo) e claramente acima do que o jogo entrega hoje. Teto **0,20**: acima de
/// ~0,17 seria preciso amplitude que a fase 1 não vai ter, e emenda em quase toda temporada faz
/// "estar em forma" deixar de ser notícia.
///
/// ## O conflito que esta faixa revela, e é melhor sabê-lo antes
///
/// A eficiência-ρ diz para tirar peso da forma (autocorrelação 0,65 contra 0 do acerto e da
/// afinidade, logo ela é a pior das três para derrubar ρ(N × N+1)). Esta métrica diz o contrário:
/// **só a forma produz emenda**, porque só ela tem memória. As duas puxam em direções opostas sobre
/// o mesmo parâmetro.
///
/// E o bracket da fase 1 (×1,4–2,2 sobre a camada inteira) leva a forma de 2,0 para ~2,8–4,4, faixa
/// em que o excesso vai de 0,046 para ~0,054 — **não chega ao piso**. Ou seja: o aumento de
/// amplitude que a campanha já ia fazer não entrega a sequência de brinde. É preciso decidir, e é
/// decisão de design, não de busca.
pub const FAIXA_DE_EMENDA: (f64, f64) = (0.08, 0.20);

/// **O piso de `FORMA_RHO` para a busca da fase 1.**
///
/// Adotado como PRESERVAÇÃO, não como perceptibilidade — e o critério anterior foi retirado.
///
/// A primeira versão desta doc dizia "o menor ρ em que o excesso passa de meia corrida em 12
/// etapas". Esse critério **não é atingido por ρ nenhum**, nem pelo valor de hoje, o que já era
/// evidência de que ele estava errado e não só a amplitude. Duas coisas estavam mal escolhidas: a
/// janela (12 etapas é curta) e, principalmente, a grandeza — excesso de comprimento médio é proxy
/// estatístico, e quem percebe percebe **contagem**. Daí [`AssinaturaTemporal::p_emenda_percebida`].
///
/// O que sustenta 0,45: ele preserva ~70% do excesso do valor de hoje (0,65) nos três cenários
/// medidos, e a perceptibilidade de verdade vem do aumento de amplitude que a fase 1 vai fazer de
/// qualquer forma — o ρ decide a FORMA da memória, a escala decide se há o que lembrar.
///
/// Ver a seção 7.9 do [CAMPANHA.md](CAMPANHA.md) para as tabelas.
pub const PISO_DE_FORMA_RHO: f64 = 0.45;

/// **O catálogo da classe** — parâmetros cujo valor degenerado satisfaz o portão do orçamento e
/// mata o mecanismo. Nenhum deles pode ter o valor degenerado dentro da faixa varrida.
///
/// O critério de entrada: mexer no parâmetro em direção ao degenerado **não muda a fatia do
/// orçamento** (ou muda pouco), e destrói uma propriedade que nenhuma das oito métricas de resultado
/// mede. É por isso que o portão passa.
pub const DEGENERADOS_QUE_O_PORTAO_NAO_PEGA: &[(&str, &str, &str)] = &[
    (
        "FORMA_RHO",
        "0 (sem memória)",
        "forma vira acerto com outro nome: a camada perde as sequências, que são a razão de ela \
         existir. E há dependência de interface — a leitura do fim de semana mostra seta de \
         tendência na forma, e com ρ = 0 a seta afirma um mecanismo que deixou de existir.",
    ),
    // REMOVIDO DO CATÁLOGO — e o motivo importa mais que a entrada.
    //
    // Eu havia listado `FORMA_PESO_ANIMO = 0` como degenerado a evitar, com o argumento de que
    // zerá-lo mataria o laço pódio → confiança → ritmo. **Estava errado, e por um erro de método:**
    // eu classifiquei olhando só o que se perde, sem verificar se o mecanismo já existia em outro
    // lugar. Existe — motivação e confiança têm elo próprio na esteira e peso próprio no score, e o
    // termo do ânimo era um TERCEIRO caminho para as mesmas duas grandezas.
    //
    // Zerá-lo não mata o laço; remove uma duplicata que, por cima, contaminava a camada de evento
    // com deslocamento permanente (termo constante em AR(1) vira média estacionária `c/(1−ρ)`).
    // A decisão foi remover, e ela é de mecanismo, não de magnitude.
    //
    // A lição para o critério de entrada deste catálogo: **"destrói uma propriedade" não basta —
    // tem que ser uma propriedade que só aquele parâmetro entrega.** Duplicata some sem perda.
    (
        "AFINIDADE_FRACAO_IDIOSSINCRATICA",
        "1 (só idiossincrasia)",
        "a afinidade vira ruído por par (piloto, pista) e deixa de ser LEGÍVEL — some a parte que \
         casa estilo com caráter de circuito, que é o que explica por que aquele piloto voa ali. \
         Variância idêntica; o portão é cego por construção. Nenhum outro parâmetro entrega essa \
         legibilidade, então ela não é duplicata.",
    ),
    (
        "MULT_AFINIDADE_QUALI",
        "1 (igual à corrida)",
        "some a distinção entre ser rápido no sábado e no domingo. A distribuição não muda de \
         tamanho, só deixa de ter dois eixos — e `reprodutibilidade_do_grid` mediria a quali como \
         saudável do mesmo jeito.",
    ),
];
