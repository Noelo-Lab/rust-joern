typedef int T;
int marked_builtin_cast(int x) { return (__extension__ int) -x; }
int marked_typeof_cast(int x) { return (__extension__ __typeof__(x)) -x; }
int marked_alias_cast(int x) { return (__extension__ T) -x; }
int marked_pointer_cast(int *p) { return *((__extension__ int *)p); }
int marked_ordinary_group(int x) { return (__extension__ (x)) - x; }
