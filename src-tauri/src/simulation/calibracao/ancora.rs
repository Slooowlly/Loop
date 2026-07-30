//! **Âncora contra o iRacing real** — o padrão-ouro que este projeto tem e quase nenhum jogo do
//! gênero tem.
//!
//! Todas as faixas de [`super::alvos`] e [`super::atrito`] são julgamento — bem argumentado, e
//! ainda assim julgamento. Mas o Loop exporta o grid como AI roster/AI season, o iRacing corre de
//! verdade, e o resultado oficial volta por `iracing_sdk::aiseason_results`. Isso é medição
//! disponível, e enquanto não for tocada as faixas continuam sendo opinião defensável.
//!
//! ## Por que a comparação aqui é forte
//!
//! Não é "distribuição do Loop vs distribuição genérica do iRacing". O `roster_gen::skill_curve_from`
//! mapeia o skill de cada piloto do Loop para o `driver_skill` do iRacing, então o grid exportado
//! tem uma distribuição de habilidade **conhecida e controlada por nós**. O experimento é de pares
//! casados: o MESMO grid, na MESMA pista, corrido duas vezes — uma pela simulação interna, outra
//! pelo iRacing. Qualquer diferença na distribuição de resultado é atribuível ao motor, não ao
//! campo.
//!
//! ## Estado hoje
//!
//! **Não há dado real no repositório.** Todos os testes de `aiseason_results` e de
//! `result_bridge` usam JSON sintético montado com `serde_json::json!`. O protocolo de coleta
//! está em [`PROTOCOLO_DE_COLETA`], e este módulo é o ingestor que o torna acionável: assim que
//! alguém soltar arquivos de aiseason numa pasta, [`medir_pasta`] devolve as mesmas métricas que
//! o harness calcula sobre a simulação interna.
//!
//! O ingestor reusa `aiseason_results::parse_event_result` em vez de reimplementar a leitura —
//! comparar contra um parser paralelo mediria a diferença entre os dois parsers.

use std::collections::HashMap;
use std::path::Path;

use crate::iracing_sdk::aiseason_results::{parse_event_result, AiEventResult, AiResultRow};

use super::metricas::spearman;

/// Uma corrida real, normalizada para o mesmo formato que as métricas do harness consomem.
#[derive(Debug, Clone)]
pub struct CorridaReal {
    pub track_id: i64,
    /// `(identidade do piloto, grid, chegada, abandonou)`.
    pub linhas: Vec<LinhaReal>,
}

#[derive(Debug, Clone)]
pub struct LinhaReal {
    /// Identidade estável entre corridas. `cust_id` quando existe (o jogador), senão o nome —
    /// que é o que o iRacing usa para os carros de IA e é estável dentro de uma temporada.
    pub identidade: String,
    pub grid: i32,
    pub chegada: i32,
    pub dnf: bool,
}

impl CorridaReal {
    fn de_evento(evento: &AiEventResult) -> Option<Self> {
        if !evento.is_final() {
            return None;
        }
        let linhas: Vec<LinhaReal> = evento.rows.iter().map(LinhaReal::de_linha).collect();
        if linhas.len() < 3 {
            return None;
        }
        Some(Self {
            track_id: evento.track_id,
            linhas,
        })
    }
}

impl LinhaReal {
    fn de_linha(row: &AiResultRow) -> Self {
        Self {
            identidade: if row.cust_id != 0 {
                format!("cust:{}", row.cust_id)
            } else {
                format!("nome:{}", row.display_name)
            },
            grid: row.grid_position,
            chegada: row.position,
            dnf: row.is_dnf(),
        }
    }
}

// ---------------------------------------------------------------------------
// Ingestão
// ---------------------------------------------------------------------------

/// Lê um `aiseasons/<Season>.json` inteiro e devolve todas as corridas com resultado final.
///
/// Um arquivo de temporada tem N eventos; só entram os que o iRacing já fechou
/// (`AiEventResult::is_final`). Eventos não corridos são pulados em silêncio — é o caso normal
/// de uma temporada em andamento.
pub fn ler_temporada(json: &serde_json::Value) -> Vec<CorridaReal> {
    let total = json
        .get("events")
        .and_then(|e| e.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    (0..total)
        .filter_map(|i| parse_event_result(json, i))
        .filter_map(|e| CorridaReal::de_evento(&e))
        .collect()
}

/// Lê todos os `.json` de uma pasta como temporadas de IA.
pub fn ler_pasta(dir: &Path) -> Vec<CorridaReal> {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut saida = Vec::new();
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.extension().is_some_and(|e| e == "json") {
            if let Ok(texto) = std::fs::read_to_string(&caminho) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&texto) {
                    saida.extend(ler_temporada(&json));
                }
            }
        }
    }
    saida
}

