;;=============================================================
;; CS 2110 - Spring 2025
;; Homework 4 - Merge Sort
;;=============================================================
;; Name: Philip Dobranowski
;;=============================================================

;;  In this file, you must implement the 'MERGESORT', 'MERGE', and 'DIVIDE' subroutines.
    
.orig x3000
    ;; You do not need to write anything here
    LD R6, STACK_PTR

    ;; Pushes arguments (starting address of array to sort,
    ;;                   starting address of buffer,
    ;;                   lower index to sort,
    ;;                   upper index to sort)
    ;;=============================================================

    LD R6, STACK_PTR
    LD R0, ARRAY
    LD R1, BUF
    AND R2, R2, 0   ; start = 0
    LD R3, LENGTH   ; end = length
    
    ADD R6, R6, -1  ; push args
    STR R3, R6, 0
    ADD R6, R6, -1
    STR R2, R6, 0
    ADD R6, R6, -1
    STR R1, R6, 0
    ADD R6, R6, -1
    STR R0, R6, 0
    
    JSR MERGESORT
    
    HALT
    
    ARRAY   .fill x4000
    BUF     .fill x5000
    LENGTH  .fill 4
    STACK_PTR .fill xF000


MERGESORT   ;; Do not change this label! Treat this as like the name of the function in a function header
            ;; Code your implementation for the MERGESORT subroutine here!

