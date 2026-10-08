typedef void code(void);

int casted_indirect_call(int *p)
{
    (*(code *)(p))();
    if (*p) return 1;
    return 0;
}
