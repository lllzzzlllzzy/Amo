# 构建阶段
FROM rust:1.87-slim AS builder

WORKDIR /app

# 换国内镜像源
RUN sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources \
    && apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
# 预编译依赖
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release

# 运行阶段
FROM debian:bookworm-slim

WORKDIR /app

RUN sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources \
    && apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/amo ./
COPY --from=builder /app/migrations ./migrations

EXPOSE 3000

CMD ["./amo"]
