//! REGISTRO DO RÁDIO: toda fala, nas duas linhas do tempo.
//!
//! O rádio tem seis bocas (spotter, quebra na grade, peça do nosso carro, ritmo, resposta ao
//! push-to-talk, ocasião) e cada uma foi medida na sua própria linha do tempo. Ninguém somou, e
//! somar é a única pergunta que importa: o jogador tem **um par de ouvidos**. Este módulo é o
//! lugar onde as seis caem juntas, em ordem, com o texto do que foi dito.
//!
//! ## Duas fases, e é por isso que o arquivo tem valor
//!
//! Uma fala do Loop passa por dois instantes distintos, e a distância entre eles é um defeito
//! já medido (ver o cabeçalho de [`crate::iracing_sdk::spotter::registrar_emissor`]):
//!
//! - `decidida` — o Rust concluiu que há o que dizer, no tique de 60 Hz. É o instante exato,
//!   com o `session_time` do sim.
//! - `tocada` — o áudio começou a sair, no front. É o que o jogador ouviu.
//!
//! Registrar só a primeira mede um rádio que talvez não tenha soado. Registrar só a segunda
//! esconde o atraso e some com tudo o que foi engolido pelo caminho. Com as duas no mesmo
//! arquivo, `tocada.rel - decidida.rel` é a latência de entrega, e uma `decidida` sem `tocada`
//! é uma fala perdida — as duas perguntas que não tinham resposta.
//!
//! ## Os desfechos são parte da medida
//!
//! Uma fala que **não** saiu é dado, não ausência de dado. O canal do rádio tem prioridade,
//! fila, validade e o portão do momento, e cada um desses mecanismos descarta ou adia falas em
//! silêncio. Sem `desfecho` registrado, "o rádio está calmo" e "o rádio está engolindo metade
//! do que produz" são o mesmo arquivo.
//!
//! ## O relógio é o do `loop.log`
//!
//! Mesmo formato, de propósito: as duas fontes se leem lado a lado sem conversão nenhuma. O
//! `t` (tempo de sessão do iRacing) vem do último tique visto pelo amostrador, e é `null`
//! quando o sim está fechado.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// Quantos arquivos de sessão guardar. Vinte cobre uma noite inteira de testes; o que passa
/// disso é histórico, e histórico de rádio se guarda fora daqui.
const MAX_ARQUIVOS: usize = 20;

/// Quantos ÁUDIOS de fala do modelo guardar.
///
/// Só as falas do modelo viram arquivo, e o motivo é que elas são as únicas sem gravação: as
/// peças do acervo já estão em `src/assets/`, e a chave registrada aponta qual soou. Guardar
/// cópia delas seria encher o disco com o que já existe versionado.
///
/// Duzentas é folgado para o que a família produz — a ocasião sai uma vez por sessão e a
/// resposta só quando o piloto aperta o botão. A ~150 KB cada, o teto fica na casa dos 30 MB.
const MAX_AUDIOS: usize = 200;

/// Pasta dos registros. `None` até o `init()` do boot.
static PASTA: OnceLock<PathBuf> = OnceLock::new();

/// Arquivo desta execução do app. Criado no PRIMEIRO registro, não no boot: um app aberto para
/// mexer na carreira não deixa arquivo vazio para trás.
static ARQUIVO: OnceLock<PathBuf> = OnceLock::new();

/// Serializa as escritas. O amostrador roda numa thread e os comandos noutra.
static TRAVA: Mutex<()> = Mutex::new(());

/// O último instante do sim, para o registro poder carimbar a linha do tempo DA CORRIDA.
static TEMPO: Mutex<Option<Tempo>> = Mutex::new(None);

/// Onde estávamos na sessão quando a fala aconteceu.
///
/// É a metade que só o amostrador sabe. Um registro carimbado apenas com o relógio do Windows
/// responde "quando", nunca "em que volta" — e a segunda é a pergunta de quem trabalha no rádio.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Tempo {
    /// `SessionTime` do sim.
    pub t: f64,
    /// `SessionNum` — separa treino, classificação e corrida dentro do mesmo fim de semana.
    pub sn: i32,
    /// `SessionState` (4 = Racing, que vale também em treino e classificação).
    pub estado: i32,
    pub volta: i32,
    /// Onde na volta, de 0 a 1. É o que diz se a fala caiu na reta ou na entrada da curva 1.
    pub pct: f64,
    pub no_box: bool,
    /// Tempo de sessão que resta. Negativo quando a prova é por voltas.
    pub restante_s: f64,
    /// Posição do jogador. Vale 0 na classificação, e isso é do sim.
    pub posicao: i32,
}

