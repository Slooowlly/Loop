//! O CUTUCÃO dos canais do engenheiro — a segunda porta de entrega, ao lado do polling.
//!
//! Os cinco canais do rádio (quebra na grade, ritmo, aviso do nosso carro, ocasião,
//! classificação) chegam ao
//! front por `invoke` periódico. É o mesmo elo frágil que o spotter já tinha, e pelo mesmo
//! motivo: em corrida a janela do webview fica coberta pelo simulador ou minimizada, e o
//! navegador estrangula os timers do JavaScript. O número está medido e escrito em
//! `EngenheiroVozAuto.jsx` — um intervalo de 800 ms virou **20 segundos**.
//!
//! O que isso custa é diferente em cada canal, e é essa diferença que justifica o módulo:
//!
//! - **quebra** — o cursor drena UMA por leitura (`umaPorVez`). Nada se perde, mas cinco
//!   quebras a 20 s por leitura chegam ao longo de um minuto e meio, cada uma comentando um
//!   acidente que já saiu da tela.
//! - **ritmo** e **aviso** — o cursor salta para a mais nova. Duas caindo entre duas leituras e
//!   a primeira some, sem card e sem áudio.
//! - **ocasião** — é uma JANELA de tempo (a volta de formação, a bandeirada), não uma fila. Uma
//!   leitura a cada 20 s atravessa a janela inteira sem ver nada, e a fala simplesmente não
//!   acontece.
//! - **classificação** — é o prazo mais curto de todos. A despedida é agendada para TERMINAR
//!   pouco antes da linha, dentro de uma janela de menos de um segundo; entregue tarde, ela sai
//!   com o piloto já na volta lançada, que é exatamente o que a família existe para não fazer.
//!
//! ## Por que um cutucão, e não o evento pronto
//!
//! O empurrão do spotter leva o anúncio inteiro (ver [`crate::iracing_sdk::spotter`]), porque
//! lá o evento nasce pronto no amostrador. Aqui não nasce: a frase de uma quebra precisa do
//! banco do save, do nome do piloto, da rivalidade e da tabela do campeonato, e nada disso o
//! amostrador tem em mãos. Mover essa montagem para cá seria refazer a arquitetura para
//! resolver um problema de ENTREGA.
//!
//! Então o que sai daqui é só o toque no ombro: "o canal X tem novidade". Quem recebe roda o
//! MESMO `invoke` que o poll rodaria — mesmo comando, mesmo cursor, mesma ordem, mesmos ids. O
//! poll continua vivo como rede, e o que chegar em dobro o cursor descarta, exatamente como no
//! spotter. É uma porta a mais, e não a troca de uma porta frágil por outra.
//!
//! ## Por que os tamanhos, e não um gancho em cada `push`
//!
//! Os logs são escritos em uma dezena de lugares dentro do `RaceMonitor`, todos com o
//! `&mut self` na mão — ou seja, com o mutex do monitor tomado. Emitir dali levaria a travessia
//! da ponte do Tauri para dentro do lock, que é justamente o que o spotter documenta não fazer.
//!
//! Comparar TAMANHOS depois do tique resolve o mesmo problema sem tocar em produtor nenhum: o
//! amostrador já roda a 60 Hz, a leitura é uma tupla de `usize` sob um lock curto, e a
//! emissão acontece fora dele. Um canal que cresceu duas vezes entre dois tiques rende um
//! cutucão só, e está certo: o cutucão não carrega o quê, só o "vá ver".
//!
//! A ÉPOCA entra na comparação junto com os tamanhos. Ela avança quando uma tentativa reinicia
//! e os logs zeram (ver [`super::RaceMonitor::radio_epoch`]); sem ela, um log que caísse de 5
//! para 0 e voltasse a 5 pareceria parado.

use std::sync::{Mutex, OnceLock};

/// Os canais que este módulo cutuca. São os nomes que o front compara — renomear um aqui sem
/// renomear lá emudece o canal em silêncio, que é o modo de falha da família inteira.
pub const CANAL_QUEBRA: &str = "quebra";
pub const CANAL_RITMO: &str = "ritmo";
pub const CANAL_AVISO: &str = "aviso";
pub const CANAL_OCASIAO: &str = "ocasiao";
pub const CANAL_CLASSIFICACAO: &str = "classificacao";

/// Assinatura de quem quer ser avisado de que um canal tem novidade.
type Emissor = Box<dyn Fn(&str) + Send + Sync + 'static>;

static EMISSOR: OnceLock<Emissor> = OnceLock::new();

