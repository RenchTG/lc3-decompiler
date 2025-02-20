; This code is written by me
.orig x3000
ld r0, ADD_FUNC_ADDR

; How do I pass arguments???
jsrr r0 ; Call ADD_FUNC()
; How do I get return value???

halt
ADD_FUNC_ADDR .fill ADD_FUNC
.end


; Written by my cousin Bubba from North GA.
; The API is: int add(int a, int b);
.orig x4000
ADD_FUNC
    ; How does Bubba get arguments???
    ; How does Bubba send back the returned value???
    ret
.end

; ...stay tuned for the answers to these questions!
