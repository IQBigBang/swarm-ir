## Structs (composite types)

Structs are another essential part of a language as there's
only so much you can do with integers and fp numbers.

This draft is split into two parts:
1. (this one) Struct pointers. Those are simpler because they are actually
pointers (= word-sized integers), and are passed around as arguments/return values/locals
easily.
2. The second one consists of flat structs. Those are harder, because WebAssembly only
allows integer/floating-point locals/arguments/returns, therefore any struct value
on the stack needs to be "decomposed" into these in some way.

### Note on typing
Because at the moment, pointers are untyped (`ptr`, not `ptr(int)`), there are two ways
to implement this draft type-wise:
1. Introduce a `structptr{...}` type, which `ptr` can be bitcast to/from (either explicitly or implicitly)
and make the struct-related instructions require this type
2. Introduce a `struct{...}` type, which won't be used as a value type directly, but
rather as a specifier for struct-related instructions.

The second implementation follows:

### Steps
1. Introduce a `Struct` type with fields - something like `Struct(Vec<Ty>)`.
**DONE - commit 08431213d848886dafb80227a7ac29b8e0185f29**

2. Add a check that no locals/arguments/returns are of this type, because that's
not possible at the moment (it may be enabled with part 2 of the structs proposal).
This also means that the `Struct` type cannot be used in Read/Write instructions. 
(However it *CAN* be used with the Offset instruction, see below)
**DONE - commit 5c9a7648230e1f57cff81feb1ce4f2c900744839**

3. Define alignment/size semantics for the struct type. For the algorithm, see section "Padding algorithm"
**DONE - commit 639cf6d32756a040d7715d6d0894f9f75b015ff2**

4. Make the `Offset` instruction support the `Struct` type, the semantics are unchanged.
*Probably already works correctly*

5. Finally, add a `GetFieldPtr(N, T)` instruction. It takes a constant offset `n` and a pointer
off the stack which points to a struct `T` and returns a pointer pointing to the `n`th field
of the struct `T`. **DONE - commit a09dc94d903bc3ee4840cec51edd3fbdf6df03d5**


### Padding algorithm
For types:
- `i32`, the size is 4 bytes and alignment is 4 bytes
- `f32`, the size is 4 bytes and alignment is 4 bytes
- `ptr`, the size and alignment is equal to `i32`
- `func`, the size and alignment is equal to `ptr`
- `struct{fields}`, the algorithm is:
    1. `size = 0, align = 1`
    2. For every field in the struct:
        - if `size` isn't divisible by this fields' alignment, add padding bytes to the `size`
        - `size +=` this fields' size
        - if this field's alignment is bigger than `align`, set `align` to this field's alignment

(technical note: as long as i32 and f32 are the only "basic" types, all structs
will have a size `field_count * 4` and alignment `4`.)