/// Uma fala a registrar.
///
/// A mesma struct serve os dois lados: o Rust monta na mão, e o front manda pelo comando
/// `radio_registrar`. Um formato só, senão as duas metades da linha do tempo sairiam com
/// colunas diferentes.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Registro {
    /// Que boca falou: `spotter`, `quebra`, `ritmo`, `aviso`, `classificacao`, `ocasiao`,
    /// `resposta`.
    pub canal: String,
    /// `decidida`, `redigida` (o modelo escreveu) ou `tocada`.
    pub fase: String,
    /// As peças, na ordem de tocar. Para o spotter é uma só, já com a variação escolhida.
    #[serde(default)]
    pub chaves: Vec<String>,
    /// O que foi dito, escrito. Vazio quando o texto não existe nesta ponta — o do spotter
    /// vive no gerador do pacote, e quem resolve chave em texto é `scripts/radio-timeline.mjs`.
    #[serde(default)]
    pub texto: String,
    /// O que aconteceu com ela: `ok`, `cortada`, `cedeu`, `adiada`, `fila_cheia`,
    /// `validade`, `suprimida`, `sem_audio`.
    #[serde(default)]
    pub desfecho: String,
    /// O que for específico do canal: prioridade, motivo do portão, quanto esperou.
    #[serde(default)]
    pub detalhe: Option<serde_json::Value>,
    /// Nome do arquivo de áudio guardado ao lado do registro, quando existe. Só as falas do
    /// modelo têm um: as peças do acervo são identificadas pelas `chaves`.
    #[serde(default)]
    pub audio: Option<String>,
}

/// Liga o registro. Chamado uma vez no boot, junto do log de diagnóstico.
pub fn init(base_dir: &Path) {
    let dir = base_dir.join("logs").join("radio");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    podar(&dir);
    let _ = PASTA.set(dir);
}

/// Caminho do arquivo desta execução, para a UI e para o script de leitura. `None` enquanto
/// nenhuma fala tiver acontecido.
pub fn caminho() -> Option<PathBuf> {
    ARQUIVO.get().cloned()
}

/// A pasta dos registros, exista arquivo ou não.
pub fn pasta() -> Option<PathBuf> {
    PASTA.get().cloned()
}

/// Guarda o instante do sim. Chamado pelo amostrador a 60 Hz, ao lado do spotter: um lock sem
/// contenção e nove cópias de campo, sem I/O nenhuma.
pub fn marcar(t: &crate::iracing_sdk::IracingTelemetry) {
    let novo = Tempo {
        t: t.session_time,
        sn: t.session_num,
        estado: t.session_state,
        volta: t.lap,
        pct: t.lap_dist_pct,
        no_box: t.player_on_pit_road,
        restante_s: t.session_time_remain,
        posicao: t.position,
    };
    if let Ok(mut guarda) = TEMPO.lock() {
        *guarda = Some(novo);
    }
}

/// Esquece o instante do sim. Chamado na borda de desconexão — sem isto, uma fala fora de
/// sessão sairia carimbada com o tempo da corrida que já acabou.
pub fn desmarcar() {
    if let Ok(mut guarda) = TEMPO.lock() {
        *guarda = None;
    }
}

/// O último instante conhecido do sim.
pub fn tempo() -> Option<Tempo> {
    TEMPO.lock().ok().and_then(|g| *g)
}

/// **Registra uma fala.** Best-effort do começo ao fim: este módulo existe para ajudar a
/// trabalhar no rádio, e nunca para atrapalhar uma fala que está saindo.
pub fn registrar(r: &Registro) {
    let Some(path) = arquivo() else {
        return;
    };
    let Ok(_guarda) = TRAVA.lock() else {
        return;
    };
    escrever_linha(&path, r, tempo());
}

