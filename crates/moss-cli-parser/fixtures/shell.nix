# Nix shell for generating CLI --help fixtures from different ecosystems.
#
# Usage:
#   nix-shell
#   ./generate.sh
#
# This provides:
#   - Rust/Cargo for clap fixtures
#   - Python with argparse (stdlib) and click
#   - Node.js with npm for commander.js
{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # Rust ecosystem
    rustc
    cargo

    # Python ecosystem
    (python3.withPackages (ps: with ps; [
      click
    ]))

    # Node.js ecosystem
    nodejs
  ];

  shellHook = ''
    echo "CLI parser fixture generation shell"
    echo ""
    echo "Available ecosystems:"
    echo "  - Rust/clap: cargo build --release (in clap/)"
    echo "  - Python/argparse: python argparse/example.py --help"
    echo "  - Python/click: python click/example.py --help"
    echo "  - Node/commander: node commander/example.js --help"
    echo ""
    echo "Run ./generate.sh to regenerate all fixtures"
  '';
}
