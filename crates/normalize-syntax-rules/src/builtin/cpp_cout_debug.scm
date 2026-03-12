# ---
# id = "cpp/cout-debug"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "std::cout/cerr found - consider using structured logging"
# languages = ["cpp"]
# allow = ["**/test/**", "**/tests/**", "**/examples/**"]
# enabled = false
# ---
#
# `std::cout` and `std::cerr` produce unstructured output. In
# production code, prefer a logging library (spdlog, glog, syslog)
# for leveled, structured, and configurable logging.
#
# ## How to fix
#
# ```cpp
# // Before
# std::cout << "Processing " << item.id << std::endl;
# // After
# SPDLOG_INFO("Processing {}", item.id);
# ```
#
# ## When to disable
#
# Disabled by default. CLI tools and examples that intentionally
# write to stdout are excluded.

((binary_expression
  left: (qualified_identifier
    scope: (namespace_identifier) @_ns
    name: (identifier) @_name)
  (#eq? @_ns "std")
  (#match? @_name "^(cout|cerr|clog)$")) @match)
