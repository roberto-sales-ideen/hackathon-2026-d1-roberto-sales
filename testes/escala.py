#!/usr/bin/env python3
"""Mede como o tempo do motor cresce com a quantidade de linhas e desenha o gráfico.

Gera entradas de 1 mil a 1,6 milhão de linhas repetindo os registros do conjunto grande (com ids
novos), mede o motor nativo e no Docker com --cpus=1 (mediana), ajusta t = a + b·linhas por
mínimos quadrados e grava:
  evidencias/escala.csv                       os números (a "tabela" do gráfico)
  evidencias/escala-claro.svg / -escuro.svg   o gráfico, para tema claro e escuro

Uso: python3 testes/escala.py            mede tudo (uns 3 min; precisa da imagem motor-regras)
     python3 testes/escala.py --grafico  só redesenha a partir do CSV
"""
import argparse, csv, gzip, math, os, shutil, statistics, subprocess, tempfile, time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.join(AQUI, "..")
GRANDE = os.path.join(RAIZ, "hackathon-2026-d1-kit", "dados", "grande", "entrada.jsonl.gz")
MOTOR = os.path.join(RAIZ, "motor", "target", "release", "motor")
EVID = os.path.join(RAIZ, "evidencias")
TAMANHOS = [1_000, 3_000, 10_000, 30_000, 100_000, 200_000, 400_000, 800_000, 1_600_000]


# ---------------------------------------------------------------- medição

def gerar(caminho, n, linhas):
    with open(caminho, "wb") as f:
        for i in range(n):
            f.write(b'{"id":%d,' % (i + 1) + linhas[i % len(linhas)])


def cronometrar(cmd, vezes):
    ts = []
    for _ in range(vezes):
        t = time.perf_counter()
        subprocess.run(cmd, stdout=subprocess.DEVNULL, check=True)
        ts.append(time.perf_counter() - t)
    return statistics.median(ts)


