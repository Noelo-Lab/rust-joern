typedef long I64;
int work(int);
int primitive_once(int n)(int n) { if (n) return work(n); return 0; }
int after_primitive_once(int n) { return n; }
int int primitive_twice(int n)(int n) { if (n) return work(n); return 0; }
int after_primitive_twice(int n) { return n; }
void void void_twice(int n)(int n) { if (n) work(n); }
int after_void_twice(int n) { return n; }
long long integer_twice(int n)(int n) { if (n) return work(n); return 0; }
int after_integer_twice(int n) { return n; }
long double long double floating_twice(int n)(int n) { if (n) return work(n); return 0; }
int after_floating_twice(int n) { return n; }
long Unbound unknown_second(int n)(int n) { if (n) return work(n); return 0; }
int after_unknown_second(int n) { return n; }
long I64 alias_second(int n)(int n) { if (n) return work(n); return 0; }
int after_alias_second(int n) { return n; }
Unbound unknown_once(int n)(int n) { if (n) return work(n); return 0; }
int after_unknown_once(int n) { return n; }
long Unbound unknown_marker(int n)(int n) marker_unknown(int n) { if (n) return work(n); return 0; }
int after_unknown_marker(int n) { return n; }
int int primitive_marker(int n)(int n) marker_primitive(int n) { if (n) return work(n); return 0; }
int after_primitive_marker(int n) { return n; }
long double long double direct_marker(int n) marker_direct(int n) { if (n) return work(n); return 0; }
int after_direct_marker(int n) { return n; }
int (*valid_pointer_return(int n))(int) { return 0; }
int after_valid_pointer_return(int n) { return n; }
