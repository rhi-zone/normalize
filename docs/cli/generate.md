# normalize generate

Generate code from API specifications.

## Usage

```bash
normalize generate <SPEC> [OPTIONS]
```

## Examples

```bash
# From OpenAPI spec
normalize generate openapi.yaml --output src/api/

# From GraphQL schema
normalize generate schema.graphql --lang typescript
```

## Supported Formats

| Format | Extensions |
|--------|------------|
| OpenAPI | `.yaml`, `.json` |
| GraphQL | `.graphql`, `.gql` |
| Protobuf | `.proto` |

## Options

- `--output <DIR>` - Output directory
- `--lang <LANG>` - Target language
- `--dry-run` - Show what would be generated
