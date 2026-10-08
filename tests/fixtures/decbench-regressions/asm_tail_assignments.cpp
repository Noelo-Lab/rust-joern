int interrupted(int x) {
 __asm { mov eax,ebx }
 tail_identifier(x)=3;
 tail_zero()=x;
 tail_number(3)=x;
}
int after(int x) { return x; }
