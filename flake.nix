{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    # note the unstable; should pin it to 24.05 sha
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        baseBuildInputs = [
            pkgs.openssl 
            pkgs.pkg-config 
            pkgs.llvmPackages_latest.llvm
            pkgs.llvmPackages_latest.bintools
            pkgs.llvmPackages_latest.lld
            # NodeJS -- npm required for running e2e tests
            pkgs.nodejs_22
            pkgs.nodePackages.nodemon
            pkgs.volta # used for something during the release build

        ];

        nativeBuildInputs = if system == "aarch64-darwin" 
          then [ 
            baseBuildInputs
            # apple-specific; need to break out of defaultSystems
            pkgs.darwin.apple_sdk.frameworks.CoreServices
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ]
          else [
            baseBuildInputs
          ];

        naersk' = pkgs.callPackage naersk {};

      in rec {
        # For `nix build` & `nix run`:
        defaultPackage = naersk'.buildPackage {
          src = ./.;
          release = false;
          nativeBuildInputs = [ 
            pkgs.openssl 
            pkgs.pkg-config 
          ];
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
          OPENSSL_NO_VENDOR = "1";
        };






        packages.xtask = naersk'.buildPackage {
          pname = "xtask";
          src = ./.;
          release = false;

          cargoBuildOptions = x: x ++ [ "-p" "xtask" ];

          nativeBuildInputs = nativeBuildInputs;

          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
          OPENSSL_NO_VENDOR = "1";
          # rover xtask specific
          # TODO: make this into a makeWrapper call for run
          #NIX_CARGO_MANIFEST_DIR = ./xtask;
          #CARGO_MANIFEST_DIR = self/xtask;
          # see note below in mkShell
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };

        # For `nix develop` (optional, can be skipped):
        devShell = pkgs.mkShell {
          shellHook = ''
            # setting secrets and so on
          '';

          buildInputs = [
              pkgs.rustup
              pkgs.rust-analyzer


              # NodeJS -- npm required for running e2e tests
              pkgs.nodejs_22
              pkgs.nodePackages.nodemon
              pkgs.volta # used for something during the release build

              # OpenSSL
              pkgs.pkg-config          # allows packages to find out about other packages, used by openssl-dev (see docs)
              pkgs.libiconv            # helps with unicode conversions, used by `cc` linker
              pkgs.openssl

              # llvm
              pkgs.llvmPackages_latest.llvm
              pkgs.llvmPackages_latest.bintools
              pkgs.llvmPackages_latest.lld

              # apple-specific headers: if linking fails, likely a missing header
              pkgs.darwin.apple_sdk.frameworks.CoreServices
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration

              # markdown shit for my editor; hotfix, need to be in base shell
              pkgs.markdownlint-cli

              # latest released rover binary
                  #unstable.rover
            ];

            # auto-accept elv2 for rover
            APOLLO_ELV2_LICENSE = "accept";

            # disable telemetry for rover
            APOLLO_TELEMETRY_DISABLED = "1";

            # stabilizes the rust path for tools like rust-analyzer; to quote nixos.wiki/wiki/rust:
            #
            # Certain Rust tools won't work without this
            # This can also be fixed by using oxalica/rust-overlay and specifying the rust-src extension
            # See https://discourse.nixos.org/t/rust-src-not-found-and-other-misadventures-of-developing-rust-on-nixos/11570/3?u=samuela. for more details.
            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };
      }
    );
}

#makeWrapper $out/bin/.zknotes-server $out/bin/zknotes-server --set ZKNOTES_STATIC_PATH $out/share/zknotes/static \
#  --prefix PATH : ${nixpkgs.lib.makeBinPath [ pkgs.youtube-dl ]}
