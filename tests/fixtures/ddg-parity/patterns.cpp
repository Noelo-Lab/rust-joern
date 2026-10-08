/* C++ contexts exercising scope, parameters, captures, and access paths. */
int cpp_global = 5;
namespace Config { int value = 7; int use(int x) { value = x; return value; } }
struct Box {
    int value;
    int set(int x) { value = x; return value; }
    int get() const { return value; }
    static int shared;
};
int Box::shared = 2;
int scoped_global(int x) { Config::value = x; return Config::value; }
int class_global(int x) { Box::shared = x; return Box::shared; }
int cpp_global_read() { return cpp_global; }
int implicit_member(Box b, int x) { return b.set(x) + b.get(); }
int explicit_member(Box *b, int x) { b->value = x; return b->value; }
int lambda_copy(int x) { auto f = [x](int y) { return x + y; }; return f(x); }
int lambda_reference(int x) { auto f = [&x](int y) { x += y; return x; }; return f(1) + x; }
int lambda_mutable(int x) { auto f = [x]() mutable { return ++x; }; return f(); }
int lambda_nested(int x) { auto f = [x]() { auto g = [x]() { return x + 1; }; return g(); }; return f(); }
int reference_store(int &x, int y) { int &alias = x; alias = y; return x; }
int reference_fields(Box &b, int x) { b.value = x; return b.value; }
int pointer_alias(int *p, int x) { int *q = p; *q = x; return *p; }
int exception_paths(int x) { int y = 1; try { if (x) throw x; y = x; } catch (int e) { y = e + 1; } return y; }
int overloaded(int x) { return x; }
int overloaded(double x) { return (int)x; }
int overload_call(int x) { return overloaded(x); }
int method_pointer(Box *b, int x) { int (Box::*m)(int) = &Box::set; return (b->*m)(x); }
int default_argument(int x = 9) { return x; }
int constant_condition(int x) { return true ? x : 0; }
