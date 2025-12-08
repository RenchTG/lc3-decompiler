# LC-3 Decompiler

A decompiler for LC-3 (Little Computer 3) assembly architecture. Converts the .TEXT and .SYMBOL sections from an object file into high-level C-like pseudocode.

Heavily inspired and aided by https://www.backerstreet.com/decompiler/introduction.php. The best introduction and explanation of decompiler design/algorithms I could find online.

## Features

- **Disassembly** - Converts LC-3 object files to readable assembly with symbol resolution
- **Control Flow Graph Construction** - Constructs a control flow graph from basic blocks
- **Control Flow Analysis** - Analyzes the control flow graph to identify loops and conditionals
- **Data Flow Analysis** - Computes liveness information for variables and propagates and combines expression
- **Control Flow Structuring** - Creates structured control flow statements to remove gotos. Supports do-while, while, for, if-else and compound conditions (&&, ||).
- **Calling Convention Analysis** - Simplifies functions to remove and condense stack frame management and parameter passing.
- **Linearization and Code Output** - Converts the structured control flow into linearized C-like pseudocode.

## Installation

```bash
git clone https://github.com/RenchTG/lc3-decompiler.git
cd lc3-decompiler
cargo build --release
```

## Usage

```bash
# Basic decompilation
cargo run -- <file.obj>

# Decompile with calling convention analysis
cargo run -- <file.obj> -c

# Specify entry point by symbol name or address (defaults to 0x3000)
cargo run -- <file.obj> -e <SYMBOL NAME or HEX ADDRESS>

# Enable debug output to see transformations
cargo run -- <file.obj> -d
```

## Examples

### Example 1: Find Minimum Index

**Input:** `findMinIndex.obj` - finds the index of the minimum element in an array

```bash
cargo run -- test_files/findMinIndex.obj
```

**Reference Pseudocode:**
```
int minIndex = 0;
int minValue = ARRAY[0];
for (int i = 1; i < LENGTH; i++) {
    if (ARRAY[i] < minValue) {
        minValue = ARRAY[i];
        minIndex = i;
    }
}
mem[mem[RESULT]] = minIndex;
```

**Output:**
```c
var0 = 0;
var1 = *(*ARRAY);
for (var2 = 1; var2 < *LENGTH; var2++) {
    var3 = *(*ARRAY + var2);
    if (var3 < var1) {
        var1 = var3;
        var0 = var2;
    }
}
**RESULT = var0;
```

### Example 2: Caesar Cipher Encryption

**Input:** `caesarCipher.obj` - encrypts a string using Caesar cipher

```bash
cargo run -- test_files/caesarCipher.obj -c -e ENCRYPT
```

**Reference Pseudocode:**
```
ENCRYPT(String str, int k) {
    int length = 0;
    while (str[length] != 0) {
        length++;
    }
    for (int i = 0; i < length; i++) {
        char = str[i];
        if (char >= 'a' && char <= 'z') {
            char = char - 'a';
            char = MOD(char + k, 26);
            char = char + 'a';
        } else if (char >= 'A' && char <= 'Z') {
            char = char - 'A';
            char = MOD(char + k, 26);
            char = char + 'A';
        }
        str[i] = char;
    }
}
```

**Output:**
```c
var0 = 0;
while (*(param0 + var0) != 0) {
    var0++;
}
for (var1 = 0; var1 < var0; var1++) {
    var3 = *(param0 + var1);
    if (var3 >= *ASCIILOWERA && var3 <= *ASCIILOWERA + *ALPHABETLEN) {
        var3 = var3 - *ASCIILOWERA;
        var3 = MOD(var3 + param1, *ALPHABETLEN) + *ASCIILOWERA;
    } else if (var3 >= *ASCIIUPPERA && var3 <= *ASCIIUPPERA + *ALPHABETLEN) {
        var3 = var3 - *ASCIIUPPERA;
        var3 = MOD(var3 + param1, *ALPHABETLEN) + *ASCIIUPPERA;
    }
    *(param0 + var1) = var3;
}
return var0;
```

### Example 3: Fibonacci

**Input:** `fib.obj` - calculates the nth Fibonacci number

```bash
cargo run -- test_files/fib.obj -c -e FIB
```

**Reference Pseudocode:**
```
int fib(int n) {
    if (n <= 1) {
        return n;
    } else {
        return fib(n-1) + fib(n-2);
    }
}
```

**Output:**
```c
var0 = param0;
var2 = var0 - 1;
if (var2 > 0) {
    var0 = var2;
    var3 = FIB(var0);
    var1 = var3 + FIB(var0 - 1);
} else {
    var1 = var0;
}
return var1;
```
