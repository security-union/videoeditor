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
      # (or `nix run` it). Builds the pinned workspace and wraps the binary so
      # EVERY runtime dependency is pinned: ffmpeg on all systems, chromium
      # (nixpkgs build) on Linux, and playwright's free-licensed Chrome for
      # Testing bundle on macOS — nixpkgs' own `chromium` is Linux-only and
      # `google-chrome` is unfree. CHROME_BIN still overrides everywhere.
      packages = forAllSystems (pkgs:
        let
          lib = pkgs.lib;
          runtimeDeps = [ pkgs.ffmpeg ]
            ++ lib.optionals pkgs.stdenv.isLinux [ pkgs.chromium ];
          pwBrowsers = pkgs.playwright-driver.browsers-chromium;
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
            # On darwin the Chrome-for-Testing binary path embeds a playwright
            # revision (chromium-NNNN/chrome-mac-<arch>/…), so locate it at
            # build time and bake the store path — the build fails loudly if
            # the bundle layout ever changes.
            postInstall = ''
              ${lib.optionalString pkgs.stdenv.isDarwin ''
                chromeBin="$(find -L ${pwBrowsers} -type f \
                  \( -name "Google Chrome for Testing" -o -name "Chromium" \) | head -1)"
                test -n "$chromeBin" || { echo "no chromium in playwright bundle"; exit 1; }
              ''}
              wrapProgram $out/bin/videoeditor \
                --prefix PATH : ${lib.makeBinPath runtimeDeps} ${lib.optionalString pkgs.stdenv.isDarwin ''\
                --set-default CHROME_BIN "$chromeBin"''}
            '';
            meta = {
              description = "Scripted short-video renderer: markdown in, vertical video out";
              homepage = "https://github.com/security-union/videoeditor";
              license = lib.licenses.mit;
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

      devShells = forAllSystems (pkgs:
        let
          pwBrowsers = pkgs.playwright-driver.browsers-chromium;
        in
        {
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
              # Rendering uses the same pinned browser as the installed
              # package: chromium from nixpkgs on Linux (found via PATH),
              # playwright's Chrome for Testing on darwin. CHROME_BIN overrides.
              ${pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
                if [ -z "$CHROME_BIN" ]; then
                  CHROME_BIN="$(find -L ${pwBrowsers} -type f \( -name "Google Chrome for Testing" -o -name "Chromium" \) 2>/dev/null | head -1)"
                  export CHROME_BIN
                fi
              ''}
            '';
          };
        });
    };
}
