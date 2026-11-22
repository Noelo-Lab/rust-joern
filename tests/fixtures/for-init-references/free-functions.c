typedef int (*getter)(void);

int first(void);
int last(void);
int step(void);

int count(int limit) {
    int total = 0;
    for (int it = first(), last = limit; it != last; it += step()) {
        total += last - it;
    }
    return total;
}

getter after(void) {
    for (int last = first(); last > 0; --last) {
        if (last == step()) {
            break;
        }
    }
    return last;
}
