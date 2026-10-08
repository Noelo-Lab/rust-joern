int g(int x);
int extension_sub(int x) { return (__extension__ ({ int r; if (x && g(x)) r = 1; else r = 2; r; })) - 'a'; }
int plain_sub(int x) { return ({ int r; if (x && g(x)) r = 1; else r = 2; r; }) - 'a'; }
int extension_mul(int x) { return (__extension__ ({ int r; if (x && g(x)) r = 1; else r = 2; r; })) * 2; }
int extension_and(int x) { return (__extension__ ({ int r; if (x && g(x)) r = 1; else r = 2; r; })) && g(x); }
int simple_extension_sub(int x) { return (__extension__ (x && g(x))) - 'a'; }
int actual_typeof_cast(int x) { return (__typeof__(x)) -x; }
int actual_builtin_cast(int x) { return (int) -x; }
