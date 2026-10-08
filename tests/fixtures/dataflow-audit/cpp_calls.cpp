extern "C" int abs(int);
extern "C" void free(void*);
int c_semantics(int x) { free((void*)x); return abs(x); }
namespace N { int abs(int x) { return x; } int n(int x) { return abs(x); } }
struct S { int abs(int x) { return x; } int use(int x) { return abs(x); } };
int method(S *s, int x) { return s->abs(x); }
int q(int x) { return N::abs(x); }
int overload(int x) { return x; }
int overload(double x) { return x; }
int choose(int x, double d) { return overload(x) + overload(d); }
int unresolved(int x) { return missing(x); }
int ambiguous(int x) { return overload(missing(x)); }
int plain(int x) { return abs(x); }
struct B { int a; int b; };
int structs(B b, int x) { b.a=x; return b.a; }
int distinct(B b, int x) { b.a=x; return b.b; }
int refs(int &x) { int &y=x; y++; return x; }
int member_arg0(S *s, int x) { return s->use(x); }
