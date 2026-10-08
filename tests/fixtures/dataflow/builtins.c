struct Pair { int x; int y; };
int offset(void) { return __builtin_offsetof(struct Pair, y); }
int vararg(void *p) { return __builtin_va_arg(p, int); }
int compatible(int x) { return x + __builtin_types_compatible_p(int, int); }
int transparent(int x) { return __offsetof__(x); }
