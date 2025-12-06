.orig x3000
;; Suggested Pseudocode (see PDF for explanation)
;;
;; int length = 0;
;;  while (BINARYSTRING[length] != 0) {
;;      length++;
;;  }
;;  int result = 0;
;;  for (int i = 0; i < length; i++) {
;;      result = result << 1;
;;      result += BINARYSTRING[i] - 48;
;;  }
;;  mem[mem[RESULTADDR]] = result;

    AND R0, R0, 0       ; length = 0
    
    WHILESTART
    LD R1, BINARYSTRING ; start of binstring
    ADD R1, R0, R1
    LDR R1, R1, 0       ; get binstring[length]
    BRZ WHILEEND
    ADD R0, R0, 1       ; length++
    BR WHILESTART
    WHILEEND
    
    AND R1, R1, 0       ; result = 0
    AND R2, R2, 0       ; i = 0
    FORIN
    AND R3, R3, 0
    ADD R3, R3, R0
    NOT R3, R3
    ADD R3, R3, 1       ; -length
    AND R4, R4, 0
    ADD R4, R4, R2
    ADD R4, R4, R3      ; i - length
    BRZP FOROUT
    ADD R1, R1, R1      ; result = result << 1
    LD R3, BINARYSTRING
    ADD R3, R3, R2      ; binstring+i
    LDR R3, R3, 0       ; binstring[i]
    LD R4, ASCIIDIG     ; 48
    NOT R4, R4
    ADD R4, R4, 1       ; -48
    ADD R3, R3, R4      ; binstring[i] - 48
    ADD R1, R1, R3      ; result = result + binstring[i] - 48
    ADD R2, R2, 1
    BR FORIN
    FOROUT
    STI R1, RESULTADDR
    HALT

;; Do not rename or remove any existing labels
;; You may change the value of LENGTH for debugging
BINARYSTRING .fill x5000
RESULTADDR .fill x4000
ASCIIDIG .fill 48
.end

.orig x4000
    .blkw 1
.end

;; You may change the value of the string for debugging
.orig x5000
    .stringz "11001"
.end
