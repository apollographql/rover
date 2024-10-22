{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        naersk' = pkgs.callPackage naersk {};

      in rec {
        # For `nix build` & `nix run`:
        defaultPackage = naersk'.buildPackage {
          src = ./.;
          nativeBuildInputs = [ 
            pkgs.openssl 
            pkgs.pkg-config 
          ];
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
          OPENSSL_NO_VENDOR = "1";
        
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

            # stabilizes the rust path
            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };
      }
    );
}