/// Registra o empurrão para o app. Chamado UMA vez no boot.
///
/// Toma um callback em vez de um `AppHandle` pelo mesmo motivo do spotter: este módulo não
/// conhece Tauri, e não é aqui que essa dependência deve nascer.
pub fn registrar_emissor(f: impl Fn(&str) + Send + Sync + 'static) {
    let _ = EMISSOR.set(Box::new(f));
}

fn empurrar(canal: &str) {
    if let Some(f) = EMISSOR.get() {
        f(canal);
    }
}

/// O retrato dos logs do rádio num tique: a época e o tamanho de cada um.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Tamanhos {
    pub epoch: usize,
    pub quebras: usize,
    pub ritmo: usize,
    pub avisos: usize,
    /// O log da CLASSIFICAÇÃO. Cresce fora da corrida, na sessão de quali, e por isso nunca
    /// concorre com os outros três — mas o estrangulamento de timer é o mesmo, e é o único
    /// canal cuja fala tem prazo de segundos (a despedida termina na linha).
    pub classificacao: usize,
}

static ULTIMO: Mutex<Option<Tamanhos>> = Mutex::new(None);
static ULTIMA_OCASIAO: Mutex<Option<&'static str>> = Mutex::new(None);

/// Compara o retrato de agora com o do tique anterior e cutuca cada canal que cresceu.
///
/// A PRIMEIRA chamada não cutuca nada: sem retrato anterior, todo log com conteúdo pareceria
/// novidade, e o front acabaria de ancorar o cursor mesmo. Ela só grava a linha de base.
pub fn conferir(agora: Tamanhos) {
    let anterior = {
        let mut guarda = ULTIMO.lock().unwrap_or_else(|e| e.into_inner());
        guarda.replace(agora)
    };
    let Some(antes) = anterior else { return };
    if antes == agora {
        return;
    }
    // Época nova = tentativa reiniciada e logs zerados. Não é novidade para falar: é o passado
    // sendo jogado fora. A linha de base já foi regravada acima, e o próximo crescimento de
    // verdade cutuca normalmente.
    if agora.epoch != antes.epoch {
        return;
    }
    if agora.quebras > antes.quebras {
        empurrar(CANAL_QUEBRA);
    }
    if agora.ritmo > antes.ritmo {
        empurrar(CANAL_RITMO);
    }
    if agora.avisos > antes.avisos {
        empurrar(CANAL_AVISO);
    }
    if agora.classificacao > antes.classificacao {
        empurrar(CANAL_CLASSIFICACAO);
    }
}

/// Cutuca a OCASIÃO quando ela ABRE. Diferente dos logs, ela não é uma fila que cresce — é um
/// estado que vale por alguns segundos, e o que interessa é a borda de entrada.
///
/// O trinco de uma vez por corrida continua no Rust, em `commands::engenheiro`: aqui só se
/// avisa que a janela abriu. Cutucar a cada tique enquanto ela dura seria pedir ao front que
/// batesse no comando 7 vezes por segundo durante a volta de formação inteira.
pub fn conferir_ocasiao(agora: Option<&'static str>) {
    let anterior = {
        let mut guarda = ULTIMA_OCASIAO.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::replace(&mut *guarda, agora)
    };
    if agora.is_some() && agora != anterior {
        empurrar(CANAL_OCASIAO);
    }
}

