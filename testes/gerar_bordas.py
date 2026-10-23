#!/usr/bin/env python3
"""Gera casos de borda a partir de registros limpos do kit.

Duas famílias de caso:
  1. sistemáticos: cada mutação do catálogo aplicada, sozinha, a cada registro do conjunto pequeno
  2. combinados: de 2 a 6 mutações sorteadas sobre registros amostrados do grande

O gabarito não sai daqui: quem classifica é o oráculo. Este script só fabrica entrada.

Uso: python3 gerar_bordas.py --saida bordas.jsonl [--combos 200000] [--semente 7]
"""
import argparse, gzip, json, os, random

AQUI = os.path.dirname(os.path.abspath(__file__))
DADOS = os.path.join(AQUI, "..", "hackathon-2026-d1-kit", "dados")


class Raw:
    """Trecho de JSON escrito exatamente como está (número cru, string com escape, lixo)."""
    def __init__(self, s): self.s = s


class Obj:
    """Objeto JSON como lista de pares, para permitir chave duplicada e ordem controlada."""
    def __init__(self, pares): self.pares = pares

    def achar(self, k):
        for i in range(len(self.pares) - 1, -1, -1):
            if self.pares[i][0] == k:
                return i
        return -1

    def get(self, k):
        i = self.achar(k)
        return self.pares[i][1] if i >= 0 else None

    def set(self, k, v):
        i = self.achar(k)
        if i >= 0: self.pares[i] = (k, v)
        else: self.pares.append((k, v))

    def remover(self, k):
        self.pares = [p for p in self.pares if p[0] != k]


def para_obj(v):
    if isinstance(v, dict): return Obj([(k, para_obj(x)) for k, x in v.items()])
    if isinstance(v, list): return [para_obj(x) for x in v]
    return v


AUSENTE = object()


def ser(v, est):
    if isinstance(v, Raw): return v.s
    if isinstance(v, Obj):
        partes = [(k.s if isinstance(k, Raw) else json.dumps(k, ensure_ascii=False)) + est["dp"] + ser(x, est)
                  for k, x in v.pares]
        return "{" + est["ab"] + est["vg"].join(partes) + est["fc"] + "}"
    if isinstance(v, list): return "[" + est["vg"].join(ser(x, est) for x in v) + "]"
    return json.dumps(v, ensure_ascii=False)


COMPACTO = {"dp": ":", "vg": ",", "ab": "", "fc": ""}
ESTILOS = [COMPACTO,
           {"dp": ": ", "vg": ", ", "ab": "", "fc": ""},
           {"dp": " : ", "vg": " , ", "ab": " ", "fc": " "},
           {"dp": ":\t", "vg": ",\t", "ab": "\t", "fc": ""},
           {"dp": ": ", "vg": ",  ", "ab": "  ", "fc": "\t "}]


def pai_e_chave(o, caminho):
    partes = caminho.split(".")
    for p in partes[:-1]:
        o = o.get(p) if isinstance(o, Obj) else None
    return (o if isinstance(o, Obj) else None), partes[-1]


def atribuir(o, caminho, v):
    pai, k = pai_e_chave(o, caminho)
    if pai is None: return
    if v is AUSENTE: pai.remover(k)
    else: pai.set(k, v)


def ler(o, caminho):
    pai, k = pai_e_chave(o, caminho)
    return pai.get(k) if pai is not None else None


# ---------------------------------------------------------------- catálogo de valores

LIXO = [AUSENTE, None, "", [], Obj([]), True, False, 0, 1, "null", Raw("[1,2]")]


def textos(dominio):
    extra = []
    for d in dominio:
        extra += [d.lower(), d.capitalize(), d + " ", " " + d, d[:-1]]
    escapado = Raw('"\\u%04X%s"' % (ord(dominio[0][0]), dominio[0][1:]))
    escapado_meio = Raw('"%s\\u%04x%s"' % (dominio[-1][0], ord(dominio[-1][1]), dominio[-1][2:]))
    return dominio * 3 + extra + LIXO + [escapado, escapado_meio, 7, Raw("1e3")]


NUMS_VALOR = [5000, 5000.0, 4999.99, 5000.01, Raw("5e3"), Raw("5E+3"), Raw("5000.000"),
              100000, 99999.99, 100000.0, 100000.01, Raw("1e5"), Raw("1E5"), Raw("1.0e5"),
              200000, 199999.99, 200000.0, Raw("2e5"), 500000, 499999.99, 500000.0, 500000.01,
              Raw("5e5"), Raw("499999.99999999999999"), Raw("4999.999999999999999"),
              0, 0.0, -1, Raw("-0.0"), Raw("-0"), 1234.5, 750000.0, 250000.0, 150000.0, 3000.0,
              Raw("1e400"), Raw("-1e400"), Raw("NaN"), Raw("Infinity"), Raw("-Infinity"),
              "15000.00", "500000", "1e5", "5000"] + LIXO

