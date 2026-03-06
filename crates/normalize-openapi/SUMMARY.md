# normalize-openapi

OpenAPI client code generation for multiple languages.

Defines the `OpenApiClientGenerator` trait and a global plugin registry (`register`, `get_generator`, `list_generators`). Built-in generators produce TypeScript (fetch), Python (urllib), and Rust (ureq) clients from an OpenAPI JSON `serde_json::Value`. Each generator emits language-appropriate type definitions from `components/schemas` and method stubs from `paths`. Custom generators can be registered at startup via `register()`.
