"""Entry point: runs a fixed calculation and prints results."""

from math_utils import Calculator


def main() -> None:
    calc = Calculator()
    print(calc.compute("add", 2, 3))
    print(calc.compute("mul", 4, 5))


if __name__ == "__main__":
    main()
