int terminal_label(int x)
{
    for (int i = 0; i < x; ++i) {
        if (i) goto next;
        x++;
next:
    }
    return x;
}
