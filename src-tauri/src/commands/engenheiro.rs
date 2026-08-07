//! Comandos do engenheiro falado: da transcrição ao dossiê de fatos.
//!
//! Casca fina sobre [`crate::engenheiro`] e
//! [`race_monitor::estado_agora`](crate::iracing_sdk::race_monitor::estado_agora), no
//! mesmo padrão do resto do projeto — a lógica é pura e testável do lado de lá, aqui só
//! se lê o monitor e se serializa.
//!
//! **Nenhuma chave de provedor passa por aqui.** O áudio e o dossiê vão para o nosso
//! servidor, que tem as chaves do Scribe, do Gemini e da Cloud TTS e devolve o áudio
//! pronto. Este comando é o pedaço que o app faz sozinho: entender do que se trata e
//! separar os fatos.

mod tabela;
mod vizinhos;
pub mod voz_propria;

use std::sync::Mutex;

use crate::engenheiro::memoria::Memoria;
use crate::engenheiro::responder::Extras;
use crate::engenheiro::tratamento;
use crate::engenheiro::{self, Intencao};
use crate::iracing_sdk::race_monitor::{self, EstadoAgora};

/// A MEMÓRIA da conversa, viva enquanto o app estiver aberto.
///
/// Mora aqui, e não no `engenheiro`, pela mesma razão que o banco: o módulo de domínio é
/// puro e testável sem nada aberto, e estado global dentro dele tornaria cada teste
/// dependente da ordem dos outros. A lógica de quando lembrar e quando calar está lá; o que
/// está aqui é só o lugar de guardar.
///
/// Uma memória só para o app inteiro, e não uma por orquestrador: o painel de diagnóstico e
/// o botão do volante falam com o MESMO engenheiro, e duas memórias fariam ele lembrar
/// coisas diferentes conforme por onde a pergunta entrou.
static MEMORIA: Mutex<Option<Memoria>> = Mutex::new(None);

/// O TRINCO das ocasiões: `(subsessão, o que já foi falado nela)`.
///
/// Uma ocasião dura minutos e o app pergunta a cada dois segundos — sem trinco, a volta de
/// formação renderia trinta falas do modelo, uma por poll, cada uma custando uma ida ao
/// servidor. O trinco é por SUBSESSÃO porque a corrida seguinte merece a sua própria.
static OCASIOES_FALADAS: Mutex<(i64, Vec<String>)> = Mutex::new((0, Vec::new()));

/// O contador do VOCATIVO: `(subsessão, respostas dadas nela)`.
///
/// Por subsessão, e não pela vida do processo, porque assim **toda corrida começa te
/// chamando**. A primeira pergunta de uma prova é o momento em que o vocativo mais vale — é
/// ali que o rádio abre —, e um contador global faria isso depender de quantas perguntas
/// sobraram da prova anterior.
static VOCATIVO: Mutex<(i64, u32)> = Mutex::new((0, 0));

/// **É a vez de ele te chamar?** `Some(rodízio)` quando sim, e o número escolhe a redação.
///
/// Conta e decide na mesma tomada do lock: separar as duas coisas deixaria duas perguntas
/// quase simultâneas lerem a mesma contagem e saírem as duas com vocativo.
///
/// O rodízio é a contagem de USOS (`n / A_CADA`), e não a de respostas. Com a contagem crua
/// e três redações, `n % 3` andaria de quatro em quatro — 0, 1, 2, 0 — e uma das três nunca
/// sairia em corrida curta.
fn vez_do_vocativo() -> Option<usize> {
    let subsessao = race_monitor::get_subsession_id();
    let mut guarda = VOCATIVO.lock().ok()?;
    if guarda.0 != subsessao {
        *guarda = (subsessao, 0);
    }
    let n = guarda.1;
    guarda.1 = n.saturating_add(1);
    (n % tratamento::A_CADA == 0).then_some((n / tratamento::A_CADA) as usize)
}

