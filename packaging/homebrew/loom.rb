# Formule Homebrew de LOOM, pour le tap munaypassaqui/homebrew-loom.
# À chaque version : mettre à jour version, url et sha256 (affichés par
# packaging/macos/make-dmg.sh). L'URL doit être publique (voir tasks/todo.md, C5).
class Loom < Formula
  desc "Langage de live-coding musical, dans le terminal"
  homepage "https://munaypassaqui.com/loom/"
  version "0.1.0"
  url "https://munaypassaqui.com/loom/dl/loom-0.1.0-macos-universal.tar.gz"
  sha256 "85cb5debdacc2ddc34fe566a109b9f3adee0db888fc5e052036cd47f0692c36e"

  depends_on :macos

  def install
    bin.install "loom"
  end

  test do
    assert_match "loom #{version}", shell_output("#{bin}/loom --version")
  end
end
