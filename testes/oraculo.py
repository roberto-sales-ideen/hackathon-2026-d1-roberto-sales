#!/usr/bin/env python3
"""Oráculo: implementação de referência, lenta e literal, das regras do ENUNCIADO.

Não lê regras/regras.json de propósito. As regras estão escritas aqui à mão, direto da
tabela do enunciado, para que o teste diferencial confira também a tradução para o JSON.

Decisões de semântica (combinadas com o Beto em 23/10/2026):
  a. chave duplicada: vale a última (comportamento do json do Python)
  b. pai nulo, ausente ou que não é objeto: filhos ausentes
  c. inteiro e decimal (inclusive 1e5) são número; booleano não é número
  d. números em f64
  e. "≠" entre textos: texto vazio é texto ("" ≠ "SP" é verdadeiro)
  f. escapes \\uXXXX são decodificados antes de comparar (chaves e valores)
  g. data só em AAAA-MM-DD com dígitos ASCII e data válida (ano 0001 a 9999)
  h. NaN / Infinity aceitos como o Python aceita
  i. linha inválida: SEM_REGRA, se o id for recuperável; sem id, a linha não gera saída
  j. o id é ecoado com o texto exato da entrada

Uso: python3 oraculo.py --entrada arquivo.jsonl > gabarito.txt
"""
import argparse, datetime, json, re, sys

REF = "2026-10-23"
SEM = "SEM_REGRA"


class Int(int):
    """Inteiro que guarda o texto original, para ecoar o id como veio."""


class Float(float):
    """Decimal que guarda o texto original."""


def _int(s):
    v = Int(s); v.raw = s; return v


def _float(s):
    v = Float(s); v.raw = s; return v


def carregar(texto):
    return json.loads(texto, parse_int=_int, parse_float=_float, parse_constant=_float)


ID_CRU = re.compile(r'"id"\s*:\s*(-?[0-9]+(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)')
DATA = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}\Z")


def get(o, caminho):
    for k in caminho.split("."):
        if not isinstance(o, dict) or k not in o:
            return None
        o = o[k]
    return o


def num(v):
    return v if isinstance(v, (int, float)) and not isinstance(v, bool) else None


def txt(v):
    return v if isinstance(v, str) else None


def data(v):
    if not isinstance(v, str) or not DATA.match(v):
        return None
    try:
        datetime.date.fromisoformat(v)
    except ValueError:
        return None
    return v


def vazio(v):
    return v is None or v == ""


def valores(o):
    """Lê cada campo uma vez, já filtrado pelo tipo esperado (None quando não serve)."""
    g = lambda c: get(o, c)
    return {
        "status": txt(g("status")),
        "entrega": txt(g("logistica.status_entrega")),
        "manif": txt(g("manifestacao.tipo")),
        "manif_cru": g("manifestacao.tipo"),
        "venc": data(g("data_vencimento")),
        "valor": num(g("financeiro.valor")),
        "pago": num(g("financeiro.valor_pago")),
        "negociacao": txt(g("financeiro.tipo_negociacao")),
        "segmento": txt(g("sacado.segmento")),
        "uf_o": txt(g("logistica.uf_origem")),
        "uf_d": txt(g("logistica.uf_destino")),
        "cfop": txt(g("fiscal.cfop")),
        "modal": txt(g("logistica.modal")),
        "peso": num(g("logistica.peso_kg")),
        "migo": g("logistica.migo"),
        "instrumento": txt(g("pagamento.instrumento")),
        "porte": txt(g("sacador.porte")),
    }


def ok(*xs):
    return all(x is not None for x in xs)


# Tabela de regras do enunciado, na ordem de avaliação.
REGRAS = [
    ("R01", lambda v: v["status"] in ("CANCELADA", "LIQUIDADA")),
    ("R02", lambda v: v["entrega"] == "DEVOLVIDA" or v["manif"] == "RCOP"),
    ("R03", lambda v: v["manif"] in ("RCAV", "RCQQ", "RCVN", "RCVL")),
    ("R04", lambda v: v["entrega"] == "AVARIADA" and vazio(v["manif_cru"])),
    ("R05", lambda v: ok(v["venc"], v["valor"], v["pago"]) and v["venc"] < REF and v["pago"] < v["valor"]),
    ("R06", lambda v: v["negociacao"] in ("CSFD", "PENR") and ok(v["valor"]) and v["valor"] >= 100000),
    ("R07", lambda v: ok(v["valor"]) and (v["valor"] >= 500000 or (v["segmento"] == "AGRO" and v["valor"] >= 200000))),
    ("R08", lambda v: ok(v["uf_o"], v["uf_d"], v["cfop"]) and v["uf_o"] != v["uf_d"] and v["cfop"].startswith("5")),
    ("R09", lambda v: v["modal"] == "AEREO" and ok(v["peso"]) and v["peso"] > 500),
    ("R10", lambda v: v["entrega"] == "ENTREGUE" and v["migo"] is False),
    ("R11", lambda v: v["manif"] == "ACTO" and v["migo"] is True),
    ("R12", lambda v: v["instrumento"] == "PPIX" and v["porte"] in ("ME", "EPP") and ok(v["valor"]) and v["valor"] <= 5000),
]


def todas(o):
    v = valores(o)
    return [cod for cod, pred in REGRAS if pred(v)]


def classificar(o):
    v = valores(o)
    return next((cod for cod, pred in REGRAS if pred(v)), SEM)


def id_de(o):
    v = o.get("id")
    if isinstance(v, (Int, Float)):
        return v.raw
    return None


def processar(linha_bytes, classificar=classificar):
    """Devolve (id, regra) ou None quando a linha não gera saída."""
    try:
        texto = linha_bytes.decode("utf-8")
    except UnicodeDecodeError:
        texto = None
    if texto is not None and not texto.strip():
        return None
    try:
        o = carregar(texto) if texto is not None else None
    except (ValueError, RecursionError):
        o = None
    if isinstance(o, dict):
        i = id_de(o)
        return (i, classificar(o)) if i is not None else None
    m = ID_CRU.search(linha_bytes.decode("utf-8", "replace"))
    return (m.group(1), SEM) if m else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--entrada", required=True)
    ap.add_argument("--todas", action="store_true", help="lista todas as regras satisfeitas, não só a primeira")
    a = ap.parse_args()
    with open(a.entrada, "rb") as f:
        dados = f.read()
    if dados.startswith(b"\xef\xbb\xbf"):
        dados = dados[3:]
    fn = (lambda o: ",".join(todas(o)) or SEM) if a.todas else classificar
    out = []
    for linha in dados.split(b"\n"):
        r = processar(linha, fn)
        if r:
            out.append(f"id:{r[0]} - regra: {r[1]}\n")
    sys.stdout.write("".join(out))


if __name__ == "__main__":
    main()
