struct Range {
    int first();
    int last();
    int step();

    bool contains(int value) {
        for (auto it = first(), last = this->last(); it != last; it += step()) {
            if (it == value || last == value) {
                return true;
            }
        }
        return false;
    }

    int span() {
        int total = 0;
        for (int it = first(), last = this->last(); it < last; ++it) {
            total += last - it;
        }
        return total + last();
    }
};
