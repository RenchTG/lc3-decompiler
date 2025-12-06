.orig x3000
    ;; You do not need to write anything here
    LD R6, STACK_PTR
    
    ;; Pushes arguments (string to encrypt, shift)
    ADD R6, R6, -1
    LD R0, SHIFT
    STR R0, R6, 0
    
    ADD R6, R6, -1
    LD R0, STRING
    STR R0, R6, 0
    
    JSR ENCRYPT
    
    LDR R0, R6, 0
    ADD R6, R6, 3

    HALT
    
    ;; Do not rename or remove any existing labels
    ;; You may change the value of STRING, LENGTH, SHIFT for debugging
    STACK_PTR .fill xF000
    STRING .fill x4000
    SHIFT .fill 7
    ASCIIUPPERA .fill 65
    ASCIILOWERA .fill 97
    ALPHABETLEN .fill 26



MOD ;; Do not change this label! Treat this as like the name of the function in a function header
    ;; Code your implementation for the MOD subroutine here!

    ;;  MOD Pseudocode (see PDF for explanation and examples)   
;;  
;;  MOD(int a, int b) {
;;      while (a >= b) {
;;          a -= b;
;;      }
;;      return a;
;;  }
    
    ; Start of Callee buildup
    ; Return value
    ADD R6, R6, -1
    ; Return address
    ADD R6, R6, -1
    STR R7, R6, 0
    ; Old frame pointer
    ADD R6, R6, -1
    STR R5, R6, 0
    ; Space for local variables
    ADD R6, R6, -1
    ; Set frame pointer to stack pointer
    AND R5, R5, 0
    ADD R5, R6, 0
    ; Save general purpose registers
    ADD R6, R6, -1
    STR R0, R6, 0
    ADD R6, R6, -1
    STR R1, R6, 0
    ADD R6, R6, -1
    STR R2, R6, 0
    ADD R6, R6, -1
    STR R3, R6, 0
    ADD R6, R6, -1
    STR R4, R6, 0
    ; End of Callee buildup
    
    ; Actual GCD code
    LDR R0, R5, 4   ; R0 = a
    LDR R1, R5, 5   ; R1 = b
    WHILEBEGIN
    NOT R2, R1
    ADD R2, R2, 1   ; R2 = -b
    ADD R3, R0, R2
    BRN WHILEEND
    ADD R0, R0, R2
    BR WHILEBEGIN
    WHILEEND
    
    ; Start of Callee Teardown
    ; Save return value
    STR R0, R5, 3
    ; Restore general purpose registers
    LDR R4, R6, 0
    ADD R6, R6, 1
    LDR R3, R6, 0
    ADD R6, R6, 1
    LDR R2, R6, 0
    ADD R6, R6, 1
    LDR R1, R6, 0
    ADD R6, R6, 1
    LDR R0, R6, 0
    ADD R6, R6, 1
    ; Pop local variables
    ADD R6, R6, 1
    ; Pop and restore frame pointer
    LDR R5, R6, 0
    ADD R6, R6, 1
    ; Pop and restore return address
    LDR R7, R6, 0
    ADD R6, R6, 1
    ; End of Callee Teardown
    
    RET


ENCRYPT ;; Do not change this label! Treat this as like the name of the function in a function header
        ;; Code your implementation for the ENCRYPT subroutine here!

    ;;  ENCRYPT Pseudocode (see PDF for explanation and examples)
