struct Counter {
    Counter() = default;
    Counter(const Counter&) = delete;
    Counter& operator=(const Counter&) = default;
    Counter& operator=(Counter&&) = delete;
    ~Counter() = default;
    Counter& operator+=(int step);
    int value() const { return count; }
    int count = 0;
};

struct Shape {
    virtual ~Shape() = default;
    virtual int area() const = 0;
};

struct Holder {
    Holder();
};

Holder::Holder() = default;

int bump(Counter& counter) {
    counter += 2;
    return counter.value();
}
