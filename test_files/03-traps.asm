; Read characters from the keyboard and display them on the screen
; (demonstration of TRAPs)

.orig x3000
LOOP
getc ; r0 <- ascii code of keypress
putc ; print ascii code in r0 to screen
br LOOP
halt
.end
