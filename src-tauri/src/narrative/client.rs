//! Cliente do servidor de boletins.
//!
//! O app NUNCA fala com um provedor de IA direto — só com o NOSSO servidor (Cloud Run
//! `iracer-news`), que guarda as chaves e ESCOLHE quem redige. O segredo abaixo é
//! embutido por design (client-side): ele vai no binário e serve só como "porta de
//! entrada" do servidor; a defesa real contra abuso é o cooldown por install_id + o
//! teto global de gasto.
//!
//! **Quem redige é decisão do servidor, não daqui.** São DOIS provedores atrás da mesma
//! interface: DeepSeek (`deepseek-v4-flash`) é o principal por ser mais barato, e Gemini
//! (`gemini-2.5-flash-lite`) assume no HORÁRIO DE PICO do DeepSeek — que cobra 2x em
//! UTC 01:00–04:00 e 06:00–10:00 — e também como FALLBACK sempre que o primário falha.
//! A ordem sai do relógio UTC do servidor, nunca do relógio do jogador.
//!
//! Consequência para quem for investigar custo ou consumo: o gasto de um dia fica
//! DIVIDIDO entre os dois consoles, e nenhum dos dois mostra o total sozinho. O servidor
//! contabiliza os dois no Firestore (`usage/<AAAA-MM-DD>` e `usage/<dia>:<provedor>`),
//! que é a única fonte com o número fechado — `GET /usage` devolve só totais e o dia
//! corrente. Código do servidor fora deste repo (projeto `iracer-news-server`).

use serde::Deserialize;
use std::time::Duration;

/// O host do servidor, em UM lugar só.
///
/// Estava copiado em quatro arquivos (aqui, `commands/ptt_voz`, `telemetry` e
/// `diagnostico`): trocar de região ou de projeto virava caça manual, e esquecer um
/// arquivo quebrava só aquele recurso — em silêncio, porque cada um falha sozinho.
///
/// É macro e não `const` porque `concat!` só come literal. Assim cada endpoint segue
/// sendo `&'static str` resolvido na compilação, sem `format!` em tempo de execução, e
/// quem mora fora deste módulo escreve `crate::narrative::client::host_do_servidor!()`.
macro_rules! host_do_servidor {
    () => {
        "https://iracer-news-124606451488.southamerica-east1.run.app"
    };
}
pub(crate) use host_do_servidor;

const SERVER_URL: &str = concat!(host_do_servidor!(), "/race-story");
/// Endpoint da prévia pré-corrida (narrativa + voz da equipe, curtas).
const PRE_RACE_URL: &str = concat!(host_do_servidor!(), "/pre-race");
/// Endpoint do debrief pós-corrida (voz única do engenheiro → piloto, com calor).
const POST_RACE_URL: &str = concat!(host_do_servidor!(), "/post-race");
/// Endpoint do rodapé "Do mundo do Grid" (reescrita jornalística das notinhas).
const WORLD_NOTES_URL: &str = concat!(host_do_servidor!(), "/world-notes");
/// Endpoint da matéria "O Que Esperar" (prévia de temporada). Ver
/// `docs/season-preview-design.md` e `docs/season-preview-endpoint.md`.
const SEASON_PREVIEW_URL: &str = concat!(host_do_servidor!(), "/season-preview");
/// Comprimento-alvo da prévia, em palavras. Vai no payload (`target_words`) porque a
/// matéria é a peça principal da aba e o bundle hoje carrega uns dez dossiês. Já foi
/// 700–900, e no playtest ninguém terminava de ler; 450–600 ainda dá tratamento próprio
/// a cada favorito (1–2 frases) e uma frase por promessa — abaixo de ~400 vira legenda
/// de foto, que é o que fazia a matéria soar rasa. O servidor manda no texto final;
/// isto é o pedido do cliente.
const SEASON_PREVIEW_TARGET_WORDS: (u32, u32) = (450, 600);
/// `pub(crate)` porque a telemetria de produto (`crate::telemetry`) entra pela
/// MESMA porta do servidor — um segredo só, um lugar só pra trocar.
pub(crate) const APP_SECRET: &str =
    "827119cc235cdea25c04545cd283749e673917d2d424340fb1059925738efcef";
