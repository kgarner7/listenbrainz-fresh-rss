FROM lukemathwalker/cargo-chef AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin listenbrainz-fresh-rss

FROM debian:bookworm-slim AS runtime
EXPOSE 8000
ENV ROCKET_ADDRESS="0.0.0.0"
RUN apt-get update -y && apt-get install -y ca-certificates openssl sqlite3
WORKDIR /app
COPY --from=builder /app/target/release/listenbrainz-fresh-rss /usr/local/bin
ENTRYPOINT ["/usr/local/bin/listenbrainz-fresh-rss"]
