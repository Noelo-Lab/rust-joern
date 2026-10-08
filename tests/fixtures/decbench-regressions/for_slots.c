int for_init_update(int x) { int i; for (i=0;;++i) { if (i>3) break; x++; } return x; }
int for_expression_body(int x) { int i; for (i=0;;++i) x++; return x; }
int for_no_update(int x) { int i; for (i=0;i<x;) x++; return x; }
int for_no_slots(int x) { for (;;) x++; return x; }
int for_init_only(int x) { int i; for (i=0;;) x++; return x; }
void target();
void cb();
void method_ref(int x) { if (x) cb(target); }
