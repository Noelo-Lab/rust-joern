struct buffers { int *first; int *second; };
struct segment { struct buffers buffers; };

int nested_designator(int x)
{
    struct segment segments[] = { { .buffers.first = 0, .buffers.second = 0 } };
    if (x) return 1;
    return 0;
}