/// Abre o banco da carreira, ou `None` em qualquer tropeço.
///
/// Devolve o `base_dir` junto porque quem lê o save quase sempre precisa também do arquivo de
/// números do grid, que mora ao lado e não dentro do banco.
fn abrir_save(
    app: &tauri::AppHandle,
    career_id: &str,
) -> Option<(std::path::PathBuf, crate::db::connection::Database)> {
    use tauri::Manager;

    if career_id.is_empty() {
        return None;
    }
    let base_dir = app.path().app_data_dir().ok()?;
    let config = crate::config::app_config::AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = crate::db::connection::Database::open_existing(&db_path).ok()?;
    Some((base_dir, db))
}

/// O que o SAVE acrescenta à telemetria: a tabela do campeonato e quem são os vizinhos.
///
/// **Nunca falha.** Qualquer tropeço — carreira sem id, save ausente, banco velho — devolve
/// o vazio, e o rádio simplesmente não menciona nem a tabela nem os nomes.
///
/// Essa é a regra que importa aqui. A resposta do engenheiro é sobre a corrida; o que vem
/// do save é o tempero. Deixar um erro de leitura derrubar a resposta trocaria uma frase a
/// menos por nenhuma frase.
fn extras_do_save(app: &tauri::AppHandle, career_id: &str, estado: &EstadoAgora) -> Extras {
    let Some((base_dir, db)) = abrir_save(app, career_id) else {
        return Extras::default();
    };
    // A ordem só é lida EM CORRIDA. Num treino ou numa classificação o `CarIdxPosition`
    // existe e não significa resultado nenhum — projetar a temporada em cima dele daria
    // "terminando assim você cai para nono" a partir de uma ordem de treino livre.
    let ordem = if estado.em_corrida {
        race_monitor::ordem_agora()
    } else {
        Vec::new()
    };
    // Uma leitura do arquivo de números por pergunta, e não duas: a projeção e o vínculo
    // dos vizinhos precisam do mesmo mapa.
    let numeros = tabela::por_numero(&base_dir, career_id);
    // A FASE do tratamento sai daqui já resolvida, e quem apaga é o chamador — ver
    // `vez_do_vocativo`. Decidir aqui "se é a vez" misturaria a leitura do save com o
    // contador de cadência, e este é o único lugar da casa que sabe abrir o banco.
    let tratamento = crate::db::queries::drivers::get_player_driver(&db.conn)
        .ok()
        .and_then(|p| {
            // A GRAVAÇÃO PRÓPRIA é uma pergunta ao sistema de arquivos, e é por isso que ela
            // é feita aqui e não lá dentro. Sem esta checagem o renderizador emitiria
            // `voc_magno` antes de o arquivo existir, e a fala sairia com a primeira
            // peça faltando — que é justamente o silêncio que ninguém liga à causa.
            let gravado = tratamento::chave_do_nome(&p.nome).is_some_and(|chave| {
                voz_propria::arquivo(&base_dir, career_id, &chave).is_file()
            });
            tratamento::decidir(
                p.stats_carreira.corridas,
                p.stats_carreira.vitorias,
                &p.nome,
                gravado,
            )
        });
    Extras {
        campeonato: tabela::carregar(&db.conn, &numeros, &ordem, estado.posicao),
        vizinhanca: vizinhos::carregar(&db.conn, estado, &numeros),
        memoria: None,
        tratamento,
        rodizio: 0,
    }
}

/// Consulta a memória e já registra o que esta resposta vai dizer.
///
/// As duas coisas acontecem juntas de propósito. Separá-las abriria a janela em que uma
/// segunda pergunta chega entre a leitura e a escrita — e a resposta dela compararia com um
/// instante que a primeira ainda não tinha registrado.
///
/// O lock envenenado não derruba a resposta: perder a memória custa uma frase, e o
/// engenheiro emudecer custa a corrida.
fn memoria_da_conversa(estado: &EstadoAgora, intencao: Intencao) -> Option<f64> {
    let subsessao = race_monitor::get_subsession_id();
    let mut guarda = MEMORIA.lock().ok()?;
    let m = guarda.get_or_insert_with(Memoria::default);
    let delta = m.consultar(estado, subsessao, intencao);
    m.registrar(estado, subsessao, intencao);
    delta
}

