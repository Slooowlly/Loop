//! Utilitários de baixo nível do caminho Windows: conversão de string para as
//! APIs `...W`, leitura desalinhada do mapeamento e decodificação dos valores
//! de telemetria conforme o `irsdk_VarType`.

use std::os::windows::ffi::OsStrExt;

/// Máximo de carros que o iRacing expõe nas variáveis de array.
pub(super) const IRSDK_MAX_CARS: usize = 64;

/// Converte uma `&str` para uma string larga (UTF-16) terminada em NUL,
/// como as APIs `...W` do Windows esperam.
pub(super) fn wide_null(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Lê um `i32` little-endian de `base + offset` sem assumir alinhamento.
///
/// # Safety
/// `base` deve apontar para um mapeamento válido com pelo menos
/// `offset + 4` bytes legíveis.
pub(super) unsafe fn read_i32(base: *const u8, offset: usize) -> i32 {
    std::ptr::read_unaligned(base.add(offset) as *const i32)
}

/// Decodifica os bytes do YAML de sessão. O iRacing usa Latin-1
/// (ISO-8859-1) para nomes; mapear byte→char preserva tudo sem panic.
pub(super) fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Decodifica um valor de telemetria como `f64`, conforme o `irsdk_VarType`.
///
/// # Safety
/// `ptr` deve apontar para um valor válido do tamanho correspondente ao tipo.
pub(super) unsafe fn read_value(ptr: *const u8, var_type: i32) -> f64 {
    match var_type {
        0 => *(ptr as *const i8) as f64,                             // char
        1 => (*ptr != 0) as i32 as f64,                              // bool
        2 | 3 => std::ptr::read_unaligned(ptr as *const i32) as f64, // int / bitField
        4 => std::ptr::read_unaligned(ptr as *const f32) as f64,     // float
        5 => std::ptr::read_unaligned(ptr as *const f64),            // double
        _ => 0.0,
    }
}

/// Tamanho em bytes de um `irsdk_VarType` (para indexar arrays).
pub(super) fn type_size(var_type: i32) -> usize {
    match var_type {
        0 | 1 => 1,     // char, bool
        2 | 3 | 4 => 4, // int, bitField, float
        5 => 8,         // double
        _ => 4,
    }
}
