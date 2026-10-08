int straight(int x) { x += 2; return x; }
int branch(int x) { if (x) return 1; return 0; }
int join(int x) { int y; if (x) y = 1; else y = 2; return y; }
int logical_and(int a, int b) { if (a && b) return 1; return 0; }
int logical_or(int a, int b) { return a || b; }
int nested_logical(int a, int b, int c) { return (a && b) || c; }
int ternary(int a, int b, int c) { return a ? b : c; }
int while_loop(int n) { int x = 0; while (x < n) { ++x; } return x; }
int empty_while(int n) { while (n) {} return n; }
int for_loop(int n) { int x = 0; for (int i = 0; i < n; ++i) x += i; return x; }
int infinite_for(int n) { for (;;) { if (n) break; n++; } return n; }
int do_loop(int n) { do { n--; } while (n > 0); return n; }
int loop_break_continue(int n) { while (n > 0) { n--; if (n == 2) continue; if (n == 1) break; n += 2; } return n; }
int switch_default(int n) { switch (n) { case 0: return 1; case 1: n++; break; default: n--; } return n; }
int switch_no_default(int n) { switch (n) { case 0: n++; break; case 1: n--; } return n; }
int switch_fallthrough(int n) { switch (n) { case 0: case 1: n++; case 2: n--; break; default: break; } return n; }
int goto_forward(int n) { if (n) goto target; n++; target: return n; }
int goto_backward(int n) { target: if (n--) goto target; return n; }
int unreachable(int n) { return n; n++; }
int statement_expression(int n) { return ({ if (n) n++; n; }); }
int sizeof_logical(int a, int b) { return sizeof(a && b); }
int declaration_initializer(int a, int b) { int x = a && b; return x; }
int empty_function(void) {}
int empty_if(int x) { if (x) {} return x; }
