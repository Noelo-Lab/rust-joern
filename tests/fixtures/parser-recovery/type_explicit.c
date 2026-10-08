typedef int T;
int explicit_address(int x) { return (T)&x; }
int explicit_plus(int x) { return (T)+x; }
int explicit_minus(int x) { return (T)-x; }
int explicit_multiply(int *x) { return (T)*x; }
int shadow_address(int T, int x) { return (T)&x; }
int shadow_plus(int T, int x) { return (T)+x; }
int shadow_minus(int T, int x) { return (T)-x; }
int shadow_multiply(int T, int x) { return (T)*x; }
