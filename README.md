# Network Parser RS

#### source code can be found at https://github.com/Lazox12/network-parser-rs

`network_parser_rs` provides a procedural macro `make_struct!` designed to make it easy to define and parse network protocols and binary data structures. 

With `make_struct!`, you can define custom bit-level and byte-level fields, skip padding, use conditional parsing, and dynamically size arrays based on previously parsed fields.

## Syntax Overview

The syntax is designed to be highly readable while closely resembling Rust struct definitions. 

Here is an example showing the full capabilities of `make_struct!`:

```rust
make_struct! {
    #[derive(Debug, Clone)]
    MyStruct,
    field1: u3,
    consume(5) // skips 5 bits
    field2: u5,
    field3: Vec<u8>;field2, // dynamically sized vector of length defined in field2
    field4: CStr,
    if peek(8) == 15 {
        consume(4) // conditionally skip 4 bits
        field5: u16,
        // field5 will be parsed as Option<u16>, filled only if the condition is met
    }
    field6: [u8; 4],
}
```

## Explanation of Features

### 1. Basic Integer Types
You can define unsigned and signed integer fields using `uX` or `iX` where `X` represents the bit size. This is useful for bit-packing non-standard integers directly from the stream.
* Example: `field: u3` (3-bit unsigned integer)
* Example: `field: i12` (12-bit signed integer)

### 2. Skipping / Consuming Data
If there is padding or reserved bits/bytes in the stream that you need to ignore, use `consume(expr)`.
* `consume(5)`: Skips a literal amount of bits.
* `consume(my_func())`: Skips an amount of bits determined by a dynamic expression.
* **Conditionally skipping**: You can place `consume()` directives inside `if` blocks to skip bits only if a certain condition is met.

### 3. Dynamic Collections (`Vec`)
Vectors can be dynamically sized based on fields that have already been parsed in the struct.
* `Vec<u8>`: Parses until EOF or exhaustion.
* `Vec<u8>; size_field`: Parses exactly `size_field` number of bytes. The identifier following the `;` references an earlier field.

### 4. C Strings (`CStr`)
Used for reading null-terminated strings from the byte stream.
* `CStr`: Reads characters until a null (`\0`) byte is encountered.
* `CStr; size_field`: Reads a string bounded by `size_field`.

### 5. Fixed-Sized Arrays
Standard fixed-sized byte arrays are fully supported.
* Example: `[u8; 4]` will unconditionally parse exactly 4 bytes.

### 6. Conditional Parsing (`if`)
You can define fields that are only parsed if a specific condition evaluates to true at runtime. Fields defined inside an `if` block will automatically be wrapped in an `Option<T>` in the generated Rust struct.

```rust
if peek(8) == 15 {
    field5: u16,
    field7: u8,
}
```
In the above example:
* The parser will evaluate the expression `peek(8) == 15`. The `peek(bits)` closure is automatically injected into the parsing scope, allowing you to look ahead in the bit stream without advancing the cursor. Local field variables (e.g., `field1 == 7`) are also available in scope for both serialization and deserialization.
* If true, it parses `field5` as a `u16` and `field7` as a `u8`, populating them as `Some(...)`.
* If false, `field5` and `field7` are skipped and set to `None`.

### 7. Struct Attributes
You can easily add standard Rust attributes (like `#[derive(Debug, Clone, PartialEq)]`) to your generated structs by placing them immediately before the struct name.

### 8. Custom Types (Nested Structs)
You can seamlessly use other structs as types for your fields, provided they implement the `NetworkParse` trait. Because `make_struct!` automatically implements `NetworkParse` for any struct it generates, you can effortlessly nest dynamically sized network structures directly inside each other!


## An Example
this example shows the complete definition of ethernetII frame including IEEE 802.1Q(VLAN). 

```rust
use network_parser_rs::make_struct;

make_struct!{ ethernetII_frame,
    dst_mac:[u8; 6],
    src_mac:[u8; 6],
    if peek(16) == 0x8100 {
        consume(16)
        pcp: u3,
        dei: bool,
        vid: u12,
    }
    ethertype: u16,
    payload: Vec<u8>;
    
}

let data: Vec<u8> = vec![];
let frame = ethernetII_frame::try_from(data).unwrap();
```

## Embeded development
for parsing data this library uses from_be_bytes and to_be_bytes fuctions
so embeded development is fully supported (tested on stm32h573)