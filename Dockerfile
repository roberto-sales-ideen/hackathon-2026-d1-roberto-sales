# Build estático (musl) e imagem final só com o binário e as regras.
FROM rust:1.90-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY motor/Cargo.toml motor/Cargo.lock ./
COPY motor/src ./src
RUN cargo build --release --locked

FROM scratch
COPY --from=build /src/target/release/motor /motor
COPY regras /regras
WORKDIR /
ENTRYPOINT ["/motor"]
