int call(int x);
int standalone(int x) { x++; [[__fallthrough__]]; return x; }
int scalar_if(int x) { if (x) [[__fallthrough__]]; return x; }
int scalar_else(int x) { if (x) x++; else [[__fallthrough__]]; return x; }
int label_attribute(int x) { here: [[__fallthrough__]]; return x; }
int scalar_while(int x) { while (x) [[__fallthrough__]]; return x; }
int scalar_do(int x) { do [[__fallthrough__]]; while (x); return x; }
int local_prefix(int x) { [[__maybe_unused__]] int y=call(x); return y; }
int local_suffix(int x) { int y [[__maybe_unused__]]=call(x); return y; }
int expression_prefix(int x) { [[unknown]] call(x); return x; }
int control_prefix(int x) { [[unknown]] if (x) x++; return x; }
int bracket_switch(int x) { switch(x){case 1:x++;[[__fallthrough__]];default:x--;} return x; }
