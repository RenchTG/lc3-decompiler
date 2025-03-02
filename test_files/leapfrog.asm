;;=============================================================
;; CS 2110 - Spring 2025
;; Homework 4 - Leapfrog
;;=============================================================
;; Name: Philip Dobranowski
;;=============================================================

.orig x3000
;; Suggested Pseudocode (see PDF for explanation)
;;
;; inc = x0100
;; func_addr = starting address of function
;; copy_addr = func_addr + inc
;; start_copy = copy_addr
;; stop_addr = xFE00  
;; if (start_copy - stop_addr == 0):
;;      HALT
;; curr = mem[func_addr]
;; while (curr != 0) {
;;      curr = mem[func_addr]
;;      mem[copy_addr] = curr
;;      func_addr++
;;      copy_addr++
;; }
;; mem[copy_addr] = 0
;; PC = start_copy
;; .fill 0

;; YOUR CODE HERE

    LD R0, INC        ; R0 = inc = x0100
    
    ; In order to fetch current PC use a small func call and grab r7-2
    JSR FUNC_ADDR
    FUNC_ADDR
    AND R1, R1, 0
    ADD R1, R1, R7
    ADD R1, R1, -2    ; R1 = FUNC_ADDR
    
    ADD R2, R0, R1    ; R2 = copy_addr = func_addr + inc
    AND R3, R3, 0
    ADD R3, R3, R2    ; R3 = start_copy = copy_addr
    LD R4, STOP_ADDR  ; R4 = stop_addr = xFe00
    NOT R5, R4
    ADD R5, R5, 1     ; R5 = -stop_addr
    ADD R5, R3, R5    ; R5 = start_copy - stop_addr
    BRNP WHILEBEGIN
    HALT
    
    WHILEBEGIN
    LDR R5, R1, 0     ; R5 = mem[func_addr]
    BRZ WHILEEND
    STR R5, R2, 0     ; mem[copy_addr] = mem[func_addr]
    ADD R1, R1, 1     ; func_addr++
    ADD R2, R2, 1     ; copy_addr++
    BR WHILEBEGIN
    WHILEEND
    
    AND R5, R5, 0
    STR R5, R2, 0     ; mem[copy_addr] = 0
    JMP R3            ; PC = start_copy
    
    INC .fill x0100
    STOP_ADDR .fill xFE00
    .fill 0
.end