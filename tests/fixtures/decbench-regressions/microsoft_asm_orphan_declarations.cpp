int asm_tail_simple(int x) {
 x++;
 __asm { mov eax,ebx }
 undeclared();
 other(x);
 if(x) conditional();
 return result();
}
int after_simple(void) { return 1; }
int asm_tail_arguments(int x) {
 __asm { op }
 tail_zero();
 tail_variable(x);
 tail_number(3);
 return tail_return();
 if(x) tail_conditional();
 while(x) { tail_nested(); }
}
int after_arguments(void) { return 2; }
int asm_tail_nested(int x) {
 if(x) {
  __asm { op }
  nested_zero();
  nested_variable(x);
 }
 nested_outer();
 return nested_return();
}
int after_nested(void) { return 3; }
int asm_tail_do(int x) {
 if(x) {
  do {
   __asm { op }
   do_first(x);
   do_second(x);
   __asm { op }
   do_third(x);
   do_fourth(x);
  } while(x);
 }
 return do_return();
}
int after_do(void) { return 4; }
