//! Parser JSON de uma linha: valida a linha inteira e guarda só os campos que as regras usam.
//!
//! Aceita o mesmo que o `json` do Python (quem gera o gabarito): inclusive NaN, Infinity e
//! surrogate solto em `\uXXXX`. Chave repetida: vale a última; se a repetida for um objeto pai,
//! o objeto inteiro é substituído, sem mesclar com o anterior.

use crate::valor::{Num, Txt, Valor, igual, prefixo};

/// Árvore dos caminhos usados pelas regras. Nó 0 é a raiz; cada folha aponta para um slot.
pub struct Trie {
    nos: Vec<No>,
}

struct No {
    filhos: Vec<Filho>,
    slot: Option<u32>,
    /// slots deste nó e de todos abaixo dele, zerados quando a chave reaparece
    abaixo: Vec<u32>,
}

impl Trie {
    /// O slot `i` corresponde a `caminhos[i]`, com partes separadas por ponto.
    pub fn novo(caminhos: &[String]) -> Trie {
        let mut t = Trie {
            nos: vec![No {
                filhos: vec![],
                slot: None,
                abaixo: vec![],
            }],
        };
        for (s, caminho) in caminhos.iter().enumerate() {
            let mut no = 0;
            for parte in caminho.split('.') {
                no = match t.filho(no, parte.as_bytes(), prefixo(parte.as_bytes())) {
                    Some(f) => f,
                    None => {
                        t.nos.push(No {
                            filhos: vec![],
                            slot: None,
                            abaixo: vec![],
                        });
                        let f = (t.nos.len() - 1) as u32;
                        let chave: Box<[u8]> = parte.as_bytes().into();
                        t.nos[no as usize].filhos.push(Filho {
                            prefixo: prefixo(&chave),
                            chave,
                            no: f,
                        });
                        f
                    }
                };
                t.nos[no as usize].abaixo.push(s as u32);
            }
            t.nos[no as usize].slot = Some(s as u32);
        }
        t
    }

    fn filho(&self, no: u32, chave: &[u8], p: u64) -> Option<u32> {
        let f = self.nos[no as usize].filhos.iter().find(|f| {
            f.prefixo == p
                && f.chave.len() == chave.len()
                && (chave.len() <= 8 || f.chave[8..] == chave[8..])
        });
        f.map(|f| f.no)
    }
}

/// Chave de um filho com tamanho e primeiros 8 bytes pré-calculados: quase toda chave da linha
/// que não é a procurada já difere aí, sem comparar byte a byte.
struct Filho {
    chave: Box<[u8]>,
    prefixo: u64,
    no: u32,
}

/// Profundidade máxima de aninhamento. O Python desiste perto de mil níveis; além disso a linha
/// é tratada como inválida.
const PROF_MAX: u32 = 900;

/// A linha não é JSON válido (para o `json` do Python).
#[derive(Debug)]
pub struct Invalido;

type R<T> = Result<T, Invalido>;

pub struct Linha<'a> {
    b: &'a [u8],
    i: usize,
    trie: &'a Trie,
    slots: &'a mut [Valor],
    arena: &'a mut Vec<u8>,
    campo_id: &'a [u8],
    /// trecho cru do id, quando o último `id` do primeiro nível é número
    pub id: Option<(u32, u32)>,
}

impl<'a> Linha<'a> {
    pub fn new(
        b: &'a [u8],
        trie: &'a Trie,
        slots: &'a mut [Valor],
        arena: &'a mut Vec<u8>,
        campo_id: &'a [u8],
    ) -> Self {
        Linha {
            b,
            i: 0,
            trie,
            slots,
            arena,
            campo_id,
            id: None,
        }
    }

    /// Ok(true): objeto válido. Ok(false): JSON válido que não é objeto. Err: JSON inválido.
    pub fn documento(&mut self) -> R<bool> {
        self.ws();
        let objeto = self.peek() == Some(b'{');
        if objeto {
            self.objeto(Some(0), 1)?;
        } else {
            self.valor(None, 0)?;
        }
        self.ws();
        if self.i != self.b.len() {
            return Err(Invalido);
        }
        Ok(objeto)
    }

