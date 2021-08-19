## Pointers, part 1

Pointers are essential as they provide a way to interact with heap memory.

Prerequisites:
- none

### Note
The most important thing to decide about pointers is whether
pointers should be typed and instructions therefore not,
or whether pointers shouldn't be typed and instructions should.

Arguments **FOR** *typed*, **AGAINST** *untyped* pointers:
- is closer to higher-level languages
- less complexity in constructing the IR (you don't need to specify the type every time)
- LLVM IR also uses typed pointers
- more type safety

Arguments **FOR** *untyped*, **AGAINST** *typed* pointers:
- for emitting, we still need type information for read/write instructions,
therefore there's gonna be much more metadata than if type
information was embedded directly in the instructions
- less bitcasts between pointer types, which in the end are a no-op anyway
- LLVM IR is transitioning from typed pointers to untyped pointers

### Steps

1. Introduce a `Ptr` (or `Ptr(T)`) type.

2. Introduce a `Read` (or `Read(T)`) instruction, which pops a pointer type off the stack
and pushes a value of type `T` read from the address of the pointer.

3. Introduce a `Write` (or `Write(T)`) instruction, which pops a value of type `T` and a pointer off
the stack and writes the value into memory at the address of the pointer.

4. Introduce a `Offset` (or `Offset(T)`) instruction, which is pops an integer value `n` and a pointer `p`
off the stack and pushes a pointer equal to `p + (n * sizeof(T))` onto the stack.

5. Introduce a `OffsetConst(N)` (or `OffsetConst(N, T)`) instruction, which behaves equally to the `Offset`
instruction except the integer value `n` is specified at compile-time.
This instruction may or may not be actually useful, I'm not sure how much benefits it has over just doing `LdConst(N)` followed by `Offset`.