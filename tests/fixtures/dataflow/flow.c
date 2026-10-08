extern int sink(int);
extern void side(int *);
int copy(int x) { int y = x; return y; }
int branch(int x) { int y; if (x > 0) y = 1; else y = 2; return y; }
int loop(int x) { int y = 0; while (x > 0) { y += x; x--; } return y; }
int firstwhile(int x) { while (x > 0) x--; return x; }
int compound(int x, int y) { x += y; return x; }
int nested(int x) { return sink(sink(x)); }
int sized(int x) { int y = sizeof(x); return y; }
struct Box { int value; int *data; };
int fields(struct Box *b, int x) { b->value = x; return b->value; }
int arrays(int *p, int i, int x) { p[i] = x; return p[i]; }
int address(int x) { int *p = &x; return *p; }
int shifted(int *p) { int *q = p + 1; return *q; }
int casted(long x) { int *p = (int *)x; return *p; }
int indirect(int (*f)(int), int x) { return f(x); }
int global;
int globals(int x) { global = x; return global; }
int conditional(int x, int y, int z) { int a = x ? y : z; return a; }
int dead(int x) { return x; x = 5; }
int statement(int x) { int y = ({ int t = x + 1; t; }); return y; }
int shortcircuit(int x, int y) { int z = x && y; return z; }
int multi(int x) { if (x > 0) return x; return 7; }
void noret(int x, int y) { sink(x + y); }
int shadows(int x) { int y = x; { int x = 2; y = x; } return y; }
int uncertain(int x) { int y; return y + x; }
