#!/bin/bash
# CIS Homebrew Formula 自动更新脚本
# Usage: ./update-formula.sh <version>

set -e

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 1.2.0"
    exit 1
fi

# 移除版本号前面的 v（如果有）
VERSION="${VERSION#v}"

echo "🚀 更新 CIS Homebrew Formula 到版本 $VERSION"

REPO="MoSiYuan/CIS"
FORMULA_FILE="$(dirname "$0")/cis.rb"

cd "$(dirname "$0")"

# 创建临时目录
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

echo "📥 下载各平台二进制文件并计算 SHA256..."

# 定义平台
PLATFORMS=(
    "macos-arm64:cis-macos-arm64.tar.gz"
    "macos-x86_64:cis-macos-x86_64.tar.gz"
    "linux-arm64:cis-linux-arm64.tar.gz"
    "linux-x86_64:cis-linux-x86_64.tar.gz"
)

# 存储 SHA256 值
declare -A SHASUMS

for platform in "${PLATFORMS[@]}"; do
    IFS=':' read -r name file <<< "$platform"
    url="https://github.com/${REPO}/releases/download/v${VERSION}/${file}"
    
    echo "  下载 $name..."
    if curl -fsL "$url" -o "$TMP_DIR/$file" 2>/dev/null; then
        sha=$(sha256sum "$TMP_DIR/$file" | cut -d' ' -f1)
        SHASUMS[$name]="$sha"
        echo "    SHA256: $sha"
    else
        echo "    ⚠️  跳过 $name (文件不存在)"
        SHASUMS[$name]="PLACEHOLDER_SHA256_${name^^}"
    fi
done

echo ""
echo "📝 更新 Formula..."

# 更新版本号
sed -i.bak "s/version \".*\"/version \"${VERSION}\"/" "$FORMULA_FILE"

# 更新各平台的 SHA256
for platform in "${PLATFORMS[@]}"; do
    IFS=':' read -r name file <<< "$platform"
    placeholder="PLACEHOLDER_SHA256_${name^^/-/_}"
    sha="${SHASUMS[$name]}"
    
    if [ -n "$sha" ] && [ "$sha" != "$placeholder" ]; then
        sed -i.bak "s/sha256 \"${placeholder}\"/sha256 \"${sha}\"/" "$FORMULA_FILE"
        echo "  ✓ 更新 $name SHA256"
    fi
done

# 清理备份文件
rm -f "${FORMULA_FILE}.bak"

echo ""
echo "✅ Formula 更新完成！"
echo ""
echo "更新摘要:"
echo "  版本: $VERSION"
for platform in "${PLATFORMS[@]}"; do
    IFS=':' read -r name file <<< "$platform"
    sha="${SHASUMS[$name]}"
    if [[ "$sha" != PLACEHOLDER* ]]; then
        echo "  $name: ${sha:0:16}..."
    else
        echo "  $name: (未更新)"
    fi
done

echo ""
echo "🧪 测试 Formula..."
echo "  brew install --formula $FORMULA_FILE"
echo ""

# 可选：提交 PR 到 Homebrew Core
read -p "是否提交 PR 到 Homebrew Core? (y/N): " submit_pr
if [[ "$submit_pr" =~ ^[Yy]$ ]]; then
    echo ""
    echo "提交 PR 步骤:"
    echo "  1. Fork https://github.com/Homebrew/homebrew-core"
    echo "  2. git clone https://github.com/<your-username>/homebrew-core"
    echo "  3. cd homebrew-core"
    echo "  4. git checkout -b cis-${VERSION}"
    echo "  5. cp ${FORMULA_FILE} Formula/c/cis.rb"
    echo "  6. git add Formula/c/cis.rb"
    echo "  7. git commit -m 'cis ${VERSION}'"
    echo "  8. git push origin cis-${VERSION}"
    echo "  9. 创建 PR"
    echo ""
    echo "或使用 brew bump-formula-pr:"
    echo "  brew bump-formula-pr --version=${VERSION} cis"
fi

echo ""
echo "🎉 完成！"
