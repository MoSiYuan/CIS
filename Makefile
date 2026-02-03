# CIS Makefile

VERSION := $(shell grep "^version" cis-node/Cargo.toml | head -1 | cut -d'"' -f2)

.PHONY: all build test clean release

all: build

build:
	cargo build --release --package cis-node

test:
	cargo test --all

clean:
	cargo clean
	rm -rf target/macos target/linux

# macOS 构建
build-macos:
	./scripts/build/macos/build_app.sh
	./scripts/build/macos/build_dmg.sh

# Linux 构建
build-linux:
	./scripts/build/linux/build_appimage.sh
	./scripts/build/linux/build_deb.sh

# 一键构建所有平台
release:
	./scripts/release/build_all.sh

# 安装到本地
install:
	cargo install --path cis-node --force
