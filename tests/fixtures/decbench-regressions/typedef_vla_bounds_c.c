int g(int x);
int typedef_ternary(int x) { typedef char T[x > 0 ? 1 : -1]; return x; }
int typedef_logical(int x) { typedef char T[x && g(x)]; return x; }
int typedef_unsized(int x) { typedef char T[]; return x; }
int typedef_constant(int x) { typedef char T[2]; return x; }
int typedef_multidim(int x) { typedef char T[x > 0 ? 1 : -1][x && g(x)]; return x; }
int ordinary_ternary(int x) { char T[x > 0 ? 1 : -1]; return x; }
int typedef_scalar(int x) { typedef char T; return x; }
