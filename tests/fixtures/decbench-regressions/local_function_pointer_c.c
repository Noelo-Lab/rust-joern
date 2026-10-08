int target(int x);
int second(int x);
int g(int x);
void fp_only_direct(int x) { int (*p)(int) = target; }
void fp_only_conditional(int x) { int (*p)(int) = x ? target : second; }
void fp_only_call(int x) { int (*p)(int) = g(x); }
void fp_only_mixed_first(int x) { int (*p)(int) = x ? target : second, b=g(x); }
void fp_only_mixed_second(int x) { int b=g(x), (*p)(int) = x ? target : second; }
