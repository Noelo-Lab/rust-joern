int asm_block(int x)
{
    __asm { MSR.W BASEPRI, R3 }
    x++;
    return x;
}

int after_asm_block(int x)
{
    if (x) return 1;
    return 0;
}
