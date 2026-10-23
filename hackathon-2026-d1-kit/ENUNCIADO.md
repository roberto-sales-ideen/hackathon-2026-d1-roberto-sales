# Desafio 1 · Motor de regras de duplicatas escriturais

Hackathon Ideen · 23/10/2026 · Individual · Prazo: segunda, 26/10, às 9h (arquivamento do repositório)

## O problema

Você recebe uma lista de duplicatas escriturais, uma por linha, com dados de identificação, partes, financeiros, fiscais, de pagamento, de logística e de manifestação do sacado. Seu motor precisa **classificar cada duplicata pela primeira regra que ela satisfaz**, na ordem da tabela de regras.

As regras estão descritas abaixo em linguagem de negócio. Traduza para o formato que quiser (JSON, YAML, DSL, tabela, código de configuração), **desde que as regras fiquem fora do código do motor**: trocar ou reordenar uma regra não pode exigir mudar o motor.

## Contrato

**Comando**

```
motor --entrada entrada.jsonl > saida.txt
```

**Entrada**: arquivo JSON Lines (um objeto JSON por linha). Cada objeto tem um `id` numérico único.

**Saída**: uma linha por duplicata, no stdout, neste formato exato:

```
id:1 - regra: R07
id:2 - regra: SEM_REGRA
```

A ordem das linhas não importa. Espaços extras ao redor de `:` e `-` são tolerados.

## Modelo da duplicata

Os dados são fictícios. O modelo segue os campos usados na escrituração da CERC e na NF-e, simplificado para o desafio.

| Campo | Tipo | Valores |
|---|---|---|
| `id` | número | identificador da linha |
| `iud` | texto | identificador único da duplicata (20 dígitos) |
| `numero` | texto | número da nota e parcela, `123456/1` |
| `status` | texto | `ATIVA`, `LIQUIDADA`, `CANCELADA` |
| `data_emissao` | data | `AAAA-MM-DD` |
| `data_vencimento` | data | `AAAA-MM-DD` |
| `sacador.cnpj` / `sacador.uf` | texto | |
| `sacador.porte` | texto | `ME`, `EPP`, `MEDIA`, `GRANDE` |
| `sacado.cnpj` / `sacado.uf` | texto | |
| `sacado.segmento` | texto | `VAREJO`, `INDUSTRIA`, `AGRO`, `SERVICOS`, `SAUDE`, `ATACADO` |
| `financeiro.valor` | número | valor da duplicata, em reais |
| `financeiro.valor_pago` | número | valor já pago |
| `financeiro.moeda` | texto | `BRL` |
| `financeiro.parcela` / `financeiro.total_parcelas` | número | |
| `financeiro.tipo_negociacao` | texto ou nulo | `CSCO`, `CSAO`, `CSFD`, `PENR`, `PRCS` |
| `financeiro.taxa_juros_mensal` | número | % ao mês |
| `pagamento.instrumento` | texto | `TRCR` (transferência), `PPIX` (Pix), `BOLETO` |
| `pagamento.compe` / `pagamento.tipo_conta` | texto | `tipo_conta`: `CCOR`, `CPAG` |
| `fiscal.chave_nfe` | texto | 44 dígitos |
| `fiscal.cfop` | texto | ex.: `5102`, `6102` |
| `logistica.modal` | texto | `RODOVIARIO`, `AEREO`, `FERROVIARIO`, `AQUAVIARIO` |
| `logistica.transportadora_cnpj` | texto | |
| `logistica.uf_origem` / `logistica.uf_destino` | texto | |
| `logistica.peso_kg` | número | |
| `logistica.status_entrega` | texto | `ENTREGUE`, `EM_TRANSITO`, `DEVOLVIDA`, `AVARIADA` |
| `logistica.data_entrega` | data ou nulo | |
| `logistica.migo` | booleano | recebimento físico registrado no ERP |
| `manifestacao.tipo` | texto ou nulo | `ACTO`, `RCAV`, `RCQQ`, `RCVN`, `RCVL`, `RCOP`, `ANTC` |
| `manifestacao.data` | data ou nulo | |

