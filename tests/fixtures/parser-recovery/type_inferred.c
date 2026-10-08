void decl(T x);
int inferred_address(int x) { return (T)&x; }
int inferred_plus(int x) { return (T)+x; }
int inferred_minus(int x) { return (T)-x; }
int inferred_multiply(int x) { return (T)*x; }
int inferred_local(int x) { T y; return (T)&x; }
int unknown_address(int x) { return (Unseen)&x; }
