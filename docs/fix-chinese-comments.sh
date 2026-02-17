#!/bin/bash
# P1-2: 统一注释为英文 - 批量修复工具
#
# 用途: 将 Rust 代码中的中文注释翻译为英文
# 使用: ./scripts/fix-chinese-comments.sh [directory]
#
# 注意: 此脚本使用翻译 API，需要配置翻译服务或手动验证

set -e

TARGET_DIR="${1:-cis-core/src}"

echo "🔍 扫描中文注释..."
echo "目录: $TARGET_DIR"
echo ""

# 统计包含中文注释的文件
chinese_files=$(find "$TARGET_DIR" -name "*.rs" -exec grep -l "//.*[\u4e00-\u9fa5]" {} \; 2>/dev/null | wc -l)
echo "找到 $chinese_files 个文件包含中文注释"

# 显示前10个文件
echo ""
echo "示例文件:"
find "$TARGET_DIR" -name "*.rs" -exec grep -l "//.*[\u4e00-\u9fa5]" {} \; 2>/dev/null | head -10

echo ""
echo "⚠️  修复策略:"
echo "1. 公共 API 文档注释 (///) → 必须翻译为英文"
echo "2. 内部注释 (//) → 可翻译为英文或保持中文"
echo "3. 测试代码注释 → 可保持中文"
echo ""

# 生成修复清单
echo "📋 生成修复清单..."
find "$TARGET_DIR" -name "*.rs" -print0 | while IFS= read -r -d '' file; do
    if grep -q "//.*[\u4e00-\u9fa5]" "$file" 2>/dev/null; then
        echo "  - $file"
    fi
done > /tmp/chinese-comment-files.txt

echo "修复清单已保存到: /tmp/chinese-comment-files.txt"
echo ""

# 提供手动修复示例
echo "📝 手动修复示例:"
echo ""
echo "修复前 (不好):"
echo "  /// 记忆服务模块"
echo "  /// 提供私域/公域记忆管理"
echo ""
echo "修复后 (推荐):"
echo "  /// Memory service module"
echo "  /// Provides private/public memory management"
echo ""

# 使用 cargo translate 示例（如果安装了）
if command -v cargo-translate &> /dev/null; then
    echo "💡 发现 cargo-translate 工具，可以使用:"
    echo "   cargo translate cis-core/src"
else
    echo "💡 推荐使用工具:"
    echo "   1. AI 辅助翻译 (Claude, ChatGPT 等)"
    echo "   2. IDE 批量替换 (VSCode, IntelliJ)"
    echo "   3. cargo-translate (需安装: cargo install cargo-translate)"
fi

echo ""
echo "🎯 优先修复文件 (公共 API):"
find "$TARGET_DIR" -name "*.rs" -print0 | while IFS= read -r -d '' file; do
    if grep -q "///.*[\u4e00-\u9fa5]" "$file" 2>/dev/null; then
        echo "  - $file"
    fi
done | head -20

echo ""
echo "✅ 修复完成后验证:"
echo "   1. 运行 cargo doc 确保文档生成正确"
echo "   2. 运行 cargo clippy 检查代码质量"
echo "   3. 运行 cargo test 确保测试通过"
echo ""
