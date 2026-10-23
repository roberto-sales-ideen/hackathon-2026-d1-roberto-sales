#!/usr/bin/env python3
"""Roda o seu motor, compara com o gabarito e mede o tempo total do comando.

Exemplos:
  python medir.py --set pequeno --cmd "python meu_motor.py --entrada {entrada}"
  python medir.py --set grande  --cmd "docker run --rm --cpus=1 -v {dir}:/dados meu-motor --entrada /dados/{arquivo}"

{entrada} = caminho completo do arquivo de entrada; {dir} = pasta dele; {arquivo} = nome do arquivo.
A entrada é sempre entregue descompactada (.jsonl). O motor escreve a saída no stdout.
"""
import argparse, gzip, os, re, shlex, shutil, statistics, subprocess, sys, tempfile, time

LINHA = re.compile(r"^\s*id\s*:\s*(\S+)\s*-\s*regra\s*:\s*(\S+)\s*$")
AQUI = os.path.dirname(os.path.abspath(__file__))

def abrir(p): return gzip.open(p, "rt", encoding="utf-8") if p.endswith(".gz") else open(p, encoding="utf-8")

def ler_resultado(linhas):
    out, invalidas = {}, 0
    for l in linhas:
        if not l.strip(): continue
        m = LINHA.match(l)
        if m: out[m.group(1)] = m.group(2)
        else: invalidas += 1
    return out, invalidas

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cmd", required=True, help="comando do seu motor, com {entrada} ou {dir}/{arquivo}")
    ap.add_argument("--set", choices=["pequeno", "grande"], help="conjunto do kit")
    ap.add_argument("--entrada", help="arquivo de entrada (uso da banca)")
    ap.add_argument("--gabarito", help="arquivo de gabarito (uso da banca)")
    ap.add_argument("--repeticoes", type=int, default=3)
    a = ap.parse_args()
    if a.set:
        base = os.path.join(AQUI, "dados", a.set)
        entrada = next(os.path.join(base, f) for f in os.listdir(base) if f.startswith("entrada"))
        gabarito = next(os.path.join(base, f) for f in os.listdir(base) if f.startswith("gabarito"))
    elif a.entrada and a.gabarito:
        entrada, gabarito = a.entrada, a.gabarito
    else:
        sys.exit("informe --set ou --entrada e --gabarito")

    tmp = tempfile.mkdtemp(prefix="motor-")
    arq = "entrada.jsonl"; caminho = os.path.join(tmp, arq)
    with abrir(entrada) as fi, open(caminho, "w", encoding="utf-8") as fo: shutil.copyfileobj(fi, fo)
    with abrir(gabarito) as f: gab, _ = ler_resultado(f)
    cmd = a.cmd.format(entrada=caminho, dir=tmp, arquivo=arq)

    tempos, saida = [], None
    for i in range(a.repeticoes):
        t0 = time.perf_counter()
        p = subprocess.run(cmd, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        tempos.append(time.perf_counter() - t0)
        if p.returncode != 0:
            print(p.stderr[-2000:], file=sys.stderr); sys.exit(f"o comando falhou (código {p.returncode}) na execução {i+1}")
        saida = p.stdout
    shutil.rmtree(tmp, ignore_errors=True)

    res, invalidas = ler_resultado(saida.splitlines())
    acertos = sum(1 for k, v in gab.items() if res.get(k) == v)
    faltando = sum(1 for k in gab if k not in res)
    erros = [(k, gab[k], res.get(k, "(ausente)")) for k in gab if res.get(k) != gab[k]][:10]
    print(f"casos:            {len(gab)}")
    print(f"acertos:          {acertos} ({100*acertos/len(gab):.2f}%)")
    print(f"ids ausentes:     {faltando}")
    print(f"linhas inválidas: {invalidas}")
    print(f"tempos (s):       {', '.join(f'{t:.3f}' for t in tempos)}")
    print(f"mediana (s):      {statistics.median(tempos):.3f}")
    if erros:
        print("primeiros erros (id, esperado, obtido):")
        for e in erros: print("  ", *e)

if __name__ == "__main__": main()
