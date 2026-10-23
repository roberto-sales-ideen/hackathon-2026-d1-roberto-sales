//! Avaliação: a árvore do manifesto vira condições sobre slots, e cada linha vira uma saída.

use crate::json::{Linha, Trie, id_cru};
use crate::manifesto::{Expr, Lado, Lit, Manifesto, Op, Tipo};
use crate::valor::{Num, Txt, Valor, cmp_num, data, igual};
use std::cmp::Ordering;

enum Cond {
    E(Box<[Cond]>),
    Ou(Box<[Cond]>),
    Nao(Box<Cond>),
    Cmp {
        a: u32,
        tipo: Tipo,
        op: Op,
        b: LadoC,
    },
    Em {
        a: u32,
        tipo: Tipo,
        lista: Box<[Lit]>,
    },
    ComecaCom {
        a: u32,
        prefixo: Box<[u8]>,
    },
    Vazio(u32),
    Booleano {
        a: u32,
        valor: bool,
    },
}

enum LadoC {
    Lit(Lit),
    Campo(u32),
}

/// Valor de um slot já filtrado pelo tipo que a regra espera.
#[derive(Clone, Copy)]
enum V<'x> {
    T(&'x [u8]),
    N(Num),
    D(u32),
    B(bool),
}

pub struct Motor {
    regras: Vec<(Box<[u8]>, Cond)>,
    sem_regra: Box<[u8]>,
    campo_id: Box<[u8]>,
    trie: Trie,
    slots: Vec<Valor>,
    arena: Vec<u8>,
    /// lista todas as regras satisfeitas, não só a primeira
    pub todas: bool,
}

impl Motor {
    pub fn novo(m: &Manifesto) -> Motor {
        let mut caminhos: Vec<String> = Vec::new();
        let regras = m
            .regras
            .iter()
            .map(|r| (r.codigo.as_bytes().into(), compilar(&r.expr, &mut caminhos)))
            .collect();
        Motor {
            regras,
            sem_regra: m.sem_regra.as_bytes().into(),
            campo_id: m.campo_id.as_bytes().into(),
            trie: Trie::novo(&caminhos),
            slots: vec![Valor::Ausente; caminhos.len()],
            arena: Vec::new(),
            todas: false,
        }
    }

    /// Classifica uma linha e acrescenta `id:X - regra: Y\n` em `out`. Linha em branco ou sem id
    /// recuperável não gera saída.
    pub fn linha(&mut self, l: &[u8], out: &mut Vec<u8>) {
        if l.iter()
            .all(|&c| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c | 0x1c..=0x1f))
        {
            return;
        }
        self.slots.fill(Valor::Ausente);
        self.arena.clear();
        let mut p = Linha::new(
            l,
            &self.trie,
            &mut self.slots,
            &mut self.arena,
            &self.campo_id,
        );
        let resultado = p.documento();
        let id = p.id;
        match resultado {
            Ok(true) => {
                let Some((a, b)) = id else { return };
                escreve_id(out, &l[a as usize..b as usize]);
                let ctx = Ctx {
                    linha: l,
                    arena: &self.arena,
                    slots: &self.slots,
                };
                let mut alguma = false;
                for (codigo, cond) in &self.regras {
                    if ctx.avalia(cond) {
                        if alguma {
                            out.push(b',');
                        }
                        out.extend_from_slice(codigo);
                        alguma = true;
                        if !self.todas {
                            break;
                        }
                    }
                }
                if !alguma {
                    out.extend_from_slice(&self.sem_regra);
                }
                out.push(b'\n');
            }
            // JSON válido que não é objeto, ou JSON inválido: SEM_REGRA, se der para achar o id
            _ => {
                if let Some(id) = id_cru(l, &self.campo_id) {
                    escreve_id(out, id);
                    out.extend_from_slice(&self.sem_regra);
                    out.push(b'\n');
                }
            }
        }
    }
}

fn escreve_id(out: &mut Vec<u8>, id: &[u8]) {
    out.extend_from_slice(b"id:");
    out.extend_from_slice(id);
    out.extend_from_slice(b" - regra: ");
}

fn slot(caminhos: &mut Vec<String>, campo: &str) -> u32 {
    match caminhos.iter().position(|c| c == campo) {
        Some(i) => i as u32,
        None => {
            caminhos.push(campo.to_string());
            (caminhos.len() - 1) as u32
        }
    }
}