    #[inline]
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    #[inline]
    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.i += 1;
        }
    }

    fn literal(&mut self, s: &[u8]) -> R<()> {
        if self.b[self.i..].starts_with(s) {
            self.i += s.len();
            Ok(())
        } else {
            Err(Invalido)
        }
    }

    fn valor(&mut self, no: Option<u32>, prof: u32) -> R<Valor> {
        self.ws();
        match self.peek().ok_or(Invalido)? {
            b'{' => self.objeto(no, prof + 1).map(|_| Valor::Outro),
            b'[' => self.lista(prof + 1).map(|_| Valor::Outro),
            b'"' => self.texto(no.is_some()).map(Valor::Texto),
            b't' => self.literal(b"true").map(|_| Valor::Bool(true)),
            b'f' => self.literal(b"false").map(|_| Valor::Bool(false)),
            b'n' => self.literal(b"null").map(|_| Valor::Nulo),
            b'N' => self.literal(b"NaN").map(|_| Valor::Num(Num::F(f64::NAN))),
            b'I' => self
                .literal(b"Infinity")
                .map(|_| Valor::Num(Num::F(f64::INFINITY))),
            b'-' if self.b.get(self.i + 1) == Some(&b'I') => self
                .literal(b"-Infinity")
                .map(|_| Valor::Num(Num::F(f64::NEG_INFINITY))),
            b'-' | b'0'..=b'9' => self.numero(no.is_some()).map(Valor::Num),
            _ => Err(Invalido),
        }
    }

    fn objeto(&mut self, no: Option<u32>, prof: u32) -> R<()> {
        if prof > PROF_MAX {
            return Err(Invalido);
        }
        self.i += 1;
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(());
        }
        let raiz = no == Some(0) && prof == 1;
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(Invalido);
            }
            let (filho, eh_id) = match no {
                Some(n) => {
                    let t = self.texto(true)?;
                    let chave = self.bytes(t);
                    let p = match t {
                        // chave sem escape: os 8 bytes vêm direto da linha, mascarados no tamanho
                        Txt::Linha(a, _) if a as usize + 8 <= self.b.len() => {
                            let w = u64::from_le_bytes(
                                self.b[a as usize..a as usize + 8].try_into().unwrap(),
                            );
                            if chave.len() >= 8 {
                                w
                            } else {
                                w & ((1u64 << (8 * chave.len())) - 1)
                            }
                        }
                        _ => prefixo(chave),
                    };
                    (
                        self.trie.filho(n, chave, p),
                        raiz && igual(chave, self.campo_id),
                    )
                }
                None => {
                    self.texto(false)?;
                    (None, false)
                }
            };
            self.ws();
            if self.peek() != Some(b':') {
                return Err(Invalido);
            }
            self.i += 1;
            if let Some(f) = filho {
                for &s in &self.trie.nos[f as usize].abaixo {
                    self.slots[s as usize] = Valor::Ausente;
                }
            }
            self.ws();
            let ini = self.i;
            let v = self.valor(filho, prof)?;
            if eh_id {
                self.id = matches!(v, Valor::Num(_)).then_some((ini as u32, self.i as u32));
            }
            if let Some(s) = filho.and_then(|f| self.trie.nos[f as usize].slot) {
                self.slots[s as usize] = v;
            }
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(());
                }
                _ => return Err(Invalido),
            }
        }
    }

    fn lista(&mut self, prof: u32) -> R<()> {
        if prof > PROF_MAX {
            return Err(Invalido);
        }
        self.i += 1;
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(());
        }
        loop {
            self.valor(None, prof)?;
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(());
                }
                _ => return Err(Invalido),
            }
        }
    }

    pub fn bytes(&self, t: Txt) -> &[u8] {
        match t {
            Txt::Linha(a, b) => &self.b[a as usize..b as usize],
            Txt::Arena(a, b) => &self.arena[a as usize..b as usize],
        }
    }

    /// Lê uma string. Sem escape, devolve o trecho da linha; com escape, decodifica na arena
    /// (só quando `guardar`; senão apenas valida). UTF-8 só é conferido se a string tiver byte
    /// não ASCII: fora de strings, um byte desses já é erro de sintaxe.
    #[inline(always)]
    fn texto(&mut self, guardar: bool) -> R<Txt> {
        self.i += 1;
        let ini = self.i;
        let mut nao_ascii = false;
        loop {
            // salta 8 bytes por vez até achar aspas, barra, controle ou byte não ASCII
            while let Some(w) = self.b.get(self.i..self.i + 8) {
                let m = especial(u64::from_le_bytes(w.try_into().unwrap()));
                if m != 0 {
                    self.i += (m.trailing_zeros() / 8) as usize;
                    break;
                }
                self.i += 8;
            }
            match *self.b.get(self.i).ok_or(Invalido)? {
                b'"' => {
                    self.i += 1;
                    self.utf8(ini, nao_ascii)?;
                    return Ok(Txt::Linha(ini as u32, (self.i - 1) as u32));
                }
                b'\\' => break,
                0..0x20 => return Err(Invalido),
                c => {
                    nao_ascii |= c >= 0x80;
                    self.i += 1;
                }
            }
        }
        let a0 = self.arena.len();
        if guardar {
            self.arena.extend_from_slice(&self.b[ini..self.i]);
        }
        loop {
            let c = *self.b.get(self.i).ok_or(Invalido)?;
            match c {
                b'"' => {
                    self.i += 1;
                    self.utf8(ini, nao_ascii)?;
                    return Ok(Txt::Arena(a0 as u32, self.arena.len() as u32));
                }
                b'\\' => self.escape(guardar)?,
                0..0x20 => return Err(Invalido),
                _ => {
                    nao_ascii |= c >= 0x80;
                    if guardar {
                        self.arena.push(c);
                    }
                    self.i += 1;
                }
            }
        }
    }

    /// Confere o UTF-8 do trecho cru da string que acabou de fechar (as aspas ficam de fora).
    fn utf8(&self, ini: usize, nao_ascii: bool) -> R<()> {
        if nao_ascii && std::str::from_utf8(&self.b[ini..self.i - 1]).is_err() {
            return Err(Invalido);
        }
        Ok(())
    }

    fn escape(&mut self, guardar: bool) -> R<()> {
        let c = *self.b.get(self.i + 1).ok_or(Invalido)?;
        self.i += 2;
        let byte = match c {
            b'"' => b'"',
            b'\\' => b'\\',
            b'/' => b'/',
            b'b' => 0x08,
            b'f' => 0x0c,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'u' => {
                let mut cp = self.hex4()?;
                // par de surrogates vira um caractere só; surrogate solto fica como está
                if (0xD800..0xDC00).contains(&cp) && self.b[self.i..].starts_with(b"\\u") {
                    let salvo = self.i;
                    self.i += 2;
                    match self.hex4() {
                        Ok(lo) if (0xDC00..0xE000).contains(&lo) => {
                            cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00)
                        }
                        _ => self.i = salvo,
                    }
                }
                if guardar {
                    wtf8(cp, self.arena);
                }
                return Ok(());
            }
            _ => return Err(Invalido),
        };
        if guardar {
            self.arena.push(byte);
        }
        Ok(())
    }

    fn hex4(&mut self) -> R<u32> {
        let h = self.b.get(self.i..self.i + 4).ok_or(Invalido)?;
        let mut v = 0;
        for &c in h {
            v = v * 16 + (c as char).to_digit(16).ok_or(Invalido)?;
        }
        self.i += 4;
        Ok(v)
    }

    /// Mesma gramática do Python: -?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][-+]?[0-9]+)?
    /// Só converte o texto em número quando `guardar`; senão apenas valida.
    #[inline(always)]
    fn numero(&mut self, guardar: bool) -> R<Num> {
        let b = self.b;
        let ini = self.i;
        let digitos = |i: &mut usize| {
            while matches!(b.get(*i), Some(b'0'..=b'9')) {
                *i += 1;
            }
        };
        if b.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        match b.get(self.i) {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => digitos(&mut self.i),
            _ => return Err(Invalido),
        }
        let mut inteiro = true;
        if b.get(self.i) == Some(&b'.') && matches!(b.get(self.i + 1), Some(b'0'..=b'9')) {
            inteiro = false;
            self.i += 1;
            digitos(&mut self.i);
        }
        if matches!(b.get(self.i), Some(b'e' | b'E')) {
            let mut j = self.i + 1;
            if matches!(b.get(j), Some(b'+' | b'-')) {
                j += 1;
            }
            if matches!(b.get(j), Some(b'0'..=b'9')) {
                inteiro = false;
                self.i = j;
                digitos(&mut self.i);
            }
        }
        if !guardar {
            return Ok(Num::I(0));
        }
        // SAFETY: o trecho casou com a gramática de número acima, então é ASCII
        let s = unsafe { std::str::from_utf8_unchecked(&b[ini..self.i]) };
        if inteiro && let Ok(v) = s.parse::<i64>() {
            return Ok(Num::I(v));
        }
        s.parse::<f64>().map(Num::F).map_err(|_| Invalido)
    }
}

