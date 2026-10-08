int by_value(int x) { auto f = [x](int y) { return x + y; }; return f(x); }
int by_reference(int x) { auto f = [&x]() { x = 3; return x; }; return f(); }
int empty_capture(int x) { auto f = [](int y) -> int { return y + 1; }; return f(x); }
int implicit_value(int x) { auto f = [=](int y) { return x + y; }; return f(x); }
int implicit_reference(int x) { auto f = [&](int y) { x += y; return x; }; return f(x); }
int immediate(int x) { return [](int y) -> int { return y + 1; }(x); }
int bracketed(int x) { auto f = [x](int y) { return x + y; }; return (f)(x); }
int alias(int x) { auto f = [x](int y) { return x + y; }; auto &g = f; return g(x); }
int nested(int x) { auto f = [x]() { auto g = [x]() { return x; }; return g(); }; return f(); }
int no_result(int x) { auto f = [&x]() mutable { x++; }; f(); return x; }
int with_local(int x) { int z = x; auto f = [z](int y) { int n = y; return z + n; }; return f(x); }
