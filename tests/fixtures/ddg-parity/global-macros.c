struct S { int x; };
unsigned offset = __builtin_offsetof(struct S, x);
char bounds[1 + __builtin_types_compatible_p(int, int)];
int ordinary(int x) { return x; }
