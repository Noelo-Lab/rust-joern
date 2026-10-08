int foo(int);
int bar(int);
int call_and(int n) { return foo(n) && bar(n); }
int call_or(int n) { return foo(n) || bar(n); }
int call_if_and(int n) { if (foo(n) && bar(n)) return 1; return 0; }
int call_ternary(int n) { return n ? foo(n) : bar(n); }
int expr_ternary(int n) { return n ? n + 1 : n - 1; }
int mixed_and(int n) { return n && foo(n); }
int sizeof_call_and(int n) { return sizeof(foo(n) && bar(n)); }
int while_true(int n) { while (1) { if (n--) break; } return n; }
int empty_do(int n) { do {} while(n); return n; }
int empty_for(int n) { for (;n;) {} return n; }
int while_empty_decl(int n) { while(n) { int unused; } return n; }
int nested_empty(int n) { { {} } return n; }
int switch_nested(int n) { switch(n) { case 1: switch(n+1) { case 2: return 5; default: break; } break; default: break; } return 0; }
int nested_continue(int n) { for (int i=0; i<n; ++i) { switch(i) { case 1: continue; default: break; } n--; } return n; }