/// Marca, em cada um dos 8 bytes da palavra, se o byte é `"`, `\`, menor que 0x20 ou não ASCII.
/// O bit mais baixo marcado é sempre o primeiro byte especial (os acima dele podem ser falsos).
#[inline]
fn especial(w: u64) -> u64 {
    const UM: u64 = 0x0101_0101_0101_0101;
    const ALTO: u64 = 0x8080_8080_8080_8080;
    let zero = |x: u64| x.wrapping_sub(UM) & !x & ALTO;
    zero(w ^ (UM * b'"' as u64))
        | zero(w ^ (UM * b'\\' as u64))
        | (w.wrapping_sub(UM * 0x20) & !w & ALTO)
        | (w & ALTO)
}

/// Codifica um code point em UTF-8, aceitando surrogates (WTF-8), para comparar byte a byte
/// com o mesmo resultado que o Python tem comparando caracteres.
fn wtf8(cp: u32, out: &mut Vec<u8>) {
    match cp {
        0..0x80 => out.push(cp as u8),
        0x80..0x800 => out.extend_from_slice(&[0xC0 | (cp >> 6) as u8, 0x80 | (cp & 0x3F) as u8]),
        0x800..0x10000 => out.extend_from_slice(&[
            0xE0 | (cp >> 12) as u8,
            0x80 | ((cp >> 6) & 0x3F) as u8,
            0x80 | (cp & 0x3F) as u8,
        ]),
        _ => out.extend_from_slice(&[
            0xF0 | (cp >> 18) as u8,
            0x80 | ((cp >> 12) & 0x3F) as u8,
            0x80 | ((cp >> 6) & 0x3F) as u8,
            0x80 | (cp & 0x3F) as u8,
        ]),
    }
}