## Regras

Avalie na ordem. **A primeira regra satisfeita classifica a duplicata.** Se nenhuma for satisfeita, a saída é `SEM_REGRA`.

Data de referência para todas as comparações de data: **2026-10-23**.

| Código | Categoria | Condição |
|---|---|---|
| R01 | Fora da carteira | `status` é `CANCELADA` ou `LIQUIDADA` |
| R02 | Risco de operação inexistente | `logistica.status_entrega` = `DEVOLVIDA` **OU** `manifestacao.tipo` = `RCOP` |
| R03 | Recusada pelo sacado | `manifestacao.tipo` é `RCAV`, `RCQQ`, `RCVN` ou `RCVL` |
| R04 | Candidata a recusa por avaria | `logistica.status_entrega` = `AVARIADA` **E** `manifestacao.tipo` está vazio |
| R05 | Vencida em aberto | `data_vencimento` < data de referência **E** `financeiro.valor_pago` < `financeiro.valor` |
| R06 | Garantia relevante | `financeiro.tipo_negociacao` é `CSFD` ou `PENR` **E** `financeiro.valor` ≥ 100000 |
| R07 | Alto valor | `financeiro.valor` ≥ 500000 **OU** (`sacado.segmento` = `AGRO` **E** `financeiro.valor` ≥ 200000) |
| R08 | CFOP interno em operação interestadual | `logistica.uf_origem` ≠ `logistica.uf_destino` **E** `fiscal.cfop` começa com `5` |
| R09 | Frete aéreo pesado | `logistica.modal` = `AEREO` **E** `logistica.peso_kg` > 500 |
| R10 | Entregue sem recebimento no ERP | `logistica.status_entrega` = `ENTREGUE` **E** `logistica.migo` é falso |
| R11 | Apta para antecipação | `manifestacao.tipo` = `ACTO` **E** `logistica.migo` é verdadeiro |
| R12 | Pequeno valor via Pix | `pagamento.instrumento` = `PPIX` **E** `sacador.porte` é `ME` ou `EPP` **E** `financeiro.valor` ≤ 5000 |

## Semântica (leia com atenção: é aqui que a surpresa pega)

1. **Campo ausente, nulo ou com tipo diferente do esperado** torna **falsa** qualquer condição de comparação sobre ele, inclusive `≠`.
   Exemplo: `financeiro.valor` igual a `"15000.00"` (texto) não é número, então `valor ≥ 100000` é falso.
2. **"Está vazio"** é verdadeiro quando o campo é ausente, nulo ou texto vazio (`""`).
3. **Texto** compara com igualdade exata, diferenciando maiúsculas e minúsculas: `ativa` ≠ `ATIVA`.
4. **Booleano** só vale como `true` ou `false` do JSON. `"false"`, `0` e `1` não são booleanos.
5. **Data** só vale no formato `AAAA-MM-DD` e precisa ser uma data válida. Fora disso, é tratada como ausente.
6. **"Começa com"** só se aplica a texto. Um CFOP numérico (`5102` sem aspas) não é texto.
7. **Limites**: `≥` e `≤` incluem o valor-limite, `>` e `<` não incluem.
8. **E / OU**: cada linha da tabela vale como está escrita. Parênteses indicam precedência.

## Os conjuntos

| Conjunto | Onde | Para quê |
|---|---|---|
| Pequeno (100 casos) | `dados/pequeno/` | validar que você acerta; cobre todas as regras |
| Grande (200 mil casos) | `dados/grande/` (compactado) | volume e tempo |
| Surpresa | só a banca, na segunda | mesmas regras, casos novos e de borda: é o que vale para o ranking |

## Ranking

1. Quantos casos da surpresa o motor acerta.
2. Desempate por **tempo total do comando**, da largada ao fim, incluindo inicialização.
3. Três execuções, 1 CPU, mesma máquina para todos; vale a mediana.

A posição no ranking vira a nota de "respeitou o desafio" (30%). Os outros critérios (entregou o mínimo 35%, reaproveitável 20%, README 15%) valem como nos outros desafios.
