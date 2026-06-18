//! Geração da pintura do carro do JOGADOR como custom paint do iRacing.
//!
//! O iRacing lê `Documentos/iRacing/paint/<carro>/car_<custid>.tga` (TGA
//! 2048×2048, 32 bits, sem compressão — formato confirmado nos arquivos reais).
//! Preenchemos a imagem inteira com a cor do time → carro na cor sólida do time.

use std::io::Write;
use std::path::Path;

/// Lado da imagem de pintura do carro (px). Os arquivos do iRacing são 2048².
const PAINT_SIZE: u16 = 2048;

/// Escreve um TGA 32bpp sem compressão preenchido com a cor sólida `hex`
/// (`RRGGBB`, com ou sem `#`). É o formato que o iRacing usa para custom paints.
pub fn write_solid_tga(path: &Path, hex: &str) -> std::io::Result<()> {
    let (r, g, b) = parse_rgb(hex);
    let w = PAINT_SIZE;
    let h = PAINT_SIZE;

    // Cabeçalho TGA (18 bytes): tipo 2 (truecolor sem compressão), 32bpp.
    let header: [u8; 18] = [
        0, // ID length
        0, // color map type
        2, // image type: uncompressed true-color
        0, 0, 0, 0, 0, // color map spec
        0, 0, // x-origin
        0, 0, // y-origin
        (w & 0xFF) as u8,
        (w >> 8) as u8,
        (h & 0xFF) as u8,
        (h >> 8) as u8,
        32, // bits por pixel
        8,  // descritor: 8 bits de alpha
    ];

    // TGA 32bpp guarda os pixels em ordem BGRA. Uma linha repetida = imagem sólida.
    let row: Vec<u8> = [b, g, r, 255u8].repeat(w as usize);

    let file = std::fs::File::create(path)?;
    let mut out = std::io::BufWriter::new(file);
    out.write_all(&header)?;
    for _ in 0..h {
        out.write_all(&row)?;
    }
    out.flush()?;
    Ok(())
}

/// `RRGGBB` (com/sem `#`) → (r, g, b). Fallback branco em entrada inválida.
fn parse_rgb(hex: &str) -> (u8, u8, u8) {
    let s = hex.trim().trim_start_matches('#');
    if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return (255, 255, 255);
    }
    let byte = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(255);
    (byte(0), byte(2), byte(4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rgb_basico() {
        assert_eq!(parse_rgb("#3A86FF"), (0x3A, 0x86, 0xFF));
        assert_eq!(parse_rgb("000000"), (0, 0, 0));
        assert_eq!(parse_rgb("ruim"), (255, 255, 255));
    }

    #[test]
    fn tga_tem_cabecalho_e_tamanho_certos() {
        let dir = std::env::temp_dir().join("loop_tga_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("car_test.tga");
        write_solid_tga(&path, "FF0000").unwrap();
        let bytes = std::fs::read(&path).unwrap();
        // 18 de cabeçalho + 2048*2048*4 de pixels.
        assert_eq!(bytes.len(), 18 + 2048 * 2048 * 4);
        assert_eq!(bytes[2], 2); // tipo 2
        assert_eq!(bytes[16], 32); // 32bpp
        // Primeiro pixel BGRA de vermelho puro: B=0, G=0, R=255, A=255.
        assert_eq!(&bytes[18..22], &[0, 0, 255, 255]);
        let _ = std::fs::remove_file(&path);
    }
}
