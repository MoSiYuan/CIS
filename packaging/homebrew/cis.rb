# CIS (Cluster of Independent Systems) Formula
# 单机 LLM Agent 记忆本地化辅助工具
# https://github.com/MoSiYuan/CIS

class Cis < Formula
  desc "Local LLM Agent memory enhancement framework with P2P federation"
  homepage "https://github.com/MoSiYuan/CIS"
  version "1.1.0"
  license "MIT"

  # 根据架构和操作系统选择下载地址
  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/MoSiYuan/CIS/releases/download/v#{version}/cis-macos-arm64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_ARM64"
    else
      url "https://github.com/MoSiYuan/CIS/releases/download/v#{version}/cis-macos-x86_64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X86_64"
    end
  elsif OS.linux?
    if Hardware::CPU.arm?
      url "https://github.com/MoSiYuan/CIS/releases/download/v#{version}/cis-linux-arm64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_ARM64"
    else
      url "https://github.com/MoSiYuan/CIS/releases/download/v#{version}/cis-linux-x86_64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_X86_64"
    end
  end

  # 构建依赖（如果从源码构建）
  head do
    url "https://github.com/MoSiYuan/CIS.git", branch: "main"
    depends_on "rust" => :build
    depends_on "pkg-config" => :build
    depends_on "openssl@3"
    depends_on "sqlite"
  end

  # 运行时依赖
  depends_on "openssl@3"
  depends_on "sqlite"

  def install
    if build.head?
      # 从源码构建
      system "cargo", "build", "--release", "--bin", "cis-node"
      system "cargo", "build", "--release", "--bin", "cis-cli"
      bin.install "target/release/cis-node"
      bin.install "target/release/cis-cli"
    else
      # 使用预编译二进制
      bin.install "cis-node"
      bin.install "cis-cli" if File.exist?("cis-cli")
      bin.install "cis-gui" if File.exist?("cis-gui")
    end

    # 安装 shell 补全脚本
    generate_completions

    # 安装文档
    doc.install "README.md" if File.exist?("README.md")
    doc.install "LICENSE" if File.exist?("LICENSE")

    # 创建默认配置目录
    (etc/"cis").mkpath
  end

  def generate_completions
    # 使用 clap_complete 生成补全脚本
    bash_completion_dir = etc/"bash_completion.d"
    zsh_completion_dir = share/"zsh/site-functions"
    fish_completion_dir = share/"fish/vendor_completions.d"

    bash_completion_dir.mkpath
    zsh_completion_dir.mkpath
    fish_completion_dir.mkpath

    # 生成补全脚本
    system "#{bin}/cis-node", "completions", "bash", "#{bash_completion_dir}/cis-node"
    system "#{bin}/cis-node", "completions", "zsh", "#{zsh_completion_dir}/_cis-node"
    system "#{bin}/cis-node", "completions", "fish", "#{fish_completion_dir}/cis-node.fish"
    
    # cis-cli 的补全
    system "#{bin}/cis-cli", "completions", "bash", "#{bash_completion_dir}/cis-cli" if File.exist?("#{bin}/cis-cli")
    system "#{bin}/cis-cli", "completions", "zsh", "#{zsh_completion_dir}/_cis-cli" if File.exist?("#{bin}/cis-cli")
    system "#{bin}/cis-cli", "completions", "fish", "#{fish_completion_dir}/cis-cli.fish" if File.exist?("#{bin}/cis-cli")
  rescue => e
    opoo "Failed to generate shell completions: #{e.message}"
  end

  def post_install
    # 创建必要的目录结构
    (var/"lib/cis/data").mkpath
    (var/"log/cis").mkpath

    # 设置权限
    chmod 0750, var/"lib/cis"
    chmod 0750, var/"log/cis"
  end

  def caveats
    <<~EOS
      CIS (Cluster of Independent Systems) 已安装完成！

      快速开始:
        1. 初始化节点: cis init
        2. 配置 AI Provider: 编辑 ~/.cis/config.toml
        3. 启动节点: cis node start

      重要提示:
        - CIS 本身不提供 LLM，需要配置 AI Provider（Claude / Kimi / OpenAI 等）
        - 首次运行会自动创建 ~/.cis 目录存储配置和数据
        - 数据默认存储在: #{var}/lib/cis/data
        - 日志位置: #{var}/log/cis

      Shell 补全:
        - Bash: 确保 #{etc}/bash_completion.d 在您的配置中
        - Zsh: 补全已安装到 #{share}/zsh/site-functions
        - Fish: 补全已安装到 #{share}/fish/vendor_completions.d

      文档: https://github.com/MoSiYuan/CIS#readme
    EOS
  end

  service do
    run [opt_bin/"cis-node", "daemon", "--config", etc/"cis/config.toml"]
    keep_alive true
    working_dir var/"lib/cis"
    log_path var/"log/cis/cis.log"
    error_log_path var/"log/cis/cis-error.log"
    environment_variables PATH: std_env:PATH
  end

  test do
    # 测试二进制文件是否存在且可执行
    assert_match version.to_s, shell_output("#{bin}/cis-node --version")
    assert_match version.to_s, shell_output("#{bin}/cis-cli --version") if File.exist?("#{bin}/cis-cli")

    # 测试初始化（使用临时目录）
    ENV["CIS_HOME"] = testpath/".cis"
    system "#{bin}/cis-node", "init", "--skip-did"
    assert_predicate testpath/".cis/config.toml", :exist?
  end
end
