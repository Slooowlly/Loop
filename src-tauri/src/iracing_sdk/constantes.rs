//! Constantes de baixo nível do mapeamento de memória do iRacing: o nome do
//! arquivo mapeado, o bit de "conectado" e o layout do `irsdk_header`.

/// Nome do arquivo mapeado em memória que o iRacing expõe enquanto roda.
pub(crate) const MEM_MAP_FILE_NAME: &str = "Local\\IRSDKMemMapFileName";

/// Mesmo mapeamento SEM o prefixo `Local\`, tentado como segunda chance.
///
/// `Local\` resolve para o namespace da SESSÃO do Windows: se o Loop e o iRacing
/// caírem em sessões diferentes (app subido por instalador/updater ou por um
/// contexto elevado distinto), o nome canônico não encontra nada mesmo com o sim
/// aberto. O nome nu cai no namespace global e cobre esse caso. Custa uma chamada
/// que só acontece quando a primeira já falhou.
pub(crate) const MEM_MAP_FILE_NAME_NU: &str = "IRSDKMemMapFileName";

/// Bit de `status` no cabeçalho indicando que o sim está conectado/ativo.
pub(crate) const STATUS_CONNECTED: i32 = 1;

/// Layout do cabeçalho `irsdk_header` (offsets em bytes a partir do início do
/// mapeamento). Só os campos que este teste consome estão nomeados.
pub(crate) mod header {
    pub const VER: usize = 0; // i32: versão da API do header
    pub const STATUS: usize = 4; // i32: bitfield (bit 0 = conectado)
    pub const TICK_RATE: usize = 8; // i32: ticks por segundo
    pub const SESSION_INFO_UPDATE: usize = 12; // i32: incrementa quando a sessão muda
    pub const SESSION_INFO_LEN: usize = 16; // i32: tamanho em bytes da string YAML
    pub const SESSION_INFO_OFFSET: usize = 20; // i32: offset da string YAML no mapeamento
    /// Tamanho mínimo do cabeçalho que precisamos ler com segurança.
    pub const MIN_LEN: usize = 24;

    // --- Telemetria ao vivo (variable buffers) ---
    pub const NUM_VARS: usize = 24; // i32: quantidade de variáveis de telemetria
    pub const VAR_HEADER_OFFSET: usize = 28; // i32: offset do array de irsdk_varHeader
    pub const NUM_BUF: usize = 32; // i32: quantos buffers de dados existem (<= 4)
    pub const VAR_BUF: usize = 48; // início do array irsdk_varBuf[4]
    pub const VAR_BUF_STRIDE: usize = 16; // tamanho de cada irsdk_varBuf
    pub const VAR_BUF_OFFSET_FIELD: usize = 4; // dentro do varBuf: i32 bufOffset (após tickCount)

    pub const VAR_HEADER_SIZE: usize = 144; // tamanho de cada irsdk_varHeader
    pub const VAR_TYPE: usize = 0; // i32: irsdk_VarType
    pub const VAR_OFFSET: usize = 4; // i32: offset do valor dentro da linha do buffer
    pub const VAR_COUNT: usize = 8; // i32: nº de entradas (1 = escalar; arrays CarIdx* = nº de carros)
    pub const VAR_NAME: usize = 16; // char[32]: nome da variável
    pub const VAR_NAME_MAX: usize = 32;
}
