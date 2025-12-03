{
  description = "eframe devShell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {inherit system overlays;};

      rustToolchainToml = fromTOML (builtins.readFile ./rust-toolchain);
      inherit (rustToolchainToml.toolchain) channel targets components;

      rustToolchain = pkgs.rust-bin.stable.${channel}.default.override {
        extensions = components;
        inherit targets;
      };
    in
      with pkgs; {
        devShells.default = mkShell rec {
          nativeBuildInputs = [
            # WASM build tools
            (lib.getBin binaryen) # wasm-opt
            lld
            llvmPackages.clang-unwrapped
            pkg-config

            # FlatBuffers compiler
            flatbuffers
          ];

          buildInputs =
            [
              # Rust
              rustToolchain
              trunk

              # Python
              python3
              python3.pkgs.pip
              python3.pkgs.virtualenv
              ruff

              gcc
              zlib

              # misc. libraries
              openssl
              pkg-config

              # nix
              nixd
              alejandra

              # misc
              typos
              just

              # pixi
              freetype

              glib
              # gtk3
              #
              nasm
            ]
            ++ lib.optionals stdenv.isLinux [
              # GUI libs
              libxkbcommon
              libGL
              fontconfig

              # wayland libraries
              wayland

              # graphics and vulkan
              mesa
              vulkan-loader

              # x11 libraries
              xorg.libXcursor
              xorg.libXrandr
              xorg.libXi
              xorg.libX11
            ];

          # Environment variables for WASM compilation
          env = let
            inherit (llvmPackages) clang-unwrapped;
            majorVersion = lib.versions.major clang-unwrapped.version;
            resourceDir = "${lib.getLib clang-unwrapped}/lib/clang/${majorVersion}";
            includeDir = "${lib.getLib llvmPackages.libclang}/lib/clang/${majorVersion}/include";
          in {
            CC_wasm32_unknown_unknown = lib.getExe clang-unwrapped;
            CFLAGS_wasm32_unknown_unknown = "-isystem ${includeDir} -resource-dir ${resourceDir}";
          };

          shellHook = ''
            # Set LD_LIBRARY_PATH first for compiled packages
            export LD_LIBRARY_PATH=${lib.makeLibraryPath buildInputs}:${lib.makeLibraryPath [stdenv.cc.cc]}:$LD_LIBRARY_PATH

            # Create virtual environment if it doesn't exist
            if [ ! -d ".venv" ]; then
                python -m venv .venv
                source .venv/bin/activate
                pip install -e .
            else
                source .venv/bin/activate
            fi

            # Allow pip to install wheels
            unset SOURCE_DATE_EPOCH
          '';
        };
      });
}
