# Project TODOs

The project structure has been updated, separating the proc macro into `network_parser_rs_macro`, and establishing the `NetworkParse` trait. Here is the remaining work needed to fully implement the parser:

## 1. Struct Code Generation
Currently, the macro only prints the struct name or attempts to output incomplete tokens. We need to generate a complete Rust `struct` definition.
- [ ] Implement `ToTokens` (or formatting logic via `quote!`) for the custom AST nodes (`Field`, `Type`, `OptionType`).
- [ ] Map custom bit-width integers (e.g., `u3`, `i12`) to the smallest standard fitting Rust type (e.g., `u8`, `u16`) for the struct fields.
- [ ] Ensure that `consume` rows are omitted from the struct's fields, as they only represent padding/skips during parsing.
- [ ] Wrap conditionally parsed fields (`if` blocks) in `Option<T>` within the struct definition.

## 2. Parsing Implementation (`TryFrom<Vec<u8>>`)
Generate the parsing logic to implement `TryFrom<Vec<u8>> for MyStruct`.
- [ ] Setup a parsing state (cursor/offset) that can track bit-level and byte-level progression.
- [ ] Generate field extraction logic: Read the correct number of bits/bytes and cast to the struct field's type.
- [ ] Implement `consume(N)` and `consume(expr)` to correctly advance the parsing cursor without storing data.
- [ ] Support dynamic vector/string parsing where the length is derived from a previously parsed field (`Vec<u8>; field_name`).
- [ ] Translate `if` conditions into runtime `if` statements that populate `Some(value)` or default to `None`.

## 3. Pointer Parsing Implementation (`From<*mut u8>`)
Generate the parsing logic from a raw pointer to implement `From<*mut u8> for MyStruct`.
- [ ] Implement unsafe pointer arithmetic and reading logic equivalent to the `Vec<u8>` parsing approach.

## 4. Serialization Implementation (`Into<Vec<u8>>`)
Generate the logic to serialize the struct back into bytes.
- [ ] Write each field sequentially into a new `Vec<u8>`.
- [ ] Handle bit-packing for custom bit-width integers (e.g., writing a `u3` followed by a `u5` into a single byte).
- [ ] Handle `consume` padding by writing zeroed bits/bytes where required.
- [ ] Write conditionally populated fields (`Option<T>`) if `Some`, otherwise handle the empty space.

## 5. Implement `NetworkParse`
- [ ] Generate the marker trait implementation `impl NetworkParse for MyStruct {}` once the required bounds (`TryFrom`, `From`, `Into`) are fully implemented.
