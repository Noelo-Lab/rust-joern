int while_comma(int x) {
    while (x = x - 1, x) { x--; }
    return x;
}
int if_comma(int x) {
    if (x = x - 1, x) { x++; }
    return x;
}
int do_comma(int x) {
    do { x--; } while (x = x - 1, x);
    return x;
}
int for_comma(int x) {
    for (; x = x - 1, x; x--) { x--; }
    return x;
}
int switch_comma(int x) {
    switch (x = x - 1, x) {
        case 1: x++; break;
        default: x--;
    }
    return x;
}
int return_comma(int x) {
    return (x = x - 1, x);
}
