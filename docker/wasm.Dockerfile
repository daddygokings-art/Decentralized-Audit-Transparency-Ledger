FROM rust:1.80.0

WORKDIR /build

COPY Cargo.toml Cargo.lock* ./
COPY src ./src

RUN rustup target add wasm32-unknown-unknown && \
    cargo build --target wasm32-unknown-unknown --release \
      --locked 2>&1 | tee build.log

RUN echo "Build artifacts:" && \
    ls -lh target/wasm32-unknown-unknown/release/audit_ledger.wasm && \
    echo "" && \
    echo "SHA-256 Hash:" && \
    sha256sum target/wasm32-unknown-unknown/release/audit_ledger.wasm

CMD ["cat", "build.log"]
