namespace demo { template<class T> struct Box { static int get(int); }; }
int tick();
int sink(int);
int f(int x) {
    if (x && demo::Box<int>::get(tick())) return sink(x);
    while (x > 0) {
        x = demo::Box<int>::get(x - 1);
        if (!x) break;
    }
    return x;
}
