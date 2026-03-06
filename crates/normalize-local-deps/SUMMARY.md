# normalize-local-deps

Local dependency discovery for language ecosystems — finds installed packages on disk (node_modules, site-packages, GOPATH, cargo registry, etc.) so they can be indexed for symbol lookup.

Defines the `LocalDeps` trait with all-defaulted methods (opt-in overrides). Key types: `ResolvedPackage`, `LocalDepSource`, `LocalDepSourceKind` (Flat/Recursive/NpmScoped/Maven/Gradle/Cargo/Deno). Implements the trait for ~10 ecosystems: Python, JavaScript, TypeScript, Rust, Go, Java, Kotlin, Scala, C, C++. Separate from syntax analysis (`normalize-languages`) and remote registry querying; answers "where are locally-installed packages on this machine?"
