class Smash < Formula
  desc "Local Markdown memory for AI agents"
  homepage "https://github.com/ebrahimisoheil/smash"
  url "https://github.com/ebrahimisoheil/smash.git",
      tag:      "v1.1.0",
      revision: "8587b6900829025ee084795b7d73ab207111bf8d"
  license "MIT"
  head "https://github.com/ebrahimisoheil/smash.git", branch: "main"

  depends_on "python@3.14"

  def python3
    Formula["python@3.14"].opt_bin/"python3.14"
  end

  def install
    libexec.install "smash.py", "serve.py", "SMASH.md", ".smashignore"
    libexec.install "logo.svg"
    libexec.install "logo.png" if File.exist?("logo.png")

    (libexec/"mcp_package").mkpath
    (libexec/"mcp_package").install "mcp_package/smash_core"

    (bin/"smash").write <<~SH
      #!/bin/sh
      SMASH_CLI_COMMAND=smash exec "#{python3}" "#{libexec}/smash.py" "$@"
    SH
  end

  def caveats
    <<~EOS
      Try Smash:
        smash demo
        smash serve smash-demo

      Then open:
        http://127.0.0.1:3000
        http://127.0.0.1:3000/graph

      To create a personal wiki:
        smash init ~/Smash

      For MCP clients, install smash-mcp with the agent installer or a venv:
        python3 -m venv ~/.smash-mcp-venv
        ~/.smash-mcp-venv/bin/python -m pip install --upgrade pip smash-mcp
    EOS
  end

  test do
    system bin/"smash", "--version"
    system bin/"smash", "demo", testpath/"smash-demo", "--force"
    system bin/"smash", "validate", testpath/"smash-demo"
    system bin/"smash", "status", "--validate", testpath/"smash-demo"
  end
end
