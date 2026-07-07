{
  description = "videoeditor — scripted short-video renderer for developers (Rust + web + ffmpeg)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            # Rust toolchain — the orchestrator
            rustc
            cargo
            clippy
            rustfmt
            rust-analyzer

            # Media pipeline
            ffmpeg
            yt-dlp

            # Repo tooling
            git
            just
            jq
          ];

          shellHook = ''
            echo "videoeditor dev shell — rustc $(rustc --version | cut -d' ' -f2), ffmpeg $(ffmpeg -version 2>/dev/null | head -1 | cut -d' ' -f3)"
            # Chrome renders the scene templates. Nix doesn't ship Chrome on
            # darwin — we use the system install; override with CHROME_BIN.
            if [ -z "$CHROME_BIN" ] && [ -x "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" ]; then
              export CHROME_BIN="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
            fi
          '';
        };
      });
    };
}
