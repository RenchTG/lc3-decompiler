; Hello world program in LC-3

.orig x3000
lea r0, MESSAGE
puts
halt
MESSAGE .stringz "Hello, world!"
.end