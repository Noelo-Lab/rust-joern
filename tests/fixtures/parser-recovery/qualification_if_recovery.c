int foo(void);
int bar(int x);
int problem_both(int x) { if (x || foo()) bar(::x); else bar(::x); return x; }
int problem_no_else(int x) { if (x || foo()) bar(::x); return x; }
int problem_valid_else(int x) { if (x || foo()) bar(::x); else x++; return x; }
int problem_braced_both(int x) { if (x || foo()) { bar(::x); } else { bar(::x); } return x; }
int problem_braced_then(int x) { if (x || foo()) { bar(::x); } else bar(::x); return x; }
int problem_braced_else(int x) { if (x || foo()) bar(::x); else { bar(::x); } return x; }
