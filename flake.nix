{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs";
    #nixpkgs-mozilla = {
    #  url = "github:mozilla/nixpkgs-mozilla";
    #  flake = false;
    #};
  };

  # add nixpkgs-mozilla
  outputs = { self, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;

          #overlays = [
          #  (import nixpkgs-mozilla)
          #];
        };

        baseBuildInputs = [
            pkgs.openssl 
            # allows packages to find out about other packages, used by openssl-dev (see docs)
            pkgs.pkg-config 
            pkgs.llvmPackages_latest.llvm
            pkgs.llvmPackages_latest.bintools
            pkgs.llvmPackages_latest.lld
            pkgs.nodejs_22
            pkgs.nodePackages.nodemon
            pkgs.volta 

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

        toolchain = (pkgs.rustChannelOf {
          rustToolChain = ./rust-toolchain.toml;
          sha256 = "sha256-szD3iFxjmh7iMAbgj/E9KWp41PAe87S3e3iac3TpqOE=";
        }).rust;

        naersk' = pkgs.callPackage naersk {
          cargo = toolchain;
          rustc = toolchain;
        };

      in rec {
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
          # toggle to release
          release = false;

          cargoBuildOptions = x: x ++ [ "-p" "xtask" ];

          nativeBuildInputs = nativeBuildInputs;

          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
          OPENSSL_NO_VENDOR = "1";
          # see note below in mkShell
          #RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };

        # For `nix develop` (optional, can be skipped):
        devShell = pkgs.mkShell {
          shellHook = ''
            # setting secrets and so on
          '';

          nativeBuildInputs = nativeBuildInputs; 

          buildInputs = [
            #toolchain
              pkgs.rustup
              pkgs.rust-analyzer

              pkgs.nodePackages.nodemon
              pkgs.volta 
              pkgs.pkg-config          

              # helps with unicode conversions, used by `cc` linker
              pkgs.libiconv            
              pkgs.markdownlint-cli

              # llvm
              pkgs.llvmPackages_latest.llvm
              pkgs.llvmPackages_latest.bintools
              pkgs.llvmPackages_latest.lld

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
