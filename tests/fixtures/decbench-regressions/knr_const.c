int knr_integer(x)
    int x;
{
    if (x) return 1;
    return 0;
}

int knr_const(p)
    const char *p;
{
    if (*p) return 1;
    return 0;
}