;;
;;  ENCRYPT(String str, int k) {
;;      int length = 0;
;;      while (str[length] != 0) {
;;          length++;
;;      }
;;      for (int i = 0; i < length; i++) {
;;          char = str[i];
;;          if (char >= 'a' && char <= 'z') {
;;              char = char - 'a';
;;              char = MOD(char + k, 26);
;;              char = char + 'a';
;;          } else if (char >= 'A' && char <= 'Z') {
;;              char = char - 'A';
;;              char = MOD(char + k, 26);
;;              char = char + 'A';
;;          }
;;
;;          str[i] = char;
;;      }
;;  }
    
    ; Start of Callee buildup
    ; Return value
    ADD R6, R6, -1
    ; Return address
    ADD R6, R6, -1
    STR R7, R6, 0
    ; Old frame pointer
    ADD R6, R6, -1
    STR R5, R6, 0
    ; Space for local variables
    ADD R6, R6, -1
    ; Set frame pointer to stack pointer
    AND R5, R5, 0
    ADD R5, R6, 0
    ; Save general purpose registers
    ADD R6, R6, -1
    STR R0, R6, 0
    ADD R6, R6, -1
    STR R1, R6, 0
    ADD R6, R6, -1
    STR R2, R6, 0
    ADD R6, R6, -1
    STR R3, R6, 0
    ADD R6, R6, -1
    STR R4, R6, 0
    ; End of Callee buildup
    
    ; Actual ENCRYPT code
    AND R0, R0, 0   ; length = 0
    WHILE2BEGIN
    LDR R1, R5, 4   ; R1 = addr of str
    ADD R1, R1, R0  ; R1 = addr of str+length
    LDR R1, R1, 0   ; R1 = str[length]
    BRZ WHILE2END
    ADD R0, R0, 1   ; length++
    BR WHILE2BEGIN
    WHILE2END
    
    AND R1, R1, 0   ; i = 0
    FORBEGIN
    NOT R2, R0
    ADD R2, R2, 1   ; R2 = -length
    ADD R2, R1, R2  ; R2 = i-length
    BRZP FOREND
    
    LDR R3, R5, 4   ; R3 = addr of str
    ADD R3, R3, R1  ; R3 = addr of str+i
    LDR R3, R3, 0   ; R3 = char = str[i]
    
    ; char >= 'a'
    LD R4, ASCIILOWERA
    NOT R4, R4
    ADD R4, R4, 1   ; R4 = -'a'
    ADD R4, R3, R4  ; R4 = char - 'a'
    BRN IFEND
    
    ; char <= 'z'
    LD R4, ASCIILOWERA
    LD R2, ALPHABETLEN
    ADD R4, R4, R2
    NOT R4, R4
    ADD R4, R4, 1   ; R4 = -'z'
    ADD R4, R3, R4  ; R4 = char - 'z'
    BRP IFEND
    
    ; inside if
    LD R4, ASCIILOWERA
    NOT R4, R4
    ADD R4, R4, 1   ; R4 = -'a'
    ADD R3, R3, R4  ; R3 = char - 'a'
    
    ADD R6, R6, -1
    LD R4, ALPHABETLEN
    STR R4, R6, 0   ; put 26 as parameter
    
    ADD R6, R6, -1
    LDR R4, R5, 5   ; R4 = k
    ADD R4, R3, R4  ; R4 = char + K
    STR R4, R6, 0   ; put char+k as parameter
    
    JSR MOD
    
    LDR R3, R6, 0   ; R3 = return value of MOD
    ADD R6, R6, 3   ; fix stack after call
    
    LD R4, ASCIILOWERA
    ADD R3, R3, R4  ; char = char + 'a'
    
    BR ELIFEND      ; dont enter the else if block

    IFEND
    
    ; char >= 'A'
    LD R4, ASCIIUPPERA
    NOT R4, R4
    ADD R4, R4, 1   ; R4 = -'A'
    ADD R4, R3, R4  ; R4 = char - 'A'
    BRN ELIFEND
    
    ; char <= 'Z'
    LD R4, ASCIIUPPERA
    LD R2, ALPHABETLEN
    ADD R4, R4, R2  ; R4 = 'Z'
    NOT R4, R4
    ADD R4, R4, 1   ; R4 = -'Z'
    ADD R4, R3, R4  ; R4 = char - 'Z'
    BRP ELIFEND
    
    ; inside elif
    LD R4, ASCIIUPPERA
    NOT R4, R4
    ADD R4, R4, 1   ; R4 = -'A'
    ADD R3, R3, R4  ; R3 = char - 'A'
    
    ADD R6, R6, -1
    LD R4, ALPHABETLEN
    STR R4, R6, 0   ; put 26 as parameter
    
    ADD R6, R6, -1
    LDR R4, R5, 5   ; R4 = k
    ADD R4, R3, R4  ; R4 = char + k
    STR R4, R6, 0   ; put char+k as parameter
    
    JSR MOD
    
    LDR R3, R6, 0   ; char = return value of MOD
    ADD R6, R6, 3   ; fix stack after call
    LD R4, ASCIIUPPERA
    ADD R3, R3, R4  ; char = char + 'A'
    
    ELIFEND
    
    LDR R2, R5, 4   ; R2 = addr of string
    ADD R2, R2, R1  ; R2 = addr of string+i
    STR R3, R2, 0   ; string[i] = char
    
    ADD R1, R1, 1   ; i++
    BR FORBEGIN
    
    FOREND
    
    ; Start of Callee Teardown
    ; Save return value
    STR R0, R5, 3
    ; Restore general purpose registers
    LDR R4, R6, 0
    ADD R6, R6, 1
    LDR R3, R6, 0
    ADD R6, R6, 1
    LDR R2, R6, 0
    ADD R6, R6, 1
    LDR R1, R6, 0
    ADD R6, R6, 1
    LDR R0, R6, 0
    ADD R6, R6, 1
    ; Pop local variables
    ADD R6, R6, 1
    ; Pop and restore frame pointer
    LDR R5, R6, 0
    ADD R6, R6, 1
    ; Pop and restore return address
    LDR R7, R6, 0
    ADD R6, R6, 1
    ; End of Callee Teardown
    
    RET

.end

;; You may change the value of the string for debugging
.orig x4000
    .stringz "hello"
.end
