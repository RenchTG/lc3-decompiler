.orig x3000
    LD R0, BUF
    AND R3, R3, 0

    LOOP
    LDR R1, R0, 0
    LD R2, A
    NOT R2, R2
    ADD R2, R2, 1
    ADD R1, R1, R2
    BRnp NEXT
    
    ADD R0, R0, 1
    LDR R1, R0, 0
    LD R2, B
    NOT R2, R2
    ADD R2, R2, 1
    ADD R1, R1, R2
    BRnp NEXT
    
    ADD R0, R0, 1
    LDR R1, R0, 0
    LD R2, C
    NOT R2, R2
    ADD R2, R2, 1
    ADD R1, R1, R2
    BRnp NEXT
    
    LD R1, D
    STR R1, R0, 0
    ADD R3, R3, 1
    
    NEXT
    LD R1, MAX
    NOT R1, R1
    ADD R1, R1, 1
    ADD R1, R0, R1
    BRzp END
    
    ADD R0, R0, 1
    BR LOOP
    
    END
    LD R0, MAX
    STR R3, R0, 0
    HALT

A .fill x41
B .fill x42
C .fill x43
D .fill x44
BUF .fill x4000
MAX .fill x401D
.end

.orig x4000
    .fill x1
    .fill x2
    .fill x3
    .fill x41
    .fill x42
    .fill x43
    .blkw 23
    .fill x64
.end
