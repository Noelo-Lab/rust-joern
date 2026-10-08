int asm_prefix(int x) {
    x += 3;
    __asm { MSR.W BASEPRI, R3 }
    x++;
    return x;
}
int after_prefix(int x) { if (x) return 1; return 0; }
int asm_multiple(int x) {
    x += 3;
    __asm { mov eax, ebx }
    x *= 2;
    __asm { mov ecx, edx }
    return x;
}
int after_multiple(int x) { return x + 1; }
int asm_if(int x) {
    x += 3;
    if (x) { __asm { mov eax, ebx } x++; }
    return x;
}
int after_if(int x) { return x + 2; }
int asm_nested(int x) {
    x += 3;
    { __asm { mov eax, ebx } x++; }
    return x;
}
int after_nested(int x) { return x + 3; }
