# CIS (Cluster of Independent Systems)
# 单机 LLM Agent 记忆本地化辅助工具
class Cis < Formula
  desc "Local-first LLM Agent memory enhancement framework"
  homepage "https://github.com/MoSiYuan/CIS"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/MoSiYuan/CIS/releases/download/v0.1.0/cis-0.1.0-macos-arm64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_ARM64"
    else
      url "https://github.com/MoSiYuan/CIS/releases/download/v0.1.0/cis-0.1.0-macos-x86_64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X86_64"
    end
  end

  on_linux do
    url "https://github.com/MoSiYuan/CIS/releases/download/v0.1.0/cis-0.1.0-linux-x86_64.tar.gz"
    sha256 "PLACEHOLDER_SHA256_LINUX"
  end

  def install
    bin.install "cis-node" => "cis"
    
    # 安装 shell 补全
    bash_completion.install "completions/cis.bash" => "cis"
    zsh_completion.install "completions/_cis" => "_cis"
    fish_completion.install "completions/cis.fish"
  end

  def post_install
    (var/"log/cis").mkpath
    (var/"cis").mkpath
  end

  def caveats
    <<~EOS
      CIS (Cluster of Independent Systems) 安装完成！
      
      快速开始:
        cis init              # 初始化 CIS
        cis doctor            # 检查环境
        cis skill list        # 列出可用技能
      
      文档:
        https://github.com/MoSiYuan/CIS/blob/main/docs/USAGE.md
      
      配置文件位置:
        ~/.cis/config.toml    # 全局配置
        ./.cis/config.toml    # 项目级配置（在 Git 项目中）
    EOS
  end

  service do
    run [opt_bin/"cis", "node", "start"]
    keep_alive true
    log_path var/"log/cis/cis.log"
    error_log_path var/"log/cis/cis.error.log"
    environment_variables PATH: std_env:PATH
  end

  test do
    system "#{bin}/cis", "--version"
    system "#{bin}/cis", "doctor"
  end
end
