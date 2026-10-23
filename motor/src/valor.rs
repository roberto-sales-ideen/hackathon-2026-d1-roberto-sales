//! Valores lidos da linha e as comparações entre eles.

use std::cmp::Ordering;

/// Número do JSON. Inteiro e decimal ficam separados para comparar como o Python compara:
/// inteiro com inteiro é exato, e inteiro com decimal também.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Num {
    I(i64),
    F(f64),
}

/// Texto do JSON: um trecho da própria linha (sem escape) ou da arena (escape decodificado).
#[derive(Clone, Copy, Debug)]
pub enum Txt {
    Linha(u32, u32),
    Arena(u32, u32),
}

/// O que a linha tem em cada campo usado pelas regras.
#[derive(Clone, Copy, Debug)]
pub enum Valor {
    Ausente,
    Nulo,
    Bool(bool),
    Num(Num),
    Texto(Txt),
    /// objeto ou lista: nunca satisfaz comparação
    Outro,
}

pub fn cmp_num(a: Num, b: Num) -> Option<Ordering> {
    match (a, b) {
        (Num::I(x), Num::I(y)) => Some(x.cmp(&y)),
        (Num::F(x), Num::F(y)) => x.partial_cmp(&y),
        (Num::I(x), Num::F(y)) => cmp_int_float(x, y),
        (Num::F(x), Num::I(y)) => cmp_int_float(y, x).map(Ordering::reverse),
    }
}

/// Comparação exata entre inteiro e decimal, sem perder precisão acima de 2^53.
fn cmp_int_float(i: i64, f: f64) -> Option<Ordering> {
    if f.is_nan() {
        return None;
    }
    if i.unsigned_abs() <= 1 << 53 {
        return (i as f64).partial_cmp(&f);
    }
    // fora de [-2^63, 2^63) o decimal está além de qualquer i64
    if f >= i64::MAX as f64 {
        return Some(Ordering::Less);
    }
    if f < i64::MIN as f64 {
        return Some(Ordering::Greater);
    }
    let t = f.trunc();
    match i.cmp(&(t as i64)) {
        Ordering::Equal => (0.0).partial_cmp(&(f - t)),
        o => Some(o),
    }
}

/// Data no formato AAAA-MM-DD, com dígitos ASCII e data que existe no calendário.
/// Devolve AAAAMMDD como inteiro, que ordena como a data.
pub fn data(b: &[u8]) -> Option<u32> {
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let mut n = [0u32; 8];
    for (k, &p) in [0, 1, 2, 3, 5, 6, 8, 9].iter().enumerate() {
        let c = b[p];
        if !c.is_ascii_digit() {
            return None;
        }
        n[k] = (c - b'0') as u32;
    }
    let ano = n[0] * 1000 + n[1] * 100 + n[2] * 10 + n[3];
    let mes = n[4] * 10 + n[5];
    let dia = n[6] * 10 + n[7];
    let bissexto = ano % 4 == 0 && (ano % 100 != 0 || ano % 400 == 0);
    let dias = match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if bissexto => 29,
        2 => 28,
        _ => return None,
    };
    if ano == 0 || dia == 0 || dia > dias {
        return None;
    }
    Some(ano * 10000 + mes * 100 + dia)
}

/// Igualdade de fatias sem chamar o memcmp da libc para as curtas: compara tamanho e os
/// primeiros 8 bytes como um inteiro; o resto só é comparado quando isso já bateu.
#[inline]
pub fn igual(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && prefixo(a) == prefixo(b) && (a.len() <= 8 || a[8..] == b[8..])
}

/// Os primeiros 8 bytes como inteiro (completando com zero).
#[inline]
pub fn prefixo(s: &[u8]) -> u64 {
    let mut v = 0;
    for (i, &c) in s.iter().take(8).enumerate() {
        v |= (c as u64) << (8 * i);
    }
    v
}
