{
  description = "normalize - structural code intelligence";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # Rust toolchain with musl cross-compilation target included.
        # fenix gives us per-component control; nixpkgs's plain rustc doesn't
        # carry the musl target stdlib without overrides.
        rustToolchain = fenix.packages.${system}.stable.withComponents [
          "rustc"
          "cargo"
          "clippy"
          "rustfmt"
          "rust-src"
          "rust-std-x86_64-unknown-linux-musl"
        ];
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "normalize";
          version = "0.3.1";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          buildInputs = with pkgs; [ sqlite ];
        };

        # `nix build .#normalize-musl` — validates that the musl target builds
        # and produces a binary with no glibc dependency.
        #
        # Uses pkgsStatic (crt-static=true, fully static) rather than
        # pkgsCross.musl64. pkgsCross.musl64 links against the HOST GCC's
        # libgcc_s.so.1 (glibc-linked), which fails on NixOS because the musl
        # loader then needs ld-linux-x86-64.so.2. pkgsStatic avoids this by
        # statically linking everything — no shared lib deps at all.
        #
        # The CI release workflow uses musl-gcc with -crt-static=false to produce
        # a dynamic binary that can dlopen grammar .so files. libgcc_s.so.1 is
        # bundled from Alpine Linux (musl-linked, not glibc-linked). pkgsStatic
        # validates the musl target locally without needing Alpine's libgcc_s.
        packages.normalize-musl = pkgs.pkgsStatic.rustPlatform.buildRustPackage {
          pname = "normalize-musl";
          version = "0.3.1";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          buildInputs = with pkgs.pkgsStatic; [ sqlite ];
          # Tests need filesystem access, grammars, and a non-sandboxed env.
          doCheck = false;
        };

        devShells.default = pkgs.mkShell rec {
          buildInputs = with pkgs; [
            stdenv.cc.cc
            sqlite
            # Rust toolchain (fenix — includes x86_64-unknown-linux-musl std)
            rustToolchain
            rust-analyzer
            # musl cross-compilation (musl-gcc, matches release.yml)
            musl.dev
            # Fast linker for incremental builds
            mold
            clang
            # JS tooling: VS Code extension, docs, sessions SPA
            bun
            # Grammar development: tree-sitter CLI for writing/testing grammars
            tree-sitter
            nodejs
          ];
          LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath buildInputs}:$LD_LIBRARY_PATH";
        };
      }
    );
}