/// A escrita em si, separada do estado global para poder ser provada contra um arquivo de
/// verdade. O contrato desta linha é lido por `scripts/radio-timeline.mjs`: nome de campo que
/// muda aqui é fala que desaparece de lá, em silêncio.
fn escrever_linha(path: &Path, r: &Registro, momento: Option<Tempo>) {
    let linha = serde_json::json!({
        "rel": chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        "canal": r.canal,
        "fase": r.fase,
        "desfecho": if r.desfecho.is_empty() { "ok" } else { &r.desfecho },
        "chaves": r.chaves,
        "texto": r.texto,
        "t": momento.map(|m| (m.t * 1000.0).round() / 1000.0),
        "sn": momento.map(|m| m.sn),
        "estado": momento.map(|m| m.estado),
        "volta": momento.map(|m| m.volta),
        "pct": momento.map(|m| (m.pct * 10000.0).round() / 10000.0),
        "no_box": momento.map(|m| m.no_box),
        "restante_s": momento.map(|m| (m.restante_s * 10.0).round() / 10.0),
        "posicao": momento.map(|m| m.posicao),
        "audio": r.audio,
        "detalhe": r.detalhe,
    });
    let Ok(mut arquivo) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = writeln!(arquivo, "{linha}");
}

/// **Guarda em disco o áudio de uma fala do modelo** e devolve o nome do arquivo, para a linha
/// do registro apontar para ele.
///
/// ## Por que o áudio CRU, e não o que saiu no alto-falante
///
/// O que se grava aqui é o que a Cloud TTS devolveu, antes da cadeia de rádio. O que o jogador
/// ouviu é esse mesmo áudio filtrado e nivelado no front (ver `processarComoPeca` em
/// `engenheiroVoz.js`), e capturar de lá exigiria trazer um `AudioBuffer` de float de volta pela
/// ponte — quatro vezes o tamanho, num caminho que não pode custar nada ao áudio. A cadeia é
/// determinística e é a mesma das 3.900 peças do acervo, então o cru responde o que se quer
/// julgar depois: a redação, a prosódia, o ritmo e a duração.
///
/// Best-effort como todo o resto: um disco que recusa custa o arquivo, nunca a fala.
pub fn guardar_audio(b64: &str, mime: &str) -> Option<String> {
    if b64.is_empty() {
        return None;
    }
    let pasta = PASTA.get()?.join("audio");
    if std::fs::create_dir_all(&pasta).is_err() {
        return None;
    }
    let bytes = crate::commands::engenheiro::voz_propria::decodificar_base64(b64).ok()?;
    // O nome começa pelo carimbo de tempo para casar de olho com a linha do registro (que traz
    // o mesmo relógio) e para a poda por nome continuar sendo poda por idade.
    let nome = format!(
        "fala-{}.{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S%.3f"),
        extensao(mime)
    );
    std::fs::write(pasta.join(&nome), bytes).ok()?;
    podar_audios(&pasta);
    Some(nome)
}

/// A extensão que casa com o que o servidor mandou. O desconhecido vira `.bin` em vez de virar
/// `.mp3` por otimismo: um arquivo com extensão errada é pior que um sem, porque o player abre
/// e falha em vez de deixar claro que o formato mudou.
fn extensao(mime: &str) -> &'static str {
    match mime.split(';').next().unwrap_or("").trim() {
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/wav" | "audio/wave" | "audio/x-wav" => "wav",
        "audio/ogg" | "audio/opus" => "ogg",
        "audio/webm" => "webm",
        _ => "bin",
    }
}

