.orig x3000
;; Suggested Pseudocode (see PDF for explanation)
;;
;; int length = 0;
;; while (STRING[length] != 0) {
;;      length++;
;; }
;;
;;
;; int start = 0;
;; int end = length - 1;

;; while (start < end) {
;;      char temp = STRING[start];
;;      STRING[start] = STRING[end];
;;      STRING[end] = temp;

;;      start++;
;;      end--;
;;}

    AND R0, R0, 0       ; length = 0
    
    WHILESTART
    LD R1, STRING ; start of string
    ADD R1, R0, R1
    LDR R1, R1, 0       ; get string[length]
    BRZ WHILEEND
    ADD R0, R0, 1       ; length++
    BR WHILESTART
    WHILEEND
    
    AND R1, R1, 0       ; start = 0
    AND R2, R2, 0
    ADD R2, R2, R0
    ADD R2, R2, -1      ; end = length - 1
    
    WHILE2START
    AND R3, R3, 0
    ADD R3, R3, R2      ; get end
    NOT R3, R3
    ADD R3, R3, 1       ; -end
    AND R4, R4, 0
    ADD R4, R4, R1
    ADD R4, R4, R3      ; start - end
    BRZP WHILE2END
    LD R3, STRING
    ADD R3, R3, R1      ; R3 = addr of string[start]
    LDR R4, R3, 0       ; R4 = string[start]
    LD R5, STRING
    ADD R5, R5, R2      ; R5 = addr of string[end]
    LDR R6, R5, 0       ; R6 = string[end]
    STR R6, R3, 0       ; string[start] = string[end]
    STR R4, R5, 0       ; string[end] = string[start]
    ADD R1, R1, 1       ; start++
    ADD R2, R2, -1      ; end--
    BR WHILE2START
    WHILE2END
    
    HALT

;; Do not rename or remove any existing labels
;; You may change the value of LENGTH for debugging
STRING .fill x4000
.end

;; You may change the value of the string for debugging
.orig x4000
    .stringz "hello"
.end
