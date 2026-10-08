extern int abs(int);
extern void free(void *);
int c_semantics(int x) { free((void*)x); return abs(x); }
namespace N { int abs(int x) { return x; } int n(int x) { return abs(x); } }
struct S { int abs(int x) { return x; } int use(int x) { return abs(x); } };
int method(S *s, int x) { return s->abs(x); }
int q(int x) { return N::abs(x); }
int lambda(int x) { auto f = [x](int y) { return x + y; }; return f(x); }
int aliases(int *p, int x) { *p = x; return p[0]; }
int offsets(int *p, int x) { p[1] = x; return *(p + 1); }
int addr(int x) { return *&x; }
int ref(int &x) { int &y = x; y++; return x; }
struct B { int a; int b; };
int distinct(B b, int x) { b.a = x; return b.b; }
int structs(B b, int x) { b.a = x; return b.a; }
int unknown_index(int *p, int i, int j, int x) { p[i] = x; return p[j]; }
int constant_index(int *p, int x) { p[0] = x; return p[1]; }
