#!/usr/bin/env bash
# Roda todos os testes do motor e diz o que passou. Uso: testes/rodar.sh [--docker]
# Precisa de: cargo, python3. Com --docker, também de docker.
set -uo pipefail
cd "$(dirname "$0")/.."

KIT=hackathon-2026-d1-kit
M=motor/target/release/motor
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
falhas=0

confere() {
    local nome=$1; shift
    if "$@" >"$TMP/saida" 2>&1; then
        printf '  ok     %s\n' "$nome"
    else
        printf '  FALHA  %s\n' "$nome"; sed 's/^/         /' "$TMP/saida" | tail -8
        falhas=$((falhas + 1))
    fi
}

# medir.py do kit com 100% de acerto
cem_porcento() { python3 $KIT/medir.py --repeticoes 1 "$@" | tee /dev/stderr | grep -q '(100.00%)'; }

# a saída do motor é igual à esperada, sem importar a ordem das linhas
mesma_saida() { diff <(sort "$1") <(sort "$2"); }

echo "== motor"
confere "cargo build" bash -c "cd motor && cargo build --release -q"
confere "cargo test (linguagem das regras e árvore esperada)" bash -c "cd motor && cargo test --release -q"

echo "== conjuntos do kit"
confere "pequeno: 100%" cem_porcento --set pequeno --cmd "$M --entrada {entrada}"
confere "grande: 100%" cem_porcento --set grande --cmd "$M --entrada {entrada}"

echo "== casos de borda (oráculo em Python × motor)"
if [ ! -f testes/bordas/gabarito.txt ]; then
    echo "  gerando testes/bordas (uns 30 s)..."
    python3 testes/gerar_bordas.py --saida testes/bordas/entrada.jsonl >/dev/null
    python3 testes/oraculo.py --entrada testes/bordas/entrada.jsonl > testes/bordas/gabarito.txt
fi
$M --entrada testes/manuais.jsonl > "$TMP/manuais"
confere "manuais: um caso por decisão de semântica" mesma_saida "$TMP/manuais" testes/manuais.gabarito.txt
confere "bordas: 100%" cem_porcento --entrada testes/bordas/entrada.jsonl --gabarito testes/bordas/gabarito.txt --cmd "$M --entrada {entrada}"
python3 testes/oraculo.py --entrada testes/bordas/entrada_utf8.jsonl > "$TMP/utf8.esperado"
$M --entrada testes/bordas/entrada_utf8.jsonl > "$TMP/utf8"
confere "UTF-8 inválido: igual ao oráculo" mesma_saida "$TMP/utf8" "$TMP/utf8.esperado"
python3 testes/oraculo.py --entrada testes/bordas/entrada.jsonl --todas > "$TMP/todas.esperado"
$M --entrada testes/bordas/entrada.jsonl --todas > "$TMP/todas"
confere "--todas: todas as regras satisfeitas, igual ao oráculo" mesma_saida "$TMP/todas" "$TMP/todas.esperado"

echo "== regras fora do código"
# inverte a ordem das regras no YAML: cada linha passa a ser da última regra que ela satisfazia
python3 - regras/regras.yaml > "$TMP/invertidas.yaml" <<'EOF'
import re, sys
cab, regras = open(sys.argv[1]).read().split("\nregras:\n")
blocos = [b.rstrip("\n") + "\n" for b in re.split(r"(?m)^(?=  - codigo:)", regras) if b.strip()]
print(cab + "\nregras:\n" + "".join(reversed(blocos)), end="")
EOF
sed -E 's/regra: (.*,)?([^,]+)$/regra: \2/' "$TMP/todas" > "$TMP/invertidas.esperado"
$M --regras "$TMP/invertidas.yaml" --entrada testes/bordas/entrada.jsonl > "$TMP/invertidas"
confere "regras em ordem inversa: vence a última satisfeita, sem recompilar" mesma_saida "$TMP/invertidas" "$TMP/invertidas.esperado"
$M --regras dominios/pedidos/regras.yaml --entrada dominios/pedidos/entrada.jsonl > "$TMP/pedidos"
confere "segundo domínio (pedidos): igual ao gabarito escrito à mão" mesma_saida "$TMP/pedidos" dominios/pedidos/gabarito.txt
sed 's/financeiro.valor ≥ 100000/financeiro.valr ≥ 100000/' regras/regras.yaml > "$TMP/erro.yaml"
recusa() {
    local msg codigo
    msg=$($M --regras "$TMP/erro.yaml" --entrada testes/manuais.jsonl 2>&1 >/dev/null)
    codigo=$?
    echo "saída $codigo: $msg"
    [ $codigo -eq 2 ] && grep -q "regra R06: campo não declarado: financeiro.valr" <<<"$msg"
}
confere "regra com campo inexistente: o motor recusa subir e aponta a regra" recusa

if [ "${1:-}" = "--docker" ]; then
    echo "== docker (como a banca roda)"
    confere "docker build" docker build -q -t motor-regras .
    D="docker run --rm --cpus=1 -v {dir}:/dados motor-regras --entrada /dados/{arquivo}"
    confere "pequeno no docker: 100%" cem_porcento --set pequeno --cmd "$D"
    confere "grande no docker: 100%" cem_porcento --set grande --cmd "$D"
fi

echo
if [ $falhas -eq 0 ]; then echo "tudo certo"; else echo "$falhas falha(s)"; exit 1; fi
