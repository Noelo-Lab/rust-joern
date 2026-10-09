int known(int);
int missing_multiply_index(int x){int v13[16];int *v23;known(x);v23=&v13[2*/* unsupported instruction */];known(x);return x;}
int missing_multiply_index_sum(int x){int v13[16];int *v23;known(x);v23=&v13[1+2*/* unsupported instruction */];known(x);return x;}
int valid_multiply_index(int x){int v13[16];int *v23;known(x);v23=&v13[2*x];known(x);return x;}
int missing_multiply_scalar(int x){int v;known(x);v=2*/* unsupported instruction */;known(x);return x;}
int valid_multiply_scalar(int x){int v;known(x);v=2*x;known(x);return x;}
int empty_index_lvalue(int x){int v16;known(x);(&v16)[/* unsupported instruction */]=x;known(x);return x;}
int valid_index_lvalue(int x){int v16;known(x);(&v16)[x]=x;known(x);return x;}
int memory_concat(int v16,int v22,int v32){known(v32);*((unsigned int *)&(&v16)[v22])=v32 CONCAT v32;known(v32);return v32;}
int valid_memory_assignment(int v16,int v22,int v32){known(v32);*((unsigned int *)&(&v16)[v22])=v32 ^ v32;known(v32);return v32;}
int plain_concat_assignment(int x,int v32){known(x);x=v32 CONCAT v32;known(x);return x;}
int valid_plain_assignment(int x,int v32){known(x);x=v32 ^ v32;known(x);return x;}
int brace_literal_ge(int x,int y){if(!({0} >= y))known(x);return x;}
int valid_ge_operand(int x,int y){if(!(0 >= y))known(x);return x;}
int valid_gnu_literal_ge(int x,int y){if(!(({0;}) >= y))known(x);return x;}
int valid_gnu_call_ge(int x,int y){if(!(({known(x);x+1;}) >= y))known(x);return x;}
int after_control(int x){return known(x);}
