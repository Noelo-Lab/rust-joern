namespace N {
struct A {
    int value;
    int non_const() { return value; }
    int constant() const { return value; }
    int non_const_assign(int x) { value = x; return value; }
    int constant_lambda() const { auto f = [this]() { return value; }; return f(); }
    int non_const_lambda() { auto f = [this]() { return value; }; return f(); }
};
}
