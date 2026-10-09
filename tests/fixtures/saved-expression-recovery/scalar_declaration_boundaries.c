void work(int x);
int suffix_compound(int x,int s,int y){x u>>=8;return x;}
int suffix_scalar_if(int x,int s,int y){if(x)x u>>=8;return x;}
int suffix_scalar_else(int x,int s,int y){if(x)work(x);else x u>>=8;return x;}
int suffix_scalar_while(int x,int s,int y){while(x)x u>>=8;return x;}
int suffix_scalar_for(int x,int s,int y){int i=0;for(i=0;i<x;++i)x u>>=8;return x;}
int suffix_scalar_do(int x,int s,int y){do x u>>=8;while(x);return x;}
int suffix_braced_if(int x,int s,int y){if(x){x u>>=8;}return x;}
int suffix_braced_while(int x,int s,int y){while(x){x u>>=8;}return x;}
int suffix_braced_for(int x,int s,int y){int i=0;for(i=0;i<x;++i){x u>>=8;}return x;}
int suffix_braced_do(int x,int s,int y){do{x u>>=8;}while(x);return x;}
int ordinary_compound(int x,int s,int y){int u;return x;}
int ordinary_scalar_if(int x,int s,int y){if(x)int u;return x;}
int ordinary_scalar_else(int x,int s,int y){if(x)work(x);else int u;return x;}
int ordinary_scalar_while(int x,int s,int y){while(x)int u;return x;}
int ordinary_scalar_for(int x,int s,int y){int i=0;for(i=0;i<x;++i)int u;return x;}
int ordinary_scalar_do(int x,int s,int y){do int u;while(x);return x;}
int ordinary_braced_if(int x,int s,int y){if(x){int u;}return x;}
int ordinary_braced_while(int x,int s,int y){while(x){int u;}return x;}
int ordinary_braced_for(int x,int s,int y){int i=0;for(i=0;i<x;++i){int u;}return x;}
int ordinary_braced_do(int x,int s,int y){do{int u;}while(x);return x;}
int initialized_compound(int x,int s,int y){int u=8;return x;}
int initialized_scalar_if(int x,int s,int y){if(x)int u=8;return x;}
int initialized_scalar_else(int x,int s,int y){if(x)work(x);else int u=8;return x;}
int initialized_scalar_while(int x,int s,int y){while(x)int u=8;return x;}
int initialized_scalar_for(int x,int s,int y){int i=0;for(i=0;i<x;++i)int u=8;return x;}
int initialized_scalar_do(int x,int s,int y){do int u=8;while(x);return x;}
int initialized_braced_if(int x,int s,int y){if(x){int u=8;}return x;}
int initialized_braced_while(int x,int s,int y){while(x){int u=8;}return x;}
int initialized_braced_for(int x,int s,int y){int i=0;for(i=0;i<x;++i){int u=8;}return x;}
int initialized_braced_do(int x,int s,int y){do{int u=8;}while(x);return x;}
int initializer_suffix_compound(int x,int s,int y){int u=x*s y;return x;}
int initializer_suffix_scalar_if(int x,int s,int y){if(x)int u=x*s y;return x;}
int initializer_suffix_scalar_else(int x,int s,int y){if(x)work(x);else int u=x*s y;return x;}
int initializer_suffix_scalar_while(int x,int s,int y){while(x)int u=x*s y;return x;}
int initializer_suffix_scalar_for(int x,int s,int y){int i=0;for(i=0;i<x;++i)int u=x*s y;return x;}
int initializer_suffix_scalar_do(int x,int s,int y){do int u=x*s y;while(x);return x;}
int initializer_suffix_braced_if(int x,int s,int y){if(x){int u=x*s y;}return x;}
int initializer_suffix_braced_while(int x,int s,int y){while(x){int u=x*s y;}return x;}
int initializer_suffix_braced_for(int x,int s,int y){int i=0;for(i=0;i<x;++i){int u=x*s y;}return x;}
int initializer_suffix_braced_do(int x,int s,int y){do{int u=x*s y;}while(x);return x;}
int unknown_type_initialized_compound(int x,int s,int y){x u=8;return x;}
int unknown_type_initialized_scalar_if(int x,int s,int y){if(x)x u=8;return x;}
int unknown_type_initialized_scalar_else(int x,int s,int y){if(x)work(x);else x u=8;return x;}
int unknown_type_initialized_scalar_while(int x,int s,int y){while(x)x u=8;return x;}
int unknown_type_initialized_scalar_for(int x,int s,int y){int i=0;for(i=0;i<x;++i)x u=8;return x;}
int unknown_type_initialized_scalar_do(int x,int s,int y){do x u=8;while(x);return x;}
int unknown_type_initialized_braced_if(int x,int s,int y){if(x){x u=8;}return x;}
int unknown_type_initialized_braced_while(int x,int s,int y){while(x){x u=8;}return x;}
int unknown_type_initialized_braced_for(int x,int s,int y){int i=0;for(i=0;i<x;++i){x u=8;}return x;}
int unknown_type_initialized_braced_do(int x,int s,int y){do{x u=8;}while(x);return x;}
int ordinary_shift_positive(int x){x>>=8;return x;}
int after_control(int x){return x;}
