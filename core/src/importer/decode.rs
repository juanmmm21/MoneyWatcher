/// Convierte los bytes de un extracto en texto.
///
/// Muchos bancos españoles siguen exportando en Windows-1252 en lugar de UTF-8,
/// y un fichero así rompería el parseo por una simple "ñ". Si los bytes no son
/// UTF-8 válido se interpretan como Windows-1252, que nunca falla porque todo
/// byte tiene un carácter asignado.
pub fn decode(bytes: &[u8]) -> String {
    let bytes = strip_bom(bytes);

    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|byte| windows_1252_char(*byte)).collect(),
    }
}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    match bytes {
        [0xef, 0xbb, 0xbf, rest @ ..] => rest,
        other => other,
    }
}

/// Windows-1252 coincide con Latin-1 salvo en el rango 0x80-0x9F, donde coloca
/// comillas tipográficas y el símbolo del euro, habituales en los extractos.
fn windows_1252_char(byte: u8) -> char {
    match byte {
        0x80 => '\u{20ac}',
        0x82 => '\u{201a}',
        0x83 => '\u{0192}',
        0x84 => '\u{201e}',
        0x85 => '\u{2026}',
        0x86 => '\u{2020}',
        0x87 => '\u{2021}',
        0x88 => '\u{02c6}',
        0x89 => '\u{2030}',
        0x8a => '\u{0160}',
        0x8b => '\u{2039}',
        0x8c => '\u{0152}',
        0x8e => '\u{017d}',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201c}',
        0x94 => '\u{201d}',
        0x95 => '\u{2022}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x98 => '\u{02dc}',
        0x99 => '\u{2122}',
        0x9a => '\u{0161}',
        0x9b => '\u{203a}',
        0x9c => '\u{0153}',
        0x9e => '\u{017e}',
        0x9f => '\u{0178}',
        other => other as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8_and_strips_bom() {
        let bytes = [0xef, 0xbb, 0xbf, b'N', 0xc3, 0xb3, b'm', b'i', b'n', b'a'];
        assert_eq!(decode(&bytes), "Nómina");
    }

    #[test]
    fn falls_back_to_windows_1252() {
        // "Nómina 12,00 €" tal y como lo exporta un banco en Windows-1252.
        let bytes = [b'N', 0xf3, b'm', b'i', b'n', b'a', b' ', 0x80];
        assert_eq!(decode(&bytes), "Nómina €");
    }
}
