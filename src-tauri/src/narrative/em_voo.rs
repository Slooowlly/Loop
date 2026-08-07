//! Trava de geração EM VOO, por chave.
//!
//! Dois caminhos independentes pedem o MESMO texto de IA ao mesmo tempo — o
//! pré-aquecimento em background (`spawn_prewarm_boletim`, `spawn_prewarm_season_preview`)
//! e a tela que o jogador acabou de abrir. Enquanto os comandos eram síncronos, a
//! serialização na thread principal do Tauri resolvia isso de graça: a segunda chamada
//! só entrava depois de a primeira ter gravado no cache, e saía com `cached` sem tocar
//! no servidor. A trava da janela funcionava como um deduplicador acidental.
//!
//! Com os comandos em `spawn_blocking` (a correção que destravou a janela) as duas
//! rodam de verdade em paralelo: leem o cache vazio no mesmo instante e as duas PAGAM
//! uma geração pelo mesmo texto. Esta trava devolve o que a serialização dava — sem
//! devolver o congelamento.
//!
//! **Contrato:** pegue o passe, RELEIA o cache, e só então chame o servidor. Sem a
//! releitura a trava não serve para nada: a segunda thread esperaria educadamente e
//! geraria de novo assim que chegasse a vez dela.
//!
//! A chave inclui a carreira porque os ids (`race_001`) são sequenciais POR save — sem
//! ela, dois saves diferentes disputariam a mesma trava à toa.

use std::collections::HashSet;
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};

static REGISTRO: OnceLock<Registro> = OnceLock::new();

struct Registro {
    /// As chaves cuja geração está acontecendo agora.
    em_voo: Mutex<HashSet<String>>,
    /// Sinaliza que alguma chave saiu do conjunto.
    liberou: Condvar,
}

fn registro() -> &'static Registro {
    REGISTRO.get_or_init(|| Registro {
        em_voo: Mutex::new(HashSet::new()),
        liberou: Condvar::new(),
    })
}

/// Trava envenenada aqui só significa que alguma thread caiu segurando o conjunto. O
/// conteúdo é um `HashSet` de chaves — não há estado pela metade para corromper, então
/// seguimos com ele em vez de derrubar o app por causa de um boletim.
fn travar(m: &Mutex<HashSet<String>>) -> MutexGuard<'_, HashSet<String>> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// O direito exclusivo de gerar UMA chave. Libera no `Drop`, inclusive se a geração
/// entrar em pânico no meio — nenhum caminho de erro deixa a chave presa.
pub struct Passe {
    chave: String,
}

impl Drop for Passe {
    fn drop(&mut self) {
        let r = registro();
        travar(&r.em_voo).remove(&self.chave);
        // `notify_all` e não `notify_one`: quem espera pode estar em OUTRA chave, e
        // acordar só um corre o risco de acordar justamente quem continua bloqueado.
        r.liberou.notify_all();
    }
}

/// Espera até ninguém estar gerando `chave` e devolve o passe dela.
///
/// Quem recebe o passe DEVE reler o cache antes de chamar o servidor — se houve espera,
/// a geração que acabou de sair já gravou o texto.
#[must_use = "o passe solta a chave ao ser descartado; ignorá-lo libera a trava na hora"]
pub fn aguardar_vez(chave: impl Into<String>) -> Passe {
    let chave = chave.into();
    let r = registro();
    let mut em_voo = travar(&r.em_voo);
    while em_voo.contains(&chave) {
        em_voo = r.liberou.wait(em_voo).unwrap_or_else(|e| e.into_inner());
    }
    em_voo.insert(chave.clone());
    Passe { chave }
}

// ── Chaves ────────────────────────────────────────────────────────────────────────
//
// Centralizadas de propósito: o pré-aquecimento e o caminho lazy TÊM que montar a mesma
// string, senão cada um pega uma trava diferente e o par volta a gerar em dobro. Duas
// chamadas de `format!` espalhadas pelos módulos divergem no primeiro refactor.

/// Boletim da corrida (`/race-story`). Colide entre `spawn_prewarm_boletim` (fim da
/// corrida) e `enrich_race_news_ai` (jogador abriu a revista) — o par mais frequente.
pub fn chave_boletim(career_id: &str, news_id: &str) -> String {
    format!("race-story:{career_id}:{news_id}")
}

/// Prévia pré-corrida (`/pre-race`). Colide entre o prefetch da animação de avanço e a
/// Sala de Estratégia, que dispara ao montar quando o prefetch ainda está em voo.
pub fn chave_pre_corrida(career_id: &str, race_id: &str) -> String {
    format!("pre-race:{career_id}:{race_id}")
}

/// Debrief pós-corrida (`/post-race`).
pub fn chave_pos_corrida(career_id: &str, race_id: &str) -> String {
    format!("post-race:{career_id}:{race_id}")
}

/// Rodapé "Do mundo do Grid" (`/world-notes`). `cache_key` é `temporada:rodada`.
pub fn chave_rodape(career_id: &str, cache_key: &str) -> String {
    format!("world-notes:{career_id}:{cache_key}")
}

/// Matéria "O Que Esperar" (`/season-preview`). `cache_key` já vem qualificado por
/// temporada e categoria.
pub fn chave_previa_temporada(career_id: &str, cache_key: &str) -> String {
    format!("season-preview:{career_id}:{cache_key}")
}

/// A carreira dona de um `.../saves/<career_id>/career.db`. O pré-aquecimento recebe o
/// caminho do banco, não o id, e precisa montar a MESMA chave do comando lazy. Caminho
/// fora do formato esperado vira string vazia — a chave continua válida (só perde o
/// desempate entre saves).
pub fn carreira_do_banco(db_path: &std::path::Path) -> String {
    db_path
        .parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}
