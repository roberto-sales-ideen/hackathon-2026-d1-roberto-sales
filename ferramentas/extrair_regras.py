#!/usr/bin/env python3
"""Gera o manifesto de regras (YAML) a partir das tabelas do ENUNCIADO.md, sem digitação.

Lê duas tabelas do enunciado:
  - "Modelo da duplicata": vira a seção `campos:` (nome do campo e tipo)
  - "Regras": vira a lista `regras:`; a coluna Condição entra quase literal em `quando:`,
    só sem a formatação markdown (crases e negrito)

O resultado é o ponto de partida. Depois de gerado, o regras.yaml é a fonte da verdade e pode
ser editado à mão.

Uso: python3 ferramentas/extrair_regras.py > regras/regras.yaml
"""
import argparse, os, re

AQUI = os.path.dirname(os.path.abspath(__file__))
PADRAO = os.path.join(AQUI, "..", "hackathon-2026-d1-kit", "ENUNCIADO.md")
TIPOS = {"número": "numero", "texto": "texto", "data": "data", "booleano": "booleano"}

CABECALHO = """\
# Regras do Desafio 1, geradas de ENUNCIADO.md por ferramentas/extrair_regras.py.
#
# A ordem da lista é a ordem de avaliação: a primeira regra satisfeita classifica.
#
# Linguagem do `quando`, a mesma notação da tabela do enunciado:
#   E, OU                     operadores lógicos (maiúsculas); E vem antes de OU; ( ) agrupa
#   = ≠ < ≤ > ≥               comparação (também != <= >=)
#   é A, B ou C               pertence à lista ("ou" minúsculo separa itens)
#   está vazio                ausente, nulo ou texto vazio
#   começa com X              prefixo de texto
#   é verdadeiro / é falso    booleano do JSON
# Uma palavra declarada em `campos:` é campo; uma declarada em `constantes:` é constante; o resto
# é literal (aspas forçam literal). Literal e constante assumem o tipo do campo comparado.
"""


def extrair(md):
    campos = {}
    for linha in md.splitlines():
        m = re.match(r"\| (`[^|]+`) \| ([^|]+?) \|", linha)
        if m:
            tipo = TIPOS[m.group(2).replace(" ou nulo", "").strip()]
            for c in re.findall(r"`([^`]+)`", m.group(1)):
                campos[c] = tipo
    ref = re.search(r"Data de referência[^*]*\*\*(\d{4}-\d{2}-\d{2})\*\*", md).group(1)
    regras = []
    for cod, cat, cond in re.findall(r"^\| (R\d+) \| ([^|]+?) \| ([^|]+?) \|$", md, re.M):
        cond = cond.replace("**", "").replace("`", "").replace("data de referência", "data_referencia")
        regras.append((cod, cat, cond))
    return campos, ref, regras


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--enunciado", default=PADRAO)
    a = ap.parse_args()
    with open(a.enunciado, encoding="utf-8") as f:
        campos, ref, regras = extrair(f.read())
    out = [CABECALHO, "campo_id: id", "sem_regra: SEM_REGRA", "", "constantes:",
           f'  data_referencia: "{ref}"', "", "campos:"]
    out += [f"  {c}: {t}" for c, t in campos.items()]
    out += ["", "regras:"]
    for cod, cat, cond in regras:
        out += [f"  - codigo: {cod}", f"    categoria: {cat}", f"    quando: {cond}"]
    print("\n".join(out))


if __name__ == "__main__":
    main()
