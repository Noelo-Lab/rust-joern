int knr_shared(a, b) int a, b; { return a + b; }
int knr_unordered(a, b, c) char *c; int a; { return a + b + *c; }
int knr_reversed(a, b) int b; int a; { return a - b; }
int knr_implicit(a, b) int a; { return a + b; }
static int knr_array(s, n) char s[]; unsigned n; { return s[n]; }
int variadic_named(const char *fmt, ...) { return fmt[0]; }
int variadic_unnamed(int n, char *, ...);
int variadic_only(...);
int attribute_after(int x __attribute__((unused))) { return 0; }
int attribute_before(__attribute__((unused)) int x) { return x; }
int attribute_between(int __attribute__((unused)) x) { return x; }
int attribute_abstract(void *__attribute__((unused)));
int attribute_first(struct stat const *st __attribute__ ((__unused__)), int y) { return y; }
