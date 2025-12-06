;; Suggested Pseudocode
;;
;;  int L = 0;
;;  int a = 1;
;;  int d = 3;
;;
;;  while (a <= N) {
;;      a = a + d;  // Compute the next perfect square
;;      d = d + 2;  // Increase the difference between squares
;;      L = L + 1;  // Increment L
;;  }
;;
;;  mem[mem[RESULT]] = L;
    
.orig x3000
    AND R0, R0, 0 ; L = 0
    AND R1, R1, 0 ; a = 0
    ADD R1, R1, 1 ; a = 1
    AND R2, R2, 0 ; d = 0
    ADD R2, R2, 3 ; d = 3
    
    WHILEBEGIN
    LD R3, N       ; get N
    NOT R3, R3
    ADD R3, R3, 1  ; -N
    AND R4, R4, 0
    ADD R4, R1, R3 ; a - N
    BRP WHILEEND   ; jump out if a > N
    ADD R1, R1, R2 ; a = a + d
    ADD R2, R2, 2  ; d = d + 2
    ADD R0, R0, 1  ; L = L + 1
    BR WHILEBEGIN
    
    WHILEEND
    STI R0, RESULT
    
    HALT

;; Do not rename or remove any existing labels
;; You may change the value of N for debugging
N .fill 20
RESULT .fill x4000
.end

.orig x4000
    .blkw 1
.end