/// Devolve o estado interno ao ponto de partida. Só para o teste — os `static` sobrevivem entre
/// casos dentro do mesmo binário, e um caso herdando a linha de base do anterior mede outra
/// coisa.
#[cfg(test)]
pub(super) fn zerar() {
    *ULTIMO.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *ULTIMA_OCASIAO.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // O emissor é um `OnceLock` global e só pode ser posto uma vez no processo. Os testes
    // registram um contador que grava o último canal, e cada caso zera o resto do estado.
    static QUEBRAS: AtomicUsize = AtomicUsize::new(0);
    static RITMOS: AtomicUsize = AtomicUsize::new(0);
    static AVISOS: AtomicUsize = AtomicUsize::new(0);
    static OCASIOES: AtomicUsize = AtomicUsize::new(0);
    static CLASSIFICACOES: AtomicUsize = AtomicUsize::new(0);

    fn preparar() {
        let _ = EMISSOR.set(Box::new(|canal| {
            let alvo = match canal {
                CANAL_QUEBRA => &QUEBRAS,
                CANAL_RITMO => &RITMOS,
                CANAL_AVISO => &AVISOS,
                CANAL_CLASSIFICACAO => &CLASSIFICACOES,
                _ => &OCASIOES,
            };
            alvo.fetch_add(1, Ordering::Relaxed);
        }));
        zerar();
        for c in [&QUEBRAS, &RITMOS, &AVISOS, &OCASIOES, &CLASSIFICACOES] {
            c.store(0, Ordering::Relaxed);
        }
    }

    fn contas() -> (usize, usize, usize, usize) {
        (
            QUEBRAS.load(Ordering::Relaxed),
            RITMOS.load(Ordering::Relaxed),
            AVISOS.load(Ordering::Relaxed),
            OCASIOES.load(Ordering::Relaxed),
        )
    }

    fn classificacoes() -> usize {
        CLASSIFICACOES.load(Ordering::Relaxed)
    }

    // Serial porque o estado é global do processo: dois casos correndo juntos leriam a linha de
    // base um do outro.
    #[test]
    #[serial_test::serial]
    fn a_primeira_leitura_so_ancora() {
        preparar();
        conferir(Tamanhos {
            epoch: 0,
            quebras: 3,
            ritmo: 2,
            avisos: 1,
            classificacao: 1,
        });
        assert_eq!(contas(), (0, 0, 0, 0), "cutucou sobre o que já existia");
        assert_eq!(classificacoes(), 0);
    }

    /// O canal da CLASSIFICAÇÃO cutuca igual aos outros. Ele é o que mais depende disso: a
    /// despedida tem de chegar dentro da janela de segundos que antecede a linha, e um poll
    /// estrangulado a 20 s atravessa a volta de preparação inteira sem ver nada.
    #[test]
    #[serial_test::serial]
    fn a_classificacao_tem_o_seu_proprio_cutucao() {
        preparar();
        let base = Tamanhos::default();
        conferir(base);
        conferir(Tamanhos {
            classificacao: 1,
            ..base
        });
        assert_eq!(classificacoes(), 1);
        assert_eq!(
            contas(),
            (0, 0, 0, 0),
            "a fala da quali cutucou os canais da corrida"
        );
    }

    #[test]
    #[serial_test::serial]
    fn cada_canal_que_cresce_ganha_o_seu_cutucao() {
        preparar();
        let base = Tamanhos::default();
        conferir(base);
        conferir(Tamanhos { quebras: 1, ..base });
        assert_eq!(contas(), (1, 0, 0, 0));
        conferir(Tamanhos {
            quebras: 1,
            ritmo: 1,
            avisos: 2,
            ..base
        });
        assert_eq!(contas(), (1, 1, 1, 0), "canal parado foi cutucado à toa");
    }

    #[test]
    #[serial_test::serial]
    fn duas_mensagens_no_mesmo_tique_rendem_um_cutucao_so() {
        // E está certo: o cutucão não carrega mensagem nenhuma. Quem drena é o cursor do
        // front, que lê o log inteiro e sabe onde parou.
        preparar();
        let base = Tamanhos::default();
        conferir(base);
        conferir(Tamanhos { quebras: 2, ..base });
        assert_eq!(contas().0, 1);
    }

    #[test]
    #[serial_test::serial]
    fn tentativa_reiniciada_nao_vira_novidade() {
        // A época avança e os logs zeram. Sem este braço, o encolhimento seguido do
        // crescimento normal faria o rádio recontar a corrida anterior.
        preparar();
        conferir(Tamanhos {
            epoch: 0,
            quebras: 4,
            ritmo: 3,
            avisos: 2,
            classificacao: 1,
        });
        conferir(Tamanhos {
            epoch: 9,
            quebras: 0,
            ritmo: 0,
            avisos: 0,
            classificacao: 0,
        });
        assert_eq!(contas(), (0, 0, 0, 0));
        assert_eq!(classificacoes(), 0);
        // E a corrida nova segue cutucando normalmente.
        conferir(Tamanhos {
            epoch: 9,
            quebras: 1,
            ritmo: 0,
            avisos: 0,
            classificacao: 0,
        });
        assert_eq!(contas().0, 1);
    }

    #[test]
    #[serial_test::serial]
    fn a_ocasiao_cutuca_na_borda_e_nao_a_cada_tique() {
        preparar();
        conferir_ocasiao(None);
        for _ in 0..50 {
            conferir_ocasiao(Some("antes_da_largada"));
        }
        assert_eq!(contas().3, 1, "cutucou a volta de formação inteira");
        conferir_ocasiao(None);
        conferir_ocasiao(Some("depois_da_bandeirada"));
        assert_eq!(contas().3, 2, "a segunda ocasião não cutucou");
    }
}
