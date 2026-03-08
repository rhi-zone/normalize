"""Sample Starlark file for normalize fixture tests."""

load("@rules_cc//cc:defs.bzl", "cc_library", "cc_binary")
load("@rules_python//python:defs.bzl", "py_library")

# Constants
DEFAULT_COPTS = ["-Wall", "-Wextra"]
SUPPORTED_PLATFORMS = ["linux", "macos", "windows"]

def make_cc_library(name, srcs, hdrs = [], deps = []):
    """Create a C++ library target with standard options."""
    cc_library(
        name = name,
        srcs = srcs,
        hdrs = hdrs,
        deps = deps,
        copts = DEFAULT_COPTS,
    )

def make_test_suite(name, tests):
    """Bundle multiple test targets into a suite."""
    for test in tests:
        if test not in SUPPORTED_PLATFORMS:
            fail("Unknown platform: " + test)
    native.test_suite(
        name = name,
        tests = tests,
    )

def platform_select(linux_val, macos_val, default_val = None):
    """Return a select expression for platform-specific values."""
    result = select({
        "@platforms//os:linux": linux_val,
        "@platforms//os:macos": macos_val,
        "//conditions:default": default_val or linux_val,
    })
    return result

def filter_srcs(srcs, suffix):
    """Filter source files by suffix."""
    return [s for s in srcs if s.endswith(suffix)]

# Main library target
make_cc_library(
    name = "sample_lib",
    srcs = ["sample.cc"],
    hdrs = ["sample.h"],
)

cc_binary(
    name = "sample_bin",
    srcs = ["main.cc"],
    deps = [":sample_lib"],
    copts = platform_select(["-O2"], ["-O3"]),
)
