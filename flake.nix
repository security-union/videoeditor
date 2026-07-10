{
  description = "videoeditor — scripted short-video renderer for developers (Rust + web + ffmpeg)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    # rust with the wasm32-unknown-unknown target for the Leptos recorder UI
    # (nixpkgs' rustc ships host-only std)
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system:
        f (import nixpkgs { inherit system; overlays = [ rust-overlay.overlays.default ]; }));
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
              # Ship templates/formats as a store path instead of relying on
              # the first-run cache extraction — fully pinned, never stale.
              mkdir -p $out/share/videoeditor
              cp -R crates/videoeditor/templates crates/videoeditor/formats $out/share/videoeditor/
              wrapProgram $out/bin/videoeditor \
                --prefix PATH : ${lib.makeBinPath runtimeDeps} \
                --set-default VIDEOEDITOR_ROOT $out/share/videoeditor ${lib.optionalString pkgs.stdenv.isDarwin ''\
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
          # one toolchain for host AND wasm: the recorder UI
          # (videoeditor-record-ui) compiles to wasm32-unknown-unknown
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            targets = [ "wasm32-unknown-unknown" ];
          };
          # `videoeditor` inside the dev shell = cargo run over THIS checkout,
          # so the command always matches the source you're editing (a
          # store-built binary here would silently go stale). First call
          # compiles; cargo caches after that.
          videoeditorDev = pkgs.writeShellScriptBin "videoeditor" ''
            exec cargo run --release --quiet \
              --manifest-path "''${VIDEOEDITOR_SRC:?nix develop must start at the repo root}/Cargo.toml" \
              -p videoeditor -- "$@"
          '';
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              # Rust toolchain — the orchestrator (host + wasm32 targets)
              rustToolchain
              rust-analyzer

              # Recorder UI: Leptos → wasm. wasm-bindgen-cli must match the
              # workspace's pinned wasm-bindgen crate — bump them together.
              trunk
              wasm-bindgen-cli

              # Recorder e2e: playwright runner + pinned browsers
              nodejs
              playwright-test

              # Media pipeline
              ffmpeg
              yt-dlp

              # Repo tooling
              git
              just
              jq

              videoeditorDev
            ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.chromium ];

            shellHook = ''
              export VIDEOEDITOR_SRC="$PWD"
              echo "videoeditor dev shell — rustc $(rustc --version | cut -d' ' -f2), ffmpeg $(ffmpeg -version 2>/dev/null | head -1 | cut -d' ' -f3)"
              echo "\`videoeditor\` builds+runs this checkout (cargo run); first call compiles"
              # Playwright e2e (just e2e): pinned browser bundle + resolvable
              # @playwright/test for the config/spec imports.
              export PLAYWRIGHT_BROWSERS_PATH=${pwBrowsers}
              export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
              export NODE_PATH=${pkgs.playwright-test}/lib/node_modules
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
