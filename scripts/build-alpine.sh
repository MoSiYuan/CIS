#!/bin/bash
# Quick Alpine build script for CIS v1.1.1

set -e

echo "=== CIS v1.1.1 Alpine Build ==="

# Use rust nightly alpine image
BUILD_IMAGE="rustlang/rust:nightly-alpine"
RUNTIME_IMAGE="alpine:3.19"

# Clean up old containers
docker rm -f cis-builder 2>/dev/null || true

echo "Building in Alpine container..."
docker run --rm -it \
    -v "$(pwd):/workspace" \
    -w /workspace \
    --name cis-builder \
    $BUILD_IMAGE \
    sh -c "
        apk add --no-cache musl-dev openssl-dev cmake clang llvm-dev pkgconfig protobuf-dev &&
        cargo build --release -p cis-node --features cis-core/vector
    "

echo "Creating runtime image..."
cat > Dockerfile.runtime << 'DOCKERFILE'
FROM alpine:3.19
RUN apk add --no-cache ca-certificates curl jq netcat-openbsd iputils bind-tools nodejs npm libgcc libstdc++
COPY target/release/cis-node /usr/local/bin/cis
RUN chmod +x /usr/local/bin/cis
ENV PATH="/usr/local/bin:$PATH"
ENV CIS_HOME=/root/.cis
ENV RUST_LOG=info
CMD ["cis", "--help"]
LABEL org.opencontainers.image.version="1.1.1"
LABEL org.opencontainers.image.title="CIS Agent Teams"
DOCKERFILE

docker build -f Dockerfile.runtime -t cis:1.1.1-alpine .
rm Dockerfile.runtime

echo "=== Build Complete ==="
echo "Image: cis:1.1.1-alpine"
docker images cis:1.1.1-alpine
