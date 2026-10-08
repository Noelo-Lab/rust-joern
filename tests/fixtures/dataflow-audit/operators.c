int sizeof_value(int x) { int y = sizeof(x); return x+y; }
int sizeof_return(int x) { return sizeof(x); }
int ptr_size(int *p, int x) { *p = x; return sizeof(*p); }
int ptr_add_zero(int *p, int x) { *p = x; return *(p+0); }
int ptr_inc_zero(int *p, int x) { p[0] = x; return *p; }
int pointer_shift(int *p, int i, int x) { p[1] = x; return *(p+i); }
int nested(int **p, int x) { **p=x; return *(*p); }
int addresses(int *p, int x) { *p=x; return *(&(*p)); }
int const_idx(int *p, int x) { p[1u] = x; return p[1]; }
int string_idx(int *p, int x) { p[0x1] = x; return p[1]; }
int input_shadow(int x) { { int x=1; x++; } return x; }
int shadow(int x) { int y=x; { int y=1; y++; } return y; }
int compound_field(int *p, int x) { p[1] += x; return p[1]; }
