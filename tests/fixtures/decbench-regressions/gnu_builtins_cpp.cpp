typedef char *va_list;
struct Inner { int member[4]; };
struct Node { int first; struct Inner inner; };
int foo(int x);
int va_integer(va_list ap) { return __builtin_va_arg(ap, int); }
char *va_pointer(va_list ap) { return __builtin_va_arg(ap, char *); }
int va_branch(int x, va_list ap, va_list other) { return __builtin_va_arg(x ? ap : other, int); }
int va_boolean(int x, va_list ap) { if (__builtin_va_arg(ap, int) && foo(x)) return 1; return 0; }
int offset_direct(void) { return __builtin_offsetof(struct Node, first); }
int offset_path(void) { return __builtin_offsetof(struct Node, inner.member[2]); }
int offset_branch(int x) { return __builtin_offsetof(struct Node, inner.member[x ? 1 : 2]); }
int sizeof_type(int x) { return sizeof(int) + __alignof__(int); }
int sizeof_value(int x) { return sizeof(foo(x)) + __alignof__(foo(x)); }
int sizeof_branch(int x) { return sizeof(x && foo(x)); }
int alignof_branch(int x) { return __alignof__(x && foo(x)); }
