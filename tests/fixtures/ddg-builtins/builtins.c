typedef unsigned long size_t;
struct Node { int member[4]; };
int foo(int x);
int types_value(int x, int y) { return __builtin_types_compatible_p(x, y); }
int types_branches(int x) { return __builtin_types_compatible_p(x && foo(x), x || foo(x)); }
int offset_known_size(void) { return __builtin_offsetof(struct Node, member); }
int offset_known_branch(int x) { return __builtin_offsetof(struct Node, member[x ? 1 : 2]); }
int alignof_type(int x) { return __alignof__(int); }
int alignof_value(int x) { return __alignof__(foo(x)); }
int alignof_branch(int x) { return __alignof__(x && foo(x)); }
int alignof_unbracketed(int x) { return __alignof__ x; }
int sizeof_value(int x) { return sizeof(foo(x)); }
