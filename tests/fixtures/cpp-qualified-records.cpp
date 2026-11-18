namespace trial {
struct Base {
    virtual int read() const = 0;
};
struct Owner {
    struct Nested;
};
struct Owner::Nested : Base {
    int state;
    int read() const override {
        if (state > 0) return state;
        return 0;
    }
};
int after(int x) { return x + 11; }
}