/// Aplica a CADÊNCIA ao tratamento que o save resolveu.
///
/// Mantém o vocativo nas respostas em que é a vez e o apaga nas outras. O contador anda em
/// TODA resposta, inclusive nas que não tinham tratamento nenhum a apagar: o que se conta são
/// as vezes que ele abriu o rádio, e não as vezes que soube o seu nome.
fn cadenciar(extras: &mut Extras) {
    match vez_do_vocativo() {
        Some(rodizio) => extras.rodizio = rodizio,
        None => extras.tratamento = None,
    }
}

/// O texto que o modelo deve usar para se dirigir ao piloto, se for a vez.
fn como_chamar(extras: &Extras) -> Option<String> {
    extras.tratamento.as_ref().map(tratamento::como_chamar)
}

/// O que o app manda ao servidor junto com o pedido de resposta falada.
#[derive(serde::Serialize)]
pub struct DossieEngenheiro {
    /// Do que a pergunta trata, decidido por palavra-chave aqui no app.
    pub intencao: Intencao,
    /// Os fatos, um por linha, já em português e já calculados. É isto que vira o
    /// contexto do modelo — ele redige em cima, não calcula nada.
    pub linhas: Vec<String>,
    /// O estado bruto que gerou as linhas. Vai junto para depuração e para o painel de
    /// desenvolvimento; o caminho de produção usa as `linhas`.
    pub estado: EstadoAgora,
    /// Como ele deve te chamar nesta fala, ou `None` — que é a maioria das vezes. Ver
    /// [`crate::engenheiro::tratamento`].
    pub tratamento: Option<String>,
}

/// Classifica a pergunta transcrita e monta o dossiê com o estado da corrida agora.
///
/// A transcrição vem do Scribe (no servidor). Este comando é chamado com ela em mãos —
/// ver a nota de arquitetura em [`crate::engenheiro`] sobre o custo de ida e volta que
/// isso implica.
#[tauri::command]
pub fn engenheiro_dossie(
    app: tauri::AppHandle,
    career_id: String,
    transcricao: String,
) -> DossieEngenheiro {
    let estado = race_monitor::estado_agora();
    let intencao = engenheiro::classificar(&transcricao);
    let mut extras = extras_do_save(&app, &career_id, &estado);
    extras.memoria = memoria_da_conversa(&estado, intencao);
    cadenciar(&mut extras);
    let linhas = engenheiro::responder::dossie_com(&estado, &extras, intencao);
    DossieEngenheiro {
        intencao,
        linhas,
        estado,
        tratamento: como_chamar(&extras),
    }
}

/// O que o app decidiu fazer com a pergunta.
///
/// Serializado com etiqueta (`caminho`), então o front distingue os dois casos sem olhar
/// que campos vieram — e um caminho novo no futuro não quebra o `switch` de quem consome.
#[derive(serde::Serialize)]
#[serde(tag = "caminho", rename_all = "snake_case")]
pub enum RespostaEngenheiro {
    /// Renderizou do acervo: é só tocar as peças, em ordem. Sem rede, sem custo.
    Gravada {
        intencao: Intencao,
        /// Chaves de `src/assets/engenheiro/<chave>.opus`, na ordem de reprodução.
        pecas: Vec<String>,
    },
    /// O acervo não cobre: sobe as linhas para o servidor redigir e sintetizar.
    Modelo {
        intencao: Intencao,
        linhas: Vec<String>,
        /// Como ele deve te chamar nesta fala. Só existe neste ramo: no gravado o vocativo
        /// já é a primeira das `pecas`.
        tratamento: Option<String>,
    },
}

