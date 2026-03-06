# normalize-ecosystems/src/ecosystems/npm

npm/yarn/pnpm/bun ecosystem implementation.

`mod.rs` implements the `Ecosystem` trait for Node.js projects: detects `package.json`, resolves the preferred tool from lockfiles (pnpm-lock.yaml, yarn.lock, package-lock.json, bun.lock), fetches package metadata from `registry.npmjs.org`, lists declared dependencies, runs `npm audit --json` for vulnerability scanning, and delegates dependency tree and installed-version lookup to per-lockfile submodules. `lockfile_npm.rs`, `lockfile_pnpm.rs`, `lockfile_yarn.rs`, and `lockfile_bun.rs` each parse their respective lockfile format.