NUMS_PESO = [500, 500.0, 500.01, 499.99, 501, Raw("5e2"), Raw("5.00000000000000001e2"), 0, 1000.0,
             Raw("1e3"), "600", "500.5", Raw("NaN"), Raw("Infinity")] + LIXO

DATAS = ["2026-10-22", "2026-10-23", "2026-10-24", "2026-09-30", "2027-01-15", "2024-02-29",
         "2025-02-29", "2026-02-30", "2026-04-31", "2026-13-01", "2026-00-10", "2026-10-00",
         "2026-1-05", "2026-10-5", "26-10-01", "2026/10/01", "2026.10.01", "2026-10-01T00:00:00",
         "2026-10-01 ", " 2026-10-01", "20261001", 20261001, "0000-01-01", "0001-01-01",
         "9999-12-31", "٢٠٢٦-١٠-٠١", "2026-10-0１",
         "+2026-10-01", "2026-10-01Z", Raw('"2026\\u002d10\\u002d01"'), "2026-W40-1"] + LIXO

CFOPS = ["5102", "5101", "5405", "6102", "6108", "5", "6", "", " 5102", "5102 ", "05102",
         "5102", Raw('"\\u0035102"'), 5102, 6102, 5, Raw("5.102e3")] + LIXO

UFS = ["SP", "RJ", "MG", "sp", "Sp", "SP ", "", " ", 35, Raw('"\\u0053P"')] + LIXO

BOOLS = [True, False, True, False, "false", "true", "False", "True", 0, 1, None, AUSENTE,
         "", Raw("[]"), Raw("{}")]

CAMPOS = {
    "status": textos(["ATIVA", "LIQUIDADA", "CANCELADA"]),
    "logistica.status_entrega": textos(["ENTREGUE", "EM_TRANSITO", "DEVOLVIDA", "AVARIADA"]),
    "manifestacao.tipo": textos(["ACTO", "RCAV", "RCQQ", "RCVN", "RCVL", "RCOP", "ANTC"]),
    "data_vencimento": DATAS,
    "financeiro.valor": NUMS_VALOR,
    "financeiro.valor_pago": NUMS_VALOR,
    "financeiro.tipo_negociacao": textos(["CSCO", "CSAO", "CSFD", "PENR", "PRCS"]),
    "sacado.segmento": textos(["VAREJO", "INDUSTRIA", "AGRO", "SERVICOS", "SAUDE", "ATACADO"]),
    "logistica.uf_origem": UFS,
    "logistica.uf_destino": UFS,
    "fiscal.cfop": CFOPS,
    "logistica.modal": textos(["RODOVIARIO", "AEREO", "FERROVIARIO", "AQUAVIARIO"]),
    "logistica.peso_kg": NUMS_PESO,
    "logistica.migo": BOOLS,
    "pagamento.instrumento": textos(["TRCR", "PPIX", "BOLETO"]),
    "sacador.porte": textos(["ME", "EPP", "MEDIA", "GRANDE"]),
}
PAIS = ["logistica", "manifestacao", "financeiro", "sacado", "sacador", "fiscal", "pagamento"]
VALORES_PAI = [AUSENTE, None, [], Obj([]), "x", 0, True, Raw('[{"tipo":"RCOP"}]')]


# ---------------------------------------------------------------- mutações estruturais

def m_valor_pago_relativo(o, r):
    v = ler(o, "financeiro.valor")
    if isinstance(v, (int, float)) and not isinstance(v, bool):
        atribuir(o, "financeiro.valor_pago", r.choice([v, v - 0.01, v + 0.01, 0.0, v / 2]))


def m_uf_iguais(o, r):
    atribuir(o, "logistica.uf_destino", ler(o, "logistica.uf_origem"))


def m_dup_chave(o, r):
    """Prefixa uma cópia da chave com outro valor: o valor que fica no fim é o que vale."""
    caminho = r.choice(list(CAMPOS))
    pai, k = pai_e_chave(o, caminho)
    if pai is None: return
    isca = r.choice(CAMPOS[caminho])
    if isca is AUSENTE: return
    if r.random() < 0.5:
        pai.pares.insert(0, (k, isca))                 # a isca vem antes e perde
    else:
        existia, atual = pai.achar(k) >= 0, pai.get(k)
        pai.remover(k)
        if existia:
            pai.pares.insert(0, (k, atual))
        pai.pares.append((k, isca))                    # a isca vem depois e vence


