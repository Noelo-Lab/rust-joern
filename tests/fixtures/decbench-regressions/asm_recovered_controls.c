void assignment_tail(int x) {
  __asm { mov eax,ebx }
  value=x;
  value(x);
  pointer=(void(*)(int))0;
  pointer(x);
  undeclared(x);
}
int after_assignment(void) { return 1; }
void second_asm_tail(int x) {
  __asm { mov eax,ebx }
  if(x) {
    before_if();
    __asm { mov eax,ecx }
    after_if(x);
    assign_if(x)=1;
  }
  while(x) {
    before_loop();
    __asm { mov eax,edx }
    after_loop(x);
    assign_loop(x)=1;
  }
}
int after_second(void) { return 2; }
void nested_second_asm_tail(int x) {
  __asm { mov eax,ebx }
  if(x) {
    while(x) {
      before_nested();
      __asm { mov eax,ecx }
      after_nested(x);
    }
    following_nested(x);
  }
}
int after_nested_body(void) { return 3; }