/// O arquivo desta execução, criado na primeira fala.
///
/// O nome carrega o carimbo de tempo do boot, e é o que torna a ordenação por nome igual à
/// ordenação por tempo — o script de leitura conta com isso, e a poda também.
fn arquivo() -> Option<PathBuf> {
    if let Some(p) = ARQUIVO.get() {
        return Some(p.clone());
    }
    let pasta = PASTA.get()?;
    let nome = format!(
        "radio-{}.jsonl",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let caminho = pasta.join(nome);
    // Cabeçalho: sem ele, um arquivo aberto meses depois não diz de que versão do rádio ele
    // fala, e comparar duas medições de versões diferentes é o erro mais fácil de cometer aqui.
    let cabecalho = serde_json::json!({
        "kind": "header",
        "rel": chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        "app": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
    });
    if let Ok(mut f) = std::fs::File::create(&caminho) {
        let _ = writeln!(f, "{cabecalho}");
    } else {
        return None;
    }
    crate::diagnostico::linha("radio", &format!("registro em {}", caminho.display()));
    // `set` perde a corrida se duas threads chegarem juntas; quem perdeu usa o arquivo do
    // vencedor, que é o comportamento certo — um arquivo por execução, sempre.
    let _ = ARQUIVO.set(caminho);
    ARQUIVO.get().cloned()
}

/// Mantém só os [`MAX_ARQUIVOS`] mais recentes.
fn podar(dir: &Path) {
    manter_mais_novos(dir, MAX_ARQUIVOS - 1, |n| {
        n.starts_with("radio-") && n.ends_with(".jsonl")
    });
}

/// Mantém só os [`MAX_AUDIOS`] mais recentes. Roda a cada gravação: são poucas por sessão, e
/// podar no boot deixaria a pasta crescer sem teto dentro de uma sessão longa de testes.
fn podar_audios(dir: &Path) {
    manter_mais_novos(dir, MAX_AUDIOS, |n| n.starts_with("fala-"));
}

/// Deixa no máximo `quantos` arquivos que casem com o filtro, apagando os de nome mais antigo.
///
/// Ordena por NOME, e isso só é poda por idade porque todo nome aqui começa com carimbo de
/// tempo. Vale a pena repetir: um esquema de nome novo sem carimbo na frente transformaria esta
/// função em apagar arquivo alfabeticamente, que é o pior jeito possível de errar.
fn manter_mais_novos(dir: &Path, quantos: usize, casa: impl Fn(&str) -> bool) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    let mut arquivos: Vec<PathBuf> = entradas
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(&casa))
        .collect();
    if arquivos.len() <= quantos {
        return;
    }
    arquivos.sort();
    let sobra = arquivos.len() - quantos;
    for velho in arquivos.into_iter().take(sobra) {
        let _ = std::fs::remove_file(velho);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sem `init()` (o caso dos testes) nada pode explodir nem escrever em lugar nenhum.
    #[test]
    fn sem_init_nao_quebra() {
        registrar(&Registro {
            canal: "spotter".to_string(),
            fase: "decidida".to_string(),
            chaves: vec!["esquerda".to_string()],
            ..Default::default()
        });
        assert!(caminho().is_none(), "sem init não pode existir arquivo");
    }

    /// O tempo do sim é opcional e some quando a conexão cai. Um registro fora de sessão sai
    /// com `t: null` em vez de carimbado com a corrida anterior.
    #[test]
    fn desmarcar_apaga_o_tempo_da_sessao() {
        desmarcar();
        assert!(tempo().is_none());
    }

    /// A extensão sai do mime e o desconhecido NÃO vira mp3 por otimismo.
    #[test]
    fn a_extensao_do_audio_vem_do_mime() {
        assert_eq!(extensao("audio/mpeg"), "mp3");
        // O servidor pode mandar o charset junto; o que vale é o tipo.
        assert_eq!(extensao("audio/wav; codecs=1"), "wav");
        assert_eq!(extensao("audio/ogg"), "ogg");
        assert_eq!(extensao("application/json"), "bin");
        assert_eq!(extensao(""), "bin");
    }

    /// A PODA é a única operação destrutiva do módulo, então ela tem teste: mantém os mais
    /// novos, apaga o excesso pelos nomes mais antigos e não toca em arquivo de fora do filtro.
    #[test]
    fn a_poda_apaga_os_mais_antigos_e_respeita_o_filtro() {
        let dir = std::env::temp_dir().join(format!("loop_radio_poda_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("pasta temporária");
        for n in 1..=6 {
            std::fs::write(dir.join(format!("fala-2026081{n}-000000.000.mp3")), b"x").unwrap();
        }
        // Um intruso: não casa com o filtro e não pode ser apagado.
        std::fs::write(dir.join("radio-20260811-000000.jsonl"), b"y").unwrap();

        manter_mais_novos(&dir, 2, |n| n.starts_with("fala-"));

        let mut restantes: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        restantes.sort();
        assert_eq!(
            restantes,
            vec![
                "fala-20260815-000000.000.mp3".to_string(),
                "fala-20260816-000000.000.mp3".to_string(),
                "radio-20260811-000000.jsonl".to_string(),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Abaixo do teto, ninguém é apagado. O erro que este teste pega é o `<=` virar `<`, que
    /// apagaria um arquivo a cada chamada mesmo com a pasta vazia de excesso.
    #[test]
    fn a_poda_no_limite_nao_apaga_nada() {
        let dir = std::env::temp_dir().join(format!("loop_radio_limite_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("pasta temporária");
        for n in 1..=3 {
            std::fs::write(dir.join(format!("fala-{n}.mp3")), b"x").unwrap();
        }
        manter_mais_novos(&dir, 3, |n| n.starts_with("fala-"));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Sem `init()` não há pasta, e guardar áudio devolve `None` em vez de escrever em `.`.
    #[test]
    fn sem_pasta_o_audio_nao_vai_para_lugar_nenhum() {
        assert!(guardar_audio("TG9vcA==", "audio/mpeg").is_none());
        // E o vazio nunca vira arquivo, mesmo com pasta.
        assert!(guardar_audio("", "audio/mpeg").is_none());
    }

    /// O CONTRATO da linha, campo por campo. Do outro lado dele está
    /// `scripts/radio-timeline.mjs`, que lê por nome: renomear `t` ou `desfecho` aqui faria as
    /// falas simplesmente desaparecerem do relatório, sem erro nenhum em lugar nenhum.
    #[test]
    fn a_linha_escrita_tem_os_dois_relogios_e_o_texto() {
        let dir = std::env::temp_dir().join(format!("loop_radio_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("pasta temporária");
        let path = dir.join("contrato.jsonl");
        let _ = std::fs::remove_file(&path);

        let momento = Tempo {
            t: 95.4321,
            sn: 1,
            estado: 4,
            volta: 7,
            pct: 0.123_456,
            no_box: false,
            restante_s: 512.34,
            posicao: 3,
        };
        escrever_linha(
            &path,
            &Registro {
                canal: "classificacao".to_string(),
                fase: "decidida".to_string(),
                chaves: vec!["cl_perdeu_0".to_string(), "cl_restam_2".to_string()],
                texto: "Perdeu essa. Ainda dá pra mais duas.".to_string(),
                desfecho: String::new(),
                // Uma fala do acervo não tem áudio próprio: ela É a peça gravada, apontada
                // pelas chaves.
                audio: None,
                detalhe: Some(serde_json::json!({ "voando": true })),
            },
            Some(momento),
        );

        let bruto = std::fs::read_to_string(&path).expect("linha escrita");
        let v: serde_json::Value = serde_json::from_str(bruto.trim()).expect("JSON válido");
        assert_eq!(v["canal"], "classificacao");
        assert_eq!(v["fase"], "decidida");
        // Desfecho vazio vira `ok`: o normal não precisa ser declarado em toda chamada.
        assert_eq!(v["desfecho"], "ok");
        assert_eq!(v["texto"], "Perdeu essa. Ainda dá pra mais duas.");
        assert_eq!(v["chaves"][1], "cl_restam_2");
        // O relógio de parede no mesmo formato do `loop.log`, para os dois se lerem lado a lado.
        assert_eq!(v["rel"].as_str().map(str::len), Some(23));
        // As casas são cortadas na escrita: um arquivo de milhares de falas não precisa de
        // dezesseis dígitos de `lap_dist_pct`.
        assert_eq!(v["t"], 95.432);
        assert_eq!(v["pct"], 0.1235);
        assert_eq!(v["sn"], 1);
        assert_eq!(v["volta"], 7);
        assert_eq!(v["posicao"], 3);
        assert_eq!(v["detalhe"]["voando"], true);

        // Sem sessão aberta, os campos do sim vêm nulos em vez de zerados — zero é um tempo
        // válido, e confundir "não sei" com "instante zero" põe falas na largada que nunca
        // aconteceram lá.
        escrever_linha(
            &path,
            &Registro {
                canal: "spotter".to_string(),
                fase: "tocada".to_string(),
                ..Default::default()
            },
            None,
        );
        let ultima = std::fs::read_to_string(&path)
            .expect("segunda linha")
            .lines()
            .last()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).expect("JSON válido"))
            .expect("há duas linhas");
        assert!(ultima["t"].is_null());
        assert!(ultima["volta"].is_null());
        let _ = std::fs::remove_file(&path);
    }
}
