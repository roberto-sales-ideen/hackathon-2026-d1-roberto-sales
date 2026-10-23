# Kit do Desafio 1 · Motor de regras

1. Leia o `ENUNCIADO.md`. Tudo que você precisa saber está lá.
2. Construa o seu motor: qualquer linguagem, bibliotecas permitidas, regras fora do código.
3. Valide com o script de medição:

```bash
python medir.py --set pequeno --cmd "python meu_motor.py --entrada {entrada}"
python medir.py --set grande  --cmd "python meu_motor.py --entrada {entrada}"
```

Com Docker, limitando a 1 CPU como a banca fará (no macOS, funciona com OrbStack ou Docker Desktop):

```bash
docker build -t meu-motor .
python medir.py --set grande --cmd "docker run --rm --cpus=1 -v {dir}:/dados meu-motor --entrada /dados/{arquivo}"
```

O script entrega a entrada descompactada, compara a saída com o gabarito, mostra os primeiros erros e o tempo de cada execução.

## O que entregar no seu repositório (`ideen-tech/hackathon-2026-d1-[seu-usuário]`)

| Caminho | O que é |
|---|---|
| código do motor | onde quiser |
| regras traduzidas | fora do código, no formato que escolher |
| `Dockerfile` | roda o motor com `--entrada <arquivo>`; sem ele, o motor não entra no ranking |
| `evidencias/` | a saída do `medir.py` nos dois conjuntos |
| `README.md` | o problema, o que você fez, como reproduzir, evidências, onde a IA errou |

Prazo: **segunda, 26/10, às 9h**, quando os repositórios são arquivados.

Lembrete: continuar no fim de semana é opcional, e horas do hackathon no fim de semana não são lançadas.
