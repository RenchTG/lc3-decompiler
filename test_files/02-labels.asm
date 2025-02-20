; Instead of writing PC offsets below, we write labels

; Compute 0+1+2+...+N

; R0 holds i
; R1 holds sum

.orig x3000 ; user programs in LC-3 start at address 0x3000
and r0, r0, 0  ; i = 0
and r1, r1, 0  ; sum = 0

LOOP
add r1, r1, r0 ; sum = sum + i
add r0, r0, 1  ; i++
ld r2, N       ; r2 = N
not r2, r2
add r2, r2, 1  ; r2 = -N
add r2, r0, r2 ; set cc for i-N
brnz LOOP      ; repeat if i <= N

st r1, SUM     ; store sum in memory
halt
SUM .blkw 1    ; reserve 1 word for sum
N .fill 3      ; N=3
.end
