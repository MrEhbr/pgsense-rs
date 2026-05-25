{
  description = "foo";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.1";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nur = {
      url = "github:nix-community/NUR";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forEachSupportedSystem = f: inputs.nixpkgs.lib.genAttrs supportedSystems (system: f {
        pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [
            inputs.self.overlays.default
            inputs.nur.overlays.default
          ];
        };
      });
    in
    {
      overlays.default = final: prev: {
        rustToolchain = with inputs.fenix.packages.${prev.stdenv.hostPlatform.system};
          combine ([
            latest.clippy
            latest.rustc
            latest.cargo
            latest.rust-src
            latest.rustfmt
            latest.llvm-tools
            targets.x86_64-apple-darwin.latest.rust-std
            targets.aarch64-apple-darwin.latest.rust-std
            targets.x86_64-unknown-linux-gnu.latest.rust-std
            targets.aarch64-unknown-linux-gnu.latest.rust-std
          ]);
      };

      devShells = forEachSupportedSystem ({ pkgs }: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain
            rust-analyzer
            openssl
            pkg-config
            cargo-deny
            cargo-edit
            cargo-watch
            cargo-nextest
            cargo-zigbuild
            cargo-shear
            cargo-criterion
            tokio-console
            gnuplot
            typos
            git-cliff
            mdbook
            nur.repos.goreleaser.goreleaser
            zig_0_14
            postgresql_18
            curl
            prek
            protobuf
          ];

          env = {
            # Required by rust-analyzer
            RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
          };
        };
      });
    };
}
