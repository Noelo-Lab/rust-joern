long brace_operand_in_nested_condition(long a, long b)
{
    long r = 0;
    if (b)
    {
        r = a * b;
        if ((a * b) >> 0x40 != {0})
        {
            g(1);
            return 0;
        }
        if (r)
            return r / b;
    }
    return r;
}

long brace_operand_in_condition(long a)
{
    if (a != {0})
    {
        g(a);
        return 0;
    }
    return a;
}
