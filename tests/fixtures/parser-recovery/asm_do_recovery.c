int asm_do_plain(int x) {
    do { x+=1; __asm { mov eax, ebx } x+=2; x+=3; } while (x);
    return x;
}
int asm_do_prefix(int x) {
    x+=10;
    do { x+=1; __asm { mov eax, ebx } x+=2; x+=3; } while (x);
    return x;
}
int asm_do_if(int x) {
    if (x) {
        do { x+=1; __asm { mov eax, ebx } x+=2; x+=3; } while (x);
    }
    return x;
}
int asm_do_two_if(int x) {
    if (x) {
        do { x+=1; __asm { mov eax, ebx } x+=2; x+=3; __asm { mov ecx, edx } x+=4; x+=5; } while (x);
    }
    return x;
}
int asm_do_post_return(int x) {
    do { x+=1; __asm { mov eax, ebx } return x; x+=3; } while (x);
    return x;
}
int asm_do_post_control(int x) {
    do { x+=1; __asm { mov eax, ebx } if (x) x+=2; x+=3; } while (x);
    return x;
}
int asm_do_post_braced_control(int x) {
    do { x+=1; __asm { mov eax, ebx } if (x) { x+=2; } x+=3; } while (x);
    return x;
}
int after_asm_do(int x) { if (x) return 1; return 0; }
