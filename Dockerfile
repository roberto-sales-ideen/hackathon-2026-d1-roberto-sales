# Build estático (musl) com PGO, e imagem final só com o binário e as regras.
#
# PGO: o motor é compilado instrumentado, roda uma vez sobre o conjunto pequeno repetido e é
# recompilado sabendo quais caminhos do código são os quentes. Muda só a organização do código
# de máquina: as regras continuam sendo lidas do YAML a cada execução, e trocar ou reordenar
# regras não exige recompilar (testes/rodar.sh --docker confere isso no binário da imagem).
FROM rust:1.99.1-alpine AS build
RUN apk add --no-cache musl-dev && rustup component add llvm-tools
WORKDIR /src
COPY motor/Cargo.toml motor/Cargo.lock ./
COPY motor/src ./src
COPY regras /regras
COPY hackathon-2026-d1-kit/dados/pequeno/entrada.jsonl /tmp/pequeno.jsonl
RUN RUSTFLAGS="-Cprofile-generate=/tmp/pgo" cargo build --release --locked --target-dir /tmp/instrumentado
RUN for i in $(seq 2000); do cat /tmp/pequeno.jsonl; done > /tmp/treino.jsonl \
 && /tmp/instrumentado/release/motor --regras /regras/regras.yaml --entrada /tmp/treino.jsonl > /dev/null
RUN "$(find / -name llvm-profdata -path '*rustlib*' | head -1)" merge -o /tmp/pgo.profdata /tmp/pgo
RUN RUSTFLAGS="-Cprofile-use=/tmp/pgo.profdata" cargo build --release --locked

FROM scratch
COPY --from=build /src/target/release/motor /motor
COPY regras /regras
WORKDIR /
ENTRYPOINT ["/motor"]
