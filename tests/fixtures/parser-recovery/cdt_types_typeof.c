struct device { int x; };
int foo(int x);
int types_typeof(int x) { return __builtin_types_compatible_p(typeof(x), typeof(int)); }
int types_typeof_pointer(struct device *dev) { return __builtin_types_compatible_p(typeof(dev), typeof(struct device *)); }
int types_typeof_branch(int x) { return __builtin_types_compatible_p(typeof(x && foo(x)), typeof(int)); }
int types_assert(int x) { _Static_assert(!__builtin_types_compatible_p(typeof(x), typeof(int)), "test"); return x; }
