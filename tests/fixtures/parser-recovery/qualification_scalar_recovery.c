int foo(void);
int bar(int x);
int problem_while(int x) { while (x || foo()) bar(::x); return x; }
int problem_do(int x) { do bar(::x); while (x || foo()); return x; }
int problem_label_if(int x) { LABEL: if (x || foo()) bar(::x); return x; }
int problem_label_while(int x) { LABEL: while (x || foo()) bar(::x); return x; }
int problem_braced_while(int x) { while (x || foo()) { bar(::x); } return x; }
int problem_braced_do(int x) { do { bar(::x); } while (x || foo()); return x; }
int asm_scalar_if(int x) { if (x) __asm { mov eax, ebx } x++; return x; }
int after_asm(int x) { return x; }
