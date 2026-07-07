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
      # The preferred install path: `nix profile install github:security-union/videoeditor`
      # (or `nix run` it). Builds the pinned workspace and wraps the binary so a
      # pinned ffmpeg — and on Linux a pinned chromium — is always found, no
      # matter what the host has installed. macOS can't get Chrome from nixpkgs;
      # there the system Chrome is used (CHROME_BIN overrides).
      packages = forAllSystems (pkgs:
        let
          runtimeDeps = [ pkgs.ffmpeg ]
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.chromium ];
        in
        rec {
          videoeditor = pkgs.rustPlatform.buildRustPackage {
            pname = "videoeditor";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            # Tests that shell out to ffmpeg/Chrome don't exist yet; unit tests
            # (parser, fit-check) run in the sandbox as-is.
            postInstall = ''
              wrapProgram $out/bin/videoeditor \
                --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
            '';
            meta = {
              description = "Scripted short-video renderer: markdown in, vertical video out";
              homepage = "https://github.com/security-union/videoeditor";
              license = pkgs.lib.licenses.mit;
              mainProgram = "videoeditor";
            };
          };
          default = videoeditor;
        });

      apps = forAllSystems (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.system}.videoeditor}/bin/videoeditor";
        };
      });

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
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.chromium ];

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
