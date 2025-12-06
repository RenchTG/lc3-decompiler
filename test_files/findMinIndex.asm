.orig x3000
;; Suggested Pseudocode (see PDF for explanation)
;;
;;  int minIndex = 0;
;;  int minValue = ARRAY[0];
;;  for (int i = 1; i < LENGTH; i++) {
;;      if (ARRAY[i] < minValue) {
;;          minValue = ARRAY[i];
;;          minIndex = i;
;;      }
;;  }
;;  mem[mem[RESULT]] = minIndex;
    
    AND R0, R0, 0   ; R0 = minIndex = 0
    LDI R1, ARRAY   ; R1 = minValue = ARRAY[0]
    AND R2, R2, 0
    ADD R2, R2, 1   ; R2 = i = 1
    
    FORSTART
    AND R3, R3, 0
    ADD R3, R2, 0   ; R3 = i
    LD R4, LENGTH
    NOT R4, R4
    ADD R4, R4, 1   ; R4 = -length
    ADD R3, R3, R4  ; R3 = i - length
    BRZP FOREND
    
    LD R3, ARRAY
    ADD R3, R3, R2  ; R3 = addr of array[i]
    LDR R3, R3, 0   ; R3 = array[i]
    AND R4, R4, 0
    ADD R4, R4, R1
    NOT R4, R4
    ADD R4, R4, 1   ; R4 = -minValue
    AND R5, R5, 0
    ADD R5, R3, R4  ; R5 = array[i] - minValue
    BRZP NOTIF
    AND R1, R1, 0
    ADD R1, R1, R3  ; minValue = array[i]
    AND R0, R0, 0
    ADD R0, R0, R2  ; minIndex = i
    NOTIF
    ADD R2, R2, 1
    BR FORSTART
    FOREND
    STI R0, RESULT
    HALT

;; Do not rename or remove any existing labels
;; You may change the value of LENGTH for debugging
RESULT .fill x4000
ARRAY .fill x5000
LENGTH .fill 5
.end

.orig x4000
    ANSWER .blkw 1
.end

;; You may change these values for debuggin
.orig x5000
    .fill -1
    .fill 2 
    .fill 7 
    .fill 3 
    .fill -8 
.end
