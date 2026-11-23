struct Table {
    void grow() {
        if (limit > 0) resize(limit);
    }
    void reset() {
        {
            resize(count);
        }
    }
    void resize(int size);
    int count;
    int limit;
};