def m_dup_pai(o, r):
    """Objeto pai repetido: vale o último inteiro, sem mesclar com o primeiro."""
    p = r.choice(PAIS)
    atual = o.get(p)
    parcial = r.choice([
        Obj([("status_entrega", "DEVOLVIDA")]), Obj([("tipo", "RCOP")]), Obj([("valor", 600000.0)]),
        Obj([("tipo", None)]), Obj([]), Obj([("segmento", "AGRO")]), Obj([("porte", "ME")]),
        Obj([("cfop", "5102")]), Obj([("instrumento", "PPIX")]), Obj([("migo", False)])])
    if r.random() < 0.5:
        o.pares.insert(0, (p, parcial))               # parcial antes: vale o original
    else:
        o.remover(p)
        o.pares.insert(0, (p, atual))                 # original antes: vale só o parcial
        o.pares.append((p, parcial))


def m_chave_escapada(o, r):
    caminho = r.choice(list(CAMPOS))
    pai, k = pai_e_chave(o, caminho)
    if pai is None: return
    i = pai.achar(k)
    if i < 0: return
    j = r.randrange(len(k))
    esc = k[:j] + "\\u%04x" % ord(k[j]) + k[j + 1:]
    pai.pares[i] = (Raw('"%s"' % esc), pai.pares[i][1])


def m_chave_parecida(o, r):
    """Chave que só parece a nossa: maiúscula, espaço, prefixo. Não deve ser lida."""
    caminho = r.choice(list(CAMPOS))
    pai, k = pai_e_chave(o, caminho)
    if pai is None: return
    i = pai.achar(k)
    if i < 0: return
    nova = r.choice([k.upper(), k + " ", " " + k, k + "_", k[:-1]])
    pai.pares[i] = (nova, pai.pares[i][1])


def m_isca_aninhada(o, r):
    """Campos com o nosso nome em lugares que não são os nossos."""
    isca = r.choice([
        ("extra", Obj([("status", "CANCELADA"), ("logistica", Obj([("migo", False)]))])),
        ("historico", [Obj([("status", "LIQUIDADA")]), Obj([("tipo", "RCOP")])]),
        ("status_anterior", "CANCELADA"),
        ("observacao", 'texto com "status":"CANCELADA" dentro'),
        ("observacao", Raw('"aspas \\" e barra \\\\"')),
        ("vazio_final", Raw('"\\\\"')),
    ])
    destino = o if r.random() < 0.5 else o.get(r.choice(PAIS))
    if isinstance(destino, Obj):
        destino.pares.insert(r.randrange(len(destino.pares) + 1), isca)
    if r.random() < 0.3:
        lg = o.get("logistica")
        if isinstance(lg, Obj):
            lg.pares.insert(0, ("detalhe", Obj([("migo", True), ("uf_origem", "XX"), ("modal", "AEREO")])))


def m_texto_com_escape(o, r):
    """Escapes em campos que o motor pula (o pulo precisa respeitar \\" e \\\\)."""
    for k in ("iud", "numero"):
        if o.achar(k) >= 0:
            o.set(k, Raw(r.choice(['"12\\"34"', '"abc\\\\"', '"\\ud83d\\ude00"', '"\\ud800"',
                                   '"\\n\\t\\r\\b\\f\\/"', '"\\\\\\""', '"%s"' % ("x" * 5000)])))


def m_surrogate_solto(o, r):
    """Surrogate solto num campo comparado: o Python aceita; o motor não pode rejeitar a linha."""
    atribuir(o, r.choice(["sacado.segmento", "status", "manifestacao.tipo"]), Raw('"AGRO\\udc00"'))


def m_embaralhar(o, r):
    def emb(v):
        if isinstance(v, Obj):
            r.shuffle(v.pares)
            for _, x in v.pares: emb(x)
    emb(o)


def m_aninhar_fundo(o, r):
    v = 1
    for _ in range(r.choice([10, 60, 200])):
        v = Obj([("n", v)]) if r.random() < 0.5 else [v]
    o.set("profundo", v)


ESTRUTURAIS = [m_valor_pago_relativo, m_uf_iguais, m_dup_chave, m_dup_pai, m_chave_escapada,
               m_chave_parecida, m_isca_aninhada, m_texto_com_escape, m_surrogate_solto,
               m_embaralhar, m_aninhar_fundo]


# ---------------------------------------------------------------- linhas inteiras