// 45s e não 20s, por DOIS motivos que se somam. (1) O servidor (Cloud Run) faz
// scale-to-zero quando ocioso, e a 1ª chamada após um período parado paga um cold start
// (subir o container + init do Firestore) ANTES de gerar; quente, gera em ~3s, frio pode
// passar de 20s. (2) O servidor dá 30s POR PROVEDOR, mas limita a CADEIA inteira a 42s
// (`ORCAMENTO_CADEIA_MS`) — o fallback só recebe o que sobrou — justamente para caber
// aqui dentro. Cortar este teto faria o cliente desistir justo quando o segundo provedor
// ia salvar; aumentá-lo permite soltar o orçamento da cadeia lá do outro lado.
const TIMEOUT_SECS: u64 = 45;
/// Raiz do serviço — usada só pelo aquecimento (`spawn_warmup`), que não pede geração.
const BASE_URL: &str = concat!(host_do_servidor!(), "/");
/// Intervalo mínimo entre aquecimentos. O gatilho vive no poll da torre (2 Hz), então
/// sem freio seriam centenas de requisições por corrida; o Cloud Run segura o container
/// vivo por alguns minutos depois da última chamada, e cinco minutos cobrem essa janela.
const WARMUP_MIN_INTERVAL_SECS: i64 = 5 * 60;
static ULTIMO_WARMUP: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// Tira o servidor do zero SEM pedir geração nenhuma.
///
/// O Cloud Run faz scale-to-zero: a 1ª chamada depois de um tempo parado paga o cold
/// start (subir o container + init do Firestore) ANTES de escrever a primeira linha —
/// é de onde vêm os 20-40s de espera que o `TIMEOUT_SECS` precisa cobrir. Um GET na raiz
/// obriga o container a subir; o status da resposta é irrelevante (404 serve). Não gasta
/// cota do modelo e não mexe no cooldown, porque não passa por nenhum endpoint de geração.
///
/// Fire-and-forget, com guarda de intervalo própria: pode ser chamado de um laço de poll.
pub fn spawn_warmup() {
    use std::sync::atomic::Ordering;

    let agora = chrono::Local::now().timestamp();
    let anterior = ULTIMO_WARMUP.load(Ordering::Relaxed);
    if agora - anterior < WARMUP_MIN_INTERVAL_SECS {
        return;
    }
    // CAS: entre duas batidas do poll só uma thread ganha o direito de aquecer.
    if ULTIMO_WARMUP
        .compare_exchange(anterior, agora, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    std::thread::spawn(move || {
        let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()
        else {
            return;
        };
        let _ = client.get(BASE_URL).send();
    });
}

/// O texto de `Server` e `Network` é lido em produção pelo `eprintln!("{err:?}")` dos
/// callsites em `commands/{ai_news,world_footer,season_preview}` — é o que separa, no
/// loop.log, "os dois provedores caíram" de "a máquina está sem rede". O `dead_code`
/// não conta leitura via `Debug` derivado, daí o allow pontual.
#[derive(Debug)]
#[allow(dead_code)]
pub enum StoryError {
    /// 429 — cooldown de 10 min ou teto diário. Cai no template, silencioso.
    RateLimited,
    /// 401 — segredo inválido (não deveria acontecer em produção).
    Unauthorized,
    /// Erro do servidor (5xx) ou resposta inesperada. Um 5xx aqui já significa que os
    /// DOIS provedores falharam — o servidor só devolve erro depois de esgotar a cadeia.
    Server(String),
    /// Falha de rede / sem internet.
    Network(String),
    /// Resposta vazia.
    Empty,
}

#[derive(Deserialize)]
struct StoryResponse {
    story: String,
}

/// Fecho de frase de verdade — o que uma matéria pode terminar sem parecer cortada.
const FECHOS_DE_FRASE: [char; 6] = ['.', '!', '?', '…', '"', '»'];

/// O servidor gera com teto de tokens: quando a geração estoura, o texto chega
/// cortado no meio da frase (fica um "O resultado em Lédenon" pendurado no fim).
/// Apara a cauda até o último fim de frase real — o texto perde a última ideia, mas
/// FECHA. NUNCA descarta: texto curto, cortado cedo ou sem nenhum fim de frase volta
/// como veio. O que a IA escreveu sempre vale mais que o template determinístico; a
/// solução do corte é o teto de geração no servidor, não jogar a matéria fora aqui.
pub(crate) fn aparar_frase_incompleta(texto: &str) -> String {
    let t = texto.trim_end();
    if t.chars()
        .last()
        .is_some_and(|c| FECHOS_DE_FRASE.contains(&c))
    {
        return t.to_string();
    }
    match t
        .char_indices()
        .rev()
        .find(|(_, c)| FECHOS_DE_FRASE.contains(c))
    {
        Some((idx, ch)) => t[..idx + ch.len_utf8()].trim_end().to_string(),
        None => t.to_string(),
    }
}

/// Expressões de quarta parede que os fatos usavam para marcar o piloto do jogador e
/// que o modelo, de tempos em tempos, copiava para dentro da matéria ("Para Fulano,
/// piloto acompanhado pelo leitor, ..."). A origem foi corrigida na redação dos fatos
/// (rótulos em linguagem de universo — ver `narrative.context` nos locales), mas os
/// fatos ficam PERSISTIDOS no save: um save antigo continua enviando a redação velha.
/// Cada entrada é (frase, substituto neutro). Ordem: da mais longa pra mais curta,
/// senão a curta come um pedaço da longa e deixa palavra pendurada — invariante preso
/// por teste, e não pela mão de quem acrescentar a próxima.
///
/// **O que faria esta camada sair:** migrar os fatos JÁ PERSISTIDOS (`ai_race_story.facts`
/// e o cache da pré-corrida) para a redação nova. Enquanto um save antigo puder mandar a
/// redação velha ao servidor, tirar o filtro é reabrir o vazamento — a lista só encolhe
/// depois da migração, nunca antes.
const FRASES_META: [(&str, &str); 9] = [
    ("piloto acompanhado pelo leitor", "piloto"),
    ("piloto acompanhada pelo leitor", "piloto"),
    ("acompanhado pelo leitor", ""),
    ("acompanhada pelo leitor", ""),
    ("piloto do leitor", "piloto"),
    ("driver followed by the reader", "driver"),
    ("followed by the reader", ""),
    ("the reader's driver", "the driver"),
    ("reader's driver", "driver"),
];

/// Busca ASCII case-insensitive. As frases são 100% ASCII, então comparar bytes é
/// seguro: byte ASCII nunca aparece no meio de um caractere multibyte de UTF-8, logo
/// todo match cai em fronteira de caractere.
fn achar_ascii_ci(texto: &str, frase: &str, a_partir_de: usize) -> Option<usize> {
    let t = texto.as_bytes();
    let f = frase.as_bytes();
    if f.is_empty() || t.len() < f.len() || a_partir_de > t.len() - f.len() {
        return None;
    }
    (a_partir_de..=t.len() - f.len()).find(|&i| t[i..i + f.len()].eq_ignore_ascii_case(f))
}

/// Última linha de defesa contra vazamento de meta-linguagem: remove do texto que
/// VOLTA do servidor a família de expressões que quebram a quarta parede. Mora aqui,
/// na saída única de rede, pelo mesmo motivo do guard de testes: endpoint novo já
/// nasce coberto. Quando a frase é um aposto (", piloto acompanhado pelo leitor,"),
/// cai o aposto inteiro; solta no meio da frase, entra o substituto neutro.
pub(crate) fn remover_vazamento_meta(texto: &str) -> String {
    let mut t = texto.to_string();
    for (frase, neutro) in FRASES_META {
        let mut de = 0;
        while let Some(i) = achar_ascii_ci(&t, frase, de) {
            let j = i + frase.len();
            let antes = t[..i].trim_end();
            let abre_virgula = antes.ends_with(',');
            let abre_parentese = antes.ends_with('(');
            // Byte onde começa a remoção de um aposto/parentético: a vírgula ou o
            // '(' que o abre (ambos ASCII, 1 byte).
            let corte = antes.len().saturating_sub(1);
            let depois = t[j..].trim_start();
            let proximo = depois.chars().next();
            let fim_sem_espaco = t.len() - depois.len();
            if abre_virgula
                && matches!(proximo, Some(',' | '.' | ';' | ':' | ')' | '!' | '?') | None)
            {
                // Aposto entre vírgulas: some com a vírgula de abertura e a frase,
                // a pontuação seguinte fecha a oração anterior.
                t.replace_range(corte..fim_sem_espaco, "");
                de = corte;
            } else if abre_parentese && proximo == Some(')') {
                // Parentético: "(piloto acompanhado pelo leitor)" cai inteiro.
                let fim = fim_sem_espaco + ')'.len_utf8();
                t.replace_range(corte..fim, "");
                de = corte;
            } else {
                t.replace_range(i..j, neutro);
                de = i + neutro.len();
            }
        }
    }
    // Cicatrizes da remoção: espaço duplo e espaço antes de pontuação.
    while t.contains("  ") {
        t = t.replace("  ", " ");
    }
    t.replace(" ,", ",").replace(" .", ".").trim().to_string()
}

/// A suíte de testes NÃO fala com o servidor de IA. Nunca.
///
/// Medido em 10/08/2026, no painel de custo do servidor: 1.275 "usuários" em julho e
/// 2.731 nos dez primeiros dias de agosto, quase todos com **uma única chamada**. Não
/// eram pessoas. `simulate_race_weekend_in_base_dir` dispara o pré-aquecimento do
/// boletim (`spawn_prewarm_boletim`), e nos testes o `base_dir` é uma pasta temporária
/// nova a cada execução — logo, um `config.json` novo, logo um `install_id` novo. Cada
/// `cargo test` virava um punhado de "instalações" no Firestore, com custo de verdade
/// nos dois provedores.
///
/// O guard fica AQUI, na saída de rede, e não em cada chamador: assim um caminho novo
/// para o servidor já nasce coberto, sem depender de alguém lembrar.
///
/// `cfg!(test)` cobre a suíte do crate. `LOOP_SEM_IA=1` cobre o resto (um binário de
/// benchmark, um harness de simulação, um agente rodando o app em lote).
fn rede_de_ia_bloqueada() -> bool {
    cfg!(test) || std::env::var("LOOP_SEM_IA").is_ok()
}

/// O POST que os cinco endpoints fazem igual: guard de rede, cliente com o timeout do
/// cold start, segredo no header, e o mapa de status para [`StoryError`].
///
/// Existia copiado cinco vezes, e a cópia era o risco: um endpoint novo nascia de um
/// exemplar escolhido a esmo, e se o exemplar fosse o errado ele vinha sem o guard de
/// rede (a suíte passa a gastar chamada de verdade — foi assim que 2.731 usuários falsos
/// apareceram no Firestore) ou sem o mapeamento de 429 (o cooldown do servidor vira
/// "erro" genérico e o app tenta de novo em vez de esperar).
///
/// A limpeza do texto NÃO entra aqui: `remover_vazamento_meta` e
/// `aparar_frase_incompleta` valem por CAMPO, e cada resposta tem os seus.
fn postar_ao_servidor<T: serde::de::DeserializeOwned>(
    url: &str,
    corpo: &serde_json::Value,
) -> Result<T, StoryError> {
    // Ver `rede_de_ia_bloqueada`: a suíte de testes não gasta chamada de verdade.
    if rede_de_ia_bloqueada() {
        return Err(StoryError::Network(
            "rede de IA bloqueada (teste)".to_string(),
        ));
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let resp = client
        .post(url)
        .header("x-app-secret", APP_SECRET)
        .json(corpo)
        .send()
        .map_err(|e| StoryError::Network(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => StoryError::Unauthorized,
            429 => StoryError::RateLimited,
            other => StoryError::Server(format!("HTTP {other}")),
        });
    }

    resp.json::<T>()
        .map_err(|e| StoryError::Server(e.to_string()))
}

/// Envia os fatos curados ao servidor e devolve o boletim redigido no idioma
/// pedido. Em QUALQUER erro, o chamador deve cair no template (notícia genérica)
/// — o boletim de IA nunca pode quebrar a tela.
pub fn fetch_story(
    facts: &str,
    lang: &str,
    install_id: &str,
    reading_seconds: Option<f64>,
) -> Result<String, StoryError> {
    let body = serde_json::json!({
        "facts": facts,
        "lang": lang,
        "install_id": install_id,
        "reading_seconds": reading_seconds,
    });

    let parsed: StoryResponse = postar_ao_servidor(SERVER_URL, &body)?;

    let story = aparar_frase_incompleta(&remover_vazamento_meta(&parsed.story));
    if story.is_empty() {
        return Err(StoryError::Empty);
    }
    Ok(story)
}

#[derive(Deserialize)]
struct PreRaceResponse {
    headline: String,
    body: String,
    team_voice: String,
}

/// Prévia pré-corrida: manchete + corpo (cinematográfico) + voz da equipe. Mesmo
/// contrato do boletim (segredo no header, cooldown próprio no servidor). Em QUALQUER
/// erro o chamador cai no template atual da Sala de Estratégia.
pub struct PreRaceBriefing {
    pub headline: String,
    pub body: String,
    pub team_voice: String,
}

pub fn fetch_pre_race_briefing(
    facts: &str,
    lang: &str,
    install_id: &str,
    force: bool,
) -> Result<PreRaceBriefing, StoryError> {
    let body = serde_json::json!({
        "facts": facts,
        "lang": lang,
        "install_id": install_id,
        // Reroll de debug: pula o cooldown no servidor (sem regravá-lo).
        "force": force,
    });

    let parsed: PreRaceResponse = postar_ao_servidor(PRE_RACE_URL, &body)?;

    let headline = remover_vazamento_meta(&parsed.headline);
    let body = aparar_frase_incompleta(&remover_vazamento_meta(&parsed.body));
    let team_voice = remover_vazamento_meta(&parsed.team_voice);
    if headline.is_empty() || body.is_empty() || team_voice.is_empty() {
        return Err(StoryError::Empty);
    }
    Ok(PreRaceBriefing {
        headline,
        body,
        team_voice,
    })
}

#[derive(Deserialize)]
struct PostRaceResponse {
    headline: String,
    body: String,
}

/// Debrief pós-corrida do engenheiro: manchete + parágrafo (2ª pessoa, com calor).
/// Voz ÚNICA (sem imprensa). Mesmo contrato dos demais (segredo no header, cooldown
/// próprio no servidor). Em QUALQUER erro o chamador cai no texto determinístico.
pub struct PostRaceDebrief {
    pub headline: String,
    pub body: String,
}

pub fn fetch_post_race_debrief(
    facts: &str,
    lang: &str,
    install_id: &str,
    force: bool,
) -> Result<PostRaceDebrief, StoryError> {
    let body = serde_json::json!({
        "facts": facts,
        "lang": lang,
        "install_id": install_id,
        "force": force,
    });

    let parsed: PostRaceResponse = postar_ao_servidor(POST_RACE_URL, &body)?;

    let headline = remover_vazamento_meta(&parsed.headline);
    let body = aparar_frase_incompleta(&remover_vazamento_meta(&parsed.body));
    if headline.is_empty() || body.is_empty() {
        return Err(StoryError::Empty);
    }
    Ok(PostRaceDebrief { headline, body })
}

/// Matéria "O Que Esperar": manchete + linha-fina + corpo (3ª pessoa, voz de revista).
/// O corpo vem em parágrafos separados por linha em branco. Em QUALQUER erro — inclusive
/// o endpoint `/season-preview` ainda não existir no servidor — o chamador cai no
/// montador determinístico (`deterministic_article`), nunca numa tela vazia.
pub struct SeasonPreview {
    pub headline: String,
    pub standfirst: String,
    pub body: String,
}

#[derive(Deserialize)]
struct SeasonPreviewResponse {
    headline: String,
    standfirst: String,
    body: String,
}

pub fn fetch_season_preview(
    facts: &str,
    lang: &str,
    install_id: &str,
) -> Result<SeasonPreview, StoryError> {
    let body = serde_json::json!({
        "facts": facts,
        "lang": lang,
        "install_id": install_id,
        "target_words": { "min": SEASON_PREVIEW_TARGET_WORDS.0, "max": SEASON_PREVIEW_TARGET_WORDS.1 },
    });

    let parsed: SeasonPreviewResponse = postar_ao_servidor(SEASON_PREVIEW_URL, &body)?;

    let headline = remover_vazamento_meta(&parsed.headline);
    let standfirst = remover_vazamento_meta(&parsed.standfirst);
    // O corpo é o que não pode faltar; manchete/linha-fina vazias o front tolera.
    let body = aparar_frase_incompleta(&remover_vazamento_meta(&parsed.body));
    if body.is_empty() {
        return Err(StoryError::Empty);
    }
    Ok(SeasonPreview {
        headline,
        standfirst,
        body,
    })
}

#[derive(Deserialize)]
struct WorldNotesResponse {
    notes: Vec<String>,
}

/// Reescreve as notinhas do rodapé "Do mundo do Grid" em voz de revista (3ª pessoa,
/// jornalística). Recebe os fatos crus (`[kind] assunto — texto`, um por linha) e
/// devolve UMA string reescrita por fato, na MESMA ordem. Mesmo contrato dos demais
/// (segredo no header, cooldown/teto no servidor). Em QUALQUER erro — inclusive o
/// endpoint ainda não existir no servidor — o chamador mantém o texto determinístico.
pub fn fetch_world_notes(
    facts: &str,
    lang: &str,
    install_id: &str,
) -> Result<Vec<String>, StoryError> {
    let body = serde_json::json!({
        "facts": facts,
        "lang": lang,
        "install_id": install_id,
    });

    let parsed: WorldNotesResponse = postar_ao_servidor(WORLD_NOTES_URL, &body)?;

    let notes: Vec<String> = parsed
        .notes
        .into_iter()
        .map(|s| remover_vazamento_meta(&s))
        .collect();
    if notes.is_empty() || notes.iter().any(|s| s.is_empty()) {
        return Err(StoryError::Empty);
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O guard que impede a suíte de gastar dinheiro de verdade. Este teste é a única
    /// coisa entre o `cargo test` e 2.700 "usuários" no painel de custo do servidor —
    /// ver o doc de [`rede_de_ia_bloqueada`].
    ///
    /// Ele é a prova pelo caminho REAL: chama as cinco funções de geração e exige que
    /// nenhuma passe do gate. Se alguém remover o guard de uma delas, a chamada vai à
    /// rede e o teste falha (offline) ou passa devagar gastando cota — os dois modos de
    /// falha são visíveis, que é o que se quer aqui.
    #[test]
    fn nenhuma_geracao_de_ia_sai_do_teste() {
        assert!(rede_de_ia_bloqueada(), "a suíte tem de nascer bloqueada");

        // Função genérica, e não closure: cada `fetch_*` devolve um tipo de sucesso
        // diferente, e uma closure fixa o primeiro que receber.
        fn bloqueado<T>(r: &Result<T, StoryError>) -> bool {
            matches!(r, Err(StoryError::Network(m)) if m.contains("bloqueada"))
        }

        assert!(bloqueado(&fetch_story("fatos", "pt-BR", "id", None)));
        assert!(bloqueado(&fetch_pre_race_briefing("fatos", "pt-BR", "id", false)));
        assert!(bloqueado(&fetch_post_race_debrief("fatos", "pt-BR", "id", false)));
        assert!(bloqueado(&fetch_season_preview("fatos", "pt-BR", "id")));
        assert!(bloqueado(&fetch_world_notes("fatos", "pt-BR", "id")));
    }

    /// Guard estrutural do próprio arquivo: nenhuma função pode montar um POST por
    /// conta própria.
    ///
    /// O teste acima prova que os CINCO endpoints de hoje estão cobertos. Este prova a
    /// regra que faz o de amanhã nascer coberto: se o `.post(...)` só existe dentro de
    /// `postar_ao_servidor`, um endpoint novo não tem como esquecer o guard de rede nem
    /// o mapeamento de 429 — ele não tem outro caminho para a rede.
    #[test]
    fn so_o_helper_monta_requisicao_para_o_servidor() {
        let fonte = include_str!("client.rs");
        // Só o código de produção: o bloco de teste cita `.post(` no texto das mensagens.
        let producao = fonte.split("#[cfg(test)]").next().unwrap_or(fonte);
        let posts = producao.matches(".post(").count();
        assert_eq!(
            posts, 1,
            "achei {posts} chamadas a `.post(` neste arquivo; só `postar_ao_servidor` \
             pode montar requisição. Um caminho novo para o servidor tem de passar por ele."
        );
    }

    /// O host do servidor mora numa macro só, e os outros arquivos que falam com ele
    /// (telemetria de produto, envio de log, push-to-talk) a INVOCAM em vez de recopiar
    /// a URL.
    ///
    /// O modo de falha que isto fecha é o de migrar de região ou de projeto: com a URL
    /// copiada em quatro arquivos, esquecer um quebrava só aquele recurso, em silêncio,
    /// porque cada um falha sozinho e nenhum deles derruba o app.
    ///
    /// A agulha é montada com `concat!` para o próprio teste não virar a quinta cópia.
    #[test]
    fn o_host_do_servidor_mora_em_um_lugar_so() {
        let agulha = concat!("run", ".app");
        let outros = [
            ("telemetry.rs", include_str!("../telemetry.rs")),
            ("diagnostico.rs", include_str!("../diagnostico.rs")),
            ("commands/ptt_voz.rs", include_str!("../commands/ptt_voz.rs")),
        ];
        for (nome, fonte) in outros {
            assert!(
                !fonte.contains(agulha),
                "{nome} tem o host do servidor escrito à mão; use \
                 `crate::narrative::client::host_do_servidor!()`"
            );
        }
        let aqui = include_str!("client.rs").matches(agulha).count();
        assert_eq!(
            aqui, 1,
            "o host aparece {aqui}x neste arquivo; só a macro `host_do_servidor!` \
             pode escrevê-lo"
        );
    }

    /// O caso real que motivou o filtro: o modelo colou o rótulo interno como aposto.
    #[test]
    fn vazamento_em_aposto_cai_com_o_aposto_inteiro() {
        assert_eq!(
            remover_vazamento_meta(
                "Para Carlos Magno, piloto acompanhado pelo leitor, a etapa terminou em P8."
            ),
            "Para Carlos Magno, a etapa terminou em P8."
        );
    }

    #[test]
    fn vazamento_no_meio_da_frase_vira_termo_neutro() {
        assert_eq!(
            remover_vazamento_meta("O piloto acompanhado pelo leitor cruzou em oitavo."),
            "O piloto cruzou em oitavo."
        );
        assert_eq!(
            remover_vazamento_meta("A batida do piloto do leitor custou posições."),
            "A batida do piloto custou posições."
        );
    }

    #[test]
    fn vazamento_entre_parenteses_cai_com_os_parenteses() {
        assert_eq!(
            remover_vazamento_meta("Carlos Magno (piloto acompanhado pelo leitor) venceu."),
            "Carlos Magno venceu."
        );
    }

    #[test]
    fn vazamento_em_ingles_tambem_cai() {
        let limpo =
            remover_vazamento_meta("For Carlos Magno, the driver followed by the reader, it ended early.");
        assert!(!limpo.to_lowercase().contains("reader"), "{limpo}");
    }

    /// A ORDEM da lista é o que faz o filtro funcionar, e mantê-la à mão é frágil: quem
    /// acrescentar uma frase curta no topo faz a longa nunca casar, e o vazamento volta
    /// com meia frase pendurada ("piloto acompanhado pelo leitor" vira "piloto pelo
    /// leitor"). O invariante é este: nenhuma entrada pode estar CONTIDA numa entrada
    /// posterior.
    #[test]
    fn a_lista_de_meta_linguagem_vai_da_mais_longa_para_a_mais_curta() {
        for (i, (curta, _)) in FRASES_META.iter().enumerate() {
            for (longa, _) in FRASES_META.iter().skip(i + 1) {
                assert!(
                    !longa.contains(curta),
                    "'{curta}' aparece antes de '{longa}', que a contém: a curta casaria \
                     primeiro e deixaria o resto da longa no texto. Suba a mais longa."
                );
            }
        }
    }

    /// Texto limpo (com acento e pontuação normais) atravessa o filtro intacto.
    #[test]
    fn texto_sem_meta_passa_intacto() {
        let texto = "Miguel Sanz venceu em Interlagos com autoridade, e o pelotão sentiu.";
        assert_eq!(remover_vazamento_meta(texto), texto);
    }
}
