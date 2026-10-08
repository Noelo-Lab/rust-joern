typedef unsigned long size_t;
typedef char *va_list;
struct Node { int member[4]; };
int foo(int x);
int types_builtin(void) { return __builtin_types_compatible_p(int, int); }
int types_pointer(void) { return __builtin_types_compatible_p(char *, int *); }
int types_value(int x, int y) { return __builtin_types_compatible_p(x, y); }
int types_branches(int x) { return __builtin_types_compatible_p(x && foo(x), x || foo(x)); }
int offset_known_size(void) { return __builtin_offsetof(struct Node, member); }
int offset_known_branch(int x) { return __builtin_offsetof(struct Node, member[x ? 1 : 2]); }
int wrap_offset(int x) { return __offsetof__(x && foo(x)); }
int imag_value(_Complex float value) { return __imag__ value; }
int real_value(_Complex float value) { return __real__ value; }
void *null_value(void) { return __null; }
int extension_value(int x) { return __extension__ (x && foo(x)); }
int va_address(va_list ap) { return __builtin_va_arg((ap), int); }
int va_binary(va_list ap, int x) { return __builtin_va_arg(ap + x, int); }