/// **A decisão do push-to-talk.** Recebe a transcrição e devolve ou as peças a tocar, ou os
/// fatos a mandar ao modelo.
///
/// É aqui que o roteamento acontece, e a ordem importa: tenta o acervo PRIMEIRO, e só cai no
/// modelo quando a fala não renderiza. O caminho gravado responde em ~0,7 s e de graça; o do
/// modelo custa uma segunda ida ao servidor e alguns segundos.
///
/// A frase de espera sai no ramo `Modelo`, e só nele — ver o orquestrador em
/// [`pttEngenheiro.js`](../../../src/lib/pttEngenheiro.js). O desenho anterior armava a
/// espera por TEMPO (dispare se nada chegou em ~700 ms) e não sobrevive ao encanamento
/// real: aos 700 ms a transcrição ainda está no ar, então nem se sabe qual é o caminho, e
/// a espera tocaria em toda pergunta — inclusive nas que este comando responde de graça,
/// que passariam a esperar a espera terminar.
#[tauri::command]
pub fn engenheiro_responder(
    app: tauri::AppHandle,
    career_id: String,
    transcricao: String,
) -> RespostaEngenheiro {
    let estado = race_monitor::estado_agora();
    let intencao = engenheiro::classificar(&transcricao);
    // A tabela é lida em TODA pergunta, e não só nas que falam dela. Sai barato — uma
    // consulta agregada por pergunta, contra uma por segundo que o feed de quebra já paga —
    // e o alternativo seria decidir aqui, pela intenção, se vale ler; com a intenção errada
    // a resposta perderia o campeonato sem nada indicar por quê.
    let mut extras = extras_do_save(&app, &career_id, &estado);
    extras.memoria = memoria_da_conversa(&estado, intencao);
    cadenciar(&mut extras);
    match engenheiro::responder::renderizar_com(&estado, &extras, intencao) {
        Some(pecas) => RespostaEngenheiro::Gravada { intencao, pecas },
        None => RespostaEngenheiro::Modelo {
            intencao,
            tratamento: como_chamar(&extras),
            // A pergunta ABERTA leva o dossiê inteiro; as demais, só o bloco do assunto.
            //
            // A distinção nasceu de um defeito visto no material de medição: um estado com
            // combustível para 6 voltas e 8 por correr produzia, para "como estamos?", um
            // dossiê que não mencionava combustível — porque `Geral` só monta o núcleo. Um
            // engenheiro de verdade não responde "estamos em quinto" com o tanque nessa
            // situação. Quem não delimitou o assunto está pedindo o retrato todo.
            linhas: engenheiro::responder::dossie_com(&estado, &extras, intencao),
        },
    }
}

/// TODA peça do acervo com o texto a gravar — a entrada do gerador de biblioteca.
///
/// Existe como comando para que a lista tenha **uma fonte só**: o mesmo código que decide o
/// que tocar é o que diz o que gravar. Uma lista de falas mantida à parte divergiria na
/// primeira frase nova, e a divergência apareceria como silêncio em corrida.
#[tauri::command]
pub fn engenheiro_catalogo() -> Vec<(String, String)> {
    engenheiro::catalogo()
}

/// Só a classificação, sem tocar o monitor — para afinar a tabela de termos sem precisar
/// do iRacing aberto. É o laboratório da palavra-chave.
#[tauri::command]
pub fn engenheiro_classificar(transcricao: String) -> Intencao {
    engenheiro::classificar(&transcricao)
}

/// O dossiê com TODOS os blocos, sem depender de transcrição.
///
/// É o caminho de UMA ida e volta: o app sobe o áudio junto com isto, e o servidor
/// transcreve, redige e sintetiza sem precisar devolver nada no meio. Custa mais contexto
/// e menos latência que o par [`engenheiro_dossie`] + segunda chamada.
#[tauri::command]
pub fn engenheiro_dossie_completo() -> DossieEngenheiro {
    let estado = race_monitor::estado_agora();
    let linhas = engenheiro::dossie_completo(&estado);
    DossieEngenheiro {
        intencao: Intencao::Geral,
        linhas,
        estado,
        // Sem `career_id` não há save, e sem save não há como saber se você ainda é novato.
        // Este comando é o caminho de uma ida só, usado no laboratório.
        tratamento: None,
    }
}

/// **O rádio pode falar agora?** `null` = pode; uma string = o motivo do silêncio.
///
/// Consultado pela FILA DE ANÚNCIOS do app antes de soltar cada fala não solicitada — e só
/// por ela. A resposta ao push-to-talk e o spotter não passam por aqui, de propósito: ver o
/// cabeçalho de [`crate::engenheiro::momento`].
///
/// Devolve o motivo, e não um booleano, porque "o engenheiro não falou" e "o engenheiro
/// está calado de propósito" são a mesma tela em branco vistos de fora.
#[tauri::command]
pub fn engenheiro_momento_quente() -> Option<String> {
    let estado = race_monitor::estado_agora();
    engenheiro::momento::quente(&estado).map(|m| format!("{m:?}"))
}

