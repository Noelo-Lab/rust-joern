int repeated_top_asm(int *result,int x) {
  __asm { mov eax,ebx }
  *result=x;
  top_first(x)=1;
  __asm { mov eax,ecx }
  *(result+1)=x;
  top_second(x)=2;
  __asm { mov eax,edx }
  *(result+2)=x;
  top_third(x);
  return x;
}
int after_top(void){return 1;}
int repeated_control_asm(int *result,int x) {
  if(x) {
    __asm { mov eax,ebx }
    *result=x;
    first_inside(x)=1;
    __asm { mov eax,ecx }
    *(result+1)=x;
    second_inside(x)=2;
  }
  after_inner(x);
  return x;
}
int after_control(void){return 2;}
