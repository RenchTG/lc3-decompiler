; Make null-terminated string of letters uppercase

; R0 holds i
; R1 holds char_addr
; R2 holds char

.orig x3000
and r0, r0, 0  ; i = 0

LOOP           ; while (true)
lea r1, STRING ; r1 = address of string[0]
add r1, r1, r0 ; r1 = address of string[i] = char_addr
ldr r2, r1, 0  ; r2 = mem[char_addr]. Set cc for char
brz BREAK      ; if (char == 0) break
ld r3, MASK    ; r3 = 0x20
not r3, r3     ; r3 = ~0x20
and r2, r2, r3 ; char = char & ~0x20
str r2, r1, 0  ; mem[char_addr] = char
add r0, r0, 1  ; i++
br LOOP

BREAK
halt

MASK .fill x20
STRING .stringz "bubba"
; ... If I run this program and look at these characters in memory, you'll
; notice they have changed to "BUBBA" instead! So it works.
.end