/// O dossiê de uma OCASIÃO — a fala longa dos momentos sem pressa.
#[derive(serde::Serialize)]
pub struct OcasiaoEngenheiro {
    /// `antes_da_largada`, `depois_da_bandeirada` — ou um MARCO, quando aquela bandeirada era
    /// outra coisa: `primeira_vitoria`, `titulo`. Vai ao servidor e escolhe o tom.
    pub ocasiao: String,
    pub linhas: Vec<String>,
    /// Como ele deve te chamar. Aqui vem **sempre**, sem passar pela cadência: as ocasiões
    /// são os dois instantes em que ele fala sem pressa, e é exatamente neles que um
    /// vocativo tem para onde ir. Racioná-lo aqui economizaria a palavra no único lugar
    /// onde ela não atrapalha.
    pub tratamento: Option<String>,
}

/// **Há algo a dizer agora, sem ninguém ter perguntado?**
///
/// Chamado no mesmo poll de dois segundos que arma o microfone. Devolve `None` quase
/// sempre; quando devolve algo, o app manda ao servidor e ANUNCIA a resposta — fila, e não
/// corte, porque é fala não solicitada.
///
/// O trinco fecha ANTES de a fala existir, e é de propósito: um erro depois daqui custa uma
/// ocasião perdida, enquanto um trinco que só fechasse no sucesso custaria uma fala por
/// poll enquanto o servidor estivesse fora do ar.
#[tauri::command]
pub fn engenheiro_ocasiao(
    app: tauri::AppHandle,
    career_id: String,
) -> Option<OcasiaoEngenheiro> {
    use crate::engenheiro::momento::Ocasiao;

    let estado = race_monitor::estado_agora();
    let ocasiao = engenheiro::momento::ocasiao(&estado)?;
    let nome = match ocasiao {
        Ocasiao::AntesDaLargada => "antes_da_largada",
        Ocasiao::DepoisDaBandeirada => "depois_da_bandeirada",
    };

    {
        let subsessao = race_monitor::get_subsession_id();
        let mut trinco = OCASIOES_FALADAS.lock().ok()?;
        if trinco.0 != subsessao {
            *trinco = (subsessao, Vec::new());
        }
        if trinco.1.iter().any(|j| j == nome) {
            return None;
        }
        trinco.1.push(nome.to_string());
    }

    // O retrato COMPLETO, e não o bloco de um assunto: ninguém perguntou nada, então não há
    // assunto a delimitar. É o mesmo raciocínio da pergunta aberta.
    let mut extras = extras_do_save(&app, &career_id, &estado);
    extras.memoria = None;
    let mut linhas = engenheiro::responder::dossie_com(&estado, &extras, Intencao::Geral);

    // O MARCO refina a bandeirada, e só ela: a volta de formação não tem resultado sobre o
    // que fazer conta. Note que o TRINCO acima fechou sobre a ocasião BASE, e não sobre o
    // nome que sai daqui — de propósito. A projeção que decide o título pode falhar num poll
    // e funcionar no seguinte, e um trinco por nome final deixaria o piloto ouvir a fala
    // normal de bandeirada e, dois segundos depois, o anúncio do título.
    let mut nome = nome.to_string();
    if ocasiao == Ocasiao::DepoisDaBandeirada {
        if let Some((_, db)) = abrir_save(&app, &career_id) {
            let ctx = tabela::marcos(&db.conn, extras.campeonato.projecao);
            if let Some(m) = engenheiro::marco::na_bandeirada(estado.posicao, &ctx) {
                nome = m.nome().to_string();
                linhas.extend(engenheiro::marco::linhas(m, &ctx));
            }
        }
    }

    Some(OcasiaoEngenheiro {
        ocasiao: nome,
        linhas,
        tratamento: como_chamar(&extras),
    })
}