;; MERGESORT PSEUDOCODE: Pseudocode (see PDF for explanation and examples)   
;; MERGESORT (int[] arr, int[] buf, int start, int end) {
;;      if (start >= end - 1) {
;;        return;
;;      }
;;      mid = DIVIDE(start + end, 2)
;;      MERGESORT(arr, buf, start, mid)
;;      MERGESORT(arr, buf, mid, end)
;;      MERGE(arr, buf, start, mid, end)
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
    
    ; Actual MERGESORT code
    
    ; if check
    LDR R0, R5, 6   ; R0 = start
    LDR R1, R5, 7   ; R1 = end
    NOT R1, R1
    ADD R1, R1, 1   ; R1 = -end
    ADD R0, R0, R1  ; R0 = start-end
    ADD R0, R0, 1   ; R0 = start-end+1
    BRZP EARLYRET
    
    ; put 2 as param
    ADD R6, R6, -1
    AND R0, R0, 0
    ADD R0, R0, 2
    STR R0, R6, 0
    ; put start+end as param
    ADD R6, R6, -1
    LDR R0, R5, 6    ; R0 = start
    LDR R1, R5, 7    ; R1 = end
    ADD R0, R0, R1  ; R0 = start+end
    STR R0, R6, 0
    ; call divide and fetch ret value
    JSR DIVIDE
    LDR R0, R6, 0   ; R0 = mid = return value of DIVIDE
    ADD R6, R6, 3   ; fix stack after call
    
    ; put mid as param
    ADD R6, R6, -1
    STR R0, R6, 0
    ; put start as param
    ADD R6, R6, -1
    LDR R1, R5, 6   ; R1 = start
    STR R1, R6, 0
    ; put buf as param
    ADD R6, R6, -1
    LDR R1, R5, 5   ; R1 = buf
    STR R1, R6, 0
    ; put arr as param
    ADD R6, R6, -1
    LDR R1, R5, 4   ; R1 = arr
    STR R1, R6, 0
    ; call mergesort
    JSR MERGESORT
    ADD R6, R6, 5   ; fix stack after call
    
    ; put end as param
    ADD R6, R6, -1
    LDR R1, R5, 7   ; R1 = end
    STR R1, R6, 0
    ; put mid as param
    ADD R6, R6, -1
    STR R0, R6, 0
    ; put buf as param
    ADD R6, R6, -1
    LDR R1, R5, 5   ; R1 = buf
    STR R1, R6, 0
    ; put arr as param
    ADD R6, R6, -1
    LDR R1, R5, 4   ; R1 = arr
    STR R1, R6, 0
    ; call mergesort
    JSR MERGESORT
    ADD R6, R6, 5   ; fix stack after call
    
    ; put end as param
    ADD R6, R6, -1
    LDR R1, R5, 7   ; R1 = end
    STR R1, R6, 0
    ; put mid as param
    ADD R6, R6, -1
    STR R0, R6, 0
    ; put start as param
    ADD R6, R6, -1
    LDR R1, R5, 6   ; R1 = start
    STR R1, R6, 0
    ; put buf as param
    ADD R6, R6, -1
    LDR R1, R5, 5   ; R1 = buf
    STR R1, R6, 0
    ; put arr as param
    ADD R6, R6, -1
    LDR R1, R5, 4   ; R1 = arr
    STR R1, R6, 0
    ; call merge
    JSR MERGE
    ADD R6, R6, 6   ; fix stack after call
    
    EARLYRET
    
    ; Start of Callee Teardown
    ; No return value
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
    
;;  DIVIDE Pseudocode (see PDF for explanation and examples)   
;;
;;  DIVIDE(int a, int b)
;;  {
;;      if (b == 0)
;;      {
;;          return 0;
;;      }
;;    
;;      int quotient = 0;
;;      while (a >= b)
;;      {
;;          a -= b;
;;          quotient++;
;;      }
;;    
;;      return quotient;
;;  }

DIVIDE  ;; Do not change this label! Treat this this like the name of the function in a function header
        ;; Code your implementation for the DIVIDE subroutine here!
    
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
    
    ; Actual DIVIDE code
    LDR R0, R5, 5  ; R0 = b
    BRZ RETIF
    
    AND R0, R0, 0  ; R0 = quotient = 0
    ; while check
    LDR R1, R5, 4  ; R1 = a
    WHILEBEGIN
    LDR R2, R5, 5  ; R2 = b
    NOT R2, R2
    ADD R2, R2, 1  ; R2 = -b
    ADD R2, R1, R2 ; R2 = a-b
    BRN WHILEEND
    
    ; while body
    AND R1, R1, 0
    ADD R1, R1, R2 ; R1 = a = a-b
    ADD R0, R0, 1  ; quotient++
    BR WHILEBEGIN
    
    WHILEEND
    
    RETIF
    
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


;;  MERGE Pseudocode (see PDF for explanation and examples)
;;  MERGE(int arr[], int buf[], int start, int mid, int end) {
;;      i = start;
;;      j = mid;
;;      k = start;
;;      while (i < mid && j < end) {
;;          if arr[i] <= arr[j] {
;;              buf[k] = arr[i];
;;              k++;
;;              i++;
;;          } else {
;;              buf[k] = arr[j];
;;              k++;
;;              j++;
;;          }
;;      }
;;      while (i < mid) {
;;          buf[k] = arr[i];
;;          k++;
;;          i++;
;;      }
;;      while (j < end) {
;;          buf[k] = arr[j];
;;          k++;
;;          j++;
;;      }
;;      for (i = start; i < end; i++) {
;;          arr[i] = buf[i];
;;      }
;;  }

MERGE   ;; Do not change this label! Treat this as like the name of the function in a function header
        ;; Code your implementation for the MERGE subroutine here!
    
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
    
    ; Actual MERGE code
    LDR R0, R5, 6   ; R0 = i = start
    LDR R1, R5, 7   ; R1 = j = mid
    LDR R2, R5, 6   ; R2 = k = start
    
    WHILE1BEGIN
    
    ; i < mid
    LDR R3, R5, 7   ; R3 = mid
    NOT R3, R3
    ADD R3, R3, 1   ; R3 = -mid
    ADD R3, R0, R3  ; R3 = i-mid
    BRZP WHILE2BEGIN
    ; j < end
    LDR R3, R5, 8   ; R3 = end
    NOT R3, R3
    ADD R3, R3, 1   ; R3 = -end
    ADD R3, R1, R3  ; R3 = j-end
    BRZP WHILE2BEGIN
    
    ; if arr[i] <= arr[j]
    LDR R3, R5, 4   ; R3 = arr
    ADD R3, R3, R0  ; R3 = arr+i
    LDR R3, R3, 0   ; R3 = arr[i]
    LDR R4, R5, 4   ; R4 = arr
    ADD R4, R4, R1  ; R4 = arr+j
    LDR R4, R4, 0   ; R4 = arr[j]
    NOT R4, R4
    ADD R4, R4, 1   ; R4 = -arr[j]
    ADD R3, R3, R4  ; R3 = arr[i] - arr[j]
    BRP ELSE
    
    ; inside if block
    LDR R3, R5, 5   ; R3 = buf
    ADD R3, R3, R2  ; R3 = buf+k
    LDR R4, R5, 4   ; R4 = arr
    ADD R4, R4, R0  ; R4 = arr+i
    LDR R4, R4, 0   ; R4 = arr[i]
    STR R4, R3, 0   ; buf[k] = arr[i]
    ADD R2, R2, 1   ; k++
    ADD R0, R0, 1   ; i++
    BR WHILE1BEGIN      ; dont execute else after if
    
    ; inside else block
    ELSE
    LDR R3, R5, 5   ; R3 = buf
    ADD R3, R3, R2  ; R3 = buf+k
    LDR R4, R5, 4   ; R4 = arr
    ADD R4, R4, R1  ; R4 = arr+j
    LDR R4, R4, 0   ; R4 = arr[j]
    STR R4, R3, 0   ; buf[k] = arr[j]
    ADD R2, R2, 1   ; k++
    ADD R1, R1, 1   ; j++
    
    BR WHILE1BEGIN
    
    WHILE2BEGIN
    
    ; i < mid
    LDR R3, R5, 7   ; R3 = mid
    NOT R3, R3
    ADD R3, R3, 1   ; R3 = -mid
    ADD R3, R0, R3  ; R3 = i-mid
    BRZP WHILE3BEGIN
    
    ; inside while2 block
    LDR R3, R5, 5   ; R3 = buf
    ADD R3, R3, R2  ; R3 = buf+k
    LDR R4, R5, 4   ; R4 = arr
    ADD R4, R4, R0  ; R4 = arr+i
    LDR R4, R4, 0   ; R4 = arr[i]
    STR R4, R3, 0   ; buf[k] = arr[i]
    ADD R2, R2, 1   ; k++
    ADD R0, R0, 1   ; i++
    BR WHILE2BEGIN
    
    WHILE3BEGIN
    
    ; j < end
    LDR R3, R5, 8   ; R3 = end
    NOT R3, R3
    ADD R3, R3, 1   ; R3 = -end
    ADD R3, R1, R3  ; R3 = j-end
    BRZP WHILE3END
    
    ; inside while3 block
    LDR R3, R5, 5   ; R3 = buf
    ADD R3, R3, R2  ; R3 = buf+k
    LDR R4, R5, 4   ; R4 = arr
    ADD R4, R4, R1  ; R4 = arr+j
    LDR R4, R4, 0   ; R4 = arr[j]
    STR R4, R3, 0   ; buf[k] = arr[j]
    ADD R2, R2, 1   ; k++
    ADD R1, R1, 1   ; j++
    BR WHILE3BEGIN
    
    WHILE3END
    
    ; can clear registers they dont matter anymore
    LDR R0, R5, 6   ; R0 = i = start
    
    ; for loop
    FORBEGIN
    LDR R1, R5, 8   ; R1 = end
    NOT R1, R1
    ADD R1, R1, 1   ; R1 = -end
    ADD R1, R0, R1  ; R1 = i-end
    BRZP FOREND
    
    ; inside for block
    LDR R2, R5, 5   ; R2 = buf
    ADD R2, R2, R0  ; R2 = buf+i
    LDR R2, R2, 0   ; R2 = buf[i]
    LDR R1, R5, 4   ; R1 = arr
    ADD R1, R1, R0  ; R1 = arr+i
    STR R2, R1, 0   ; arr[i] = buf[i]
    ADD R0, R0, 1   ; i++
    BR FORBEGIN
    
    FOREND
    
    ; Start of Callee Teardown
    ; No return value
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
    
;; You may change the values in these arrays for debugging. Remember to change the value of LENGTH as well. 
.orig x4000
    .fill 5 
    .fill 2 
    .fill 3 
    .fill 1
.end

.orig x5000
    .fill 5 
    .fill 2 
    .fill 3 
    .fill 1
.end
