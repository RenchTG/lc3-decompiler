; In the LC-3 calling convention, the following process is performed for every
; function call f(arg1, arg2, ..., argn).
;
; Definitions of terms used below:
; * The "caller" is the assembly code that makes the f(...) call
; * The "callee" is the assembly code for f() itself
; * "Pushing Rx onto the stack" means doing:
;       ADD R6, R6, -1
;       STR Rx, R6, 0
; * "Popping Ry off the stack" means doing:
;       LDR Ry, R6, 0
;       ADD R6, R6, 1
;
;===> Step 1: Caller pushes arguments (in reverse order!) onto stack
;
;         ----------  -------\
;  R6 -->|  arg 1   |         |
;         ----------          |
;        |  arg 2   |         |____ Step 1: Pushed by caller
;         ----------          |
;        |   ...    |         |
;         ----------          |
;        |  arg n   |         |
;         ----------  --------/
;
;===> Step 2: Caller executes a JSR or a JSRR to jump into callee code
;     -> Why JSR(R)? The JSR(R) instruction sets R7 to the return address
;
; At this point, control is handed over to the callee code.
;
;===> Step 3: Callee allocates a location on the stack for the return value,
;             then pushes R7 onto the stack, and then pushes R5 onto the stack.
;     -> The location on the stack for the return value should still be
;        allocated even if the callee is a void function (i.e., if the callee
;        doesn't return anything).
;
;         ---------- ----------
;  R6 -->| saved R5 |          \
;         ----------           |
;        | saved R7 |          |_____ Step 3: Pushed by callee
;         ----------           |
;        | ret. val |          /
;         ----------  ---------
;        |  arg 1   |
;         ----------
;        |  arg 2   |
;         ----------
;        |   ...    |
;         ----------
;        |  arg n   |
;         ----------
;
;===> Step 4: Callee sets its frame pointer (R5) to R6-1 (that is, the slot
;             above "saved R5")
;
;         ----------
;  R5 -->|   ???    |
;         ----------
;  R6 -->| saved R5 |
;         ----------
;        | saved R7 |
;         ----------
;        | ret. val |
;         ----------
;        |  arg 1   |
;         ----------
;        |  arg 2   |
;         ----------
;        |   ...    |
;         ----------
;        |  arg n   |
;         ----------
;
;===> Step 5: Callee allocates space for local variables
;     -> These are locations on the stack designated to hold temporary values.
;        For example, if your callee function does a big computation that
;        requires more temporary values than can be held in just R0-R4.
;
;         ---------- ----------
;  R6 -->| local m  |          \
;         ----------           |
;        |   ...    |          |
;         ----------           |____ Step 5: Pushed by callee
;        | local 2  |          |
;         ----------           |
;  R5 -->| local 1  |          /
;         ---------- ----------
;        | saved R5 |
;         ----------
;        | saved R7 |
;         ----------
;        | ret. val |
;         ----------
;        |  arg 1   |
;         ----------
;        |  arg 2   |
;         ----------
;        |   ...    |
;         ----------
;        |  arg n   |
;         ----------
;
;===> Step 6: Callee backs up any registers R0-R4 that it will overwrite.
;     -> In the diagram below, I save all of R0-R4, but only the ones the
;        callee overwrites would be needed.
;     -> The order of pushing does not matter as long as it is the reverse of
;        the order of popping (Step 10)
;
;         ---------- ----------
;  R6 -->| saved R4 |          |
;         ----------           |
;        | saved R3 |          |
;         ----------           |
;        | saved R2 |          |
;         ----------           |____ Step 6: Pushed by callee
;        | saved R1 |          |
;         ----------           |
;        | saved R0 |          /
;         ---------- ----------
;        | local m  |
;         ----------
;        |   ...    |
;         ----------
;        | local 2  |
;         ----------
;  R5 -->| local 1  |
;         ----------
;        | saved R5 |
;         ----------
;        | saved R7 |
;         ----------
;        | ret. val |
;         ----------
;        |  arg 1   |
;         ----------
;        |  arg 2   |
;         ----------
;        |   ...    |
;         ----------
;        |  arg n   |
;         ----------
;
;===> Step 7: Callee loads arguments from stack into registers (if needed)
;     -> For the programmer's sanity, this should be done using R5.
;     -> This could be mid-computation (mid-Step 8) too, but I mention it as a
;        discrete step here for simplicity.
;
;===> Step 8: Callee does its computation, temporarily putting the return value
;             in some register
;
;===> Step 9: Callee stores return value in stack
;     -> For the programmer's sanity, this should be done using R5.
;     -> If the callee is a void function (returns nothing), then this step can
;        be skipped.
;
;         ----------
;  R6 -->| saved R4 |
;         ----------
;        | saved R3 |
;         ----------
;        | saved R2 |
;         ----------
;        | saved R1 |
;         ----------
;        | saved R0 |
;         ----------
;        | local m  |
;         ----------
;        |   ...    |
;         ----------
;        | local 2  |
;         ----------
;  R5 -->| local 1  |
;         ----------
;        | saved R5 |
;         ----------
;        | saved R7 |
;         ----------
;        | ret. val | <--- here
;         ----------
;        |  arg 1   |
;         ----------
;        |  arg 2   |
;         ----------
;        |   ...    |
;         ----------
;        |  arg n   |
;         ----------
;
;===> Step 10: Callee pops everything above return value
;     -> This includes restoring original values of R5, R7, and any of R0-R4
;        saved as "locals"
;
;         ----------  ---------
;       X| saved R4 |X         \
;       X ---------- X         |
;       X| saved R3 |X         |
;       X ---------- X         |
;       X| saved R2 |X         |
;       X ---------- X         |
;       X| saved R1 |X         |
;       X ---------- X         |
;       X| saved R0 |X         |
;       X ---------- X         |
;       X| local m  |X         |
;       X ---------- X         |
;       X|   ...    |X         |
;       X ---------- X         |
;       X| local 2  |X         |
;       X ---------- X         |____ Step 10: Popped by callee
;       X| local 1  |X         |
;       X ---------- X         |
;       X| saved R5 |X         |
;       X ---------- X         |
;       X| saved R7 |X         /
;         ---------- ----------
;  R6 -->| ret. val |
;         ----------
;        |  arg 1   |
;         ----------
;        |  arg 2   |
;         ----------
;        |   ...    |
;         ----------
;        |  arg n   |
;         ----------
;
;===> Step 11: Callee returns with `ret`
;     -> `ret` is shorthand provided by the assembler for writing `jmp r7`
;
; Control now returns to the caller.
;
;===> Step 12: Caller pops return value off the stack
;
;         ---------- <---------.
;       X| ret. val |X         |____ Step 12: Popped by caller
;         ---------- <---------'
; R6 --> |  arg 1   |
;         ----------
;        |  arg 2   |
;         ----------
;        |   ...    |
;         ----------
;        |  arg n   |
;         ----------
;
;===> Step 13: Caller pops arguments off the stack
;     -> This will return the stack pointer (R6) to where it was was before
;        Step 1.
;
;         ---------- ----------
;       X|  arg 1   |X         \
;       X ---------- X         |
;       X|  arg 2   |X         |____ Step 13: Popped by caller
;       X ---------- X         |
;       X|   ...    |X         |
;       X ---------- X         |
;       X|  arg n   |X         /
;         ----------  ---------
;  R6 -->|   ???    |<--- This was the top of the stack before Step 1. Who
;         ----------      knows what it is, who cares.
;
; --------------------------------------------------------------
;
; For reference, here are all these steps in one giant picture:
;
;         ---------- ---------. --------.
;  R6 -->| saved R4 |          \         \
;         ----------           |         |
;        | saved R3 |          |         |
;         ----------           |         |
;        | saved R2 |          |         |
;         ----------           |         |
;        | saved R1 |          |         |
;         ----------           |         |
;        | saved R0 |          |         |
;         ----------           |         |
;        | local m  |          |         |
;         ----------           |         |
;        |   ...    |          |         |
;         ----------           |         |
;        | local 2  |          |         |
;         ----------           |____ Steps 3, 5 and 6: Pushed by callee
;  R5 -->| local 1  |          |         |
;         ----------           |         |____ Step 10: Popped by callee
;        | saved R5 |          |         |
;         ----------           |         |
;        | saved R7 |__________|_________/
;         ---------- ----------+--------,
;        | ret. val | _________/         \
;         ----------  -------.           |
;        |  arg 1   |         \          |
;         ----------          |          |
;        |  arg 2   |         |____ Step 1: Pushed by caller
;         ----------          |          |
;        |   ...    |         |          |____ Steps 12 & 13: Popped by caller
;         ----------          |          |
;        |  arg n   | ________/          /
;         ----------  ------------------'
;
; The range of locations on the stack corresponding to this function call
; (drawn directly above) is called the **STACK FRAME** for this function call.
;
; I have annotated the code below with these steps, specifically from the
; perspective of the fib() call in the entry point code on line 323 below.
;
; ----------------------------------------------------------------------------

; Entry point of our program (which does one thing right now: call fib(n))
.orig x3000
ld r6, STACK_INIT_ADDR
; For readability in LC3Tools, load argument n to fib(n) from memory (below)
ld r0, N ; r0 = n (input)

; ABOVE CODE GUARANTEES ARGUMENT IS IN R0

;===> Step 1: Caller pushes arguments (in reverse order!) onto stack
add r6, r6, -1  ; push R0 (last argument) onto the stack
str r0, r6, 0

;===> Step 2: Caller executes a JSR or a JSRR to jump into callee code
ld r3, FIB_ADDR
jsrr r3

;===> Step 12: Caller pops return value off the stack
ldr r1, r6, 0 ; pop result of fib(n) into r1
add r6, r6, 1

;===> Step 13: Caller pops arguments off the stack
add r6, r6, 1 ; get rid of argument

; BELOW CODE EXPECTS RESULT IS IN R1

st r1, FIBN ; For readability in LC3Tools, store result in memory (below)
halt

N .fill 8
FIBN .blkw 1
STACK_INIT_ADDR .fill xF000
FIB_ADDR .fill FIB
.end

; ----------------------------------------------------------------------------

; This assembly implements the following pseudocode:
;     int fib(int n) {
;         if (n <= 1) {
;             return n;
;         } else {
;             return fib(n-1) + fib(n-2);
;         }
;     }
.orig x4000
FIB
    ;===> Step 3: Callee allocates a location on the stack for the return value...
    add r6, r6, -1 ; fill this in later (return value)
    ;===> Step 3: ...then pushes R7 onto the stack...
    add r6, r6, -1  ; push return addres
    str r7, r6, 0
    ;===> Step 3: ...then pushes R5 onto the stack.
    add r6, r6, -1  ; back up R5
    str r5, r6, 0

    ;===> Step 4: Callee sets its frame pointer (R5) to R6-1 (that is, the slot
    ;             above "saved R5")
    add r5, r6, -1 ; set frame pointer

    ;===> Step 5: Callee allocates space for local variables
    add r6, r6, -2 ; allocate 2 local variables (FOR EXAMPLE)

    ;===> Step 6: Callee backs up any registers R0-R4 that it will overwrite
    ; Order doesn't matter as long as I pop them in the opposite order below
    add r6, r6, -1  ; back up R0
    str r0, r6, 0
    add r6, r6, -1  ; back up R1
    str r1, r6, 0
    add r6, r6, -1  ; back up R2
    str r2, r6, 0
    add r6, r6, -1  ; back up R3
    str r3, r6, 0

    ;===> Step 7: Callee loads arguments from stack into registers (if needed)
    ldr r0, r5, 4 ; load first argument into r0

    ;===> Step 8: Callee does its computation, temporarily putting the return
    ;             value in some register (in this case, R1, arbitrarily)

    ; COMPUTATION BEGINS (It wants argument n in R0)
    ;                    (Overwrites R0, R1, R2, R3)

    ; Check for base case
    add r2, r0, -1  ; r2 = n-1
    brp SKIP ; if (n-1 <= 0) aka if (n <= 1)
        add r1, r0, 0 ; r1 = n, aka return n
        br TEARDOWN
    SKIP

    ; Prepare argument for recursive call #1
    add r0, r2, 0  ; r0 = n-1

    ; Push n-1 onto the stack
    add r6, r6, -1
    str r0, r6, 0
    jsr FIB  ; Call fib(n-1)
    ; pop return value of fib(n-1) off stack
    ldr r3, r6, 0  ; r3 = fib(n-1)
    add r6, r6, 1
    ; ...and pop argument n-1 too
    add r6, r6, 1

    ; Prepare argument for recursive call #2
    add r0, r0, -1 ; r0 = n-2

    ; Push n-2 onto the stack
    add r6, r6, -1
    str r0, r6, 0
    jsr FIB ; Call fib(n-2)
    ; pop return value of fib(n-2) off stack
    ldr r1, r6, 0  ; r1 = fib(n-2)
    add r6, r6, 1
    ; ...and pop argument n-2 too
    add r6, r6, 1

    ; Final result computation for recursive case
    add r1, r3, r1 ; r1 = fib(n-1) + fib(n-2)

    ; COMPUTATION ENDS (return value is in R1)

    TEARDOWN
    ;===> Step 9: Callee stores return value in stack
    str r1, r5, 3  ; store return value in stack

    ;===> Step 10: Callee pops everything above return value
    ldr r3, r6, 0  ; pop r3
    add r6, r6, 1
    ldr r2, r6, 0  ; pop r2
    add r6, r6, 1
    ldr r1, r6, 0  ; pop r1
    add r6, r6, 1
    ldr r0, r6, 0  ; pop r0
    add r6, r6, 1

    ; pop both locals too
    add r6, r6, 2

    ; Pop caller's frame pointer
    ldr r5, r6, 0
    add r6, r6, 1
    ; Pop return address
    ldr r7, r6, 0
    add r6, r6, 1

    ;===> Step 11: Callee returns with `ret'
    ret
.end
