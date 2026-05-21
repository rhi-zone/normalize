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
        # `withComponents` only takes host components; target stdlibs live under
        # `targets.<triple>.stable.rust-std` and must be merged via `combine`.
        fenixPkgs = fenix.packages.${system};
        rustToolchain = fenixPkgs.combine [
          (fenixPkgs.stable.withComponents [
            "rustc"
            "cargo"
            "clippy"
            "rustfmt"
            "rust-src"
          ])
          fenixPkgs.targets.x86_64-unknown-linux-musl.stable.rust-std
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

        devShells.default =
          let
            # mkShell merges `packages` into `nativeBuildInputs`. stdenv's
            # cc-wrapper setup hook then propagates any nativeBuildInput with an
            # include/ or lib/ dir into NIX_CFLAGS_COMPILE / NIX_LDFLAGS, leaking
            # musl/tree-sitter paths into rust-lld and breaking the host build.
            #
            # symlinkJoin strips include/lib dirs but does NOT suppress a package's
            # propagated-build-inputs — musl.dev propagates musl/lib through.
            #
            # runCommand with an explicit bin-copy creates a truly isolated
            # derivation with no propagated deps and no include/lib dirs, so the
            # cc-wrapper hook has nothing to propagate.
            binOnly =
              pkg: name:
              pkgs.runCommand name { } ''
                mkdir -p $out/bin
                for f in ${pkg}/bin/*; do
                  ln -s "$f" "$out/bin/$(basename "$f")"
                done
              '';
            treeShitterBinOnly = binOnly pkgs.tree-sitter "tree-sitter-bin";
            muslDevBinOnly = binOnly pkgs.musl.dev "musl-dev-bin";
            nodejsBinOnly = binOnly pkgs.nodejs "nodejs-bin";
          in
          pkgs.mkShell {
            buildInputs = with pkgs; [
              stdenv.cc.cc
              sqlite
            ];
            packages = with pkgs; [
              rustToolchain
              rust-analyzer
              mold
              clang
              bun
              nodejsBinOnly
              treeShitterBinOnly
              muslDevBinOnly
            ];
            LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath (with pkgs; [ stdenv.cc.cc sqlite ])}:$LD_LIBRARY_PATH";
          };
      }
    );
}