/// Recupera o id de uma linha inválida, como o oráculo: primeira ocorrência de
/// `"id"\s*:\s*` seguida de número.
pub fn id_cru<'b>(linha: &'b [u8], campo_id: &[u8]) -> Option<&'b [u8]> {
    let mut agulha = Vec::with_capacity(campo_id.len() + 2);
    agulha.push(b'"');
    agulha.extend_from_slice(campo_id);
    agulha.push(b'"');
    let espaco = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c | 0x1c..=0x1f);
    for pos in memchr::memmem::find_iter(linha, &agulha) {
        let mut j = pos + agulha.len();
        while j < linha.len() && espaco(linha[j]) {
            j += 1;
        }
        if linha.get(j) != Some(&b':') {
            continue;
        }
        j += 1;
        while j < linha.len() && espaco(linha[j]) {
            j += 1;
        }
        let ini = j;
        if linha.get(j) == Some(&b'-') {
            j += 1;
        }
        let d0 = j;
        while matches!(linha.get(j), Some(b'0'..=b'9')) {
            j += 1;
        }
        if j == d0 {
            continue;
        }
        if linha.get(j) == Some(&b'.') && matches!(linha.get(j + 1), Some(b'0'..=b'9')) {
            j += 1;
            while matches!(linha.get(j), Some(b'0'..=b'9')) {
                j += 1;
            }
        }
        if matches!(linha.get(j), Some(b'e' | b'E')) {
            let mut k = j + 1;
            if matches!(linha.get(k), Some(b'+' | b'-')) {
                k += 1;
            }
            if matches!(linha.get(k), Some(b'0'..=b'9')) {
                j = k;
                while matches!(linha.get(j), Some(b'0'..=b'9')) {
                    j += 1;
                }
            }
        }
        return Some(&linha[ini..j]);
    }
    None
}
