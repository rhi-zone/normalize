# fixtures/typescript

TypeScript fixture files for `.scm` query tests and extraction fixtures.

- `sample.ts` — defines a `Logger` interface and `FileLogger` class implementing it, plus `formatPath` and `groupBy` exported functions; imports `EventEmitter` from `events` and `path` as a namespace.
- `classes/` — extraction fixture: two classes (`Animal`, `Dog`) with inheritance and a `createDog` function with `input.ts` + `expected.json`; verifies class symbol extraction.
