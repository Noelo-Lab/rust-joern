int ordinary(int x);
int words(int new, int delete, int throw, int noexcept) { try(new); return delete ? throw + noexcept : ordinary(new); }
int generic(int x) { return _Generic(x, int: ordinary(x), default: ordinary(x+1)); }
int generic_type(int x) { return _Generic(x, const __typeof__(x) *: ordinary(x), default: ordinary(x+1)); }
int generic_cond(int x) { if (_Generic(x, int: x&&ordinary(x), default: x||ordinary(x))) return 1; return 0; }
int generic_init(int x) { int y=_Generic(x, int: ordinary(x), default: ordinary(x+1)); return y; }
int generic_nested(int x) { return x&&_Generic(x, int: x||ordinary(x), default: x&&ordinary(x)); }