fn compilar(e: &Expr, cs: &mut Vec<String>) -> Cond {
    match e {
        Expr::E(xs) => Cond::E(xs.iter().map(|x| compilar(x, cs)).collect()),
        Expr::Ou(xs) => Cond::Ou(xs.iter().map(|x| compilar(x, cs)).collect()),
        Expr::Nao(x) => Cond::Nao(Box::new(compilar(x, cs))),
        Expr::Cmp {
            campo,
            tipo,
            op,
            lado,
        } => {
            let a = slot(cs, campo);
            let b = match lado {
                Lado::Lit(l) | Lado::Const(_, l) => LadoC::Lit(l.clone()),
                Lado::Campo(c) => LadoC::Campo(slot(cs, c)),
            };
            Cond::Cmp {
                a,
                tipo: *tipo,
                op: *op,
                b,
            }
        }
        Expr::Em { campo, tipo, lista } => Cond::Em {
            a: slot(cs, campo),
            tipo: *tipo,
            lista: lista.clone().into(),
        },
        Expr::ComecaCom { campo, prefixo } => Cond::ComecaCom {
            a: slot(cs, campo),
            prefixo: prefixo.clone(),
        },
        Expr::Vazio { campo } => Cond::Vazio(slot(cs, campo)),
        Expr::Booleano { campo, valor } => Cond::Booleano {
            a: slot(cs, campo),
            valor: *valor,
        },
    }
}

struct Ctx<'x> {
    linha: &'x [u8],
    arena: &'x [u8],
    slots: &'x [Valor],
}

impl<'x> Ctx<'x> {
    fn texto(&self, t: Txt) -> &'x [u8] {
        match t {
            Txt::Linha(a, b) => &self.linha[a as usize..b as usize],
            Txt::Arena(a, b) => &self.arena[a as usize..b as usize],
        }
    }

    /// Campo ausente, nulo ou de outro tipo não tem valor: qualquer comparação com ele é falsa.
    fn tipado(&self, s: u32, tipo: Tipo) -> Option<V<'x>> {
        match (self.slots[s as usize], tipo) {
            (Valor::Texto(t), Tipo::Texto) => Some(V::T(self.texto(t))),
            (Valor::Texto(t), Tipo::Data) => data(self.texto(t)).map(V::D),
            (Valor::Num(n), Tipo::Numero) => Some(V::N(n)),
            (Valor::Bool(b), Tipo::Booleano) => Some(V::B(b)),
            _ => None,
        }
    }

    fn avalia(&self, c: &Cond) -> bool {
        match c {
            Cond::E(xs) => xs.iter().all(|x| self.avalia(x)),
            Cond::Ou(xs) => xs.iter().any(|x| self.avalia(x)),
            Cond::Nao(x) => !self.avalia(x),
            Cond::Cmp { a, tipo, op, b } => {
                let Some(va) = self.tipado(*a, *tipo) else {
                    return false;
                };
                let vb = match b {
                    LadoC::Lit(l) => lit(l),
                    LadoC::Campo(s) => match self.tipado(*s, *tipo) {
                        Some(v) => v,
                        None => return false,
                    },
                };
                aplica(*op, compara(va, vb))
            }
            Cond::Em { a, tipo, lista } => match self.tipado(*a, *tipo) {
                Some(va) => lista
                    .iter()
                    .any(|l| compara(va, lit(l)) == Some(Ordering::Equal)),
                None => false,
            },
            Cond::ComecaCom { a, prefixo } => match self.tipado(*a, Tipo::Texto) {
                Some(V::T(t)) => t.starts_with(prefixo),
                _ => false,
            },
            Cond::Vazio(a) => match self.slots[*a as usize] {
                Valor::Ausente | Valor::Nulo => true,
                Valor::Texto(t) => self.texto(t).is_empty(),
                _ => false,
            },
            Cond::Booleano { a, valor } => {
                matches!(self.slots[*a as usize], Valor::Bool(b) if b == *valor)
            }
        }
    }
}

fn lit(l: &Lit) -> V<'_> {
    match l {
        Lit::Texto(t) => V::T(t),
        Lit::Num(n) => V::N(*n),
        Lit::Data(d) => V::D(*d),
        Lit::Bool(b) => V::B(*b),
    }
}

/// None quando não há ordem (NaN): aí só ≠ é verdadeiro, como no Python.
fn compara(a: V, b: V) -> Option<Ordering> {
    match (a, b) {
        // texto só tem igualdade (o manifesto recusa < e > em texto)
        (V::T(x), V::T(y)) => Some(if igual(x, y) {
            Ordering::Equal
        } else {
            Ordering::Less
        }),
        (V::N(x), V::N(y)) => cmp_num(x, y),
        (V::D(x), V::D(y)) => Some(x.cmp(&y)),
        (V::B(x), V::B(y)) => Some(x.cmp(&y)),
        _ => None,
    }
}

fn aplica(op: Op, o: Option<Ordering>) -> bool {
    use Ordering::*;
    match op {
        Op::Eq => o == Some(Equal),
        Op::Ne => o != Some(Equal),
        Op::Lt => o == Some(Less),
        Op::Le => matches!(o, Some(Less | Equal)),
        Op::Gt => o == Some(Greater),
        Op::Ge => matches!(o, Some(Greater | Equal)),
    }
}