// ---------------------------------------------------------------------------
// As métricas, calculadas sobre dado real
// ---------------------------------------------------------------------------

/// As métricas do harness que dá para calcular só com posição de grid, chegada e DNF — ou seja,
/// as que o resultado oficial do iRacing expõe. É a interseção entre o que medimos e o que o
/// padrão-ouro entrega, e é ela que define o que é comparável.
#[derive(Debug, Clone)]
pub struct MetricasReais {
    pub corridas: usize,
    pub pilotos_distintos: usize,
    /// **A métrica âncora**: desvio-padrão da posição de chegada de um piloto ao longo da
    /// sequência de corridas. É o sintoma que originou o projeto inteiro — a simulação mede
    /// 0,71 (rookie) e 0,45 (gt3), e o alvo proposto é 3,5–6,5 e 2,5–5,0 por julgamento.
    pub desvio_posicao: f64,
    pub spearman_grid_chegada: f64,
    pub spearman_corridas_consecutivas: f64,
    pub pct_vitorias_do_pole: f64,
    pub vencedores_distintos: usize,
    pub dnfs_por_corrida: f64,
    pub recuperacao_maxima: f64,
}

fn media(v: &[f64]) -> f64 {
    let f: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if f.is_empty() {
        return f64::NAN;
    }
    f.iter().sum::<f64>() / f.len() as f64
}

