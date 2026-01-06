#!/usr/bin/env python3
"""Example argparse CLI for testing help output parsing."""

import argparse


def main():
    parser = argparse.ArgumentParser(
        prog="example",
        description="An example CLI tool for testing",
    )

    parser.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Enable verbose output"
    )
    parser.add_argument(
        "-c", "--config",
        metavar="FILE",
        help="Config file path"
    )
    parser.add_argument(
        "-p", "--port",
        type=int,
        default=8080,
        help="Port number (default: %(default)s)"
    )

    subparsers = parser.add_subparsers(dest="command", help="Commands")

    # Build command
    build_parser = subparsers.add_parser("build", help="Build the project")
    build_parser.add_argument(
        "-r", "--release",
        action="store_true",
        help="Build in release mode"
    )
    build_parser.add_argument(
        "-t", "--target",
        metavar="DIR",
        help="Target directory"
    )

    # Run command
    run_parser = subparsers.add_parser("run", help="Run the project")
    run_parser.add_argument(
        "args",
        nargs="*",
        help="Arguments to pass"
    )

    # Clean command
    subparsers.add_parser("clean", help="Clean build artifacts")

    args = parser.parse_args()
    print("CLI parsed successfully")


if __name__ == "__main__":
    main()
