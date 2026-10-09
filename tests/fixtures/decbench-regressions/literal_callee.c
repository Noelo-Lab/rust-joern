unsigned int literal_call_after_label(int a0)
{
    unsigned int v0;

    v0 = sub_608255e8(a0, 3238528420);
    if (!v0)
        goto LABEL_60825874;
    return v0;
LABEL_60825874:
    1619153864();
}

int literal_call_in_branch(int x)
{
    if (x)
        1619153864();
    return x;
}

int literal_call_assignment(int x)
{
    if (x)
        x = 5();
    return x;
}
