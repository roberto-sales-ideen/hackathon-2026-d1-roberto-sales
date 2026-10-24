# Onde a IA acelerou e onde errou

Registro corrido, para a seção do README. Cada item: o que aconteceu, como foi pego.

## Onde acelerou

- **Levantamento dos dados antes de codar.** Em minutos a IA mostrou que os dois conjuntos do kit
  são 100% limpos (nenhum caso da seção "Semântica"), o que redirecionou o esforço para gerar as
  próprias bordas.
- **Oráculo, gerador de bordas e motor no mesmo dia.** O motor em Rust acertou 100% nos quatro
  conjuntos (pequeno, grande, manuais, 269 mil bordas) na primeira execução, e zero divergência
  com o oráculo em "todas as regras satisfeitas" em mais duas sementes do gerador.
- **Manifesto gerado do enunciado.** O `regras.yaml` sai das tabelas do `ENUNCIADO.md` por
  script, sem digitação e sem LLM em tempo de execução.

## Onde errou

## 23/10/2026

1. **Plano sem estratégia de leitura da entrada.** O primeiro plano detalhava o scanner, mas não
   dizia como ler o arquivo, e implicitamente juntava a saída num buffer único. O Beto apontou
   leitura em stream. A medição confirmou: memória de 154 MB para 1 MB e ~15% menos tempo, por
   evitar page faults de 160 MB e manter o buffer no cache.
2. **`bool` é `int` no Python.** Um `isinstance(v, (int, float))` aceita `true` como 1, e
   `"valor": true` passaria em `valor_pago < valor`. O enunciado diz que tipo diferente torna a
   condição falsa. Pego ao escrever o oráculo; teste manual id 4.
3. **Gerador com lógica confusa na primeira escrita.** A deduplicação das mutações sistemáticas
   (`dict.fromkeys(map(id, vs)) and vs`) não deduplicava nada, e o ramo "isca depois" da chave
   duplicada tinha uma condição sempre verdadeira. Corrigido na revisão antes de rodar.
4. **Projeto criado dentro do kit.** Regras, oráculo e gerador foram parar em
   `hackathon-2026-d1-kit/`, misturando o material do desafio com a solução. O Beto apontou; o
   projeto passou para a raiz do repositório e o kit voltou ao estado original.
5. **Argumento de linguagem afirmado sem medição.** A IA justificou Rust dizendo que "num laço
   apertado sobre bytes o Rust dá menos trabalho". Quando o Beto pediu para reavaliar, a medição
   mostrou o Go mais rápido no laço ingênuo (0,14 s contra 0,20 s) e o Rust 1,7× mais rápido no
   padrão de salto com SIMD (0,20 s contra 0,34 s). A escolha se manteve, mas pelo motivo medido.
   A mesma medição mostrou que o `docker run` custa ~0,29 s fixos, cerca de dois terços do total.
6. **Dockerfile sem `musl-dev`.** A IA escreveu o build sobre `rust:alpine` sem o pacote que o
   linker precisa para os proc-macros; o primeiro `docker build` falhou ao linkar o
   `serde_derive`. Erro conhecido da imagem, corrigido com `apk add musl-dev`.
7. **Otimização no chute, antes do profiler.** A primeira aposta foi varrer strings 8 bytes por vez
   (SWAR): a validação foi de 171 para 166 ms, quase nada. Só o `sample` do macOS mostrou onde o
   tempo estava (parser por token, `memcmp` da libc em comparações curtas, UTF-8 da linha
   inteira). A partir daí, cada mudança foi medida.
8. **A troca do `memcmp` piorou antes de melhorar.** O primeiro `igual` usava `copy_from_slice` com
   tamanho variável, que vira chamada de `memcpy`: o motor foi de 215 para 238 ms. A versão final
   pré-calcula os 8 primeiros bytes de cada chave da trie e lê os da chave direto da linha.
9. **Thread de leitura que não ganhou nada.** A IA propôs ler o próximo bloco em paralelo, porque a
   leitura no container é mais espera que CPU. Medido no Docker com `--cpus=1`, em três rodadas
   intercaladas, o ganho ficou dentro do ruído; a thread foi retirada. O `mmap`, testado em seguida,
   ganhou ~40 ms em todas as rodadas e ficou.
10. **Bordas com UTF-8 inválido quebraram o `medir.py`.** A IA pôs as linhas com bytes inválidos no
    arquivo principal de bordas; o `medir.py` lê a entrada como texto UTF-8 e caiu. Foram para um
    arquivo à parte, conferido com `diff` contra o oráculo. O achado ficou: se a banca usa o
    `medir.py`, a surpresa não pode ter UTF-8 inválido, e `\r` vira `\n` antes de chegar ao motor.

11. **Teste com a lógica invertida.** No `testes/rodar.sh`, a checagem "o motor recusa regra com
    campo inexistente" acusou falha com o motor certo: a função de teste negava o resultado duas
    vezes. A mensagem do motor no próprio relatório de falha mostrou que o erro era do teste.

12. **Exemplo do README corrompido na gravação.** O texto `ATIVA`, usado para ilustrar escape
    decodificado, foi gravado já decodificado (`ATIVA`), e a frase passou a dizer "ATIVA é ATIVA".
    Pego na releitura do README; restaurado como texto literal.
13. **README com afirmações mais fortes que a evidência.** A primeira versão dizia que o gabarito do
    kit "bate com o json do Python" (o kit não tem bordas, então isso não prova a semântica) e que a
    surpresa "não pode ter" UTF-8 inválido (só vale se a banca usar o `medir.py`). Reescrito como
    aposta e como condição.

14. **Pasta de teste com o nome de um arquivo.** Ao levar os testes de ordem para o binário da
    imagem, o `rodar.sh` copiou o segundo domínio para `$TMP/pedidos`, nome que o próprio script já
    usava para um arquivo de saída. A cópia falhou e o teste acusou divergência no binário com PGO;
    o erro era do script, e a pasta ganhou outro nome.

## Onde a otimização chegou

Grande (160 MB, 200 mil linhas), com o arquivo em memória, no M1 Pro: 248 ms para 177 ms.
No Docker com `--cpus=1`: de 0,707 s para ~0,60 s, dos quais ~0,27 s são do próprio `docker run`.
O PGO na imagem cortou mais ~15% do CPU do motor (0,18 s para 0,155 s dentro do container); com
1,6 milhão de linhas, o Docker foi de 3,12 s para 2,72 s. O Beto questionou se o PGO feria a regra
de trocar regras sem recompilar e a avaliação de cima para baixo; os testes de regras invertidas,
`--todas` e segundo domínio rodam no binário da imagem e passam.
