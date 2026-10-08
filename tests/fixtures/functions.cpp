namespace bench {
int branch(int x) { if (x) return 1; return 0; }
struct Counter {
    int value;
    int get() const { return value; }
    int step(int n) { for (int i = 0; i < n; ++i) value += i; return value; }
};
int scope(int x) { if (int y = x) return y; return 0; }
int loop(int x) { for (int v : {1, 2, 3}) x += v; return x; }
int exception(int x) { try { if (x) throw x; x++; } catch (int e) { x = e; } return x; }
template <class T> T choose(T a, T b) { return a > b ? a : b; }
}
int main(void) { bench::Counter c; c.value=0; return c.step(3); }
