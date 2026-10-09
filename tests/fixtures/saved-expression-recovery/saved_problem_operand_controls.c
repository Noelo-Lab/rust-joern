int work(int);
int ellipsis_condition(int n){if(...)work(n);return n;}
int ellipsis_return(int n){return ...;}
int ellipsis_argument(int n){return work(...);}
int ellipsis_initializer(int n){int x=...;work(n);return n;}
int ellipsis_scalar(int n){if(n)n=...;return n;}
int ellipsis_block(int n){if(n){n=...;work(n);}return n;}
int valid_range_designator(int n){int x[5]={[1 ... 3]=n};return x[1];}
int valid_function_pointer_cast(void*p,int n){return ((int(*)(int,...))p)(n);}
int hash_return(int n){return n#1;}
int hash_condition(int n){if(n#1)work(n);return n;}
int identifier_argument(int n){return work(out result);}
int identifier_argument_pair(int n){return work(n result);}
int valid_sizeof_argument(int n){return work(sizeof n);}
int valid_extension_argument(int n){return work(__extension__ n);}
int width_assignment(int n){n=n/32 divisor;work(n);return n;}
int width_bare_assignment(int n){n=32 divisor;work(n);return n;}
int width_plus_assignment(int n){n=n+32 divisor;work(n);return n;}
int width_return(int n){return n/32 divisor;}
int width_argument(int n){return work(n/32 divisor);}
int valid_literal_hash(int n){return work("#");}
