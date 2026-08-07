//! Fachada pública do SDK: as funções que o resto do app chama. Cada uma só
//! delega para a implementação do SO (`imp`), que é Windows real ou stub.

use std::sync::atomic::{AtomicU64, Ordering};

use super::imp;
use super::{DiagnosticoIracing, IracingError, IracingSession, IracingTelemetry, VarDoSdk};

/// Quantos ticks de telemetria o sampler já observou desde o boot.
///
/// É a métrica que separa "o sim nunca conectou" de "conectou e os dados é que
/// vieram vazios" — sem ela, uma tela toda zerada tem as duas leituras e nenhuma
/// forma de escolher entre elas.
static TICKS_OBSERVADOS: AtomicU64 = AtomicU64::new(0);

/// Marca um tick observado. Chamado pelo sampler a cada leitura bem-sucedida;
/// `Relaxed` porque é um contador de diagnóstico, sem ordenação com mais nada.
pub fn nota_tick_observado() {
    TICKS_OBSERVADOS.fetch_add(1, Ordering::Relaxed);
}

/// Total de ticks observados desde o boot. Zero com o simulador aberto é o
/// sintoma exato de "telemetria toda zerada".
pub fn ticks_observados() -> u64 {
    TICKS_OBSERVADOS.load(Ordering::Relaxed)
}

/// Retrato cruzado da conexão (memória do SDK × janela do sim × elevação), para
/// a tela de diagnóstico e para o log. Ver [`DiagnosticoIracing`].
pub fn diagnosticar() -> DiagnosticoIracing {
    imp::diagnosticar()
}

/// Conecta no iRacing, valida o cabeçalho e devolve a info de sessão.
///
/// Falha com [`IracingError::NotRunning`] se o sim não estiver aberto.
pub fn read_session() -> Result<IracingSession, IracingError> {
    imp::read_session()
}

/// Lê um snapshot de telemetria ao vivo do buffer mais recente.
///
/// Pensado para ser chamado repetidamente (polling) pela UI. Falha com
/// [`IracingError::NotRunning`] se o sim não estiver aberto.
pub fn read_telemetry() -> Result<IracingTelemetry, IracingError> {
    imp::read_telemetry()
}

/// Inventário do que o SDK publica nesta build: nome, tipo, quantidade, unidade e
/// descrição de cada variável, como o próprio cabeçalho as declara.
///
/// Não serve à lógica do jogo — vai para a captura de corrida. É a única forma de
/// distinguir "esse canal não existe nesta versão do iRacing" de "esse canal existe e
/// vem zerado", que da telemetria pura são indistinguíveis.
pub fn read_var_inventory() -> Result<Vec<VarDoSdk>, IracingError> {
    imp::read_var_inventory()
}

/// Dispara um MACRO de chat do iRacing (1-15) via broadcast do Windows. O texto
/// do macro é configurado dentro do iRacing — ex.: um macro com `!yellow`. É o
/// único jeito do SDK "digitar" um comando de chat como `!yellow`.
pub fn send_chat_macro(macro_num: i32) -> Result<(), IracingError> {
    imp::send_chat_macro(macro_num)
}

/// Envia um comando de chat de TEXTO LIVRE ao iRacing (foca a janela, abre o
/// chat e digita o texto + Enter). Ao contrário do macro, o texto é montado em
/// runtime — é o caminho para comandos parametrizados como `!black #1 20`.
/// Requer a janela do sim em foreground; falha com [`IracingError::NotRunning`]
/// se o iRacing não estiver aberto. Fora do Windows, [`IracingError::Unsupported`].
pub fn send_chat_text(text: &str) -> Result<(), IracingError> {
    imp::send_chat_text(text)
}

/// Traz a janela do iRacing para frente (para o jogador "cair" no sim logo após
/// exportar os dados). Prefere o SIMULADOR ("iRacing.com Simulator"); se não
/// houver, foca a UI/launcher ("iRacing"). Devolve `Ok(true)` se achou e focou
/// alguma janela, `Ok(false)` se o iRacing não está aberto. Fora do Windows
/// devolve [`IracingError::Unsupported`].
pub fn focus_iracing_window() -> Result<bool, IracingError> {
    imp::focus_iracing_window()
}

/// Força NOSSA janela (handle bruto `HWND` como `isize`) para o primeiro plano,
/// contornando a trava anti-roubo-de-foco do Windows via `AttachThreadInput` com
/// a thread da janela atualmente em foco. Usado quando o iRacing fecha e o app
/// precisa subir sozinho (sem o usuário ter clicado nele). No-op fora do Windows.
pub fn force_foreground_window(hwnd_raw: isize) {
    imp::force_foreground_window(hwnd_raw);
}

/// `true` se a janela em primeiro plano AGORA pertence ao iRacing (título contém
/// "iracing"). Usado para reconquistar o foco só quando o iRacing o rouba, sem
/// incomodar o usuário caso ele tenha aberto outra coisa. `false` fora do Windows.
pub fn foreground_is_iracing() -> bool {
    imp::foreground_is_iracing()
}
