struct Item {
    Item(const Item &other);
};

void construct_if(Item *items, const Item *source, int count) {
    if (count)
        new(items + count) Item(source[count]);
    count--;
}

void construct_for(Item *items, const Item *source, int count) {
    for (int slot = 0; slot < count; slot++)
        new(items + slot) Item(source[slot]);
}

void construct_while(Item *items, const Item *source, int count) {
    while (count-- > 0)
        new(items + count) Item(source[count]);
}

void construct_else(Item *items, const Item *source, int count) {
    if (!count) {
        return;
    } else
        new(items) Item(source[0]);
}
