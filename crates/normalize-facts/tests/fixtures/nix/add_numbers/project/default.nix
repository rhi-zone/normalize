let
  mathUtils = import ./math_utils.nix;
in
{
  sum = mathUtils.add 2 3;
  product = mathUtils.multiply 4 5;
}
