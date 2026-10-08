struct Counter {
    int value;
    int get() const { return value; }
    int step(int n) { for (int i = 0; i < n; ++i) value += i; return value; }
    int shadow(int value) { return value; }
    int later_get() const { return later; }
    int capture() { auto f = [this]() { return value; }; return f(); }
    int captured_shadow(int value) { auto f = [value]() { return value; }; return f(); }
    int later;
    static int shared;
    static int static_get() { return shared; }
};