fn desvio(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = media(v);
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

/// Calcula as métricas comparáveis sobre uma sequência de corridas reais.
///
/// A convenção de DNF é a MESMA da simulação (correlações e dispersão só sobre quem terminou),
/// senão a comparação mede a diferença de convenção em vez da diferença de motor.
pub fn medir_reais(corridas: &[CorridaReal]) -> MetricasReais {
    let mut por_piloto: HashMap<&str, Vec<f64>> = HashMap::new();
    let mut rho_grid = Vec::new();
    let mut vitorias_do_pole = 0usize;
    let mut vencedores: std::collections::HashSet<&str> = Default::default();
    let mut dnfs = 0usize;
    let mut recuperacoes = Vec::new();

    for corrida in corridas {
        let terminaram: Vec<&LinhaReal> = corrida.linhas.iter().filter(|l| !l.dnf).collect();
        dnfs += corrida.linhas.len() - terminaram.len();

        let (g, f): (Vec<f64>, Vec<f64>) = terminaram
            .iter()
            .map(|l| (l.grid as f64, l.chegada as f64))
            .unzip();
        if let Some(rho) = spearman(&g, &f) {
            rho_grid.push(rho);
        }

        for l in &terminaram {
            por_piloto
                .entry(&l.identidade)
                .or_default()
                .push(l.chegada as f64);
        }

        if let Some(vencedor) = corrida.linhas.iter().find(|l| l.chegada == 1) {
            vencedores.insert(&vencedor.identidade);
            if vencedor.grid == 1 {
                vitorias_do_pole += 1;
            }
        }
        recuperacoes.push(
            terminaram
                .iter()
                .map(|l| (l.grid - l.chegada) as f64)
                .fold(f64::NEG_INFINITY, f64::max),
        );
    }

    // Correlação entre corridas consecutivas — o sintoma direto.
    let mut rho_consecutivas = Vec::new();
    for par in corridas.windows(2) {
        let mapa: HashMap<&str, f64> = par[1]
            .linhas
            .iter()
            .filter(|l| !l.dnf)
            .map(|l| (l.identidade.as_str(), l.chegada as f64))
            .collect();
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for l in par[0].linhas.iter().filter(|l| !l.dnf) {
            if let Some(pos) = mapa.get(l.identidade.as_str()) {
                xs.push(l.chegada as f64);
                ys.push(*pos);
            }
        }
        if let Some(rho) = spearman(&xs, &ys) {
            rho_consecutivas.push(rho);
        }
    }

    let desvios: Vec<f64> = por_piloto
        .values()
        .filter(|v| v.len() >= 2)
        .map(|v| desvio(v))
        .collect();
    let n = corridas.len().max(1) as f64;

    MetricasReais {
        corridas: corridas.len(),
        pilotos_distintos: por_piloto.len(),
        desvio_posicao: media(&desvios),
        spearman_grid_chegada: media(&rho_grid),
        spearman_corridas_consecutivas: media(&rho_consecutivas),
        pct_vitorias_do_pole: vitorias_do_pole as f64 / n,
        vencedores_distintos: vencedores.len(),
        dnfs_por_corrida: dnfs as f64 / n,
        recuperacao_maxima: media(&recuperacoes),
    }
}

/// Atalho de ponta a ponta: aponte para a pasta de aiseasons e receba as métricas.
pub fn medir_pasta(dir: &Path) -> Option<MetricasReais> {
    let corridas = ler_pasta(dir);
    if corridas.is_empty() {
        return None;
    }
    Some(medir_reais(&corridas))
}

/// Pasta padrão do iRacing na máquina, quando existe. `None` fora do Windows ou sem instalação.
pub fn pasta_padrao() -> Option<std::path::PathBuf> {
    crate::iracing_sdk::paths::aiseasons_dir().filter(|d| d.exists())
}

// ---------------------------------------------------------------------------
// O pedido de coleta
// ---------------------------------------------------------------------------

/// O que precisa ser coletado para a âncora sair do papel. Escrito como pedido acionável: alguém
/// com o jogo instalado consegue satisfazer isto sem tocar em código.
pub const PROTOCOLO_DE_COLETA: &str = "\
FORMATO — os arquivos já existem na máquina de quem joga: \
`Documentos/iRacing/aiseasons/<Season>.json`. É o mesmo arquivo que o Loop escreve ao exportar e \
que o iRacing reescreve com `events[i].results` depois de cada etapa. Copiar o .json inteiro \
basta; `iracing_sdk::aiseason_results::parse_event_result` já sabe lê-lo e é o parser que este \
módulo reusa.

QUANTAS CORRIDAS — o desvio-padrão da posição de chegada por piloto é o alvo principal e precisa \
de várias corridas do MESMO grid. Com 20 pilotos: 8 corridas dão erro-padrão de ~0,25 posição no \
desvio, o suficiente para distinguir 0,71 (o que a simulação faz) de 3,5 (o piso do alvo). \
12 corridas é o ideal — uma temporada inteira, que é o que o exportador já gera. Menos de 5 não \
serve para nada.

DE QUE CATEGORIAS — no mínimo `mazda_rookie` e uma de topo, pelas mesmas razões do baseline: a \
hipótese é que elas devam ser caóticas por motivos opostos. Se só der para uma, que seja a \
rookie: carro spec elimina a variável de carro e isola o piloto, que é o caso mais limpo.

CONDIÇÃO DE VALIDADE — a mesma temporada, o mesmo roster, sem troca de piloto no meio. É a \
fixidez do grid que torna a dispersão interpretável, exatamente como no harness.

O QUE CADA MÉTRICA EXIGE do JSON (tudo já está em `AiResultRow`):
  - desvio da posição / vencedores distintos / trocas de liderança: `position` + identidade.
  - Spearman grid×chegada e entre corridas: `position` + `grid_position`.
  - vitórias do pole: `grid_position == 1`.
  - recuperação máxima: `grid_position - position`.
  - taxa de abandono: `reason_out`.
NÃO dá para ancorar, porque o resultado oficial não traz: posição por segmento, gaps intermediários, \
tentativas de ultrapassagem, tempo em ar sujo. As métricas de PROCESSO e as do pacote D continuam \
sem padrão-ouro — só as de RESULTADO são ancoráveis.

DIFICULDADE DA IA — anotar junto o `driver_skill` exportado (o `roster_gen::skill_curve_from` \
mapeia o skill do Loop para a força da IA do iRacing). Sem isso a comparação vira 'nosso grid \
contra um grid qualquer' em vez do experimento de pares casados que ela pode ser.";

/// Lacuna de infraestrutura encontrada durante a investigação — reportada, não alterada.
pub const LACUNA_NO_BANCO: &str = "\
A tabela `race_results` NÃO tem coluna marcando a ORIGEM do resultado: uma corrida que veio do \
iRacing pela `result_bridge` e uma que a simulação interna produziu ficam indistinguíveis no \
save. Consequência para a calibração: mesmo num save de jogador com etapas reais disputadas, o \
harness não consegue separar as linhas reais das simuladas — e é justamente o save de jogador a \
fonte mais provável de dado real em volume.

Uma coluna `origem TEXT NOT NULL DEFAULT 'simulada'` (valores 'simulada' | 'iracing') resolveria, \
preenchida em `commands::iracing::resultado` no caminho que já grava o resultado oficial. É uma \
migração de uma linha no array MIGRATIONS. Fora da fronteira deste pacote.";
