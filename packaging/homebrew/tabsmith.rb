class Tabsmith < Formula
  desc "IDE-style autocomplete for your terminal: dropdown menu and inline ghost text"
  homepage "https://github.com/PedroLaRosa/tabsmith"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/PedroLaRosa/tabsmith/releases/download/v@VERSION@/tabsmith-darwin-arm64"
      sha256 "@DARWIN_ARM64@"
    end
    on_intel do
      url "https://github.com/PedroLaRosa/tabsmith/releases/download/v@VERSION@/tabsmith-darwin-x64"
      sha256 "@DARWIN_X64@"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/PedroLaRosa/tabsmith/releases/download/v@VERSION@/tabsmith-linux-arm64"
      sha256 "@LINUX_ARM64@"
    end
    on_intel do
      url "https://github.com/PedroLaRosa/tabsmith/releases/download/v@VERSION@/tabsmith-linux-x64"
      sha256 "@LINUX_X64@"
    end
  end

  def install
    bin.install Dir["tabsmith-*"].first => "tabsmith"
  end

  def caveats
    text = <<~EOS
      To add the shell integration to your dotfiles (it asks first), run:
        tabsmith setup
      Homebrew updates this copy (brew upgrade tabsmith); it does not update itself.
      Before brew uninstall tabsmith, run: tabsmith uninstall
    EOS
    # ponytail: tabsmith 1.0.0 wrote the versioned Cellar path into the dotfiles on Linux, so an upgrade from it, to
    # whichever later version, needs one doctor run. Delete this once upgrades from 1.0.0 are unlikely.
    if OS.linux?
      text += <<~EOS
        On Linux, after upgrading from tabsmith 1.0.0, run tabsmith doctor once to repoint the shell integration.
      EOS
    end
    text
  end

  test do
    assert_equal "tabsmith #{version}", shell_output("#{bin}/tabsmith --version").strip
  end
end
