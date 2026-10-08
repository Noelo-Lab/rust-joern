__attribute__((format(printf,1,2)))
static _Noreturn void attr_noreturn(const char *format,...) { return; }
int after_attribute(int x) { return x; }
void unknown<R1,R2> decorated(int x) { return; }
int after_decorated(int x) { return x; }
unknown<R1,R2> template_return(int x) { return x; }
int after_template(int x) { return x; }
int target(int x) { return x; }
int pointer_tail(int x) {
 int (*ptr)(int)=target;
 __asm { mov eax,ebx }
 ptr(x);
 plain(x);
}
int after_pointer(int x) { return x; }
int variable_tail(int x) {
 int value=3;
 __asm { mov eax,ebx }
 value(x);
}
int after_variable(int x) { return x; }
