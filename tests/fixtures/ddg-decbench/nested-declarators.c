typedef int T2;
int unknown_type(int x) { T (y) = x; return y; }
int builtin_type(int x) { int (y) = x; return y; }
int redeclared(int x) { int v0; LOBYTE(v0) = 0; return v0 + x; }
int typedef_type(int x) { T2 (w) = x; return w; }
int doubled(int x) { int ((y)) = x; return y; }
int pointer(int x) { int (*p) = &x; return *p; }
int unknown_pointer(int x) { T (*q) = &x; return *q; }
int array(int x) { int (a[2]) = {x, x}; return a[0]; }
long long partial_words(int a) {
  int v0;
  long long result;
  v0 = -(a == 0);
  LOBYTE(v0) = 0;
  result = (unsigned int)(v0 + 258);
  if (a)
    BYTE1(result) |= 4u;
  return result;
}
