int global = 7;
int uses_global(int x) { global = x; return global; }
int reads_global(void) { return global; }
int captures_value(int x) { auto f = [x](int y) { return x + y; }; return f(2); }
int captures_ref(int x) { auto f = [&x]() { x = 3; return x; }; return f(); }
int nested_capture(int x) { auto f = [x]() { auto g = [x]() { return x; }; return g(); }; return f(); }
