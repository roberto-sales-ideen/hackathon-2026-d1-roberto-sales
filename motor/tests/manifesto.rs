//! O `regras.yaml` precisa compilar exatamente na árvore escrita à mão em
//! `testes/regras_ast_esperada.json`, e erros de escrita nas regras precisam ser recusados.

use motor::manifesto::{self, Expr, Lado, Lit, Op};
use motor::valor::Num;
use serde_json::{Value, json};

const RAIZ: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

fn op(o: Op) -> &'static str {
    match o {
        Op::Eq => "=",
        Op::Ne => "!=",
        Op::Lt => "<",
        Op::Le => "<=",
        Op::Gt => ">",
        Op::Ge => ">=",
    }
}

fn lit(l: &Lit) -> Value {
    match l {
        Lit::Texto(t) => json!(String::from_utf8_lossy(t)),
        Lit::Num(Num::I(i)) => json!(i),
        Lit::Num(Num::F(f)) => json!(f),
        Lit::Data(d) => json!(format!("{}-{:02}-{:02}", d / 10000, d / 100 % 100, d % 100)),
        Lit::Bool(b) => json!(b),
    }
}

/// Converte a árvore para o formato do JSON esperado (sem a chave "tipo", que agora vem do campo).
fn para_json(e: &Expr) -> Value {
    match e {
        Expr::E(xs) => json!({ "e": xs.iter().map(para_json).collect::<Vec<_>>() }),
        Expr::Ou(xs) => json!({ "ou": xs.iter().map(para_json).collect::<Vec<_>>() }),
        Expr::Nao(x) => json!({ "nao": para_json(x) }),
        Expr::Cmp {
            campo, op: o, lado, ..
        } => match lado {
            Lado::Lit(l) => json!({ "campo": campo, "op": op(*o), "valor": lit(l) }),
            Lado::Const(n, _) => json!({ "campo": campo, "op": op(*o), "constante": n }),
            Lado::Campo(c) => json!({ "campo": campo, "op": op(*o), "outro_campo": c }),
        },
        Expr::Em { campo, lista, .. } => {
            json!({ "campo": campo, "op": "em", "valor": lista.iter().map(lit).collect::<Vec<_>>() })
        }
        Expr::ComecaCom { campo, prefixo } => {
            json!({ "campo": campo, "op": "comeca_com", "valor": String::from_utf8_lossy(prefixo) })
        }
        Expr::Vazio { campo } => json!({ "campo": campo, "op": "vazio" }),
        Expr::Booleano { campo, valor } => {
            json!({ "campo": campo, "op": if *valor { "verdadeiro" } else { "falso" } })
        }
    }
}

fn sem_tipo(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.remove("tipo");
            m.values_mut().for_each(sem_tipo);
        }
        Value::Array(a) => a.iter_mut().for_each(sem_tipo),
        _ => {}
    }
}

#[test]
fn regras_yaml_compila_na_arvore_esperada() {
    let m = manifesto::carregar(&format!("{RAIZ}/regras/regras.yaml")).unwrap();
    let texto = std::fs::read_to_string(format!("{RAIZ}/testes/regras_ast_esperada.json")).unwrap();
    let mut esperado: Value = serde_json::from_str(&texto).unwrap();
    sem_tipo(&mut esperado);
    let regras = esperado["regras"].as_array().unwrap();
    assert_eq!(m.regras.len(), regras.len());
    for (r, e) in m.regras.iter().zip(regras) {
        assert_eq!(r.codigo, e["codigo"], "ordem das regras");
        assert_eq!(r.categoria, e["categoria"], "{}", r.codigo);
        assert_eq!(para_json(&r.expr), e["condicao"], "{}", r.codigo);
    }
}

fn erro(quando: &str) -> String {
    let yaml = format!(
        "sem_regra: X\nconstantes:\n  ref: \"2026-10-23\"\ncampos:\n  a: texto\n  n: numero\n  m: numero\n  d: data\n  b: booleano\nregras:\n  - codigo: R1\n    quando: {quando}\n"
    );
    match manifesto::ler(&yaml) {
        Ok(_) => String::new(),
        Err(e) => e,
    }
}

#[test]
fn aceita_a_notacao_do_enunciado() {
    for ok in [
        "a é X, Y ou Z",
        "a = X OU (n ≥ 10 E n < 20)",
        "n >= 1e5 E n <= 5000 E n != 3",
        "d < ref E n < m",
        "a está vazio E b é falso",
        "a começa com 5",
        "NÃO a = X",
        "a = \"OU\"",
    ] {
        assert_eq!(erro(ok), "", "{ok}");
    }
}

#[test]
fn recusa_regra_mal_escrita() {
    for (ruim, trecho) in [
        ("x = 1", "campo não declarado: x"),
        (
            "a = financeiro.valr",
            "campo não declarado: financeiro.valr",
        ),
        ("n ≥ muito", "não é número"),
        ("d < 2026-02-30", "não é data"),
        ("a < X", "só aceita = e ≠"),
        ("n começa com 5", "só se aplica a texto"),
        ("b é talvez", "esperava verdadeiro ou falso"),
        ("a = n", "tipos diferentes"),
        ("(a = X", "parêntese sem fechar"),
        ("a = X OU", "terminou no meio"),
        ("a = X Y", "sobrou texto"),
    ] {
        let e = erro(ruim);
        assert!(
            e.contains(trecho),
            "'{ruim}' devia falhar com '{trecho}', falhou com '{e}'"
        );
    }
}