def medir():
    with gzip.open(GRANDE, "rb") as f:
        # cada linha começa com {"id":N, — guarda só o resto, para trocar o id
        linhas = [l[l.index(b",") + 1:] for l in f]
    tmp = tempfile.mkdtemp(prefix="escala-")
    resultados = []
    try:
        for n in TAMANHOS:
            arq = os.path.join(tmp, "entrada.jsonl")
            gerar(arq, n, linhas)
            saida = subprocess.run([MOTOR, "--entrada", arq], capture_output=True, check=True).stdout
            assert saida.count(b"\n") == n, f"{n} linhas, {saida.count(b'\n')} saídas"
            nativo = cronometrar([MOTOR, "--entrada", arq], 5)
            docker = cronometrar(["docker", "run", "--rm", "--cpus=1", "-v", f"{tmp}:/dados", "motor-regras",
                                  "--entrada", "/dados/entrada.jsonl"], 3)
            resultados.append({"linhas": n, "bytes": os.path.getsize(arq), "nativo_s": round(nativo, 4),
                               "docker_s": round(docker, 4)})
            print(f"{n:>9} linhas  nativo {nativo:7.3f} s  docker {docker:7.3f} s", flush=True)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    with open(os.path.join(EVID, "escala.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(resultados[0]))
        w.writeheader()
        w.writerows(resultados)


def ajuste(xs, ys):
    """Mínimos quadrados de y = a + b·x; devolve a, b e R²."""
    mx, my = statistics.fmean(xs), statistics.fmean(ys)
    b = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum((x - mx) ** 2 for x in xs)
    a = my - b * mx
    r2 = 1 - sum((y - a - b * x) ** 2 for x, y in zip(xs, ys)) / sum((y - my) ** 2 for y in ys)
    return a, b, r2


# ---------------------------------------------------------------- gráfico

TEMAS = {
    "claro": {"fundo": "#fcfcfb", "tinta": "#0b0b0b", "tinta2": "#52514e", "muda": "#898781",
              "grade": "#e1e0d9", "eixo": "#c3c2b7", "s1": "#2a78d6", "s2": "#eb6834"},
    "escuro": {"fundo": "#1a1a19", "tinta": "#f0efec", "tinta2": "#c3c2b7", "muda": "#898781",
               "grade": "#2c2c2a", "eixo": "#383835", "s1": "#3987e5", "s2": "#d95926"},
}


def br(v, casas):
    return f"{v:,.{casas}f}".replace(",", "X").replace(".", ",").replace("X", ".")


def linhas_curto(n):
    if n >= 1_000_000:
        return br(n / 1_000_000, 1 if n % 1_000_000 else 0) + " mi"
    if n >= 1_000:
        return br(n / 1_000, 0) + " mil"
    return str(n)


def passo_bonito(maximo, alvo=5):
    bruto = maximo / alvo
    mag = 10 ** math.floor(math.log10(bruto))
    for m in (1, 2, 2.5, 5, 10):
        if bruto <= m * mag:
            return m * mag


def svg(dados, tema):
    c = TEMAS[tema]
    W, H = 960, 470
    xs = [d["linhas"] for d in dados]
    series = [("Nativo (Apple M1 Pro)", "nativo_s", c["s1"]), ("Docker, --cpus=1", "docker_s", c["s2"])]
    fontes = 'font-family="system-ui, -apple-system, Segoe UI, sans-serif"'
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" {fontes}>',
         f'<rect width="{W}" height="{H}" fill="{c["fundo"]}"/>',
         f'<text x="40" y="38" font-size="18" font-weight="600" fill="{c["tinta"]}">'
         f'Tempo do motor por quantidade de duplicatas</text>',
         f'<text x="40" y="60" font-size="13" fill="{c["tinta2"]}">Mediana de 5 execuções (nativo) e 3 (Docker); '
         f'entradas montadas repetindo o conjunto grande com ids novos</text>']
    # legenda com o ajuste de cada série
    for k, (nome, chave, cor) in enumerate(series):
        a, b, r2 = ajuste(xs, [d[chave] for d in dados])
        y = 90 + k * 22
        o.append(f'<line x1="40" y1="{y - 4}" x2="60" y2="{y - 4}" stroke="{cor}" stroke-width="2" stroke-linecap="round"/>')
        o.append(f'<circle cx="50" cy="{y - 4}" r="4" fill="{cor}" stroke="{c["fundo"]}" stroke-width="2"/>')
        o.append(f'<text x="68" y="{y}" font-size="13" fill="{c["tinta"]}">{nome}: '
                 f'<tspan fill="{c["tinta2"]}">t = {br(a, 3)} s + {br(b * 1e6, 2)} µs × linhas '
                 f'(R² = {br(r2, 4)})</tspan></text>')
    topo, base = 150, 400

    def painel(x0, x1, titulo, xmap, xticks, ymax, ypasso, yfmt, pontos_de, reta):
        o.append(f'<text x="{x0}" y="{topo - 14}" font-size="13" font-weight="600" fill="{c["tinta"]}">{titulo}</text>')
        ymap = lambda v: base - (base - topo) * v / ymax
        v = 0
        while v <= ymax + 1e-9:
            y = ymap(v)
            o.append(f'<line x1="{x0}" y1="{y:.1f}" x2="{x1}" y2="{y:.1f}" stroke="{c["grade"] if v else c["eixo"]}" stroke-width="1"/>')
            o.append(f'<text x="{x0 - 8}" y="{y + 4:.1f}" font-size="11" text-anchor="end" fill="{c["muda"]}" '
                     f'style="font-variant-numeric: tabular-nums">{yfmt(v)}</text>')
            v += ypasso
        for t in xticks:
            x = xmap(t)
            o.append(f'<line x1="{x:.1f}" y1="{base}" x2="{x:.1f}" y2="{base + 4}" stroke="{c["eixo"]}" stroke-width="1"/>')
            o.append(f'<text x="{x:.1f}" y="{base + 18}" font-size="11" text-anchor="middle" fill="{c["muda"]}">{linhas_curto(t)}</text>')
        o.append(f'<text x="{(x0 + x1) / 2}" y="{base + 38}" font-size="12" text-anchor="middle" fill="{c["tinta2"]}">linhas na entrada</text>')
        for nome, chave, cor in series:
            pts = [(xmap(d["linhas"]), ymap(pontos_de(d, chave))) for d in dados]
            if reta:
                a, b, _ = ajuste(xs, [d[chave] for d in dados])
                xa, xb = 0, max(xs)
                o.append(f'<line x1="{xmap(xa):.1f}" y1="{ymap(a + b * xa):.1f}" x2="{xmap(xb):.1f}" y2="{ymap(a + b * xb):.1f}" '
                         f'stroke="{cor}" stroke-width="2" stroke-linecap="round"/>')
            else:
                caminho = " ".join(f"{'M' if i == 0 else 'L'}{x:.1f},{y:.1f}" for i, (x, y) in enumerate(pts))
                o.append(f'<path d="{caminho}" fill="none" stroke="{cor}" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>')
            for x, y in pts:
                o.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="4" fill="{cor}" stroke="{c["fundo"]}" stroke-width="2"/>')

    # painel A: tempo × linhas, eixos lineares, reta ajustada
    tmax = max(d["docker_s"] for d in dados)
    pa = passo_bonito(tmax, 5)
    ya = math.ceil(tmax / pa) * pa
    xa0, xa1 = 90, 450
    xmax = max(xs)
    painel(xa0, xa1, "Tempo total (s): pontos medidos e reta ajustada", lambda n: xa0 + (xa1 - xa0) * n / xmax,
           [0, 400_000, 800_000, 1_200_000, 1_600_000], ya, pa, lambda v: br(v, 1 if pa < 1 else 0),
           lambda d, k: d[k], True)
    # painel B: vazão × linhas, eixo x em log
    vmax = max(d["linhas"] / d["nativo_s"] for d in dados)
    pb = passo_bonito(vmax / 1e3, 5) * 1e3
    yb = math.ceil(vmax / pb) * pb
    xb0, xb1 = 590, 930
    lmin, lmax = math.log10(1_000), math.log10(xmax)
    painel(xb0, xb1, "Vazão (mil linhas por segundo); eixo x em escala log",
           lambda n: xb0 + (xb1 - xb0) * (math.log10(n) - lmin) / (lmax - lmin),
           [1_000, 10_000, 100_000, 1_000_000], yb, pb, lambda v: br(v / 1e3, 0),
           lambda d, k: d["linhas"] / d[k], False)
    o.append("</svg>")
    return "\n".join(o)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--grafico", action="store_true", help="só redesenha a partir de evidencias/escala.csv")
    a = ap.parse_args()
    if not a.grafico:
        medir()
    with open(os.path.join(EVID, "escala.csv")) as f:
        dados = [{k: float(v) if "." in v else int(v) for k, v in r.items()} for r in csv.DictReader(f)]
    for tema in TEMAS:
        with open(os.path.join(EVID, f"escala-{tema}.svg"), "w") as f:
            f.write(svg(dados, tema))
    for nome, chave in (("nativo", "nativo_s"), ("docker", "docker_s")):
        a_, b_, r2 = ajuste([d["linhas"] for d in dados], [d[chave] for d in dados])
        print(f"{nome}: t = {a_:.4f} s + {b_ * 1e6:.3f} µs/linha, R² = {r2:.5f}")


if __name__ == "__main__":
    main()