def quebrar(texto, r):
    """Transforma uma linha válida em JSON inválido, mantendo o id recuperável quando possível."""
    op = r.randrange(9)
    if op == 0: return texto[:r.randrange(10, max(11, len(texto) - 1))]
    if op == 1: return texto[:-1] + ",}"
    if op == 2: return texto + " lixo"
    if op == 3: return texto + texto
    if op == 4: return texto.replace('"status"', "'status'", 1)
    if op == 5: return texto.replace('"status"', "status", 1)
    if op == 6: return texto[:-1]
    if op == 7: return texto.replace(":", "::", 1)
    return texto.replace('"ATIVA"', '"AT\x01IVA"', 1)


def ler_base(n_grande, r):
    with open(os.path.join(DADOS, "pequeno", "entrada.jsonl")) as f:
        pequeno = [json.loads(l) for l in f if l.strip()]
    grande = []
    with gzip.open(os.path.join(DADOS, "grande", "entrada.jsonl.gz"), "rt") as f:
        for i, l in enumerate(f):
            if i % (200000 // n_grande) == 0:
                grande.append(json.loads(l))
    return pequeno, grande


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--saida", required=True)
    ap.add_argument("--combos", type=int, default=200000)
    ap.add_argument("--semente", type=int, default=7)
    a = ap.parse_args()
    r = random.Random(a.semente)
    pequeno, grande = ler_base(4000, r)
    prox = [0]

    def emitir(base, mutar, f, quebrada=False):
        prox[0] += 1
        o = para_obj(base)
        o.set("id", prox[0])
        mutar(o)
        if o.get("id") != prox[0]:                     # alguma mutação mexeu no id: devolve
            o.set("id", prox[0])
        est = r.choice(ESTILOS) if r.random() < 0.3 else COMPACTO
        texto = ser(o, est)
        if quebrada: texto = quebrar(texto, r)
        fim = r.choice(["\n", "\n", "\n", "\r\n", " \n", "\t\n"])
        f.write(texto + fim)
        if r.random() < 0.01: f.write(r.choice(["\n", "   \n", "\t\r\n"]))

    def atrib(c, v):
        return lambda o: atribuir(o, c, v)

    def pai(p, v):
        return lambda o: (o.remover(p) if v is AUSENTE else o.set(p, v))

    def struct(fn):
        return lambda o: fn(o, r)

    def unicos(vs):
        vistos, out = set(), []
        for v in vs:
            chave = ("raw", v.s) if isinstance(v, Raw) else (type(v).__name__, repr(v) if not isinstance(v, Obj) else id(v))
            if chave not in vistos:
                vistos.add(chave); out.append(v)
        return out

    sistematicas = [atrib(c, v) for c, vs in CAMPOS.items() for v in unicos(vs)]
    sistematicas += [pai(p, v) for p in PAIS for v in VALORES_PAI]
    sistematicas += [struct(fn) for fn in ESTRUTURAIS for _ in range(5)]

    with open(a.saida, "w", encoding="utf-8", newline="", errors="surrogatepass") as f:
        f.write("﻿")                              # BOM no início do arquivo
        for mut in sistematicas:
            for base in pequeno:
                emitir(base, mut, f)
        for _ in range(a.combos):
            base = r.choice(grande if r.random() < 0.8 else pequeno)
            muts = []
            for _ in range(r.randint(2, 6)):
                if r.random() < 0.8:
                    c = r.choice(list(CAMPOS)); muts.append(atrib(c, r.choice(CAMPOS[c])))
                elif r.random() < 0.5:
                    muts.append(pai(r.choice(PAIS), r.choice(VALORES_PAI)))
                else:
                    muts.append(struct(r.choice(ESTRUTURAIS)))
            emitir(base, lambda o: [m(o) for m in muts], f, quebrada=r.random() < 0.01)
        f.write('{"sem_id": true}\n[{"id": 999999999}]\nnull\n"texto"\n{"id": 999999998')
    # UTF-8 inválido vai num arquivo à parte: o medir.py lê a entrada como texto UTF-8 e quebraria
    with open(a.saida.replace(".jsonl", "_utf8.jsonl"), "wb") as f:
        modelo = ser(para_obj(dict(pequeno[0], status="CANCELADA")), COMPACTO).encode()
        for bruto in [b"\xff", b"\xc0\xaf", b"\xed\xa0\x80", b"\xc3", b"\xe2\x82", b"\xf4\x90\x80\x80", b"\xc3\xa9"]:
            for alvo in [b'"iud":"', b'"status":"CANCELADA', b'"status":']:
                prox[0] += 1
                linha = modelo.replace(b'"id":%d' % pequeno[0]["id"], b'"id":%d' % prox[0], 1)
                f.write(linha.replace(alvo, alvo + bruto, 1) + b"\n")
    print(f"{prox[0]} linhas em {a.saida}")


if __name__ == "__main__":
    main()
