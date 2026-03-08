# Sample Nix expression file

{ pkgs ? import <nixpkgs> {} }:

let
  version = "1.0.0";

  greet = name: "Hello, ${name}!";

  factorial = n:
    if n <= 1
    then 1
    else n * factorial (n - 1);

  filterEvens = lst:
    builtins.filter (x: builtins.div x 2 * 2 == x) lst;

  makePackage = { name, src, buildInputs ? [] }:
    pkgs.stdenv.mkDerivation {
      inherit name src buildInputs;
      installPhase = ''
        mkdir -p $out/bin
        cp $src $out/bin/${name}
      '';
    };

  utils = {
    join = sep: lst:
      builtins.concatStringsSep sep lst;

    mapValues = f: attrs:
      builtins.mapAttrs (_: v: f v) attrs;

    defaultTo = default: value:
      if value == null then default else value;
  };

in {
  inherit greet factorial filterEvens;
  inherit utils;

  samplePackage = makePackage {
    name = "sample";
    src = ./src;
    buildInputs = with pkgs; [ bash coreutils ];
  };

  message = greet "World";
  fact5 = factorial 5;
  evens = filterEvens [ 1 2 3 4 5 6 ];
}
