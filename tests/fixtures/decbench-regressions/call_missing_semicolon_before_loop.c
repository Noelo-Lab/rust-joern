int micros_isr(unsigned int a0, unsigned int i, unsigned int a2)
{
    unsigned int v1;
    unsigned int v0;

    v0 = v1;
    __unsupported_jumpkind_Ijk_NoDecode()
    for (; i; i = 0)
    {
        if (g_e000e010 & 0xffffff & 0x10000)
            g_2000be48 = 1;
    }
    __unsupported_jumpkind_Ijk_NoDecode()
}

int consecutive_calls_before_loop(unsigned int a0)
{
    int v0;

    g_20000b98 = 1;
    __unsupported_jumpkind_Ijk_NoDecode()
    __unsupported_jumpkind_Ijk_NoDecode()
    while (1)
    {
        v0 = sub_8001519();
        __unsupported_jumpkind_Ijk_NoDecode()
        if (v0)
            return v0;
    }
}
