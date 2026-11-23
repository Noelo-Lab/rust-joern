template <class E>
struct Span {
    E *first;
    int length;
};

namespace store {
template <class T>
class Buffer {
public:
    struct Cursor {
        int at;
        bool same(const Cursor &other) const { return at == other.at; }
    };
    Buffer(const Buffer &other) {
        count = 0;
        for (int i = 0; i < other.count; i++) {
            if (other.items[i] != 0) {
                count++;
            }
        }
    }
    Buffer(Buffer &&other) {
        count = other.count;
        if (count > 0) {
            other.count = 0;
        }
    }
    Buffer(Span<T> values) {
        count = values.length;
    }
    Buffer(Span<int> sizes, int extra) {
        count = extra;
        if (sizes.length > extra) {
            count = sizes.length;
        }
    }
    Buffer &operator=(const Buffer &other) {
        if (this != &other) {
            count = other.count;
        }
        return *this;
    }
    Buffer &operator=(Buffer &&other) {
        count = other.count;
        return *this;
    }
    int total(const Buffer<int> &other, Cursor at) const {
        return other.count + count + at.at;
    }
    Buffer *self() { return this; }
    T *items;
    int count;
};
}
