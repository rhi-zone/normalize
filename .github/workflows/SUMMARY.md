# workflows

GitHub Actions workflow definitions. `ci.yml` runs tests and clippy on pull requests. `deploy-docs.yml` builds and deploys the documentation site. `normalize.yml` runs normalize checks. `release.yml` builds binaries and creates GitHub releases on `v*` tags. `publish.yml` publishes all publishable workspace crates to crates.io in topological dependency order on `v*` tags.
