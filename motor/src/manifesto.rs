//! Leitura do manifesto (YAML) e compilação da linguagem do `quando` para uma árvore tipada.
//!
//! A linguagem é a notação da tabela de regras do enunciado:
//!   expr  := e ("OU" e)*
//!   e     := unario ("E" unario)*
//!   unario:= "NÃO" unario | "(" expr ")" | cond
//!   cond  := campo op operando
//!          | campo "é" ("verdadeiro" | "falso")          (campo booleano)
//!          | campo "é" operando ((","|"ou") operando)*  (pertence à lista)
//!          | campo "está" "vazio"
//!          | campo "começa" "com" operando

use crate::valor::{Num, data};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arquivo {
    #[serde(default = "id_padrao")]
    campo_id: String,
    sem_regra: String,
    #[serde(default)]
    constantes: BTreeMap<String, serde_yaml_ng::Value>,
    campos: BTreeMap<String, String>,
    regras: Vec<RegraArquivo>,
}

fn id_padrao() -> String {
    "id".into()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegraArquivo {
    codigo: String,
    #[serde(default)]
    categoria: String,
    quando: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tipo {
    Texto,
    Numero,
    Data,
    Booleano,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Lit {
    Texto(Box<[u8]>),
    Num(Num),
    Data(u32),
    Bool(bool),
}

#[derive(Clone, Debug)]
pub enum Lado {
    Lit(Lit),
    Const(String, Lit),
    Campo(String),
}

#[derive(Clone, Debug)]
pub enum Expr {
    E(Vec<Expr>),
    Ou(Vec<Expr>),
    Nao(Box<Expr>),
    Cmp {
        campo: String,
        tipo: Tipo,
        op: Op,
        lado: Lado,
    },
    Em {
        campo: String,
        tipo: Tipo,
        lista: Vec<Lit>,
    },
    ComecaCom {
        campo: String,
        prefixo: Box<[u8]>,
    },
    Vazio {
        campo: String,
    },
    Booleano {
        campo: String,
        valor: bool,
    },
}

pub struct Regra {
    pub codigo: String,
    pub categoria: String,
    pub expr: Expr,
}

pub struct Manifesto {
    pub campo_id: String,
    pub sem_regra: String,
    pub regras: Vec<Regra>,
}

pub fn carregar(caminho: &str) -> Result<Manifesto, String> {
    let texto = std::fs::read_to_string(caminho).map_err(|e| format!("{caminho}: {e}"))?;
    ler(&texto).map_err(|e| format!("{caminho}: {e}"))
}

pub fn ler(texto: &str) -> Result<Manifesto, String> {
    let arq: Arquivo = serde_yaml_ng::from_str(texto).map_err(|e| e.to_string())?;
    if arq.campo_id.contains('.') {
        return Err("campo_id precisa ser um campo do primeiro nível".into());
    }
    let mut campos = BTreeMap::new();
    for (nome, tipo) in &arq.campos {
        let t = match tipo.as_str() {
            "texto" => Tipo::Texto,
            "numero" => Tipo::Numero,
            "data" => Tipo::Data,
            "booleano" => Tipo::Booleano,
            outro => return Err(format!("campo {nome}: tipo desconhecido '{outro}'")),
        };
        campos.insert(nome.clone(), t);
    }
    let mut constantes = BTreeMap::new();
    for (nome, v) in &arq.constantes {
        let s = match v {
            serde_yaml_ng::Value::String(s) => s.clone(),
            serde_yaml_ng::Value::Number(n) => n.to_string(),
            serde_yaml_ng::Value::Bool(b) => b.to_string(),
            _ => {
                return Err(format!(
                    "constante {nome}: precisa ser texto, número ou booleano"
                ));
            }
        };
        constantes.insert(nome.clone(), s);
    }
    let ctx = Contexto {
        campos: &campos,
        constantes: &constantes,
    };
    let mut regras = Vec::new();
    for r in arq.regras {
        let expr = ctx
            .compilar(&r.quando)
            .map_err(|e| format!("regra {}: {e}", r.codigo))?;
        regras.push(Regra {
            codigo: r.codigo,
            categoria: r.categoria,
            expr,
        });
    }
    Ok(Manifesto {
        campo_id: arq.campo_id,
        sem_regra: arq.sem_regra,
        regras,
    })
}

// ---------------------------------------------------------------- tokens

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    AbreP,
    FechaP,
    Virgula,
    Op(Op),
    Palavra(String),
    Aspas(String),
}

fn tokens(s: &str) -> Result<Vec<Tok>, String> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let eh_op = |c: char| matches!(c, '=' | '≠' | '!' | '<' | '>' | '≤' | '≥');
    while i < cs.len() {
        let c = cs[i];
        let prox = cs.get(i + 1).copied();
        match c {
            _ if c.is_whitespace() => i += 1,
            '(' => {
                out.push(Tok::AbreP);
                i += 1
            }
            ')' => {
                out.push(Tok::FechaP);
                i += 1
            }
            ',' => {
                out.push(Tok::Virgula);
                i += 1
            }
            '=' => {
                out.push(Tok::Op(Op::Eq));
                i += 1
            }
            '≠' => {
                out.push(Tok::Op(Op::Ne));
                i += 1
            }
            '≤' => {
                out.push(Tok::Op(Op::Le));
                i += 1
            }
            '≥' => {
                out.push(Tok::Op(Op::Ge));
                i += 1
            }
            '!' if prox == Some('=') => {
                out.push(Tok::Op(Op::Ne));
                i += 2
            }
            '<' if prox == Some('=') => {
                out.push(Tok::Op(Op::Le));
                i += 2
            }
            '>' if prox == Some('=') => {
                out.push(Tok::Op(Op::Ge));
                i += 2
            }
            '<' => {
                out.push(Tok::Op(Op::Lt));
                i += 1
            }
            '>' => {
                out.push(Tok::Op(Op::Gt));
                i += 1
            }
            '"' => {
                let fim = cs[i + 1..]
                    .iter()
                    .position(|&x| x == '"')
                    .ok_or("aspas sem fechar")?;
                out.push(Tok::Aspas(cs[i + 1..i + 1 + fim].iter().collect()));
                i += fim + 2;
            }
            _ if eh_op(c) => return Err(format!("operador inválido perto de '{c}'")),
            _ => {
                let ini = i;
                while i < cs.len() {
                    let x = cs[i];
                    if x.is_whitespace() || matches!(x, '(' | ')' | ',' | '"') || eh_op(x) {
                        break;
                    }
                    i += 1;
                }
                out.push(Tok::Palavra(cs[ini..i].iter().collect()));
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- parser

struct Contexto<'a> {
    campos: &'a BTreeMap<String, Tipo>,
    constantes: &'a BTreeMap<String, String>,
}

struct Parser<'a> {
    ctx: &'a Contexto<'a>,
    t: Vec<Tok>,
    i: usize,
}

impl Contexto<'_> {
    fn compilar(&self, quando: &str) -> Result<Expr, String> {
        let mut p = Parser {
            ctx: self,
            t: tokens(quando)?,
            i: 0,
        };
        let e = p.expr()?;
        if p.i != p.t.len() {
            return Err(format!("sobrou texto a partir de {:?}", p.t[p.i]));
        }
        Ok(e)
    }
}

fn palavra(t: Option<&Tok>, w: &str) -> bool {
    matches!(t, Some(Tok::Palavra(x)) if x == w)
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn proximo(&mut self) -> Result<Tok, String> {
        let t = self
            .t
            .get(self.i)
            .cloned()
            .ok_or("condição terminou no meio")?;
        self.i += 1;
        Ok(t)
    }

    fn espera(&mut self, w: &str) -> Result<(), String> {
        if palavra(self.peek(), w) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("esperava '{w}', veio {:?}", self.peek()))
        }
    }

    fn expr(&mut self) -> Result<Expr, String> {
        let mut xs = vec![self.e()?];
        while palavra(self.peek(), "OU") {
            self.i += 1;
            xs.push(self.e()?);
        }
        Ok(if xs.len() == 1 {
            xs.pop().unwrap()
        } else {
            Expr::Ou(xs)
        })
    }

    fn e(&mut self) -> Result<Expr, String> {
        let mut xs = vec![self.unario()?];
        while palavra(self.peek(), "E") {
            self.i += 1;
            xs.push(self.unario()?);
        }
        Ok(if xs.len() == 1 {
            xs.pop().unwrap()
        } else {
            Expr::E(xs)
        })
    }

    fn unario(&mut self) -> Result<Expr, String> {
        if palavra(self.peek(), "NÃO") {
            self.i += 1;
            return Ok(Expr::Nao(Box::new(self.unario()?)));
        }
        if self.peek() == Some(&Tok::AbreP) {
            self.i += 1;
            let e = self.expr()?;
            if self.peek() != Some(&Tok::FechaP) {
                return Err("parêntese sem fechar".into());
            }
            self.i += 1;
            return Ok(e);
        }
        self.cond()
    }

    fn cond(&mut self) -> Result<Expr, String> {
        let campo = match self.proximo()? {
            Tok::Palavra(w) => w,
            t => return Err(format!("esperava um campo, veio {t:?}")),
        };
        let tipo = *self
            .ctx
            .campos
            .get(&campo)
            .ok_or(format!("campo não declarado: {campo}"))?;
        match self.proximo()? {
            Tok::Op(op) => {
                if tipo == Tipo::Texto && !matches!(op, Op::Eq | Op::Ne) {
                    return Err(format!("{campo} é texto: só aceita = e ≠"));
                }
                if tipo == Tipo::Booleano && !matches!(op, Op::Eq | Op::Ne) {
                    return Err(format!("{campo} é booleano: só aceita = e ≠"));
                }
                let lado = self.operando(&campo, tipo)?;
                Ok(Expr::Cmp {
                    campo,
                    tipo,
                    op,
                    lado,
                })
            }
            Tok::Palavra(w) if w == "é" => {
                if tipo == Tipo::Booleano {
                    return match self.proximo()? {
                        Tok::Palavra(v) if v == "verdadeiro" => {
                            Ok(Expr::Booleano { campo, valor: true })
                        }
                        Tok::Palavra(v) if v == "falso" => Ok(Expr::Booleano {
                            campo,
                            valor: false,
                        }),
                        t => Err(format!(
                            "{campo} é booleano: esperava verdadeiro ou falso, veio {t:?}"
                        )),
                    };
                }
                let mut lista = vec![self.literal_da_lista(&campo, tipo)?];
                while self.peek() == Some(&Tok::Virgula) || palavra(self.peek(), "ou") {
                    self.i += 1;
                    lista.push(self.literal_da_lista(&campo, tipo)?);
                }
                if lista.len() == 1 {
                    let lit = lista.pop().unwrap();
                    return Ok(Expr::Cmp {
                        campo,
                        tipo,
                        op: Op::Eq,
                        lado: Lado::Lit(lit),
                    });
                }
                Ok(Expr::Em { campo, tipo, lista })
            }
            Tok::Palavra(w) if w == "está" => {
                self.espera("vazio")?;
                Ok(Expr::Vazio { campo })
            }
            Tok::Palavra(w) if w == "começa" => {
                self.espera("com")?;
                if tipo != Tipo::Texto {
                    return Err(format!(
                        "'começa com' só se aplica a texto, e {campo} não é texto"
                    ));
                }
                match self.operando(&campo, tipo)? {
                    Lado::Lit(Lit::Texto(p)) | Lado::Const(_, Lit::Texto(p)) => {
                        Ok(Expr::ComecaCom { campo, prefixo: p })
                    }
                    _ => Err("'começa com' precisa de um literal de texto".into()),
                }
            }
            t => Err(format!(
                "esperava um operador depois de {campo}, veio {t:?}"
            )),
        }
    }

    fn literal_da_lista(&mut self, campo: &str, tipo: Tipo) -> Result<Lit, String> {
        match self.operando(campo, tipo)? {
            Lado::Lit(l) | Lado::Const(_, l) => Ok(l),
            Lado::Campo(c) => Err(format!("lista de {campo} não aceita campo ({c})")),
        }
    }

    fn operando(&mut self, campo: &str, tipo: Tipo) -> Result<Lado, String> {
        match self.proximo()? {
            Tok::Aspas(s) => Ok(Lado::Lit(literal(&s, tipo)?)),
            Tok::Palavra(w) => {
                if let Some(&t) = self.ctx.campos.get(&w) {
                    if t != tipo {
                        return Err(format!("{campo} e {w} têm tipos diferentes"));
                    }
                    return Ok(Lado::Campo(w));
                }
                if let Some(v) = self.ctx.constantes.get(&w) {
                    return Ok(Lado::Const(
                        w.clone(),
                        literal(v, tipo).map_err(|e| format!("constante {w}: {e}"))?,
                    ));
                }
                let parece_campo = w.contains('.') && w.parse::<f64>().is_err();
                if parece_campo {
                    return Err(format!("campo não declarado: {w}"));
                }
                Ok(Lado::Lit(literal(&w, tipo)?))
            }
            t => Err(format!("esperava um valor depois de {campo}, veio {t:?}")),
        }
    }
}

/// Converte o texto de um literal para o tipo do campo com que ele é comparado.
fn literal(s: &str, tipo: Tipo) -> Result<Lit, String> {
    match tipo {
        Tipo::Texto => Ok(Lit::Texto(s.as_bytes().into())),
        Tipo::Numero => {
            let numerico = s
                .bytes()
                .next()
                .is_some_and(|c| c.is_ascii_digit() || c == b'-');
            if numerico {
                if let Ok(i) = s.parse::<i64>() {
                    return Ok(Lit::Num(Num::I(i)));
                }
                if let Ok(f) = s.parse::<f64>() {
                    return Ok(Lit::Num(Num::F(f)));
                }
            }
            Err(format!("'{s}' não é número"))
        }
        Tipo::Data => data(s.as_bytes())
            .map(Lit::Data)
            .ok_or(format!("'{s}' não é data AAAA-MM-DD")),
        Tipo::Booleano => match s {
            "verdadeiro" | "true" => Ok(Lit::Bool(true)),
            "falso" | "false" => Ok(Lit::Bool(false)),
            _ => Err(format!("'{s}' não é booleano")),
        },
    }
}
