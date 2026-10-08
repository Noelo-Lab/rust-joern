void take(int (*p)(int));
int foo(int x){return x;}
int self_callback(int x){take(self_callback);return x;}
int outside_callback(int x){take(foo);return x;}
int multiline_callback(int x){take(
    foo);return x;}
long foo(long x){return x;}
int overload_callback(int x){take(foo);return x;}
int ambiguous_callback(int x){unknown_sink(foo);return x;}
namespace N {
int namespaced(int x){return x;}
int inside(int x){take(namespaced);return x;}
}
int qualified_callback(int x){take(N::namespaced);return x;}
