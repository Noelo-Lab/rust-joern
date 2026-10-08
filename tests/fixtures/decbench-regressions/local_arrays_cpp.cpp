int g(int x);
int array_constant(int x) { int a[3]; return x; }
int array_uninitialized_bound(int x) { int a[x && g(x)]; return x; }
int array_initialized_bound(int x) { int a[x && g(x)] = {1}; return x; }
int arrays_two_bounds(int x) { int a[x && g(x)], b[x || g(x)]; return x; }
int arrays_two_initializers(int x) { int a[x && g(x)] = {g(1)}, b[x || g(x)] = {g(2)}; return x; }
int array_constant_initializer(int x) { int a[3] = {x && g(x)}; return x; }
int array_multidimensional(int x) { int a[x && g(x)][x || g(x)]; return x; }
int array_unsized_partial(int x) { int a[][3] = {{1}, {2}}; return x; }
int array_unsized_vla_partial(int x) { int a[][x && g(x)] = {{1}, {2}}; return x; }
int array_unsized(int x) { int a[] = {g(x), g(2)}; return x; }
int array_no_initializer_unsized(int x) { int a[]; return x; }
int array_mixed_declarators(int x) { int a[x && g(x)] = {1}, b = g(2), c[x || g(x)]; return x; }
