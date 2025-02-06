# xml

Personal arena-based XML-adjacent parser implementation.

Does not strictly follow any specification; be wary of parser mismatch vulnerabilities ;)

## Features

- [x] Numeric character references: decimal & hexadecimal
- [x] Character entity references: limited to `qout`, `amp`, `apos`, `lt`, `gt`
- [ ] Support for [Document Type Definitions](https://en.wikipedia.org/wiki/Document_type_definition): not planned

## Potential Future Additions

- One big hashmap (or other data structure) for attributes instead of storing them on each node?
